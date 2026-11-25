//! Output: the stage log, the results table, the gate and the results file.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::lang::Lang;
use crate::pipeline::{Measured, Pipeline, SampleResult};
use crate::stats::{GATE_MEDIAN_MS, GATE_P90_MS, Quantiles, Row, gate_problems};

/// Samples per language in the full gate run.
const PLANNED_PER_LANGUAGE: usize = 25;
const NOT_FILLED_IN: &str = "_not filled in_";

/// Everything about a run that is not a measurement.
pub struct RunInfo {
    /// `YYYY-MM-DD`, local.
    pub date: String,
    /// Local time and zone.
    pub clock: String,
    /// The run used the fake providers: its numbers are simulated.
    pub fake: bool,
    pub machine: String,
    pub network: String,
    pub commit: String,
    /// Providers, models and flags, as label and value.
    pub settings: Vec<(&'static str, String)>,
}

/// One line per measurement: every stage, in milliseconds from key release.
pub fn stage_line(result: &SampleResult) -> String {
    let head = format!(
        "{:<8} {}  run {}  audio {:>5.1} s",
        result.sample,
        result.pipeline.label(),
        result.run,
        result.audio_ms as f64 / 1000.0
    );
    match &result.outcome {
        Err(error) => format!("{head} | FAILED: {error}"),
        Ok(m) => {
            let mut line = format!(
                "{head} | open {:>6}  last audio {:>4}  stt {:>5}  first {:>5}  total {:>5}",
                m.open_ms,
                opt(m.last_audio_ms),
                m.stt_ms,
                opt(m.first_ms),
                m.total_ms
            );
            match m.lang_ok {
                Some(true) => line.push_str(" | lang ok"),
                Some(false) => line.push_str(" | LANG WRONG"),
                None => {}
            }
            if let Some(note) = &m.fallback_note {
                let _ = write!(line, " | FALLBACK: {note}");
            }
            line
        }
    }
}

fn opt<T: ToString>(value: Option<T>) -> String {
    value.map_or("-".to_owned(), |v| v.to_string())
}

fn lang_label(lang: Option<Lang>) -> &'static str {
    lang.map_or("all", Lang::code)
}

fn pair(quantiles: Option<Quantiles>) -> (String, String) {
    (opt(quantiles.map(|q| q.p50)), opt(quantiles.map(|q| q.p90)))
}

/// The results table for the terminal.
pub fn console_table(rows: &[Row]) -> String {
    let mut out = format!(
        "{:<9} {:<4} {:>3}  {:>13}  {:>13}  {:>13}  {:>7}  {:>9}  {:>6}\n",
        "pipeline",
        "lang",
        "n",
        "stt p50/p90",
        "first p50/p90",
        "total p50/p90",
        "lang_ok",
        "fallbacks",
        "failed"
    );
    for row in rows {
        let cell = |q| {
            let (p50, p90) = pair(q);
            format!("{p50} / {p90}")
        };
        let _ = writeln!(
            out,
            "{:<9} {:<4} {:>3}  {:>13}  {:>13}  {:>13}  {:>7}  {:>9}  {:>6}",
            row.pipeline.label(),
            lang_label(row.lang),
            row.n,
            cell(row.stt),
            cell(row.first),
            cell(row.total),
            format!("{}/{}", row.lang_ok, row.lang_checked),
            row.fallbacks,
            row.failed
        );
    }
    out
}

