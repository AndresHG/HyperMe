//! AC-1 (scoring as flowbench) and AC-2 (internal names), issue 99.

use hyperme_quality::score::{
    AgainstGold, NameScore, against_gold, edit_distance, fillers, names, normalise, occurrences,
    parse_names, unchanged,
};

/// AC-1: the same filler list and whole-word rule as flowbench.
#[test]
fn fillers_are_counted_as_whole_words_in_both_languages() {
    assert_eq!(fillers("Um, I think, uh, yes"), 2);
    assert_eq!(fillers("Eh, mañana, o sea, el lunes"), 2);
    assert_eq!(fillers("you know what, I know you"), 1);
    assert_eq!(fillers("The umbrella is here, Emma."), 0);
    assert_eq!(fillers("Hola equipo."), 0);
}

/// AC-1.
#[test]
fn spacing_is_normalised_but_line_breaks_stay() {
    assert_eq!(
        normalise("  Hola   equipo.\n\n- uno \n"),
        "Hola equipo.\n- uno"
    );
    assert!(unchanged("hola  equipo", " hola equipo "));
    assert!(!unchanged("hola equipo", "Hola, equipo."));
}

/// AC-1.
#[test]
fn edit_distance_counts_characters() {
    assert_eq!(edit_distance("", ""), 0);
    assert_eq!(edit_distance("kitten", "sitting"), 3);
    assert_eq!(edit_distance("mañana", "manana"), 1);
    assert_eq!(edit_distance("abc", ""), 3);
}

/// AC-1: zero-edit and edits per 100 characters of the gold text.
#[test]
fn gold_comparison_gives_zero_edit_and_a_rate() {
    assert_eq!(
        against_gold("Hola  equipo.", "Hola equipo."),
        AgainstGold {
            zero_edit: true,
            edits_per_100: 0.0
        }
    );
    let off = against_gold("Hola equipo", "Hola equipo.");
    assert!(!off.zero_edit);
    assert!((off.edits_per_100 - 100.0 / 12.0).abs() < 1e-9);
}

/// AC-2: names come from flowbench's dictionary format, the written form before any `:`.
#[test]
fn names_come_from_the_dictionary_format() {
    let list = parse_names("# internal names\nZentrix: zen trix, sun trix\n\nQuorbit\n : none\n");
    assert_eq!(list, ["Zentrix", "Quorbit"]);
}

/// AC-2: whole words, case-sensitive.
#[test]
fn occurrences_are_whole_words_and_case_sensitive() {
    assert_eq!(occurrences("Zentrix, and Zentrix.", "Zentrix"), 2);
    assert_eq!(occurrences("zentrix", "Zentrix"), 0);
    assert_eq!(occurrences("Zentrixes", "Zentrix"), 0);
    assert_eq!(occurrences("the Lang Chain way", "Lang Chain"), 1);
    assert_eq!(occurrences("añoZentrix", "Zentrix"), 0);
}

/// AC-2: expected from the gold, right up to that count, the rest wrong substitutions.
#[test]
fn names_score_right_and_wrong_substitutions() {
    let list = vec!["Zentrix".to_owned(), "Quorbit".to_owned()];
    assert_eq!(
        names(
            "Open Zentrix and sun trix.",
            "Open Zentrix and Zentrix.",
            &list
        ),
        NameScore {
            expected: 2,
            right: 1,
            wrong: 0
        }
    );
    assert_eq!(
        names("Quorbit is a Quorbit.", "Corbit is a Quorbit.", &list),
        NameScore {
            expected: 1,
            right: 1,
            wrong: 1
        }
    );
    let mut total = NameScore::default();
    total += names("Zentrix", "Zentrix", &list);
    total += names("Quorbit", "nothing", &list);
    assert_eq!(
        total,
        NameScore {
            expected: 1,
            right: 1,
            wrong: 1
        }
    );
}
