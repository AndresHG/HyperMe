//! OpenAI speech: file upload and realtime transcription over WebSocket.
//!
//! URL, session shape, event names and model names were checked on 24 November 2026 against
//! <https://developers.openai.com/api/docs/guides/realtime-transcription> and
//! <https://developers.openai.com/api/docs/guides/speech-to-text>. Check them again before
//! changing anything here: they changed more than once in 2026.

use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use futures_util::SinkExt;
use reqwest::multipart::{Form, Part};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

use super::ws::{self, LiveSocket};
use super::{LiveSession, SpeechProvider, Transcript};
use crate::audio::{SAMPLE_RATE, pcm_bytes};
use crate::http::ensure_ok;

pub const KEY_ENV: &str = "OPENAI_API_KEY";
pub const FILE_URL: &str = "https://api.openai.com/v1/audio/transcriptions";

const DELTA: &str = "conversation.item.input_audio_transcription.delta";
const COMPLETED: &str = "conversation.item.input_audio_transcription.completed";
const FAILED: &str = "conversation.item.input_audio_transcription.failed";

pub struct OpenAiSpeech {
    pub http: reqwest::Client,
    pub key: String,
    pub file_url: String,
    pub realtime_url: String,
    pub file_model: String,
    pub live_model: String,
    /// Language hints, the same for every dictation. Empty sends none.
    pub languages: Vec<String>,
    /// The live model's `delay` setting. `None` leaves the provider's default.
    pub delay: Option<String>,
    /// Print the type of every realtime event to stderr.
    pub trace: bool,
}

impl SpeechProvider for OpenAiSpeech {
    type Live = OpenAiLive;

    async fn transcribe_file(&self, label: &str, wav: Vec<u8>) -> Result<Transcript> {
        let file = Part::bytes(wav)
            .file_name(format!("{label}.wav"))
            .mime_str("audio/wav")?;
        let mut form = Form::new()
            .part("file", file)
            .text("model", self.file_model.clone());
        for language in &self.languages {
            form = form.text("languages[]", language.clone());
        }
        let resp = self
            .http
            .post(&self.file_url)
            .bearer_auth(&self.key)
            .multipart(form)
            .send()
            .await
            .context("uploading the recording")?;
        let body = ensure_ok(resp, KEY_ENV)
            .await?
            .text()
            .await
            .context("reading the transcription")?;
        parse_file_response(&body)
    }

    async fn open_live(&self, _label: &str) -> Result<OpenAiLive> {
        let (mut sink, stream) =
            ws::connect(&self.realtime_url, &format!("Bearer {}", self.key)).await?;
        let update = session_update(&self.live_model, &self.languages, self.delay.as_deref());
        sink.send(Message::text(update.to_string()))
            .await
            .context("sending session.update")?;
        let mut partial = String::new();
        let reader = ws::read_until_final(stream, self.trace, move |event| {
            match on_event(event, &mut partial) {
                Step::Continue => ws::Step::Continue,
                Step::Done { text, languages } => ws::Step::Done(Transcript { text, languages }),
                Step::Failed(body) => ws::Step::Failed(body),
            }
        });
        Ok(OpenAiLive(LiveSocket::new(sink, tokio::spawn(reader))))
    }
}

pub struct OpenAiLive(LiveSocket);

impl LiveSession for OpenAiLive {
    async fn send_audio(&mut self, pcm: &[i16]) -> Result<()> {
        let event = json!({
            "type": "input_audio_buffer.append",
            "audio": STANDARD.encode(pcm_bytes(pcm)),
        });
        self.0.send(Message::text(event.to_string())).await
    }

    async fn finish(self) -> Result<Transcript> {
        let commit = json!({"type": "input_audio_buffer.commit"});
        self.0
            .finish(vec![Message::text(commit.to_string())], "the commit")
            .await
    }
}

/// The `session.update` event for a transcription session with manual commit.
fn session_update(model: &str, languages: &[String], delay: Option<&str>) -> Value {
    let mut transcription = json!({"model": model});
    if !languages.is_empty() {
        transcription["languages"] = json!(languages);
    }
    if let Some(delay) = delay {
        transcription["delay"] = json!(delay);
    }
    json!({
        "type": "session.update",
        "session": {
            "type": "transcription",
            "audio": {"input": {
                "format": {"type": "audio/pcm", "rate": SAMPLE_RATE},
                "transcription": transcription,
                "turn_detection": null,
            }},
        },
    })
}

