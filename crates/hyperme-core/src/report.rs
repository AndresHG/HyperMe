//! Report a problem (specs/200-diagnostics, slice 5): the bundle text a user reads in full and
//! may attach to an issue, and the issue link the browser opens. Pure: the app reads the files
//! and the store and hands everything here as plain values. HyperMe sends nothing itself (R-16).
//!
//! Settings enter only from an allowlist (R-17), so a new setting stays out until it is added
//! here. Every free-text line (warnings, crash files, system reports, the user's description)
//! passes through the warnings file's scrub as a second net (design.md).

use std::fmt::Write as _;

use crate::diagnostics::{Diagnostics, TraceRow, Window};
use crate::warnings::scrub;

/// The most the bundle holds (R-19).
pub const MAX_BYTES: usize = 1024 * 1024;
/// Traces in the bundle (R-14).
pub const TRACES: usize = 50;
/// Warning lines in the bundle (R-14).
pub const WARNINGS: usize = 200;
/// The longest issue link (R-15a).
pub const ISSUE_URL_MAX: usize = 6_000;
/// Where a new issue is opened, with the repository's bug report form.
pub const ISSUE_NEW: &str = "https://github.com/AndresHG/hyperme/issues/new";

/// How an allowed setting is shown.
#[derive(Clone, Copy)]
enum Show {
    /// The value as written: a name, a number, a level.
    AsIs,
    /// A switch: on or off.
    OnOff,
    /// An address or a path: only whether one is set, never what it is.
    Custom,
    Set,
}

/// The settings the bundle may show (R-17).
const ALLOWED: &[(&str, Show)] = &[
    ("dictation.hotkey", Show::AsIs),
    ("dictation.min_hold_ms", Show::AsIs),
    ("dictation.double_tap_ms", Show::AsIs),
    ("dictation.max_length_s", Show::AsIs),
    ("dictation.short_dictation_words", Show::AsIs),
    ("dictation.sounds", Show::OnOff),
    ("dictation.show_while_idle", Show::OnOff),
    ("dictation.dictionary", Show::Set),
    ("dictation.dictionary_prompt_cap", Show::AsIs),
    ("speech.provider", Show::AsIs),
    ("speech.model", Show::AsIs),
    ("speech.endpoint", Show::Custom),
    ("speech.fallback.provider", Show::AsIs),
    ("speech.fallback.model", Show::AsIs),
    ("cleanup.provider", Show::AsIs),
    ("cleanup.model", Show::AsIs),
    ("cleanup.endpoint", Show::Custom),
    ("cleanup.reasoning_effort", Show::AsIs),
    ("cleanup.level", Show::AsIs),
    ("cleanup.prompt", Show::AsIs),
    ("cleanup.deadline_ms", Show::AsIs),
    ("cleanup.fallback.provider", Show::AsIs),
    ("cleanup.fallback.model", Show::AsIs),
    ("history.enabled", Show::OnOff),
    ("history.retention_days", Show::AsIs),
    ("history.size_limit_mb", Show::AsIs),
    ("history.keep_traces", Show::OnOff),
    ("timing.log", Show::OnOff),
];

/// Facts about the Mac and the build (R-14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct System {
    pub version: String,
    pub build: String,
    /// The system and its version, such as "macOS 26.1".
    pub os: String,
    /// The chip family, such as "Apple M1 Pro".
    pub chip: String,
    pub memory_gb: u64,
}

/// Everything a report is built from.
#[derive(Debug)]
pub struct Parts<'a> {
    /// The creation time with its offset only, such as "2026-12-03 10:41 +01:00".
    pub created: String,
    pub now_ms: i64,
    /// What the user typed in the preview; may be empty.
    pub description: String,
    pub system: System,
    /// The user's config file as a TOML table.
    pub config: &'a toml::Table,
    /// Traces newest first.
    pub traces: &'a [TraceRow],
    /// Warning lines newest first.
    pub warnings: &'a [String],
    /// HyperMe's crash files, newest first, as name and text.
    pub crashes: &'a [(String, String)],
    /// The system's crash reports, already cut, as name and text; empty when the user left
    /// them out.
    pub reports: &'a [(String, String)],
    pub home: Option<&'a str>,
}

