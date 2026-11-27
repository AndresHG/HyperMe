//! Provider settings, slice 7: a cleanup server on this Mac (`specs/96-provider-settings`,
//! R-21 to R-23). Tests cite AC-9.

mod chat_server;

use std::path::Path;

use chat_server::*;
use hyperme_core::config::{Config, DEFAULT_CONFIG, Role, choose_provider};
use hyperme_core::hints::hint;
use hyperme_core::secrets::MemorySecretStore;
use hyperme_core::traits::CleanupProvider;
use hyperme_providers::any::{key_ready, resolve};
use hyperme_providers::local::discover;
use tokio::net::TcpListener;

fn listing(ids: &[&str]) -> Answer {
    let data: Vec<_> = ids
        .iter()
        .map(|id| serde_json::json!({ "id": id }))
        .collect();
    answer(
        "200 OK",
        "application/json",
        &serde_json::json!({ "object": "list", "data": data }).to_string(),
    )
}

/// A port nothing listens on.
async fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap().port()
}

/// R-21: the catalogue has a Local server for cleanup that needs no key and lists its models.
#[test]
fn the_catalogue_has_a_local_server() {
    let config = Config::default();
    let local = config.provider("local").unwrap();
    assert!(!local.key_required);
    assert!(local.list_models);
    assert!(local.endpoint.starts_with("http://127.0.0.1:"));
    assert!(!local.ports.is_empty());
    let menu: Vec<&str> = config
        .providers_for(Role::Cleanup)
        .map(|(id, _)| id)
        .collect();
    assert_eq!(menu.last(), Some(&"local"), "{menu:?}");
    assert!(key_ready(&config, "local", &MemorySecretStore::default()));
}

/// AC-9, R-22: a server on one of the usual ports is found with its models; closed ports are
/// skipped quickly.
#[tokio::test]
async fn ac9_finds_a_server_on_a_usual_port() {
    let (base, _) = serve(listing(&["llama3.2:3b", "qwen3:4b"]), listing(&[])).await;
    let port: u16 = base
        .trim_start_matches("http://127.0.0.1:")
        .trim_end_matches("/v1")
        .parse()
        .unwrap();
    let closed = closed_port().await;
    let started = std::time::Instant::now();
    let found = discover(&[closed, port]).await;
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].port, port);
    assert_eq!(found[0].endpoint, base);
    assert_eq!(found[0].models, ["llama3.2:3b", "qwen3:4b"]);
}

/// AC-9: chosen with a model, the local server cleans a dictation with no key.
#[tokio::test]
async fn ac9_cleans_with_no_key() {
    let (base, requests) = serve(listing(&["tiny"]), streamed(&["Hello."])).await;
    let text = choose_provider(DEFAULT_CONFIG, Role::Cleanup, "local", "tiny").unwrap();
    let text =
        hyperme_core::config::set_value(&text, "cleanup", "endpoint", &format!("\"{base}\""))
            .unwrap();
    let config = Config::parse(&text, Path::new("config.toml")).unwrap();
    let mut keys = MemorySecretStore::default();
    use hyperme_core::secrets::SecretStore;
    keys.set("speechmatics", "speech-key").unwrap();
    let providers = resolve(&config, &keys).unwrap();
    let mut out = String::new();
    providers
        .cleanup
        .clean("hello", None, &mut |t| out.push_str(t))
        .await
        .unwrap();
    assert_eq!(out, "Hello.");
    assert!(
        requests
            .lock()
            .unwrap()
            .iter()
            .all(|r| !r.to_lowercase().contains("authorization"))
    );
    let _ = &mut keys;
}

/// R-23: the hint says it runs on this Mac and nothing leaves it for cleanup.
#[test]
fn the_hint_says_it_runs_on_this_mac() {
    let config = Config::default();
    let h = hint(&config, Role::Cleanup, "local", "tiny", 30, None);
    assert!(h.cost.contains("Runs on this Mac"), "{}", h.cost);
    assert!(h.cost.contains("nothing leaves it"), "{}", h.cost);
}

/// R-23: only an address on this Mac counts as local.
#[test]
fn only_addresses_on_this_mac_are_local() {
    use hyperme_core::config::on_this_mac;
    for local in [
        "http://127.0.0.1:11434/v1",
        "http://localhost:1234/v1",
        "http://[::1]:8080/v1",
    ] {
        assert!(on_this_mac(local), "{local}");
    }
    for remote in [
        "https://api.groq.com/openai/v1",
        "http://127.0.0.1.example.com/v1",
        "http://localhost.example.com:8080/v1",
        "http://192.168.1.20:11434/v1",
    ] {
        assert!(!on_this_mac(remote), "{remote}");
    }
}

/// AC-9: with Local server chosen at a found address, the model menu lists that server's
/// models, not the catalogue address's.
#[tokio::test]
async fn ac9_lists_the_chosen_servers_models() {
    let (base, _) = serve(listing(&["qwen3:4b"]), listing(&[])).await;
    let text = choose_provider(DEFAULT_CONFIG, Role::Cleanup, "local", "qwen3:4b").unwrap();
    let text =
        hyperme_core::config::set_value(&text, "cleanup", "endpoint", &format!("\"{base}\""))
            .unwrap();
    let config = Config::parse(&text, Path::new("config.toml")).unwrap();
    let menu =
        hyperme_providers::models::model_menu(&config, "local", &MemorySecretStore::default())
            .await;
    assert_eq!(menu.notice, None);
    assert_eq!(menu.models, ["qwen3:4b"]);
}
