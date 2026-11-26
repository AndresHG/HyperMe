//! Automatic quality signals for one output against the owner's corrected text (AC-1, AC-2).
//! Ported from `spikes/flowbench/src/quality.rs`, with the same definitions, so numbers compare
//! with Phase 0's. The owner's reading stays the judge.
//!
//! - **Zero-edit:** the output equals the gold text once spacing is normalised.
//! - **Edits per 100 characters:** character edit distance to the gold text.
//! - **Fillers left:** hesitations that carry no meaning in any context. Words that only
//!   sometimes are fillers (bueno, pues, like) are left out, so this undercounts.
//! - **Internal names:** names from the user's dictionary, counted in the gold and the output.

use std::ops::AddAssign;

/// Hesitations in English and Spanish that are never wanted in typed text.
const FILLERS: &[&str] = &[
    "um", "umm", "uh", "uhh", "uhm", "er", "erm", "hmm", "mm", "mmm", "eh", "ehh", "em", "ehm",
];
/// Filler phrases, matched as whole words.
const FILLER_PHRASES: &[&[&str]] = &[&["o", "sea"], &["you", "know"]];

/// Lower case, words only: punctuation dropped, so "Um," is the word "um".
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Filler words and phrases in `text`.
pub fn fillers(text: &str) -> usize {
    let words = words(text);
    let single = words
        .iter()
        .filter(|w| FILLERS.contains(&w.as_str()))
        .count();
    let phrases = FILLER_PHRASES
        .iter()
        .map(|phrase| {
            words
                .windows(phrase.len())
                .filter(|window| window.iter().zip(phrase.iter()).all(|(w, p)| w == p))
                .count()
        })
        .sum::<usize>();
    single + phrases
}

/// Spacing collapsed to single spaces, line breaks kept, ends trimmed.
pub fn normalise(text: &str) -> String {
    text.trim()
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The cleanup changed nothing but spacing.
pub fn unchanged(raw: &str, cleaned: &str) -> bool {
    normalise(raw) == normalise(cleaned)
}

/// Levenshtein distance over characters.
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitution = previous[j] + usize::from(ca != cb);
            current[j + 1] = substitution.min(previous[j + 1] + 1).min(current[j] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

/// Comparison of one output with the gold text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AgainstGold {
    pub zero_edit: bool,
    /// Character edits per 100 characters of the gold text.
    pub edits_per_100: f64,
}

pub fn against_gold(output: &str, gold: &str) -> AgainstGold {
    let (output, gold) = (normalise(output), normalise(gold));
    let distance = edit_distance(&output, &gold);
    AgainstGold {
        zero_edit: distance == 0,
        edits_per_100: 100.0 * distance as f64 / gold.chars().count().max(1) as f64,
    }
}

/// The written names in a dictionary file in flowbench's format: one term per line, the
/// written form before an optional `:` and its sound-alikes, `#` for comments.
pub fn parse_names(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .map(|line| line.split(':').next().unwrap_or_default().trim().to_owned())
        .filter(|name| !name.is_empty())
        .collect()
}

/// How often `name` appears in `text` as a whole word or phrase, case-sensitive.
pub fn occurrences(text: &str, name: &str) -> usize {
    if name.is_empty() {
        return 0;
    }
    let boundary = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric());
    text.match_indices(name)
        .filter(|(at, _)| {
            boundary(text[..*at].chars().next_back())
                && boundary(text[at + name.len()..].chars().next())
        })
        .count()
}

/// Internal names in one output, or summed over many.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NameScore {
    /// Occurrences of the names in the gold texts.
    pub expected: usize,
    /// Of those, how many the output has.
    pub right: usize,
    /// Occurrences in the output beyond the gold's: a name written where none was said.
    pub wrong: usize,
}

impl AddAssign for NameScore {
    fn add_assign(&mut self, other: Self) {
        self.expected += other.expected;
        self.right += other.right;
        self.wrong += other.wrong;
    }
}

/// Scores the names in one output against its gold text.
pub fn names(output: &str, gold: &str, names: &[String]) -> NameScore {
    let mut score = NameScore::default();
    for name in names {
        let (want, got) = (occurrences(gold, name), occurrences(output, name));
        score += NameScore {
            expected: want,
            right: want.min(got),
            wrong: got.saturating_sub(want),
        };
    }
    score
}
