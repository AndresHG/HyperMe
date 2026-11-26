//! Provider settings, slice 6: the hint line under each role (`specs/96-provider-settings`,
//! R-18 to R-20). Tests cite AC-8.

use std::path::Path;

use hyperme_core::config::{Config, DEFAULT_CONFIG, Role};
use hyperme_core::cost::{monthly_cost, shown_as};
use hyperme_core::hints::hint;

/// AC-8: the cost matches onboarding's estimate for the same talking time.
#[test]
fn ac8_the_cost_matches_the_onboarding_estimate() {
    let config = Config::default();
    for minutes in [10, 30, 60] {
        let cost = monthly_cost(&config, minutes);
        let speech = hint(
            &config,
            Role::Speech,
            "speechmatics",
            "melia-1",
            minutes,
            None,
        );
        assert!(
            speech.cost.starts_with(&shown_as(cost.speech_usd)),
            "{}",
            speech.cost
        );
        let cleanup = hint(
            &config,
            Role::Cleanup,
            "groq",
            "openai/gpt-oss-20b",
            minutes,
            None,
        );
        assert!(
            cleanup.cost.starts_with(&shown_as(cost.cleanup_usd)),
            "{}",
            cleanup.cost
        );
    }
    assert!(
        hint(&config, Role::Speech, "speechmatics", "melia-1", 30, None)
            .cost
            .contains("30 min a day")
    );
}

/// AC-8, R-18, R-19: a measured model shows its latency and the result it comes from; any other
/// says "Speed not measured".
#[test]
fn ac8_measured_latency_with_its_source_or_not_measured() {
    let config = Config::default();
    let measured = hint(&config, Role::Speech, "speechmatics", "melia-1", 30, None);
    assert!(measured.speed.contains("0.43 s"), "{}", measured.speed);
    assert_eq!(
        measured.source.as_deref(),
        Some("docs/results/2026-11-25-eu-speech.md")
    );
    let unmeasured = hint(&config, Role::Cleanup, "openai", "gpt-6-luna", 30, None);
    assert_eq!(unmeasured.speed, "Speed not measured");
    assert_eq!(unmeasured.source, None);
}

/// A model typed by hand has no price in the catalogue, and says so instead of $0.
#[test]
fn a_model_without_a_price_says_so() {
    let config = Config::default();
    let typed = hint(&config, Role::Cleanup, "groq", "some-new-model", 30, None);
    assert_eq!(typed.cost, "Price not in the catalogue");
}

/// R-20: with the timing log on, the user's own median on that provider.
#[test]
fn the_users_own_median_when_known() {
    let config = Config::default();
    let mine = hint(
        &config,
        Role::Cleanup,
        "groq",
        "openai/gpt-oss-20b",
        30,
        Some((471, 50)),
    );
    assert_eq!(
        mine.mine.as_deref(),
        Some("Your median 0.47 s over 50 dictations")
    );
    assert_eq!(
        hint(
            &config,
            Role::Cleanup,
            "groq",
            "openai/gpt-oss-20b",
            30,
            None
        )
        .mine,
        None
    );
}

/// U-11: the talking time comes from the config, where onboarding's choice is saved; an older
/// file takes the default, 30 minutes.
#[test]
fn talking_time_has_a_default() {
    assert_eq!(Config::default().dictation.talking_minutes_per_day, 30);
    let old = include_str!("fixtures/config-before-96.toml");
    let config = Config::parse(old, Path::new("old.toml")).unwrap();
    assert_eq!(config.dictation.talking_minutes_per_day, 30);
    let text = DEFAULT_CONFIG.replacen(
        "talking_minutes_per_day = 30",
        "talking_minutes_per_day = 0",
        1,
    );
    let err = Config::parse(&text, Path::new("c.toml"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("dictation.talking_minutes_per_day"), "{err}");
}
