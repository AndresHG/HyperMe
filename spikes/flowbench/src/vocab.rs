//! The user's dictionary: names and terms the speech engine cannot guess, such as internal
//! product names. One entry per line in a text file:
//!
//! ```text
//! # comment
//! LangChain
//! Zentrix: zen trix, sun trix
//! ```
//!
//! The part before `:` is how the term is written. The optional comma-separated part after it
//! lists how it sounds. Speechmatics takes both as `additional_vocab` (single-language models
//! only); the cleanup gets the written forms. Before the cleanup, [`Replacing`] swaps each
//! sound-alike in the raw transcript for the written term.

use std::path::Path;

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::cleanup::{CleanupProvider, Piece};

#[derive(Debug, Clone, PartialEq)]
pub struct Term {
    pub written: String,
    pub sounds_like: Vec<String>,
}

pub fn parse(text: &str) -> Vec<Term> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (written, sounds) = line.split_once(':').unwrap_or((line, ""));
            Term {
                written: written.trim().to_owned(),
                sounds_like: sounds
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect(),
            }
        })
        .filter(|term| !term.written.is_empty())
        .collect()
}

pub fn load(path: &Path) -> Result<Vec<Term>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading the dictionary {}", path.display()))?;
    Ok(parse(&text))
}

/// Speechmatics `additional_vocab`.
pub fn speechmatics(terms: &[Term]) -> Value {
    json!(
        terms
            .iter()
            .map(|t| if t.sounds_like.is_empty() {
                json!({"content": t.written})
            } else {
                json!({"content": t.written, "sounds_like": t.sounds_like})
            })
            .collect::<Vec<_>>()
    )
}

/// The line added to the cleanup prompt. Empty when there are no terms.
pub fn prompt_line(terms: &[Term]) -> String {
    if terms.is_empty() {
        return String::new();
    }
    let list = terms
        .iter()
        .map(|t| t.written.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "\n\nThe speaker's dictionary: {list}. When the transcript has a word that sounds like \
         one of these, write the dictionary spelling."
    )
}

