//! Speechmatics real-time transcription over WebSocket, ported from `spikes/flowbench`.
//!
//! Checked on 26 November 2026 against
//! <https://docs.speechmatics.com/api-ref/realtime-transcription-websocket>: `Authorization:
//! Bearer` on the handshake (a bad key is a 401 there, or close code 4001 / Error
//! `not_authorised`), `StartRecognition` with raw `pcm_s16le`, `model` `melia-1` needing
//! `language: "multi"`, binary AddAudio frames, `ForceEndOfUtterance` (answered with
//! `forced: true`) and `EndOfStream` with `last_seq_no`, then `EndOfTranscript`. Not stated
//! there: how `seq_no` is numbered; this counts binary frames, as flowbench did.

use std::fmt;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use hyperme_core::config;
use hyperme_core::traits::{Error, ErrorKind, LiveSession, SpeechProvider, Transcript};
use serde_json::{Value, json};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::{self, Message};

use crate::ws::{self, Sink, Stream, auth_error, provider_error};

/// The rate of the PCM the session expects: mono, 16-bit. Phase 0 measured at this rate.
pub const SAMPLE_RATE: u32 = 24_000;
/// R-3: audio goes out in chunks of at most 100 ms.
pub const MAX_CHUNK_SAMPLES: usize = SAMPLE_RATE as usize / 10;

/// Speechmatics warns that a session can take seconds to start.
const STARTED_TIMEOUT: Duration = Duration::from_secs(20);
/// How long to wait for the final transcript after the end of speech.
const FINAL_TIMEOUT: Duration = Duration::from_secs(30);
/// Close code for `not_authorised`.
const CLOSE_NOT_AUTHORISED: u16 = 4001;

/// The Speechmatics provider: opens one session per dictation.
pub struct Speechmatics {
    /// The provider name from the config, used in messages (R-21).
    provider: String,
    endpoint: String,
    model: String,
    key: String,
}

impl fmt::Debug for Speechmatics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Speechmatics")
            .field("endpoint", &self.endpoint)
            .field("model", &self.model)
            .finish_non_exhaustive()
    }
}

impl Speechmatics {
    /// `key` comes from the keychain; it is sent only in the handshake's header.
    pub fn new(config: &config::Speech, key: String) -> Self {
        Speechmatics {
            provider: config.provider.clone(),
            endpoint: config.endpoint.clone(),
            model: config.model.clone(),
            key,
        }
    }

    fn error(&self, kind: ErrorKind, what: impl fmt::Display) -> Error {
        provider_error(&self.provider, kind, what)
    }

    async fn connect(&self) -> Result<(Sink, Stream), Error> {
        ws::connect(&self.provider, &self.endpoint, &self.key).await
    }
}

/// The `StartRecognition` message: raw 16-bit PCM at [`SAMPLE_RATE`], no partials, and no
/// expected language (R-10): `multi` lets the model detect it.
fn start_recognition(model: &str) -> Value {
    json!({
        "message": "StartRecognition",
        "audio_format": {"type": "raw", "encoding": "pcm_s16le", "sample_rate": SAMPLE_RATE},
        "transcription_config": {"model": model, "language": "multi"},
    })
}

impl SpeechProvider for Speechmatics {
    type Session = SpeechmaticsSession;

