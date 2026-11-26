//! AC-6 (the report) and AC-5 (numbers only in the results file), issue 99.

use std::collections::BTreeMap;

use hyperme_quality::replay::{Pasted, Replayed};
use hyperme_quality::report::{gold_draft, markdown, summarise, table, texts};
use hyperme_quality::set::Lang;

fn done(sample: &str, raw: &str, text: &str, fallback: bool, stt: u64, total: u64) -> Replayed {
    Replayed {
        sample: sample.to_owned(),
        lang: Lang::of_sample(sample),
        raw: Some(raw.to_owned()),
        outcome: Ok(Pasted {
            text: text.to_owned(),
            fallback,
            stt_ms: stt,
            first_ms: (!fallback).then_some(stt + 100),
            total_ms: total,
        }),
    }
}

fn results() -> Vec<Replayed> {
    vec![
        done(
            "es-01",
            "eh hola zen trix",
            "Hola, Zentrix.",
            false,
            100,
            400,
        ),
        done("es-02", "eh vale", "eh vale", true, 120, 1600),
        done("en-01", "um hi team", "Hi team.", false, 90, 300),
        Replayed {
            sample: "en-02".to_owned(),
            lang: Some(Lang::En),
            raw: None,
            outcome: Err("Speech: connection dropped".to_owned()),
        },
    ]
}

fn gold() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("es-01".to_owned(), "Hola, Zentrix.".to_owned()),
        ("es-02".to_owned(), "Vale.".to_owned()),
        ("en-01".to_owned(), "Hi, team.".to_owned()),
    ])
}

/// AC-6: rows per language and all, with quality, names, fallbacks, failures and timings.
#[test]
fn rows_per_language_carry_every_measure() {
    let names = vec!["Zentrix".to_owned()];
    let rows = summarise(&results(), &gold(), &names);
    let labels: Vec<&str> = rows.iter().map(|r| r.label).collect();
    assert_eq!(labels, ["es", "en", "all"]);

    let es = &rows[0];
    assert_eq!((es.n, es.failed, es.fallbacks), (2, 0, 1));
    let (compared, zero, mean) = es.gold.unwrap();
    assert_eq!((compared, zero), (2, 1));
    assert!(mean > 0.0);
    assert_eq!(
        (es.names.expected, es.names.right, es.names.wrong),
        (1, 1, 0)
    );
    assert_eq!((es.fillers_raw, es.fillers_left), (2, 1));
    // The fallback is not counted as unchanged: only cleaned outputs are.
    assert_eq!(es.unchanged, 0);
    assert_eq!(es.stt, (Some(110), Some(120)));
    assert_eq!(es.total, (Some(1000), Some(1600)));

    let all = &rows[2];
    assert_eq!((all.n, all.failed, all.fallbacks), (4, 1, 1));
    assert_eq!(all.gold.unwrap().0, 3);

    let table = table(&rows);
    assert!(table.contains("| es | 2 |"), "{table}");
    assert!(table.contains("1/1"), "{table}");
}

/// AC-5, AC-6: the results file has the setup and numbers, and none of the texts or names.
#[test]
fn the_results_file_holds_numbers_only() {
    let names = vec!["Zentrix".to_owned()];
    let rows = summarise(&results(), &gold(), &names);
    let md = markdown(
        ("2026-11-26", "22:00 CET"),
        &[("Cleanup", "groq openai/gpt-oss-20b, prompt v4".to_owned())],
        &rows,
        true,
        true,
    );
    assert!(md.contains("| all | 4 |"));
    assert!(md.contains("prompt v4"));
    for private in [
        "Hola",
        "Zentrix",
        "zen trix",
        "Hi team",
        "Vale",
        "connection dropped",
    ] {
        assert!(
            !md.contains(private),
            "the results file must not contain {private:?}"
        );
    }
}

/// AC-5: the texts and the gold draft are for the set folder only, and hold the texts.
#[test]
fn texts_and_gold_draft_hold_the_texts() {
    let private = texts(&results(), &gold(), "Quality replay");
    assert!(private.contains("eh hola zen trix") && private.contains("Hola, Zentrix."));
    assert!(private.contains("connection dropped"));
    let draft: serde_json::Value = serde_json::from_str(&gold_draft(&results())).unwrap();
    assert_eq!(draft["en-01"], "Hi team.");
    // A failed sample has no draft.
    assert!(draft.get("en-02").is_none());
}
