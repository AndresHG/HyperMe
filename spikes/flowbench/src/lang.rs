//! A small Spanish-or-English check for `lang_ok`.
//!
//! The live speech model reports no detected language, so the tool checks the text itself.
//! The test has only two languages, so counting common words is enough. This is not general
//! language identification.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Es,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::Es => "es",
            Lang::En => "en",
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        match code {
            "es" => Some(Lang::Es),
            "en" => Some(Lang::En),
            _ => None,
        }
    }
}

// Words spelled the same in both languages ("no", "me", "a", "he", "has") are left out.
const ES_WORDS: &[&str] = &[
    "el", "la", "los", "las", "de", "del", "que", "y", "en", "un", "una", "es", "por", "con",
    "para", "se", "lo", "le", "mi", "su", "al", "como", "pero", "ya", "este", "esta", "esto",
    "eso", "porque", "cuando", "muy", "sin", "sobre", "hasta", "hay", "donde", "desde", "nos",
    "ni", "otro", "otra", "tu", "te", "estoy", "tengo", "tiene", "tenemos", "hacer", "hace",
    "quiero", "necesito", "vamos", "ella", "ellos", "nosotros", "mucho", "poco", "bien", "ahora",
    "luego", "entonces", "gracias", "hola", "puedes", "puedo", "son", "fue", "ser", "estar", "hoy",
    "todos", "cada", "tres", "dos", "uno", "primero", "segundo",
];

const EN_WORDS: &[&str] = &[
    "the", "of", "and", "to", "in", "is", "it", "you", "that", "was", "for", "on", "are", "with",
    "as", "i", "they", "be", "at", "have", "this", "from", "or", "by", "not", "but", "what", "all",
    "were", "we", "when", "your", "can", "there", "an", "which", "do", "how", "their", "if",
    "will", "about", "then", "them", "these", "so", "some", "would", "like", "into", "more", "my",
    "than", "been", "who", "now", "did", "get", "want", "need", "please", "should", "could",
    "also", "just", "let", "make", "sure", "thanks", "hello", "today", "tomorrow", "going", "one",
    "two", "three", "first", "second", "each",
];

/// The language of `text`, or `None` when the evidence is missing or tied.
pub fn detect(text: &str) -> Option<Lang> {
    let lower = text.to_lowercase();
    let mut es = lower.matches(['¿', '¡']).count();
    let mut en = 0;
    for word in lower
        .split(|c: char| !c.is_alphabetic())
        .filter(|w| !w.is_empty())
    {
        if ES_WORDS.contains(&word) || word.contains(['ñ', 'á', 'é', 'í', 'ó', 'ú', 'ü']) {
            es += 1;
        } else if EN_WORDS.contains(&word) {
            en += 1;
        }
    }
    match es.cmp(&en) {
        std::cmp::Ordering::Greater => Some(Lang::Es),
        std::cmp::Ordering::Less => Some(Lang::En),
        std::cmp::Ordering::Equal => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_spanish() {
        let text = "Vale, necesito que revises el informe y me digas si hay algo que cambiar.";
        assert_eq!(detect(text), Some(Lang::Es));
    }

    #[test]
    fn detects_english() {
        let text = "Okay, I need you to review the report and tell me if anything should change.";
        assert_eq!(detect(text), Some(Lang::En));
    }

    #[test]
    fn spanish_with_english_product_names_is_still_spanish() {
        let text = "Abre un ticket en Linear para el bug del login y avisa por Slack.";
        assert_eq!(detect(text), Some(Lang::Es));
    }

    #[test]
    fn english_with_a_spanish_name_is_still_english() {
        let text = "Send the summary to José and ask if we can meet tomorrow.";
        assert_eq!(detect(text), Some(Lang::En));
    }

    #[test]
    fn no_evidence_gives_none() {
        assert_eq!(detect(""), None);
        assert_eq!(detect("OK 42"), None);
    }

    #[test]
    fn a_translation_is_caught() {
        // Spoken in Spanish, returned in English: lang_ok must be false for an `es` sample.
        let returned = "I want to book a table for two at nine.";
        assert_eq!(detect(returned), Some(Lang::En));
    }
}
