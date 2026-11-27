//! Spec 182, AC-11 (R-19): what the trace layer adds to key release to text ready.
//!
//! Ignored by default because it measures, on the real clock, rather than checks behaviour. Run
//! it in release mode on the machine whose numbers go to `docs/results/`:
//!
//!   cargo test --release -p hyperme-core --test trace_overhead -- --ignored --nocapture
//!
//! The providers are fakes that answer at once, so the time from key release to the pasted
//! text is the engine's own work, and the difference between the two runs is the trace layer,
//! its timing sink and a trace collector (the app's trace store hands each trace to a writer
//! thread the same way). Runs alternate in blocks so that drift hits both alike.

mod common;

use std::time::{Duration, Instant};

use common::*;
use hyperme_core::dictation::{Event, Settings, Status};

const BLOCKS: usize = 20;
const PER_BLOCK: usize = 100;

fn fast_settings() -> Settings {
    let mut s = settings();
    s.min_hold = Duration::ZERO;
    s.double_tap = Duration::ZERO;
    s.sounds = false;
    s
}

fn engine(traced: bool) -> Harness {
    let log = Log::new();
    let speech = FakeSpeech::new(
        &log,
        "please send the quarterly numbers to the whole team today",
    );
    let cleanup = FakeCleanup::new(
        &log,
        "Please send the quarterly numbers to the whole team today.",
    );
    if traced {
        Harness::start(&log, speech, cleanup, fast_settings())
    } else {
        Harness::start_untraced(&log, speech, cleanup, fast_settings())
    }
}

/// One dictation: microseconds from key release to the text pasted.
async fn dictation(h: &mut Harness) -> u64 {
    h.send(Event::KeyDown).await;
    h.frames(0, 3).await;
    tokio::time::sleep(Duration::from_millis(2)).await;
    let released = Instant::now();
    h.send(Event::KeyUp).await;
    h.until(&Status::Done {
        raw: false,
        via: None,
    })
    .await;
    let took = released.elapsed().as_micros() as u64;
    h.until(&Status::Idle).await;
    took
}

fn percentile(sorted: &[u64], p: f64) -> u64 {
    let rank = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "a measurement: run with --release -- --ignored --nocapture"]
async fn ac11_trace_overhead_on_release_to_text() {
    let mut on = engine(true);
    let mut off = engine(false);
    // Warm both up so first-use costs (allocation, lazy statics) are not counted. The traced
    // engine goes first: tracing caches each callsite's interest when it is first hit, and a
    // first hit under the untraced engine left the traced one recording nothing.
    for _ in 0..20 {
        dictation(&mut on).await;
        dictation(&mut off).await;
    }
    let (mut without, mut with) = (Vec::new(), Vec::new());
    for _ in 0..BLOCKS {
        for _ in 0..PER_BLOCK {
            without.push(dictation(&mut off).await);
        }
        for _ in 0..PER_BLOCK {
            with.push(dictation(&mut on).await);
        }
    }
    without.sort_unstable();
    with.sort_unstable();
    let row = |name: &str, v: &[u64]| {
        println!(
            "| {name} | {} | {} | {} | {} |",
            v.len(),
            percentile(v, 50.0),
            percentile(v, 90.0),
            percentile(v, 99.0)
        );
    };
    println!("| Engine | Dictations | p50 µs | p90 µs | p99 µs |");
    println!("| --- | --- | --- | --- | --- |");
    row("without traces", &without);
    row("with traces", &with);
    let p90 = |v: &[u64]| percentile(v, 90.0) as i64;
    println!("p90 difference: {} µs", p90(&with) - p90(&without));
    let traces = on.traces.all();
    let pasted = traces
        .iter()
        .filter(|t| t.root().name("outcome") == Some("pasted"))
        .count();
    assert!(
        pasted >= BLOCKS * PER_BLOCK,
        "{pasted} of {} traces pasted",
        traces.len()
    );
    // R-19: within 1 ms at the 90th percentile.
    assert!(p90(&with) - p90(&without) <= 1_000);
}
