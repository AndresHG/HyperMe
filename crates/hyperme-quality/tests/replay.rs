//! AC-3 (the app's own pipeline) and AC-7 (offline), issue 99. Fake providers stand in for
//! Speechmatics and the cleanup host; the state machine is the app's.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use hyperme_core::dictation::Settings;
use hyperme_core::dictionary::Dictionary;
use hyperme_core::traits::{
    CleanupProvider, Error, ErrorKind, LiveSession, SpeechProvider, Transcript,
};
use hyperme_quality::replay::{replay, replay_raw};
use hyperme_quality::set::{Lang, Sample};
use tokio::time::{Instant, sleep};

/// Each sample's first audio value picks its transcript; `None` fails the speech.
struct FakeSpeech {
    transcripts: Vec<Option<&'static str>>,
    /// Audio samples received by each session.
    received: Arc<Mutex<Vec<usize>>>,
}

struct FakeSession {
    transcripts: Vec<Option<&'static str>>,
    first: Option<i16>,
    count: usize,
    received: Arc<Mutex<Vec<usize>>>,
}

impl SpeechProvider for FakeSpeech {
    type Session = FakeSession;

    async fn open(&self) -> Result<FakeSession, Error> {
        sleep(Duration::from_millis(50)).await;
        Ok(FakeSession {
            transcripts: self.transcripts.clone(),
            first: None,
            count: 0,
            received: self.received.clone(),
        })
    }
}

impl LiveSession for FakeSession {
    async fn send_audio(&mut self, pcm: &[i16]) -> Result<(), Error> {
        self.first = self.first.or(pcm.first().copied());
        self.count += pcm.len();
        Ok(())
    }

    async fn finish(self) -> Result<Transcript, Error> {
        sleep(Duration::from_millis(100)).await;
        self.received.lock().unwrap().push(self.count);
        let index = self.first.unwrap_or_default() as usize;
        match self.transcripts[index] {
            Some(text) => Ok(Transcript {
                text: text.to_owned(),
                language: None,
                via: None,
            }),
            None => Err(Error {
                kind: ErrorKind::Network,
                message: "connection dropped".to_owned(),
            }),
        }
    }
}

/// Capitalises and adds a full stop after 200 ms; a raw text with "slow" takes 2 s.
struct FakeCleanup;

impl CleanupProvider for FakeCleanup {
    async fn warm(&self) -> Result<(), Error> {
        Ok(())
    }

    async fn clean(
        &self,
        raw: &str,
        _app: Option<&str>,
        on_text: &mut (dyn FnMut(&str) + Send),
    ) -> Result<(), Error> {
        let delay = if raw.contains("slow") { 2000 } else { 200 };
        sleep(Duration::from_millis(delay)).await;
        let cleaned = raw.replace("eh ", "");
        let mut chars = cleaned.chars();
        let first: String = chars
            .next()
            .into_iter()
            .flat_map(char::to_uppercase)
            .collect();
        on_text(&first);
        on_text(&format!("{}.", chars.as_str()));
        Ok(())
    }
}

fn settings() -> Settings {
    Settings {
        min_hold: Duration::from_millis(300),
        max_length: Duration::from_secs(60),
        double_tap: Duration::from_millis(400),
        short_words: 3,
        cleanup_deadline: Duration::from_millis(1500),
        sounds: false,
        dictionary: Default::default(),
        prompt_cap: hyperme_core::dictionary::PROMPT_CAP,
    }
}

/// One second of audio whose first value picks the transcript.
fn sample(name: &str, pick: i16) -> Sample {
    let mut pcm = vec![0i16; 24_000];
    pcm[0] = pick;
    Sample {
        name: name.to_owned(),
        pcm,
    }
}

/// AC-3: the engine's own rules decide the text: cleaned, raw after the deadline, short
/// formatting, and a speech failure. The raw transcript and stage timings are recorded.
#[tokio::test(start_paused = true)]
async fn samples_run_through_the_app_state_machine() {
    let received = Arc::new(Mutex::new(Vec::new()));
    let speech = FakeSpeech {
        transcripts: vec![
            Some("eh hola equipo esto es una prueba"),
            Some("the slow one passes the deadline"),
            Some("ok"),
            None,
        ],
        received: received.clone(),
    };
    let samples = [
        sample("es-01", 0),
        sample("en-01", 1),
        sample("en-02", 2),
        sample("es-02", 3),
    ];
    let mut seen = Vec::new();
    let start = Instant::now();
    let results = replay(&speech, &FakeCleanup, settings(), &samples, |r| {
        seen.push(r.sample.clone())
    })
    .await;
    assert_eq!(seen, ["es-01", "en-01", "en-02", "es-02"]);

    let cleaned = results[0].outcome.as_ref().unwrap();
    assert_eq!(results[0].lang, Some(Lang::Es));
    assert_eq!(
        results[0].raw.as_deref(),
        Some("eh hola equipo esto es una prueba")
    );
    assert_eq!(cleaned.text, "Hola equipo esto es una prueba.");
    assert!(!cleaned.fallback);
    // From key release, as in the app's timing log: 100 ms of speech, then 200 of cleanup.
    assert_eq!(cleaned.stt_ms, 100);
    assert_eq!(cleaned.first_ms, Some(300));
    assert_eq!(cleaned.total_ms, 300);

    let late = results[1].outcome.as_ref().unwrap();
    assert!(late.fallback);
    assert_eq!(late.text, "the slow one passes the deadline");
    assert_eq!(late.first_ms, None);
    assert_eq!(late.total_ms, 1600);

    let short = results[2].outcome.as_ref().unwrap();
    assert_eq!(short.text, "Ok.");
    assert!(!short.fallback);

    let failed = results[3].outcome.as_ref().unwrap_err();
    assert!(failed.contains("connection dropped"), "{failed}");
    assert_eq!(results[3].raw, None);

    // Every sample's audio reached the speech session, paced in real time: four seconds of
    // audio take at least four seconds.
    assert_eq!(*received.lock().unwrap(), [24_000; 4]);
    assert!(start.elapsed() >= Duration::from_secs(4));
}

