//! Slice 12: the timing log file and its summary (R-30, AC-20, AC-10's numbers).

use std::path::{Path, PathBuf};

use hyperme_core::traits::{TimingLine, TimingLog};
use hyperme_platform::timing::{FileTimingLog, HEADER, Names, median, percentile, summarise};

fn temp_file(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("timing");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let _ = std::fs::remove_file(&path);
    path
}

fn names() -> Names {
    Names {
        speech_provider: "speechmatics".into(),
        speech_model: "melia-1".into(),
        cleanup_provider: "groq".into(),
        cleanup_model: "openai/gpt-oss-20b".into(),
    }
}

fn line(total: u64, fallback: bool) -> TimingLine {
    TimingLine {
        first_audio_ms: Some(31),
        release_ms: 4200,
        stt_ms: total / 2,
        first_ms: (!fallback).then_some(total - 50),
        total_ms: total,
        paste_ms: total + 2,
        language: Some("es".into()),
        words: 12,
        fallback,
        speech_via: None,
        cleanup_via: None,
    }
}

/// AC-20: one line per dictation after a header, numbers and names only, in flowbench's columns.
#[test]
fn writes_a_header_then_one_line_per_dictation() {
    let path = temp_file("lines.tsv");
    let mut log = FileTimingLog::with_clock(path.clone(), names(), || 1_700_000_000_000);
    log.record(&line(430, false));
    log.record(&line(1500, true));
    let text = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], HEADER);
    assert!(HEADER.contains("\tstt_ms\tfirst_ms\ttotal_ms\t"));
    assert_eq!(
        lines[1],
        "1700000000000\t31\t4200\t215\t380\t430\t432\tes\t12\t0\tspeechmatics\tmelia-1\tgroq\topenai/gpt-oss-20b"
    );
    assert_eq!(
        lines[2].split('\t').nth(4),
        Some("-"),
        "no first token on a fallback"
    );
    assert_eq!(lines[2].split('\t').nth(9), Some("1"));

    // A second log on the same file appends without a second header.
    FileTimingLog::new(path.clone(), names()).record(&line(500, false));
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.matches("unix_ms").count(), 1);
    assert_eq!(text.lines().count(), 4);
}

/// AC-10: median and 90th percentile from the log, with flowbench's definitions.
#[test]
fn summary_gives_median_and_p90() {
    let path = temp_file("summary.tsv");
    let mut log = FileTimingLog::new(path.clone(), names());
    for total in [400, 420, 440, 460, 480, 500, 520, 540, 560, 1500] {
        log.record(&line(total, total == 1500));
    }
    let summary = summarise(&std::fs::read_to_string(&path).unwrap());
    assert_eq!(summary.dictations, 10);
    assert_eq!(summary.fallbacks, 1);
    let total = summary.total.unwrap();
    assert_eq!((total.median, total.p90), (490, 560));
    let paste = summary.paste.unwrap();
    assert_eq!((paste.median, paste.p90), (492, 562));
    assert!(summarise("").total.is_none());
}

#[test]
fn quantiles_match_flowbench() {
    assert_eq!(median(&[3, 1, 2]), Some(2));
    assert_eq!(median(&[1, 2, 3, 4]), Some(3));
    let fifty: Vec<u64> = (1..=50).collect();
    assert_eq!(percentile(&fifty, 90), Some(45));
}

/// Provider settings R-20: the user's own median from key release to text over their last 50
/// dictations on one provider, from the timing log's provider columns.
#[test]
fn own_median_counts_the_last_fifty_on_one_provider() {
    use hyperme_platform::timing::own_median;
    let mut text = format!("{HEADER}\n");
    let row = |total: u64, cleanup: &str| {
        format!(
            "1\t10\t900\t300\t-\t{total}\t{total}\ten\t5\t0\tspeechmatics\tmelia-1\t{cleanup}\tm\n"
        )
    };
    // 10 old lines at 2000 ms, then 50 at 400 ms, on groq; other lines on openai.
    for _ in 0..10 {
        text.push_str(&row(2000, "groq"));
    }
    for _ in 0..50 {
        text.push_str(&row(400, "groq"));
        text.push_str(&row(900, "openai"));
    }
    assert_eq!(
        own_median(&text, "cleanup_provider", "groq", 50),
        Some((400, 50))
    );
    assert_eq!(
        own_median(&text, "cleanup_provider", "openai", 50),
        Some((900, 50))
    );
    // The last 50 on Speechmatics are 25 of each: the median is the mean of the middle two.
    assert_eq!(
        own_median(&text, "speech_provider", "speechmatics", 50),
        Some((650, 50))
    );
    assert_eq!(own_median(&text, "cleanup_provider", "anthropic", 50), None);
    assert_eq!(own_median("", "cleanup_provider", "groq", 50), None);
}

/// specs/96, AC-10: when a fallback cleaned the dictation, the line names it, not the main one.
#[test]
fn a_fallback_provider_is_named_on_its_line() {
    let path = temp_file("via.tsv");
    let mut log = FileTimingLog::with_clock(path.clone(), names(), || 1);
    let mut via = line(600, false);
    via.cleanup_via = Some(("openai".into(), "gpt-6-luna".into()));
    via.speech_via = Some(("openai-speech".into(), "gpt-live-transcribe".into()));
    log.record(&via);
    let text = std::fs::read_to_string(&path).unwrap();
    let cols: Vec<&str> = text.lines().nth(1).unwrap().split('\t').collect();
    assert_eq!(
        &cols[10..],
        [
            "openai-speech",
            "gpt-live-transcribe",
            "openai",
            "gpt-6-luna"
        ]
    );
}
