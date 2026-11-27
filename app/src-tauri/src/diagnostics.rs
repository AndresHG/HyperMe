//! The Diagnostics section of Settings (specs/200-diagnostics, slice 2): the numbers the core
//! computes from the trace store, and the last warnings. Read when the section opens and on
//! Refresh, never in the background (R-9).

use std::time::{SystemTime, UNIX_EPOCH};

use hyperme_core::config::Config;
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

#[derive(Debug, Serialize)]
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
pub fn diagnostics_view(app: AppHandle) -> DiagnosticsView {
    let keep = Config::load_or_create(&runtime::data_dir(&app).join("config.toml"))
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
