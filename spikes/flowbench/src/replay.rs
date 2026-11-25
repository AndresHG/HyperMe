//! Replay: the saved raw transcripts through a cleanup setup, with no speaking and no speech
//! provider. For comparing prompts and cleanup models on time and quality.
//!
//! Every real run saves its raw transcripts to `samples/transcripts.json` (git-ignored, as the
//! texts contain work content). A results file that still has its "Texts" section can be read
//! too. The owner's corrected texts, if any, are read from `samples/gold.json`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};
use tokio::time::Instant;

use crate::cleanup::{CleanupProvider, clean_with_deadline};
use crate::lang::{Lang, detect};
use crate::pipeline::{Pipeline, SampleResult};
use crate::quality;
use crate::stats::{median, percentile};

/// One saved transcript.
#[derive(Clone, Debug, PartialEq)]
pub struct Saved {
    pub sample: String,
    pub raw: String,
}

/// Reads transcripts from a JSON object (`{"es-01": "raw text", ...}`) or from a results file's
/// "Texts" section, preferring the streaming transcript when both pipelines are there.
pub fn load_transcripts(path: &Path) -> Result<Vec<Saved>> {
    let text = std::fs::read_to_string(path).with_context(|| {
        format!(
            "reading {}. A run with real providers writes it; or pass --transcripts with a \
             results file that still has its Texts section",
            path.display()
        )
    })?;
    let saved = if path.extension().is_some_and(|ext| ext == "md") {
        from_results_markdown(&text)
    } else {
        from_json(&text).with_context(|| format!("{}", path.display()))?
    };
    if saved.is_empty() {
        bail!("no transcripts in {}", path.display());
    }
    Ok(saved)
}

fn from_json(text: &str) -> Result<Vec<Saved>> {
    let map: Map<String, Value> =
        serde_json::from_str(text).context("expected a JSON object of sample name to text")?;
    Ok(map
        .into_iter()
        .filter_map(|(sample, raw)| {
            raw.as_str().map(|raw| Saved {
                sample,
                raw: raw.to_owned(),
            })
        })
        .collect())
}

/// Reads `### es-01` headings and the fenced block after `B stream, raw:` (or `A upload, raw:`).
fn from_results_markdown(text: &str) -> Vec<Saved> {
    let mut found: BTreeMap<String, (bool, String)> = BTreeMap::new();
    let mut sample: Option<String> = None;
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if let Some(name) = line.strip_prefix("### ") {
            sample = Some(name.trim().to_owned());
            continue;
        }
        let streaming = match line.trim() {
            "B stream, raw:" => true,
            "A upload, raw:" => false,
            _ => continue,
        };
        let Some(name) = &sample else { continue };
        // Skip to the opening fence, then read until the same fence closes it.
        let Some(open) = lines.by_ref().find(|l| l.starts_with("```")) else {
            break;
        };
        let fence: String = open.chars().take_while(|&c| c == '`').collect();
        let body: Vec<&str> = lines.by_ref().take_while(|l| l.trim() != fence).collect();
        let raw = body.join("\n");
        let better = found.get(name).is_none_or(|(had_b, _)| streaming && !had_b);
        if better {
            found.insert(name.clone(), (streaming, raw));
        }
    }
    found
        .into_iter()
        .map(|(sample, (_, raw))| Saved { sample, raw })
        .collect()
}

/// Adds the raw transcripts of a run to the JSON file at `path`, replacing older ones for the
/// same samples. Uses pipeline B's transcript from the first run, else pipeline A's.
pub fn save_transcripts(path: &Path, results: &[SampleResult]) -> Result<usize> {
    let mut map: Map<String, Value> = match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .with_context(|| format!("{} is not a JSON object", path.display()))?,
        Err(_) => Map::new(),
    };
    // Pipeline B last, so its transcript wins when both are there.
    let mut samples = BTreeSet::new();
    for pipeline in [Pipeline::A, Pipeline::B] {
        for result in results
            .iter()
            .filter(|r| r.run == 1 && r.pipeline == pipeline)
        {
            if let Ok(m) = &result.outcome {
                map.insert(result.sample.clone(), Value::String(m.raw.clone()));
                samples.insert(result.sample.as_str());
            }
        }
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(&map)? + "\n";
    std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))?;
    Ok(samples.len())
}

