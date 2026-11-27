//! OpenAI streaming speech: a Realtime API transcription session over WebSocket, ported from
//! `spikes/flowbench` (specs/96-provider-settings, slice 8).
//!
//! Checked on 27 November 2026 against
//! <https://developers.openai.com/api/docs/guides/realtime-transcription>: the URL
//! `wss://api.openai.com/v1/realtime?intent=transcription` with no model in it, `Authorization:
//! Bearer`, a `session.update` of `type: "transcription"` with `audio.input.format` `audio/pcm`
//! at 24 kHz, `transcription.model` and `turn_detection: null` (manual commit), base64 PCM16 in
//! `input_audio_buffer.append`, `input_audio_buffer.commit` at the end of the turn, and the
//! transcript in `conversation.item.input_audio_transcription.completed`. Not stated there: what
//! a rejected key looks like; this treats a 401 on the handshake, or an `error` event whose code
//! names the API key, as a key error. No language is sent (Phase 1 R-10), and
//! `gpt-live-transcribe` reports none.

use std::fmt;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use futures_util::{SinkExt, StreamExt};
use hyperme_core::config;
use hyperme_core::traits::{Error, ErrorKind, LiveSession, SpeechProvider, Transcript};
use serde_json::{Value, json};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

use crate::speechmatics::{MAX_CHUNK_SAMPLES, SAMPLE_RATE};
use crate::ws::{self, Sink, Stream, auth_error, dropped, provider_error};

const DELTA: &str = "conversation.item.input_audio_transcription.delta";
const COMPLETED: &str = "conversation.item.input_audio_transcription.completed";
const FAILED: &str = "conversation.item.input_audio_transcription.failed";

/// How long to wait for `session.updated`.
const CONFIGURED_TIMEOUT: Duration = Duration::from_secs(20);
/// How long to wait for the transcript after the commit.
const FINAL_TIMEOUT: Duration = Duration::from_secs(30);

/// OpenAI streaming speech: opens one transcription session per dictation.
pub struct OpenAiRealtime {
    /// The provider name from the config, used in messages (R-21).
    provider: String,
    endpoint: String,
    model: String,
    key: String,
}

impl fmt::Debug for OpenAiRealtime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenAiRealtime")
            .field("endpoint", &self.endpoint)
            .field("model", &self.model)
            .finish_non_exhaustive()
    }
}

impl OpenAiRealtime {
    /// `key` comes from the keychain; it is sent only in the handshake's header.
    pub fn new(config: &config::Speech, key: String) -> Self {
        OpenAiRealtime {
            provider: config.provider.clone(),
            endpoint: config.endpoint.clone(),
            model: config.model.clone(),
            key,
        }
    }
}

/// The `session.update` for a transcription session with manual commit and no language.
fn session_update(model: &str) -> Value {
    json!({
        "type": "session.update",
        "session": {
            "type": "transcription",
            "audio": {"input": {
                "format": {"type": "audio/pcm", "rate": SAMPLE_RATE},
                "transcription": {"model": model},
                "turn_detection": null,
            }},
        },
    })
}

impl SpeechProvider for OpenAiRealtime {
    type Session = OpenAiSession;

    /// Waits for `session.updated`, so Test fails on a model the key cannot use (R-13). Audio
    /// captured meanwhile waits in the engine's buffer.
    async fn open(&self) -> Result<OpenAiSession, Error> {
        let (mut sink, mut stream) = ws::connect(&self.provider, &self.endpoint, &self.key).await?;
        let update = session_update(&self.model).to_string();
        sink.send(Message::text(update))
            .await
            .map_err(|e| dropped(&self.provider, e))?;
        let configured = async {
            while let Some(message) = stream.next().await {
                match message {
                    Ok(Message::Text(event)) => {
                        let value: Value = serde_json::from_str(event.as_str()).unwrap_or_default();
                        match value["type"].as_str() {
                            Some("session.updated") => return Ok(()),
                            Some("error") => return Err(server_error(&self.provider, &value)),
                            _ => {}
                        }
                    }
                    Ok(Message::Close(_)) => break,
                    Ok(_) => {}
                    Err(e) => return Err(dropped(&self.provider, e)),
                }
            }
            Err(provider_error(
                &self.provider,
                ErrorKind::Network,
                "the connection closed before the session started",
            ))
        };
        let outcome = timeout(CONFIGURED_TIMEOUT, configured)
            .await
            .unwrap_or_else(|_| {
                Err(provider_error(
                    &self.provider,
                    ErrorKind::Network,
                    "the session did not start in time",
                ))
            });
        if let Err(error) = outcome {
            let _ = sink.send(Message::Close(None)).await;
            return Err(error);
        }
        let reader = tokio::spawn(read_until_final(stream, self.provider.clone()));
        Ok(OpenAiSession {
            provider: self.provider.clone(),
            sink,
            reader: Some(reader),
        })
    }
}