/// Replaces each `sounds_like` variant in `text` with its written term: case-insensitive,
/// whole words or phrases only, the longest variant first, one pass so a replaced term is never
/// matched again. Text that already reads as the written term is left alone. Returns the new
/// text and each substitution made, as (what was heard, the term).
pub fn replace(text: &str, terms: &[Term]) -> (String, Vec<(String, String)>) {
    let mut variants: Vec<(Vec<char>, &str)> = terms
        .iter()
        .flat_map(|t| {
            t.sounds_like
                .iter()
                .map(move |v| (v.chars().collect(), t.written.as_str()))
        })
        .filter(|(v, _): &(Vec<char>, &str)| !v.is_empty())
        .collect();
    variants.sort_by_key(|(v, _)| std::cmp::Reverse(v.len()));
    let chars: Vec<char> = text.chars().collect();
    let word = |c: char| c.is_alphanumeric();
    let same = |a: char, b: char| a.to_lowercase().eq(b.to_lowercase());
    let (mut out, mut made) = (String::with_capacity(text.len()), Vec::new());
    let mut i = 0;
    'scan: while i < chars.len() {
        if i == 0 || !word(chars[i - 1]) {
            for (variant, written) in &variants {
                let end = i + variant.len();
                let fits = end <= chars.len()
                    && chars[i..end].iter().zip(variant).all(|(&a, &b)| same(a, b))
                    && (end == chars.len() || !word(chars[end]));
                if !fits {
                    continue;
                }
                let heard: String = chars[i..end].iter().collect();
                if heard == *written {
                    break;
                }
                out.push_str(written);
                made.push((heard, (*written).to_owned()));
                i = end;
                continue 'scan;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    (out, made)
}

/// The stage note for one sample: how many substitutions, and which.
pub fn note(made: &[(String, String)]) -> String {
    let count = match made.len() {
        1 => "1 substitution".to_owned(),
        n => format!("{n} substitutions"),
    };
    if made.is_empty() {
        return format!("    [vocab] {count}");
    }
    let list = made
        .iter()
        .map(|(heard, term)| format!("{heard} -> {term}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("    [vocab] {count}: {list}")
}

/// A cleanup that first replaces the dictionary's sound-alikes in the raw transcript. With no
/// terms (no `--vocab`, or `--vocab-replace off`) it passes the transcript through unchanged.
pub struct Replacing<C> {
    pub inner: C,
    pub terms: Vec<Term>,
}

impl<C: CleanupProvider> CleanupProvider for Replacing<C> {
    async fn warm(&self) -> Result<()> {
        self.inner.warm().await
    }

    async fn clean(&self, raw: &str, on_piece: &mut dyn FnMut(Piece<'_>)) -> Result<()> {
        if self.terms.is_empty() {
            return self.inner.clean(raw, on_piece).await;
        }
        let (text, made) = replace(raw, &self.terms);
        println!("{}", note(&made));
        self.inner.clean(&text, on_piece).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Issue 31: AC-1 (file format), AC-2 (Speechmatics shape), AC-3 (prompt line).
    #[test]
    fn lines_give_terms_with_optional_sounds() {
        let terms = parse("# names\nLangChain\n\nZentrix: zen trix, sun trix\n : nothing\n");
        assert_eq!(
            terms,
            vec![
                Term {
                    written: "LangChain".to_owned(),
                    sounds_like: vec![]
                },
                Term {
                    written: "Zentrix".to_owned(),
                    sounds_like: vec!["zen trix".to_owned(), "sun trix".to_owned()]
                },
            ]
        );
        assert_eq!(
            speechmatics(&terms),
            json!([{"content": "LangChain"},
                   {"content": "Zentrix", "sounds_like": ["zen trix", "sun trix"]}])
        );
        assert!(prompt_line(&terms).contains("dictionary: LangChain, Zentrix."));
        assert_eq!(prompt_line(&[]), "");
    }

    /// Issue 34, AC-1: case-insensitive, whole words or phrases only.
    #[test]
    fn sound_alikes_become_the_term() {
        let terms =
            parse("Zentrix: zen trix, sun trix\nQuorbit: corbit\nflowbench: flow bench");
        let (text, made) = replace(
            "We tried Zen Trix, sun trixie and Corbit in flow bench.",
            &terms,
        );
        assert_eq!(
            text,
            "We tried Zentrix, sun trixie and Quorbit in flowbench."
        );
        assert_eq!(
            made,
            vec![
                ("Zen Trix".to_owned(), "Zentrix".to_owned()),
                ("Corbit".to_owned(), "Quorbit".to_owned()),
                ("flow bench".to_owned(), "flowbench".to_owned()),
            ]
        );
        // Inside a longer word nothing changes.
        assert_eq!(replace("corbital", &terms).0, "corbital");
        // Accented terms and punctuation around the match.
        let accent = parse("Málaga: malaga");
        assert_eq!(replace("¿Malaga?", &accent).0, "¿Málaga?");
    }

    /// Issue 34, AC-1: the longest variant wins, and the written term is never changed.
    #[test]
    fn the_longest_variant_wins_and_the_term_stays() {
        let terms = parse("LangChain: lang, lang chain\nZentrix: zentrix");
        let (text, made) = replace("lang chain and lang, Zentrix and zentrix", &terms);
        assert_eq!(text, "LangChain and LangChain, Zentrix and Zentrix");
        assert_eq!(made.len(), 3);
        // A term with no sound-alikes, or no terms at all, changes nothing.
        assert_eq!(replace("lang chain", &parse("LangChain")).1.len(), 0);
        assert_eq!(replace("anything", &[]).0, "anything");
    }

    /// Issue 34, AC-2: the stage note says how many substitutions were made, and which.
    #[test]
    fn the_note_counts_substitutions() {
        let made = vec![("zen trix".to_owned(), "Zentrix".to_owned())];
        assert_eq!(
            note(&made),
            "    [vocab] 1 substitution: zen trix -> Zentrix"
        );
        assert_eq!(note(&[]), "    [vocab] 0 substitutions");
    }

    /// A cleanup that records the text it was given.
    struct Recorder(std::sync::Mutex<Vec<String>>);

    impl CleanupProvider for Recorder {
        async fn warm(&self) -> Result<()> {
            Ok(())
        }

        async fn clean(&self, raw: &str, on_piece: &mut dyn FnMut(Piece<'_>)) -> Result<()> {
            self.0.lock().unwrap().push(raw.to_owned());
            on_piece(Piece::Text(raw));
            Ok(())
        }
    }

    /// Issue 34, AC-1 and AC-4: the step runs on the raw transcript before the cleanup, and
    /// with no terms (`--vocab-replace off`) the cleanup gets the transcript unchanged.
    #[tokio::test]
    async fn the_cleanup_gets_the_replaced_transcript() {
        let on = Replacing {
            inner: Recorder(Default::default()),
            terms: parse("Zentrix: zen trix"),
        };
        on.clean("run zen trix now", &mut |_| {}).await.unwrap();
        assert_eq!(
            *on.inner.0.lock().unwrap(),
            vec!["run Zentrix now".to_owned()]
        );

        let off = Replacing {
            inner: Recorder(Default::default()),
            terms: vec![],
        };
        off.clean("run zen trix now", &mut |_| {}).await.unwrap();
        assert_eq!(
            *off.inner.0.lock().unwrap(),
            vec!["run zen trix now".to_owned()]
        );
    }
}