/// The owner's corrected texts, by sample. Empty when there is no gold file.
pub fn load_gold(path: &Path) -> Result<BTreeMap<String, String>> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(BTreeMap::new());
    };
    Ok(from_json(&text)
        .with_context(|| format!("{}", path.display()))?
        .into_iter()
        .map(|s| (s.sample, s.raw))
        .collect())
}

/// One transcript through the cleanup, timed from the moment the request is sent.
pub struct Replayed {
    pub sample: String,
    pub lang: Option<Lang>,
    pub run: u32,
    pub raw: String,
    pub outcome: Result<Timed, String>,
}

pub struct Timed {
    pub first_any_ms: Option<u64>,
    pub first_ms: Option<u64>,
    pub total_ms: u64,
    pub thinking_chars: usize,
    pub reasoning_tokens: Option<u64>,
    pub fallback: bool,
    pub text: String,
}

/// Runs every transcript `runs` times. The connection is warmed before each, as it is on
/// key-down in the real pipeline.
pub async fn run_all<C: CleanupProvider>(
    llm: &C,
    saved: &[Saved],
    runs: u32,
    deadline: Duration,
) -> Vec<Replayed> {
    let mut out = Vec::new();
    for run in 1..=runs {
        for entry in saved {
            let outcome = async {
                llm.warm().await?;
                let start = Instant::now();
                let cleaned = clean_with_deadline(llm, entry.raw.trim(), deadline).await?;
                let since = |at: Instant| at.duration_since(start).as_millis() as u64;
                anyhow::Ok(Timed {
                    first_any_ms: cleaned.first_any.map(since),
                    first_ms: cleaned.first.map(since),
                    total_ms: since(cleaned.done),
                    thinking_chars: cleaned.thinking_chars,
                    reasoning_tokens: cleaned.reasoning_tokens,
                    fallback: cleaned.fallback,
                    text: cleaned.text,
                })
            }
            .await;
            let replayed = Replayed {
                sample: entry.sample.clone(),
                lang: crate::audio::lang_of(&entry.sample),
                run,
                raw: entry.raw.clone(),
                outcome: outcome.map_err(|err| format!("{err:#}")),
            };
            println!("{}", line(&replayed));
            out.push(replayed);
        }
    }
    out
}

fn line(r: &Replayed) -> String {
    let head = format!("{:<8} run {}", r.sample, r.run);
    match &r.outcome {
        Err(error) => format!("{head} | FAILED: {error}"),
        Ok(t) => {
            let mut line = format!(
                "{head} | first any {:>5}  first {:>5}  total {:>5}",
                opt(t.first_any_ms),
                opt(t.first_ms),
                t.total_ms
            );
            if t.thinking_chars > 0 || t.reasoning_tokens.is_some_and(|n| n > 0) {
                let _ = write!(
                    line,
                    " | reasoning {} chars, {} tokens",
                    t.thinking_chars,
                    opt(t.reasoning_tokens)
                );
            }
            if t.fallback {
                line.push_str(" | FALLBACK");
            }
            line
        }
    }
}

fn opt<T: ToString>(value: Option<T>) -> String {
    value.map_or("-".to_owned(), |v| v.to_string())
}

/// One line of the replay table.
pub struct Summary {
    pub label: &'static str,
    pub n: usize,
    pub failed: usize,
    pub fallbacks: usize,
    pub lang_wrong: usize,
    pub first_any: (Option<u64>, Option<u64>),
    pub first: (Option<u64>, Option<u64>),
    pub total: (Option<u64>, Option<u64>),
    pub reasoning_tokens: Option<u64>,
    pub unchanged: usize,
    pub fillers_raw: usize,
    pub fillers_left: usize,
    /// Outputs compared with a gold text, zero-edit among them, mean edits per 100 characters.
    pub gold: Option<(usize, usize, f64)>,
}

