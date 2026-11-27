//! Provider settings, slice 1: the provider catalogue in the config
//! (`specs/96-provider-settings/spec.md`, R-1 to R-5). Tests cite the spec's acceptance criteria.

use std::path::Path;

use hyperme_core::config::{
    Config, ConfigError, DEFAULT_CONFIG, DEFAULT_PROVIDERS, Protocol, Role,
};
use hyperme_core::cost::{cleanup_usd, monthly_cost, speech_usd};

/// A config file as HyperMe wrote it before the catalogue existed: endpoints, key pages and
/// prices inside `[speech]` and `[cleanup]`.
const BEFORE_CATALOGUE: &str = include_str!("fixtures/config-before-96.toml");

fn parse(text: &str) -> Result<Config, ConfigError> {
    Config::parse(text, Path::new("/tmp/hyperme/config.toml"))
}

/// A cleanup server added by hand, as a user would for a self-hosted endpoint.
const HAND_ADDED: &str = r#"
[providers.myserver]
name = "My server"
protocol = "openai-chat"
endpoint = "http://127.0.0.1:9999/v1"
roles = ["cleanup"]
key_required = false

[[providers.myserver.models]]
id = "tiny"
price_input_per_million_usd = 0
price_output_per_million_usd = 0
"#;

/// AC-1: a file from before this change loads unchanged and works exactly as before.
#[test]
fn ac1_a_config_from_before_the_catalogue_loads_unchanged() {
    let old = parse(BEFORE_CATALOGUE).unwrap();
    let new = Config::default();
    assert_eq!(old.speech.provider, new.speech.provider);
    assert_eq!(old.speech.model, new.speech.model);
    assert_eq!(old.speech.endpoint, new.speech.endpoint);
    assert_eq!(old.speech.key_url(), new.speech.key_url());
    assert_eq!(old.cleanup.endpoint, new.cleanup.endpoint);
    assert_eq!(old.cleanup.key_url(), new.cleanup.key_url());
    for minutes in [10, 30, 60] {
        assert_eq!(monthly_cost(&old, minutes), monthly_cost(&new, minutes));
    }
}

/// AC-1: the catalogue appears with the defaults, though the old file names none.
#[test]
fn ac1_the_catalogue_comes_from_the_defaults() {
    let old = parse(BEFORE_CATALOGUE).unwrap();
    assert!(!BEFORE_CATALOGUE.contains("[providers"));
    assert_eq!(old.providers, Config::default().providers);
    let speech = old.provider(&old.speech.provider).unwrap();
    assert_eq!(speech.protocol, Protocol::SpeechmaticsRt);
    assert!(speech.models.iter().any(|m| m.id == old.speech.model));
    let cleanup = old.provider(&old.cleanup.provider).unwrap();
    assert_eq!(cleanup.protocol, Protocol::OpenaiChat);
    assert!(cleanup.models.iter().any(|m| m.id == old.cleanup.model));
}

/// R-1, R-4: the user's file names roles only; what a provider is lives in the built-in
/// catalogue, so a new release's prices and providers reach files written by older ones.
#[test]
fn the_default_file_names_roles_and_the_catalogue_names_providers() {
    assert!(!DEFAULT_CONFIG.lines().any(|l| l.starts_with("[providers")));
    for field in ["endpoint =", "key_url =", "price_"] {
        assert!(
            !DEFAULT_CONFIG.lines().any(|l| l.starts_with(field)),
            "default-config.toml still sets {field}"
        );
        assert!(
            DEFAULT_PROVIDERS.contains(field),
            "catalogue has no {field}"
        );
    }
}