/// What one server event means for the dictation.
#[derive(Debug, PartialEq)]
enum Step {
    Continue,
    Done {
        text: String,
        languages: Vec<String>,
    },
    Failed(String),
}

/// Handles one server event. `partial` collects the deltas, as a backstop for a completion
/// event that carries no transcript.
fn on_event(event: &str, partial: &mut String) -> Step {
    let Ok(value) = serde_json::from_str::<Value>(event) else {
        return Step::Continue;
    };
    match value["type"].as_str().unwrap_or_default() {
        DELTA => {
            partial.push_str(value["delta"].as_str().unwrap_or_default());
            Step::Continue
        }
        COMPLETED => Step::Done {
            text: match value["transcript"].as_str() {
                Some(transcript) => transcript.to_owned(),
                None => std::mem::take(partial),
            },
            languages: language_codes(&value["languages"]),
        },
        FAILED | "error" => Step::Failed(event.to_owned()),
        _ => Step::Continue,
    }
}

/// The body of a successful `/v1/audio/transcriptions` call in the default `json` format.
fn parse_file_response(body: &str) -> Result<Transcript> {
    let value: Value = serde_json::from_str(body)
        .with_context(|| format!("unexpected transcription response: {body}"))?;
    let text = value["text"]
        .as_str()
        .with_context(|| format!("no text in the transcription response: {body}"))?;
    Ok(Transcript {
        text: text.to_owned(),
        languages: language_codes(&value["languages"]),
    })
}

