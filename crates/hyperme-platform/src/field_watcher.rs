//! The macOS field watcher (spec 93, slice D7; design.md, "The watcher on macOS"): after a
//! paste, it watches the focused element through an `AXObserver` on a thread of its own, for at
//! most 60 seconds, and follows the rules of [`hyperme_core::field_watch::FieldWatch`]. Between
//! watches there is no thread and no observer, so it costs nothing at rest (AC-16).

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use core_foundation::base::TCFType;
use core_foundation::runloop::{CFRunLoop, CFRunLoopSource, kCFRunLoopDefaultMode};
use core_foundation::string::{CFString, CFStringRef};
use hyperme_core::field_watch::{End, Field, FieldWatch};
use hyperme_core::traits::FieldWatcher;

#[allow(unsafe_code)]
mod ffi {
    use std::ffi::c_void;

    use core_foundation::runloop::CFRunLoopSourceRef;
    use core_foundation::string::CFStringRef;

    pub type Element = *const c_void;
    pub type Observer = *const c_void;
    pub type Callback = unsafe extern "C" fn(Observer, Element, CFStringRef, *mut c_void);

    /// `kAXValueCFRangeType`.
    pub const VALUE_CF_RANGE: u32 = 4;

    #[repr(C)]
    #[derive(Default)]
    pub struct CFRange {
        pub location: isize,
        pub length: isize,
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        pub fn AXUIElementCreateSystemWide() -> Element;
        pub fn AXUIElementCreateApplication(pid: i32) -> Element;
        pub fn AXUIElementCopyAttributeValue(
            element: Element,
            attribute: CFStringRef,
            value: *mut *const c_void,
        ) -> i32;
        pub fn AXUIElementGetPid(element: Element, pid: *mut i32) -> i32;
        pub fn AXObserverCreate(pid: i32, callback: Callback, observer: *mut Observer) -> i32;
        pub fn AXObserverAddNotification(
            observer: Observer,
            element: Element,
            notification: CFStringRef,
            refcon: *mut c_void,
        ) -> i32;
        pub fn AXObserverRemoveNotification(
            observer: Observer,
            element: Element,
            notification: CFStringRef,
        ) -> i32;
        pub fn AXObserverGetRunLoopSource(observer: Observer) -> CFRunLoopSourceRef;
        pub fn AXValueGetValue(value: *const c_void, kind: u32, out: *mut c_void) -> u8;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        pub fn CFRelease(object: *const c_void);
        pub fn CFGetTypeID(object: *const c_void) -> usize;
        pub fn CFStringGetTypeID() -> usize;
    }

    #[link(name = "Carbon", kind = "framework")]
    unsafe extern "C" {
        /// True while any process has secure input on.
        pub safe fn IsSecureEventInputEnabled() -> u8;
    }
}

/// How long to wait for the paste to land in the field, and how often to look.
const LAND_TRIES: u32 = 6;
const LAND_EVERY: Duration = Duration::from_millis(150);

/// What the observer callback saw, on the watch's own thread.
#[derive(Default)]
struct Seen {
    changed: AtomicBool,
    focus: AtomicBool,
}

/// The running watch, for [`FieldWatcher::stop`].
struct Running {
    stop: Arc<AtomicBool>,
    /// The watch thread's run loop while it runs; `None` once it has finished.
    run_loop: Arc<Mutex<Option<CFRunLoop>>>,
}

impl Running {
    fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(run_loop) = &*self.run_loop.lock().unwrap_or_else(PoisonError::into_inner) {
            run_loop.stop();
        }
    }
}

/// Watches the field HyperMe pasted into (R-16, R-20, R-21).
#[derive(Default)]
pub struct MacFieldWatcher {
    running: Mutex<Option<Running>>,
}

impl std::fmt::Debug for MacFieldWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MacFieldWatcher").finish_non_exhaustive()
    }
}

impl MacFieldWatcher {
    pub fn new() -> Self {
        Self::default()
    }
}