/// AC-2 (config half): a provider added by hand appears for its role and is used, with no code.
#[test]
fn ac2_a_hand_added_provider_appears_for_its_role() {
    let text = format!("{DEFAULT_CONFIG}{HAND_ADDED}");
    let config = parse(&text).unwrap();
    let cleanup: Vec<&str> = config
        .providers_for(Role::Cleanup)
        .map(|(id, _)| id)
        .collect();
    assert!(cleanup.contains(&"myserver"), "{cleanup:?}");
    assert!(cleanup.contains(&config.cleanup.provider.as_str()));
    let speech: Vec<&str> = config
        .providers_for(Role::Speech)
        .map(|(id, _)| id)
        .collect();
    assert!(!speech.contains(&"myserver"), "{speech:?}");
    assert!(!config.provider("myserver").unwrap().key_required);

    let text = text
        .replacen("provider = \"groq\"", "provider = \"myserver\"", 1)
        .replacen("model = \"openai/gpt-oss-20b\"", "model = \"tiny\"", 1);
    let config = parse(&text).unwrap();
    assert_eq!(config.cleanup.endpoint, "http://127.0.0.1:9999/v1");
    assert_eq!(config.cleanup_model().id, "tiny");
    assert_eq!(monthly_cost(&config, 60).cleanup_usd, 0.0);
}

/// R-5: an entry with a built-in provider's name replaces it, so a user can point one at
/// another region or host.
#[test]
fn a_hand_written_entry_replaces_the_built_in_one() {
    let entry = DEFAULT_PROVIDERS
        .split("\n[")
        .find(|block| block.starts_with("providers.groq]"))
        .unwrap();
    let mine = format!("[{entry}").replace(
        "https://api.groq.com/openai/v1",
        "https://proxy.example.com/v1",
    );
    let config = parse(&format!("{DEFAULT_CONFIG}\n{mine}")).unwrap();
    assert_eq!(config.cleanup.endpoint, "https://proxy.example.com/v1");
    assert!(config.provider("speechmatics").is_some());
}

/// AC-3: an unknown protocol stops the app with the file, the line and the known protocols.
#[test]
fn ac3_an_unknown_protocol_names_file_line_and_protocols() {
    let text = format!(
        "{DEFAULT_CONFIG}{}",
        HAND_ADDED.replace("\"openai-chat\"", "\"grpc-magic\"")
    );
    let line = text.lines().position(|l| l.contains("grpc-magic")).unwrap() + 1;
    let msg = parse(&text).unwrap_err().to_string();
    assert!(msg.contains("/tmp/hyperme/config.toml"), "{msg}");
    assert!(msg.contains(&format!("line {line}")), "{msg}");
    for protocol in Protocol::ALL {
        assert!(msg.contains(protocol.name()), "{msg}");
    }
}

/// A role naming a provider the catalogue lacks names the role and the providers that exist.
#[test]
fn a_role_with_an_unknown_provider_names_the_providers() {
    let text = DEFAULT_CONFIG.replacen("provider = \"groq\"", "provider = \"nobody\"", 1);
    let msg = parse(&text).unwrap_err().to_string();
    assert!(msg.contains("cleanup.provider"), "{msg}");
    assert!(msg.contains("nobody"), "{msg}");
    assert!(msg.contains("groq"), "{msg}");
}

/// R-7: a role only takes a provider whose protocol can fill it.
#[test]
fn a_role_refuses_a_provider_of_the_wrong_kind() {
    let text = DEFAULT_CONFIG.replacen("provider = \"groq\"", "provider = \"speechmatics\"", 1);
    let msg = parse(&text).unwrap_err().to_string();
    assert!(msg.contains("cleanup.provider"), "{msg}");
    assert!(msg.contains("groq"), "{msg}");

    let text = format!(
        "{DEFAULT_CONFIG}{}",
        HAND_ADDED.replace("roles = [\"cleanup\"]", "roles = [\"speech\"]")
    );
    let msg = parse(&text).unwrap_err().to_string();
    assert!(msg.contains("providers.myserver.roles"), "{msg}");
}

/// Catalogue entries are checked like the rest of the file.
#[test]
fn catalogue_values_are_checked() {
    for (from, to, key) in [
        (
            "http://127.0.0.1:9999/v1",
            "ftp://127.0.0.1/v1",
            "providers.myserver.endpoint",
        ),
        (
            "price_output_per_million_usd = 0",
            "price_output_per_million_usd = -2",
            "providers.myserver.models.tiny.price_output_per_million_usd",
        ),
        ("id = \"tiny\"", "id = \" \"", "providers.myserver.models"),
        (
            "name = \"My server\"",
            "name = \"\"",
            "providers.myserver.name",
        ),
    ] {
        let text = format!("{DEFAULT_CONFIG}{}", HAND_ADDED.replacen(from, to, 1));
        let msg = parse(&text).unwrap_err().to_string();
        assert!(msg.contains(key), "{key}: {msg}");
    }
    // The built-in catalogue passes the same checks.
    assert!(!Config::default().providers.is_empty());
}

