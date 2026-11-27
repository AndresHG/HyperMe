//! Provider settings, slice 3: choosing a provider and model for a role, adding a custom
//! endpoint and resetting to the recommended pair (`specs/96-provider-settings/spec.md`, R-6 to
//! R-10). Tests cite the spec's requirements and acceptance criteria.

use std::path::Path;

use hyperme_core::config::{
    Config, DEFAULT_CONFIG, Role, add_provider, choose_provider, reset_to_recommended,
};

const BEFORE_CATALOGUE: &str = include_str!("fixtures/config-before-96.toml");

fn parse(text: &str) -> Config {
    Config::parse(text, Path::new("config.toml")).unwrap()
}

/// A second cleanup provider, as the catalogue will have.
const OTHER: &str = r#"
[providers.other]
name = "Other"
protocol = "openai-chat"
endpoint = "https://other.example.com/v1"
key_url = "https://other.example.com/keys"
roles = ["cleanup"]

[[providers.other.models]]
id = "fast-1"
prices_checked = "2026-11-26"

[[providers.other.models]]
id = "thinker-1"
prices_checked = "2026-11-26"
reasoning_effort = ["minimal", "low"]
"#;

/// R-8: a provider and model are saved together, in one valid write that keeps comments.
#[test]
fn choosing_saves_provider_and_model_together() {
    let text = format!("{DEFAULT_CONFIG}{OTHER}");
    let out = choose_provider(&text, Role::Cleanup, "other", "fast-1").unwrap();
    let config = parse(&out);
    assert_eq!(config.cleanup.provider, "other");
    assert_eq!(config.cleanup.model, "fast-1");
    assert_eq!(config.cleanup.endpoint, "https://other.example.com/v1");
    assert_eq!(config.speech.provider, "speechmatics");
    assert!(out.contains("# How much the model changes the transcript"));
}

/// An older file's endpoint and prices belong to its old provider: switching comments them
/// out, so the Groq endpoint never follows the user to another provider.
#[test]
fn switching_drops_the_old_providers_overrides() {
    let text = format!("{BEFORE_CATALOGUE}{OTHER}");
    let out = choose_provider(&text, Role::Cleanup, "other", "fast-1").unwrap();
    let config = parse(&out);
    assert_eq!(config.cleanup.endpoint, "https://other.example.com/v1");
    assert_eq!(config.cleanup.key_url(), "https://other.example.com/keys");
    assert_eq!(config.cleanup_model().price_input_per_million_usd, None);
    // The speech section's overrides are left alone.
    assert!(out.contains("\nendpoint = \"wss://eu.rt.speechmatics.com/v2\""));
    // Choosing another model of the same provider keeps them.
    let out = choose_provider(BEFORE_CATALOGUE, Role::Cleanup, "groq", "llama-x").unwrap();
    assert!(out.contains("\nendpoint = \"https://api.groq.com/openai/v1\""));
}

/// The reasoning effort is one the chosen model takes, or off when it takes none.
#[test]
fn reasoning_effort_follows_the_model() {
    let text = format!("{DEFAULT_CONFIG}{OTHER}");
    let effort = |provider, model| {
        parse(&choose_provider(&text, Role::Cleanup, provider, model).unwrap())
            .cleanup
            .reasoning_effort
    };
    assert_eq!(effort("other", "fast-1"), "off");
    // On another provider the model's first effort, the fastest; on the same provider the
    // current one stays when the model takes it.
    assert_eq!(effort("other", "thinker-1"), "minimal");
    let on_other = choose_provider(&text, Role::Cleanup, "other", "thinker-1").unwrap();
    let low = on_other.replacen(
        "reasoning_effort = \"minimal\"",
        "reasoning_effort = \"low\"",
        1,
    );
    let out = choose_provider(&low, Role::Cleanup, "other", "thinker-1").unwrap();
    assert_eq!(parse(&out).cleanup.reasoning_effort, "low");
    assert_eq!(effort("groq", "openai/gpt-oss-20b"), "low");
    // A model typed by hand on another provider sends no effort it may not take.
    assert_eq!(effort("other", "typed-by-hand"), "off");
}

