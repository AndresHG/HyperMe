//! The report (AC-6): a table per language for the terminal and for `docs/results/`, which
//! holds numbers only (AC-5), and the texts and gold draft, which go to the set folder only.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use hyperme_platform::timing::{median, percentile};
use serde_json::{Map, Value};

use crate::replay::{Pasted, Replayed};
use crate::score::{self, NameScore};
use crate::set::Lang;

/// One line of the table.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub label: &'static str,
    pub n: usize,
    pub failed: usize,
    pub fallbacks: usize,
    /// Cleaned outputs identical to the raw transcript apart from spacing.
    pub unchanged: usize,
    pub fillers_raw: usize,
    pub fillers_left: usize,
    /// Outputs compared with a gold text, zero-edit among them, mean edits per 100 characters.
    pub gold: Option<(usize, usize, f64)>,
    /// Over the outputs that have a gold text.
    pub names: NameScore,
    /// Median and 90th percentile, milliseconds from key release.
    pub stt: (Option<u64>, Option<u64>),
    pub total: (Option<u64>, Option<u64>),
}

/// Rows for Spanish, English (when there are any) and all samples.
pub fn summarise(
    results: &[Replayed],
    gold: &BTreeMap<String, String>,
    names: &[String],
) -> Vec<Row> {
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
            (!group.is_empty()).then(|| row(label, &group, gold, names))
        })
        .collect()
}

fn row(
    label: &'static str,
    group: &[&Replayed],
    gold: &BTreeMap<String, String>,
    names: &[String],
) -> Row {
    let ok: Vec<(&Replayed, &Pasted)> = group
        .iter()
        .filter_map(|r| r.outcome.as_ref().ok().map(|p| (*r, p)))
        .collect();
    let raw = |r: &Replayed| r.raw.clone().unwrap_or_default();
    let mut name_score = NameScore::default();
    let compared: Vec<score::AgainstGold> = ok
        .iter()
        .filter_map(|(r, p)| {
            let g = gold.get(&r.sample)?;
            name_score += score::names(&p.text, g, names);
            Some(score::against_gold(&p.text, g))
        })
        .collect();
    let quantiles = |values: Vec<u64>| (median(&values), percentile(&values, 90));
    Row {
        label,
        n: group.len(),
        failed: group.len() - ok.len(),
        fallbacks: ok.iter().filter(|(_, p)| p.fallback).count(),
        unchanged: ok
            .iter()
            .filter(|(r, p)| !p.fallback && score::unchanged(&raw(r), &p.text))
            .count(),
        fillers_raw: ok.iter().map(|(r, _)| score::fillers(&raw(r))).sum(),
        fillers_left: ok.iter().map(|(_, p)| score::fillers(&p.text)).sum(),
        gold: (!compared.is_empty()).then(|| {
            let zero = compared.iter().filter(|c| c.zero_edit).count();
            let mean =
                compared.iter().map(|c| c.edits_per_100).sum::<f64>() / compared.len() as f64;
            (compared.len(), zero, mean)
        }),
        names: name_score,
        stt: quantiles(ok.iter().map(|(_, p)| p.stt_ms).collect()),
        total: quantiles(ok.iter().map(|(_, p)| p.total_ms).collect()),
    }
}

fn opt<T: ToString>(value: Option<T>) -> String {
    value.map_or("-".to_owned(), |v| v.to_string())
}

fn pair((p50, p90): (Option<u64>, Option<u64>)) -> String {
    format!("{} / {}", opt(p50), opt(p90))
}

const COLUMNS: &str = "| Language | Samples | Zero-edit | Edits per 100 chars | Names right | \
Wrong names | Fillers left (in raw) | Unchanged | Fallbacks | Failed | `stt_ms` | `total_ms` |\n\
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n";

