//! Spec 182 (trace store), slice 2: traces kept in the history's SQLite file. Traces are made
//! with the core's own spans; every store lives in a fresh folder under the target directory.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use hyperme_core::config::Retention;
use hyperme_core::trace::{DictationTrace, Outcome, TraceSink, subscriber};
use hyperme_core::traits::History;
use hyperme_platform::history::{HistorySettings, SqliteHistory};
use hyperme_platform::trace_store::{Labels, TraceSettings, TraceStore};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

fn temp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("traces")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn settings(history: bool) -> TraceSettings {
    TraceSettings {
        keep: true,
        history,
        retention: Retention::Days(30),
    }
}

/// A store on a manual clock starting at day 100, in the history's file.
fn open(dir: &Path, settings: TraceSettings) -> (Arc<TraceStore>, Arc<AtomicI64>) {
    let clock = Arc::new(AtomicI64::new(100 * DAY_MS));
    let now = clock.clone();
    let store = TraceStore::open_with_clock(&dir.join("history.sqlite3"), settings, move || {
        now.load(Ordering::SeqCst)
    })
    .unwrap();
    store.set_labels(Labels {
        speech_provider: "speechmatics".into(),
        speech_model: "enhanced".into(),
        cleanup_provider: "groq".into(),
        cleanup_model: "fast-model".into(),
        cleanup_level: "light".into(),
    });
    (Arc::new(store), clock)
}

fn history(dir: &Path, enabled: bool) -> SqliteHistory {
    SqliteHistory::open_with_clock(
        dir,
        HistorySettings {
            enabled,
            retention: Retention::Days(30),
            size_limit_bytes: 1024 * 1024 * 1024,
        },
        || 100 * DAY_MS,
    )
    .unwrap()
}

/// Runs one dictation's spans through a subscriber that writes to `store`.
fn dictation(store: &Arc<TraceStore>, history_id: Option<i64>, outcome: Outcome) {
    let sink: Arc<dyn TraceSink> = store.clone();
    tracing::dispatcher::with_default(&subscriber(vec![sink]), || {
        let mut trace = DictationTrace::dictation();
        if let Some(id) = history_id {
            trace.history_id(id);
        }
        trace.first_audio();
        trace.released();
        trace.transcript(Some("en"));
        trace.dictionary(1);
        trace.words(6);
        trace.cleanup_started();
        trace.first_token();
        trace.ready(false);
        trace.inserted();
        trace.end(outcome);
    });
    store.flush();
}

/// AC-2 (R-8), AC-5 (R-10): every dictation is kept, pasted or not, with its labels; a
/// history item's trace comes back with its spans in order; stage percentiles group speech and
/// cleanup by provider and model.
#[test]
fn traces_are_kept_and_read_back() {
    let dir = temp_dir("kept");
    let mut h = history(&dir, true);
    let (store, _) = open(&dir, settings(true));
    let id = h.begin(Some("TextEdit")).unwrap();
    h.done(id, "hello there", Some("en")).unwrap();
    dictation(&store, Some(id), Outcome::Pasted);
    dictation(&store, None, Outcome::Cancelled);
    assert_eq!(store.count().unwrap(), 2);

    let trace = store.trace_for(id).unwrap().unwrap();
    assert_eq!(trace.outcome, "pasted");
    assert_eq!(trace.labels.cleanup_model, "fast-model");
    let names: Vec<&str> = trace.spans.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "dictation",
            "mic_start",
            "capture",
            "speech",
            "dictionary",
            "cleanup",
            "insert"
        ]
    );
    assert_eq!(trace.spans[0].fields["words"], 6);
    assert_eq!(trace.spans[4].fields["entries_applied"], 1);
    assert!(trace.spans[5].events.contains_key("first_token"));

    let stats = store.stage_percentiles(0).unwrap();
    let speech = stats.iter().find(|s| s.stage == "speech").unwrap();
    assert_eq!(speech.count, 2);
    assert_eq!(speech.provider.as_deref(), Some("speechmatics"));
    let insert = stats.iter().find(|s| s.stage == "insert").unwrap();
    assert_eq!(insert.provider, None);
    assert!(store.trace_for(9999).unwrap().is_none());
}