    async fn open(&self) -> Result<SpeechmaticsSession, Error> {
        let (mut sink, mut stream) = self.connect().await?;
        let start = start_recognition(&self.model).to_string();
        sink.send(Message::text(start))
            .await
            .map_err(|e| self.error(ErrorKind::Network, format!("the connection dropped: {e}")))?;
        // Audio sent before the session has started could be refused, so wait for it.
        let started = async {
            while let Some(message) = stream.next().await {
                match message {
                    Ok(Message::Text(event)) => {
                        let value: Value = serde_json::from_str(event.as_str()).unwrap_or_default();
                        match value["message"].as_str() {
                            Some("RecognitionStarted") => return Ok(()),
                            Some("Error") => return Err(server_error(&self.provider, &value)),
                            _ => {}
                        }
                    }
                    Ok(Message::Close(frame)) => {
                        return Err(closed_error(&self.provider, frame.as_ref()));
                    }
                    Ok(_) => {}
                    Err(e) => {
                        return Err(
                            self.error(ErrorKind::Network, format!("the connection dropped: {e}"))
                        );
                    }
                }
            }
            Err(self.error(
                ErrorKind::Network,
                "the connection closed before the session started",
            ))
        };
        let outcome = timeout(STARTED_TIMEOUT, started).await.unwrap_or_else(|_| {
            Err(self.error(ErrorKind::Network, "the session did not start in time"))
        });
        if let Err(error) = outcome {
            // A refused session counts against the concurrent quota until it is closed.
            let _ = sink.send(Message::Close(None)).await;
            return Err(error);
        }
        let provider = self.provider.clone();
        let reader = tokio::spawn(read_until_final(stream, provider.clone()));
        Ok(SpeechmaticsSession {
            provider,
            sink,
            reader: Some(reader),
            sent: 0,
        })
    }
}

/// One streaming dictation. Dropping it closes the connection.
pub struct SpeechmaticsSession {
    provider: String,
    sink: Sink,
    /// Ends with the final transcript. `None` once its result has been taken.
    reader: Option<JoinHandle<Result<Transcript, Error>>>,
    /// Binary frames sent, for `EndOfStream`'s `last_seq_no`.
    sent: u64,
}

impl fmt::Debug for SpeechmaticsSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpeechmaticsSession")
            .field("sent", &self.sent)
            .finish_non_exhaustive()
    }
}

impl SpeechmaticsSession {
    fn reader_finished(&self) -> bool {
        self.reader.as_ref().is_some_and(JoinHandle::is_finished)
    }

    async fn reader_result(&mut self) -> Result<Transcript, Error> {
        let Some(mut reader) = self.reader.take() else {
            return Err(provider_error(
                &self.provider,
                ErrorKind::Other,
                "the session has already ended",
            ));
        };
        match timeout(FINAL_TIMEOUT, &mut reader).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(provider_error(
                &self.provider,
                ErrorKind::Other,
                "the transcript reader stopped",
            )),
            Err(_) => {
                reader.abort();
                Err(provider_error(
                    &self.provider,
                    ErrorKind::Network,
                    format!("no final transcript within {} s", FINAL_TIMEOUT.as_secs()),
                ))
            }
        }
    }

    /// Sends a Close frame so a failed session does not stay open. Failures are ignored: the
    /// socket may already be gone.
    async fn close(&mut self) {
        let _ = self.sink.send(Message::Close(None)).await;
    }

    async fn send_frame(&mut self, pcm: &[i16]) -> Result<(), Error> {
        // Before the end of speech the reader only ends on a failure: report it rather than
        // stream into a dead session.
        if self.reader.is_none() || self.reader_finished() {
            let result = self.reader_result().await;
            self.close().await;
            return Err(match result {
                Ok(_) => {
                    provider_error(&self.provider, ErrorKind::Other, "the session ended early")
                }
                Err(error) => error,
            });
        }
        let bytes: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
        self.sink.send(Message::binary(bytes)).await.map_err(|e| {
            provider_error(
                &self.provider,
                ErrorKind::Network,
                format!("the connection dropped: {e}"),
            )
        })?;
        self.sent += 1;
        Ok(())
    }
}

impl LiveSession for SpeechmaticsSession {
    /// R-3: split into chunks of at most 100 ms, so audio buffered while the session opened
    /// still goes out in small frames.
    async fn send_audio(&mut self, pcm: &[i16]) -> Result<(), Error> {
        for chunk in pcm.chunks(MAX_CHUNK_SAMPLES) {
            self.send_frame(chunk).await?;
        }
        Ok(())
    }

