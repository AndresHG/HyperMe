//! Provider settings, slice 2: one `resolve` path builds the providers a config names, for the
//! runtime and for Test (`specs/96-provider-settings`, design.md, "Enum dispatch").

mod chat_server;

use std::path::Path;

use chat_server::*;
use hyperme_core::config::{Config, DEFAULT_CONFIG};
use hyperme_core::secrets::{MemorySecretStore, SecretStore};
use hyperme_core::traits::{CleanupProvider, ErrorKind};
use hyperme_providers::any::{AnyCleanup, AnySpeech, key_ready, resolve};

const KEY: &str = "not-a-real-key-for-tests";

fn keys(providers: &[&str]) -> MemorySecretStore {
    let store = MemorySecretStore::default();
    for p in providers {
        store.set(p, KEY).unwrap();
    }
    store
}

/// A config whose cleanup is a hand-added server at `base`.
fn with_server(base: &str, key_required: bool) -> Config {
    let text = format!(
        "{}\n[providers.myserver]\nname = \"My server\"\nprotocol = \"openai-chat\"\n\
         endpoint = \"{base}\"\nroles = [\"cleanup\"]\nkey_required = {key_required}\n",
        DEFAULT_CONFIG.replacen("provider = \"groq\"", "provider = \"myserver\"", 1)
    );
    Config::parse(&text, Path::new("config.toml")).unwrap()
}

#[test]
fn resolves_the_recommended_pair_by_protocol() {
    let providers = resolve(&Config::default(), &keys(&["speechmatics", "groq"])).unwrap();
    assert!(matches!(providers.speech, AnySpeech::Speechmatics(_)));
    assert!(matches!(providers.cleanup, AnyCleanup::Chat(_)));
}

/// R-12: a provider with no key is not built, and nothing is half-built.
#[test]
fn a_missing_key_names_the_provider() {
    let error = resolve(&Config::default(), &keys(&["speechmatics"])).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Auth);
    assert!(error.message.contains("groq"), "{}", error.message);
}

/// AC-2: a provider added by hand cleans a dictation through the same path, with no code.
#[tokio::test]
async fn a_hand_added_provider_cleans_through_resolve() {
    let (base, requests) = serve(models(), streamed(&["Hello ", "there."])).await;
    let config = with_server(&base, true);
    let providers = resolve(&config, &keys(&["speechmatics", "myserver"])).unwrap();
    let mut text = String::new();
    providers
        .cleanup
        .clean("hello there", None, &mut |piece| text.push_str(piece))
        .await
        .unwrap();
    assert_eq!(text, "Hello there.");
    let seen = requests.lock().unwrap();
    assert!(seen.iter().any(|r| r.contains(&format!("Bearer {KEY}"))));
}

/// R-14: a provider that needs no key resolves without one.
#[test]
fn a_keyless_provider_needs_no_key() {
    let config = with_server("http://127.0.0.1:9/v1", false);
    let providers = resolve(&config, &keys(&["speechmatics"])).unwrap();
    assert!(matches!(providers.cleanup, AnyCleanup::Chat(_)));
}

/// AC-5: a provider is ready only with a key, unless it needs none, so Settings keeps the old
/// provider until the new one has a key that passed Test.
#[test]
fn ac5_a_provider_without_a_key_is_not_ready() {
    let config = with_server("http://127.0.0.1:9/v1", true);
    assert!(!key_ready(&config, "myserver", &keys(&["speechmatics"])));
    assert!(key_ready(&config, "myserver", &keys(&["myserver"])));
    let keyless = with_server("http://127.0.0.1:9/v1", false);
    assert!(key_ready(&keyless, "myserver", &keys(&[])));
}
