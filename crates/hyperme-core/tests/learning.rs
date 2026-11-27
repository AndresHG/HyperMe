//! Spec 93, slice D6: the candidate finder (AC-13) and the learning policy (AC-14), on fixture
//! edits made of invented names only.

use hyperme_core::dictionary::Dictionary;
use hyperme_core::learning::{Candidate, Known, Step, candidates, step};

const EDITS: &str = include_str!("fixtures/dictionary/edits.txt");

/// A candidate written as the fixtures write it: `heard, heard -> wanted`, with
/// ` (spelling)` for a case-only fix.
fn written(c: &Candidate) -> String {
    let spelling = if c.spelling_only { " (spelling)" } else { "" };
    format!("{} -> {}{spelling}", c.heard.join(", "), c.wanted)
}

/// AC-13 (R-16): misheard-word fixes are kept, with the raw transcript's words as the first
/// heard form; rewrites, added sentences, punctuation-only and deleted-word changes are not.
#[test]
fn the_finder_keeps_misheard_words_and_drops_edits() {
    let mut checked = 0;
    for line in EDITS
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
    {
        let parts: Vec<&str> = line.split('|').map(str::trim).collect();
        let [kind, raw, pasted, edited, expected] = parts[..] else {
            panic!("bad fixture line: {line}");
        };
        let found: Vec<String> = candidates(pasted, edited, raw)
            .iter()
            .map(written)
            .collect();
        let want: Vec<String> = match kind {
            "keep" => expected.split("; ").map(str::to_owned).collect(),
            _ => Vec::new(),
        };
        assert_eq!(found, want, "{line}");
        checked += 1;
    }
    assert_eq!(checked, 19);
}

fn candidate(heard: &str, wanted: &str) -> Candidate {
    Candidate {
        heard: vec![heard.to_owned()],
        wanted: wanted.to_owned(),
        spelling_only: false,
    }
}

/// AC-14 (R-17 to R-19): seen once, a suggestion; seen twice, learned; a common heard form,
/// built in or seen uncorrected three times, stays a suggestion; a dismissed one, or one the
/// dictionary already fixes, never comes back.
#[test]
fn the_policy_suggests_then_learns_and_respects_dismissals() {
    let empty = Dictionary::default();
    let fix = candidate("sun trix", "Zentrix");
    let first = Known::default();
    let again = Known {
        seen_before: 1,
        ..Known::default()
    };
    assert_eq!(step(&fix, &empty, first), Step::Suggest);
    assert_eq!(step(&fix, &empty, again), Step::Learn);

    // Common heard forms: the built-in list, and the user's own frequent words.
    let common = candidate("mesa", "Mesa Labs");
    assert_eq!(
        step(
            &common,
            &empty,
            Known {
                seen_before: 5,
                ..again
            }
        ),
        Step::Suggest
    );
    let frequent = Known {
        uncorrected: 3,
        ..again
    };
    assert_eq!(step(&fix, &empty, frequent), Step::Suggest);

    // Dismissed, or already fixed by the dictionary: nothing.
    let dismissed = Known {
        dismissed: true,
        ..again
    };
    assert_eq!(step(&fix, &empty, dismissed), Step::Nothing);
    let known = Dictionary::parse("Zentrix: sun trix");
    assert_eq!(step(&fix, &known, again), Step::Nothing);
}
