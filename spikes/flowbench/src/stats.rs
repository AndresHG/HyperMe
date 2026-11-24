//! Median, 90th percentile and the gate, per pipeline and per language.

use crate::lang::Lang;
use crate::pipeline::{Measured, Pipeline, SampleResult};

/// Gate thresholds from `docs/SPEED_TEST.md`, on `total_ms`. The owner can change them.
pub const GATE_MEDIAN_MS: u64 = 800;
pub const GATE_P90_MS: u64 = 1200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quantiles {
    pub p50: u64,
    pub p90: u64,
}

/// The middle value; with an even count, the mean of the two middle values, rounded up.
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

/// Nearest-rank percentile: the smallest value with at least `percent` % of the values at or
/// below it. For 50 samples the 90th percentile is the 45th smallest.
pub fn percentile(values: &[u64], percent: usize) -> Option<u64> {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let rank = (sorted.len() * percent).div_ceil(100).max(1);
    sorted.get(rank - 1).copied()
}

fn quantiles(values: &[u64]) -> Option<Quantiles> {
    Some(Quantiles {
        p50: median(values)?,
        p90: percentile(values, 90)?,
    })
}

/// One line of the results table.
pub struct Row {
    pub pipeline: Pipeline,
    /// `None` is every language together.
    pub lang: Option<Lang>,
    /// Measurements attempted: samples times runs.
    pub n: usize,
    pub failed: usize,
    pub fallbacks: usize,
    /// Completed measurements whose spoken language is known.
    pub lang_checked: usize,
    pub lang_ok: usize,
    pub stt: Option<Quantiles>,
    /// Over the measurements in which a cleaned token arrived.
    pub first: Option<Quantiles>,
    /// Over every completed measurement, fallbacks included.
    pub total: Option<Quantiles>,
}

/// For each pipeline: Spanish, English, then every sample together.
pub fn aggregate(results: &[SampleResult]) -> Vec<Row> {
    let mut rows = Vec::new();
    for pipeline in [Pipeline::A, Pipeline::B] {
        for lang in [Some(Lang::Es), Some(Lang::En), None] {
            let group: Vec<&SampleResult> = results
                .iter()
                .filter(|r| r.pipeline == pipeline && (lang.is_none() || r.lang == lang))
                .collect();
            if !group.is_empty() {
                rows.push(row(pipeline, lang, &group));
            }
        }
    }
    rows
}

fn row(pipeline: Pipeline, lang: Option<Lang>, group: &[&SampleResult]) -> Row {
    let done: Vec<&Measured> = group
        .iter()
        .filter_map(|r| r.outcome.as_ref().ok())
        .collect();
    let column = |pick: fn(&Measured) -> Option<u64>| {
        let values: Vec<u64> = done.iter().filter_map(|m| pick(m)).collect();
        quantiles(&values)
    };
    Row {
        pipeline,
        lang,
        n: group.len(),
        failed: group.len() - done.len(),
        fallbacks: done.iter().filter(|m| m.fallback).count(),
        lang_checked: done.iter().filter(|m| m.lang_ok.is_some()).count(),
        lang_ok: done.iter().filter(|m| m.lang_ok == Some(true)).count(),
        stt: column(|m| Some(m.stt_ms)),
        first: column(|m| m.first_ms),
        total: column(|m| Some(m.total_ms)),
    }
}