/// The gate, in plain words: one line per pipeline, then the conclusion.
pub fn gate_lines(rows: &[Row], results: &[SampleResult]) -> Vec<String> {
    let mut lines = vec![format!(
        "Gate, over all samples: median total_ms at or under {GATE_MEDIAN_MS}; 90th percentile at \
         or under {GATE_P90_MS}; language correct on every sample; no failed sample; the raw \
         transcript used instead of cleaned text on at most one sample in ten."
    )];
    let mut passed = Vec::new();
    for row in rows.iter().filter(|row| row.lang.is_none()) {
        let problems = gate_problems(row);
        let total = row.total.map_or(String::new(), |q| {
            format!(" (median {}, 90th percentile {})", q.p50, q.p90)
        });
        if problems.is_empty() {
            passed.push(row.pipeline.label());
            lines.push(format!("- {}: PASS{total}", row.pipeline.label()));
        } else {
            lines.push(format!(
                "- {}: FAIL: {}",
                row.pipeline.label(),
                problems.join("; ")
            ));
        }
    }
    lines.push(if passed.is_empty() {
        "The speed and language gate did NOT pass on any pipeline.".to_owned()
    } else {
        format!(
            "The speed and language gate PASSED on: {}.",
            passed.join(", ")
        )
    });
    let (es, en) = (
        sample_count(results, Lang::Es),
        sample_count(results, Lang::En),
    );
    if (es, en) != (PLANNED_PER_LANGUAGE, PLANNED_PER_LANGUAGE) {
        lines.push(format!(
            "This run had {es} Spanish and {en} English samples, not the planned \
             {PLANNED_PER_LANGUAGE} and {PLANNED_PER_LANGUAGE}, so it is not the full gate run."
        ));
    }
    lines.push(
        "Formatting quality is not judged here: the owner reads the texts and compares them \
         with Wispr Flow."
            .to_owned(),
    );
    lines
}

fn sample_count(results: &[SampleResult], lang: Lang) -> usize {
    let mut names: Vec<&str> = results
        .iter()
        .filter(|r| r.lang == Some(lang))
        .map(|r| r.sample.as_str())
        .collect();
    names.sort_unstable();
    names.dedup();
    names.len()
}

/// The first run of each sample and pipeline, in the order they ran.
fn first_runs(results: &[SampleResult]) -> impl Iterator<Item = &SampleResult> {
    results.iter().filter(|r| r.run == 1)
}

/// Raw and cleaned text of each sample, for reading in the terminal.
pub fn console_texts(results: &[SampleResult]) -> String {
    let mut out = String::new();
    for result in first_runs(results) {
        let _ = writeln!(out, "{} ({})", result.sample, result.pipeline.label());
        match &result.outcome {
            Err(error) => {
                let _ = writeln!(out, "  failed: {error}");
            }
            Ok(m) => out.push_str(&text_lines(m)),
        }
    }
    out
}

/// Raw and cleaned text of one measurement, indented for the terminal.
pub fn text_lines(m: &Measured) -> String {
    let cleaned = if m.fallback {
        "(none: the raw transcript was used)".to_owned()
    } else {
        m.text.replace('\n', "\n           ")
    };
    format!("  raw:     {}\n  cleaned: {cleaned}\n", m.raw)
}