/// A built bundle and what its size cap left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub text: String,
    pub trimmed: Vec<String>,
}

/// The user's config file as a table for [`settings_lines`]; an unreadable file gives none.
pub fn config_table(text: &str) -> toml::Table {
    toml::from_str(text).unwrap_or_default()
}

/// Whether a link is a new-issue link this module built, the only kind the app opens (R-16).
pub fn is_issue_url(url: &str) -> bool {
    url.starts_with(&format!("{ISSUE_NEW}?template=bug_report.yml&"))
}

/// The allowed settings present in the config, one `path = value` per line (R-17).
pub fn settings_lines(config: &toml::Table) -> Vec<String> {
    ALLOWED
        .iter()
        .filter_map(|(path, show)| {
            let value = lookup(config, path)?;
            let shown = match show {
                Show::AsIs => match value {
                    toml::Value::String(s) => s.clone(),
                    toml::Value::Integer(n) => n.to_string(),
                    toml::Value::Float(f) => f.to_string(),
                    toml::Value::Boolean(b) => b.to_string(),
                    _ => return None,
                },
                Show::OnOff => match value.as_bool()? {
                    true => "on".to_owned(),
                    false => "off".to_owned(),
                },
                Show::Custom => "custom".to_owned(),
                Show::Set => "set".to_owned(),
            };
            Some(format!("{path} = {shown}"))
        })
        .collect()
}

fn lookup<'a>(config: &'a toml::Table, path: &str) -> Option<&'a toml::Value> {
    let mut parts = path.split('.');
    let mut value = config.get(parts.next()?)?;
    for part in parts {
        value = value.as_table()?.get(part)?;
    }
    Some(value)
}

/// Builds the bundle (R-14), trimming the oldest traces, warnings and crash texts first until
/// it fits in [`MAX_BYTES`] (R-19). Each free-text part is scrubbed once; trimming only counts
/// bytes, so a full store and a large warnings file still build quickly (R-20).
pub fn build(parts: &Parts<'_>, view: &Diagnostics) -> Report {
    let home = parts.home;
    let traces: Vec<String> = parts
        .traces
        .iter()
        .take(TRACES)
        .map(|t| trace_line(t, parts.now_ms) + "\n")
        .collect();
    let warnings: Vec<String> = parts
        .warnings
        .iter()
        .take(WARNINGS)
        .map(|w| scrub(w, home) + "\n")
        .collect();
    let crash = |(name, text): &(String, String)| {
        format!("-- {name} --\n{}\n", scrub(text, home).trim_end())
    };
    let crashes: Vec<String> = parts.crashes.iter().map(crash).collect();
    let reports: Vec<String> = parts.reports.iter().map(crash).collect();
    let head = head(parts, view);

    // Room for the section titles, the "(none)" lines and the Trimmed section.
    const SLACK: usize = 1024;
    let size = |list: &[String], n: usize| list[..n].iter().map(String::len).sum::<usize>();
    let (mut t, mut w, mut c, mut r) = (traces.len(), warnings.len(), crashes.len(), reports.len());
    let total = |t, w, c, r| {
        head.len() + size(&traces, t) + size(&warnings, w) + size(&crashes, c) + size(&reports, r)
    };
    while total(t, w, c, r) + SLACK > MAX_BYTES {
        if t > 0 {
            t -= 1;
        } else if w > 0 {
            w -= 1;
        } else if c > 0 {
            c -= 1;
        } else if r > 0 {
            r -= 1;
        } else {
            break;
        }
    }
    let mut trimmed = Vec::new();
    for (what, kept, had) in [
        ("dictations", t, traces.len()),
        ("warning lines", w, warnings.len()),
        ("crash files", c, crashes.len()),
        ("system crash reports", r, reports.len()),
    ] {
        if kept < had {
            trimmed.push(format!("{} older {what} left out", had - kept));
        }
    }

    let mut text = head;
    let _ = writeln!(text, "\n== Last {TRACES} dictations ==");
    traces[..t].iter().for_each(|l| text.push_str(l));
    let _ = writeln!(text, "\n== Warnings and errors ==");
    if w == 0 {
        let _ = writeln!(text, "(none)");
    }
    warnings[..w].iter().for_each(|l| text.push_str(l));
    let _ = writeln!(text, "\n== Crashes ==");
    if c == 0 && r == 0 {
        let _ = writeln!(text, "(none)");
    }
    crashes[..c]
        .iter()
        .chain(&reports[..r])
        .for_each(|l| text.push_str(l));
    if !trimmed.is_empty() {
        let _ = writeln!(text, "\n== Trimmed ==");
        for line in &trimmed {
            let _ = writeln!(text, "{line}");
        }
    }
    cut(&mut text, MAX_BYTES);
    Report { text, trimmed }
}

