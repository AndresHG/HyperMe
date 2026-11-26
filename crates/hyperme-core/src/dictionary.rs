//! The user's dictionary (spec 93): names and terms the speech engine cannot guess, such as
//! internal product names. Ported from `spikes/flowbench/src/vocab.rs` (issues 31 and 34).
//!
//! The text format, one entry per line:
//!
//! ```text
//! # comment
//! LangChain
//! Zentrix: zen trix, sun trix
//! ```
//!
//! The part before `:` is how the entry is written; the optional comma-separated part after it
//! lists how the speech engine gets it wrong (heard forms). Before anything else uses the final
//! transcript, [`Dictionary::apply`] swaps each heard form for its entry (R-1 to R-3), and
//! [`Dictionary::prompt_line`] gives the cleanup model the written forms (R-4).
//!
//! Pure code: the app reads the file and hands its text to [`Dictionary::parse`].

use std::collections::BTreeMap;

/// At most this many entries go in the cleanup prompt (R-5). Slice D3 adds a relevance filter
/// for the rest; until then the first ones in alphabetical order are used.
pub const PROMPT_CAP: usize = 150;

/// Frequent Spanish and English words. An entry that is one of them gets no implicit forms
/// (R-3, R-17), so adding "Mesa" does not capitalise every "mesa". Written by us; slice D6 adds
/// the words the user says often without correcting them.
const COMMON: &[&str] = &[
    // English
    "a", "about", "after", "all", "also", "an", "and", "any", "are", "as", "at", "back", "be",
    "because", "but", "by", "can", "come", "could", "day", "do", "even", "first", "for", "from",
    "get", "give", "go", "good", "have", "he", "her", "him", "his", "how", "i", "if", "in", "into",
    "is", "it", "its", "just", "know", "like", "look", "make", "me", "most", "my", "new", "no",
    "not", "now", "of", "on", "one", "only", "or", "other", "our", "out", "over", "people", "say",
    "see", "she", "so", "some", "take", "than", "that", "the", "their", "them", "then", "there",
    "these", "they", "think", "this", "time", "to", "two", "up", "us", "use", "want", "way", "we",
    "well", "what", "when", "which", "who", "will", "with", "work", "would", "year", "you", "your",
    // Spanish
    "al", "algo", "ahora", "año", "aquí", "así", "bien", "casa", "como", "con", "cosa", "cuando",
    "de", "del", "día", "donde", "el", "ella", "en", "entre", "era", "es", "esa", "ese", "eso",
    "esta", "este", "esto", "hacer", "hay", "hoy", "la", "las", "le", "lo", "los", "mañana", "más",
    "me", "mesa", "mi", "mucho", "muy", "nada", "ni", "no", "nos", "o", "para", "pero", "poco",
    "por", "porque", "que", "qué", "se", "ser", "si", "sí", "sin", "sobre", "son", "su", "también",
    "te", "tiempo", "todo", "tu", "un", "una", "uno", "vale", "vez", "y", "ya", "yo",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Term {
    pub written: String,
    pub heard: Vec<String>,
}

/// The dictionary, compiled once when it is loaded so a dictation only scans (R-6, R-8).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dictionary {
    terms: Vec<Term>,
    /// Folded heard forms, explicit and implicit, longest first, each with its entry.
    variants: Vec<(Vec<char>, String)>,
    /// Indexes into `variants` by first folded character, still longest first.
    by_first: BTreeMap<char, Vec<usize>>,
}

/// Lower case, and `á é í ó ú ü à è ì ò ù` without their accents. `ñ` stays: `año` and `ano`
/// are different words.
fn fold(c: char) -> char {
    let c = c.to_lowercase().next().unwrap_or(c);
    match c {
        'á' | 'à' => 'a',
        'é' | 'è' => 'e',
        'í' | 'ì' => 'i',
        'ó' | 'ò' => 'o',
        'ú' | 'ù' | 'ü' => 'u',
        _ => c,
    }
}

fn folded(text: &str) -> Vec<char> {
    text.chars().map(fold).collect()
}

fn is_common(written: &str) -> bool {
    let word: String = written.chars().map(fold).collect();
    COMMON.iter().any(|c| c.chars().map(fold).eq(word.chars()))
}