/// The results file.
pub fn markdown(info: &RunInfo, rows: &[Row], results: &[SampleResult]) -> String {
    let mut md = format!("# Speed test, {}\n\n", info.date);
    if info.fake {
        md.push_str(
            "> **Fake providers.** Every number and text below is simulated by the tool itself. \
             This file is not a measurement and is not evidence for the gate.\n\n",
        );
    }

    md.push_str("## Setup\n\n| Item | Value |\n| --- | --- |\n");
    let _ = writeln!(md, "| Date | {} {} |", info.date, info.clock);
    let _ = writeln!(md, "| Machine | {} |", info.machine);
    let _ = writeln!(md, "| Network | {} |", info.network);
    let _ = writeln!(
        md,
        "| Tool | `spikes/flowbench` at commit {} |",
        info.commit
    );
    for (label, value) in &info.settings {
        let _ = writeln!(md, "| {label} | {} |", cell(value));
    }

    md.push_str(
        "\n## Results\n\n\
         Times are in milliseconds from key release: the start of the upload in pipeline A, the \
         commit in pipeline B. `stt_ms` is the final transcript, `first_ms` the first cleaned \
         token, `total_ms` the full text. Each cell is median / 90th percentile.\n\n\
         | Pipeline | Language | Measurements | `stt_ms` | `first_ms` | `total_ms` | `lang_ok` | Fallbacks | Failed |\n\
         | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n",
    );
    for row in rows {
        let cell = |q| {
            let (p50, p90) = pair(q);
            format!("{p50} / {p90}")
        };
        let _ = writeln!(
            md,
            "| {} | {} | {} | {} | {} | {} | {}/{} | {} | {} |",
            row.pipeline.label(),
            lang_label(row.lang),
            row.n,
            cell(row.stt),
            cell(row.first),
            cell(row.total),
            row.lang_ok,
            row.lang_checked,
            row.fallbacks,
            row.failed
        );
    }

    md.push_str("\n## Gate\n\n");
    let lines = gate_lines(rows, results);
    for (i, line) in lines.iter().enumerate() {
        // List items stay together; everything else gets its own paragraph.
        let in_list =
            line.starts_with("- ") && lines.get(i + 1).is_some_and(|next| next.starts_with("- "));
        let gap = if in_list { "\n" } else { "\n\n" };
        let _ = write!(md, "{line}{gap}");
    }

    md.push_str(
        "## Stage log\n\n\
         Milliseconds from key release; a negative value is before the release. On a fallback, \
         \"Last token\" is the moment the raw transcript was shown.\n\n\
         | Sample | Run | Pipeline | Audio | Connections open | Last audio sent | Final transcript | First token | Last token | `lang_ok` | Note |\n\
         | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n",
    );
    for result in results {
        let head = format!(
            "| {} | {} | {} | {} |",
            result.sample,
            result.run,
            result.pipeline.label(),
            result.audio_ms
        );
        let _ = match &result.outcome {
            Err(error) => writeln!(
                md,
                "{head} - | - | - | - | - | - | FAILED: {} |",
                cell(error)
            ),
            Ok(m) => writeln!(
                md,
                "{head} {} | {} | {} | {} | {} | {} | {} |",
                m.open_ms,
                opt(m.last_audio_ms),
                m.stt_ms,
                opt(m.first_ms),
                m.total_ms,
                m.lang_ok.map_or("-", |ok| if ok { "yes" } else { "NO" }),
                cell(&note(m))
            ),
        };
    }

    md.push_str(
        "\n## Texts, and comparison with Wispr Flow\n\n\
         **The Wispr Flow comparison is not filled in.** The owner dictates the same samples \
         into Wispr Flow, pastes each result under \"Wispr Flow\" and marks which reads better.\n",
    );
    let mut samples: Vec<&str> = Vec::new();
    for result in first_runs(results) {
        if !samples.contains(&result.sample.as_str()) {
            samples.push(&result.sample);
        }
    }
    for sample in samples {
        let _ = write!(md, "\n### {sample}\n\n");
        for pipeline in [Pipeline::B, Pipeline::A] {
            let Some(result) =
                first_runs(results).find(|r| r.sample == sample && r.pipeline == pipeline)
            else {
                continue;
            };
            match &result.outcome {
                Err(error) => {
                    let _ = write!(md, "{}: failed: {}\n\n", pipeline.label(), cell(error));
                }
                Ok(m) => {
                    let label = pipeline.label();
                    let _ = write!(md, "{label}, raw:\n\n{}\n", fenced(&m.raw));
                    let _ = if m.fallback {
                        write!(
                            md,
                            "{label}, cleaned: none. The cleanup missed its deadline, so the raw transcript was used.\n\n"
                        )
                    } else {
                        write!(md, "{label}, cleaned:\n\n{}\n", fenced(&m.text))
                    };
                }
            }
        }
        let _ = write!(
            md,
            "Wispr Flow: {NOT_FILLED_IN}\n\nReads better: {NOT_FILLED_IN}\n"
        );
    }

    md.push_str(
        "\n## Method\n\n\
         - Pipeline A uploads the whole recording at key release. Pipeline B opens and warms \
         its connections first, streams the audio in 100 ms chunks in real time and sends an \
         explicit commit at key release.\n\
         - Both pipelines use the same warm HTTP client for the cleanup call.\n\
         - Median: the middle value, or the mean of the two middle values. 90th percentile: \
         nearest rank, so the 45th smallest of 50.\n\
         - A fallback is a sample whose cleanup missed its deadline. The raw transcript is \
         used, and `total_ms` is the moment it would have been shown. Fallbacks count in \
         `total_ms` and have no `first_ms`. The gate allows them on one sample in ten, as a \
         90th percentile does.\n\
         - A sample fails when the speech provider or the cleanup returns an error, or when \
         the transcript is empty. Failed samples are left out of the times and fail the gate.\n\
         - `lang_ok` is a word-count check that raw and cleaned text are both in the language \
         named by the sample file (`es-` or `en-`). It catches a wrong language or a \
         translation, not a single wrong word.\n",
    );
    md
}

