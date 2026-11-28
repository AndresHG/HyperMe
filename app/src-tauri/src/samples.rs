//! Samples (spec 267): History's **Correct** keeps a dictation the user corrected, with its
//! audio, as ground truth for the quality harness. Different from Fix a word, which teaches the
//! dictionary (R-8).
//!
//! [`correct`] is pure over the `Samples` trait and tested on every platform; the commands are
//! macOS only, like the rest of Home.

use hyperme_core::samples::{SampleFacts, Samples};
use hyperme_platform::history::{BenchmarkTexts, Item};
use hyperme_platform::trace_store::Labels;
use serde::Serialize;

/// What Correct did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum CorrectOutcome {
    /// The first sample: show the sheet, then send again agreed (R-6). Nothing was written.
    NeedsAgreement,
    /// Saved. `changed` when the row's text should become `text` (R-5, GT-3).
    Saved { text: String, changed: bool },
}

/// The same text apart from spacing (R-5).
fn same_text(a: &str, b: &str) -> bool {
    a.split_whitespace().eq(b.split_whitespace())
}

fn named(provider: &str, model: &str) -> Option<String> {
    match (provider.is_empty(), model.is_empty()) {
        (true, _) => None,
        (false, true) => Some(provider.to_owned()),
        (false, false) => Some(format!("{provider}/{model}")),
    }
}

/// Writes the sample of `item` with the user's `corrected` text (R-4 to R-7, R-9, R-10).
/// `labels` are its trace's provider names, when the trace is still there; `wav` reads the
/// history's audio file. Never touches the dictionary (R-8).
pub fn correct(
    samples: &mut dyn Samples,
    item: &Item,
    texts: Option<BenchmarkTexts>,
    labels: Option<&Labels>,
    wav: impl FnOnce() -> Result<Vec<u8>, String>,
    corrected: &str,
    agreed: bool,
) -> Result<CorrectOutcome, String> {
    let corrected = corrected.trim();
    if corrected.is_empty() {
        return Err("Type the text as it should have been".to_owned());
    }
    if item.state != "done" || !item.has_audio {
        return Err("This dictation has no audio to keep".to_owned());
    }
    if !samples.agreed() {
        if !agreed {
            return Ok(CorrectOutcome::NeedsAgreement);
        }
        samples.agree().map_err(|e| e.message)?;
    }
    let texts = texts.unwrap_or(BenchmarkTexts {
        transcript: None,
        first_text: None,
    });
    let facts = SampleFacts {
        history_id: item.id,
        dictated_ms: item.started_ms,
        language: item.language.clone(),
        app: item.app.clone(),
        cleanup_level: labels
            .map(|l| l.cleanup_level.clone())
            .filter(|l| !l.is_empty()),
        speech: labels.and_then(|l| named(&l.speech_provider, &l.speech_model)),
        cleanup: labels.and_then(|l| named(&l.cleanup_provider, &l.cleanup_model)),
        transcript: texts.transcript,
        first_text: texts.first_text,
    };
    samples
        .save(&facts, &wav()?, corrected)
        .map_err(|e| e.message)?;
    let changed = !same_text(item.text.as_deref().unwrap_or_default(), corrected);
    Ok(CorrectOutcome::Saved {
        text: corrected.to_owned(),
        changed,
    })
}

#[cfg(target_os = "macos")]
pub use commands::*;

#[cfg(target_os = "macos")]
mod commands {
    use std::path::PathBuf;
    use std::sync::{Mutex, PoisonError};

    use hyperme_core::samples::Samples;
    use hyperme_core::traits::{DictationId, History};
    use hyperme_platform::sample_store::SampleStore;
    use serde::Serialize;
    use tauri::{AppHandle, Manager};

    use super::{CorrectOutcome, correct};

    /// The store, opened at the first use. One writer: every command goes through this lock.
    #[derive(Default)]
    pub struct SamplesState(Mutex<Option<SampleStore>>);

    fn folder(app: &AppHandle) -> PathBuf {
        crate::runtime::data_dir(app).join("samples")
    }

    fn with_store<T>(
        app: &AppHandle,
        f: impl FnOnce(&mut SampleStore) -> Result<T, String>,
    ) -> Result<T, String> {
        let state = app.state::<SamplesState>();
        let mut guard = state.0.lock().unwrap_or_else(PoisonError::into_inner);
        if guard.is_none() {
            *guard = Some(SampleStore::open(&folder(app)).map_err(|e| e.message)?);
        }
        f(guard.as_mut().expect("opened above"))
    }

