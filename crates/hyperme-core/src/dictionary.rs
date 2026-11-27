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

/// Limits on what the Settings page accepts (R-15), the same as Wispr Flow's and
/// Speechmatics' so an import from either fits and a later `additional_vocab` needs no new rules.
pub const MAX_ENTRIES: usize = 1000;
pub const MAX_CHARS: usize = 60;
pub const MAX_HEARD_WORDS: usize = 6;

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

/// What a fix from History made of the dictionary (R-12, R-17).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Corrected {
    /// The dictionary with the fix, to save and use from the next dictation.
    Added(Dictionary),
    /// The heard form is a common word: ask, then send it again confirmed.
    Common,
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

    /// Checks entries from the Settings page (R-15) and says what is wrong, naming the entry.
    pub fn check(terms: &[Term]) -> Result<(), String> {
        if terms.len() > MAX_ENTRIES {
            return Err(format!(
                "At most {MAX_ENTRIES} words; this list has {}.",
                terms.len()
            ));
        }
        let mut seen: Vec<Vec<char>> = Vec::with_capacity(terms.len());
        for term in terms {
            let written = term.written.trim();
            if written.is_empty() {
                return Err("A word is empty.".to_owned());
            }
            if written.contains(':') || written.contains(['\n', '\r']) {
                return Err(format!(
                    "\"{written}\" cannot contain a colon or a line break."
                ));
            }
            if written.chars().count() > MAX_CHARS {
                return Err(format!(
                    "\"{written}\" is longer than {MAX_CHARS} characters."
                ));
            }
            let key = folded(written);
            if seen.contains(&key) {
                return Err(format!("\"{written}\" is in the list twice."));
            }
            seen.push(key);
            for heard in &term.heard {
                let heard = heard.trim();
                if heard.contains([',', ':', '\n', '\r']) {
                    return Err(format!(
                        "\"{heard}\" (heard as, for {written}) cannot contain a comma, a colon or a line break."
                    ));
                }
                if heard.chars().count() > MAX_CHARS
                    || heard.split_whitespace().count() > MAX_HEARD_WORDS
                {
                    return Err(format!(
                        "\"{heard}\" (heard as, for {written}) is longer than {MAX_HEARD_WORDS} words or {MAX_CHARS} characters."
                    ));
                }
            }
        }
        Ok(())
    }

    /// A fix from History (R-12, AC-12): `heard` should have been `wanted`. The heard form joins
    /// the entry for `wanted`, made if there is none, and leaves any other entry that had it, so
    /// the fix made last wins. A heard form the entry already covers adds nothing. A common heard
    /// form needs `confirmed_common` (R-17). A result that breaks the limits is the check's
    /// message (R-15).
    pub fn corrected(
        &self,
        heard: &str,
        wanted: &str,
        confirmed_common: bool,
    ) -> Result<Corrected, String> {
        let (heard, wanted) = (heard.trim(), wanted.trim());
        if heard.is_empty() || wanted.is_empty() {
            return Err("Select a word and type what it should be.".to_owned());
        }
        if is_common(heard) && !confirmed_common {
            return Ok(Corrected::Common);
        }
        let key = folded(heard);
        let wanted_key = folded(wanted);
        let mut terms = self.terms.clone();
        let at = match terms.iter().position(|t| folded(&t.written) == wanted_key) {
            Some(at) => at,
            None => {
                terms.push(Term {
                    written: wanted.to_owned(),
                    heard: Vec::new(),
                });
                terms.len() - 1
            }
        };
        // The entry in any case is already an implicit form, unless the entry is common.
        let implicit = key == wanted_key && !is_common(wanted);
        for (i, term) in terms.iter_mut().enumerate() {
            if i != at {
                term.heard.retain(|h| folded(h) != key);
            }
        }
        let term = &mut terms[at];
        if !implicit && !term.heard.iter().any(|h| folded(h) == key) {
            term.heard.push(heard.to_owned());
        }
        Self::check(&terms)?;
        Ok(Corrected::Added(Dictionary::new(terms)))
    }

    /// The text format, as [`Dictionary::parse`] reads it, for saving.
    pub fn to_text(&self) -> String {
        let mut out = String::from(
            "# HyperMe dictionary: one word per line, then how it is heard wrong.\n\
             # Zentrix: zen trix, sun trix\n",
        );
        for term in &self.terms {
            out.push_str(&term.written);
            if !term.heard.is_empty() {
                out.push_str(": ");
                out.push_str(&term.heard.join(", "));
            }
            out.push('\n');
        }
        out
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

    /// AC-10 (R-15): entries over the limits, duplicates and characters the text format
    /// cannot hold are refused with a reason naming the entry.
    #[test]
    fn check_refuses_what_the_file_cannot_hold() {
        let term = |w: &str, h: &[&str]| Term {
            written: w.to_owned(),
            heard: h.iter().map(|s| (*s).to_owned()).collect(),
        };
        assert!(
            Dictionary::check(&[term("Zentrix", &["zen trix"]), term("LangChain", &[])]).is_ok()
        );
        let refused = |terms: &[Term], part: &str| {
            let error = Dictionary::check(terms).unwrap_err();
            assert!(error.contains(part), "{error}");
        };
        refused(&[term("", &[])], "empty");
        refused(&[term("a:b", &[])], "colon");
        refused(&[term(&"x".repeat(61), &[])], "60 characters");
        refused(&[term("Zentrix", &[]), term("zentrix", &[])], "twice");
        refused(&[term("Zentrix", &["zen, trix"])], "comma");
        refused(
            &[term("Zentrix", &["one two three four five six seven"])],
            "6 words",
        );
        let many: Vec<Term> = (0..=MAX_ENTRIES)
            .map(|i| term(&format!("w{i}"), &[]))
            .collect();
        refused(&many, "At most 1000");
    }

    /// AC-10: saving and reading the text format gives the same entries back.
    #[test]
    fn the_text_format_round_trips() {
        let d = Dictionary::parse("Zentrix: zen trix, sun trix\nLangChain\nAna Ruiz: ana ruis\n");
        let again = Dictionary::parse(&d.to_text());
        assert_eq!(again, d);
        assert!(d.to_text().starts_with("# HyperMe dictionary"));
    }

    /// AC-12 (R-12, R-17): a fix from History adds the heard form at once, to the entry it names
    /// or to a new one, and the next dictation gets the entry. A common heard form waits for a
    /// yes; a heard form moves to the entry fixed last; a fix the entry already covers adds
    /// nothing.
    #[test]
    fn a_fix_adds_the_heard_form_at_once() {
        let d = Dictionary::parse("Zentrix: zen trix\n");
        let added = |c: Corrected| match c {
            Corrected::Added(d) => d,
            Corrected::Common => panic!("asked about a common word"),
        };

        let d = added(d.corrected("sun trix", "Zentrix", false).unwrap());
        assert_eq!(d.terms()[0].heard, ["zen trix", "sun trix"]);
        assert_eq!(d.apply("I use sun trix daily").0, "I use Zentrix daily");

        let d = added(d.corrected("cloud", "Claude", false).unwrap());
        assert_eq!(d.terms().len(), 2);
        assert_eq!(d.apply("Ask cloud.").0, "Ask Claude.");

        // Already covered: the same heard form again, or only the entry's own case.
        assert_eq!(added(d.corrected("Cloud", "claude", false).unwrap()), d);
        let d = added(d.corrected("langchain", "LangChain", false).unwrap());
        assert_eq!(d.terms()[2].heard, Vec::<String>::new());

        // R-17: a common heard form is asked about, then added once confirmed.
        assert_eq!(d.corrected("mesa", "Mesa", false), Ok(Corrected::Common));
        assert_eq!(
            d.corrected("Sun", "Zentrix", false)
                .map(|c| c != Corrected::Common),
            Ok(true)
        );
        assert_eq!(d.corrected("the", "Thé", false), Ok(Corrected::Common));
        let d = added(d.corrected("the", "Thé", true).unwrap());
        assert_eq!(d.apply("the cup").0, "Thé cup");

        // A heard form fixed to another entry moves there.
        let d = added(d.corrected("sun trix", "Suntrix", false).unwrap());
        assert_eq!(d.terms()[0].heard, ["zen trix"]);
        assert_eq!(d.apply("sun trix").0, "Suntrix");

        // Hard refusals are the check's messages.
        assert!(d.corrected(" ", "Zentrix", false).is_err());
        assert!(
            d.corrected("zen", "Zen: trix", false)
                .unwrap_err()
                .contains("colon")
        );
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
