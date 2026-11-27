//! Spec 200 (Diagnostics), slice 1: the numbers Diagnostics shows, computed from plain trace
//! rows. No store and no clock: the tests pass the rows and "now".

use hyperme_core::diagnostics::{Diagnostics, MIN_VALUES, StageTime, TraceRow, Window, compute};

const HOUR_MS: i64 = 60 * 60 * 1000;
const DAY_MS: i64 = 24 * HOUR_MS;
const NOW: i64 = 100 * DAY_MS;

/// A pasted dictation `ago_ms` before now. Key release to text is speech + 1 ms of dictionary +
/// cleanup + insert, all in milliseconds.
fn pasted(ago_ms: i64, speech: u64, cleanup: u64, insert: u64) -> TraceRow {
    let stages = vec![
        StageTime::new("mic_start", 30_000),
        StageTime::new("speech", speech * 1000),
        StageTime::new("dictionary", 1_000),
        StageTime::new("cleanup", cleanup * 1000),
        StageTime::new("insert", insert * 1000),
        StageTime::new("history_write", 2_000),
    ];
    TraceRow {
        started_ms: NOW - ago_ms,
        outcome: "pasted".into(),
        error_kind: None,
        speech: Some(("speechmatics".into(), "enhanced".into())),
        cleanup: Some(("groq".into(), "fast-model".into())),
        release_to_text_us: Some((speech + 1 + cleanup + insert) * 1000),
        stages,
    }
}

fn ended(ago_ms: i64, outcome: &str, error_kind: Option<&str>) -> TraceRow {
    TraceRow {
        started_ms: NOW - ago_ms,
        outcome: outcome.into(),
        error_kind: error_kind.map(str::to_owned),
        speech: Some(("speechmatics".into(), "enhanced".into())),
        cleanup: None,
        release_to_text_us: None,
        stages: vec![StageTime::new("mic_start", 30_000)],
    }
}

fn ready(window: &Window) -> (usize, u64, u64) {
    match window {
        Window::Ready {
            count,
            p50_us,
            p90_us,
            ..
        } => (*count, *p50_us, *p90_us),
        other => panic!("expected percentiles, got {other:?}"),
    }
}

/// AC-1 (R-2, R-3, R-4): with known durations, key release to text, the stages per provider and
/// model, and the outcome counts match a hand computation, nearest rank as the store's
/// `stage_percentiles` does.
#[test]
fn the_numbers_match_a_fixture_of_known_durations() {
    let mut rows = Vec::new();
    // Ten in the last day: speech 100..=1000 ms in steps of 100, cleanup 50, insert 9.
    for i in 1..=10u64 {
        rows.push(pasted(i as i64 * HOUR_MS / 2, i * 100, 50, 9));
    }
    // Five more earlier in the week, on another cleanup model.
    for i in 1..=5u64 {
        let mut row = pasted(3 * DAY_MS + i as i64, 200, 300, 9);
        row.cleanup = Some(("groq".into(), "big-model".into()));
        rows.push(row);
    }
    // Outside the week: ignored.
    rows.push(pasted(8 * DAY_MS, 9000, 9000, 9000));
    rows.push(ended(HOUR_MS, "failed", Some("speech_timeout")));
    rows.push(ended(2 * HOUR_MS, "failed", Some("speech_timeout")));
    rows.push(ended(4 * DAY_MS, "failed", Some("cleanup_http")));
    rows.push(ended(HOUR_MS, "cancelled", None));
    rows.push(ended(HOUR_MS, "nothing_heard", None));

    let view: Diagnostics = compute(&rows, NOW);

    // Last day: 160, 260, ..., 1060 ms. p50 = 5th = 560, p90 = 9th = 960.
    assert_eq!(ready(&view.last_day), (10, 560_000, 960_000));
    // Week: those ten plus five of 510 ms. Sorted: 160 260 360 460 510×5 560 660 760 860 960
    // 1060; p50 = rank 8 = 510, p90 = rank 14 = 960.
    assert_eq!(ready(&view.last_week), (15, 510_000, 960_000));

    let stage = |name: &str, model: Option<&str>| {
        view.stages
            .iter()
            .find(|s| s.stage == name && s.model.as_deref() == model)
            .unwrap_or_else(|| panic!("no row for {name} {model:?}"))
    };
    let speech = stage("speech", Some("enhanced"));
    assert_eq!(speech.provider.as_deref(), Some("speechmatics"));
    // Speech over the week: 100..=1000 and 200×5, 15 values; the rows that ended before speech
    // have no speech stage. Sorted: 100 200×6 300 ... 1000; p50 = rank 8 = 300, p90 = rank 14.
    assert_eq!(ready(&speech.window), (15, 300_000, 900_000));
    assert_eq!(
        ready(&stage("cleanup", Some("fast-model")).window),
        (10, 50_000, 50_000)
    );
    let big = stage("cleanup", Some("big-model"));
    assert_eq!(ready(&big.window), (5, 300_000, 300_000));
    let insert = stage("insert", None);
    assert_eq!(insert.provider, None);
    assert_eq!(ready(&insert.window).0, 15);
    // Mic start counts every dictation in the week that opened the microphone: 15 + 5.
    assert_eq!(ready(&stage("mic_start", None).window).0, 20);
    assert!(view.stages.iter().any(|s| s.stage == "history_write"));

    assert_eq!(
        view.outcomes,
        vec![
            ("pasted".to_string(), 15),
            ("copied".to_string(), 0),
            ("failed".to_string(), 3),
            ("cancelled".to_string(), 1),
            ("nothing_heard".to_string(), 1),
        ]
    );
    assert_eq!(
        view.failures,
        vec![
            ("speech_timeout".to_string(), 2),
            ("cleanup_http".to_string(), 1)
        ]
    );
    assert_eq!(view.slowest_stage, Some("speech".to_string()));
    assert!(!view.empty);
}