pub fn summarise(results: &[Replayed], gold: &BTreeMap<String, String>) -> Vec<Summary> {
    let groups: [(&'static str, Option<Lang>); 3] = [
        ("es", Some(Lang::Es)),
        ("en", Some(Lang::En)),
        ("all", None),
    ];
    groups
        .into_iter()
        .filter_map(|(label, lang)| {
            let group: Vec<&Replayed> = results
                .iter()
                .filter(|r| lang.is_none() || r.lang == lang)
                .collect();
            (!group.is_empty()).then(|| summary(label, &group, gold))
        })
        .collect()
}

fn summary(label: &'static str, group: &[&Replayed], gold: &BTreeMap<String, String>) -> Summary {
    let ok: Vec<(&Replayed, &Timed)> = group
        .iter()
        .filter_map(|r| r.outcome.as_ref().ok().map(|t| (*r, t)))
        .collect();
    let quantiles = |values: Vec<u64>| (median(&values), percentile(&values, 90));
    let cleaned: Vec<&(&Replayed, &Timed)> = ok.iter().filter(|(_, t)| !t.fallback).collect();
    let tokens: Vec<u64> = ok.iter().filter_map(|(_, t)| t.reasoning_tokens).collect();
    let compared: Vec<quality::AgainstGold> = ok
        .iter()
        .filter_map(|(r, t)| {
            gold.get(&r.sample)
                .map(|g| quality::against_gold(&t.text, g))
        })
        .collect();
    Summary {
        label,
        n: group.len(),
        failed: group.len() - ok.len(),
        fallbacks: ok.len() - cleaned.len(),
        lang_wrong: ok
            .iter()
            .filter(|(r, t)| r.lang.is_some() && detect(&t.text) != r.lang)
            .count(),
        first_any: quantiles(ok.iter().filter_map(|(_, t)| t.first_any_ms).collect()),
        first: quantiles(ok.iter().filter_map(|(_, t)| t.first_ms).collect()),
        total: quantiles(ok.iter().map(|(_, t)| t.total_ms).collect()),
        reasoning_tokens: median(&tokens),
        unchanged: cleaned
            .iter()
            .filter(|(r, t)| quality::unchanged(&r.raw, &t.text))
            .count(),
        fillers_raw: ok.iter().map(|(r, _)| quality::fillers(&r.raw)).sum(),
        fillers_left: ok.iter().map(|(_, t)| quality::fillers(&t.text)).sum(),
        gold: (!compared.is_empty()).then(|| {
            let zero = compared.iter().filter(|c| c.zero_edit).count();
            let mean =
                compared.iter().map(|c| c.edits_per_100).sum::<f64>() / compared.len() as f64;
            (compared.len(), zero, mean)
        }),
    }
}

fn pair((p50, p90): (Option<u64>, Option<u64>)) -> String {
    format!("{} / {}", opt(p50), opt(p90))
}

fn gold_cell(gold: Option<(usize, usize, f64)>) -> (String, String) {
    match gold {
        Some((n, zero, mean)) => (format!("{zero}/{n}"), format!("{mean:.1}")),
        None => ("-".to_owned(), "-".to_owned()),
    }
}

const COLUMNS: &str = "| Language | Runs | First token, any kind | First text token | Full text | \
Reasoning tokens (median) | Unchanged | Fillers left (in raw) | Zero-edit | Edits per 100 chars | \
Fallbacks | Wrong language | Failed |\n\
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n";

/// The table, for the terminal and the results file alike.
pub fn table(rows: &[Summary]) -> String {
    let mut out = String::from(COLUMNS);
    for s in rows {
        let (zero, edits) = gold_cell(s.gold);
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} ({}) | {} | {} | {} | {} | {} |",
            s.label,
            s.n,
            pair(s.first_any),
            pair(s.first),
            pair(s.total),
            opt(s.reasoning_tokens),
            s.unchanged,
            s.fillers_left,
            s.fillers_raw,
            zero,
            edits,
            s.fallbacks,
            s.lang_wrong,
            s.failed
        );
    }
    out
}

