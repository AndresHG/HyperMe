//! Presence and the clock on macOS (specs/92-daily-tasks, design.md, "Presence on macOS"):
//! system notifications turned into the scheduler's [`PresenceEvent`]s, the seconds since the
//! last input, and a [`Clock`] on the wall clock in the Mac's current zone.
//!
//! The notification names were checked against the macOS 26 SDK headers on 26 November 2026:
//! `NSWorkspaceDidWakeNotification` and `NSWorkspaceWillSleepNotification` (NSWorkspace.h,
//! posted to the workspace's own center), `NSSystemClockDidChangeNotification` (NSDate.h),
//! `NSSystemTimeZoneDidChangeNotification` (NSTimeZone.h) and
//! `NSProcessInfoPowerStateDidChangeNotification` (NSProcessInfo.h, macOS 12 and later).
//! `com.apple.screenIsLocked` and `com.apple.screenIsUnlocked` are distributed notifications
//! that Apple has posted for many years but never documented.
//!
//! The battery-under-20% half of R-33a needs IOKit's power sources and is not here yet; Low
//! Power Mode is.

use std::ptr::NonNull;
use std::time::Duration;

use block2::RcBlock;
use hyperme_core::schedule::Zone;
use hyperme_core::scheduler::{Clock, Presence, PresenceEvent};
use objc2::rc::Retained;
use objc2::runtime::{NSObjectProtocol, ProtocolObject};
use objc2_app_kit::{
    NSWorkspace, NSWorkspaceDidWakeNotification, NSWorkspaceWillSleepNotification,
};
use objc2_foundation::{
    NSDate, NSDistributedNotificationCenter, NSNotification, NSNotificationCenter,
    NSNotificationName, NSOperationQueue, NSProcessInfo,
    NSProcessInfoPowerStateDidChangeNotification, NSString, NSSystemClockDidChangeNotification,
    NSSystemTimeZoneDidChangeNotification, NSTimeZone,
};
use time::{UtcDateTime, UtcOffset};
use tokio::sync::mpsc;

#[allow(unsafe_code)]
mod ffi {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        /// Seconds since the last event of a type, for a source state. Reads a counter; no
        /// permission needed.
        pub safe fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
    }
}

/// `kCGEventSourceStateCombinedSessionState`.
const COMBINED_SESSION_STATE: i32 = 0;
/// `kCGAnyInputEventType`.
const ANY_INPUT_EVENT: u32 = u32::MAX;

/// Seconds since the user last touched the keyboard, mouse or trackpad. Cheap: read only when
/// a job asks, so it costs nothing at idle.
pub fn seconds_since_input() -> u64 {
    let seconds =
        ffi::CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION_STATE, ANY_INPUT_EVENT);
    if seconds.is_finite() && seconds > 0.0 {
        seconds as u64
    } else {
        0
    }
}

/// Whether Low Power Mode is on (R-33a).
pub fn low_power_mode() -> bool {
    NSProcessInfo::processInfo().isLowPowerModeEnabled()
}

/// The scheduler's presence events from macOS.
#[derive(Debug)]
pub struct MacPresence {
    events: mpsc::UnboundedReceiver<PresenceEvent>,
}

/// The notification observers. They deliver on the main queue, so the app's main run loop must
/// be running; keep this on the main thread for as long as events are wanted. Dropping it
/// removes the observers.
pub struct Observers {
    tokens: Vec<(Retained<NSNotificationCenter>, Token)>,
}

/// What `addObserverForName:...` returns, needed to remove the observer.
type Token = Retained<ProtocolObject<dyn NSObjectProtocol>>;

impl std::fmt::Debug for Observers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Observers")
            .field("count", &self.tokens.len())
            .finish()
    }
}

impl Drop for Observers {
    fn drop(&mut self) {
        for (center, token) in &self.tokens {
            // SAFETY: `token` is the object `addObserverForName:...` returned for this center.
            #[allow(unsafe_code)]
            unsafe {
                center.removeObserver(token.as_ref());
            }
        }
    }
}

