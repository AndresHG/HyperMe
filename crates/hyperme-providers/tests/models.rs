//! Provider settings, slice 4: the model menu lists the models the key offers
//! (`specs/96-provider-settings/spec.md`, R-15 to R-17). Tests cite AC-7.

mod chat_server;

use std::path::Path;

use chat_server::*;
use hyperme_core::config::{Config, DEFAULT_CONFIG};
use hyperme_core::secrets::{MemorySecretStore, SecretStore};
use hyperme_providers::models::model_menu;

const KEY: &str = "not-a-real-key-for-tests";

/// A cleanup provider at `base` with two catalogue models, listing on.
fn config(base: &str) -> Config {
    let text = format!(
        "{DEFAULT_CONFIG}\n[providers.box]\nname = \"Box\"\nprotocol = \"openai-chat\"\n\
         endpoint = \"{base}\"\nroles = [\"cleanup\"]\nlist_models = true\n\
         hide_models = [\"embed\", \"whisper\"]\n\
         [[providers.box.models]]\nid = \"fast-1\"\n[[providers.box.models]]\nid = \"smart-1\"\n"
    );
    Config::parse(&text, Path::new("config.toml")).unwrap()
}

fn keys() -> MemorySecretStore {
    let store = MemorySecretStore::default();
    store.set("box", KEY).unwrap();
    store
}

fn listing(ids: &[&str]) -> Answer {
    let data: Vec<_> = ids
        .iter()
        .map(|id| serde_json::json!({"id": id, "object": "model"}))
        .collect();
    answer(
        "200 OK",
        "application/json",
        &serde_json::json!({"object": "list", "data": data}).to_string(),
    )
}

/// AC-7: with a working key the menu lists the server's models, the catalogue's first, without
/// the ones the catalogue hides (embeddings, speech) and without repeats.
#[tokio::test]
async fn ac7_lists_the_keys_models_catalogue_first() {
    let (base, requests) = serve(
        listing(&["zeta-2", "smart-1", "text-embed-3", "whisper-1", "alpha-9"]),
        listing(&[]),
    )
    .await;
    let menu = model_menu(&config(&base), "box", &keys()).await;
    assert_eq!(menu.models, ["fast-1", "smart-1", "alpha-9", "zeta-2"]);
    assert_eq!(menu.notice, None);
    let seen = requests.lock().unwrap();
    assert!(seen[0].starts_with("GET /v1/models"));
    assert!(seen[0].contains(&format!("Bearer {KEY}")));
}

/// AC-7, R-17: when the listing fails, the catalogue's models and a one-line notice.
#[tokio::test]
async fn ac7_a_failed_listing_falls_back_to_the_catalogue() {
    let refused = answer(
        "401 Unauthorized",
        "application/json",
        r#"{"error":"bad key"}"#,
    );
    let (base, _) = serve(refused.clone(), refused).await;
    let menu = model_menu(&config(&base), "box", &keys()).await;
    assert_eq!(menu.models, ["fast-1", "smart-1"]);
    let notice = menu.notice.unwrap();
    assert!(notice.contains("catalogue"), "{notice}");
    assert!(!notice.contains(KEY));
    assert!(!notice.contains('\n'));

    // No server at all.
    let menu = model_menu(&config("http://127.0.0.1:1/v1"), "box", &keys()).await;
    assert_eq!(menu.models, ["fast-1", "smart-1"]);
    assert!(menu.notice.is_some());
}

/// R-15, R-30: no key, or a provider that cannot list, sends nothing and shows the catalogue.
#[tokio::test]
async fn without_a_key_nothing_is_sent() {
    let (base, requests) = serve(listing(&["x"]), listing(&[])).await;
    let menu = model_menu(&config(&base), "box", &MemorySecretStore::default()).await;
    assert_eq!(menu.models, ["fast-1", "smart-1"]);
    assert_eq!(menu.notice, None);
    assert!(requests.lock().unwrap().is_empty());

    // Speech providers list from the catalogue only.
    let config = Config::default();
    let menu = model_menu(&config, "speechmatics", &keys()).await;
    assert_eq!(menu.models, ["melia-1"]);
}

/// The menu is fetched from Settings on Tauri's async runtime, so its future must be Send.
#[test]
fn the_menu_future_is_send() {
    fn send<T: Send>(_: T) {}
    let config = Config::default();
    let keys = MemorySecretStore::default();
    send(model_menu(&config, "groq", &keys));
}