/// AC-3 (R-5): the database file holds no text: not the transcript the history keeps in its
/// own table, not the app name; only the trace's numbers and names.
#[test]
fn the_trace_tables_hold_no_text() {
    let dir = temp_dir("no-text");
    let mut h = history(&dir, true);
    let (store, _) = open(&dir, settings(true));
    let id = h.begin(Some("SecretApp")).unwrap();
    h.done(id, "confidential quarterly numbers", Some("en"))
        .unwrap();
    dictation(&store, Some(id), Outcome::Pasted);
    let trace = store.trace_for(id).unwrap().unwrap();
    let shown = format!("{trace:?}");
    for text in ["confidential", "quarterly", "SecretApp"] {
        assert!(!shown.contains(text), "{shown}");
    }
}

/// AC-4 (R-9): deleting a history item deletes its trace; retention drops old traces; a
/// cancelled dictation, whose item is discarded at once, keeps its trace until it is old.
#[test]
fn traces_follow_their_history_items_and_retention() {
    let dir = temp_dir("retention");
    let mut h = history(&dir, true);
    let (store, clock) = open(&dir, settings(true));
    let id = h.begin(Some("TextEdit")).unwrap();
    h.done(id, "keep me", Some("en")).unwrap();
    dictation(&store, Some(id), Outcome::Pasted);
    let gone = h.begin(Some("TextEdit")).unwrap();
    h.discard(gone).unwrap();
    dictation(&store, Some(gone), Outcome::Cancelled);
    assert_eq!(store.count().unwrap(), 2);

    h.delete_item(id).unwrap();
    store.cleanup();
    store.flush();
    assert!(store.trace_for(id).unwrap().is_none());
    assert_eq!(store.count().unwrap(), 1, "the cancelled trace stays");

    clock.store(131 * DAY_MS, Ordering::SeqCst);
    store.cleanup();
    store.flush();
    assert_eq!(store.count().unwrap(), 0);
}

/// AC-4 (R-9a, TS-1): with history off, traces are kept 30 days though their items are gone;
/// with Keep timing traces off, nothing is written.
#[test]
fn history_off_keeps_traces_thirty_days_and_the_switch_stops_them() {
    let dir = temp_dir("history-off");
    let mut h = history(&dir, false);
    let (store, clock) = open(&dir, settings(false));
    let id = h.begin(Some("TextEdit")).unwrap();
    h.done(id, "gone at once", Some("en")).unwrap();
    h.cleanup().unwrap();
    dictation(&store, Some(id), Outcome::Pasted);
    store.cleanup();
    store.flush();
    assert_eq!(store.count().unwrap(), 1);
    clock.store(129 * DAY_MS, Ordering::SeqCst);
    store.cleanup();
    store.flush();
    assert_eq!(store.count().unwrap(), 1);
    clock.store(131 * DAY_MS, Ordering::SeqCst);
    store.cleanup();
    store.flush();
    assert_eq!(store.count().unwrap(), 0);

    store.set_settings(TraceSettings {
        keep: false,
        ..settings(false)
    });
    dictation(&store, None, Outcome::Pasted);
    assert_eq!(store.count().unwrap(), 0);
}

/// AC-6 (R-11): a store that cannot write drops the trace; the caller is not slowed or failed.
#[test]
fn a_store_that_cannot_write_drops_traces_quietly() {
    let dir = temp_dir("broken");
    let (store, _) = open(&dir, settings(true));
    // Replace the database with a folder: every write fails from now on.
    std::fs::remove_file(dir.join("history.sqlite3")).unwrap();
    let _ = std::fs::remove_file(dir.join("history.sqlite3-wal"));
    let _ = std::fs::remove_file(dir.join("history.sqlite3-shm"));
    std::fs::create_dir(dir.join("history.sqlite3")).unwrap();
    let started = std::time::Instant::now();
    dictation(&store, None, Outcome::Pasted);
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
    assert!(store.count().is_err());
}