fn note(m: &Measured) -> String {
    let mut parts = Vec::new();
    if let Some(fallback) = &m.fallback_note {
        parts.push(format!("FALLBACK: {fallback}"));
    }
    if !m.provider_languages.is_empty() {
        parts.push(format!(
            "provider detected {}",
            m.provider_languages.join(", ")
        ));
    }
    parts.join("; ")
}

/// Text for a table cell: one line, no column breaks.
fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

/// Text in a fenced block, so lists and line breaks from the cleanup show as they are.
fn fenced(text: &str) -> String {
    // A fence one backtick longer than any run inside the text cannot be closed by it.
    let longest = text
        .split(|c| c != '`')
        .map(str::len)
        .max()
        .unwrap_or_default();
    let fence = "`".repeat(longest.max(2) + 1);
    format!("{fence}text\n{text}\n{fence}\n")
}

/// Writes `content` to `<dir>/<stem>.md`, or `<stem>-2.md` and so on: an earlier results file
/// is never overwritten.
pub fn write_new(dir: &Path, stem: &str, content: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    for n in 1.. {
        let name = match n {
            1 => format!("{stem}.md"),
            _ => format!("{stem}-{n}.md"),
        };
        let path = dir.join(name);
        // `create_new` refuses an existing file, so nothing can be overwritten.
        match std::fs::File::create_new(&path) {
            Ok(mut file) => {
                file.write_all(content.as_bytes())
                    .with_context(|| format!("writing {}", path.display()))?;
                return Ok(path);
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err).with_context(|| format!("creating {}", path.display())),
        }
    }
    unreachable!("the loop only ends by returning")
}

/// Output of a system command, when it runs and prints something.
fn command(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (output.status.success() && !text.is_empty()).then_some(text)
}

/// Local date as `YYYY-MM-DD`, and local time with zone.
pub fn today() -> Result<(String, String)> {
    let date = command("date", &["+%F"]).context("could not read the date from `date`")?;
    let clock = command("date", &["+%H:%M %Z"]).unwrap_or_default();
    Ok((date, clock))
}

/// Model, chip, memory and system version. No host or user name.
pub fn machine() -> String {
    let memory = command("sysctl", &["-n", "hw.memsize"])
        .and_then(|bytes| bytes.parse::<u64>().ok())
        .map(|bytes| format!("{} GB", bytes >> 30));
    let parts: Vec<String> = [
        command("sysctl", &["-n", "hw.model"]),
        command("sysctl", &["-n", "machdep.cpu.brand_string"]),
        memory,
        command("sw_vers", &["-productVersion"]).map(|version| format!("macOS {version}")),
    ]
    .into_iter()
    .flatten()
    .collect();
    if parts.is_empty() {
        format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
    } else {
        parts.join(", ")
    }
}

/// The interface that carries the default route, such as `Wi-Fi (en0)`.
pub fn network_interface() -> String {
    let Some(device) = command("route", &["-n", "get", "default"]).and_then(|out| {
        out.lines()
            .find_map(|line| line.trim().strip_prefix("interface: ").map(str::to_owned))
    }) else {
        return "interface unknown".to_owned();
    };
    let ports = command("networksetup", &["-listallhardwareports"]).unwrap_or_default();
    match hardware_port(&ports, &device) {
        Some(port) => format!("{port} ({device})"),
        None => format!("{device} (no hardware port: a VPN or tunnel)"),
    }
}