/// Every built-in provider: one protocol that fills its roles, a key page unless it needs no key,
/// and dated prices on every model.
#[test]
fn built_in_providers_are_complete() {
    let config = Config::default();
    for (id, entry) in &config.providers {
        assert!(!entry.roles.is_empty(), "{id}");
        if entry.key_required {
            assert!(entry.key_url.is_some(), "{id} has no key page");
        }
        for model in &entry.models {
            assert!(model.prices_checked.is_some(), "{id} {} undated", model.id);
        }
    }
}

/// Slice 1: the estimate works per catalogue model, so onboarding and every Settings row give
/// the same number (AC-8 later builds on this).
#[test]
fn cost_reads_catalogue_models() {
    let config = Config::default();
    let speech = config.speech_model();
    let cleanup = config.cleanup_model();
    let total = monthly_cost(&config, 30);
    assert_eq!(speech_usd(&speech, 30), total.speech_usd);
    assert_eq!(
        cleanup_usd(&cleanup, &config.cleanup, 30),
        total.cleanup_usd
    );
    // A model the catalogue does not list (typed by hand, R-16) has no price yet, not a guess.
    let text = DEFAULT_CONFIG.replacen(
        "model = \"openai/gpt-oss-20b\"",
        "model = \"some-new-model\"",
        1,
    );
    let config = parse(&text).unwrap();
    assert_eq!(config.cleanup_model().price_input_per_million_usd, None);
}

/// Settings' advanced endpoint fields still work on a file written with the catalogue: the
/// commented-out override is switched on and wins over the catalogue's endpoint (R-4, R-5).
#[test]
fn settings_can_still_override_an_endpoint() {
    use hyperme_core::config::set_value;
    let text = set_value(
        DEFAULT_CONFIG,
        "cleanup",
        "endpoint",
        "\"http://127.0.0.1:8080/v1\"",
    )
    .unwrap();
    assert_eq!(
        parse(&text).unwrap().cleanup.endpoint,
        "http://127.0.0.1:8080/v1"
    );
    let text = set_value(
        DEFAULT_CONFIG,
        "speech",
        "endpoint",
        "\"wss://global.example.com/v2\"",
    )
    .unwrap();
    assert_eq!(
        parse(&text).unwrap().speech.endpoint,
        "wss://global.example.com/v2"
    );
}

/// Paid accounts research (docs/research/2026-11-27-paid-accounts-and-agent-sessions.md): the
/// Anthropic entry tells Claude Max, Ultra and Team users that their plan's monthly API credits
/// pay for the key. The words live in the catalogue, not in code (R-1).
#[test]
fn the_anthropic_entry_carries_the_plan_credits_note() {
    let config = Config::default();
    let note = config
        .provider("anthropic")
        .and_then(|p| p.key_note.as_deref())
        .expect("anthropic has a key note");
    for word in ["Max", "Ultra", "Team", "API credits"] {
        assert!(note.contains(word), "{word} missing from {note:?}");
    }
    let groq = config.provider("groq").unwrap();
    assert_eq!(groq.key_note, None);
}

/// A hand-added entry may carry its own note; it is optional.
#[test]
fn a_hand_added_entry_may_carry_a_key_note() {
    let text = format!(
        "{DEFAULT_CONFIG}\n{}",
        HAND_ADDED.replace(
            "key_required = false",
            "key_required = false\nkey_note = \"Ask IT for the key\""
        )
    );
    let config = parse(&text).unwrap();
    assert_eq!(
        config.provider("myserver").unwrap().key_note.as_deref(),
        Some("Ask IT for the key")
    );
}
