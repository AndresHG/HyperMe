//! Replays samples through the app's own state machine (AC-3): `hyperme_core::dictation::Engine`
//! with the given speech and cleanup providers, so the short-dictation rule, the cleanup
//! deadline and the raw fallback are exactly the app's. For each sample it sends key-down, the
//! audio in 20 ms frames at the pace of real time, as the microphone would, and key-up.
//!
//! The other parts are stand-ins: no microphone, sound or history, and an inserter that keeps
//! the text instead of pasting it.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use hyperme_core::audio::frame_samples;
use hyperme_core::dictation::{Engine, Event, Parts, Settings, Status};
use hyperme_core::traits::{
    CleanupProvider, DictationId, Error, ErrorKind, FocusInfo, History, Inserter, LiveSession,
    Microphone, Sound, Sounds, SpeechProvider, TimingLine, TimingLog, Transcript,
};
use tokio::sync::mpsc;
use tokio::time::{MissedTickBehavior, interval, timeout};

use crate::set::{Lang, SAMPLE_RATE, Sample};

/// How long a sample may take after key-up before it counts as failed.
const AFTER_KEY_UP: Duration = Duration::from_secs(60);

/// The text the app would have pasted, and its stage timings from key release (R-30).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pasted {
    pub text: String,
    /// The raw transcript was pasted: the cleanup missed its deadline or failed.
    pub fallback: bool,
    pub stt_ms: u64,
    pub first_ms: Option<u64>,
    pub total_ms: u64,
}

/// One sample's replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replayed {
    pub sample: String,
    pub lang: Option<Lang>,
    /// The speech provider's final transcript, when one arrived.
    pub raw: Option<String>,
    /// What was pasted, or why nothing was.
    pub outcome: Result<Pasted, String>,
}

/// What the stand-ins saw during one sample.
#[derive(Debug, Default)]
struct Seen {
    raw: Option<String>,
    pasted: Option<String>,
    timing: Option<TimingLine>,
}

type Shared = Arc<Mutex<Seen>>;

fn with<T>(seen: &Shared, f: impl FnOnce(&mut Seen) -> T) -> T {
    f(&mut seen.lock().unwrap_or_else(PoisonError::into_inner))
}

/// Wraps the real speech provider to keep the raw transcript, which the engine does not report.
struct KeepRaw<'a, S> {
    inner: &'a S,
    seen: Shared,
}

struct KeepRawSession<L> {
    inner: L,
    seen: Shared,
}

impl<S: SpeechProvider + Sync> SpeechProvider for KeepRaw<'_, S> {
    type Session = KeepRawSession<S::Session>;

    async fn open(&self) -> Result<Self::Session, Error> {
        Ok(KeepRawSession {
            inner: self.inner.open().await?,
            seen: self.seen.clone(),
        })
    }
}

impl<L: LiveSession + Send> LiveSession for KeepRawSession<L> {
    async fn send_audio(&mut self, pcm: &[i16]) -> Result<(), Error> {
        self.inner.send_audio(pcm).await
    }

    async fn finish(self) -> Result<Transcript, Error> {
        let transcript = self.inner.finish().await?;
        with(&self.seen, |s| s.raw = Some(transcript.text.clone()));
        Ok(transcript)
    }
}

/// Borrows the real cleanup provider, so the caller keeps it.
struct Borrowed<'a, C>(&'a C);

impl<C: CleanupProvider + Sync> CleanupProvider for Borrowed<'_, C> {
    async fn warm(&self) -> Result<(), Error> {
        self.0.warm().await
    }

    async fn clean(
        &self,
        raw: &str,
        app: Option<&str>,
        on_text: &mut (dyn FnMut(&str) + Send),
    ) -> Result<(), Error> {
        self.0.clean(raw, app, on_text).await
    }
}

/// The audio comes from the replay's own events, not a device.
struct NoMicrophone;

impl Microphone for NoMicrophone {
    fn start(&mut self) -> Result<(), Error> {
        Ok(())
    }
    fn stop(&mut self) {}
}

/// Keeps the text instead of pasting it.
struct KeepText(Shared);

impl Inserter for KeepText {
    async fn paste(&mut self, text: &str) -> Result<(), Error> {
        with(&self.0, |s| s.pasted = Some(text.to_owned()));
        Ok(())
    }

    async fn copy(&mut self, text: &str) -> Result<(), Error> {
        self.paste(text).await
    }
}

/// No app in front: the cleanup prompt gets no app name, the same for every sample.
struct NoFocus;

impl FocusInfo for NoFocus {
    fn frontmost_app(&self) -> Option<String> {
        None
    }
    fn secure_input(&self) -> bool {
        false
    }
}

/// Nothing written: replayed audio is already saved, and the app's history stays untouched.
struct NoHistory;

