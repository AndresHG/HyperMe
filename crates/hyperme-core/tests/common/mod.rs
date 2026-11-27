//! Fakes for every trait the state machine uses, and a harness that runs it. They record what
//! happened, with the time on tokio's clock, in one shared [`Log`].

#![allow(dead_code)]

pub mod tasks;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use std::collections::BTreeMap;

use hyperme_core::dictation::{Engine, Event, Parts, Providers, Settings, Status};
use hyperme_core::trace::{FinishedTrace, TimingSink, TraceLayer, TraceSink};
use hyperme_core::traits::{
    Added, CleanupProvider, Correction, DictationId, Dictionary, Error, ErrorKind, FocusInfo,
    History, Inserter, LiveSession, Microphone, Sound, Sounds, SpeechProvider, TimingLine,
    TimingLog, Transcript,
};
use std::sync::atomic::AtomicBool;
use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, sleep};
use tracing::instrument::WithSubscriber;
use tracing_subscriber::layer::SubscriberExt;

/// What happened, in order, with when.
#[derive(Clone)]
pub struct Log {
    start: Instant,
    entries: Arc<Mutex<Vec<(Duration, String)>>>,
}

impl Log {
    pub fn new() -> Self {
        Log {
            start: Instant::now(),
            entries: Arc::default(),
        }
    }

    pub fn push(&self, what: impl Into<String>) {
        let at = self.start.elapsed();
        self.entries.lock().unwrap().push((at, what.into()));
    }

    pub fn entries(&self) -> Vec<String> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .map(|(_, what)| what.clone())
            .collect()
    }

    /// When `what` first happened, in ms from the log's start.
    pub fn at(&self, what: &str) -> Option<u128> {
        let entries = self.entries.lock().unwrap();
        entries
            .iter()
            .find(|(_, w)| w == what)
            .map(|(at, _)| at.as_millis())
    }

    pub fn has(&self, what: &str) -> bool {
        self.at(what).is_some()
    }

    /// Index of the first entry equal to `what`.
    pub fn index(&self, what: &str) -> usize {
        self.entries()
            .iter()
            .position(|w| w == what)
            .unwrap_or_else(|| panic!("no `{what}` in {:?}", self.entries()))
    }

    pub fn pasted(&self) -> Vec<String> {
        self.entries()
            .iter()
            .filter_map(|w| w.strip_prefix("paste: ").map(str::to_owned))
            .collect()
    }
}

pub fn err(kind: ErrorKind, message: &str) -> Error {
    Error {
        kind,
        message: message.to_owned(),
    }
}

pub struct FakeMic {
    pub log: Log,
}

impl Microphone for FakeMic {
    fn start(&mut self) -> Result<(), Error> {
        self.log.push("mic start");
        Ok(())
    }

    fn stop(&mut self) {
        self.log.push("mic stop");
    }
}

#[derive(Clone)]
pub struct FakeSpeech {
    pub log: Log,
    pub open_delay: Duration,
    pub open_error: Option<Error>,
    pub finish_delay: Duration,
    /// Shared by every session, so a test can make the next attempt succeed.
    pub finish_error: Arc<Mutex<Option<Error>>>,
    pub transcript: String,
    pub language: Option<String>,
}

impl FakeSpeech {
    pub fn new(log: &Log, transcript: &str) -> Self {
        FakeSpeech {
            log: log.clone(),
            open_delay: Duration::ZERO,
            open_error: None,
            finish_delay: Duration::ZERO,
            finish_error: Arc::default(),
            transcript: transcript.to_owned(),
            language: Some("en".to_owned()),
        }
    }
}

pub struct FakeSession {
    speech: FakeSpeech,
}

impl SpeechProvider for FakeSpeech {
    type Session = FakeSession;

    async fn open(&self) -> Result<FakeSession, Error> {
        self.log.push("speech open start");
        if !self.open_delay.is_zero() {
            sleep(self.open_delay).await;
        }
        if let Some(error) = self.open_error.clone() {
            self.log.push("speech open failed");
            return Err(error);
        }
        self.log.push("speech opened");
        Ok(FakeSession {
            speech: self.clone(),
        })
    }
}