/// The bundle up to the traces: the description, the Mac, the settings and the numbers.
fn head(parts: &Parts<'_>, view: &Diagnostics) -> String {
    let mut t = String::new();
    let _ = writeln!(t, "HyperMe problem report");
    let _ = writeln!(t, "Created: {}", parts.created);
    let _ = writeln!(t, "\n== Description (written by you) ==");
    let description = scrub(parts.description.trim(), parts.home);
    let _ = writeln!(
        t,
        "{}",
        if description.is_empty() {
            "(none)"
        } else {
            &description
        }
    );
    let _ = writeln!(t, "\n== This Mac ==");
    let s = &parts.system;
    let _ = writeln!(t, "HyperMe {} ({})", s.version, s.build);
    let _ = writeln!(t, "{}, {}, {} GB", s.os, s.chip, s.memory_gb);
    let _ = writeln!(t, "\n== Settings ==");
    for line in settings_lines(parts.config) {
        let _ = writeln!(t, "{line}");
    }
    let _ = writeln!(t, "\n== Last 7 days ==");
    for line in diagnostics_lines(view) {
        let _ = writeln!(t, "{line}");
    }
    t
}

/// The Diagnostics numbers as text lines: numbers and names only.
fn diagnostics_lines(view: &Diagnostics) -> Vec<String> {
    let mut lines = vec![
        format!("key release to text, 24 h: {}", window_text(&view.last_day)),
        format!(
            "key release to text, 7 days: {}",
            window_text(&view.last_week)
        ),
    ];
    if let Some(stage) = &view.slowest_stage {
        lines.push(format!("over the 800 ms budget; slowest stage: {stage}"));
    }
    for s in &view.stages {
        let via = match (&s.provider, &s.model) {
            (Some(p), Some(m)) => format!(" ({p} {m})"),
            _ => String::new(),
        };
        lines.push(format!("{}{via}: {}", s.stage, window_text(&s.window)));
    }
    let outcomes: Vec<String> = view
        .outcomes
        .iter()
        .map(|(o, n)| format!("{o} {n}"))
        .collect();
    lines.push(format!("outcomes: {}", outcomes.join(", ")));
    if !view.failures.is_empty() {
        let failures: Vec<String> = view
            .failures
            .iter()
            .map(|(k, n)| format!("{k} {n}"))
            .collect();
        lines.push(format!("failures: {}", failures.join(", ")));
    }
    lines
}

fn window_text(window: &Window) -> String {
    match window {
        Window::TooFew { count } => format!("too few ({count})"),
        Window::Ready {
            count,
            p50_us,
            p90_us,
            ..
        } => format!(
            "median {} ms, 90% {} ms, {count} dictations",
            p50_us / 1000,
            p90_us / 1000
        ),
    }
}

/// One trace as a line: how long ago, the outcome, the error kind, each stage in ms, and the
/// providers. Offsets, not clock times, so a bundle does not show when someone works.
fn trace_line(trace: &TraceRow, now_ms: i64) -> String {
    let mut line = format!("{} ago  {}", ago(now_ms - trace.started_ms), trace.outcome);
    if let Some(kind) = &trace.error_kind {
        let _ = write!(line, " ({kind})");
    }
    if let Some(us) = trace.release_to_text_us {
        let _ = write!(line, "  total {} ms", us / 1000);
    }
    let stages: Vec<String> = trace
        .stages
        .iter()
        .filter(|s| s.name != "capture")
        .map(|s| format!("{} {}", s.name, s.duration_us / 1000))
        .collect();
    if !stages.is_empty() {
        let _ = write!(line, "  [{}]", stages.join(", "));
    }
    for (provider, model) in [&trace.speech, &trace.cleanup].into_iter().flatten() {
        let _ = write!(line, "  {provider} {model}");
    }
    line
}

