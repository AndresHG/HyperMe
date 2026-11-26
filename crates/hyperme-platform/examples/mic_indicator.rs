//! Option B by eye: records 2 s, then pauses 8 s, four times, so the macOS microphone
//! indicator (which lags by a second or two) has time to go off in each pause.
//! `cargo run --release -p hyperme-platform --example mic_indicator` on a Mac.
//! Prints what to look at; no audio is kept.

use std::time::Duration;

use hyperme_core::traits::Microphone;
use hyperme_platform::microphone::CpalMicrophone;

fn main() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(4096);
    let mut mic = CpalMicrophone::new(tx, 24_000);
    for round in 1..=4 {
        if let Err(error) = mic.start() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        println!("{round}/4 RECORDING (2 s): the orange dot should be on");
        std::thread::sleep(Duration::from_secs(2));
        mic.stop();
        while rx.try_recv().is_ok() {}
        println!("{round}/4 PAUSED (8 s): check the dot now, it should go off");
        std::thread::sleep(Duration::from_secs(8));
    }
    drop(mic);
    println!("Done: the stream is released.");
}