impl LiveSession for FakeSession {
    async fn send_audio(&mut self, pcm: &[i16]) -> Result<(), Error> {
        let first = pcm.first().copied().unwrap_or_default();
        self.speech
            .log
            .push(format!("audio {} from {first}", pcm.len()));
        Ok(())
    }

    async fn finish(self) -> Result<Transcript, Error> {
        let log = self.speech.log.clone();
        log.push("finalize");
        if !self.speech.finish_delay.is_zero() {
            sleep(self.speech.finish_delay).await;
        }
        if let Some(error) = self.speech.finish_error.lock().unwrap().clone() {
            return Err(error);
        }
        log.push("final transcript");
        Ok(Transcript {
            text: self.speech.transcript.clone(),
            language: self.speech.language.clone(),
        })
    }
}

impl Drop for FakeSession {
    fn drop(&mut self) {
        self.speech.log.push("session closed");
    }
}

pub struct FakeCleanup {
    pub log: Log,
    pub warm_delay: Duration,
    pub delay: Duration,
    pub reply: Result<String, Error>,
}

impl FakeCleanup {
    pub fn new(log: &Log, reply: &str) -> Self {
        FakeCleanup {
            log: log.clone(),
            warm_delay: Duration::ZERO,
            delay: Duration::ZERO,
            reply: Ok(reply.to_owned()),
        }
    }
}

impl CleanupProvider for FakeCleanup {
    fn set_dictionary(&self, line: &str) {
        self.log.push(format!("dictionary:{line}"));
    }

    async fn warm(&self) -> Result<(), Error> {
        self.log.push("warm start");
        if !self.warm_delay.is_zero() {
            sleep(self.warm_delay).await;
        }
        self.log.push("warmed");
        Ok(())
    }

    async fn clean(
        &self,
        raw: &str,
        app: Option<&str>,
        on_text: &mut (dyn FnMut(&str) + Send),
    ) -> Result<(), Error> {
        self.log
            .push(format!("clean: {raw} [{}]", app.unwrap_or("-")));
        if !self.delay.is_zero() {
            sleep(self.delay).await;
        }
        let reply = self.reply.clone()?;
        // Two pieces, as a stream would deliver them.
        let (a, b) = reply.split_at(reply.len() / 2);
        on_text(a);
        on_text(b);
        Ok(())
    }
}

pub struct FakeInserter {
    pub log: Log,
}

impl Inserter for FakeInserter {
    async fn paste(&mut self, text: &str) -> Result<(), Error> {
        self.log.push(format!("paste: {text}"));
        Ok(())
    }

    async fn copy(&mut self, text: &str) -> Result<(), Error> {
        self.log.push(format!("copy: {text}"));
        Ok(())
    }
}

/// A history in memory. Logs each call; keeps each dictation's audio.
#[derive(Clone)]
pub struct FakeHistory {
    pub log: Log,
    pub audio: Arc<Mutex<BTreeMap<DictationId, Vec<i16>>>>,
    /// The app each dictation was dictated into.
    apps: Arc<Mutex<BTreeMap<DictationId, Option<String>>>>,
    next: Arc<Mutex<DictationId>>,
}

impl FakeHistory {
    pub fn new(log: &Log) -> Self {
        FakeHistory {
            log: log.clone(),
            audio: Arc::default(),
            apps: Arc::default(),
            next: Arc::default(),
        }
    }

    pub fn samples(&self, id: DictationId) -> usize {
        self.audio.lock().unwrap().get(&id).map_or(0, Vec::len)
    }
}

impl History for FakeHistory {
    fn begin(&mut self, app: Option<&str>) -> Result<DictationId, Error> {
        let mut next = self.next.lock().unwrap();
        *next += 1;
        self.audio.lock().unwrap().insert(*next, Vec::new());
        self.apps
            .lock()
            .unwrap()
            .insert(*next, app.map(str::to_owned));
        self.log
            .push(format!("history begin {} [{}]", *next, app.unwrap_or("-")));
        Ok(*next)
    }

    fn append(&mut self, id: DictationId, pcm: &[i16]) -> Result<(), Error> {
        self.audio
            .lock()
            .unwrap()
            .get_mut(&id)
            .unwrap()
            .extend_from_slice(pcm);
        Ok(())
    }

    fn done(&mut self, id: DictationId, text: &str, language: Option<&str>) -> Result<(), Error> {
        self.log.push(format!(
            "history done {id}: {text} ({})",
            language.unwrap_or("-")
        ));
        Ok(())
    }