/// AC-2 (R-6, R-7): with 4 dictations in a window, the window says there are too few in place of
/// percentiles; with no traces at all, the view says it is empty.
#[test]
fn a_window_with_too_few_dictations_says_so() {
    assert_eq!(MIN_VALUES, 5);
    let rows: Vec<TraceRow> = (1..=4).map(|i| pasted(i * HOUR_MS, 300, 100, 9)).collect();
    let view = compute(&rows, NOW);
    assert_eq!(view.last_day, Window::TooFew { count: 4 });
    assert_eq!(view.last_week, Window::TooFew { count: 4 });
    assert!(!view.empty);

    let empty = compute(&[], NOW);
    assert!(empty.empty);
    assert_eq!(empty.last_week, Window::TooFew { count: 0 });
    assert!(empty.stages.is_empty());
    assert!(empty.failures.is_empty());
    assert_eq!(empty.slowest_stage, None);
}

/// AC-3 (R-8): a p90 of key release to text over 800 ms marks the window and names the slowest
/// stage after key release; under the budget, nothing is marked.
#[test]
fn a_p90_over_budget_is_marked_and_names_the_slowest_stage() {
    // Ten dictations; the slowest two have a 790 ms cleanup, so p90 = 950 ms.
    let mut rows: Vec<TraceRow> = (1..=8).map(|i| pasted(i * HOUR_MS, 200, 100, 9)).collect();
    rows.push(pasted(9 * HOUR_MS, 150, 790, 9));
    rows.push(pasted(10 * HOUR_MS, 150, 790, 9));
    let view = compute(&rows, NOW);
    let Window::Ready {
        p90_us,
        over_budget,
        ..
    } = view.last_day
    else {
        panic!("expected percentiles");
    };
    assert_eq!(p90_us, 950_000);
    assert!(over_budget);
    assert_eq!(view.slowest_stage, Some("cleanup".to_string()));

    let fast: Vec<TraceRow> = (1..=10).map(|i| pasted(i * HOUR_MS, 200, 100, 9)).collect();
    let view = compute(&fast, NOW);
    assert!(matches!(
        view.last_day,
        Window::Ready {
            over_budget: false,
            ..
        }
    ));
    assert_eq!(view.slowest_stage, None);
}