/// `LangChain` -> `lang chain`, `Next.js` -> `next js`; `None` for a single plain word.
fn split_parts(written: &str) -> Option<String> {
    let mut parts: Vec<String> = vec![String::new()];
    let mut previous: Option<char> = None;
    for c in written.chars() {
        if !c.is_alphanumeric() {
            if !parts.last().is_some_and(String::is_empty) {
                parts.push(String::new());
            }
        } else {
            if c.is_uppercase()
                && previous.is_some_and(char::is_lowercase)
                && !parts.last().is_some_and(String::is_empty)
            {
                parts.push(String::new());
            }
            if let Some(last) = parts.last_mut() {
                last.push(c);
            }
        }
        previous = Some(c);
    }
    parts.retain(|p| !p.is_empty());
    (parts.len() > 1).then(|| parts.join(" "))
}

impl Dictionary {
    /// Parses the text format. Blank lines, `#` comments and lines with no written form are
    /// skipped.
    pub fn parse(text: &str) -> Self {
        let terms = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| {
                let (written, heard) = line.split_once(':').unwrap_or((line, ""));
                Term {
                    written: written.trim().to_owned(),
                    heard: heard
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .collect(),
                }
            })
            .filter(|term| !term.written.is_empty())
            .collect();
        Self::new(terms)
    }

    pub fn new(terms: Vec<Term>) -> Self {
        let mut variants: Vec<(Vec<char>, String)> = Vec::new();
        for term in &terms {
            let mut forms: Vec<String> = term.heard.clone();
            // R-3: the entry in any case or with accents, and split into its parts.
            if !is_common(&term.written) {
                forms.push(term.written.clone());
                forms.extend(split_parts(&term.written));
            }
            for form in forms {
                let form = folded(&form);
                if !form.is_empty() && !variants.iter().any(|(v, _)| *v == form) {
                    variants.push((form, term.written.clone()));
                }
            }
        }
        variants.sort_by_key(|(v, _)| std::cmp::Reverse(v.len()));
        let mut by_first: BTreeMap<char, Vec<usize>> = BTreeMap::new();
        for (index, (variant, _)) in variants.iter().enumerate() {
            by_first.entry(variant[0]).or_default().push(index);
        }
        Dictionary {
            terms,
            variants,
            by_first,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn terms(&self) -> &[Term] {
        &self.terms
    }

    /// Replaces each heard form in `text` with its entry (R-2): whole words or phrases only,
    /// ignoring case and accents, the longest form first, in one pass so a replaced entry is
    /// never matched again. Text that already reads exactly as the entry is left alone.
    /// Returns the new text and the number of substitutions.
    pub fn apply(&self, text: &str) -> (String, usize) {
        if self.variants.is_empty() {
            return (text.to_owned(), 0);
        }
        let chars: Vec<char> = text.chars().collect();
        let folded_text: Vec<char> = chars.iter().map(|&c| fold(c)).collect();
        let word = |c: char| c.is_alphanumeric();
        let (mut out, mut made) = (String::with_capacity(text.len()), 0);
        let mut i = 0;
        'scan: while i < chars.len() {
            let candidates = (i == 0 || !word(chars[i - 1]))
                .then(|| self.by_first.get(&folded_text[i]))
                .flatten();
            if let Some(candidates) = candidates {
                for (variant, written) in candidates.iter().map(|&k| &self.variants[k]) {
                    let end = i + variant.len();
                    let fits = end <= chars.len()
                        && folded_text[i..end] == variant[..]
                        && (end == chars.len() || !word(chars[end]));
                    if !fits {
                        continue;
                    }
                    let heard: String = chars[i..end].iter().collect();
                    if heard == *written {
                        break;
                    }
                    out.push_str(written);
                    made += 1;
                    i = end;
                    continue 'scan;
                }
            }
            out.push(chars[i]);
            i += 1;
        }
        (out, made)
    }

    /// The paragraph added to the cleanup prompt (R-4): the written forms, alphabetical, with
    /// no commas or line breaks of their own, at most [`PROMPT_CAP`]. Empty when there are no
    /// entries. The same dictionary always gives the same line, so the prompt's start stays
    /// cacheable.
    pub fn prompt_line(&self) -> String {
        let mut written: Vec<String> = self
            .terms
            .iter()
            .map(|t| {
                t.written
                    .chars()
                    .map(|c| if c == ',' || c.is_control() { ' ' } else { c })
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .filter(|w| !w.is_empty())
            .collect();
        written.sort_by_key(|w| (w.to_lowercase(), w.clone()));
        written.dedup();
        written.truncate(PROMPT_CAP);
        if written.is_empty() {
            return String::new();
        }
        format!("\n\nThe speaker's dictionary: {}.", written.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The text format, as flowbench reads it (issue 31).
    #[test]
    fn lines_give_entries_with_optional_heard_forms() {
        let d =
            Dictionary::parse("# names\nLangChain\n\nZentrix: zen trix, sun trix\n : nothing\n");
        assert_eq!(
            d.terms(),
            &[
                Term {
                    written: "LangChain".to_owned(),
                    heard: vec![]
                },
                Term {
                    written: "Zentrix".to_owned(),
                    heard: vec!["zen trix".to_owned(), "sun trix".to_owned()]
                },
            ]
        );
        assert!(Dictionary::parse("# only a comment\n").is_empty());
    }

    /// AC-1: whole words or phrases, ignoring case, in one pass; inside a longer word nothing
    /// changes.
    #[test]
    fn heard_forms_become_the_entry() {
        let d = Dictionary::parse(
            "Zentrix: zen trix, sun trix\nQuorbit: corbit\nflowbench: flow bench",
        );
        let (text, made) = d.apply("We tried Zen Trix, sun trixie and Corbit in flow bench.");
        assert_eq!(
            text,
            "We tried Zentrix, sun trixie and Quorbit in flowbench."
        );
        assert_eq!(made, 3);
        assert_eq!(d.apply("corbital").0, "corbital");
    }

    /// AC-1: accents are ignored when matching, and punctuation around a match stays.
    #[test]
    fn accents_are_ignored_and_punctuation_stays() {
        let d = Dictionary::parse("Málaga: malaga\nZentrix: sentrís");
        assert_eq!(d.apply("¿Malaga?").0, "¿Málaga?");
        assert_eq!(d.apply("abre sentris ya").0, "abre Zentrix ya");
        // ñ is not folded: año and ano stay different.
        let n = Dictionary::parse("Añón: anon");
        assert_eq!(n.apply("año").0, "año");
    }

    /// AC-1: the longest form wins, and an entry already written right is never changed.
    #[test]
    fn the_longest_form_wins_and_the_entry_stays() {
        let d = Dictionary::parse("LangChain: lang, lang chain\nZentrix: zentrix");
        let (text, made) = d.apply("lang chain and lang, Zentrix and zentrix");
        assert_eq!(text, "LangChain and LangChain, Zentrix and Zentrix");
        assert_eq!(made, 3);
        assert_eq!(
            Dictionary::default().apply("anything"),
            ("anything".to_owned(), 0)
        );
    }

    /// AC-3: an entry heard in the wrong case or split into its parts is fixed with no heard
    /// forms saved; a common word gets no such forms.
    #[test]
    fn implicit_forms_fix_case_and_split_words() {
        let d = Dictionary::parse("LangChain\nNext.js\nkubectl");
        assert_eq!(
            d.apply("langchain or lang chain with next js and Kubectl")
                .0,
            "LangChain or LangChain with Next.js and kubectl"
        );
        let common = Dictionary::parse("Mesa");
        assert_eq!(common.apply("la mesa").0, "la mesa");
        // A common-word entry still uses the heard forms the user gave it.
        let given = Dictionary::parse("Mesa: meza");
        assert_eq!(given.apply("la meza").0, "la Mesa");
    }

    /// AC-4: alphabetical, deduplicated, no commas of its own, and the same for the same
    /// dictionary.
    #[test]
    fn the_prompt_line_is_stable_and_clean() {
        let d = Dictionary::parse("zentrix\nAna Ruiz\nLangChain: lang chain\nAcme, Inc\nLangChain");
        assert_eq!(
            d.prompt_line(),
            "\n\nThe speaker's dictionary: Acme Inc, Ana Ruiz, LangChain, zentrix."
        );
        assert_eq!(Dictionary::default().prompt_line(), "");
        let many: String = (0..400).map(|i| format!("Term{i:03}\n")).collect();
        let line = Dictionary::parse(&many).prompt_line();
        assert_eq!(line.matches(", ").count() + 1, PROMPT_CAP);
    }

    /// AC-6 (rough bound; release-mode timing is slice D3): 1,000 entries over a long
    /// dictation finishes quickly even in a debug build.
    #[test]
    fn a_large_dictionary_is_fast() {
        let entries: String = (0..1000)
            .map(|i| format!("Entry{i}: heard {i} form, other {i}\n"))
            .collect();
        let d = Dictionary::parse(&entries);
        let text = "palabras de un dictado largo y normal ".repeat(300);
        let start = std::time::Instant::now();
        let (_, made) = d.apply(&text);
        assert_eq!(made, 0);
        assert!(start.elapsed().as_millis() < 200, "{:?}", start.elapsed());
    }
}