/// Finds the port name for a device in the output of `networksetup -listallhardwareports`.
fn hardware_port(ports: &str, device: &str) -> Option<String> {
    let mut port = None;
    for line in ports.lines() {
        if let Some(name) = line.strip_prefix("Hardware Port: ") {
            port = Some(name.to_owned());
        } else if line.strip_prefix("Device: ") == Some(device) {
            return port;
        }
    }
    None
}

/// Short commit of the tool, marked when the working tree has changes.
pub fn commit() -> String {
    let Some(hash) = command("git", &["rev-parse", "--short", "HEAD"]) else {
        return "unknown".to_owned();
    };
    match command("git", &["status", "--porcelain", "--", "."]) {
        Some(_) => format!("{hash}, with uncommitted changes"),
        None => hash,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::aggregate;
    use crate::stats::tests::{done, failed};

    fn info(fake: bool) -> RunInfo {
        RunInfo {
            date: "2026-11-25".to_owned(),
            clock: "10:30 CET".to_owned(),
            fake,
            machine: "Mac15,6, Apple M3 Pro, 36 GB, macOS 26.1".to_owned(),
            network: "Wi-Fi (en0); cold request 210 ms, warm round trip 45 ms".to_owned(),
            commit: "abc1234".to_owned(),
            settings: vec![("Cleanup model", "some-model".to_owned())],
        }
    }

    fn results() -> Vec<SampleResult> {
        let mut list = done("es-01", Pipeline::B, 210, Some(430), 520);
        list.outcome.as_mut().unwrap().text = "Tres cosas:\n- uno\n- dos | tres".to_owned();
        vec![
            done("es-01", Pipeline::A, 640, Some(880), 990),
            list,
            done("en-01", Pipeline::A, 600, Some(850), 940),
            done("en-01", Pipeline::B, 190, None, 1_690),
            failed("en-02", Pipeline::A),
            done("en-02", Pipeline::B, 230, Some(450), 560),
        ]
    }

    #[test]
    fn markdown_has_setup_table_gate_stages_and_texts() {
        let results = results();
        let md = markdown(&info(false), &aggregate(&results), &results);

        assert!(md.starts_with("# Speed test, 2026-11-25\n"));
        assert!(!md.contains("Fake providers"));
        assert!(md.contains("| Machine | Mac15,6, Apple M3 Pro, 36 GB, macOS 26.1 |"));
        assert!(
            md.contains("| Network | Wi-Fi (en0); cold request 210 ms, warm round trip 45 ms |")
        );
        assert!(md.contains("| Cleanup model | some-model |"));

        // Results: one row per pipeline and language, median / 90th percentile.
        assert!(
            md.contains("| A upload | es | 1 | 640 / 640 | 880 / 880 | 990 / 990 | 1/1 | 0 | 0 |")
        );
        assert!(
            md.contains("| A upload | all | 3 | 620 / 640 | 865 / 880 | 965 / 990 | 2/2 | 0 | 1 |")
        );
        assert!(
            md.contains(
                "| B stream | all | 3 | 210 / 230 | 440 / 450 | 560 / 1690 | 3/3 | 1 | 0 |"
            )
        );

        // Gate: said plainly, per pipeline.
        assert!(md.contains("- A upload: FAIL: median total_ms 965 is over 800; 1 of 3 failed"));
        assert!(md.contains("- B stream: FAIL: 90th percentile total_ms 1690 is over 1200"));
        assert!(md.contains("The speed and language gate did NOT pass on any pipeline."));
        assert!(md.contains("1 Spanish and 2 English samples, not the planned 25 and 25"));

        // Stage log: every stage of every measurement.
        assert!(
            md.contains("| es-01 | 1 | B stream | 8000 | -8100 | 0 | 210 | 430 | 520 | yes |  |")
        );
        assert!(md.contains("| en-01 | 1 | B stream | 8000 | -8100 | 0 | 190 | - | 1690 | yes | FALLBACK: cleanup not complete |"));
        assert!(md.contains("| en-02 | 1 | A upload | 8000 | - | - | - | - | - | - | FAILED: HTTP 500: upstream error |"));

        // Texts: raw and cleaned, line breaks kept, Wispr Flow left for the owner.
        assert!(md.contains("B stream, raw:\n\n```text\nraw text of es-01\n```\n"));
        assert!(
            md.contains("B stream, cleaned:\n\n```text\nTres cosas:\n- uno\n- dos | tres\n```\n")
        );
        assert!(md.contains(
            "B stream, cleaned: none. The cleanup missed its deadline, so the raw transcript was used."
        ));
        assert!(md.contains(
            "- B stream: FAIL: 90th percentile total_ms 1690 is over 1200; raw transcript used on 1 of 3, more than one in ten"
        ));
        assert!(md.contains("A upload: failed: HTTP 500: upstream error"));
        assert!(md.contains("**The Wispr Flow comparison is not filled in.**"));
        assert_eq!(md.matches("Wispr Flow: _not filled in_").count(), 3);
        assert_eq!(md.matches("Reads better: _not filled in_").count(), 3);
    }

    #[test]
    fn a_fake_run_says_so_at_the_top() {
        let results = results();
        let md = markdown(&info(true), &aggregate(&results), &results);
        assert!(md.contains("> **Fake providers.**"));
        assert!(md.contains("not a measurement"));
    }

    #[test]
    fn a_passing_gate_is_said_plainly() {
        let mut results = Vec::new();
        for i in 1..=25 {
            for lang in ["es", "en"] {
                let name = format!("{lang}-{i:02}");
                results.push(done(&name, Pipeline::A, 600, Some(900), 1_000));
                results.push(done(&name, Pipeline::B, 200, Some(400), 500 + i));
            }
        }
        let lines = gate_lines(&aggregate(&results), &results);
        assert!(lines.contains(&"- A upload: FAIL: median total_ms 1000 is over 800".to_owned()));
        assert!(lines.contains(&"- B stream: PASS (median 513, 90th percentile 523)".to_owned()));
        assert!(lines.contains(&"The speed and language gate PASSED on: B stream.".to_owned()));
        assert!(
            !lines
                .iter()
                .any(|line| line.contains("not the full gate run"))
        );
    }

    #[test]
    fn console_table_and_stage_line_show_the_numbers() {
        let results = results();
        let table = console_table(&aggregate(&results));
        assert!(table.lines().next().unwrap().contains("total p50/p90"));
        assert!(table.contains("B stream  all    3      210 / 230      440 / 450     560 / 1690      3/3          1       0"));
        assert_eq!(
            stage_line(&results[1]),
            "es-01    B stream  run 1  audio   8.0 s | open  -8100  last audio    0  stt   210  first   430  total   520 | lang ok"
        );
        assert!(stage_line(&results[4]).ends_with("| FAILED: HTTP 500: upstream error"));
        assert!(console_texts(&results).contains("  cleaned: Tres cosas:\n           - uno"));
    }

    #[test]
    fn fences_grow_past_backticks_in_the_text() {
        assert_eq!(fenced("plain"), "```text\nplain\n```\n");
        assert_eq!(fenced("a ```code``` b"), "````text\na ```code``` b\n````\n");
    }

    #[test]
    fn results_files_are_never_overwritten() {
        let dir = std::env::temp_dir().join(format!("flowbench-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let first = write_new(&dir, "speed-test-2026-11-25", "one").unwrap();
        let second = write_new(&dir, "speed-test-2026-11-25", "two").unwrap();
        assert_eq!(first.file_name().unwrap(), "speed-test-2026-11-25.md");
        assert_eq!(second.file_name().unwrap(), "speed-test-2026-11-25-2.md");
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "one");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn hardware_port_is_found_by_device() {
        let ports = "\nHardware Port: Ethernet Adapter (en4)\nDevice: en4\nEthernet Address: 00:00\n\nHardware Port: Wi-Fi\nDevice: en0\nEthernet Address: 00:00\n";
        assert_eq!(hardware_port(ports, "en0").as_deref(), Some("Wi-Fi"));
        assert_eq!(hardware_port(ports, "utun4"), None);
    }
}