    /// R-9: sends ForceEndOfUtterance, then EndOfStream, and waits for the forced end of
    /// utterance or EndOfTranscript, whichever comes first.
    async fn finish(mut self) -> Result<Transcript, Error> {
        if self.reader.is_none() {
            return Err(provider_error(
                &self.provider,
                ErrorKind::Other,
                "the session has already ended",
            ));
        }
        let force = json!({"message": "ForceEndOfUtterance"}).to_string();
        let end = json!({"message": "EndOfStream", "last_seq_no": self.sent}).to_string();
        for message in [force, end] {
            if let Err(e) = self.sink.send(Message::text(message)).await {
                // If the provider said why it hung up, the reader has the useful message.
                if !self.reader_finished() {
                    return Err(provider_error(
                        &self.provider,
                        ErrorKind::Network,
                        format!("the connection dropped: {e}"),
                    ));
                }
                break;
            }
        }
        let result = self.reader_result().await;
        if result.is_err() {
            self.close().await;
        }
        result
    }
}

impl Drop for SpeechmaticsSession {
    fn drop(&mut self) {
        // An abandoned session must not keep reading in the background.
        if let Some(reader) = &self.reader {
            reader.abort();
        }
    }
}

async fn read_until_final(mut stream: Stream, provider: String) -> Result<Transcript, Error> {
    let mut collector = Collector::default();
    while let Some(message) = stream.next().await {
        match message {
            Ok(Message::Text(event)) => match collector.on_event(event.as_str()) {
                Step::Continue => {}
                Step::Done(transcript) => return Ok(transcript),
                Step::Failed(value) => return Err(server_error(&provider, &value)),
            },
            Ok(Message::Close(frame)) => return Err(closed_error(&provider, frame.as_ref())),
            Ok(_) => {}
            Err(e) => {
                return Err(provider_error(
                    &provider,
                    ErrorKind::Network,
                    format!("the connection dropped: {e}"),
                ));
            }
        }
    }
    Err(provider_error(
        &provider,
        ErrorKind::Network,
        "the connection dropped before the final transcript",
    ))
}

/// An `Error` event from the server.
fn server_error(provider: &str, value: &Value) -> Error {
    let kind = value["type"].as_str().unwrap_or("unknown_error");
    if kind == "not_authorised" {
        return auth_error(provider);
    }
    let reason = value["reason"].as_str().unwrap_or_default();
    provider_error(provider, ErrorKind::Other, format!("{kind}: {reason}"))
}

/// A Close frame before the final transcript.
fn closed_error(provider: &str, frame: Option<&tungstenite::protocol::CloseFrame>) -> Error {
    match frame {
        Some(frame) if frame.code == CloseCode::from(CLOSE_NOT_AUTHORISED) => auth_error(provider),
        Some(frame) => provider_error(
            provider,
            ErrorKind::Network,
            format!(
                "the connection closed before the final transcript (code {})",
                u16::from(frame.code)
            ),
        ),
        None => provider_error(
            provider,
            ErrorKind::Network,
            "the connection closed before the final transcript",
        ),
    }
}

/// What one server event means for the dictation.
#[derive(Debug, PartialEq)]
enum Step {
    Continue,
    Done(Transcript),
    Failed(Value),
}

/// Collects final transcripts until the forced end of utterance or the end of transcript.
#[derive(Default)]
struct Collector {
    text: String,
    language: Option<String>,
    /// The last piece opens the next word, so no space follows it.
    glue_next: bool,
}

impl Collector {
    fn on_event(&mut self, event: &str) -> Step {
        let Ok(value) = serde_json::from_str::<Value>(event) else {
            return Step::Continue;
        };
        match value["message"].as_str().unwrap_or_default() {
            "AddTranscript" => {
                match value["results"].as_array().filter(|r| !r.is_empty()) {
                    Some(results) => {
                        for result in results {
                            self.push_result(result);
                        }
                    }
                    // No word list: take the formatted text as it is.
                    None => self.push_text(
                        value["metadata"]["transcript"].as_str().unwrap_or_default(),
                        false,
                    ),
                }
                Step::Continue
            }
            "LanguageInfo" => {
                if self.language.is_none() {
                    self.language = value["language"].as_str().map(str::to_owned);
                }
                Step::Continue
            }
            // Only the utterance ended by our ForceEndOfUtterance closes the dictation.
            "EndOfUtterance" if value["forced"].as_bool() == Some(true) => self.done(),
            "EndOfTranscript" => self.done(),
            "Error" => Step::Failed(value),
            _ => Step::Continue,
        }
    }