/// Reads `[{"code": "es"}]`. Only `gpt-transcribe` sends it.
fn language_codes(languages: &Value) -> Vec<String> {
    languages
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|language| language["code"].as_str().map(str::to_owned))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures_util::StreamExt;
    use tokio::task::JoinHandle;
    use tokio::time::timeout;

    use super::*;

    #[test]
    fn session_update_without_extras_is_the_json_in_the_plan() {
        // Copied from docs/SPEED_TEST.md, "Pipeline B", step 2.
        let plan: Value = serde_json::from_str(
            r#"{"type": "session.update",
                "session": {"type": "transcription",
                  "audio": {"input": {
                    "format": {"type": "audio/pcm", "rate": 24000},
                    "transcription": {"model": "gpt-live-transcribe"},
                    "turn_detection": null}}}}"#,
        )
        .unwrap();
        assert_eq!(session_update("gpt-live-transcribe", &[], None), plan);
    }

    #[test]
    fn session_update_carries_language_hints_and_delay() {
        let languages = ["es".to_owned(), "en".to_owned()];
        let update = session_update("m", &languages, Some("low"));
        let transcription = &update["session"]["audio"]["input"]["transcription"];
        assert_eq!(transcription["languages"], json!(["es", "en"]));
        assert_eq!(transcription["delay"], "low");
        assert!(update["session"]["audio"]["input"]["turn_detection"].is_null());
    }

    #[test]
    fn deltas_then_completed_give_the_final_transcript() {
        let mut partial = String::new();
        let delta = r#"{"type":"conversation.item.input_audio_transcription.delta","item_id":"item_003","content_index":0,"delta":"Hello,"}"#;
        assert_eq!(on_event(delta, &mut partial), Step::Continue);
        assert_eq!(partial, "Hello,");
        let completed = r#"{"type":"conversation.item.input_audio_transcription.completed","item_id":"item_003","content_index":0,"transcript":"Hello, how are you?"}"#;
        assert_eq!(
            on_event(completed, &mut partial),
            Step::Done {
                text: "Hello, how are you?".to_owned(),
                languages: vec![],
            }
        );
    }

    #[test]
    fn completed_keeps_detected_languages_when_the_model_sends_them() {
        let completed = r#"{"type":"conversation.item.input_audio_transcription.completed","item_id":"i","content_index":0,"transcript":"Bonjour","languages":[{"code":"fr"}]}"#;
        let Step::Done { languages, .. } = on_event(completed, &mut String::new()) else {
            panic!("expected Done");
        };
        assert_eq!(languages, ["fr"]);
    }

    #[test]
    fn completed_without_a_transcript_falls_back_to_the_deltas() {
        let mut partial = "Hola".to_owned();
        let completed =
            r#"{"type":"conversation.item.input_audio_transcription.completed","item_id":"i"}"#;
        let Step::Done { text, .. } = on_event(completed, &mut partial) else {
            panic!("expected Done");
        };
        assert_eq!(text, "Hola");
    }

    #[test]
    fn failed_and_error_events_are_failures_with_their_bodies() {
        let failed = r#"{"type":"conversation.item.input_audio_transcription.failed","event_id":"e","item_id":"i","content_index":0,"error":{"code":"x","message":"bad audio"}}"#;
        assert_eq!(
            on_event(failed, &mut String::new()),
            Step::Failed(failed.to_owned())
        );
        let error = r#"{"type":"error","event_id":"e","error":{"type":"invalid_request_error","message":"buffer is empty"}}"#;
        assert_eq!(
            on_event(error, &mut String::new()),
            Step::Failed(error.to_owned())
        );
    }

    #[test]
    fn other_events_are_ignored() {
        for event in [
            r#"{"type":"session.created"}"#,
            r#"{"type":"session.updated"}"#,
            r#"{"type":"input_audio_buffer.committed","item_id":"i"}"#,
            "not json",
        ] {
            assert_eq!(on_event(event, &mut String::new()), Step::Continue);
        }
    }

    #[test]
    fn file_response_gives_text_and_detected_language() {
        let body = r#"{"text":"Bonjour, pouvez-vous m'entendre ?","languages":[{"code":"fr"}]}"#;
        let transcript = parse_file_response(body).unwrap();
        assert_eq!(transcript.text, "Bonjour, pouvez-vous m'entendre ?");
        assert_eq!(transcript.languages, ["fr"]);
        let plain = parse_file_response(r#"{"text":"hi","usage":{"type":"tokens"}}"#).unwrap();
        assert!(plain.languages.is_empty());
        assert!(parse_file_response(r#"{"error":"x"}"#).is_err());
    }

    fn speech(file_url: String, realtime_url: String) -> OpenAiSpeech {
        OpenAiSpeech {
            http: reqwest::Client::new(),
            key: "test-key".to_owned(),
            file_url,
            realtime_url,
            file_model: "file-model".to_owned(),
            live_model: "live-model".to_owned(),
            languages: vec!["es".to_owned(), "en".to_owned()],
            delay: None,
            trace: false,
        }
    }

    #[tokio::test]
    async fn upload_sends_the_wav_the_model_and_the_language_hints() {
        let (base_url, server) = crate::testserver::serve(vec![crate::testserver::ok(
            "application/json",
            "{\"text\":\"hola equipo\",\"languages\":[{\"code\":\"es\"}]}",
        )])
        .await;
        let stt = speech(format!("{base_url}/audio/transcriptions"), String::new());

        let wav = crate::audio::encode_wav(&[1, 2, 3]);
        let transcript = stt.transcribe_file("es-01", wav).await.unwrap();
        assert_eq!(transcript.text, "hola equipo");
        assert_eq!(transcript.languages, ["es"]);

        let request = server.await.unwrap().remove(0);
        assert!(request.starts_with("POST /v1/audio/transcriptions HTTP/1.1\r\n"));
        let lower = request.to_lowercase();
        assert!(lower.contains("authorization: bearer test-key\r\n"));
        assert!(lower.contains("content-type: multipart/form-data; boundary="));
        assert!(request.contains("name=\"file\"; filename=\"es-01.wav\""));
        assert!(lower.contains("content-type: audio/wav"));
        assert!(request.contains("name=\"model\"\r\n\r\nfile-model\r\n"));
        assert!(request.contains("name=\"languages[]\"\r\n\r\nes\r\n"));
        assert!(request.contains("name=\"languages[]\"\r\n\r\nen\r\n"));
        assert!(request.contains("RIFF"));
    }

    /// A local realtime server: it takes the handshake, then answers each client event with
    /// whatever `reply` returns, until the client goes away. It returns the query string, the
    /// authorization header and every event received.
    // The handshake callback's error type is set by tungstenite.
    #[allow(clippy::result_large_err)]
    async fn realtime_server(
        reply: fn(&Value) -> Vec<Value>,
    ) -> (String, JoinHandle<(String, String, Vec<Value>)>) {
        use tokio_tungstenite::accept_hdr_async;
        use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!(
            "ws://{}/v1/realtime?intent=transcription",
            listener.local_addr().unwrap()
        );
        let handle = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let (mut query, mut auth) = (String::new(), String::new());
            let mut socket = accept_hdr_async(tcp, |request: &Request, response: Response| {
                query = request.uri().query().unwrap_or_default().to_owned();
                auth = request.headers()["authorization"]
                    .to_str()
                    .unwrap()
                    .to_owned();
                Ok(response)
            })
            .await
            .unwrap();
            let mut events = Vec::new();
            while let Some(Ok(Message::Text(text))) = socket.next().await {
                let event: Value = serde_json::from_str(text.as_str()).unwrap();
                for answer in reply(&event) {
                    if socket
                        .send(Message::text(answer.to_string()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                events.push(event);
            }
            (query, auth, events)
        });
        (url, handle)
    }

    #[tokio::test]
    async fn live_session_configures_streams_commits_and_reads_the_transcript() {
        let (url, server) = realtime_server(|event| match event["type"].as_str().unwrap() {
            "session.update" => vec![json!({"type": "session.updated"})],
            "input_audio_buffer.append" => {
                vec![json!({"type": DELTA, "item_id": "item_1", "content_index": 0, "delta": "hola "})]
            }
            "input_audio_buffer.commit" => vec![
                json!({"type": "input_audio_buffer.committed", "item_id": "item_1"}),
                json!({"type": COMPLETED, "item_id": "item_1", "content_index": 0, "transcript": "Hola, hola."}),
            ],
            other => panic!("unexpected client event: {other}"),
        })
        .await;
        let stt = speech(String::new(), url);

        let mut live = stt.open_live("es-01").await.unwrap();
        live.send_audio(&[1, -2]).await.unwrap();
        live.send_audio(&[3]).await.unwrap();
        let transcript = live.finish().await.unwrap();
        assert_eq!(transcript.text, "Hola, hola.");

        let (query, auth, events) = server.await.unwrap();
        assert_eq!(query, "intent=transcription");
        assert_eq!(auth, "Bearer test-key");
        let languages = ["es".to_owned(), "en".to_owned()];
        assert_eq!(
            events,
            [
                session_update("live-model", &languages, None),
                json!({"type": "input_audio_buffer.append", "audio": STANDARD.encode([1, 0, 0xFE, 0xFF])}),
                json!({"type": "input_audio_buffer.append", "audio": STANDARD.encode([3, 0])}),
                json!({"type": "input_audio_buffer.commit"}),
            ]
        );
    }

    #[tokio::test]
    async fn a_rejected_session_stops_the_stream_with_the_provider_message() {
        let (url, _server) = realtime_server(|event| match event["type"].as_str().unwrap() {
            "session.update" => vec![json!({
                "type": "error",
                "event_id": "event_1",
                "error": {"type": "invalid_request_error", "message": "Model live-model is not available."},
            })],
            _ => vec![],
        })
        .await;
        let stt = speech(String::new(), url);

        let mut live = stt.open_live("es-01").await.unwrap();
        let mut failure = None;
        for _ in 0..200 {
            match live.send_audio(&[0; 240]).await {
                Ok(()) => tokio::time::sleep(Duration::from_millis(5)).await,
                Err(err) => {
                    failure = Some(format!("{err:#}"));
                    break;
                }
            }
        }
        let failure = failure.expect("the stream should stop once the session is rejected");
        // The reader's result is taken; committing afterwards reports that and does not panic.
        let after = live.finish().await.expect_err("a failure").to_string();
        assert_eq!(after, "the session has already ended");
        assert!(
            failure.contains("Model live-model is not available."),
            "{failure}"
        );
    }

    #[tokio::test]
    async fn a_failed_transcription_is_reported_at_the_commit() {
        let (url, _server) = realtime_server(|event| match event["type"].as_str().unwrap() {
            "input_audio_buffer.commit" => vec![json!({
                "type": FAILED,
                "item_id": "item_1",
                "content_index": 0,
                "error": {"code": "audio_unintelligible", "message": "could not transcribe"},
            })],
            _ => vec![],
        })
        .await;
        let stt = speech(String::new(), url);

        let mut live = stt.open_live("es-01").await.unwrap();
        live.send_audio(&[0; 240]).await.unwrap();
        let failure = format!("{:#}", live.finish().await.expect_err("a failure"));
        assert!(failure.contains("could not transcribe"), "{failure}");
    }

    #[tokio::test]
    async fn an_abandoned_session_closes_its_socket() {
        let (url, server) = realtime_server(|_| vec![]).await;
        let stt = speech(String::new(), url);

        let mut live = stt.open_live("es-01").await.unwrap();
        live.send_audio(&[0; 240]).await.unwrap();
        drop(live);

        // The server only returns once the client has gone away.
        let (_, _, events) = timeout(Duration::from_secs(5), server)
            .await
            .expect("the socket should close when the session is dropped")
            .unwrap();
        assert_eq!(events.len(), 2);
    }
}
