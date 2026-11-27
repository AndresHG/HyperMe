//! Provider settings, slice 10: the speech fallback (`specs/96-provider-settings/spec.md`,
//! R-25 to R-28). Tests cite AC-11, AC-12 and AC-13.

mod common;

use std::sync::{Arc, Mutex};

use common::*;
use hyperme_core::config::{self, Config, DEFAULT_CONFIG, Role};
use hyperme_core::dictation::{Event, Status};
use hyperme_core::fallback::{Fallback, FallbackStreak};
use hyperme_core::traits::{Error, ErrorKind, LiveSession, SpeechProvider, Transcript, Via};

fn via() -> Via {
    Via {
        provider: "openai-speech".into(),
        model: "gpt-live-transcribe".into(),
        name: "OpenAI".into(),
    }
}

/// A speech provider that records every sample it receives, and can refuse to open or drop
/// after some frames.
#[derive(Clone, Default)]
struct Rec {
    name: &'static str,
    refuse: bool,
    drop_after: Option<usize>,
    got: Arc<Mutex<Vec<i16>>>,
    opened: Arc<Mutex<usize>>,
}

struct RecSession {
    rec: Rec,
    frames: usize,
}

impl SpeechProvider for Rec {
    type Session = RecSession;
    async fn open(&self) -> Result<RecSession, Error> {
        *self.opened.lock().unwrap() += 1;
        if self.refuse {
            return Err(err(ErrorKind::Network, &format!("{}: refused", self.name)));
        }
        Ok(RecSession {
            rec: self.clone(),
            frames: 0,
        })
    }
}

impl LiveSession for RecSession {
    async fn send_audio(&mut self, pcm: &[i16]) -> Result<(), Error> {
        if self.rec.drop_after.is_some_and(|n| self.frames >= n) {
            return Err(err(ErrorKind::Network, "the connection dropped"));
        }
        self.frames += 1;
        self.rec.got.lock().unwrap().extend_from_slice(pcm);
        Ok(())
    }
    async fn finish(self) -> Result<Transcript, Error> {
        if self.rec.drop_after.is_some_and(|n| self.frames >= n) {
            return Err(err(ErrorKind::Network, "the connection dropped"));
        }
        Ok(Transcript {
            text: format!("from {}", self.rec.name),
            language: None,
            via: None,
        })
    }
}

fn frames() -> Vec<Vec<i16>> {
    (0..5).map(|i| vec![i as i16; 480]).collect()
}

/// AC-11 (R-25): the main provider refuses; the fallback opens and gets every frame.
#[tokio::test]
async fn ac11_a_refused_main_session_opens_the_fallback_with_every_frame() {
    let main = Rec {
        name: "main",
        refuse: true,
        ..Rec::default()
    };
    let spare = Rec {
        name: "spare",
        ..Rec::default()
    };
    let both = Fallback::new(Arc::new(main), Some((Arc::new(spare.clone()), via())));
    let mut session = both.open().await.unwrap();
    for f in frames() {
        session.send_audio(&f).await.unwrap();
    }
    let transcript = session.finish().await.unwrap();
    assert_eq!(transcript.text, "from spare");
    assert_eq!(transcript.via, Some(via()));
    assert_eq!(spare.got.lock().unwrap().len(), 5 * 480);
}

/// AC-12 (R-26): the main session drops mid-dictation; on release the saved audio, from the
/// first frame, goes to the fallback and its text comes back.
#[tokio::test]
async fn ac12_a_drop_mid_dictation_sends_the_saved_audio_on_release() {
    let main = Rec {
        name: "main",
        drop_after: Some(2),
        ..Rec::default()
    };
    let spare = Rec {
        name: "spare",
        ..Rec::default()
    };
    let both = Fallback::new(Arc::new(main), Some((Arc::new(spare.clone()), via())));
    let mut session = both.open().await.unwrap();
    for f in frames() {
        session.send_audio(&f).await.unwrap();
    }
    assert_eq!(
        *spare.opened.lock().unwrap(),
        0,
        "the fallback waits for the release"
    );
    let transcript = session.finish().await.unwrap();
    assert_eq!(transcript.text, "from spare");
    assert_eq!(transcript.via, Some(via()));
    let expected: Vec<i16> = frames().concat();
    assert_eq!(*spare.got.lock().unwrap(), expected);
}

