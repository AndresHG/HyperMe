//! Provider settings, slice 9: the cleanup fallback (`specs/96-provider-settings/spec.md`,
//! R-24, R-24a, R-27). Tests cite AC-10 and AC-10a.

mod common;

use std::path::Path;
use std::time::Duration;

use common::*;
use hyperme_core::config::{self, Config, DEFAULT_CONFIG, Role};
use hyperme_core::dictation::{Event, Status};
use hyperme_core::fallback::Fallback;
use hyperme_core::traits::{CleanupProvider, ErrorKind, Via};

fn via() -> Via {
    Via {
        provider: "openai".into(),
        model: "gpt-6-luna".into(),
        name: "OpenAI".into(),
    }
}

async fn clean<C: CleanupProvider>(c: &C) -> (Result<(), hyperme_core::traits::Error>, String) {
    let mut text = String::new();
    let result = c.clean("raw words", None, &mut |p| text.push_str(p)).await;
    (result, text)
}

/// AC-10: the main provider answers an error, and the fallback cleans with the same transcript.
#[tokio::test]
async fn ac10_a_failing_main_provider_hands_over_to_the_fallback() {
    let log = Log::new();
    let mut main = FakeCleanup::new(&log, "");
    main.reply = Err(err(ErrorKind::Other, "groq: HTTP 500"));
    let both = Fallback::new(main, Some((FakeCleanup::new(&log, "Clean words."), via())));
    let (result, text) = clean(&both).await;
    result.unwrap();
    assert_eq!(text, "Clean words.");
    assert_eq!(both.fallback_used(), Some(via()));
    assert_eq!(
        log.entries()
            .iter()
            .filter(|e| e.starts_with("clean: raw words"))
            .count(),
        2
    );
}

/// The fallback is not used when the main one works, and is forgotten on the next call.
#[tokio::test]
async fn a_working_main_provider_never_calls_the_fallback() {
    let log = Log::new();
    let both = Fallback::new(
        FakeCleanup::new(&log, "Main."),
        Some((FakeCleanup::new(&log, "Fallback."), via())),
    );
    let (_, text) = clean(&both).await;
    assert_eq!(text, "Main.");
    assert_eq!(both.fallback_used(), None);
    // Only the main provider is warmed (R-30).
    both.warm().await.unwrap();
    assert_eq!(
        log.entries().iter().filter(|e| *e == "warm start").count(),
        1
    );
}

/// AC-10a: with no fallback ready, a failing main provider gives today's error at once.
#[tokio::test(start_paused = true)]
async fn ac10a_without_a_ready_fallback_the_error_comes_back_at_once() {
    let log = Log::new();
    let mut main = FakeCleanup::new(&log, "");
    main.reply = Err(err(ErrorKind::Auth, "groq: key rejected"));
    let alone = Fallback::alone(main);
    let start = tokio::time::Instant::now();
    let (result, _) = clean(&alone).await;
    assert!(result.is_err());
    assert_eq!(start.elapsed(), Duration::ZERO);
    assert_eq!(alone.fallback_used(), None);
}

/// AC-10 (R-27): the pill says "via" the fallback and the timing line names it.
#[tokio::test(start_paused = true)]
async fn ac10_the_pill_and_timing_line_name_the_fallback() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "please send the numbers to the team today");
    let mut cleanup = FakeCleanup::new(&log, "Please send the numbers to the team today.");
    cleanup.via = Some(via());
    let timing = FakeTiming::default();
    let mut h = Harness::start_timed(&log, speech, cleanup, settings(), timing.clone(), true);
    h.send(Event::KeyDown).await;
    h.frames(0, 5).await;
    tokio::time::sleep(ms(600)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done {
        raw: false,
        via: Some("OpenAI".into()),
    })
    .await;
    h.settle().await;
    let line = timing.lines.lock().unwrap()[0].clone();
    assert!(!line.fallback);
    assert_eq!(
        line.cleanup_via,
        Some(("openai".to_owned(), "gpt-6-luna".to_owned()))
    );
}

/// R-3, R-24: a fallback in the config is built from the catalogue: the main prompt and
/// deadline, the fallback's endpoint and the model's fastest effort.
#[test]
fn a_cleanup_fallback_in_the_config_takes_its_endpoint_from_the_catalogue() {
    let text = config::set_fallback(
        DEFAULT_CONFIG,
        Role::Cleanup,
        Some(("openai", "gpt-6-luna")),
    )
    .unwrap();
    assert!(text.contains("fallback = { provider = \"openai\", model = \"gpt-6-luna\" }"));
    let config = Config::parse(&text, Path::new("config.toml")).unwrap();
    let fallback = config.cleanup_fallback().unwrap();
    assert_eq!(fallback.provider, "openai");
    assert_eq!(fallback.endpoint, "https://api.openai.com/v1");
    assert_eq!(fallback.reasoning_effort, "none");
    assert_eq!(fallback.deadline_ms, config.cleanup.deadline_ms);
    // Cleared again, the file is as it was.
    let cleared = config::set_fallback(&text, Role::Cleanup, None).unwrap();
    assert_eq!(cleared, DEFAULT_CONFIG);
    assert!(Config::default().cleanup_fallback().is_none());
}

/// A fallback must be a provider that can fill the role.
#[test]
fn a_fallback_of_the_wrong_kind_is_refused() {
    let msg = config::set_fallback(
        DEFAULT_CONFIG,
        Role::Cleanup,
        Some(("speechmatics", "melia-1")),
    )
    .unwrap_err()
    .to_string();
    assert!(msg.contains("cleanup.fallback"), "{msg}");
}

/// AC-14 (R-29): the Privacy section's list names every provider set, fallbacks included, and
/// a server on this Mac as such.
#[test]
fn ac14_every_provider_set_is_a_destination_fallbacks_included() {
    let only = Config::default().destinations();
    let names: Vec<&str> = only.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["Speechmatics", "Groq"]);
    assert!(
        only.iter()
            .all(|d| d.fallback_for.is_none() && !d.on_this_mac)
    );

    let text = config::set_fallback(
        DEFAULT_CONFIG,
        Role::Cleanup,
        Some(("openai", "gpt-6-luna")),
    )
    .unwrap();
    let text = config::set_fallback(
        &text,
        Role::Speech,
        Some(("openai-speech", "gpt-live-transcribe")),
    )
    .unwrap();
    let both = Config::parse(&text, Path::new("config.toml"))
        .unwrap()
        .destinations();
    let shown: Vec<(Role, &str, Option<&str>)> = both
        .iter()
        .map(|d| (d.role, d.name.as_str(), d.fallback_for.as_deref()))
        .collect();
    assert_eq!(
        shown,
        [
            (Role::Speech, "Speechmatics", None),
            (Role::Speech, "OpenAI", Some("Speechmatics")),
            (Role::Cleanup, "Groq", None),
            (Role::Cleanup, "OpenAI", Some("Groq")),
        ]
    );
}