/// R-7: a role refuses a provider that cannot fill it, and nothing changes.
#[test]
fn choosing_a_provider_of_the_wrong_kind_fails() {
    let err = choose_provider(DEFAULT_CONFIG, Role::Cleanup, "speechmatics", "melia-1")
        .unwrap_err()
        .to_string();
    assert!(err.contains("cleanup.provider"), "{err}");
    let err = choose_provider(DEFAULT_CONFIG, Role::Cleanup, "groq", " ").unwrap_err();
    assert!(err.to_string().contains("cleanup.model"), "{err}");
}

/// R-10: Reset puts every role back to the recommended pair, dropping overrides; keys live in
/// the keychain and are not touched.
#[test]
fn reset_restores_the_recommended_pair() {
    let text = format!("{BEFORE_CATALOGUE}{OTHER}");
    let text = choose_provider(&text, Role::Cleanup, "other", "thinker-1").unwrap();
    let out = reset_to_recommended(&text).unwrap();
    let config = parse(&out);
    let recommended = Config::default();
    assert_eq!(config.cleanup.provider, recommended.cleanup.provider);
    assert_eq!(config.cleanup.model, recommended.cleanup.model);
    assert_eq!(
        config.cleanup.reasoning_effort,
        recommended.cleanup.reasoning_effort
    );
    assert_eq!(config.speech.provider, recommended.speech.provider);
    assert_eq!(config.speech.model, recommended.speech.model);
    assert_eq!(config.cleanup.endpoint, recommended.cleanup.endpoint);
    // A provider the user added stays in the catalogue.
    assert!(config.provider("other").is_some());
}

/// R-7: the recommended provider comes first in a role's menu, marked as such.
#[test]
fn the_recommended_provider_comes_first() {
    let config = parse(&format!("{DEFAULT_CONFIG}{OTHER}"));
    let menu: Vec<&str> = config
        .providers_for(Role::Cleanup)
        .map(|(id, _)| id)
        .collect();
    assert_eq!(menu.first(), Some(&"groq"));
    assert!(menu.contains(&"other"));
    assert_eq!(Config::recommended(Role::Cleanup).0, "groq");
    assert_eq!(Config::recommended(Role::Speech).0, "speechmatics");
}

/// AC-2, design.md "Add a custom endpoint…": a new OpenAI-compatible entry is written to the
/// user's file, appears in the cleanup menu, and can be chosen with no code change.
#[test]
fn ac2_a_custom_endpoint_is_added_and_chosen() {
    let (out, id) =
        add_provider(DEFAULT_CONFIG, "My LLM box", "https://llm.example.com/v1/").unwrap();
    assert_eq!(id, "my-llm-box");
    let config = parse(&out);
    let entry = config.provider(&id).unwrap();
    assert_eq!(entry.name, "My LLM box");
    assert_eq!(entry.endpoint, "https://llm.example.com/v1");
    assert!(config.providers_for(Role::Cleanup).any(|(p, _)| p == id));
    let out = choose_provider(&out, Role::Cleanup, &id, "my-model").unwrap();
    assert_eq!(parse(&out).cleanup.endpoint, "https://llm.example.com/v1");

    // The same name again gets its own entry; a bad address is refused with nothing written.
    let (again, second) = add_provider(&out, "My LLM box", "http://127.0.0.1:8080/v1").unwrap();
    assert_eq!(second, "my-llm-box-2");
    assert!(parse(&again).provider("my-llm-box").is_some());
    assert!(add_provider(DEFAULT_CONFIG, "Bad", "ftp://x").is_err());
    assert!(add_provider(DEFAULT_CONFIG, "  ", "https://x.example.com").is_err());
}