/// One failed dictation that ended after speech, its kind recorded on the root.
fn failed(store: &Arc<TraceStore>, kind: &'static str) {
    let sink: Arc<dyn TraceSink> = store.clone();
    tracing::dispatcher::with_default(&subscriber(vec![sink]), || {
        let mut trace = DictationTrace::dictation();
        trace.first_audio();
        trace.released();
        trace.fail(kind);
    });
    store.flush();
}

/// Spec 200 AC-1 (R-2 to R-4): `traces_since` gives the traces of a window newest first, up to
/// a limit, as rows Diagnostics computes from; `outcome_counts` counts every trace of the
/// window, past the limit. A fallback's provider is the one a row names.
#[test]
fn diagnostics_reads_the_traces_of_a_window() {
    let dir = temp_dir("diagnostics");
    let (store, clock) = open(&dir, settings(false));
    dictation(&store, None, Outcome::Pasted);
    clock.store(105 * DAY_MS, Ordering::SeqCst);
    for _ in 0..3 {
        dictation(&store, None, Outcome::Pasted);
    }
    {
        let sink: Arc<dyn TraceSink> = store.clone();
        tracing::dispatcher::with_default(&subscriber(vec![sink]), || {
            let mut trace = DictationTrace::dictation();
            trace.first_audio();
            trace.released();
            trace.transcript(Some("es"));
            trace.speech_via("backup", "backup-model");
            trace.cleanup_started();
            trace.ready(true);
            trace.inserted();
            trace.end(Outcome::Copied);
        });
        store.flush();
    }
    failed(&store, "speech_timeout");

    let since = 101 * DAY_MS;
    let traces = store.traces_since(since, 100).unwrap();
    assert_eq!(
        traces.len(),
        5,
        "the trace of day 100 is outside the window"
    );
    assert_eq!(traces[0].outcome, "failed", "newest first");
    assert_eq!(store.traces_since(since, 2).unwrap().len(), 2);

    let rows: Vec<_> = traces.iter().map(|t| t.row()).collect();
    assert_eq!(rows[0].error_kind.as_deref(), Some("speech_timeout"));
    assert_eq!(rows[0].release_to_text_us, None);
    let copied = &rows[1];
    assert_eq!(copied.outcome, "copied");
    assert_eq!(
        copied.speech,
        Some(("backup".to_string(), "backup-model".to_string()))
    );
    assert_eq!(
        copied.cleanup,
        Some(("groq".to_string(), "fast-model".to_string()))
    );
    assert!(copied.release_to_text_us.is_some());
    let pasted = &rows[2];
    assert_eq!(
        pasted.speech,
        Some(("speechmatics".to_string(), "enhanced".to_string()))
    );
    let names: Vec<&str> = pasted.stages.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "mic_start",
            "capture",
            "speech",
            "dictionary",
            "cleanup",
            "insert"
        ]
    );
    let after_capture: u64 = pasted
        .stages
        .iter()
        .filter(|s| !["mic_start", "capture"].contains(&s.name.as_str()))
        .map(|s| s.duration_us)
        .sum();
    assert!(pasted.release_to_text_us.unwrap() >= after_capture);

    let view = hyperme_core::diagnostics::compute(&rows, 105 * DAY_MS);
    assert_eq!(view.failures, vec![("speech_timeout".to_string(), 1)]);

    let counts = store.outcome_counts(since).unwrap();
    assert_eq!(
        counts,
        vec![
            ("copied".to_string(), 1),
            ("failed".to_string(), 1),
            ("pasted".to_string(), 3)
        ]
    );
    assert_eq!(
        store.outcome_counts(0).unwrap()[2],
        ("pasted".to_string(), 4)
    );
}
