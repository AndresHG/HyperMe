//! The timing log on disk (R-30, AC-20): one tab-separated line per pasted dictation, written
//! only while the timing setting is on. Numbers, the language code, flags and the provider and
//! model names: never text. The columns from key release match flowbench's (`stt_ms`,
//! `first_ms`, `total_ms`), so live numbers compare with Phase 0. [`summarise`] gives the
//! median and 90th percentile for AC-10, with flowbench's definitions.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use hyperme_core::traits::{TimingLine, TimingLog};

/// The header line, written when the file is new.
pub const HEADER: &str = "unix_ms\tfirst_audio_ms\trelease_ms\tstt_ms\tfirst_ms\ttotal_ms\tpaste_ms\tlanguage\twords\tfallback\tspeech_provider\tspeech_model\tcleanup_provider\tcleanup_model";

/// The providers and models a dictation used, from the config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Names {
    pub speech_provider: String,
    pub speech_model: String,
    pub cleanup_provider: String,
    pub cleanup_model: String,
}

/// Appends timing lines to a file.
#[derive(Debug, Clone)]
pub struct FileTimingLog {
    path: PathBuf,
    names: Names,
    now: fn() -> u64,
}

fn system_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

impl FileTimingLog {
    pub fn new(path: PathBuf, names: Names) -> Self {
        FileTimingLog {
            path,
            names,
            now: system_now_ms,
        }
    }

    /// As [`new`](Self::new), with a fixed clock, for tests.
    pub fn with_clock(path: PathBuf, names: Names, now: fn() -> u64) -> Self {
        FileTimingLog { path, names, now }
    }

    fn line(&self, t: &TimingLine) -> String {
        let opt = |v: Option<u64>| v.map_or_else(|| "-".to_owned(), |v| v.to_string());
        // Names come from the config; tabs or line breaks in them would break the columns.
        let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
        [
            (self.now)().to_string(),
            opt(t.first_audio_ms),
            t.release_ms.to_string(),
            t.stt_ms.to_string(),
            opt(t.first_ms),
            t.total_ms.to_string(),
            t.paste_ms.to_string(),
            t.language.as_deref().map_or_else(|| "-".to_owned(), clean),
            t.words.to_string(),
            u8::from(t.fallback).to_string(),
            clean(&self.names.speech_provider),
            clean(&self.names.speech_model),
            // specs/96, R-27: the fallback that answered, when one did.
            clean(
                t.cleanup_via
                    .as_ref()
                    .map_or(&self.names.cleanup_provider, |v| &v.0),
            ),
            clean(
                t.cleanup_via
                    .as_ref()
                    .map_or(&self.names.cleanup_model, |v| &v.1),
            ),
        ]
        .join("\t")
    }
}

impl TimingLog for FileTimingLog {
    fn record(&mut self, line: &TimingLine) {
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        else {
            return;
        };
        let new = file.metadata().map(|m| m.len() == 0).unwrap_or(false);
        let mut text = String::new();
        if new {
            text.push_str(HEADER);
            text.push('\n');
        }
        text.push_str(&self.line(line));
        text.push('\n');
        // A timing log that cannot be written never stops a dictation.
        let _ = file.write_all(text.as_bytes());
    }
}

/// The median and 90th percentile of a column, over the lines read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quantiles {
    pub median: u64,
    pub p90: u64,
}

/// AC-10's numbers from a timing log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub dictations: usize,
    pub fallbacks: usize,
    /// Key release to text ready (flowbench's `total_ms`).
    pub total: Option<Quantiles>,
    /// Key release to paste sent: AC-10's "key release to text pasted".
    pub paste: Option<Quantiles>,
    /// Key release to final transcript.
    pub stt: Option<Quantiles>,
}

/// The middle value; with an even count, the mean of the two middle ones rounded up (as
/// flowbench).
pub fn median(values: &[u64]) -> Option<u64> {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    match n {
        0 => None,
        _ if n % 2 == 1 => Some(sorted[n / 2]),
        _ => Some((sorted[n / 2 - 1] + sorted[n / 2]).div_ceil(2)),
    }
}

/// Nearest-rank percentile (as flowbench): for 50 values the 90th percentile is the 45th
/// smallest.
pub fn percentile(values: &[u64], percent: usize) -> Option<u64> {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let rank = (sorted.len() * percent).div_ceil(100).max(1);
    sorted.get(rank - 1).copied()
}

fn quantiles(values: &[u64]) -> Option<Quantiles> {
    Some(Quantiles {
        median: median(values)?,
        p90: percentile(values, 90)?,
    })
}

/// Reads a timing log and summarises it. Lines that do not parse are skipped.
pub fn summarise(text: &str) -> Summary {
    let index = |name: &str| HEADER.split('\t').position(|c| c == name);
    let (stt, total, paste, fallback) = (
        index("stt_ms"),
        index("total_ms"),
        index("paste_ms"),
        index("fallback"),
    );
    let (mut stts, mut totals, mut pastes, mut fallbacks) = (Vec::new(), Vec::new(), Vec::new(), 0);
    for line in text
        .lines()
        .filter(|l| !l.starts_with("unix_ms") && !l.trim().is_empty())
    {
        let fields: Vec<&str> = line.split('\t').collect();
        let number = |i: Option<usize>| {
            i.and_then(|i| fields.get(i))
                .and_then(|v| v.parse::<u64>().ok())
        };
        let (Some(s), Some(t), Some(p)) = (number(stt), number(total), number(paste)) else {
            continue;
        };
        stts.push(s);
        totals.push(t);
        pastes.push(p);
        if number(fallback) == Some(1) {
            fallbacks += 1;
        }
    }
    Summary {
        dictations: totals.len(),
        fallbacks,
        total: quantiles(&totals),
        paste: quantiles(&pastes),
        stt: quantiles(&stts),
    }
}

/// The user's median from key release to text (`total_ms`) over their last `last` dictations
/// whose `column` (`speech_provider` or `cleanup_provider`) is `provider`, and how many that
/// is (specs/96-provider-settings, R-20). `None` when there are none.
pub fn own_median(text: &str, column: &str, provider: &str, last: usize) -> Option<(u64, usize)> {
    let index = |name: &str| HEADER.split('\t').position(|c| c == name);
    let (column, total) = (index(column)?, index("total_ms")?);
    let mut totals: Vec<u64> = text
        .lines()
        .filter(|l| !l.starts_with("unix_ms"))
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            (fields.get(column) == Some(&provider))
                .then(|| fields.get(total)?.parse::<u64>().ok())
                .flatten()
        })
        .collect();
    let keep = totals.len().saturating_sub(last);
    totals.drain(..keep);
    Some((median(&totals)?, totals.len()))
}