    /// Registers the state. Called once at setup.
    pub fn init(app: &AppHandle) {
        app.manage(SamplesState::default());
    }

    /// Correct (R-3 to R-7): saves the sample, then shows the corrected text in the row.
    /// Async so the copy runs off the main thread (R-17).
    #[tauri::command]
    #[specta::specta]
    pub async fn home_correct(
        app: AppHandle,
        id: DictationId,
        text: String,
        agreed: bool,
    ) -> Result<CorrectOutcome, String> {
        let (item, texts) =
            crate::runtime::with_history(&app, |h| Ok((h.item(id)?, h.benchmark_texts(id)?)))?;
        let item = item.ok_or_else(|| "This dictation is gone".to_owned())?;
        let labels = crate::runtime::trace_store(&app)
            .and_then(|store| store.trace_for(id).ok().flatten())
            .map(|trace| trace.labels);
        let wav = || crate::runtime::with_history(&app, |h| h.audio_wav(id));
        let outcome = with_store(&app, |store| {
            correct(store, &item, texts, labels.as_ref(), wav, &text, agreed)
        })?;
        if let CorrectOutcome::Saved {
            text,
            changed: true,
        } = &outcome
        {
            crate::runtime::with_history_mut(&app, |h| {
                h.replace_text(id, text, item.language.as_deref())
            })?;
        }
        Ok(outcome)
    }

    /// Delete sample on a row (R-12).
    #[tauri::command]
    #[specta::specta]
    pub fn home_delete_sample(app: AppHandle, id: DictationId) -> Result<(), String> {
        with_store(&app, |store| store.delete(id).map_err(|e| e.message))
    }

    /// Which of these history items have a sample, for the sample mark (R-5).
    #[tauri::command]
    #[specta::specta]
    pub fn home_samples_of(
        app: AppHandle,
        ids: Vec<DictationId>,
    ) -> Result<Vec<DictationId>, String> {
        with_store(&app, |store| {
            Ok(ids.into_iter().filter(|id| store.has(*id)).collect())
        })
    }

    /// The samples row in Settings → Privacy (R-12).
    #[derive(Debug, Clone, Serialize, specta::Type)]
    pub struct SamplesView {
        pub count: usize,
        pub bytes: u64,
        pub agreed: bool,
    }

    #[tauri::command]
    #[specta::specta]
    pub fn samples_view(app: AppHandle) -> Result<SamplesView, String> {
        with_store(&app, |store| {
            let (count, bytes) = store.summary().map_err(|e| e.message)?;
            Ok(SamplesView {
                count,
                bytes,
                agreed: store.agreed(),
            })
        })
    }

    /// Delete all samples (R-12); the page asks once before calling.
    #[tauri::command]
    #[specta::specta]
    pub fn samples_delete_all(app: AppHandle) -> Result<(), String> {
        with_store(&app, |store| store.delete_all().map_err(|e| e.message))
    }

