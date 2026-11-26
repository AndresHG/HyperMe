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