impl FieldWatcher for MacFieldWatcher {
    fn watch(&self, pasted: String, done: Box<dyn FnOnce(String, String) + Send>) {
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(before) = running.take() {
            before.stop();
        }
        let stop = Arc::new(AtomicBool::new(false));
        let run_loop = Arc::new(Mutex::new(None));
        let (thread_stop, thread_loop) = (Arc::clone(&stop), Arc::clone(&run_loop));
        let spawned = std::thread::Builder::new()
            .name("field-watch".to_owned())
            .stack_size(256 * 1024)
            .spawn(move || {
                if let Some((pasted, now)) = run(&pasted, &thread_stop, &thread_loop) {
                    done(pasted, now);
                }
            });
        if spawned.is_ok() {
            *running = Some(Running { stop, run_loop });
        }
    }

    fn stop(&self) {
        if let Some(running) = self
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            running.stop();
        }
    }
}

fn secure_input() -> bool {
    ffi::IsSecureEventInputEnabled() != 0
}

/// An accessibility object this code owns, released once.
struct Owned(*const c_void);

impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: created or copied here with a +1 count, released exactly once.
            #[allow(unsafe_code)]
            unsafe {
                ffi::CFRelease(self.0)
            };
        }
    }
}

/// Copies one attribute of `element`, or `None`.
fn attribute(element: ffi::Element, name: &'static str) -> Option<Owned> {
    let name = CFString::from_static_string(name);
    let mut value: *const c_void = std::ptr::null();
    // SAFETY: the element is valid for the call; a returned value is owned by the caller.
    #[allow(unsafe_code)]
    let error = unsafe {
        ffi::AXUIElementCopyAttributeValue(element, name.as_concrete_TypeRef(), &mut value)
    };
    (error == 0 && !value.is_null()).then_some(Owned(value))
}

/// An attribute that is a string.
fn string_attribute(element: ffi::Element, name: &'static str) -> Option<String> {
    let value = attribute(element, name)?;
    // SAFETY: the value is a live CF object; it is wrapped with a retain so `value` still
    // releases its own count.
    #[allow(unsafe_code)]
    unsafe {
        if ffi::CFGetTypeID(value.0) != ffi::CFStringGetTypeID() {
            return None;
        }
        Some(CFString::wrap_under_get_rule(value.0 as CFStringRef).to_string())
    }
}

/// The insertion point, in UTF-16 units.
fn caret(element: ffi::Element) -> Option<usize> {
    let value = attribute(element, "AXSelectedTextRange")?;
    let mut range = ffi::CFRange::default();
    // SAFETY: AXValueGetValue checks the value's kind and writes a CFRange.
    #[allow(unsafe_code)]
    let ok = unsafe {
        ffi::AXValueGetValue(
            value.0,
            ffi::VALUE_CF_RANGE,
            (&raw mut range).cast::<c_void>(),
        )
    };
    (ok != 0 && range.location >= 0).then(|| (range.location + range.length) as usize)
}

/// The text of the field, or `None` while secure input is on (R-20).
fn read(element: ffi::Element) -> Option<String> {
    if secure_input() {
        return None;
    }
    string_attribute(element, "AXValue")
}

/// Whether the element is a secure text field (R-20).
fn is_secure(element: ffi::Element) -> bool {
    let secure = Some("AXSecureTextField");
    string_attribute(element, "AXRole").as_deref() == secure
        || string_attribute(element, "AXSubrole").as_deref() == secure
}

/// Records what the observer saw; runs on the watch thread, inside its run loop.
#[allow(unsafe_code)]
unsafe extern "C" fn noticed(
    _observer: ffi::Observer,
    _element: ffi::Element,
    notification: CFStringRef,
    refcon: *mut c_void,
) {
    if refcon.is_null() || notification.is_null() {
        return;
    }
    // SAFETY: `refcon` points to the `Seen` that `run` keeps alive until the observer is gone,
    // and the notification name is valid for the call.
    let (seen, name) = unsafe {
        (
            &*(refcon as *const Seen),
            CFString::wrap_under_get_rule(notification).to_string(),
        )
    };
    if name == "AXValueChanged" {
        seen.changed.store(true, Ordering::SeqCst);
    } else {
        seen.focus.store(true, Ordering::SeqCst);
    }
}

