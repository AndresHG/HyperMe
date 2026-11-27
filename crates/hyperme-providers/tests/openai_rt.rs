//! Provider settings, slice 8: OpenAI streaming speech (`openai-realtime`) against a local
//! WebSocket server, so the real client runs with no network and no key. Tests cite
//! `specs/96-provider-settings/spec.md` (AC-11, speech half; R-13) and Phase 1's R-10.

mod common;

use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use common::{Got, Reply, serve};
use hyperme_core::config::{Config, DEFAULT_CONFIG};
use hyperme_core::secrets::{MemorySecretStore, SecretStore};
use hyperme_core::traits::{ErrorKind, LiveSession, SpeechProvider};
use hyperme_providers::any::{self, AnySpeech};
use hyperme_providers::openai_rt::OpenAiRealtime;
use serde_json::json;

const KEY: &str = "not-a-real-key-for-tests";
const DELTA: &str = "conversation.item.input_audio_transcription.delta";
const COMPLETED: &str = "conversation.item.input_audio_transcription.completed";
const FAILED: &str = "conversation.item.input_audio_transcription.failed";

/// The default config with OpenAI speech at `endpoint`.
fn config(endpoint: &str) -> Config {
    let text = DEFAULT_CONFIG
        .replacen(
            "provider = \"speechmatics\"",
            "provider = \"openai-speech\"",
            1,
        )
        .replacen(
            "model = \"melia-1\"",
            &format!("model = \"gpt-live-transcribe\"\nendpoint = \"{endpoint}\""),
            1,
        );
    Config::parse(&text, Path::new("config.toml")).unwrap()
}

fn speech(endpoint: &str) -> OpenAiRealtime {
    OpenAiRealtime::new(&config(endpoint).speech, KEY.to_owned())
}

fn typed(got: &Got, kind: &str) -> bool {
    matches!(got, Got::Json(v) if v["type"] == kind)
}

/// Answers `session.update` with `session.updated`; other events go to `reply`.
fn updated_then(
    mut reply: impl FnMut(&Got) -> Vec<Reply> + Send + 'static,
) -> impl FnMut(&Got) -> Vec<Reply> + Send + 'static {
    move |got| {
        if typed(got, "session.update") {
            vec![Reply::Json(json!({"type": "session.updated"}))]
        } else {
            reply(got)
        }
    }
}

fn appended(got: &[Got]) -> Vec<u8> {
    got.iter()
        .filter_map(|g| match g {
            Got::Json(v) if v["type"] == "input_audio_buffer.append" => {
                Some(STANDARD.decode(v["audio"].as_str().unwrap()).unwrap())
            }
            _ => None,
        })
        .flatten()
        .collect()
}

/// AC-11 (speech half), R-10: the session is configured as the current docs say, with no
/// language; every captured sample reaches the server; commit returns the transcript.
#[tokio::test]
async fn streams_every_frame_and_returns_the_completed_transcript() {
    let (base, server) = serve(
        None,
        updated_then(|got| {
            if typed(got, "input_audio_buffer.append") {
                vec![Reply::Json(
                    json!({"type": DELTA, "item_id": "i", "delta": "Hola "}),
                )]
            } else if typed(got, "input_audio_buffer.commit") {
                vec![
                    Reply::Json(json!({"type": "input_audio_buffer.committed", "item_id": "i"})),
                    Reply::Json(
                        json!({"type": COMPLETED, "item_id": "i", "transcript": "Hola, equipo."}),
                    ),
                ]
            } else {
                vec![]
            }
        }),
    )
    .await;
    let mut session = speech(&base).open().await.unwrap();
    let samples: Vec<i16> = (0..7200).map(|i| (i % 300) as i16 - 150).collect();
    session.send_audio(&samples).await.unwrap();
    session.send_audio(&[1, -2]).await.unwrap();
    let transcript = session.finish().await.unwrap();
    assert_eq!(transcript.text, "Hola, equipo.");
    assert_eq!(transcript.language, None);

    let seen = server.await.unwrap();
    assert_eq!(seen.auth, format!("Bearer {KEY}"));
    let Got::Json(update) = &seen.got[0] else {
        panic!("session.update comes first");
    };
    let input = &update["session"]["audio"]["input"];
    assert_eq!(update["session"]["type"], "transcription");
    assert_eq!(input["format"], json!({"type": "audio/pcm", "rate": 24000}));
    assert_eq!(
        input["transcription"],
        json!({"model": "gpt-live-transcribe"})
    );
    assert!(input["turn_detection"].is_null());
    assert!(input.get("turn_detection").is_some());
    let expected: Vec<u8> = samples
        .iter()
        .chain([1, -2].iter())
        .flat_map(|s| s.to_le_bytes())
        .collect();
    assert_eq!(appended(&seen.got), expected);
    let commit = seen
        .got
        .iter()
        .position(|g| typed(g, "input_audio_buffer.commit"))
        .expect("a commit");
    let last_append = seen
        .got
        .iter()
        .rposition(|g| typed(g, "input_audio_buffer.append"))
        .unwrap();
    assert!(commit > last_append, "the commit follows the audio");
}

