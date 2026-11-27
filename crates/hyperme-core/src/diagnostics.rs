//! The numbers the Diagnostics section shows (spec 200 R-2 to R-8), computed from stored traces.
//!
//! Pure: no SQLite and no clock. The trace store turns its rows into [`TraceRow`]s and the caller
//! passes "now", so the arithmetic is tested with fixtures.

use std::collections::BTreeMap;

use serde::Serialize;

/// Fewer values than this in a window show "too few" in place of percentiles (R-6).
pub const MIN_VALUES: usize = 5;

/// The budget for key release to text at the 90th percentile (`CLAUDE.md`, R-8).
pub const BUDGET_US: u64 = 800_000;

const HOUR_MS: i64 = 60 * 60 * 1000;
const DAY_MS: i64 = 24 * HOUR_MS;

/// The stages Diagnostics lists, in the order a dictation runs them (spec 182 R-2).
pub const STAGES: &[&str] = &[
    "mic_start",
    "speech",
    "dictionary",
    "cleanup",
    "insert",
    "history_write",
];

/// The stages after key release, the ones that make up key release to text; the slowest of
/// them is named when the budget is missed (R-8).
const AFTER_RELEASE: &[&str] = &["speech", "dictionary", "cleanup", "insert"];

/// The outcomes, in the order Diagnostics lists them (R-4).
pub const OUTCOMES: &[&str] = &["pasted", "copied", "failed", "cancelled", "nothing_heard"];

/// One stage's duration in one trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageTime {
    pub name: String,
    pub duration_us: u64,
}

impl StageTime {
    pub fn new(name: &str, duration_us: u64) -> Self {
        StageTime {
            name: name.to_owned(),
            duration_us,
        }
    }
}

/// One stored trace, reduced to what Diagnostics reads: numbers and names only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceRow {
    pub started_ms: i64,
    pub outcome: String,
    /// The failure's kind, for a failed dictation (spec 182 R-3).
    pub error_kind: Option<String>,
    /// The speech provider and model that answered: the fallback's when one was used.
    pub speech: Option<(String, String)>,
    /// The cleanup provider and model that answered, likewise.
    pub cleanup: Option<(String, String)>,
    /// From key release to the text pasted, for a pasted or copied dictation with a capture.
    pub release_to_text_us: Option<u64>,
    pub stages: Vec<StageTime>,
}

/// The percentiles of one window, or why there are none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Window {
    /// Fewer than [`MIN_VALUES`] values (R-6).
    TooFew { count: usize },
    Ready {
        count: usize,
        p50_us: u64,
        p90_us: u64,
        /// The p90 is over [`BUDGET_US`]; only key release to text is held to the budget.
        over_budget: bool,
    },
}

/// One stage's percentiles over the last 7 days, by provider and model for speech and cleanup
/// (R-3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StageRow {
    pub stage: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub window: Window,
}

/// Everything the Diagnostics section shows except the warnings, which come from their file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostics {
    /// No traces at all in the last 7 days (R-7).
    pub empty: bool,
    /// Key release to text over the last 24 hours (R-2).
    pub last_day: Window,
    /// Key release to text over the last 7 days (R-2).
    pub last_week: Window,
    /// Stages over the last 7 days, in [`STAGES`] order, then provider and model (R-3).
    pub stages: Vec<StageRow>,
    /// Count per outcome over the last 7 days, in [`OUTCOMES`] order (R-4).
    pub outcomes: Vec<(String, usize)>,
    /// Failures by kind over the last 7 days, the most frequent first (R-4).
    pub failures: Vec<(String, usize)>,
    /// When a window is over budget, the stage after key release with the largest p90 (R-8).
    pub slowest_stage: Option<String>,
}

/// Computes the view from the traces of at least the last 7 days. Older rows are ignored.
pub fn compute(traces: &[TraceRow], now_ms: i64) -> Diagnostics {
    let week: Vec<&TraceRow> = traces
        .iter()
        .filter(|t| t.started_ms >= now_ms - 7 * DAY_MS)
        .collect();
    let release = |since: i64| {
        let values = week
            .iter()
            .filter(|t| t.started_ms >= since)
            .filter_map(|t| t.release_to_text_us)
            .collect();
        window(values, true)
    };
    let last_day = release(now_ms - DAY_MS);
    let last_week = release(i64::MIN);

    type Key = (usize, Option<String>, Option<String>);
    let mut groups: BTreeMap<Key, Vec<u64>> = BTreeMap::new();
    let mut by_stage: BTreeMap<&str, Vec<u64>> = BTreeMap::new();
    for trace in &week {
        for stage in &trace.stages {
            let Some(order) = STAGES.iter().position(|s| *s == stage.name) else {
                continue;
            };
            let via = match stage.name.as_str() {
                "speech" => trace.speech.clone(),
                "cleanup" => trace.cleanup.clone(),
                _ => None,
            };
            let (provider, model) = via.map_or((None, None), |(p, m)| (Some(p), Some(m)));
            groups
                .entry((order, provider, model))
                .or_default()
                .push(stage.duration_us);
            by_stage
                .entry(STAGES[order])
                .or_default()
                .push(stage.duration_us);
        }
    }
    let stages = groups
        .into_iter()
        .map(|((order, provider, model), values)| StageRow {
            stage: STAGES[order].to_owned(),
            provider,
            model,
            window: window(values, false),
        })
        .collect();

    let outcomes = OUTCOMES
        .iter()
        .map(|o| {
            let n = week.iter().filter(|t| t.outcome == *o).count();
            ((*o).to_owned(), n)
        })
        .collect();
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
    for trace in &week {
        if trace.outcome == "failed" {
            let kind = trace.error_kind.as_deref().unwrap_or("unknown");
            *kinds.entry(kind).or_default() += 1;
        }
    }
    let mut failures: Vec<(String, usize)> =
        kinds.into_iter().map(|(k, n)| (k.to_owned(), n)).collect();
    failures.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let over = |w: &Window| {
        matches!(
            w,
            Window::Ready {
                over_budget: true,
                ..
            }
        )
    };
    let slowest_stage = if over(&last_day) || over(&last_week) {
        AFTER_RELEASE
            .iter()
            .filter_map(|s| {
                let mut values = by_stage.get(s)?.clone();
                values.sort_unstable();
                Some((percentile(&values, 90), *s))
            })
            .max_by_key(|(p90, _)| *p90)
            .map(|(_, s)| s.to_owned())
    } else {
        None
    };

    Diagnostics {
        empty: week.is_empty(),
        last_day,
        last_week,
        stages,
        outcomes,
        failures,
        slowest_stage,
    }
}

fn window(mut values: Vec<u64>, budget: bool) -> Window {
    if values.len() < MIN_VALUES {
        return Window::TooFew {
            count: values.len(),
        };
    }
    values.sort_unstable();
    let p90_us = percentile(&values, 90);
    Window::Ready {
        count: values.len(),
        p50_us: percentile(&values, 50),
        p90_us,
        over_budget: budget && p90_us > BUDGET_US,
    }
}

/// The nearest-rank percentile of sorted values, as the trace store's `stage_percentiles`.
fn percentile(sorted: &[u64], p: usize) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (p * sorted.len()).div_ceil(100).max(1);
    sorted[rank.min(sorted.len()) - 1]
}
