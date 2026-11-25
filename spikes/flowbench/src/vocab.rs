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
//! lists how it sounds. Speechmatics takes both as `additional_vocab`; the cleanup gets the
//! written forms.

use std::path::Path;

use anyhow::{Context, Result};
use serde_json::{Value, json};

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
}
