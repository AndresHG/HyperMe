//! Tasks slice 5: presence and the clock on macOS. Tests cite `specs/92-daily-tasks/spec.md`
//! (R-15, R-32, R-34). Wake, lock and zone events need a person or admin rights to trigger, so
//! they are checked by hand with `cargo run --example presence_probe`.

#![cfg(target_os = "macos")]

use std::time::{SystemTime, UNIX_EPOCH};

use hyperme_core::schedule::Zone;
use hyperme_core::scheduler::{Clock, Presence, PresenceEvent};
use hyperme_platform::presence::{MacClock, MacPresence, seconds_since_input};

/// R-15: "HyperMe starts" counts as presence, so the first event is always `Launched`.
#[test]
fn the_first_event_is_launched() {
    let (mut presence, _observers) = MacPresence::start();
    assert_eq!(presence.try_event(), Some(PresenceEvent::Launched));
    assert_eq!(
        presence.try_event(),
        None,
        "nothing else without the run loop"
    );
}

/// design.md, "Presence on macOS": seconds since the last input, read on demand. On a Mac in
/// use it is a plausible number; under CI with no session it may be large but never negative.
#[test]
fn seconds_since_input_is_read_on_demand() {
    let (presence, _observers) = MacPresence::start();
    let idle = presence.seconds_since_input();
    assert!(idle < 10 * 365 * 24 * 3600, "{idle}");
    assert_eq!(idle / 60, seconds_since_input() / 60);
}

/// R-32, T-9: the clock's zone is the Mac's current one. Its offset now matches what `date`
/// says, and its time matches the system clock.
#[test]
fn the_clock_uses_the_macs_zone_and_time() {
    let clock = MacClock;
    let now = clock.now();
    let system = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    assert!((now.unix_timestamp() - system).abs() <= 2);

    let out = std::process::Command::new("date")
        .arg("+%z")
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    let text = text.trim();
    let sign = if text.starts_with('-') { -1 } else { 1 };
    let hours: i32 = text[1..3].parse().unwrap();
    let minutes: i32 = text[3..5].parse().unwrap();
    let expected = sign * (hours * 3600 + minutes * 60);
    assert_eq!(
        clock.offset_at(now).whole_seconds(),
        expected,
        "date says {text}"
    );
}

/// R-34: the timer sleeps until a wall-clock time.
#[tokio::test]
async fn the_clock_sleeps_until_a_time() {
    let clock = MacClock;
    let start = clock.now();
    clock
        .sleep_until(start + time::SignedDuration::milliseconds(50))
        .await;
    assert!(clock.now() - start >= time::SignedDuration::milliseconds(45));
}
