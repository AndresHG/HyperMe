//! AC-2 live check: key-down to the first captured audio sample, over 50 starts of one built
//! stream, 150 ms apart. Timing starts at the device lookup, as `CpalMicrophone::start` does.
//! The first sample was captured when the first callback arrived minus that callback's length
//! (frames / device rate). Prints the median, p90 and max of `play()` returning, the first
//! callback and the first captured sample. Numbers only; no audio is kept.
//! `cargo run --release -p hyperme-platform --example mic_first_sample`

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// The first callback after a start: when it arrived and how many frames it held.
type First = Arc<Mutex<Option<(Instant, usize)>>>;

fn main() {
    let device = cpal::default_host()
        .default_input_device()
        .expect("no microphone found");
    let supported = device.default_input_config().expect("no input format");
    let rate = f64::from(supported.sample_rate());
    let channels = usize::from(supported.channels());
    let first: First = Arc::default();
    let seen = Arc::clone(&first);
    let stream = device
        .build_input_stream(
            supported.config(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let now = Instant::now();
                let mut seen = seen.lock().unwrap();
                seen.get_or_insert((now, data.len() / channels));
            },
            |e| eprintln!("stream error: {e}"),
            None,
        )
        .expect("could not build the input stream");

    let (mut play, mut callback, mut sample) = (vec![], vec![], vec![]);
    for _ in 0..50 {
        *first.lock().unwrap() = None;
        let start = Instant::now();
        let _ = cpal::default_host().default_input_device().map(|d| d.id());
        stream.play().expect("could not start the microphone");
        let played = Instant::now();
        let (arrived, frames) = loop {
            std::thread::sleep(Duration::from_millis(1));
            if let Some(got) = *first.lock().unwrap() {
                break got;
            }
        };
        stream.pause().expect("could not pause the microphone");
        let ms = |at: Instant| (at - start).as_secs_f64() * 1000.0;
        play.push(ms(played));
        callback.push(ms(arrived));
        sample.push(ms(arrived) - frames as f64 / rate * 1000.0);
        std::thread::sleep(Duration::from_millis(150));
    }
    for (what, mut v) in [
        ("play() returns", play),
        ("first callback", callback),
        ("first captured sample", sample),
    ] {
        v.sort_by(f64::total_cmp);
        println!(
            "{what:22} median {:5.1} ms, p90 {:5.1} ms, max {:5.1} ms",
            v[v.len() / 2],
            v[v.len() * 9 / 10],
            v[v.len() - 1]
        );
    }
}
