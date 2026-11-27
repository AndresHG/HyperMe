//! Onboarding's cost estimate (docs/UX.md, "Models and keys in onboarding"; U-11): monthly cost
//! for three amounts of talking a day, from prices and estimates in the config file, never in
//! code.

use std::path::Path;

use hyperme_core::config::{Config, DEFAULT_CONFIG, DEFAULT_PROVIDERS, Role};
use hyperme_core::cost::{CHOICES, free_credit_line, monthly_cost, other_key_notes, shown_as};

/// The defaults reproduce UX.md's table: about USD 2, 6 and 12 of speech and 0.05, 0.15 and
/// 0.30 of cleanup a month for 10 minutes, 30 minutes and 1 hour a day.
#[test]
fn defaults_match_the_ux_table() {
    let config = Config::default();
    let cases = [(10, 2.0, 0.045), (30, 6.0, 0.135), (60, 12.0, 0.27)];
    for (minutes, speech, cleanup) in cases {
        let cost = monthly_cost(&config, minutes);
        assert!(
            (cost.speech_usd - speech).abs() < 1e-9,
            "{minutes} min: speech {}",
            cost.speech_usd
        );
        assert!(
            (cost.cleanup_usd - cleanup).abs() < 1e-9,
            "{minutes} min: cleanup {}",
            cost.cleanup_usd
        );
    }
    assert_eq!(
        CHOICES
            .iter()
            .map(|c| c.minutes_per_day)
            .collect::<Vec<_>>(),
        [10, 30, 60]
    );
}

#[test]
fn shown_as_rounds_to_dollars_or_cents() {
    let config = Config::default();
    assert_eq!(
        shown_as(monthly_cost(&config, 10).total_usd()),
        "About $2 a month"
    );
    assert_eq!(
        shown_as(monthly_cost(&config, 30).total_usd()),
        "About $6 a month"
    );
    assert_eq!(
        shown_as(monthly_cost(&config, 60).total_usd()),
        "About $12 a month"
    );
    assert_eq!(shown_as(0.27), "About $0.27 a month");
}

/// The free-credit line comes from the config: USD 100 at USD 0.40 an hour is 250 hours.
#[test]
fn free_credit_line_is_worked_out_from_the_config() {
    assert_eq!(
        free_credit_line(&Config::default()).as_deref(),
        Some("Speechmatics gives new accounts USD 100 of free credit, about 250 hours of talking.")
    );
}

/// A file written before the provider catalogue (specs/96-provider-settings, R-4).
const BEFORE_CATALOGUE: &str = include_str!("fixtures/config-before-96.toml");

/// Prices live in the config: changing them there changes the estimate, and no price is written
/// in the code. An older file's prices still override the catalogue's.
#[test]
fn prices_come_from_the_config_not_the_code() {
    let text = BEFORE_CATALOGUE
        .replacen("price_per_hour_usd = 0.40", "price_per_hour_usd = 1.00", 1)
        .replacen(
            "price_output_per_million_usd = 0.30",
            "price_output_per_million_usd = 3.00",
            1,
        );
    let config = Config::parse(&text, Path::new("c.toml")).unwrap();
    let cost = monthly_cost(&config, 60);
    assert!((cost.speech_usd - 30.0).abs() < 1e-9);
    assert!(cost.cleanup_usd > 1.0);

    // The same through a catalogue entry in the user's file.
    let start = DEFAULT_PROVIDERS.find("[providers.speechmatics]").unwrap();
    let end = DEFAULT_PROVIDERS.find("[providers.groq]").unwrap();
    let entry = &DEFAULT_PROVIDERS[start..end];
    let mine = entry.replace("price_per_hour_usd = 0.40", "price_per_hour_usd = 1.00");
    let config = Config::parse(&format!("{DEFAULT_CONFIG}\n{mine}"), Path::new("c.toml")).unwrap();
    assert!((monthly_cost(&config, 60).speech_usd - 30.0).abs() < 1e-9);

    let source = include_str!("../src/cost.rs");
    for price in ["0.40", "0.075", "0.30", "100.0", "0.4"] {
        assert!(!source.contains(price), "cost.rs holds the price {price}");
    }
}

/// An older config file without the onboarding keys still loads, and takes the defaults for them.
#[test]
fn an_older_config_without_prices_still_loads() {
    let old: String = BEFORE_CATALOGUE
        .lines()
        .filter(|l| {
            !l.starts_with("price_")
                && !l.starts_with("key_url")
                && !l.starts_with("free_credit")
                && !l.starts_with("estimate_")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let config = Config::parse(&old, Path::new("old.toml")).unwrap();
    assert_eq!(
        monthly_cost(&config, 60),
        monthly_cost(&Config::default(), 60)
    );
    assert_eq!(config.speech.key_url(), Config::default().speech.key_url());
    assert_eq!(
        free_credit_line(&config),
        free_credit_line(&Config::default())
    );
}

#[test]
fn key_pages_and_prices_are_checked() {
    for (from, to, key) in [
        (
            "key_url = \"https://portal.speechmatics.com/signup\"",
            "key_url = \"portal.speechmatics.com\"",
            "speech.key_url",
        ),
        (
            "price_per_hour_usd = 0.40",
            "price_per_hour_usd = -1.0",
            "speech.price_per_hour_usd",
        ),
    ] {
        assert!(BEFORE_CATALOGUE.contains(from), "{from}");
        let err = Config::parse(&BEFORE_CATALOGUE.replacen(from, to, 1), Path::new("c.toml"))
            .unwrap_err()
            .to_string();
        assert!(err.contains(key), "{err}");
    }
}

/// Onboarding's keys screen names the cleanup providers it does not use whose catalogue entry has
/// a key note, such as Anthropic's plan credits, so a Claude Max user knows their key is covered
/// (docs/research/2026-11-27-paid-accounts-and-agent-sessions.md).
#[test]
fn onboarding_lists_other_cleanup_providers_with_a_key_note() {
    let config = Config::default();
    let notes = other_key_notes(&config, Role::Cleanup);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        notes[0].starts_with("Anthropic: Have Claude Max"),
        "{notes:?}"
    );
    assert!(notes[0].ends_with("Choose it later in Settings, Providers."));
    assert!(other_key_notes(&config, Role::Speech).is_empty());
}
