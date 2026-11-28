//! Samples (spec 267): dictations the user corrected in History, kept with their audio as
//! ground truth for the quality harness. The core holds the facts and the trait; the folder
//! lives in `hyperme_platform::sample_store`.

use crate::traits::{DictationId, Error};

/// What a sample records besides its audio and the corrected text (R-10).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SampleFacts {
    pub history_id: DictationId,
    /// When the dictation started, in milliseconds since the Unix epoch.
    pub dictated_ms: i64,
    pub language: Option<String>,
    /// The app it went into, for the harness's tone per app.
    pub app: Option<String>,
    pub cleanup_level: Option<String>,
    /// `provider/model` of the speech and cleanup that made it, from its trace.
    pub speech: Option<String>,
    pub cleanup: Option<String>,
    /// The speech provider's transcript before the dictionary.
    pub transcript: Option<String>,
    /// The text first pasted or copied.
    pub first_text: Option<String>,
}

/// The samples on disk. One sample per dictation: saving again replaces its corrected text.
pub trait Samples: Send {
    /// Writes or replaces the sample of `facts.history_id`. `audio_wav` is the history's WAV.
    fn save(&mut self, facts: &SampleFacts, audio_wav: &[u8], corrected: &str)
    -> Result<(), Error>;
    fn delete(&mut self, history_id: DictationId) -> Result<(), Error>;
    fn delete_all(&mut self) -> Result<(), Error>;
    fn has(&self, history_id: DictationId) -> bool;
    /// How many samples there are and the bytes they take (R-12).
    fn summary(&self) -> Result<(usize, u64), Error>;
    /// The user agreed to keep samples (R-6). Asked before the first sample is written.
    fn agreed(&self) -> bool;
    fn agree(&mut self) -> Result<(), Error>;
}

/// A sample's name in a harness set (R-9): `es-h12`, `en-h12`, or `xx-h12` when the language is
/// not known. The prefix is the language the harness groups by.
pub fn sample_name(history_id: DictationId, language: Option<&str>) -> String {
    let lang = match language.map(str::to_lowercase) {
        Some(code) if code.starts_with("es") => "es",
        Some(code) if code.starts_with("en") => "en",
        _ => "xx",
    };
    format!("{lang}-h{history_id}")
}

#[cfg(test)]
mod tests {
    use super::sample_name;

    /// R-9.
    #[test]
    fn names_carry_the_language() {
        assert_eq!(sample_name(12, Some("es")), "es-h12");
        assert_eq!(sample_name(12, Some("en-US")), "en-h12");
        assert_eq!(sample_name(3, None), "xx-h3");
        assert_eq!(sample_name(3, Some("fr")), "xx-h3");
    }
}
