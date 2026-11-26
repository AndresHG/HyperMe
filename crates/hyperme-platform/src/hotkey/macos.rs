//! The macOS hotkey listener: a listen-only Quartz event tap on its own thread, feeding
//! [`Matcher`](super::Matcher). It needs the Input Monitoring permission; without it the tap
//! cannot be created and `start` says so.
//!
//! Listen-only means keys still reach the app in front: Esc too, and Fn, so macOS's own Globe
//! key action (emoji picker or dictation) still runs unless the user sets it to "Do Nothing".

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use core_foundation::runloop::{CFRunLoop, kCFRunLoopCommonModes, kCFRunLoopDefaultMode};
use core_graphics::event::{
    CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
    CallbackResult, EventField,
};
use hyperme_core::config::Hotkey;
use hyperme_core::dictation::Event;
use hyperme_core::traits::{Error, ErrorKind};
use tokio::sync::mpsc;

use super::{Kind, Matcher, RawKey};

/// Whether a keyboard event tap can be created right now: the real test of Input Monitoring.
/// `CGPreflightListenEventAccess` can stay false after the user grants access until the app
/// restarts, while creating a tap already works; the walkthrough uses both.
pub fn can_listen() -> bool {
    CGEventTap::new(
        CGEventTapLocation::Session,
        CGEventTapPlacement::TailAppendEventTap,
        CGEventTapOptions::ListenOnly,
        vec![CGEventType::FlagsChanged],
        |_proxy, _type, _event| CallbackResult::Keep,
    )
    .is_ok()
}

/// Owns the listener thread, which runs for the life of the app.
#[derive(Debug)]
pub struct HotkeyListener {
    _thread: std::thread::JoinHandle<()>,
}

impl HotkeyListener {
    /// Starts listening for `hotkey` and Esc, sending events to `events`.
    pub fn start(hotkey: &Hotkey, events: mpsc::Sender<Event>) -> Result<Self, Error> {
        let matcher = Matcher::new(hotkey)?;
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("hyperme-hotkey".to_owned())
            .spawn(move || run(matcher, events, ready_tx))
            .map_err(|e| Error {
                kind: ErrorKind::Device,
                message: format!("hotkey thread: {e}"),
            })?;
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(HotkeyListener { _thread: thread }),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(Error {
                kind: ErrorKind::Device,
                message: "the hotkey listener stopped".to_owned(),
            }),
        }
    }
}

fn run(
    matcher: Matcher,
    events: mpsc::Sender<Event>,
    ready: std::sync::mpsc::Sender<Result<(), Error>>,
) {
    let matcher = Arc::new(Mutex::new(matcher));
    let disabled = Arc::new(AtomicBool::new(false));
    let send = {
        let events = events.clone();
        // Never blocks the event tap; the state machine drains the channel quickly.
        move |event: Event| {
            let _ = events.try_send(event);
        }
    };
    let tap = {
        let matcher = matcher.clone();
        let disabled = disabled.clone();
        let send = send.clone();
        CGEventTap::new(
            CGEventTapLocation::Session,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::ListenOnly,
            vec![
                CGEventType::FlagsChanged,
                CGEventType::KeyDown,
                CGEventType::KeyUp,
            ],
            move |_proxy, event_type, event| {
                let kind = match event_type {
                    CGEventType::FlagsChanged => Kind::FlagsChanged,
                    CGEventType::KeyDown => Kind::KeyDown,
                    CGEventType::KeyUp => Kind::KeyUp,
                    CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
                        disabled.store(true, Ordering::SeqCst);
                        return CallbackResult::Keep;
                    }
                    _ => return CallbackResult::Keep,
                };
                let raw = RawKey {
                    kind,
                    keycode: event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE)
                        as u16,
                    flags: event.get_flags().bits(),
                    repeat: event.get_integer_value_field(EventField::KEYBOARD_EVENT_AUTOREPEAT)
                        != 0,
                };
                let result = matcher.lock().map(|mut m| m.on(raw));
                if let Ok(Some(event)) = result {
                    send(event);
                }
                CallbackResult::Keep
            },
        )
    };
    let Ok(tap) = tap else {
        let _ = ready.send(Err(Error {
            kind: ErrorKind::Device,
            message: "could not listen for the hotkey. Allow HyperMe in System Settings > \
                      Privacy & Security > Input Monitoring"
                .to_owned(),
        }));
        return;
    };
    let Ok(source) = tap.mach_port().create_runloop_source(0) else {
        let _ = ready.send(Err(Error {
            kind: ErrorKind::Device,
            message: "could not attach the hotkey listener".to_owned(),
        }));
        return;
    };
    // SAFETY: reading Core Foundation's run-loop mode constants, which are immutable statics.
    #[allow(unsafe_code)]
    let (common, default) = unsafe { (kCFRunLoopCommonModes, kCFRunLoopDefaultMode) };
    CFRunLoop::get_current().add_source(&source, common);
    tap.enable();
    let _ = ready.send(Ok(()));
    loop {
        CFRunLoop::run_in_mode(default, Duration::from_secs(1), false);
        // macOS disables a tap that is slow or while secure input is on; turn it back on, and
        // cancel a hold whose release may have been missed.
        if disabled.swap(false, Ordering::SeqCst) {
            if let Ok(Some(event)) = matcher.lock().map(|mut m| m.reset()) {
                send(event);
            }
            tap.enable();
        }
        if events.is_closed() {
            return;
        }
    }
}
