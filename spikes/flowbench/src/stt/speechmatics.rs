//! Speechmatics real-time transcription over WebSocket, streaming only.
//!
//! URL, authentication and message shapes were checked on 25 November 2026 against
//! <https://docs.speechmatics.com/api-ref/realtime-transcription-websocket> and
//! <https://docs.speechmatics.com/speech-to-text/realtime/turn-detection>. Not verified there:
//! the exact form of `language_hints`, sent here as a list of codes. `--languages off` leaves it
//! out if the service rejects it.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

use super::ws::{self, LiveSocket, Step};
use super::{LiveSession, SpeechProvider, Transcript};
use crate::audio::{SAMPLE_RATE, pcm_bytes};

pub const KEY_ENV: &str = "SPEECHMATICS_API_KEY";
/// The EU region. `wss://global.rt.speechmatics.com/v2` is the global one.
pub const DEFAULT_URL: &str = "wss://eu.rt.speechmatics.com/v2";
/// Melia 1 takes `language: multi` and finds the language itself.
pub const DEFAULT_MODEL: &str = "melia-1";
const STARTED_TIMEOUT: Duration = Duration::from_secs(10);

pub struct SpeechmaticsSpeech {
    pub key: String,
    pub url: String,
    /// `melia-1`, `oak-1`, `enhanced` or `standard`.
    pub model: String,
    /// Language hints for the multi-language models; the one language for the others.
    pub languages: Vec<String>,
    /// `max_delay` in seconds (0.7 to 4). `None` leaves the service default.
    pub max_delay: Option<f64>,
    pub trace: bool,
}

/// Models that detect the language themselves and need `language: multi`.
fn is_multi(model: &str) -> bool {
    matches!(model, "melia-1" | "oak-1")
}

/// The `StartRecognition` message: raw 16-bit PCM at our rate, no partials.
fn start_recognition(model: &str, languages: &[String], max_delay: Option<f64>) -> Result<Value> {
    let mut config = json!({"model": model});
    if is_multi(model) {
        config["language"] = json!("multi");
        if !languages.is_empty() {
            config["language_hints"] = json!(languages);
        }
    } else {
        let [language] = languages else {
            bail!(
                "Speechmatics model {model} needs exactly one language in --languages; \
                 melia-1 and oak-1 detect it themselves"
            );
        };
        config["language"] = json!(language);
    }
    if let Some(delay) = max_delay {
        config["max_delay"] = json!(delay);
    }
    Ok(json!({
        "message": "StartRecognition",
        "audio_format": {"type": "raw", "encoding": "pcm_s16le", "sample_rate": SAMPLE_RATE},
        "transcription_config": config,
    }))
}

/// Collects final transcripts until the forced end of utterance.
#[derive(Default)]
struct Collector {
    text: String,
    languages: Vec<String>,
}

impl Collector {
    fn on_event(&mut self, event: &str) -> Step {
        let Ok(value) = serde_json::from_str::<Value>(event) else {
            return Step::Continue;
        };
        match value["message"].as_str().unwrap_or_default() {
            "AddTranscript" => {
                self.text
                    .push_str(value["metadata"]["transcript"].as_str().unwrap_or_default());
                let words = value["results"].as_array().into_iter().flatten();
                for language in words.filter_map(|w| w["alternatives"][0]["language"].as_str()) {
                    if !self.languages.iter().any(|l| l == language) {
                        self.languages.push(language.to_owned());
                    }
                }
                Step::Continue
            }
            // Only the utterance ended by our ForceEndOfUtterance closes the dictation.
            "EndOfUtterance" if value["forced"].as_bool() == Some(true) => self.done(),
            "EndOfTranscript" => self.done(),
            "Error" => Step::Failed(event.to_owned()),
            _ => Step::Continue,
        }
    }

    fn done(&mut self) -> Step {
        Step::Done(Transcript {
            text: self.text.trim().to_owned(),
            languages: std::mem::take(&mut self.languages),
        })
    }
}

impl SpeechProvider for SpeechmaticsSpeech {
    type Live = SpeechmaticsLive;

    async fn transcribe_file(&self, _label: &str, _wav: Vec<u8>) -> Result<Transcript> {
        bail!("the Speechmatics provider only streams (pipeline B)")
    }

    fn supports_upload(&self) -> bool {
        false
    }

    async fn open_live(&self, _label: &str) -> Result<SpeechmaticsLive> {
        let start = start_recognition(&self.model, &self.languages, self.max_delay)?;
        let (mut sink, mut stream) =
            ws::connect(&self.url, &format!("Bearer {}", self.key)).await?;
        sink.send(Message::text(start.to_string()))
            .await
            .context("sending StartRecognition")?;
        // Audio sent before the session has started could be refused, so wait for it.
        let started = async {
            while let Some(message) = stream.next().await {
                if let Message::Text(event) = message.context("streaming socket")? {
                    let value: Value = serde_json::from_str(event.as_str()).unwrap_or_default();
                    match value["message"].as_str() {
                        Some("RecognitionStarted") => return Ok(()),
                        Some("Error") => bail!("Speechmatics refused the session: {event}"),
                        _ => {}
                    }
                }
            }
            bail!("the socket closed before the session started")
        };
        timeout(STARTED_TIMEOUT, started)
            .await
            .context("no RecognitionStarted from Speechmatics")??;
        let mut collector = Collector::default();
        let reader = ws::read_until_final(stream, self.trace, move |e| collector.on_event(e));
        Ok(SpeechmaticsLive(LiveSocket::new(
            sink,
            tokio::spawn(reader),
        )))
    }
}

pub struct SpeechmaticsLive(LiveSocket);