/// The numbers-only results file, safe for the repository: no transcript or cleaned text.
pub fn markdown(
    date_clock: (&str, &str),
    settings: &[(&str, String)],
    rows: &[Summary],
    has_gold: bool,
) -> String {
    let (date, clock) = date_clock;
    let mut md = format!(
        "# Cleanup replay, {date}\n\n\
         Saved raw transcripts sent to one cleanup setup; no speech provider involved. Numbers \
         only: the texts stay in `spikes/flowbench/samples/`, which is not in the repository.\n\n\
         ## Setup\n\n| Item | Value |\n| --- | --- |\n| Date | {date} {clock} |\n"
    );
    for (label, value) in settings {
        let _ = writeln!(md, "| {label} | {} |", value.replace('|', "\\|"));
    }
    let _ = write!(
        md,
        "\n## Results\n\n\
         Milliseconds from the moment the cleanup request is sent, median / 90th percentile. \
         Add the speech provider's `stt_ms` for the time from key release.\n\n{}",
        table(rows)
    );
    md.push_str(
        "\n## Columns\n\n\
         - **First token, any kind:** reasoning included, when the host streams it. A gap to \
         the first text token is time the model spends thinking.\n\
         - **Unchanged:** cleaned text identical to the raw transcript apart from spacing.\n\
         - **Fillers left:** um, uh, eh, em, hmm, o sea, you know and similar, counted in the \
         cleaned text, with the count in the raw transcripts in brackets. Words that are only \
         sometimes fillers are not counted.\n",
    );
    if has_gold {
        md.push_str(
            "- **Zero-edit:** outputs equal to the owner's corrected text in \
             `samples/gold.json`, apart from spacing. **Edits per 100 chars:** character edits \
             needed to reach it.\n",
        );
    } else {
        md.push_str(
            "- **Zero-edit** and **edits per 100 chars** need `samples/gold.json`, the owner's \
             corrected texts. This run wrote `samples/gold-draft.json` to start from.\n",
        );
    }
    md
}

/// The texts, for the owner only: raw, cleaned and gold for each sample's first run.
pub fn texts(results: &[Replayed], gold: &BTreeMap<String, String>, title: &str) -> String {
    let mut md = format!("# {title}\n\nPrivate: keep out of the repository.\n");
    for r in results.iter().filter(|r| r.run == 1) {
        let _ = write!(md, "\n## {}\n\nRaw:\n\n{}\n", r.sample, fence(&r.raw));
        match &r.outcome {
            Ok(t) if t.fallback => md.push_str("Cleaned: none, the deadline passed.\n"),
            Ok(t) => {
                let _ = write!(md, "Cleaned:\n\n{}\n", fence(&t.text));
            }
            Err(error) => {
                let _ = writeln!(md, "Failed: {error}");
            }
        }
        if let Some(g) = gold.get(&r.sample) {
            let _ = write!(md, "Gold:\n\n{}\n", fence(g));
        }
    }
    md
}

/// A JSON object of each sample's first cleaned text, for the owner to correct into gold.
pub fn gold_draft(results: &[Replayed]) -> String {
    let map: Map<String, Value> = results
        .iter()
        .filter(|r| r.run == 1)
        .map(|r| {
            let text = match &r.outcome {
                Ok(t) => t.text.clone(),
                Err(_) => r.raw.clone(),
            };
            (r.sample.clone(), Value::String(text))
        })
        .collect();
    serde_json::to_string_pretty(&map).unwrap_or_default() + "\n"
}