    /// Show in Finder (R-12).
    #[tauri::command]
    #[specta::specta]
    pub fn samples_reveal(app: AppHandle) -> Result<(), String> {
        let dir = folder(&app);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::process::Command::new("open")
            .arg(&dir)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use hyperme_core::samples::{SampleFacts, Samples};
    use hyperme_core::traits::{DictationId, Error};
    use hyperme_platform::history::{BenchmarkTexts, Item};
    use hyperme_platform::trace_store::Labels;

    use super::{CorrectOutcome, correct};

    #[derive(Default)]
    struct FakeSamples {
        agreed: bool,
        saved: BTreeMap<DictationId, (SampleFacts, Vec<u8>, String)>,
    }

    impl Samples for FakeSamples {
        fn save(&mut self, facts: &SampleFacts, wav: &[u8], corrected: &str) -> Result<(), Error> {
            self.saved.insert(
                facts.history_id,
                (facts.clone(), wav.to_vec(), corrected.to_owned()),
            );
            Ok(())
        }
        fn delete(&mut self, id: DictationId) -> Result<(), Error> {
            self.saved.remove(&id);
            Ok(())
        }
        fn delete_all(&mut self) -> Result<(), Error> {
            self.saved.clear();
            Ok(())
        }
        fn has(&self, id: DictationId) -> bool {
            self.saved.contains_key(&id)
        }
        fn summary(&self) -> Result<(usize, u64), Error> {
            Ok((self.saved.len(), 0))
        }
        fn agreed(&self) -> bool {
            self.agreed
        }
        fn agree(&mut self) -> Result<(), Error> {
            self.agreed = true;
            Ok(())
        }
    }

    fn item(has_audio: bool) -> Item {
        Item {
            id: 5,
            started_ms: 1_000,
            app: Some("Slack".to_owned()),
            language: Some("en".to_owned()),
            text: Some("Send the sun trix report.".to_owned()),
            state: "done".to_owned(),
            audio_bytes: 4_844,
            has_audio,
            message: None,
        }
    }

    fn texts() -> Option<BenchmarkTexts> {
        Some(BenchmarkTexts {
            transcript: Some("send the sun trix report".to_owned()),
            first_text: Some("Send the sun trix report.".to_owned()),
        })
    }

    fn labels() -> Labels {
        Labels {
            speech_provider: "speechmatics".to_owned(),
            speech_model: "enhanced".to_owned(),
            cleanup_provider: "groq".to_owned(),
            cleanup_model: String::new(),
            cleanup_level: "light".to_owned(),
        }
    }

    fn wav() -> Result<Vec<u8>, String> {
        Ok(b"RIFF wav".to_vec())
    }

    /// AC-2 (R-4, R-5, R-7, R-10): a correction writes the sample with its facts and the row's
    /// new text; correcting again replaces it; the same text still saves and changes nothing.
    #[test]
    fn a_correction_saves_a_sample() {
        let mut samples = FakeSamples {
            agreed: true,
            ..FakeSamples::default()
        };
        let outcome = correct(
            &mut samples,
            &item(true),
            texts(),
            Some(&labels()),
            wav,
            "  Send the Zentrix report. ",
            false,
        )
        .unwrap();
        assert_eq!(
            outcome,
            CorrectOutcome::Saved {
                text: "Send the Zentrix report.".to_owned(),
                changed: true
            }
        );
        let (facts, audio, corrected) = &samples.saved[&5];
        assert_eq!(corrected, "Send the Zentrix report.");
        assert_eq!(audio, b"RIFF wav");
        assert_eq!(facts.app.as_deref(), Some("Slack"));
        assert_eq!(facts.speech.as_deref(), Some("speechmatics/enhanced"));
        assert_eq!(facts.cleanup.as_deref(), Some("groq"));
        assert_eq!(facts.cleanup_level.as_deref(), Some("light"));
        assert_eq!(
            facts.transcript.as_deref(),
            Some("send the sun trix report")
        );
        assert_eq!(
            facts.first_text.as_deref(),
            Some("Send the sun trix report.")
        );

        let again = correct(
            &mut samples,
            &item(true),
            texts(),
            None,
            wav,
            "Send the  sun trix report.",
            false,
        )
        .unwrap();
        assert!(matches!(
            again,
            CorrectOutcome::Saved { changed: false, .. }
        ));
        assert_eq!(samples.saved.len(), 1);
        assert_eq!(samples.saved[&5].0.speech, None, "no trace, no names");
    }

    /// AC-2 (R-3): no audio, a failed row or an empty text saves nothing.
    #[test]
    fn nothing_is_saved_without_audio_or_text() {
        let mut samples = FakeSamples {
            agreed: true,
            ..FakeSamples::default()
        };
        assert!(
            correct(
                &mut samples,
                &item(false),
                texts(),
                None,
                wav,
                "Fixed.",
                false
            )
            .is_err()
        );
        let mut failed = item(true);
        failed.state = "failed".to_owned();
        assert!(correct(&mut samples, &failed, texts(), None, wav, "Fixed.", false).is_err());
        assert!(correct(&mut samples, &item(true), texts(), None, wav, "   ", false).is_err());
        assert!(samples.saved.is_empty());
    }

    /// AC-3 (R-6): before the first sample nothing is written until the user agrees; Not now
    /// (no second call) leaves nothing; once agreed, it is not asked again.
    #[test]
    fn the_first_sample_asks_first() {
        let mut samples = FakeSamples::default();
        let asked = correct(
            &mut samples,
            &item(true),
            texts(),
            None,
            wav,
            "Fixed.",
            false,
        );
        assert_eq!(asked, Ok(CorrectOutcome::NeedsAgreement));
        assert!(samples.saved.is_empty() && !samples.agreed);

        let saved = correct(
            &mut samples,
            &item(true),
            texts(),
            None,
            wav,
            "Fixed.",
            true,
        );
        assert!(matches!(saved, Ok(CorrectOutcome::Saved { .. })));
        assert!(samples.agreed);
        let next = correct(
            &mut samples,
            &item(true),
            texts(),
            None,
            wav,
            "Again.",
            false,
        );
        assert!(matches!(next, Ok(CorrectOutcome::Saved { .. })));
    }
}