impl History for NoHistory {
    fn begin(&mut self, _app: Option<&str>) -> Result<DictationId, Error> {
        Err(no_history())
    }
    fn append(&mut self, _id: DictationId, _pcm: &[i16]) -> Result<(), Error> {
        Err(no_history())
    }
    fn done(&mut self, _id: DictationId, _text: &str, _lang: Option<&str>) -> Result<(), Error> {
        Err(no_history())
    }
    fn failed(&mut self, _id: DictationId, _message: &str) -> Result<(), Error> {
        Err(no_history())
    }
    fn discard(&mut self, _id: DictationId) -> Result<(), Error> {
        Err(no_history())
    }
    fn audio(&self, _id: DictationId) -> Result<Vec<i16>, Error> {
        Err(no_history())
    }
}

fn no_history() -> Error {
    Error {
        kind: ErrorKind::Device,
        message: "no history in a replay".to_owned(),
    }
}

struct NoSounds;

impl Sounds for NoSounds {
    fn play(&self, _sound: Sound) {}
}

struct KeepTiming(Shared);

impl TimingLog for KeepTiming {
    fn record(&mut self, line: &TimingLine) {
        with(&self.0, |s| s.timing = Some(line.clone()));
    }
}

/// Replays each sample in turn and calls `each` as it finishes. The engine's settings are the
/// app's, with sounds off and the timing log on, as the replay reads its timings from it.
pub async fn replay<S, C>(
    speech: &S,
    cleanup: &C,
    settings: Settings,
    samples: &[Sample],
    mut each: impl FnMut(&Replayed),
) -> Vec<Replayed>
where
    S: SpeechProvider + Sync,
    C: CleanupProvider + Sync,
{
    let seen = Shared::default();
    let parts = Parts {
        mic: NoMicrophone,
        speech: KeepRaw {
            inner: speech,
            seen: seen.clone(),
        },
        cleanup: Borrowed(cleanup),
        inserter: KeepText(seen.clone()),
        focus: NoFocus,
        history: NoHistory,
        sounds: NoSounds,
        timing: KeepTiming(seen.clone()),
    };
    let settings = Settings {
        sounds: false,
        timing_log: true,
        ..settings
    };
    let (events, events_rx) = mpsc::channel(1024);
    let (status_tx, mut statuses) = mpsc::unbounded_channel();
    let engine = Engine::new(parts, settings, status_tx);

    let drive = async {
        let mut out = Vec::new();
        for sample in samples {
            with(&seen, |s| *s = Seen::default());
            let outcome = dictate(&events, &mut statuses, &sample.pcm).await;
            let (raw, pasted, timing) =
                with(&seen, |s| (s.raw.take(), s.pasted.take(), s.timing.take()));
            let outcome = outcome.and_then(|fallback| match (pasted, timing) {
                (Some(text), Some(t)) => Ok(Pasted {
                    text,
                    fallback,
                    stt_ms: t.stt_ms,
                    first_ms: t.first_ms,
                    total_ms: t.total_ms,
                }),
                _ => Err("the engine reported a paste but no text or timings".to_owned()),
            });
            let replayed = Replayed {
                sample: sample.name.clone(),
                lang: Lang::of_sample(&sample.name),
                raw,
                outcome,
            };
            each(&replayed);
            out.push(replayed);
        }
        // Closing the channel ends the engine.
        drop(events);
        out
    };
    let ((), out) = tokio::join!(engine.run(events_rx), drive);
    out
}

/// One dictation: key-down, the audio in real time, key-up, then the engine's statuses until it
/// is idle again. Returns whether the raw transcript was pasted, or why nothing was.
async fn dictate(
    events: &mpsc::Sender<Event>,
    statuses: &mut mpsc::UnboundedReceiver<Status>,
    pcm: &[i16],
) -> Result<bool, String> {
    let closed = |_| "the engine stopped".to_owned();
    events.send(Event::KeyDown).await.map_err(closed)?;
    let frame = frame_samples(SAMPLE_RATE);
    let mut tick = interval(Duration::from_millis(
        1000 * frame as u64 / u64::from(SAMPLE_RATE),
    ));
    tick.set_missed_tick_behavior(MissedTickBehavior::Burst);
    for chunk in pcm.chunks(frame) {
        tick.tick().await;
        events
            .send(Event::Audio(chunk.to_vec()))
            .await
            .map_err(closed)?;
    }
    tick.tick().await;
    events.send(Event::KeyUp).await.map_err(closed)?;

    let mut ended: Option<Result<bool, String>> = None;
    let waited = timeout(AFTER_KEY_UP, async {
        while let Some(status) = statuses.recv().await {
            match status {
                Status::Done { raw } | Status::Copied { raw } => ended = Some(Ok(raw)),
                Status::Failed { message, .. } => ended = Some(Err(message)),
                Status::NothingHeard => ended = Some(Err("nothing heard".to_owned())),
                Status::Cancelled => {
                    ended = Some(Err(
                        "cancelled: shorter than the minimum hold, or past the maximum length"
                            .to_owned(),
                    ));
                }
                Status::SecureInput => ended = Some(Err("secure input".to_owned())),
                Status::Idle if ended.is_some() => return,
                _ => {}
            }
        }
    })
    .await;
    match (waited, ended) {
        (Ok(()), Some(result)) => result,
        (Ok(()), None) => Err("the engine stopped".to_owned()),
        (Err(_), _) => Err(format!(
            "no result {} s after key-up",
            AFTER_KEY_UP.as_secs()
        )),
    }
}