fn fence(text: &str) -> String {
    let longest = text
        .split(|c| c != '`')
        .map(str::len)
        .max()
        .unwrap_or_default();
    let fence = "`".repeat(longest.max(2) + 1);
    format!("{fence}text\n{text}\n{fence}\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeCleanup;

    #[test]
    fn transcripts_come_from_json() {
        let saved = from_json(r#"{"es-01": "eh hola", "en-01": "um hi", "bad": 3}"#).unwrap();
        assert_eq!(
            saved,
            [
                Saved {
                    sample: "en-01".to_owned(),
                    raw: "um hi".to_owned()
                },
                Saved {
                    sample: "es-01".to_owned(),
                    raw: "eh hola".to_owned()
                },
            ]
        );
    }

    #[test]
    fn transcripts_come_from_a_results_file_preferring_streaming() {
        let md = "## Texts\n\n### es-01\n\nB stream, raw:\n\n```text\neh hola\nequipo\n```\n\
                  B stream, cleaned:\n\n```text\nHola equipo.\n```\n\
                  A upload, raw:\n\n```text\nupload text\n```\n\
                  ### en-01\n\nB stream: failed: boom\n\n\
                  A upload, raw:\n\n````text\nuses ``` inside\n````\n";
        assert_eq!(
            from_results_markdown(md),
            [
                Saved {
                    sample: "en-01".to_owned(),
                    raw: "uses ``` inside".to_owned()
                },
                Saved {
                    sample: "es-01".to_owned(),
                    raw: "eh hola\nequipo".to_owned()
                },
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_replay_is_timed_and_summarised_with_quality_signals() {
        let saved = vec![
            Saved {
                sample: "es-01".to_owned(),
                raw: "eh hola equipo".to_owned(),
            },
            Saved {
                sample: "en-01".to_owned(),
                raw: "um hello team".to_owned(),
            },
        ];
        let results = run_all(&FakeCleanup, &saved, 1, Duration::from_millis(1500)).await;
        let timed = results[0].outcome.as_ref().unwrap();
        assert_eq!(timed.text, "Hola equipo.");
        assert_eq!(
            timed.first_ms,
            Some(FakeCleanup::first_token_delay("eh hola equipo").as_millis() as u64)
        );
        assert_eq!(timed.first_any_ms, timed.first_ms);

        let gold = BTreeMap::from([("es-01".to_owned(), "Hola, equipo.".to_owned())]);
        let rows = summarise(&results, &gold);
        let all = rows.iter().find(|r| r.label == "all").unwrap();
        assert_eq!(
            (all.n, all.failed, all.fallbacks, all.lang_wrong),
            (2, 0, 0, 0)
        );
        assert_eq!(
            (all.fillers_raw, all.fillers_left, all.unchanged),
            (2, 0, 0)
        );
        let (compared, zero, edits) = all.gold.unwrap();
        assert_eq!((compared, zero), (1, 0));
        assert!(edits > 0.0);

        let md = markdown(
            ("2026-11-25", "14:00 CET"),
            &[("Cleanup", "x".to_owned())],
            &rows,
            true,
        );
        assert!(md.contains("| all | 2 |"));
        assert!(
            !md.contains("Hola equipo"),
            "the results file must hold no text"
        );
        let private = texts(&results, &gold, "Replay");
        assert!(private.contains("Hola equipo.") && private.contains("Hola, equipo."));
        let draft: Map<String, Value> = serde_json::from_str(&gold_draft(&results)).unwrap();
        assert_eq!(draft["es-01"], "Hola equipo.");
    }

    #[test]
    fn saved_transcripts_merge_into_the_file() {
        use crate::pipeline::Measured;
        let dir = std::env::temp_dir().join(format!("flowbench-replay-{}", std::process::id()));
        let path = dir.join("transcripts.json");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, r#"{"en-09": "old", "es-01": "older"}"#).unwrap();
        let result = |pipeline, raw: &str| SampleResult {
            sample: "es-01".to_owned(),
            lang: Some(Lang::Es),
            pipeline,
            run: 1,
            audio_ms: 1,
            outcome: Ok(Measured {
                open_ms: 0,
                last_audio_ms: None,
                stt_ms: 1,
                first_ms: None,
                first_any_ms: None,
                thinking_chars: 0,
                reasoning_tokens: None,
                total_ms: 1,
                raw: raw.to_owned(),
                text: raw.to_owned(),
                fallback: false,
                fallback_note: None,
                provider_languages: vec![],
                lang_ok: None,
            }),
        };
        let results = [result(Pipeline::A, "upload"), result(Pipeline::B, "stream")];
        assert_eq!(save_transcripts(&path, &results).unwrap(), 1);
        let saved: Map<String, Value> =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved["es-01"], "stream");
        assert_eq!(saved["en-09"], "old");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
