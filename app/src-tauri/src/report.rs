//! Report a problem (specs/200-diagnostics, slice 5): the app's side. It gathers the parts the
//! core's `report` module turns into the bundle, saves the file where the user says, and opens
//! the issue page in the browser. HyperMe itself sends nothing (R-16).

use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use hyperme_core::diagnostics;
use hyperme_core::report::{self, Parts, System};
use hyperme_platform::crash::{CrashFiles, LocalCrashFiles};
use hyperme_platform::log_file;
use serde::Serialize;
use tauri::AppHandle;

use crate::runtime;

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Serialize, specta::Type)]
pub struct ReportView {
    /// The whole bundle, shown before anything else happens (R-13).
    text: String,
    trimmed: Vec<String>,
    /// The new-issue link with the context of R-15a.
    url: String,
    /// macOS has crash reports for HyperMe, so the preview offers the switch (R-24).
    has_reports: bool,
}

fn output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default()
}

fn system() -> System {
    let memory: u64 = output("sysctl", &["-n", "hw.memsize"]).parse().unwrap_or(0);
    System {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        build: option_env!("HYPERME_COMMIT")
            .unwrap_or("local build")
            .to_owned(),
        os: format!("macOS {}", output("sw_vers", &["-productVersion"])),
        chip: output("sysctl", &["-n", "machdep.cpu.brand_string"]),
        memory_gb: memory / (1024 * 1024 * 1024),
    }
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Builds the bundle and the issue link. `from_item` is the History item a report started from:
/// its trace leads the link (R-15a).
#[tauri::command]
#[specta::specta]
pub fn report_build(
    app: AppHandle,
    description: String,
    include_reports: bool,
    from_item: Option<i64>,
) -> ReportView {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64);
    let store = runtime::trace_store(&app);
    let read = |since, limit| {
        store
            .as_ref()
            .and_then(|s| s.traces_since(since, limit).ok())
            .unwrap_or_default()
    };
    let week: Vec<_> = read(now - 7 * DAY_MS, 5_000)
        .iter()
        .map(|t| t.row())
        .collect();
    let view = diagnostics::compute(&week, now);
    let mut stored = read(i64::MIN, report::TRACES);
    let mut first = None;
    if let Some(id) = from_item {
        first = stored.iter().position(|t| t.history_id == Some(id));
        if first.is_none()
            && let Some(Ok(Some(trace))) = store.as_ref().map(|s| s.trace_for(id))
        {
            stored.insert(0, trace);
            first = Some(0);
        }
    }
    let traces: Vec<_> = stored.iter().map(|t| t.row()).collect();

    let config_text = runtime::config_file(&app)
        .load_with_text()
        .map(|(text, _)| text.to_string())
        .unwrap_or_else(|_| {
            // An invalid file is still worth showing in the report, as it is on disk.
            std::fs::read_to_string(runtime::config_file(&app).path()).unwrap_or_default()
        });
    let config = report::config_table(&config_text);
    let warnings = log_file::last_lines(&runtime::warnings_path(&app), report::WARNINGS);
    let home = home();
    let files = home
        .as_deref()
        .map(|h| LocalCrashFiles::for_home(h, env!("CARGO_PKG_VERSION")));
    let pairs = |texts: Vec<hyperme_platform::crash::CrashText>| {
        texts
            .into_iter()
            .map(|c| (c.name, c.text))
            .collect::<Vec<_>>()
    };
    let crashes = files.as_ref().map_or_else(Vec::new, |f| pairs(f.crashes()));
    let system_reports = files
        .as_ref()
        .map_or_else(Vec::new, |f| pairs(f.system_reports()));
    let has_reports = !system_reports.is_empty();
    let reports = if include_reports {
        system_reports
    } else {
        Vec::new()
    };
    let home_text = home.map(|h| h.to_string_lossy().into_owned());
    let parts = Parts {
        created: output("date", &["+%Y-%m-%d %H:%M %z"]),
        now_ms: now,
        description,
        system: system(),
        config: &config,
        traces: &traces,
        warnings: &warnings,
        crashes: &crashes,
        reports: &reports,
        home: home_text.as_deref(),
    };
    let built = report::build(&parts, &view);
    ReportView {
        url: report::issue_url(&parts, &view, first),
        text: built.text,
        trimmed: built.trimmed,
        has_reports,
    }
}

fn file_name() -> String {
    format!(
        "HyperMe problem report {}.txt",
        output("date", &["+%Y-%m-%d %H.%M"])
    )
}

/// Save…: macOS's own save panel picks the file (through `osascript`, so no new crate), then
/// the bundle is written there. `None` when the user cancels (R-15).
#[tauri::command]
#[specta::specta]
pub fn report_save(text: String) -> Result<Option<String>, String> {
    let script = format!(
        "POSIX path of (choose file name with prompt \"Save the problem report\" default name {} default location (path to downloads folder))",
        serde_json::json!(file_name())
    );
    let path = output("osascript", &["-e", &script]);
    if path.is_empty() {
        return Ok(None);
    }
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(Some(path))
}

/// Open issue: saves the bundle to Downloads, opens the new-issue page in the browser and shows
/// the saved file in Finder so the user can attach it (R-15). Opens only links the core built.
#[tauri::command]
#[specta::specta]
pub fn report_open_issue(text: String, url: String) -> Result<String, String> {
    if !report::is_issue_url(&url) {
        return Err("Not an issue link".to_owned());
    }
    let downloads = home()
        .ok_or_else(|| "No home folder".to_owned())?
        .join("Downloads");
    std::fs::create_dir_all(&downloads).map_err(|e| e.to_string())?;
    let path = downloads.join(file_name());
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Command::new("open")
        .arg(&url)
        .spawn()
        .map_err(|e| e.to_string())?;
    let _ = Command::new("open").arg("-R").arg(&path).spawn();
    Ok(path.display().to_string())
}
