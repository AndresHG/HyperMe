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

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

/// Limits on what the Settings page accepts (R-15), the same as Wispr Flow's and
/// Speechmatics' so an import from either fits and a later `additional_vocab` needs no new rules.
pub const MAX_ENTRIES: usize = 1000;
pub const MAX_CHARS: usize = 60;
pub const MAX_HEARD_WORDS: usize = 6;

/// The default prompt cap (R-5): up to this many entries, the prompt carries them all,
/// alphabetical, so its start stays cacheable. `dictation.dictionary_prompt_cap` changes it.
pub const PROMPT_CAP: usize = 150;

/// Above the cap, at most this many relevant entries go in the prompt (design.md, "Prompt").
pub const RELEVANT_MAX: usize = 50;

/// Above the cap, an entry is relevant when this share of the character trigrams of one of its
/// forms appears in a window of 1 to 3 words of the transcript (design.md, "Prompt").
const RELEVANT_OVERLAP: f32 = 0.5;

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
    /// How many dictations used each entry, in the order of `terms` (design.md, "Data
    /// model"). Orders the relevant entries above the prompt cap (R-5).
    uses: Vec<u32>,
    /// Folded heard forms, explicit and implicit, longest first, each with its entry's index.
    variants: Vec<(Vec<char>, usize)>,
    /// Indexes into `variants` by their first folded word, still longest first (R-6).
    by_first: HashMap<Vec<char>, Vec<usize>, Fast>,
    /// The relevance filter's index (R-5), built with the dictionary so a dictation only scans.
    grams: Box<GramIndex>,
}

