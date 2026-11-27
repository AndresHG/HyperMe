//! Learning the dictionary from the user's own fixes (spec 93, R-16 to R-19; design.md,
//! "Learning"). Pure code: the field watcher (slice D7) hands over what HyperMe pasted and what
//! the same stretch of the field held when the watch ended; [`candidates`] finds the fixes of
//! misheard words in it, and [`step`] decides what becomes of each one.

use crate::dictionary::{Dictionary, is_common_word, key};

/// Above this share of the pasted words changed, the edit is a rewrite, not a fix.
const MAX_CHANGED: f32 = 0.4;
/// The longest replaced run kept, in words, on either side.
const MAX_RUN: usize = 3;
/// Letters at least this alike (bigram Dice) look like a misheard word (`sun trix`, `Zentrix`).
const MIN_LIKE: f32 = 0.4;
/// A name-like replacement must still be this alike (`Ruis` to `Ruiz`, not `john` to `Mary`).
const MIN_NAME_LIKE: f32 = 0.2;
/// Seen in this many dictations without a fix, a word counts as common (R-17).
pub const COMMON_AFTER: u32 = 3;

/// A fix that looks like a misheard word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// How it was heard: the raw transcript's words first, then the pasted words if they
    /// differ (the cleanup may have changed them).
    pub heard: Vec<String>,
    /// How the user wrote it.
    pub wanted: String,
    /// Only the case changed (`zentrix` to `Zentrix`): the entry alone fixes it.
    pub spelling_only: bool,
}

/// A word as compared: letters and digits only, lower case, accents folded.
fn norm(word: &str) -> String {
    key(&word
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>())
}

/// A word as kept: without the punctuation around it.
fn trimmed(word: &str) -> &str {
    word.trim_matches(|c: char| !c.is_alphanumeric())
}

/// The words of `text` that hold a letter or a digit.
fn words(text: &str) -> Vec<&str> {
    text.split_whitespace()
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .collect()
}

