//! Provider settings, slice 9: the cleanup fallback with the real client against two local
//! OpenAI-compatible servers (`specs/96-provider-settings/spec.md`, AC-10, AC-10a).

mod chat_server;

use std::path::Path;

use chat_server::*;
use hyperme_core::config::{self, Config, DEFAULT_CONFIG, Role};
use hyperme_core::secrets::{MemorySecretStore, SecretStore};
use hyperme_core::traits::CleanupProvider;
use hyperme_providers::any::{cleanup_fallback, cleanup_with_fallback, fallback_ready};

const KEY: &str = "not-a-real-key-for-tests";

/// The main cleanup at `main`, and a hand-added fallback provider `spare` at `spare`.
fn config(main: &str, spare: &str) -> Config {
    let base = DEFAULT_CONFIG.replacen(
        "provider = \"groq\"",
        &format!("provider = \"groq\"\nendpoint = \"{main}\""),
        1,
    );
    let text = format!(
        "{base}\n[providers.spare]\nname = \"Spare\"\nprotocol = \"openai-chat\"\n\
         endpoint = \"{spare}\"\nroles = [\"cleanup\"]\n"
    );
    let text = config::set_fallback(&text, Role::Cleanup, Some(("spare", "tiny"))).unwrap();
    Config::parse(&text, Path::new("config.toml")).unwrap()
}

fn keys(providers: &[&str]) -> MemorySecretStore {
    let store = MemorySecretStore::default();
    for p in providers {
        store.set(p, KEY).unwrap();
    }
    store
}

/// AC-10: the main host answers 500; the fallback cleans the dictation and is reported.
#[tokio::test]
async fn ac10_a_500_from_the_main_host_is_cleaned_by_the_fallback() {
    let (main, _) = serve(
        models(),
        answer("500 Internal Server Error", "text/plain", "down"),
    )
    .await;
    let (spare, seen) = serve(models(), streamed(&["Hello ", "there."])).await;
    let config = config(&main, &spare);
    let cleanup = cleanup_with_fallback(&config, &keys(&["groq", "spare"])).unwrap();
    let mut text = String::new();
    cleanup
        .clean("hello there", None, &mut |p| text.push_str(p))
        .await
        .unwrap();
    assert_eq!(text, "Hello there.");
    let via = cleanup.fallback_used().unwrap();
    assert_eq!(
        (via.provider.as_str(), via.name.as_str()),
        ("spare", "Spare")
    );
    assert!(
        seen.lock()
            .unwrap()
            .iter()
            .any(|r| r.contains("\"model\":\"tiny\""))
    );
}

/// AC-10a: a fallback whose key was deleted is skipped: the main error comes back as today,
/// and the Setup check sees the fallback as not ready. Its Test fails, so it cannot be saved.
#[tokio::test]
async fn ac10a_a_fallback_without_its_key_is_skipped_and_fails_test() {
    let (main, _) = serve(
        models(),
        answer("500 Internal Server Error", "text/plain", "down"),
    )
    .await;
    let (spare, seen) = serve(models(), streamed(&["never"])).await;
    let config = config(&main, &spare);
    let keys = keys(&["groq"]);
    let cleanup = cleanup_with_fallback(&config, &keys).unwrap();
    assert!(cleanup.fallback().is_none());
    let result = cleanup.clean("hello there", None, &mut |_| {}).await;
    assert!(result.is_err());
    assert!(
        seen.lock().unwrap().is_empty(),
        "nothing goes to a fallback with no key"
    );
    assert_eq!(fallback_ready(&config, Role::Cleanup, &keys), Some(false));
    assert!(cleanup_fallback(&config, &keys).is_err());
    assert_eq!(
        fallback_ready(&Config::default(), Role::Cleanup, &keys),
        None
    );
}
