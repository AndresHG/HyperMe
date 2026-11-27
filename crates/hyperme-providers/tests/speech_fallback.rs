//! Provider settings, slice 10: the speech fallback with the real clients, Speechmatics as the
//! main provider and OpenAI streaming speech as the fallback, each against a local WebSocket
//! server (`specs/96-provider-settings/spec.md`, AC-11, AC-12, AC-10a for speech).

mod common;

use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use common::{Got, Reply, serve, started_then};
use hyperme_core::config::{self, Config, DEFAULT_CONFIG, Role};
use hyperme_core::secrets::{MemorySecretStore, SecretStore};
use hyperme_core::traits::{LiveSession, SpeechProvider};
use hyperme_providers::any::{fallback_ready, speech_fallback, speech_with_fallback};
use serde_json::json;

const KEY: &str = "not-a-real-key-for-tests";

fn typed(got: &Got, kind: &str) -> bool {
    matches!(got, Got::Json(v) if v["type"] == kind)
}

/// An OpenAI realtime server that configures, collects appends, and answers the commit.
fn openai_server(got: &Got) -> Vec<Reply> {
    if typed(got, "session.update") {
        vec![Reply::Json(json!({"type": "session.updated"}))]
    } else if typed(got, "input_audio_buffer.commit") {
        vec![Reply::Json(
            json!({"type": "conversation.item.input_audio_transcription.completed",
            "item_id": "i", "transcript": "Hola desde el respaldo."}),
        )]
    } else {
        vec![]
    }
}

fn config(main: &str, spare: &str) -> Config {
    let base = DEFAULT_CONFIG.replacen(
        "model = \"melia-1\"",
        &format!("model = \"melia-1\"\nendpoint = \"{main}\""),
        1,
    );
    let text = format!(
        "{base}\n[providers.spare]\nname = \"Spare\"\nprotocol = \"openai-realtime\"\n\
         endpoint = \"{spare}\"\nroles = [\"speech\"]\n"
    );
    let text =
        config::set_fallback(&text, Role::Speech, Some(("spare", "gpt-live-transcribe"))).unwrap();
    Config::parse(&text, Path::new("config.toml")).unwrap()
}

fn keys(providers: &[&str]) -> MemorySecretStore {
    let store = MemorySecretStore::default();
    for p in providers {
        store.set(p, KEY).unwrap();
    }
    store
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

fn audio() -> Vec<Vec<i16>> {
    (1..=6).map(|i| vec![i as i16; 480]).collect()
}

fn bytes(frames: &[Vec<i16>]) -> Vec<u8> {
    frames
        .concat()
        .iter()
        .flat_map(|s| s.to_le_bytes())
        .collect()
}

/// AC-11: Speechmatics refuses the connection; OpenAI receives every captured frame and the
/// text comes back naming it.
#[tokio::test]
async fn ac11_a_refused_main_provider_is_transcribed_by_the_fallback() {
    let (main, _) = serve(Some(503), |_| vec![]).await;
    let (spare, server) = serve(None, openai_server).await;
    let speech =
        speech_with_fallback(&config(&main, &spare), &keys(&["speechmatics", "spare"])).unwrap();
    let mut session = speech.open().await.unwrap();
    for frame in audio() {
        session.send_audio(&frame).await.unwrap();
    }
    let transcript = session.finish().await.unwrap();
    assert_eq!(transcript.text, "Hola desde el respaldo.");
    assert_eq!(transcript.via.unwrap().name, "Spare");
    assert_eq!(appended(&server.await.unwrap().got), bytes(&audio()));
}

/// AC-12: the Speechmatics connection drops after the second frame; on release the saved
/// audio goes to OpenAI and its text comes back.
#[tokio::test]
async fn ac12_a_dropped_main_session_sends_the_saved_audio_on_release() {
    let mut frames = 0;
    let (main, _) = serve(
        None,
        started_then(move |got| {
            if matches!(got, Got::Binary(_)) {
                frames += 1;
                if frames == 2 {
                    return vec![Reply::Drop];
                }
            }
            vec![]
        }),
    )
    .await;
    let (spare, server) = serve(None, openai_server).await;
    let speech =
        speech_with_fallback(&config(&main, &spare), &keys(&["speechmatics", "spare"])).unwrap();
    let mut session = speech.open().await.unwrap();
    for frame in audio() {
        session.send_audio(&frame).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    let transcript = session.finish().await.unwrap();
    assert_eq!(transcript.text, "Hola desde el respaldo.");
    assert_eq!(appended(&server.await.unwrap().got), bytes(&audio()));
}

/// AC-10a for speech: a fallback without its key is skipped and fails its Test.
#[test]
fn a_speech_fallback_without_its_key_is_skipped() {
    let config = config("ws://127.0.0.1:9/v2", "ws://127.0.0.1:9/v1");
    let keys = keys(&["speechmatics"]);
    assert!(
        speech_with_fallback(&config, &keys)
            .unwrap()
            .fallback()
            .is_none()
    );
    assert_eq!(fallback_ready(&config, Role::Speech, &keys), Some(false));
    assert!(speech_fallback(&config, &keys).is_err());
}