/// Why a row misses the speed and language gate. Empty means it passes.
///
/// Formatting quality is the other half of the decision and is judged by a person.
pub fn gate_problems(row: &Row) -> Vec<String> {
    let mut problems = Vec::new();
    match row.total {
        None => problems.push("no sample completed".to_owned()),
        Some(total) => {
            if total.p50 > GATE_MEDIAN_MS {
                problems.push(format!(
                    "median total_ms {} is over {GATE_MEDIAN_MS}",
                    total.p50
                ));
            }
            if total.p90 > GATE_P90_MS {
                problems.push(format!(
                    "90th percentile total_ms {} is over {GATE_P90_MS}",
                    total.p90
                ));
            }
        }
    }
    if row.lang_ok < row.lang_checked {
        problems.push(format!(
            "language wrong on {} of {}",
            row.lang_checked - row.lang_ok,
            row.lang_checked
        ));
    }
    // A fallback has no cleaned text. A 90th percentile lets one sample in ten miss, and no
    // more: otherwise a short deadline would pass the gate without any cleanup.
    let done = row.n - row.failed;
    if row.fallbacks > done - (done * 90).div_ceil(100) {
        problems.push(format!(
            "raw transcript used on {} of {}, more than one in ten",
            row.fallbacks, done
        ));
    }
    if row.failed > 0 {
        problems.push(format!("{} of {} failed", row.failed, row.n));
    }
    problems
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn median_of_odd_and_even_counts() {
        assert_eq!(median(&[]), None);
        assert_eq!(median(&[7]), Some(7));
        assert_eq!(median(&[900, 100, 500]), Some(500));
        assert_eq!(median(&[400, 100, 300, 200]), Some(250));
        assert_eq!(median(&[1, 2]), Some(2));
    }

    #[test]
    fn percentile_is_nearest_rank() {
        let ten: Vec<u64> = (1..=10).map(|i| i * 100).collect();
        assert_eq!(percentile(&ten, 90), Some(900));
        let fifty: Vec<u64> = (1..=50).rev().collect();
        assert_eq!(percentile(&fifty, 90), Some(45));
        assert_eq!(percentile(&[42], 90), Some(42));
        assert_eq!(percentile(&[], 90), None);
    }

    /// A completed measurement with the given times.
    pub fn done(
        sample: &str,
        pipeline: Pipeline,
        stt_ms: u64,
        first_ms: Option<u64>,
        total_ms: u64,
    ) -> SampleResult {
        let lang = Lang::from_code(&sample[..2]);
        SampleResult {
            sample: sample.to_owned(),
            lang,
            pipeline,
            run: 1,
            audio_ms: 8_000,
            outcome: Ok(Measured {
                open_ms: -8_100,
                last_audio_ms: (pipeline == Pipeline::B).then_some(0),
                stt_ms,
                first_ms,
                total_ms,
                raw: format!("raw text of {sample}"),
                text: format!("Clean text of {sample}."),
                fallback: first_ms.is_none(),
                fallback_note: first_ms
                    .is_none()
                    .then(|| "cleanup not complete".to_owned()),
                provider_languages: vec![],
                lang_ok: Some(true),
            }),
        }
    }

    pub fn failed(sample: &str, pipeline: Pipeline) -> SampleResult {
        SampleResult {
            sample: sample.to_owned(),
            lang: Lang::from_code(&sample[..2]),
            pipeline,
            run: 1,
            audio_ms: 8_000,
            outcome: Err("HTTP 500: upstream error".to_owned()),
        }
    }

    #[test]
    fn rows_are_per_pipeline_and_per_language() {
        let results = vec![
            done("es-01", Pipeline::A, 600, Some(900), 1_000),
            done("es-02", Pipeline::A, 700, Some(1_000), 1_200),
            done("en-01", Pipeline::A, 500, Some(800), 900),
            done("es-01", Pipeline::B, 200, Some(400), 500),
            done("es-02", Pipeline::B, 300, None, 1_800),
            done("en-01", Pipeline::B, 250, Some(450), 600),
            failed("en-02", Pipeline::B),
        ];
        let rows = aggregate(&results);
        let labels: Vec<_> = rows
            .iter()
            .map(|r| (r.pipeline, r.lang.map(Lang::code), r.n))
            .collect();
        assert_eq!(
            labels,
            vec![
                (Pipeline::A, Some("es"), 2),
                (Pipeline::A, Some("en"), 1),
                (Pipeline::A, None, 3),
                (Pipeline::B, Some("es"), 2),
                (Pipeline::B, Some("en"), 2),
                (Pipeline::B, None, 4),
            ]
        );

        let a_all = &rows[2];
        assert_eq!(a_all.stt, Some(Quantiles { p50: 600, p90: 700 }));
        assert_eq!(
            a_all.total,
            Some(Quantiles {
                p50: 1_000,
                p90: 1_200
            })
        );
        assert_eq!((a_all.failed, a_all.fallbacks), (0, 0));

        let b_all = &rows[5];
        // The failure is left out of the times; the fallback counts in total but has no first.
        assert_eq!(b_all.stt, Some(Quantiles { p50: 250, p90: 300 }));
        assert_eq!(b_all.first, Some(Quantiles { p50: 425, p90: 450 }));
        assert_eq!(
            b_all.total,
            Some(Quantiles {
                p50: 600,
                p90: 1_800
            })
        );
        assert_eq!((b_all.failed, b_all.fallbacks), (1, 1));
        assert_eq!((b_all.lang_ok, b_all.lang_checked), (3, 3));
    }

    #[test]
    fn gate_passes_at_the_thresholds_and_fails_above_them() {
        let at_limit = vec![
            done("es-01", Pipeline::B, 200, Some(400), 800),
            done("en-01", Pipeline::B, 200, Some(400), 1_200),
            done("en-02", Pipeline::B, 200, Some(400), 700),
        ];
        assert!(gate_problems(&aggregate(&at_limit)[2]).is_empty());

        let slow = vec![
            done("es-01", Pipeline::B, 200, Some(400), 801),
            done("en-01", Pipeline::B, 200, Some(400), 1_201),
            done("en-02", Pipeline::B, 200, Some(400), 900),
        ];
        assert_eq!(
            gate_problems(&aggregate(&slow)[2]),
            [
                "median total_ms 900 is over 800",
                "90th percentile total_ms 1201 is over 1200"
            ]
        );
    }

    #[test]
    fn gate_fails_on_wrong_language_and_on_failures() {
        let mut wrong = done("es-01", Pipeline::B, 200, Some(400), 500);
        wrong.outcome.as_mut().unwrap().lang_ok = Some(false);
        let results = vec![
            wrong,
            done("en-01", Pipeline::B, 200, Some(400), 500),
            failed("en-02", Pipeline::B),
        ];
        assert_eq!(
            gate_problems(&aggregate(&results)[2]),
            ["language wrong on 1 of 2", "1 of 3 failed"]
        );
        let none = vec![failed("en-01", Pipeline::A)];
        assert_eq!(
            gate_problems(&aggregate(&none)[1]),
            ["no sample completed", "1 of 1 failed"]
        );
    }

    #[test]
    fn gate_fails_when_more_than_one_sample_in_ten_falls_back() {
        // With a short deadline every fallback is fast, so the times alone would pass.
        let all: Vec<_> = (1..=10)
            .map(|i| done(&format!("en-{i:02}"), Pipeline::B, 200, None, 500))
            .collect();
        assert_eq!(
            gate_problems(&aggregate(&all)[1]),
            ["raw transcript used on 10 of 10, more than one in ten"]
        );

        // One in ten is what a 90th percentile allows.
        let mut one: Vec<_> = (1..=9)
            .map(|i| done(&format!("en-{i:02}"), Pipeline::B, 200, Some(400), 500))
            .collect();
        one.push(done("en-10", Pipeline::B, 200, None, 1_700));
        assert!(gate_problems(&aggregate(&one)[1]).is_empty());
        one.push(done("en-11", Pipeline::B, 200, None, 1_700));
        assert_eq!(
            gate_problems(&aggregate(&one)[1]),
            [
                "90th percentile total_ms 1700 is over 1200",
                "raw transcript used on 2 of 11, more than one in ten"
            ]
        );
    }
}