/// Lower case, and `á é í ó ú ü à è ì ò ù` without their accents. `ñ` stays: `año` and `ano`
/// are different words.
fn fold(c: char) -> char {
    if c.is_ascii() {
        return c.to_ascii_lowercase();
    }
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

/// Whether a word is one of the frequent words a heard form should not be without a yes (R-17).
pub fn is_common_word(word: &str) -> bool {
    is_common(word.trim())
}

fn is_common(written: &str) -> bool {
    let word: String = written.chars().map(fold).collect();
    COMMON.iter().any(|c| c.chars().map(fold).eq(word.chars()))
}

/// The form two words are compared in: lower case, accents folded (as the matcher compares).
pub fn key(text: &str) -> String {
    text.trim().chars().map(fold).collect()
}

/// Splits CSV text into rows of cells: commas, double-quoted cells with `""` for a quote, and
/// `\n` or `\r\n` line ends.
fn csv_rows(text: &str) -> Result<Vec<Vec<String>>, String> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (quoted, c) {
            (true, '"') if chars.peek() == Some(&'"') => {
                chars.next();
                cell.push('"');
            }
            (true, '"') => quoted = false,
            (true, c) => cell.push(c),
            (false, '"') if cell.is_empty() => quoted = true,
            (false, ',') => row.push(std::mem::take(&mut cell)),
            (false, '\r') => {}
            (false, '\n') => {
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
            }
            (false, c) => cell.push(c),
        }
    }
    if quoted {
        return Err("The CSV has a quote that is never closed.".to_owned());
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    Ok(rows)
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
        let mut variants: Vec<(Vec<char>, usize)> = Vec::new();
        let mut known: HashSet<Vec<char>, Fast> = HashSet::default();
        let mut grams = Box::<GramIndex>::default();
        for (at, term) in terms.iter().enumerate() {
            for form in std::iter::once(&term.written).chain(&term.heard) {
                grams.add(at, form);
            }
            let mut forms: Vec<String> = term.heard.clone();
            // R-3: the entry in any case or with accents, and split into its parts.
            if !is_common(&term.written) {
                forms.push(term.written.clone());
                forms.extend(split_parts(&term.written));
            }
            for form in forms {
                let form = folded(&form);
                if !form.is_empty() && known.insert(form.clone()) {
                    variants.push((form, at));
                }
            }
        }
        variants.sort_by_key(|(v, _)| std::cmp::Reverse(v.len()));
        let mut by_first: HashMap<Vec<char>, Vec<usize>, Fast> = HashMap::default();
        for (index, (variant, _)) in variants.iter().enumerate() {
            by_first
                .entry(variant[..first_word(variant)].to_vec())
                .or_default()
                .push(index);
        }
        grams.finish();
        Dictionary {
            uses: vec![0; terms.len()],
            terms,
            variants,
            by_first,
            grams,
        }
    }

    /// The use count of each entry, in the order of [`Dictionary::terms`], from the store.
    /// Missing counts are 0.
    pub fn with_uses(mut self, mut uses: Vec<u32>) -> Self {
        uses.resize(self.terms.len(), 0);
        self.uses = uses;
        self
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

    /// An import (R-13): new entries go at the end, new heard forms join the entry already
    /// there, and what is there stays as written. The result must pass the check (R-15).
    pub fn merged(&self, incoming: &[Term]) -> Result<Dictionary, String> {
        let mut terms = self.terms.clone();
        for new in incoming {
            let written = new.written.trim();
            if written.is_empty() {
                continue;
            }
            let at = match terms.iter().position(|t| key(&t.written) == key(written)) {
                Some(at) => at,
                None => {
                    terms.push(Term {
                        written: written.to_owned(),
                        heard: Vec::new(),
                    });
                    terms.len() - 1
                }
            };
            for heard in new.heard.iter().map(|h| h.trim()).filter(|h| !h.is_empty()) {
                if !terms[at].heard.iter().any(|h| key(h) == key(heard)) {
                    terms[at].heard.push(heard.to_owned());
                }
            }
        }
        Self::check(&terms)?;
        Ok(Dictionary::new(terms))
    }

    /// The CSV export (R-13): a header, then the word and its heard forms, comma separated in
    /// one quoted cell.
    pub fn to_csv(&self) -> String {
        let cell = |text: &str| {
            if text.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", text.replace('"', "\"\""))
            } else {
                text.to_owned()
            }
        };
        let mut out = String::from("word,heard as\n");
        for term in &self.terms {
            out.push_str(&cell(&term.written));
            out.push(',');
            out.push_str(&cell(&term.heard.join(", ")));
            out.push('\n');
        }
        out
    }

    /// Reads a CSV (R-13): the first column is the word, the second, if any, its heard forms,
    /// comma separated. A first row that starts with "word" is a header. Blank rows are skipped.
    pub fn parse_csv(text: &str) -> Result<Vec<Term>, String> {
        let mut terms = Vec::new();
        for (n, row) in csv_rows(text)?.into_iter().enumerate() {
            let written = row.first().map(|c| c.trim()).unwrap_or_default();
            if written.is_empty() || (n == 0 && written.eq_ignore_ascii_case("word")) {
                continue;
            }
            let heard = row
                .get(1)
                .map(|c| {
                    c.split(',')
                        .map(str::trim)
                        .filter(|h| !h.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            terms.push(Term {
                written: written.to_owned(),
                heard,
            });
        }
        Ok(terms)
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
        let s = self.substitute(text);
        (s.text, s.made)
    }

    /// [`Dictionary::apply`], also naming the entries found in the text, replaced or already
    /// written right, for the prompt above the cap (R-5).
    pub fn substitute(&self, text: &str) -> Substituted {
        let mut seen = Vec::new();
        if self.variants.is_empty() {
            return Substituted {
                text: text.to_owned(),
                made: 0,
                seen,
            };
        }
        let chars: Vec<char> = text.chars().collect();
        let folded_text: Vec<char> = chars.iter().map(|&c| fold(c)).collect();
        let word = |c: char| c.is_alphanumeric();
        let (mut out, mut made) = (String::with_capacity(text.len()), 0);
        let mut i = 0;
        'scan: while i < chars.len() {
            let candidates = (i == 0 || !word(chars[i - 1]))
                .then(|| {
                    self.by_first
                        .get(&folded_text[i..i + first_word(&folded_text[i..])])
                })
                .flatten();
            if let Some(candidates) = candidates {
                for &(ref variant, at) in candidates.iter().map(|&k| &self.variants[k]) {
                    let end = i + variant.len();
                    let fits = end <= chars.len()
                        && folded_text[i..end] == variant[..]
                        && (end == chars.len() || !word(chars[end]));
                    if !fits {
                        continue;
                    }
                    if !seen.contains(&at) {
                        seen.push(at);
                    }
                    let written = &self.terms[at].written;
                    if chars[i..end].iter().copied().eq(written.chars()) {
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
        Substituted {
            text: out,
            made,
            seen,
        }
    }

    /// The paragraph added to the cleanup prompt (R-4): every written form, alphabetical, with
    /// no commas or line breaks of their own. Empty when there are no entries, and above `cap`
    /// entries, where [`Dictionary::relevant_line`] chooses per dictation (R-5). The same
    /// dictionary always gives the same line, so the prompt's start stays cacheable.
    pub fn prompt_line(&self, cap: usize) -> String {
        if self.terms.len() > cap {
            return String::new();
        }
        line((0..self.terms.len()).map(|at| &self.terms[at].written))
    }

    /// Above `cap` entries (R-5): the line for this dictation, with only the entries that look
    /// relevant to `substituted`, the transcript after [`Dictionary::substitute`]. First the
    /// entries it found (`seen`), then those with a form whose character trigrams overlap a
    /// window of 1 to 3 words of the transcript, most used first; at most [`RELEVANT_MAX`] and
    /// never more than `cap`. `None` at or below the cap: the stable line stands.
    pub fn relevant_line(&self, substituted: &Substituted, cap: usize) -> Option<String> {
        if self.terms.len() <= cap {
            return None;
        }
        let max = RELEVANT_MAX.min(cap);
        let mut chosen: Vec<usize> = substituted.seen.iter().copied().take(max).collect();
        if chosen.len() < max {
            let mut near = self.grams.near(&substituted.text);
            near.retain(|at| !chosen.contains(at));
            near.sort_by_key(|&at| (std::cmp::Reverse(self.uses[at]), at));
            chosen.extend(near.into_iter().take(max - chosen.len()));
        }
        Some(line(chosen.into_iter().map(|at| &self.terms[at].written)))
    }
}

/// What [`Dictionary::substitute`] made of a transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Substituted {
    pub text: String,
    /// Substitutions made.
    pub made: usize,
    /// The entries found, by index into [`Dictionary::terms`], in the order first found.
    pub seen: Vec<usize>,
}

/// The dictionary paragraph for these written forms: cleaned, alphabetical, deduplicated.
fn line<'a>(forms: impl Iterator<Item = &'a String>) -> String {
    let mut written: Vec<String> = forms
        .map(|w| {
            w.chars()
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
    if written.is_empty() {
        return String::new();
    }
    format!("\n\nThe speaker's dictionary: {}.", written.join(", "))
}

/// The length of the first word of folded text: its run of letters and digits, or its first
/// character when it starts with neither. The matcher's index key (R-6).
fn first_word(text: &[char]) -> usize {
    match text.iter().position(|c| !c.is_alphanumeric()) {
        Some(0) => 1.min(text.len()),
        Some(n) => n,
        None => text.len(),
    }
}

/// A small fast hasher for the matcher's and the filter's maps (the keys are short and never
/// come from the network, so the default hasher's protection against flooding buys nothing).
#[derive(Debug, Default, Clone, Copy)]
pub struct FastHasher(u64);

type Fast = BuildHasherDefault<FastHasher>;

impl Hasher for FastHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(u64::from(b));
        }
    }

    fn write_u32(&mut self, n: u32) {
        self.write_u64(u64::from(n));
    }

    fn write_usize(&mut self, n: usize) {
        self.write_u64(n as u64);
    }

    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

/// A trigram of folded characters, packed.
type Trigram = u64;

/// Calls `each` with the trigrams of every word of `text` as folded, padded with a space at
/// both ends (` ze`, `zen`, ..., `ix `), and the word's position. Allocates nothing.
fn trigrams(text: &str, mut each: impl FnMut(u32, Trigram)) {
    let pack =
        |a: char, b: char, c: char| (u64::from(a) << 42) | (u64::from(b) << 21) | u64::from(c);
    let (mut word, mut a, mut b) = (0u32, ' ', ' ');
    let mut inside = false;
    for c in text.chars() {
        if c.is_alphanumeric() {
            let c = fold(c);
            if inside {
                each(word, pack(a, b, c));
            } else {
                inside = true;
                b = ' ';
            }
            (a, b) = (b, c);
        } else if inside {
            each(word, pack(a, b, ' '));
            inside = false;
            word += 1;
        }
    }
    if inside {
        each(word, pack(a, b, ' '));
    }
}

/// For each form (written or heard) of each entry, its distinct trigrams, indexed by trigram,
/// so a dictation finds the close entries in one pass over its words (R-5, R-6).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct GramIndex {
    /// Each form's entry and how many of its trigrams a window must hold.
    forms: Vec<(u32, u8)>,
    /// Trigram to a range of `postings`.
    by_gram: HashMap<Trigram, (u32, u32), Fast>,
    /// (form, bit of that trigram in the form), grouped by trigram.
    postings: Vec<(u32, u8)>,
    /// While building: (trigram, form, bit).
    building: Vec<(Trigram, u32, u8)>,
}

impl GramIndex {
    fn add(&mut self, entry: usize, form: &str) {
        let mut mine: Vec<Trigram> = Vec::new();
        trigrams(form, |_, t| mine.push(t));
        mine.sort_unstable();
        mine.dedup();
        // A form holds at most 60 characters (R-15), so at most 60 trigrams; 64 bits each.
        mine.truncate(64);
        if mine.is_empty() {
            return;
        }
        let need = ((mine.len() as f32 * RELEVANT_OVERLAP).ceil() as u8).max(1);
        let id = u32::try_from(self.forms.len()).unwrap_or(u32::MAX);
        self.forms
            .push((u32::try_from(entry).unwrap_or(u32::MAX), need));
        for (bit, t) in mine.into_iter().enumerate() {
            self.building.push((t, id, bit as u8));
        }
    }

    fn finish(&mut self) {
        let mut building = std::mem::take(&mut self.building);
        building.sort_unstable();
        self.postings = Vec::with_capacity(building.len());
        let mut i = 0;
        while i < building.len() {
            let t = building[i].0;
            let start = self.postings.len() as u32;
            while i < building.len() && building[i].0 == t {
                self.postings.push((building[i].1, building[i].2));
                i += 1;
            }
            self.by_gram.insert(t, (start, self.postings.len() as u32));
        }
        self.by_gram.shrink_to_fit();
        self.forms.shrink_to_fit();
    }

    /// The entries with a form of which a window of 1 to 3 consecutive words of `text` holds
    /// at least [`RELEVANT_OVERLAP`] of the trigrams, in entry order.
    fn near(&self, text: &str) -> Vec<usize> {
        if self.forms.is_empty() {
            return Vec::new();
        }
        // Per form: the last word it was seen in, and which of its trigrams that word and the
        // two before it held. `need` 0 marks a form already passed.
        let mut seen: Vec<(u32, [u64; 3])> = vec![(u32::MAX, [0; 3]); self.forms.len()];
        let mut passed: Vec<usize> = Vec::new();
        trigrams(text, |word, t| {
            let Some(&(start, end)) = self.by_gram.get(&t) else {
                return;
            };
            for &(form, bit) in &self.postings[start as usize..end as usize] {
                let (last, masks) = &mut seen[form as usize];
                if *last == u32::MAX - 1 {
                    continue;
                }
                if *last != word {
                    let gap = if *last == u32::MAX { 3 } else { word - *last };
                    *masks = match gap {
                        1 => [0, masks[0], masks[1]],
                        2 => [0, 0, masks[0]],
                        _ => [0; 3],
                    };
                    *last = word;
                }
                masks[0] |= 1 << bit;
                let held = (masks[0] | masks[1] | masks[2]).count_ones();
                let (entry, need) = self.forms[form as usize];
                if held >= u32::from(need) {
                    *last = u32::MAX - 1;
                    passed.push(entry as usize);
                }
            }
        });
        passed.sort_unstable();
        passed.dedup();
        passed
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
            d.prompt_line(PROMPT_CAP),
            "\n\nThe speaker's dictionary: Acme Inc, Ana Ruiz, LangChain, zentrix."
        );
        assert_eq!(Dictionary::default().prompt_line(PROMPT_CAP), "");
    }

    /// Invented filler entries that match nothing in the transcripts below.
    fn filler(n: usize) -> String {
        (0..n).map(|i| format!("Kfz{i:03}\n")).collect()
    }

    /// AC-5 (R-5): up to the cap the line carries every entry and no dictation changes it;
    /// above it, the stable line is empty and each dictation gets only the relevant entries:
    /// the ones found in it, then the ones whose trigrams are close to a window of its words.
    #[test]
    fn above_the_cap_only_relevant_entries_go_in_the_prompt() {
        let small = Dictionary::parse("Zentrix: zen trix\nQuorbit\n");
        let said = small.substitute("the zen trix build");
        assert_eq!(small.relevant_line(&said, 2), None);
        assert!(small.prompt_line(2).contains("Quorbit, Zentrix"));

        let big = Dictionary::parse(&format!(
            "Zentrix: zen trix\nQuorbit\nAna Ruiz\nLangChain\n{}",
            filler(200)
        ));
        assert_eq!(big.prompt_line(PROMPT_CAP), "");
        let said = big.substitute("send the zen trix build and the corbit report to the team");
        assert_eq!(
            said.text,
            "send the Zentrix build and the corbit report to the team"
        );
        assert_eq!(
            big.relevant_line(&said, PROMPT_CAP).as_deref(),
            Some("\n\nThe speaker's dictionary: Quorbit, Zentrix.")
        );
        let none = big.substitute("nothing to see here");
        assert_eq!(big.relevant_line(&none, PROMPT_CAP).as_deref(), Some(""));
    }

    /// AC-5 (R-5): among the close entries the most used come first, and the line never holds
    /// more than 50 entries or more than the cap.
    #[test]
    fn relevant_entries_are_most_used_first_and_capped() {
        let d =
            Dictionary::parse("Corbitt\nQuorbit\nKorbit\nZentrix\n").with_uses(vec![1, 9, 5, 0]);
        let said = d.substitute("the corbit report");
        assert_eq!(said.seen, Vec::<usize>::new());
        assert_eq!(
            d.relevant_line(&said, 2).as_deref(),
            Some("\n\nThe speaker's dictionary: Korbit, Quorbit.")
        );

        let many: String = (0..300).map(|i| format!("Zen{i}: zen {i}\n")).collect();
        let d = Dictionary::parse(&many);
        let text: String = (0..300).map(|i| format!("zen {i} ")).collect();
        let said = d.substitute(&text);
        assert_eq!(said.seen.len(), 300);
        let count = |line: Option<String>| line.unwrap().matches(", ").count() + 1;
        assert_eq!(count(d.relevant_line(&said, PROMPT_CAP)), RELEVANT_MAX);
        assert_eq!(count(d.relevant_line(&said, 20)), 20);
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

    /// AC-10 (R-13): the CSV export reads back as the same entries; a header row, quotes and
    /// a one-column file import too.
    #[test]
    fn csv_round_trips_and_imports() {
        let d = Dictionary::parse("Zentrix: zen trix, sun trix\nLangChain\nAna \"Ani\" Ruiz\n");
        let csv = d.to_csv();
        assert!(csv.starts_with("word,heard as\n"), "{csv}");
        assert!(csv.contains("Zentrix,\"zen trix, sun trix\"\n"), "{csv}");
        assert!(csv.contains("\"Ana \"\"Ani\"\" Ruiz\",\n"), "{csv}");
        assert_eq!(Dictionary::new(Dictionary::parse_csv(&csv).unwrap()), d);

        let one_column = "Word\r\nKubernetes\r\n\r\nNext.js\r\n";
        let terms = Dictionary::parse_csv(one_column).unwrap();
        assert_eq!(
            terms.iter().map(|t| t.written.as_str()).collect::<Vec<_>>(),
            ["Kubernetes", "Next.js"]
        );
        assert!(Dictionary::parse_csv("\"open quote,x\n").is_err());
    }

    /// AC-10 (R-13, R-15): an import adds new entries and new heard forms to the ones there,
    /// keeps what is there, and refuses what breaks the limits.
    #[test]
    fn an_import_merges_into_the_list() {
        let d = Dictionary::parse("Zentrix: zen trix\nLangChain\n");
        let incoming =
            Dictionary::parse("zentrix: Zen Trix, sun trix\nKubernetes: cube net ease\n");
        let merged = d.merged(incoming.terms()).unwrap();
        assert_eq!(
            merged.to_text(),
            Dictionary::parse(
                "Zentrix: zen trix, sun trix\nLangChain\nKubernetes: cube net ease\n"
            )
            .to_text()
        );
        let many: Vec<Term> = (0..MAX_ENTRIES)
            .map(|i| Term {
                written: format!("w{i}"),
                heard: vec![],
            })
            .collect();
        assert!(merged.merged(&many).unwrap_err().contains("At most 1000"));
        assert_eq!(key("Árbol"), key("arbol"));
        assert!(is_common_word(" Mesa ") && !is_common_word("Zentrix"));
    }
}