/// R-13: open waits until the session is configured, so Test fails on a model the key cannot
/// use, with the provider's message and no key.
#[tokio::test]
async fn a_refused_session_update_fails_open() {
    let (base, _server) = serve(None, |got| {
        if typed(got, "session.update") {
            vec![Reply::Json(json!({"type": "error", "error": {
                "type": "invalid_request_error", "code": "model_not_found",
                "message": "The model nope does not exist."}}))]
        } else {
            vec![]
        }
    })
    .await;
    let error = speech(&base).open().await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::Other);
    assert!(
        error.message.contains("does not exist"),
        "{}",
        error.message
    );
    assert!(!error.message.contains(KEY));
}

/// AC-11: a key refused on the handshake is a key error naming the provider, without the key.
#[tokio::test]
async fn a_401_is_a_key_error() {
    let (base, _server) = serve(Some(401), |_| vec![]).await;
    let error = speech(&base).open().await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::Auth);
    assert!(error.message.contains("openai-speech"), "{}", error.message);
    assert!(!error.message.contains(KEY));
}

/// An invalid key reported in an error event is also a key error.
#[tokio::test]
async fn an_invalid_key_event_is_a_key_error() {
    let (base, _server) = serve(None, |_| {
        vec![Reply::Json(json!({"type": "error", "error": {
            "type": "invalid_request_error", "code": "invalid_api_key",
            "message": "Incorrect API key provided."}}))]
    })
    .await;
    let error = speech(&base).open().await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::Auth);
}

/// A failed transcription is reported at the commit.
#[tokio::test]
async fn a_failed_transcription_is_reported_on_finish() {
    let (base, _server) = serve(
        None,
        updated_then(|got| {
            if typed(got, "input_audio_buffer.commit") {
                vec![Reply::Json(json!({"type": FAILED, "item_id": "i",
                    "error": {"code": "audio_unintelligible", "message": "could not transcribe"}}))]
            } else {
                vec![]
            }
        }),
    )
    .await;
    let mut session = speech(&base).open().await.unwrap();
    session.send_audio(&[0; 480]).await.unwrap();
    let error = session.finish().await.unwrap_err();
    assert!(
        error.message.contains("could not transcribe"),
        "{}",
        error.message
    );
}

/// A connection dropped mid-dictation fails the next send or the finish, as a network error.
#[tokio::test]
async fn a_dropped_connection_is_a_network_error() {
    let (base, _server) = serve(
        None,
        updated_then(|got| {
            if typed(got, "input_audio_buffer.append") {
                vec![Reply::Drop]
            } else {
                vec![]
            }
        }),
    )
    .await;
    let mut session = speech(&base).open().await.unwrap();
    let mut failed = None;
    for _ in 0..50 {
        if let Err(e) = session.send_audio(&[0; 480]).await {
            failed = Some(e);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    let error = match failed {
        Some(e) => e,
        None => session.finish().await.unwrap_err(),
    };
    assert_eq!(error.kind, ErrorKind::Network, "{}", error.message);
}

/// D-2: `resolve` builds OpenAI speech with the one OpenAI key, through the same path as Test.
#[test]
fn resolve_builds_openai_speech_with_the_shared_openai_key() {
    let keys = MemorySecretStore::default();
    keys.set("groq", KEY).unwrap();
    let config = config("wss://api.openai.com/v1/realtime?intent=transcription");
    let missing = any::resolve(&config, &keys).unwrap_err();
    assert!(missing.message.contains("openai"), "{}", missing.message);
    keys.set("openai", KEY).unwrap();
    let providers = any::resolve(&config, &keys).unwrap();
    assert!(matches!(**providers.speech.main(), AnySpeech::OpenAi(_)));
    assert!(any::key_ready(&config, "openai-speech", &keys));
}
