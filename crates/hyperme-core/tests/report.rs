//! Spec 200 (Report a problem), slice 5: the bundle text and the issue link, built from plain
//! values. Nothing here reads a file or the network.

use hyperme_core::diagnostics::{StageTime, TraceRow, compute};
use hyperme_core::report::{
    ISSUE_URL_MAX, MAX_BYTES, Parts, System, build, issue_url, settings_lines,
};

const HOUR_MS: i64 = 60 * 60 * 1000;
const NOW: i64 = 100 * 24 * HOUR_MS;
const HOME: &str = "/Users/someone";

fn config() -> toml::Table {
    toml::from_str(
        r#"
        [dictation]
        hotkey = "fn"
        min_hold_ms = 250
        sounds = true
        dictionary = "/Users/someone/private-terms.txt"
        [speech]
        provider = "speechmatics"
        model = "melia-1"
        endpoint = "wss://eu.example.internal/v2"
        [cleanup]
        provider = "groq"
        model = "openai/gpt-oss-20b"
        level = "light"
        deadline_ms = 1500
        [history]
        enabled = true
        keep_traces = true
        [connectors.wiki]
        url = "https://wiki.customer-name.example/mcp"
        "#,
    )
    .unwrap()
}

fn row(ago_ms: i64, outcome: &str) -> TraceRow {
    TraceRow {
        started_ms: NOW - ago_ms,
        outcome: outcome.into(),
        error_kind: (outcome == "failed").then(|| "speech_timeout".into()),
        speech: Some(("speechmatics".into(), "melia-1".into())),
        cleanup: Some(("groq".into(), "openai/gpt-oss-20b".into())),
        release_to_text_us: (outcome == "pasted").then_some(420_000),
        stages: vec![
            StageTime::new("speech", 300_000),
            StageTime::new("cleanup", 110_000),
            StageTime::new("insert", 9_000),
        ],
    }
}

fn system() -> System {
    System {
        version: "0.1.0".into(),
        build: "abc1234".into(),
        os: "macOS 26.1".into(),
        chip: "Apple M1 Pro".into(),
        memory_gb: 16,
    }
}

struct Fixture {
    config: toml::Table,
    traces: Vec<TraceRow>,
    warnings: Vec<String>,
    crashes: Vec<(String, String)>,
    reports: Vec<(String, String)>,
}

impl Fixture {
    fn new() -> Self {
        Fixture {
            config: config(),
            traces: (1..=60)
                .map(|i| row(i * HOUR_MS, if i == 3 { "failed" } else { "pasted" }))
                .collect(),
            warnings: vec![
                "2026-11-27 WARN hyperme::speech: retry for ana@example.com in /Users/someone/x"
                    .into(),
            ],
            crashes: vec![(
                "crash-1.txt".into(),
                "message: boom in /Users/someone/y".into(),
            )],
            reports: vec![(
                "HyperMe-1.ips".into(),
                "native crash /Users/someone/z".into(),
            )],
        }
    }

    fn parts(&self) -> Parts<'_> {
        Parts {
            created: "2026-11-27 21:00 +01:00".into(),
            now_ms: NOW,
            description: "It pasted nothing in /Users/someone/app, write to bob@example.org".into(),
            system: system(),
            config: &self.config,
            traces: &self.traces,
            warnings: &self.warnings,
            crashes: &self.crashes,
            reports: &self.reports,
            home: Some(HOME),
        }
    }
}

/// AC-5 (R-17, R-18, R-22): no key, text, path, email, custom endpoint address or connector URL
/// reaches the bundle; the home folder becomes `~`; settings come from the allowlist.
#[test]
fn the_bundle_holds_no_private_text() {
    let f = Fixture::new();
    let parts = f.parts();
    let view = compute(&f.traces, NOW);
    let report = build(&parts, &view);
    let text = &report.text;
    for private in [
        "/Users/someone",
        "ana@example.com",
        "bob@example.org",
        "private-terms",
        "eu.example.internal",
        "customer-name",
    ] {
        assert!(!text.contains(private), "{private} leaked:\n{text}");
    }
    assert!(text.contains("dictation.hotkey = fn"), "{text}");
    assert!(text.contains("speech.endpoint = custom"), "{text}");
    assert!(text.contains("dictation.dictionary = set"), "{text}");
    assert!(text.contains("~/app"), "{text}");
    for section in [
        "== Description (written by you) ==",
        "== This Mac ==",
        "== Settings ==",
        "== Last 7 days ==",
        "== Last 50 dictations ==",
        "== Warnings and errors ==",
        "== Crashes ==",
    ] {
        assert!(text.contains(section), "{section}");
    }
    let dictations = text
        .split("== Last 50 dictations ==")
        .nth(1)
        .unwrap()
        .split("==")
        .next()
        .unwrap();
    assert_eq!(
        dictations.lines().filter(|l| !l.trim().is_empty()).count(),
        50
    );
    assert!(text.contains("speech_timeout"));
    assert!(report.trimmed.is_empty());
}