    fn failed(&mut self, id: DictationId, _message: &str) -> Result<(), Error> {
        self.log.push(format!("history failed {id}"));
        Ok(())
    }

    fn discard(&mut self, id: DictationId) -> Result<(), Error> {
        self.audio.lock().unwrap().remove(&id);
        self.log.push(format!("history discard {id}"));
        Ok(())
    }

    fn audio(&self, id: DictationId) -> Result<Vec<i16>, Error> {
        self.audio
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| err(ErrorKind::Other, "no such dictation"))
    }

    fn app(&self, id: DictationId) -> Result<Option<String>, Error> {
        self.apps
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| err(ErrorKind::Other, "no such dictation"))
    }

    fn replace_text(
        &mut self,
        id: DictationId,
        text: &str,
        language: Option<&str>,
    ) -> Result<(), Error> {
        self.log.push(format!(
            "history replace {id}: {text} ({})",
            language.unwrap_or("-")
        ));
        Ok(())
    }
}

/// A dictionary that records every correction and answers from a queue: `Ok(Added::Added)`
/// once the queue is empty (#98 R-20).
#[derive(Default)]
pub struct FakeDictionary {
    pub corrections: Vec<Correction>,
    pub answers: std::collections::VecDeque<Result<Added, Error>>,
}

impl Dictionary for FakeDictionary {
    fn add_correction(&mut self, c: Correction) -> Result<Added, Error> {
        self.corrections.push(c);
        self.answers.pop_front().unwrap_or(Ok(Added::Added))
    }
}

/// Collects timing lines.
#[derive(Clone, Default)]
pub struct FakeTiming {
    pub lines: Arc<Mutex<Vec<TimingLine>>>,
}

/// Collects every finished trace (spec 182).
#[derive(Clone, Default)]
pub struct FakeTraces {
    pub traces: Arc<Mutex<Vec<FinishedTrace>>>,
}

impl FakeTraces {
    pub fn all(&self) -> Vec<FinishedTrace> {
        self.traces.lock().unwrap().clone()
    }
}

impl TraceSink for FakeTraces {
    fn finished(&self, trace: &FinishedTrace) {
        self.traces.lock().unwrap().push(trace.clone());
    }
}

impl TimingLog for FakeTiming {
    fn record(&mut self, line: &TimingLine) {
        self.lines.lock().unwrap().push(line.clone());
    }
}

pub struct FakeSounds {
    pub log: Log,
}

impl Sounds for FakeSounds {
    fn play(&self, sound: Sound) {
        self.log.push(format!("sound {sound:?}"));
    }
}

pub struct FakeFocus {
    /// Whether a password field has focus (secure input on).
    pub secure: bool,
    /// The frontmost app; changeable while the engine runs.
    pub app: Arc<Mutex<String>>,
}

impl FakeFocus {
    pub fn textedit() -> Self {
        FakeFocus {
            secure: false,
            app: Arc::new(Mutex::new("TextEdit".to_owned())),
        }
    }
}

impl FocusInfo for FakeFocus {
    fn frontmost_app(&self) -> Option<String> {
        Some(self.app.lock().unwrap().clone())
    }

    fn secure_input(&self) -> bool {
        self.secure
    }
}

pub fn settings() -> Settings {
    Settings::from_config(&hyperme_core::config::Config::default())
}

/// A running engine: send events, read statuses.
pub struct Harness {
    pub events: mpsc::Sender<Event>,
    /// Where a changed dictionary goes (spec 93, R-10a).
    pub dictionary: hyperme_core::dictation::DictionaryUpdates,
    pub status: mpsc::UnboundedReceiver<Status>,
    pub log: Log,
    /// Replaces the providers, as Settings does (specs/96-provider-settings, R-8).
    pub switch: watch::Sender<Arc<Providers<FakeSpeech, FakeCleanup>>>,
    /// Every finished trace of this engine (spec 182).
    pub traces: FakeTraces,
}

impl Harness {
    pub fn start(log: &Log, speech: FakeSpeech, cleanup: FakeCleanup, settings: Settings) -> Self {
        Self::start_with(log, speech, cleanup, settings, FakeFocus::textedit())
    }

