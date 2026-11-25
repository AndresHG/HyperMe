//! Rough, automatic quality signals for cleaned text. The owner's reading stays the judge.
//!
//! - **Zero-edit:** the output equals the owner's gold text once spacing is normalised. Voice
//!   Arena ranks dictation apps on the same idea.
//! - **Edits per 100 characters:** character edit distance to the gold text, so progress shows
//!   before an output is perfect.
//! - **Fillers left:** words from a short list of hesitations that carry no meaning in any
//!   context. Words that only sometimes are fillers (bueno, pues, like) are left out, so this
//!   count undercounts rather than flags good text.

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
#[derive(Debug, PartialEq)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fillers_are_counted_as_whole_words_in_both_languages() {
        assert_eq!(fillers("Um, I think, uh, yes"), 2);
        assert_eq!(fillers("Eh, mañana, o sea, el lunes"), 2);
        assert_eq!(fillers("you know what, I know you"), 1);
        // Words that only contain a filler are not fillers.
        assert_eq!(fillers("The umbrella is here, Emma."), 0);
        assert_eq!(fillers("Hola equipo."), 0);
    }

    #[test]
    fn spacing_is_normalised_but_line_breaks_stay() {
        assert_eq!(
            normalise("  Hola   equipo.\n\n- uno \n"),
            "Hola equipo.\n- uno"
        );
        assert!(unchanged("hola  equipo", " hola equipo "));
        assert!(!unchanged("hola equipo", "Hola, equipo."));
    }

    #[test]
    fn edit_distance_counts_characters() {
        assert_eq!(edit_distance("", ""), 0);
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("mañana", "manana"), 1);
        assert_eq!(edit_distance("abc", ""), 3);
    }

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
}