    /// Adds one word or punctuation mark. Joining `metadata.transcript` pieces glued words
    /// together across messages in flowbench's first run, so the text is built from the words.
    fn push_result(&mut self, result: &Value) {
        let alternative = &result["alternatives"][0];
        if self.language.is_none() {
            self.language = alternative["language"].as_str().map(str::to_owned);
        }
        let content = alternative["content"].as_str().unwrap_or_default();
        let attaches = result["attaches_to"].as_str().unwrap_or_default();
        let punctuation = result["type"] == "punctuation";
        // Punctuation sticks to the word before it unless it opens the next one (¿, ¡).
        let glue_before = punctuation && attaches != "next";
        self.push_text(content, glue_before);
        self.glue_next = punctuation && attaches == "next";
    }

    fn push_text(&mut self, piece: &str, glue_before: bool) {
        let piece = piece.trim();
        if piece.is_empty() {
            return;
        }
        if !self.text.is_empty() && !glue_before && !self.glue_next {
            self.text.push(' ');
        }
        self.text.push_str(piece);
        self.glue_next = false;
    }

    fn done(&mut self) -> Step {
        Step::Done(Transcript {
            text: self.text.trim().to_owned(),
            language: self.language.take(),
            via: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_spaced_and_punctuation_attached_across_messages() {
        let word =
            |w: &str| json!({"type": "word", "alternatives": [{"content": w, "language": "es"}]});
        let mark = |m: &str, to: &str| json!({"type": "punctuation", "attaches_to": to, "alternatives": [{"content": m}]});
        let mut collector = Collector::default();
        let first = json!({"message": "AddTranscript",
            "results": [word("Ok"), mark(",", "previous"), word("creo"), word("que"), word("está")]});
        let second = json!({"message": "AddTranscript",
            "results": [word("grabando"), mark(".", "previous"), mark("¿", "next"), word("Sí"), mark("?", "previous")]});
        let natural = json!({"message": "EndOfUtterance", "metadata": {}});
        for event in [&first, &natural, &second] {
            assert_eq!(collector.on_event(&event.to_string()), Step::Continue);
        }
        let forced = json!({"message": "EndOfUtterance", "forced": true});
        assert_eq!(
            collector.on_event(&forced.to_string()),
            Step::Done(Transcript {
                text: "Ok, creo que está grabando. ¿Sí?".to_owned(),
                language: Some("es".to_owned()),
                via: None,
            })
        );
    }

    #[test]
    fn transcript_without_words_uses_the_formatted_text() {
        let mut collector = Collector::default();
        let event =
            json!({"message": "AddTranscript", "metadata": {"transcript": "Hola."}, "results": []});
        collector.on_event(&event.to_string());
        let Step::Done(transcript) = collector.on_event(r#"{"message":"EndOfTranscript"}"#) else {
            panic!("EndOfTranscript ends the dictation");
        };
        assert_eq!(transcript.text, "Hola.");
    }

    /// AC-7 (R-10): no expected language is sent; `multi` lets the model detect it.
    #[test]
    fn start_recognition_names_no_language() {
        let start = start_recognition("melia-1");
        assert_eq!(
            start["audio_format"],
            json!({"type": "raw", "encoding": "pcm_s16le", "sample_rate": 24000})
        );
        let config = &start["transcription_config"];
        assert_eq!(config["language"], "multi");
        assert!(config.get("language_hints").is_none());
        assert!(config.get("enable_partials").is_none());
    }
}