impl LiveSession for SpeechmaticsLive {
    async fn send_audio(&mut self, pcm: &[i16]) -> Result<()> {
        self.0.send(Message::binary(pcm_bytes(pcm))).await
    }

    async fn finish(self) -> Result<Transcript> {
        let force = json!({"message": "ForceEndOfUtterance"});
        self.0
            .finish(
                vec![Message::text(force.to_string())],
                "ForceEndOfUtterance",
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::ws::testserver::{self, Got};
    use super::*;

    #[test]
    fn multi_language_models_send_multi_and_hints() {
        let languages = ["es".to_owned(), "en".to_owned()];
        let start = start_recognition("melia-1", &languages, Some(0.7)).unwrap();
        assert_eq!(start["message"], "StartRecognition");
        assert_eq!(
            start["audio_format"],
            json!({"type": "raw", "encoding": "pcm_s16le", "sample_rate": 24000})
        );
        let config = &start["transcription_config"];
        assert_eq!(config["model"], "melia-1");
        assert_eq!(config["language"], "multi");
        assert_eq!(config["language_hints"], json!(["es", "en"]));
        assert_eq!(config["max_delay"], 0.7);
        assert!(config.get("enable_partials").is_none());

        let no_hints = start_recognition("melia-1", &[], None).unwrap();
        assert!(
            no_hints["transcription_config"]
                .get("language_hints")
                .is_none()
        );
        assert!(no_hints["transcription_config"].get("max_delay").is_none());
    }

    #[test]
    fn single_language_models_need_exactly_one_language() {
        let start = start_recognition("enhanced", &["es".to_owned()], None).unwrap();
        assert_eq!(start["transcription_config"]["language"], "es");
        let two = ["es".to_owned(), "en".to_owned()];
        let error = start_recognition("enhanced", &two, None).unwrap_err();
        assert!(error.to_string().contains("exactly one language"));
    }

    #[test]
    fn transcripts_are_joined_until_the_forced_end_of_utterance() {
        let mut collector = Collector::default();
        let first = json!({"message": "AddTranscript", "metadata": {"transcript": "Hola equipo. "},
            "results": [{"type": "word", "alternatives": [{"content": "Hola", "language": "es"}]}]});
        let natural = json!({"message": "EndOfUtterance", "metadata": {}});
        let second = json!({"message": "AddTranscript", "forced": true,
            "metadata": {"transcript": "Mañana a las once."}, "results": []});
        let forced = json!({"message": "EndOfUtterance", "forced": true});
        for event in [&first, &natural, &second] {
            assert_eq!(collector.on_event(&event.to_string()), Step::Continue);
        }
        assert_eq!(
            collector.on_event(&forced.to_string()),
            Step::Done(Transcript {
                text: "Hola equipo. Mañana a las once.".to_owned(),
                languages: vec!["es".to_owned()],
            })
        );
        let error = json!({"message": "Error", "type": "invalid_model", "reason": "no such model"});
        assert!(matches!(
            Collector::default().on_event(&error.to_string()),
            Step::Failed(body) if body.contains("no such model")
        ));
    }

    fn speech(url: String) -> SpeechmaticsSpeech {
        SpeechmaticsSpeech {
            key: "test-key".to_owned(),
            url,
            model: "melia-1".to_owned(),
            languages: vec!["es".to_owned(), "en".to_owned()],
            max_delay: None,
            trace: false,
        }
    }

    #[tokio::test]
    async fn a_session_starts_streams_binary_audio_and_forces_the_end() {
        let (base, server) = testserver::serve(vec![], |got| match got {
            Got::Json(event) if event["message"] == "StartRecognition" => {
                vec![json!({"message": "RecognitionStarted", "id": "abc"})]
            }
            Got::Binary(_) => vec![json!({"message": "AudioAdded", "seq_no": 1})],
            Got::Json(event) if event["message"] == "ForceEndOfUtterance" => vec![
                json!({"message": "AddTranscript", "forced": true,
                    "metadata": {"transcript": "Hola, hola."}, "results": []}),
                json!({"message": "EndOfUtterance", "forced": true}),
            ],
            other => panic!("unexpected client message: {other:?}"),
        })
        .await;
        let stt = speech(format!("{base}/v2"));

        let mut live = stt.open_live("es-01").await.unwrap();
        live.send_audio(&[1, -2]).await.unwrap();
        let transcript = live.finish().await.unwrap();
        assert_eq!(transcript.text, "Hola, hola.");

        let seen = server.await.unwrap();
        assert_eq!(seen.path_and_query, "/v2");
        assert_eq!(seen.auth, "Bearer test-key");
        assert_eq!(seen.got.len(), 3);
        assert!(matches!(&seen.got[0], Got::Json(e) if e["message"] == "StartRecognition"));
        assert_eq!(seen.got[1], Got::Binary(vec![1, 0, 0xFE, 0xFF]));
        assert_eq!(
            seen.got[2],
            Got::Json(json!({"message": "ForceEndOfUtterance"}))
        );
    }

    #[tokio::test]
    async fn a_refused_session_fails_before_any_audio() {
        let (base, _server) = testserver::serve(vec![], |_| {
            vec![json!({"message": "Error", "type": "not_authorised", "reason": "bad key"})]
        })
        .await;
        let error = format!("{:#}", speech(base).open_live("es-01").await.err().unwrap());
        assert!(error.contains("bad key"), "{error}");
    }

    #[tokio::test]
    async fn upload_is_not_offered() {
        let stt = speech(String::new());
        assert!(!stt.supports_upload());
        assert!(stt.transcribe_file("es-01", vec![]).await.is_err());
    }
}