/// Both failing gives the main provider's error, as today (Phase 1 R-19); with no fallback, a
/// drop fails at once and nothing is kept in memory.
#[tokio::test]
async fn both_failing_shows_the_main_error_and_no_fallback_fails_at_once() {
    let main = Rec {
        name: "main",
        refuse: true,
        ..Rec::default()
    };
    let spare = Rec {
        name: "spare",
        refuse: true,
        ..Rec::default()
    };
    let both = Fallback::new(Arc::new(main.clone()), Some((Arc::new(spare), via())));
    let error = both.open().await.unwrap_err();
    assert!(error.message.starts_with("main:"), "{}", error.message);

    let dropping = Rec {
        name: "main",
        drop_after: Some(1),
        ..Rec::default()
    };
    let alone = Fallback::alone(Arc::new(dropping));
    let mut session = alone.open().await.unwrap();
    session.send_audio(&[1; 480]).await.unwrap();
    assert!(session.send_audio(&[2; 480]).await.is_err());
    assert!(format!("{session:?}").contains("saved: 0"));
}

/// AC-11 (R-27): the pill says "via" the fallback, and the timing line names it.
#[tokio::test(start_paused = true)]
async fn ac11_the_pill_and_timing_line_name_the_speech_fallback() {
    let log = Log::new();
    let mut speech = FakeSpeech::new(&log, "hi there");
    speech.via = Some(via());
    let timing = FakeTiming::default();
    let cleanup = FakeCleanup::new(&log, "unused");
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
    assert_eq!(
        line.speech_via,
        Some(("openai-speech".to_owned(), "gpt-live-transcribe".to_owned()))
    );
}

/// AC-13 (R-28): three fallbacks in a row bring one notice for that role; a dictation on the
/// main provider starts the count again.
#[tokio::test(start_paused = true)]
async fn ac13_three_fallbacks_in_a_row_bring_one_notice() {
    let log = Log::new();
    let mut speech = FakeSpeech::new(&log, "hi there");
    speech.via = Some(via());
    let cleanup = FakeCleanup::new(&log, "unused");
    let mut h = Harness::start(&log, speech.clone(), cleanup, settings());
    let dictate = async |h: &mut Harness| {
        h.send(Event::KeyDown).await;
        h.frames(0, 3).await;
        tokio::time::sleep(ms(400)).await;
        h.send(Event::KeyUp).await;
        h.until(&Status::Idle).await;
    };
    for _ in 0..4 {
        dictate(&mut h).await;
    }
    h.switch
        .send(Arc::new(hyperme_core::dictation::Providers {
            speech: FakeSpeech::new(&log, "hi there"),
            cleanup: FakeCleanup::new(&log, "unused"),
        }))
        .unwrap();
    dictate(&mut h).await;
    h.settle().await;

    let mut streak = FallbackStreak::default();
    let notices: Vec<Vec<Role>> = h.traces.all().iter().map(|t| streak.observe(t)).collect();
    assert_eq!(notices.len(), 5);
    assert_eq!(notices[2], [Role::Speech], "{notices:?}");
    let total: usize = notices.iter().map(Vec::len).sum();
    assert_eq!(total, 1, "one notice, not one per dictation: {notices:?}");
}

/// R-3: a speech fallback in the config takes its endpoint from the catalogue.
#[test]
fn a_speech_fallback_in_the_config() {
    let text = config::set_fallback(
        DEFAULT_CONFIG,
        Role::Speech,
        Some(("openai-speech", "gpt-live-transcribe")),
    )
    .unwrap();
    let config = Config::parse(&text, std::path::Path::new("config.toml")).unwrap();
    let fallback = config.speech_fallback().unwrap();
    assert_eq!(fallback.provider, "openai-speech");
    assert!(fallback.endpoint.starts_with("wss://api.openai.com/"));
    assert!(
        config::set_fallback(DEFAULT_CONFIG, Role::Speech, Some(("groq", "x"))).is_err(),
        "a cleanup provider cannot be a speech fallback"
    );
}