impl MacPresence {
    /// Starts observing. The first event is always `Launched` (R-15).
    pub fn start() -> (MacPresence, Observers) {
        let (tx, events) = mpsc::unbounded_channel();
        let _ = tx.send(PresenceEvent::Launched);
        let mut observers = Observers { tokens: Vec::new() };

        let workspace = NSWorkspace::sharedWorkspace().notificationCenter();
        let local = NSNotificationCenter::defaultCenter();
        let distributed: Retained<NSNotificationCenter> =
            Retained::into_super(NSDistributedNotificationCenter::defaultCenter());
        let locked = NSString::from_str("com.apple.screenIsLocked");
        let unlocked = NSString::from_str("com.apple.screenIsUnlocked");

        // SAFETY: these are Foundation and AppKit's own constant strings.
        #[allow(unsafe_code)]
        let (wake, sleep, clock, zone, power) = unsafe {
            (
                NSWorkspaceDidWakeNotification,
                NSWorkspaceWillSleepNotification,
                NSSystemClockDidChangeNotification,
                NSSystemTimeZoneDidChangeNotification,
                NSProcessInfoPowerStateDidChangeNotification,
            )
        };

        let fixed = |event: PresenceEvent| move || event;
        observe(
            &mut observers,
            &workspace,
            wake,
            &tx,
            fixed(PresenceEvent::Woke),
        );
        observe(
            &mut observers,
            &workspace,
            sleep,
            &tx,
            fixed(PresenceEvent::Asleep),
        );
        observe(
            &mut observers,
            &distributed,
            &locked,
            &tx,
            fixed(PresenceEvent::Locked),
        );
        observe(
            &mut observers,
            &distributed,
            &unlocked,
            &tx,
            fixed(PresenceEvent::Unlocked),
        );
        observe(
            &mut observers,
            &local,
            clock,
            &tx,
            fixed(PresenceEvent::ClockChanged),
        );
        observe(&mut observers, &local, zone, &tx, || {
            // The cached system zone is stale until reset (NSTimeZone.h).
            NSTimeZone::resetSystemTimeZone();
            PresenceEvent::ZoneChanged
        });
        observe(&mut observers, &local, power, &tx, || {
            PresenceEvent::PowerChanged {
                low: low_power_mode(),
            }
        });
        (MacPresence { events }, observers)
    }

    /// The next event if one is waiting, without blocking.
    pub fn try_event(&mut self) -> Option<PresenceEvent> {
        self.events.try_recv().ok()
    }
}

fn observe(
    observers: &mut Observers,
    center: &Retained<NSNotificationCenter>,
    name: &NSNotificationName,
    tx: &mpsc::UnboundedSender<PresenceEvent>,
    event: impl Fn() -> PresenceEvent + 'static,
) {
    let tx = tx.clone();
    let block = RcBlock::new(move |_note: NonNull<NSNotification>| {
        // The scheduler may have stopped; then the event has nowhere to go.
        let _ = tx.send(event());
    });
    let queue = NSOperationQueue::mainQueue();
    // SAFETY: no object filter; the main queue runs blocks on the main thread; the block only
    // sends on a channel, which is thread-safe.
    #[allow(unsafe_code)]
    let token = unsafe {
        center.addObserverForName_object_queue_usingBlock(Some(name), None, Some(&queue), &block)
    };
    observers.tokens.push((center.clone(), token));
}

impl Presence for MacPresence {
    async fn next_event(&mut self) -> Option<PresenceEvent> {
        self.events.recv().await
    }

    fn seconds_since_input(&self) -> u64 {
        seconds_since_input()
    }
}

/// Posts the clock-change and zone-change notifications to this process's own default center,
/// for the probe's self-test. Nothing outside the process sees them.
pub fn post_local_test_notifications() {
    let center = NSNotificationCenter::defaultCenter();
    // SAFETY: Foundation's own constant strings.
    #[allow(unsafe_code)]
    let names = unsafe {
        [
            NSSystemClockDidChangeNotification,
            NSSystemTimeZoneDidChangeNotification,
        ]
    };
    for name in names {
        // SAFETY: no object and no user info.
        #[allow(unsafe_code)]
        unsafe {
            center.postNotificationName_object(name, None);
        }
    }
}

/// Runs the current thread's run loop for `duration`, so notifications on the main queue are
/// delivered. For the probe and tests; the app's main thread already runs one.
pub fn run_loop_for(duration: Duration) {
    // SAFETY: `kCFRunLoopDefaultMode` is a constant CFString exported by CoreFoundation.
    #[allow(unsafe_code)]
    let mode = unsafe { core_foundation::runloop::kCFRunLoopDefaultMode };
    core_foundation::runloop::CFRunLoop::run_in_mode(mode, duration, false);
}

/// Wall time, the Mac's current zone and a Tokio timer. After a sleep the timer is late, which
/// the scheduler fixes by recomputing on `Woke` (design.md, "Why recompute on wake").
#[derive(Debug, Clone, Copy, Default)]
pub struct MacClock;

impl Zone for MacClock {
    fn offset_at(&self, at: UtcDateTime) -> UtcOffset {
        let date = NSDate::dateWithTimeIntervalSince1970(at.unix_timestamp() as f64);
        let seconds = NSTimeZone::systemTimeZone().secondsFromGMTForDate(&date);
        i32::try_from(seconds)
            .ok()
            .and_then(|s| UtcOffset::from_whole_seconds(s).ok())
            .unwrap_or(UtcOffset::UTC)
    }
}

impl Clock for MacClock {
    fn now(&self) -> UtcDateTime {
        UtcDateTime::now()
    }

    async fn sleep_until(&self, at: UtcDateTime) {
        let wait = at - self.now();
        if wait.is_positive() {
            tokio::time::sleep(wait.unsigned_abs()).await;
        }
    }
}