/// AC-6 (R-17): a setting not on the allowlist, added to the config, stays out of the bundle.
#[test]
fn a_new_setting_is_left_out_until_it_is_allowed() {
    let mut config = config();
    config["dictation"].as_table_mut().unwrap().insert(
        "brand_new_setting".into(),
        toml::Value::String("visible?".into()),
    );
    let lines = settings_lines(&config).join("\n");
    assert!(!lines.contains("brand_new_setting"), "{lines}");
    assert!(!lines.contains("visible?"));
    assert!(lines.contains("cleanup.level = light"));
    assert!(lines.contains("history.keep_traces = on"));
}

/// AC-7 (R-15, R-15a): the issue link opens the bug template with the context prefilled, under
/// 6,000 characters even with many traces, with no warning line, crash text or description; a
/// report started from a failed item puts that item first.
#[test]
fn the_issue_link_holds_the_context_and_no_free_text() {
    let mut f = Fixture::new();
    f.traces = (1..=10_000).map(|i| row(i * 1000, "pasted")).collect();
    f.traces[500] = row(500 * 1000, "failed");
    let parts = f.parts();
    let view = compute(&f.traces, NOW);
    let url = issue_url(&parts, &view, Some(500));
    assert!(url.len() <= ISSUE_URL_MAX, "{}", url.len());
    assert!(
        url.starts_with("https://github.com/AndresHG/hyperme/issues/new?template=bug_report.yml")
    );
    let decoded = decode(&url);
    assert!(
        decoded.contains("title=Problem report: HyperMe 0.1.0"),
        "{decoded}"
    );
    assert!(decoded.contains("version=0.1.0 (abc1234)"));
    assert!(decoded.contains("system=macOS 26.1, Apple M1 Pro, 16 GB"));
    assert!(decoded.contains("providers=speechmatics melia-1, groq openai/gpt-oss-20b"));
    assert!(decoded.contains("cleanup.level = light"), "{decoded}");
    let timing = decoded.split("timing=").nth(1).unwrap();
    let first_trace = timing.lines().find(|l| l.contains(" ago ")).unwrap();
    assert!(first_trace.contains("failed"), "{first_trace}");
    for free in [
        "boom",
        "native crash",
        "pasted nothing",
        "retry for",
        "/Users/",
        "~/",
    ] {
        assert!(!decoded.contains(free), "{free} in the link");
    }
}

/// AC-8 (R-19): a huge warnings list and many crash files give a bundle under 1 MB that says
/// what was trimmed.
#[test]
fn a_huge_bundle_is_trimmed_under_one_megabyte() {
    let mut f = Fixture::new();
    f.warnings = (0..200)
        .map(|i| format!("line {i} {}", "w ".repeat(12_500)))
        .collect();
    f.crashes = (0..10)
        .map(|i| (format!("crash-{i}.txt"), "c ".repeat(20_000)))
        .collect();
    let parts = f.parts();
    let view = compute(&f.traces, NOW);
    let report = build(&parts, &view);
    assert!(report.text.len() <= MAX_BYTES, "{}", report.text.len());
    assert!(!report.trimmed.is_empty());
    assert!(report.text.contains("== Trimmed =="));
    assert!(report.text.contains("line 0 "), "the newest warning stays");
}

fn decode(url: &str) -> String {
    let bytes = url.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap();
            out.push(u8::from_str_radix(hex, 16).unwrap());
            i += 3;
        } else {
            out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
            i += 1;
        }
    }
    String::from_utf8(out).unwrap()
}

/// R-16: the app opens only links this module built; a broken config gives no settings.
#[test]
fn only_issue_links_are_opened_and_a_broken_config_shows_nothing() {
    let f = Fixture::new();
    let parts = f.parts();
    let url = issue_url(&parts, &compute(&f.traces, NOW), None);
    assert!(hyperme_core::report::is_issue_url(&url));
    assert!(!hyperme_core::report::is_issue_url(
        "https://example.com/?x"
    ));
    assert!(!hyperme_core::report::is_issue_url("file:///etc/passwd"));
    let broken = hyperme_core::report::config_table("not [valid");
    assert!(settings_lines(&broken).is_empty());
}