/// One streaming dictation. Dropping it closes the connection.
pub struct OpenAiSession {
    provider: String,
    sink: Sink,
    /// Ends with the final transcript. `None` once its result has been taken.
    reader: Option<JoinHandle<Result<Transcript, Error>>>,
}

impl fmt::Debug for OpenAiSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenAiSession").finish_non_exhaustive()
    }
}

impl OpenAiSession {
    fn ended(&self) -> Error {
        provider_error(
            &self.provider,
            ErrorKind::Other,
            "the session has already ended",
        )
    }

    async fn reader_result(&mut self) -> Result<Transcript, Error> {
        let Some(mut reader) = self.reader.take() else {
            return Err(self.ended());
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
}

impl LiveSession for OpenAiSession {
    /// R-3: chunks of at most 100 ms, so audio buffered while the session opened still goes
    /// out in small events.
    async fn send_audio(&mut self, pcm: &[i16]) -> Result<(), Error> {
        for chunk in pcm.chunks(MAX_CHUNK_SAMPLES) {
            // Before the commit the reader only ends on a failure: report it rather than
            // stream into a dead session.
            if self.reader.as_ref().is_none_or(JoinHandle::is_finished) {
                let result = self.reader_result().await;
                let _ = self.sink.send(Message::Close(None)).await;
                return Err(match result {
                    Ok(_) => {
                        provider_error(&self.provider, ErrorKind::Other, "the session ended early")
                    }
                    Err(error) => error,
                });
            }
            let bytes: Vec<u8> = chunk.iter().flat_map(|s| s.to_le_bytes()).collect();
            let event = json!({
                "type": "input_audio_buffer.append",
                "audio": STANDARD.encode(bytes),
            });
            self.sink
                .send(Message::text(event.to_string()))
                .await
                .map_err(|e| dropped(&self.provider, e))?;
        }
        Ok(())
    }

    /// R-9: commits the audio and waits for the completed transcript.
    async fn finish(mut self) -> Result<Transcript, Error> {
        if self.reader.is_none() {
            return Err(self.ended());
        }
        let commit = json!({"type": "input_audio_buffer.commit"}).to_string();
        if let Err(e) = self.sink.send(Message::text(commit)).await {
            // If the provider said why it hung up, the reader has the useful message.
            if !self.reader.as_ref().is_some_and(JoinHandle::is_finished) {
                return Err(dropped(&self.provider, e));
            }
        }
        let result = self.reader_result().await;
        let _ = self.sink.send(Message::Close(None)).await;
        result
    }
}

impl Drop for OpenAiSession {
    fn drop(&mut self) {
        // An abandoned session must not keep reading in the background.
        if let Some(reader) = &self.reader {
            reader.abort();
        }
    }
}

async fn read_until_final(mut stream: Stream, provider: String) -> Result<Transcript, Error> {
    // The deltas, for a completion event that carries no transcript.
    let mut partial = String::new();
    while let Some(message) = stream.next().await {
        match message {
            Ok(Message::Text(event)) => {
                let Ok(value) = serde_json::from_str::<Value>(event.as_str()) else {
                    continue;
                };
                match value["type"].as_str().unwrap_or_default() {
                    DELTA => partial.push_str(value["delta"].as_str().unwrap_or_default()),
                    COMPLETED => {
                        let text = match value["transcript"].as_str() {
                            Some(text) => text.to_owned(),
                            None => partial,
                        };
                        return Ok(Transcript {
                            text: text.trim().to_owned(),
                            language: None,
                            via: None,
                        });
                    }
                    FAILED | "error" => return Err(server_error(&provider, &value)),
                    _ => {}
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => {}
            Err(e) => return Err(dropped(&provider, e)),
        }
    }
    Err(provider_error(
        &provider,
        ErrorKind::Network,
        "the connection closed before the final transcript",
    ))
}

/// An `error` or `failed` event. The message is OpenAI's own; it never holds the key.
fn server_error(provider: &str, value: &Value) -> Error {
    let error = &value["error"];
    let code = error["code"].as_str().unwrap_or_default();
    if code.contains("api_key") || error["type"] == "authentication_error" {
        return auth_error(provider);
    }
    let message = error["message"].as_str().unwrap_or("the request failed");
    provider_error(provider, ErrorKind::Other, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Copied from the guide's first example; only the model differs by config.
    #[test]
    fn session_update_is_the_documented_json() {
        let documented: Value = serde_json::from_str(
            r#"{"type": "session.update",
                "session": {"type": "transcription",
                  "audio": {"input": {
                    "format": {"type": "audio/pcm", "rate": 24000},
                    "transcription": {"model": "gpt-live-transcribe"},
                    "turn_detection": null}}}}"#,
        )
        .unwrap();
        assert_eq!(session_update("gpt-live-transcribe"), documented);
    }
}
