//! Provider settings, slice 2: switching providers without a restart
//! (`specs/96-provider-settings/spec.md`, R-8, R-9). Tests cite AC-4.

mod common;

use std::sync::Arc;

use common::*;
use hyperme_core::dictation::{Event, Providers, Status};

/// AC-4: a provider change reaches the next dictation without a restart, and a dictation in
/// progress finishes on the providers it started with.
#[tokio::test(start_paused = true)]
async fn ac4_a_switch_takes_effect_on_the_next_dictation_only() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "first dictation words here");
    let cleanup = FakeCleanup::new(&log, "Cleaned by the old provider.");
    let mut h = Harness::start(&log, speech, cleanup, settings());

    h.send(Event::KeyDown).await;
    h.until(&Status::Recording).await;
    h.frames(0, 5).await;
    // Settings saves another provider while the user is still talking.
    h.switch
        .send(Arc::new(Providers {
            speech: FakeSpeech::new(&log, "second dictation words here"),
            cleanup: FakeCleanup::new(&log, "Cleaned by the new provider."),
        }))
        .unwrap();
    h.frames(5, 5).await;
    tokio::time::sleep(ms(300)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: false }).await;
    assert_eq!(log.pasted(), ["Cleaned by the old provider."]);

    h.until(&Status::Idle).await;
    h.send(Event::KeyDown).await;
    h.until(&Status::Recording).await;
    h.frames(0, 5).await;
    tokio::time::sleep(ms(300)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: false }).await;
    assert_eq!(
        log.pasted(),
        [
            "Cleaned by the old provider.",
            "Cleaned by the new provider."
        ]
    );
}

/// A dictionary saved in Settings (spec 93, R-10a) reaches a provider switched in after it, so
/// changing provider never drops the user's words from the cleanup prompt.
#[tokio::test(start_paused = true)]
async fn a_saved_dictionary_follows_a_provider_switch() {
    let log = Log::new();
    let speech = FakeSpeech::new(&log, "send the sun trix report");
    let cleanup = FakeCleanup::new(&log, "Send the report.");
    let mut h = Harness::start(&log, speech, cleanup, settings());
    h.dictionary
        .send(hyperme_core::dictionary::Dictionary::parse(
            "Zentrix: sun trix",
        ));

    let new_log = Log::new();
    h.switch
        .send(Arc::new(Providers {
            speech: FakeSpeech::new(&new_log, "send the sun trix report"),
            cleanup: FakeCleanup::new(&new_log, "Send the Zentrix report."),
        }))
        .unwrap();
    h.send(Event::KeyDown).await;
    h.until(&Status::Recording).await;
    h.frames(0, 5).await;
    tokio::time::sleep(ms(300)).await;
    h.send(Event::KeyUp).await;
    h.until(&Status::Done { raw: false }).await;
    assert!(
        new_log.has("dictionary:\n\nThe speaker's dictionary: Zentrix."),
        "{:?}",
        new_log.entries()
    );
}
