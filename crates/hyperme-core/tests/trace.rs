//! Spec 182 (trace store), slice 1: the spans each dictation records, and the no-text rule.

mod common;

use std::sync::Arc;

use common::*;
use hyperme_core::dictation::{Event, Status};
use hyperme_core::trace::{FinishedTrace, TraceLayer, TraceSink};
use hyperme_core::traits::ErrorKind;
use tracing_subscriber::layer::SubscriberExt;

fn names(trace: &FinishedTrace) -> Vec<&'static str> {
    trace.spans.iter().map(|s| s.name).collect()
}

/// AC-1 (R-1 to R-4): a pasted dictation is one trace, the root first and every stage nested
/// under it in order, with its fields.
#[tokio::test(start_paused = true)]
async fn a_pasted_dictation_is_one_trace_with_its_stages() {
    let log = Log::new();
    let mut speech = FakeSpeech::new(&log, "please send the quarterly numbers to the team");
    speech.finish_delay = ms(300);
    let mut cleanup = FakeCleanup::new(&log, "Please send the quarterly numbers to the team.");
    cleanup.delay = ms(120);
    let mut h = Harness::start(&log, speech, cleanup, settings());

    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(40)).await;
    h.frames(0, 5).await;
    tokio::time::sleep(ms(960)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: false }).await;
    h.until(&Status::Idle).await;

    let traces = h.traces.all();
    assert_eq!(traces.len(), 1);
    let trace = &traces[0];
    assert_eq!(
        names(trace),
        [
            "dictation",
            "mic_start",
            "capture",
            "speech",
            "dictionary",
            "cleanup",
            "insert",
            "history_write"
        ]
    );
    assert!(trace.spans[1..].iter().all(|s| s.parent == Some(0)));
    let root = trace.root();
    assert_eq!(root.name("outcome"), Some("pasted"));
    assert_eq!(root.name("language"), Some("en"));
    assert_eq!(root.u64("words"), Some(8));
    assert_eq!(root.bool("fallback"), Some(false));
    assert!(root.u64("history_id").is_some());
    assert_eq!(trace.span("mic_start").unwrap().end_us(), 40_000);
    assert_eq!(trace.span("capture").unwrap().end_us(), 1_000_000);
    assert_eq!(trace.span("speech").unwrap().duration_us, 300_000);
    // #195 (R-2): the history write comes after the paste.
    let insert = trace.span("insert").unwrap();
    assert!(trace.span("history_write").unwrap().start_us >= insert.end_us());
    let cleanup = trace.span("cleanup").unwrap();
    assert_eq!(cleanup.event_us("first_token"), Some(1_420_000));
    assert_eq!(
        trace.span("dictionary").unwrap().u64("entries_applied"),
        Some(0)
    );
}

/// AC-2 (R-2, R-3, R-8): a failed and a cancelled dictation each leave a trace with their
/// outcome; a retry is its own trace, naming the dictation it retries.
#[tokio::test(start_paused = true)]
async fn failed_cancelled_and_retried_dictations_leave_traces() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "still failing");
    *speech.finish_error.lock().unwrap() = Some(err(ErrorKind::Network, "connection reset"));
    let mut h = Harness::start(&log, speech, FakeCleanup::new(&log, "x"), settings());
    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(600)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Idle).await;
    h.send(Event::Retry(None)).await;
    h.until(&Status::Idle).await;
    h.send(Event::KeyDown).await;
    h.send(Event::Esc).await;
    h.until(&Status::Idle).await;

    let traces = h.traces.all();
    let outcomes: Vec<_> = traces.iter().map(|t| t.root().name("outcome")).collect();
    assert_eq!(
        outcomes,
        [Some("failed"), Some("failed"), Some("cancelled")]
    );
    assert_eq!(traces[0].root().name("error_kind"), Some("speech"));
    let first = traces[0].root().u64("history_id");
    assert!(first.is_some());
    assert_eq!(traces[1].root().u64("retry_of"), first);
    assert_eq!(names(&traces[1]), ["dictation", "speech"]);
    // #195 (R-2): nothing was saved as done, so no history write.
    assert!(traces.iter().all(|t| t.span("history_write").is_none()));
}

/// AC-3, schema half (R-5): a span field that is not declared, or text in a debug value, is
/// never kept; the transcript never reaches a trace.
#[tokio::test(start_paused = true)]
async fn undeclared_fields_and_text_never_reach_a_trace() {
    let traces = FakeTraces::default();
    let sinks: Vec<Arc<dyn TraceSink>> = vec![Arc::new(traces.clone())];
    let subscriber = tracing_subscriber::registry().with(TraceLayer::new(sinks));
    tracing::subscriber::with_default(subscriber, || {
        let root = tracing::info_span!(
            "dictation",
            words = 3u64,
            transcript = "secret words here",
            outcome = tracing::field::Empty,
            language = tracing::field::Empty,
        );
        root.record("outcome", "pasted");
        // A Name field holding a line break is text, not a name.
        root.record("language", "en\nsecret");
        let child = tracing::info_span!(parent: &root, "speech", note = ?"secret");
        drop(child);
        let ignored = tracing::info_span!(parent: &root, "not_in_schema", value = 1u64);
        drop(ignored);
    });
    let all = traces.all();
    assert_eq!(all.len(), 1);
    let shown = format!("{all:?}");
    assert!(!shown.contains("secret"), "{shown}");
    assert_eq!(names(&all[0]), ["dictation", "speech"]);
    assert_eq!(all[0].root().fields.len(), 2, "{shown}");

    // And through the engine: the transcript and cleaned text are nowhere in the trace.
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "the quarterly numbers are confidential");
    let cleanup = FakeCleanup::new(&log, "The quarterly numbers are confidential.");
    let mut h = Harness::start(&log, speech, cleanup, settings());
    h.send(Event::KeyDown).await;
    tokio::time::sleep(ms(600)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Idle).await;
    let shown = format!("{:?}", h.traces.all());
    assert!(!shown.contains("quarterly"), "{shown}");
    assert!(!shown.contains("TextEdit"), "{shown}");
}
