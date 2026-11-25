//! Deepgram streaming transcription (`/v1/listen`) over WebSocket, streaming only.
//!
//! URL, query parameters, authentication and the Finalize and CloseStream messages were checked
//! on 25 November 2026 against <https://developers.deepgram.com/reference/regional-endpoints>,
//! <https://developers.deepgram.com/docs/finalize>, <https://developers.deepgram.com/docs/close-stream>
//! and <https://developers.deepgram.com/reference/speech-to-text/listen-streaming>.
//!
//! Deepgram says the `from_finalize` reply is sent "in most cases" but not always, so the
//! session also sends CloseStream: its closing Metadata message ends the dictation if the
//! Finalize reply never comes.

use anyhow::{Result, bail};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

use super::ws::{self, LiveSocket, Step};
use super::{LiveSession, SpeechProvider, Transcript};
use crate::audio::{SAMPLE_RATE, pcm_bytes};

pub const KEY_ENV: &str = "DEEPGRAM_API_KEY";
/// The EU endpoint. `wss://api.deepgram.com/v1/listen` is the default one.
pub const DEFAULT_URL: &str = "wss://api.eu.deepgram.com/v1/listen";
pub const DEFAULT_MODEL: &str = "nova-3";

pub struct DeepgramSpeech {
    pub key: String,
    pub url: String,
    pub model: String,
    /// One language is sent as is; none or several become `multi`.
    pub languages: Vec<String>,
    pub trace: bool,
}

/// The streaming URL with its query: model, language, raw 16-bit PCM at our rate, formatting.
fn listen_url(base: &str, model: &str, languages: &[String]) -> String {
    let language = match languages {
        [one] => one.as_str(),
        _ => "multi",
    };
    let separator = if base.contains('?') { '&' } else { '?' };
    format!(
        "{base}{separator}model={model}&language={language}&encoding=linear16\
         &sample_rate={SAMPLE_RATE}&channels=1&punctuate=true&smart_format=true"
    )
}

/// Collects final results until the reply to Finalize, or the Metadata sent on close.
#[derive(Default)]
struct Collector {
    parts: Vec<String>,
    languages: Vec<String>,
}

impl Collector {
    fn on_event(&mut self, event: &str) -> Step {
        let Ok(value) = serde_json::from_str::<Value>(event) else {
            return Step::Continue;
        };
        match value["type"].as_str().unwrap_or_default() {
            "Results" => {
                let from_finalize = value["from_finalize"].as_bool() == Some(true);
                if value["is_final"].as_bool() == Some(true) || from_finalize {
                    let alternative = &value["channel"]["alternatives"][0];
                    let text = alternative["transcript"]
                        .as_str()
                        .unwrap_or_default()
                        .trim();
                    if !text.is_empty() {
                        self.parts.push(text.to_owned());
                    }
                    let languages = alternative["languages"].as_array().into_iter().flatten();
                    for language in languages.filter_map(Value::as_str) {
                        if !self.languages.iter().any(|l| l == language) {
                            self.languages.push(language.to_owned());
                        }
                    }
                }
                if from_finalize {
                    self.done()
                } else {
                    Step::Continue
                }
            }
            "Metadata" => self.done(),
            "Error" => Step::Failed(event.to_owned()),
            _ => Step::Continue,
        }
    }

    fn done(&mut self) -> Step {
        Step::Done(Transcript {
            text: self.parts.join(" "),
            languages: std::mem::take(&mut self.languages),
        })
    }
}

impl SpeechProvider for DeepgramSpeech {
    type Live = DeepgramLive;

    async fn transcribe_file(&self, _label: &str, _wav: Vec<u8>) -> Result<Transcript> {
        bail!("the Deepgram provider only streams (pipeline B)")
    }

    fn supports_upload(&self) -> bool {
        false
    }

    async fn open_live(&self, _label: &str) -> Result<DeepgramLive> {
        let url = listen_url(&self.url, &self.model, &self.languages);
        let (sink, stream) = ws::connect(&url, &format!("Token {}", self.key)).await?;
        let mut collector = Collector::default();
        let reader = ws::read_until_final(stream, self.trace, move |e| collector.on_event(e));
        Ok(DeepgramLive(LiveSocket::new(sink, tokio::spawn(reader))))
    }
}