fn ago(ms: i64) -> String {
    let minutes = ms.max(0) / 60_000;
    match minutes {
        0 => "<1 min".to_owned(),
        m if m < 60 => format!("{m} min"),
        m if m < 48 * 60 => format!("{} h {} min", m / 60, m % 60),
        m => format!("{} days", m / (24 * 60)),
    }
}

/// The new-issue link (R-15, R-15a): the bug report form with its title, version, system and
/// providers filled, and in its timing field the settings, Diagnostics and as many recent traces
/// as fit under [`ISSUE_URL_MAX`], `first` (a failed item's trace) leading. Numbers and allowed
/// names only: no warning line, crash text or description.
pub fn issue_url(parts: &Parts<'_>, view: &Diagnostics, first: Option<usize>) -> String {
    let s = &parts.system;
    let providers = [
        lookup(parts.config, "speech.provider"),
        lookup(parts.config, "speech.model"),
        lookup(parts.config, "cleanup.provider"),
        lookup(parts.config, "cleanup.model"),
    ]
    .map(|v| v.and_then(toml::Value::as_str).unwrap_or("?").to_owned());
    let head = [
        ("template", "bug_report.yml".to_owned()),
        ("title", format!("Problem report: HyperMe {}", s.version)),
        ("version", format!("{} ({})", s.version, s.build)),
        (
            "system",
            format!("{}, {}, {} GB", s.os, s.chip, s.memory_gb),
        ),
        (
            "providers",
            format!(
                "{} {}, {} {}",
                providers[0], providers[1], providers[2], providers[3]
            ),
        ),
    ];
    let mut url = String::from(ISSUE_NEW);
    for (i, (key, value)) in head.iter().enumerate() {
        url.push(if i == 0 { '?' } else { '&' });
        url.push_str(key);
        url.push('=');
        url.push_str(&encode(value));
    }
    let mut timing = String::from(
        "Prefilled by HyperMe's Report a problem. The saved file has more.\n\nSettings\n",
    );
    for line in settings_lines(parts.config) {
        let _ = writeln!(timing, "{line}");
    }
    timing.push_str("\nLast 7 days\n");
    for line in diagnostics_lines(view) {
        let _ = writeln!(timing, "{line}");
    }
    timing.push_str("\nRecent dictations, newest first\n");
    let order = first
        .into_iter()
        .chain((0..parts.traces.len()).filter(|i| Some(*i) != first));
    let base = url.len() + "&timing=".len();
    let mut encoded = encode(&timing);
    for i in order {
        let Some(trace) = parts.traces.get(i) else {
            continue;
        };
        let more = encode(&format!("{}\n", trace_line(trace, parts.now_ms)));
        if base + encoded.len() + more.len() > ISSUE_URL_MAX {
            break;
        }
        encoded.push_str(&more);
    }
    if base + encoded.len() > ISSUE_URL_MAX {
        // The settings and numbers alone are too long: keep what fits on a character boundary.
        let mut end = ISSUE_URL_MAX - base;
        while end > 0 && encoded.as_bytes().get(end - 1) == Some(&b'%') {
            end -= 1;
        }
        if end >= 2 && encoded.as_bytes()[end - 2] == b'%' {
            end -= 2;
        }
        encoded.truncate(end);
    }
    url.push_str("&timing=");
    url.push_str(&encoded);
    url
}

/// Percent-encodes a query value: letters, digits and `-._~` stay, everything else is escaped.
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => {
                let _ = write!(out, "%{b:02X}");
            }
        }
    }
    out
}

fn cut(text: &mut String, max: usize) {
    if text.len() > max {
        let mut end = max;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
}