/// The pairs `(i, j)` of a longest common subsequence of `a` and `b`, in order. Dictations are
/// short, so the quadratic table is small.
fn lcs(a: &[String], b: &[String]) -> Vec<(usize, usize)> {
    let (n, m) = (a.len(), b.len());
    let mut table = vec![0u32; (n + 1) * (m + 1)];
    let at = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[at(i, j)] = if a[i] == b[j] {
                table[at(i + 1, j + 1)] + 1
            } else {
                table[at(i + 1, j)].max(table[at(i, j + 1)])
            };
        }
    }
    let (mut i, mut j, mut pairs) = (0, 0, Vec::new());
    while i < n && j < m {
        if a[i] == b[j] {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if table[at(i + 1, j)] >= table[at(i, j + 1)] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pairs
}

/// Character-bigram Dice similarity of the folded letters of two texts, 0 to 1.
fn alike(a: &str, b: &str) -> f32 {
    let bigrams = |s: &str| -> Vec<(char, char)> {
        let letters: Vec<char> = norm(s).chars().collect();
        letters.windows(2).map(|w| (w[0], w[1])).collect()
    };
    let (x, mut y) = (bigrams(a), bigrams(b));
    if x.is_empty() || y.is_empty() {
        return 0.0;
    }
    let total = x.len() + y.len();
    let mut shared = 0;
    for g in x {
        if let Some(at) = y.iter().position(|h| *h == g) {
            y.swap_remove(at);
            shared += 1;
        }
    }
    (2 * shared) as f32 / total as f32
}

fn joined(words: &[&str]) -> String {
    words
        .iter()
        .map(|w| trimmed(w))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The fixes of misheard words between what was `pasted` and what the user left (`edited`),
/// with `raw`, the transcript the pasted text was cleaned from, for the heard forms (R-16,
/// AC-13). Rewrites, added or deleted words and punctuation are not fixes.
pub fn candidates(pasted: &str, edited: &str, raw: &str) -> Vec<Candidate> {
    let (old, new) = (words(pasted), words(edited));
    let (old_n, new_n): (Vec<String>, Vec<String>) = (
        old.iter().map(|w| norm(w)).collect(),
        new.iter().map(|w| norm(w)).collect(),
    );
    if old.is_empty() {
        return Vec::new();
    }
    let pairs = lcs(&old_n, &new_n);
    if (old.len() - pairs.len()) as f32 > MAX_CHANGED * old.len() as f32 {
        return Vec::new();
    }
    let raw_words = words(raw);
    let raw_n: Vec<String> = raw_words.iter().map(|w| norm(w)).collect();
    let raw_pairs = lcs(&raw_n, &old_n);

    let mut found = Vec::new();
    // Case only: the words match as compared but not as written.
    for &(i, j) in &pairs {
        let (was, now) = (trimmed(old[i]), trimmed(new[j]));
        if was != now && now.chars().any(char::is_uppercase) && !is_common_word(now) {
            let capitals = |w: &str| w.chars().filter(|c| c.is_uppercase()).count();
            // A capital at the start of a sentence, the user's or the paste's, is not a name.
            let ends = |w: &str| w.ends_with(['.', '!', '?', ':']);
            let sentence_start = i == 0 || j == 0 || ends(new[j - 1]) || ends(old[i - 1]);
            if capitals(now) > capitals(was) && !sentence_start {
                found.push(Candidate {
                    heard: vec![was.to_owned()],
                    wanted: now.to_owned(),
                    spelling_only: true,
                });
            }
        }
    }
    // Replaced runs between the matches.
    let ends = pairs.iter().copied().chain([(old.len(), new.len())]);
    let (mut i0, mut j0) = (0, 0);
    for (i, j) in ends {
        let (was, now) = (&old[i0..i], &new[j0..j]);
        let start = i0;
        (i0, j0) = (i + 1, j + 1);
        if was.is_empty() || now.is_empty() || was.len() > MAX_RUN || now.len() > MAX_RUN {
            continue;
        }
        let now_n: Vec<String> = new_n[j - now.len()..j].to_vec();
        let already = old_n.windows(now_n.len()).any(|w| w == now_n.as_slice());
        let (was_text, now_text) = (joined(was), joined(now));
        let capitals = |ws: &[&str]| ws.iter().any(|w| w.chars().any(char::is_uppercase));
        let like = alike(&was_text, &now_text);
        // A name, or a term like `k8s`: letters with a digit, or a new capital; never a plain
        // number (`nine` to `9`), and somewhat alike, so one name swapped for another is not a fix.
        let name_like = like >= MIN_NAME_LIKE
            && ((now_text.chars().any(|c| c.is_ascii_digit())
                && now_text.chars().any(char::is_alphabetic))
                || (capitals(now) && !capitals(was) && was.len() <= 2));
        if already || !(like >= MIN_LIKE || name_like) {
            continue;
        }
        let mut heard = Vec::new();
        if let Some(from_raw) = raw_stretch(&raw_words, &raw_pairs, start, start + was.len())
            && norm(&from_raw.replace(' ', "")) != norm(&was_text.replace(' ', ""))
        {
            heard.push(from_raw);
        }
        heard.push(was_text);
        found.push(Candidate {
            heard,
            wanted: now_text,
            spelling_only: false,
        });
    }
    found
}

/// The raw words in the place of pasted words `from..to`. When some of those pasted words
/// match raw words, the raw words from the first match to the last; otherwise the raw words
/// between the matches just before and just after, so a filler the cleanup dropped next to the
/// run is not swept in when the run itself is matched. `None` unless that is 1 to 3 words.
fn raw_stretch(raw: &[&str], pairs: &[(usize, usize)], from: usize, to: usize) -> Option<String> {
    let inside: Vec<usize> = pairs
        .iter()
        .filter(|&&(_, p)| (from..to).contains(&p))
        .map(|&(r, _)| r)
        .collect();
    let (before, after) = match (inside.first(), inside.last()) {
        (Some(&first), Some(&last)) => (first, last + 1),
        _ => (
            pairs
                .iter()
                .rev()
                .find(|&&(_, p)| p < from)
                .map_or(0, |&(r, _)| r + 1),
            pairs
                .iter()
                .find(|&&(_, p)| p >= to)
                .map_or(raw.len(), |&(r, _)| r),
        ),
    };
    (after > before && after - before <= MAX_RUN).then(|| joined(&raw[before..after]))
}

/// What is already known about a candidate, from the store and the history.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Known {
    /// Times this same fix was seen before.
    pub seen_before: u32,
    /// The user dismissed it, or deleted it once learned (R-19).
    pub dismissed: bool,
    /// Dictations that held the heard form and were not corrected there (R-17).
    pub uncorrected: u32,
}

/// What becomes of a candidate (R-17 to R-19, AC-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Nothing: dismissed before, or the dictionary already gives the right word.
    Nothing,
    /// A suggestion that waits for the user's yes.
    Suggest,
    /// Added at once, marked learned, with undo.
    Learn,
}

/// The learning policy (owner, 26 November 2026): seen once, a suggestion; seen twice, added
/// as learned; a common heard form stays a suggestion however often it is seen; a dismissed one
/// never comes back.
pub fn step(candidate: &Candidate, dictionary: &Dictionary, known: Known) -> Step {
    let Some(heard) = candidate.heard.first() else {
        return Step::Nothing;
    };
    let covered = candidate
        .heard
        .iter()
        .all(|h| dictionary.apply(h).0 == candidate.wanted);
    if known.dismissed || covered {
        return Step::Nothing;
    }
    let common = known.uncorrected >= COMMON_AFTER
        || (!candidate.spelling_only
            && (candidate.heard.iter().any(|h| is_common_word(h))
                || words(heard).iter().all(|w| is_common_word(w))));
    if common || known.seen_before == 0 {
        Step::Suggest
    } else {
        Step::Learn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alike_scores_misheard_names_high_and_unrelated_words_low() {
        assert!(alike("sun trix", "Zentrix") >= MIN_LIKE);
        assert!(alike("corbit", "Quorbit") >= MIN_LIKE);
        assert!(alike("send", "email") < MIN_LIKE);
        assert_eq!(alike("", "x"), 0.0);
    }

    #[test]
    fn raw_words_in_place_of_pasted_ones() {
        let raw = ["the", "sun", "trix", "build"];
        let pairs = [(0, 0), (3, 2)];
        assert_eq!(raw_stretch(&raw, &pairs, 1, 2).as_deref(), Some("sun trix"));
    }
}