pub struct DeepgramLive(LiveSocket);

impl LiveSession for DeepgramLive {
    async fn send_audio(&mut self, pcm: &[i16]) -> Result<()> {
        self.0.send(Message::binary(pcm_bytes(pcm))).await
    }

    async fn finish(self) -> Result<Transcript> {
        let finalize = json!({"type": "Finalize"}).to_string();
        let close = json!({"type": "CloseStream"}).to_string();
        self.0
            .finish(
                vec![Message::text(finalize), Message::text(close)],
                "Finalize",
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::ws::testserver::{self, Got};
    use super::*;

    #[test]
    fn the_url_carries_model_language_and_audio_format() {
        let both = ["es".to_owned(), "en".to_owned()];
        assert_eq!(
            listen_url(DEFAULT_URL, "nova-3", &both),
            "wss://api.eu.deepgram.com/v1/listen?model=nova-3&language=multi&encoding=linear16\
             &sample_rate=24000&channels=1&punctuate=true&smart_format=true"
        );
        let one = listen_url("wss://x/v1/listen?tag=a", "nova-3", &["es".to_owned()]);
        assert!(one.starts_with("wss://x/v1/listen?tag=a&model=nova-3&language=es&"));
    }

    fn results(text: &str, is_final: bool, from_finalize: bool) -> String {
        json!({"type": "Results", "is_final": is_final, "from_finalize": from_finalize,
            "channel": {"alternatives": [{"transcript": text, "languages": ["es"]}]}})
        .to_string()
    }

    #[test]
    fn final_results_are_joined_until_the_finalize_reply() {
        let mut collector = Collector::default();
        assert_eq!(
            collector.on_event(&results("hola", false, false)),
            Step::Continue
        );
        assert_eq!(
            collector.on_event(&results("Hola equipo.", true, false)),
            Step::Continue
        );
        assert_eq!(
            collector.on_event(&results("Mañana a las once.", true, true)),
            Step::Done(Transcript {
                text: "Hola equipo. Mañana a las once.".to_owned(),
                languages: vec!["es".to_owned()],
            })
        );
    }

    #[test]
    fn metadata_on_close_ends_the_dictation_when_no_finalize_reply_came() {
        let mut collector = Collector::default();
        collector.on_event(&results("Hola.", true, false));
        let metadata = json!({"type": "Metadata", "request_id": "r"}).to_string();
        assert!(matches!(collector.on_event(&metadata), Step::Done(t) if t.text == "Hola."));
    }

    fn speech(url: String) -> DeepgramSpeech {
        DeepgramSpeech {
            key: "test-key".to_owned(),
            url,
            model: "nova-3".to_owned(),
            languages: vec!["es".to_owned(), "en".to_owned()],
            trace: false,
        }
    }

    #[tokio::test]
    async fn a_session_streams_binary_audio_and_finalizes() {
        let (base, server) = testserver::serve(vec![], |got| match got {
            Got::Binary(_) => vec![],
            Got::Json(event) if event["type"] == "Finalize" => {
                vec![serde_json::from_str(&results("Hola, hola.", true, true)).unwrap()]
            }
            Got::Json(event) if event["type"] == "CloseStream" => vec![],
            other => panic!("unexpected client message: {other:?}"),
        })
        .await;
        let stt = speech(format!("{base}/v1/listen"));

        let mut live = stt.open_live("es-01").await.unwrap();
        live.send_audio(&[1, -2]).await.unwrap();
        let transcript = live.finish().await.unwrap();
        assert_eq!(transcript.text, "Hola, hola.");

        let seen = server.await.unwrap();
        assert!(
            seen.path_and_query
                .starts_with("/v1/listen?model=nova-3&language=multi&encoding=linear16"),
            "{}",
            seen.path_and_query
        );
        assert_eq!(seen.auth, "Token test-key");
        assert_eq!(seen.got[0], Got::Binary(vec![1, 0, 0xFE, 0xFF]));
        assert_eq!(seen.got[1], Got::Json(json!({"type": "Finalize"})));
    }
}
