//! The Diagnostics section of Settings (specs/200-diagnostics, slice 2): the numbers the core
//! computes from the trace store, and the last warnings. Read when the section opens and on
//! Refresh, never in the background (R-9).

use std::time::{SystemTime, UNIX_EPOCH};

use hyperme_core::diagnostics::{self, Diagnostics};
use hyperme_platform::log_file;
use serde::Serialize;
use tauri::AppHandle;

use crate::runtime;

const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// The most traces one view reads: a week of heavy use, well under a megabyte (design.md).
const LIMIT: usize = 5_000;
/// Warnings shown (R-5).
const WARNINGS: usize = 20;

#[derive(Debug, Serialize, specta::Type)]
pub struct DiagnosticsView {
    /// The trace store exists, so the section is shown (R-1).
    shown: bool,
    /// Keep timing traces is on (R-7).
    keep: bool,
    view: Option<Diagnostics>,
    /// The last warnings and errors, newest first (R-5).
    warnings: Vec<String>,
    /// A read that failed, in one line.
    error: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub fn diagnostics_view(app: AppHandle) -> DiagnosticsView {
    let keep = runtime::config(&app)
        .map(|c| c.history.keep_traces)
        .unwrap_or(true);
    let warnings = log_file::last_lines(&runtime::warnings_path(&app), WARNINGS);
    let Some(store) = runtime::trace_store(&app) else {
        return DiagnosticsView {
            shown: false,
            keep,
            view: None,
            warnings,
            error: None,
        };
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64);
    let read = store.traces_since(now - 7 * DAY_MS, LIMIT).map(|traces| {
        let rows: Vec<_> = traces.iter().map(|t| t.row()).collect();
        diagnostics::compute(&rows, now)
    });
    let (view, error) = match read {
        Ok(view) => (Some(view), None),
        Err(e) => (None, Some(e.to_string())),
    };
    DiagnosticsView {
        shown: true,
        keep,
        view,
        warnings,
        error,
    }
}

/// One stage bar of a History item (R-10), in milliseconds from key-down.
#[derive(Debug, Serialize, specta::Type)]
pub struct BarView {
    stage: String,
    start_ms: f64,
    duration_ms: f64,
}

/// A History item's trace for its stage bars (R-10): the bars, the providers and models that
/// answered, and the outcome.
#[derive(Debug, Serialize, specta::Type)]
pub struct TraceView {
    bars: Vec<BarView>,
    speech: Option<(String, String)>,
    cleanup: Option<(String, String)>,
    outcome: String,
}

/// The trace of one History item, or none: traces off, or older than their retention (R-11).
#[tauri::command]
#[specta::specta]
pub fn history_trace(app: AppHandle, id: i64) -> Result<Option<TraceView>, String> {
    let Some(store) = runtime::trace_store(&app) else {
        return Ok(None);
    };
    let trace = store.trace_for(id).map_err(|e| e.to_string())?;
    Ok(trace.map(|trace| {
        let row = trace.row();
        let ms = |us: u64| us as f64 / 1000.0;
        TraceView {
            bars: trace
                .bars()
                .into_iter()
                .map(|b| BarView {
                    stage: b.stage,
                    start_ms: ms(b.start_us),
                    duration_ms: ms(b.duration_us),
                })
                .collect(),
            speech: row.speech,
            cleanup: row.cleanup,
            outcome: row.outcome,
        }
    }))
}
