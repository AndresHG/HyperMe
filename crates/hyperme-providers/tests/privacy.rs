//! Provider settings, slice 11: choosing providers sends nothing until Test or a dictation
//! (`specs/96-provider-settings/spec.md`, R-30, AC-14), checked with servers that count every
//! connection they get.

mod chat_server;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use chat_server::*;
use hyperme_core::config::{self, Config, DEFAULT_CONFIG, Role};
use hyperme_core::secrets::{MemorySecretStore, SecretStore};
use hyperme_providers::any::{cleanup, cleanup_fallback, resolve};

const KEY: &str = "not-a-real-key-for-tests";

/// A TCP listener that only counts connections, standing in for a speech provider.
async fn counting() -> (String, Arc<AtomicUsize>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/v2", listener.local_addr().unwrap());
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    tokio::spawn(async move {
        while listener.accept().await.is_ok() {
            seen.fetch_add(1, Ordering::SeqCst);
        }
    });
    (url, count)
}

fn entry(id: &str, protocol: &str, endpoint: &str, role: &str) -> String {
    format!(
        "\n[providers.{id}]\nname = \"{id}\"\nprotocol = \"{protocol}\"\nendpoint = \"{endpoint}\"\n\
         roles = [\"{role}\"]\n"
    )
}

/// AC-14: picking providers and fallbacks for both roles, saving them and building the
/// providers the next dictation uses (what Settings does on every change) opens no connection
/// to any of them. Test is the first thing that does.
#[tokio::test]
async fn ac14_choosing_providers_sends_nothing_until_test() {
    let (speech_a, speech_a_hits) = counting().await;
    let (speech_b, speech_b_hits) = counting().await;
    let (chat_a, chat_a_seen) = serve(models(), streamed(&["ok"])).await;
    let (chat_b, chat_b_seen) = serve(models(), streamed(&["ok"])).await;
    let mut text = DEFAULT_CONFIG.to_owned();
    text.push_str(&entry("sa", "speechmatics-rt", &speech_a, "speech"));
    text.push_str(&entry("sb", "openai-realtime", &speech_b, "speech"));
    text.push_str(&entry("ca", "openai-chat", &chat_a, "cleanup"));
    text.push_str(&entry("cb", "openai-chat", &chat_b, "cleanup"));
    let keys = MemorySecretStore::default();
    for id in ["sa", "sb", "ca", "cb", "speechmatics", "groq"] {
        keys.set(id, KEY).unwrap();
    }

    let steps: [&dyn Fn(&str) -> String; 4] = [
        &|t| config::choose_provider(t, Role::Speech, "sa", "m").unwrap(),
        &|t| config::choose_provider(t, Role::Cleanup, "ca", "m").unwrap(),
        &|t| config::set_fallback(t, Role::Speech, Some(("sb", "m"))).unwrap(),
        &|t| config::set_fallback(t, Role::Cleanup, Some(("cb", "m"))).unwrap(),
    ];
    for step in steps {
        text = step(&text);
        let config = Config::parse(&text, Path::new("config.toml")).unwrap();
        let built = resolve(&config, &keys).unwrap();
        drop(built);
    }
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let hits = || {
        (
            speech_a_hits.load(Ordering::SeqCst),
            speech_b_hits.load(Ordering::SeqCst),
            chat_a_seen.lock().unwrap().len(),
            chat_b_seen.lock().unwrap().len(),
        )
    };
    assert_eq!(hits(), (0, 0, 0, 0), "choosing must send nothing");

    // Test is the first request, and only to the provider tested.
    let config = Config::parse(&text, Path::new("config.toml")).unwrap();
    cleanup(&config, &keys).unwrap().check().await.unwrap();
    assert_eq!(hits(), (0, 0, 1, 0));
    cleanup_fallback(&config, &keys)
        .unwrap()
        .check()
        .await
        .unwrap();
    assert_eq!(hits(), (0, 0, 1, 1));
}