/// The table, for the terminal and the results file alike.
pub fn table(rows: &[Row]) -> String {
    let mut out = String::from(COLUMNS);
    for r in rows {
        let (zero, edits) = match r.gold {
            Some((n, zero, mean)) => (format!("{zero}/{n}"), format!("{mean:.1}")),
            None => ("-".to_owned(), "-".to_owned()),
        };
        let (right, wrong) = if r.names.expected > 0 || r.names.wrong > 0 {
            (
                format!("{}/{}", r.names.right, r.names.expected),
                r.names.wrong.to_string(),
            )
        } else {
            ("-".to_owned(), "-".to_owned())
        };
        let _ = writeln!(
            out,
            "| {} | {} | {zero} | {edits} | {right} | {wrong} | {} ({}) | {} | {} | {} | {} | {} |",
            r.label,
            r.n,
            r.fillers_left,
            r.fillers_raw,
            r.unchanged,
            r.fallbacks,
            r.failed,
            pair(r.stt),
            pair(r.total),
        );
    }
    out
}

/// The numbers-only results file for `docs/results/`: no transcript, output, gold text, name or
/// error message (AC-5).
pub fn markdown(
    date_clock: (&str, &str),
    setup: &[(&str, String)],
    rows: &[Row],
    has_gold: bool,
    has_names: bool,
) -> String {
    let (date, clock) = date_clock;
    let mut md = format!(
        "# Quality replay, {date}\n\n\
         Saved dictations replayed through the app's own state machine and providers \
         (`hyperme-quality`), scored against the owner's corrected texts. Numbers only: the \
         audio, texts and names stay in the set folder on the owner's Mac.\n\n\
         ## Setup\n\n| Item | Value |\n| --- | --- |\n| Date | {date} {clock} |\n"
    );
    for (label, value) in setup {
        let _ = writeln!(md, "| {label} | {} |", value.replace('|', "\\|"));
    }
    let _ = write!(
        md,
        "\n## Results\n\nTimings are milliseconds from key release, median / 90th percentile, \
         as in the app's timing log.\n\n{}",
        table(rows)
    );
    md.push_str(
        "\n## Columns\n\n\
         - **Zero-edit:** outputs equal to the owner's corrected text apart from spacing. \
         **Edits per 100 chars:** character edits needed to reach it, per 100 characters of the \
         corrected text. Both over the samples that have one.\n\
         - **Names right:** internal names in the corrected texts that the output has too. \
         **Wrong names:** names in the output where the corrected text has none.\n\
         - **Fillers left:** um, uh, eh, em, hmm, o sea, you know and similar in the pasted \
         text, with the count in the raw transcripts in brackets.\n\
         - **Unchanged:** cleaned text identical to the raw transcript apart from spacing.\n\
         - **Fallbacks:** the raw transcript was pasted, because the cleanup missed its deadline \
         or failed. **Failed:** nothing was pasted.\n",
    );
    if !has_gold {
        md.push_str(
            "\nNo corrected texts in this set, so no zero-edit or edit rate. The run wrote \
             `gold-draft.json` in the set folder to correct into `gold.json`.\n",
        );
    }
    if !has_names {
        md.push_str("\nNo names list in this set, so no names columns.\n");
    }
    md
}

/// The texts, for the owner only: raw, pasted and gold for each sample.
pub fn texts(results: &[Replayed], gold: &BTreeMap<String, String>, title: &str) -> String {
    let mut md = format!("# {title}\n\nPrivate: keep out of the repository.\n");
    for r in results {
        let _ = write!(md, "\n## {}\n\n", r.sample);
        if let Some(raw) = &r.raw {
            let _ = write!(md, "Raw:\n\n{}\n", fence(raw));
        }
        match &r.outcome {
            Ok(p) => {
                let label = if p.fallback {
                    "Pasted (raw, fallback)"
                } else {
                    "Pasted"
                };
                let _ = write!(md, "{label}:\n\n{}\n", fence(&p.text));
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

/// A JSON object of each pasted text, for the owner to correct into gold. Failed samples are
/// left out.
pub fn gold_draft(results: &[Replayed]) -> String {
    let map: Map<String, Value> = results
        .iter()
        .filter_map(|r| {
            let p = r.outcome.as_ref().ok()?;
            Some((r.sample.clone(), Value::String(p.text.clone())))
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
