//! Offline stand-ins for the speech and cleanup providers.
//!
//! They return canned text after fixed delays, so the whole run can be shown and tested without
//! keys or network. Their timings say nothing about any real provider.

use std::time::Duration;

use anyhow::Result;
use tokio::time::sleep;

use crate::audio::{Clip, SAMPLE_RATE};
use crate::cleanup::CleanupProvider;
use crate::stt::{LiveSession, SpeechProvider, Transcript};

const ES_LINES: &[&str] = &[
    "eh hola equipo, mañana tenemos la revisión del proyecto y necesito que cada uno traiga sus números",
    "um apunta tres cosas para la reunión: primero el presupuesto, segundo las fechas y tercero quién se encarga de la demo",
    "abre un ticket para el fallo del inicio de sesión y avisa al equipo cuando esté listo",
];

const EN_LINES: &[&str] = &[
    "um hello team, tomorrow we have the project review and I need each of you to bring your numbers",
    "uh write down three things for the meeting: first the budget, second the dates and third who is doing the demo",
    "this one misses the deadline on purpose, so you can see the raw transcript used as the fallback",
];

const FILLERS: &[&str] = &["um", "uh", "eh"];
/// A transcript with this word makes the fake cleanup stall, to show the deadline fallback.
const STALL_WORD: &str = "deadline";

/// Six short silent clips, three per language, so a fake run takes seconds.
pub fn clips() -> Vec<Clip> {
    ["es-01", "es-02", "es-03", "en-01", "en-02", "en-03"]
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let millis = 600 + 100 * i;
            Clip::from_pcm(name, vec![0; SAMPLE_RATE as usize * millis / 1000])
        })
        .collect()
}

/// The canned transcript for a label such as `es-02`.
fn line_for(label: &str) -> &'static str {
    let lines = if label.starts_with("es-") {
        ES_LINES
    } else {
        EN_LINES
    };
    let digits: String = label.chars().filter(char::is_ascii_digit).collect();
    let number: usize = digits.parse().unwrap_or(1);
    lines[number.saturating_sub(1) % lines.len()]
}

/// A small spread, fixed per label, so the percentiles have something to show.
fn spread(label: &str) -> Duration {
    let sum: u64 = label.bytes().map(u64::from).sum();
    Duration::from_millis(sum % 9 * 15)
}

pub struct FakeSpeech;

impl FakeSpeech {
    pub const OPEN: Duration = Duration::from_millis(110);

    pub fn file_delay(label: &str) -> Duration {
        Duration::from_millis(520) + spread(label)
    }

    pub fn final_delay(label: &str) -> Duration {
        Duration::from_millis(150) + spread(label)
    }
}

impl SpeechProvider for FakeSpeech {
    type Live = FakeLive;

    async fn transcribe_file(&self, label: &str, _wav: Vec<u8>) -> Result<Transcript> {
        sleep(Self::file_delay(label)).await;
        Ok(transcript(label))
    }

    async fn open_live(&self, label: &str) -> Result<FakeLive> {
        sleep(Self::OPEN).await;
        Ok(FakeLive {
            label: label.to_owned(),
        })
    }
}

pub struct FakeLive {
    label: String,
}

impl LiveSession for FakeLive {
    async fn send_audio(&mut self, _pcm: &[i16]) -> Result<()> {
        Ok(())
    }

    async fn finish(self) -> Result<Transcript> {
        sleep(FakeSpeech::final_delay(&self.label)).await;
        Ok(transcript(&self.label))
    }
}

fn transcript(label: &str) -> Transcript {
    Transcript {
        text: line_for(label).to_owned(),
        languages: vec![],
    }
}

pub struct FakeCleanup;

impl FakeCleanup {
    pub const PER_WORD: Duration = Duration::from_millis(6);

    pub fn first_token_delay(raw: &str) -> Duration {
        Duration::from_millis(180 + (raw.len() as u64 % 7) * 10)
    }

    /// Drops filler words, capitalises the start and closes the sentence.
    pub fn cleaned(raw: &str) -> String {
        let words: Vec<&str> = raw
            .split_whitespace()
            .filter(|word| !FILLERS.contains(&word.to_lowercase().as_str()))
            .collect();
        let mut text = words.join(" ");
        if let Some(first) = text.chars().next() {
            text.replace_range(..first.len_utf8(), &first.to_uppercase().to_string());
        }
        if !text.ends_with(['.', '?', '!']) {
            text.push('.');
        }
        text
    }
}

impl CleanupProvider for FakeCleanup {
    async fn warm(&self) -> Result<()> {
        sleep(Duration::from_millis(5)).await;
        Ok(())
    }

    async fn clean(&self, raw: &str, on_text: &mut dyn FnMut(&str)) -> Result<()> {
        if raw.contains(STALL_WORD) {
            sleep(Duration::from_secs(10)).await;
        }
        sleep(Self::first_token_delay(raw)).await;
        for (i, word) in Self::cleaned(raw).split(' ').enumerate() {
            if i > 0 {
                sleep(Self::PER_WORD).await;
                on_text(" ");
            }
            on_text(word);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::{Lang, detect};

    #[test]
    fn canned_lines_are_in_the_language_of_their_label() {
        for clip in clips() {
            let raw = line_for(&clip.name);
            assert_eq!(detect(raw), clip.lang, "{raw}");
            assert_eq!(detect(&FakeCleanup::cleaned(raw)), clip.lang, "{raw}");
        }
        assert_eq!(detect(line_for("mic-01")), Some(Lang::En));
    }

    #[test]
    fn fake_cleanup_removes_fillers() {
        assert_eq!(FakeCleanup::cleaned("eh hola equipo"), "Hola equipo.");
        assert_eq!(FakeCleanup::cleaned("ready?"), "Ready?");
    }

    #[test]
    fn clips_are_short_and_cover_both_languages() {
        let clips = clips();
        assert_eq!(clips.iter().filter(|c| c.lang == Some(Lang::Es)).count(), 3);
        assert_eq!(clips.iter().filter(|c| c.lang == Some(Lang::En)).count(), 3);
        assert!(clips.iter().all(|c| c.duration_ms() <= 1_100));
    }
}
