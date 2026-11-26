//! Prints the presence events macOS sends, to check the notification names on a real Mac
//! (specs/92-daily-tasks, slice 5). Run it, then lock and unlock the screen, sleep and wake the
//! Mac, change the time zone in System Settings, or turn Low Power Mode on and off:
//!
//!     cargo run -p hyperme-platform --example presence_probe -- 60
//!
//! The argument is how many seconds to listen (default 30). With `--self-test` it also posts
//! the clock-change and zone-change notifications inside this process only (the default
//! center is per process, so nothing else on the Mac sees them), to check the observers and
//! the main-queue delivery.

#[cfg(target_os = "macos")]
fn main() {
    use std::time::{Duration, Instant};

    use hyperme_core::schedule::Zone;
    use hyperme_core::scheduler::Clock;
    use hyperme_platform::presence::{
        MacClock, MacPresence, low_power_mode, run_loop_for, seconds_since_input,
    };

    let args: Vec<String> = std::env::args().skip(1).collect();
    let self_test = args.iter().any(|a| a == "--self-test");
    let seconds: u64 = args.iter().find_map(|a| a.parse().ok()).unwrap_or(30);
    let clock = MacClock;
    let now = clock.now();
    println!(
        "now {now} UTC, offset {}, idle {} s, low power {}",
        clock.offset_at(now),
        seconds_since_input(),
        low_power_mode()
    );
    let (mut presence, _observers) = MacPresence::start();
    let start = Instant::now();
    println!("listening for {seconds} s");
    if self_test {
        hyperme_platform::presence::post_local_test_notifications();
    }
    while start.elapsed() < Duration::from_secs(seconds) {
        run_loop_for(Duration::from_millis(250));
        while let Some(event) = presence.try_event() {
            println!("{:>6.1} s  {event:?}", start.elapsed().as_secs_f64());
        }
    }
    println!("done, idle {} s", seconds_since_input());
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("presence_probe runs on macOS only");
}