    pub fn start_with(
        log: &Log,
        speech: FakeSpeech,
        cleanup: FakeCleanup,
        settings: Settings,
        focus: FakeFocus,
    ) -> Self {
        Self::start_full(log, speech, cleanup, settings, focus, FakeHistory::new(log))
    }

    pub fn start_timed(
        log: &Log,
        speech: FakeSpeech,
        cleanup: FakeCleanup,
        settings: Settings,
        timing: FakeTiming,
        timing_on: bool,
    ) -> Self {
        Self::start_parts(
            log,
            speech,
            cleanup,
            settings,
            FakeFocus::textedit(),
            FakeHistory::new(log),
            (timing, timing_on),
            true,
        )
    }

    /// An engine with no subscriber at all: the "before" of spec 182's overhead measure (AC-11).
    pub fn start_untraced(
        log: &Log,
        speech: FakeSpeech,
        cleanup: FakeCleanup,
        settings: Settings,
    ) -> Self {
        Self::start_parts(
            log,
            speech,
            cleanup,
            settings,
            FakeFocus::textedit(),
            FakeHistory::new(log),
            (FakeTiming::default(), false),
            false,
        )
    }

    pub fn start_full(
        log: &Log,
        speech: FakeSpeech,
        cleanup: FakeCleanup,
        settings: Settings,
        focus: FakeFocus,
        history: FakeHistory,
    ) -> Self {
        Self::start_parts(
            log,
            speech,
            cleanup,
            settings,
            focus,
            history,
            (FakeTiming::default(), false),
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn start_parts(
        log: &Log,
        speech: FakeSpeech,
        cleanup: FakeCleanup,
        settings: Settings,
        focus: FakeFocus,
        history: FakeHistory,
        (timing, timing_on): (FakeTiming, bool),
        traced: bool,
    ) -> Self {
        let (events, events_rx) = mpsc::channel(256);
        let (status_tx, status) = mpsc::unbounded_channel();
        let (switch, providers) = watch::channel(Arc::new(Providers { speech, cleanup }));
        let parts = Parts {
            mic: FakeMic { log: log.clone() },
            providers,
            inserter: FakeInserter { log: log.clone() },
            focus,
            history,
            sounds: FakeSounds { log: log.clone() },
        };
        let engine = Engine::new(parts, settings, status_tx);
        let dictionary = engine.dictionary_updates();
        // Spec 182: the engine's spans go to a timing sink and a trace collector, as in the app.
        let traces = FakeTraces::default();
        let sinks: Vec<Arc<dyn TraceSink>> = vec![
            Arc::new(TimingSink::new(
                Arc::new(AtomicBool::new(timing_on)),
                timing,
            )),
            Arc::new(traces.clone()),
        ];
        let subscriber = tracing_subscriber::registry().with(TraceLayer::new(sinks));
        // Spawned on the runtime, which proves the engine's future is Send.
        if traced {
            tokio::spawn(engine.run(events_rx).with_subscriber(subscriber));
        } else {
            tokio::spawn(
                engine
                    .run(events_rx)
                    .with_subscriber(tracing::Dispatch::none()),
            );
        }
        Harness {
            events,
            dictionary,
            status,
            log: log.clone(),
            switch,
            traces,
        }
    }

    pub async fn send(&self, event: Event) {
        self.events.send(event).await.unwrap();
    }

    /// Sends 20 ms frames of 24 kHz audio (480 samples), each starting with its index.
    pub async fn frames(&self, from: i16, count: i16) {
        for i in from..from + count {
            let mut frame = vec![0i16; 480];
            frame[0] = i;
            self.send(Event::Audio(frame)).await;
        }
    }

    /// Waits for the next status equal to `want`, returning every status seen on the way.
    pub async fn until(&mut self, want: &Status) -> Vec<Status> {
        let mut seen = Vec::new();
        loop {
            let status = tokio::time::timeout(Duration::from_secs(600), self.status.recv())
                .await
                .expect("no status within 600 s")
                .expect("engine stopped");
            let done = &status == want;
            seen.push(status);
            if done {
                return seen;
            }
        }
    }

    /// Lets the engine run until it has nothing left to do.
    pub async fn settle(&self) {
        for _ in 0..50 {
            tokio::task::yield_now().await;
        }
    }
}

pub fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
