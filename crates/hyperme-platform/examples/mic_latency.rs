//! AC-2 preparation: time from `Microphone::start` to the first 20 ms frame, over 50 starts.
//! Run on a Mac with a microphone: `cargo run --release -p hyperme-platform --example mic_latency`.
//! Prints numbers only; no audio is kept.

use std::time::{Duration, Instant};

use hyperme_core::dictation::Event;
use hyperme_core::traits::Microphone;
use hyperme_platform::microphone::CpalMicrophone;
use tokio::sync::mpsc;

fn main() {
    let (tx, mut rx) = mpsc::channel(1024);
    let mut mic = CpalMicrophone::new(tx, 24_000);
    let mut times = Vec::new();
    for _ in 0..50 {
        let start = Instant::now();
        if let Err(error) = mic.start() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        match rx.blocking_recv() {
            Some(Event::Audio(_)) => times.push(start.elapsed()),
            _ => unreachable!("only audio frames are sent"),
        }
        mic.stop();
        while rx.try_recv().is_ok() {}
        std::thread::sleep(Duration::from_millis(100));
    }
    times.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    println!(
        "start to first frame over {} starts: median {:.1} ms, p90 {:.1} ms, max {:.1} ms",
        times.len(),
        ms(times[times.len() / 2]),
        ms(times[times.len() * 9 / 10]),
        ms(times[times.len() - 1])
    );
}