/// One watch, start to end, on the current thread. Returns the pasted text and what its
/// stretch holds at the end, when the user changed it.
fn run(
    pasted: &str,
    stop: &AtomicBool,
    run_loop: &Mutex<Option<CFRunLoop>>,
) -> Option<(String, String)> {
    // SAFETY: the system-wide element is created here and owned by `Owned`.
    #[allow(unsafe_code)]
    let system = Owned(unsafe { ffi::AXUIElementCreateSystemWide() });
    // Wait for the paste to land: the keystroke is posted before the app handles it.
    let mut found = None;
    for _ in 0..LAND_TRIES {
        std::thread::sleep(LAND_EVERY);
        if stop.load(Ordering::SeqCst) {
            return None;
        }
        let element = attribute(system.0, "AXFocusedUIElement")?;
        let field = if is_secure(element.0) || secure_input() {
            // R-20: never read a secure field or a field under secure input.
            Field {
                secure_field: true,
                ..Field::default()
            }
        } else {
            Field {
                secure_field: false,
                secure_input: secure_input(),
                value: read(element.0),
                caret_utf16: caret(element.0),
            }
        };
        if field.secure_field {
            return None;
        }
        if let Some(watch) = FieldWatch::start(pasted, field, Instant::now()) {
            found = Some((element, watch));
            break;
        }
    }
    let (element, mut watch) = found?;

    let mut pid = 0;
    let mut observer: ffi::Observer = std::ptr::null();
    // SAFETY: plain calls on a valid element; the observer is owned below.
    #[allow(unsafe_code)]
    let created = unsafe {
        ffi::AXUIElementGetPid(element.0, &mut pid) == 0
            && ffi::AXObserverCreate(pid, noticed, &mut observer) == 0
    };
    if !created || observer.is_null() {
        return None;
    }
    let observer = Owned(observer);
    // SAFETY: as above.
    #[allow(unsafe_code)]
    let app = Owned(unsafe { ffi::AXUIElementCreateApplication(pid) });
    let seen = Box::new(Seen::default());
    let refcon = (&raw const *seen).cast_mut().cast::<c_void>();
    let notes: [(ffi::Element, &'static str); 3] = [
        (element.0, "AXValueChanged"),
        (app.0, "AXFocusedUIElementChanged"),
        (app.0, "AXApplicationDeactivated"),
    ];
    let current = CFRunLoop::get_current();
    // SAFETY: the observer, elements and `seen` all outlive the registrations, which are
    // removed before any of them is dropped; the run loop source is retained by the wrapper.
    #[allow(unsafe_code)]
    let source = unsafe {
        for (on, name) in notes {
            let name = CFString::from_static_string(name);
            ffi::AXObserverAddNotification(observer.0, on, name.as_concrete_TypeRef(), refcon);
        }
        CFRunLoopSource::wrap_under_get_rule(ffi::AXObserverGetRunLoopSource(observer.0))
    };
    // SAFETY: the default mode is a static CF string.
    #[allow(unsafe_code)]
    let mode = unsafe { kCFRunLoopDefaultMode };
    current.add_source(&source, mode);
    *run_loop.lock().unwrap_or_else(PoisonError::into_inner) = Some(current.clone());

    while watch.ended().is_none() {
        if stop.load(Ordering::SeqCst) {
            watch.end(End::NextDictation, || read(element.0));
            break;
        }
        let wait = watch.next_wake().saturating_duration_since(Instant::now());
        CFRunLoop::run_in_mode(mode, wait.max(Duration::from_millis(10)), true);
        let now = Instant::now();
        if seen.focus.swap(false, Ordering::SeqCst) {
            watch.end(End::FocusChanged, || None);
            break;
        }
        if seen.changed.swap(false, Ordering::SeqCst) {
            watch.changed(now);
        }
        watch.wake(now, || read(element.0));
    }

    *run_loop.lock().unwrap_or_else(PoisonError::into_inner) = None;
    current.remove_source(&source, mode);
    // SAFETY: the registrations made above, removed with the same arguments.
    #[allow(unsafe_code)]
    unsafe {
        for (on, name) in notes {
            let name = CFString::from_static_string(name);
            ffi::AXObserverRemoveNotification(observer.0, on, name.as_concrete_TypeRef());
        }
    }
    drop(seen);
    watch.finish()
}