/// AC-3: a sample shorter than the minimum hold is cancelled by the engine, as in the app.
#[tokio::test(start_paused = true)]
async fn a_sample_shorter_than_the_minimum_hold_is_reported() {
    let speech = FakeSpeech {
        transcripts: vec![Some("never used")],
        received: Arc::default(),
    };
    let short = Sample {
        name: "en-09".to_owned(),
        pcm: vec![0; 2_400],
    };
    let results = replay(&speech, &FakeCleanup, settings(), &[short], |_| {}).await;
    let error = results[0].outcome.as_ref().unwrap_err();
    assert!(error.contains("minimum hold"), "{error}");
}

/// Slice A, AC-1: a cleanup-only replay runs saved transcripts through the same engine, at once
/// (no audio pacing): dictionary substitution, the short rule, the cleanup and the deadline's
/// raw fallback are the app's. Only the cleanup is called.
#[tokio::test(start_paused = true)]
async fn saved_transcripts_replay_through_the_engine_without_audio() {
    let raw: BTreeMap<String, String> = [
        ("es-01", "eh hola equipo esto es zen trix"),
        ("en-01", "the slow one passes the deadline"),
        ("en-02", "ok"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect();
    let settings = Settings {
        dictionary: Dictionary::parse("Zentrix: zen trix"),
        ..settings()
    };
    // Ten seconds of audio each, which a live replay would take in real time.
    let samples = [
        Sample {
            name: "es-01".to_owned(),
            pcm: vec![0; 240_000],
        },
        Sample {
            name: "en-01".to_owned(),
            pcm: vec![0; 240_000],
        },
        Sample {
            name: "en-02".to_owned(),
            pcm: vec![0; 240_000],
        },
    ];
    let start = Instant::now();
    let results = replay_raw(
        &FakeCleanup,
        settings,
        &samples,
        &raw,
        Duration::ZERO,
        |_| {},
    )
    .await;

    let cleaned = results[0].outcome.as_ref().unwrap();
    assert_eq!(
        results[0].raw.as_deref(),
        Some("eh hola equipo esto es zen trix")
    );
    assert_eq!(cleaned.text, "Hola equipo esto es Zentrix.");
    assert_eq!(cleaned.stt_ms, 0);
    assert_eq!(cleaned.total_ms, 200);
    let late = results[1].outcome.as_ref().unwrap();
    assert!(late.fallback);
    assert_eq!(late.text, "the slow one passes the deadline");
    assert_eq!(results[2].outcome.as_ref().unwrap().text, "Ok.");
    // 200 ms + 1500 ms of cleanup, nothing for the 30 s of audio.
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "{:?}",
        start.elapsed()
    );
}

/// Slice A, AC-2: `--pace-ms` waits between samples; a sample with no saved transcript is
/// reported, not sent.
#[tokio::test(start_paused = true)]
async fn a_replay_of_saved_transcripts_is_paced_and_reports_missing_ones() {
    let raw: BTreeMap<String, String> = [
        ("es-01", "hola equipo esto es una prueba"),
        ("es-03", "hola equipo esto es otra prueba"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect();
    let samples = [sample("es-01", 0), sample("es-02", 0), sample("es-03", 0)];
    let start = Instant::now();
    let results = replay_raw(
        &FakeCleanup,
        settings(),
        &samples,
        &raw,
        Duration::from_secs(3),
        |_| {},
    )
    .await;
    assert!(results[0].outcome.is_ok());
    assert!(results[2].outcome.is_ok());
    let missing = results[1].outcome.as_ref().unwrap_err();
    assert!(missing.contains("no saved transcript"), "{missing}");
    // One gap between the two samples sent, plus two cleanups.
    assert!(start.elapsed() >= Duration::from_secs(3));
    assert!(start.elapsed() < Duration::from_secs(4));
}

/// Slice C: a fallback after a cleanup error says why, so a rate limit is not mistaken for a
/// slow model.
#[tokio::test(start_paused = true)]
async fn a_fallback_after_a_cleanup_error_says_why() {
    struct Limited;
    impl CleanupProvider for Limited {
        async fn warm(&self) -> Result<(), Error> {
            Ok(())
        }
        async fn clean(
            &self,
            _raw: &str,
            _app: Option<&str>,
            _on_text: &mut (dyn FnMut(&str) + Send),
        ) -> Result<(), Error> {
            Err(Error {
                kind: ErrorKind::Other,
                message: "groq: HTTP 429 Too Many Requests".to_owned(),
            })
        }
    }
    let raw: BTreeMap<String, String> = [("es-01", "hola equipo esto es una prueba")]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    let results = replay_raw(
        &Limited,
        settings(),
        &[sample("es-01", 0)],
        &raw,
        Duration::ZERO,
        |_| {},
    )
    .await;
    let pasted = results[0].outcome.as_ref().unwrap();
    assert!(pasted.fallback);
    assert!(pasted.why.as_deref().unwrap().contains("429"), "{pasted:?}");
}
