//! Fakes for every trait the state machine uses, and a harness that runs it. They record what
//! happened, with the time on tokio's clock, in one shared [`Log`].

#![allow(dead_code)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use hyperme_core::dictation::{Engine, Event, Settings, Status};
use hyperme_core::traits::{
    CleanupProvider, Error, ErrorKind, FocusInfo, Inserter, LiveSession, Microphone,
    SpeechProvider, Transcript,
};
use tokio::sync::mpsc;
use tokio::time::{Instant, sleep};

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
    pub finish_error: Option<Error>,
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
            finish_error: None,
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
        if let Some(error) = self.speech.finish_error.clone() {
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
}

pub struct FakeFocus;

impl FocusInfo for FakeFocus {
    fn frontmost_app(&self) -> Option<String> {
        Some("TextEdit".to_owned())
    }
}

pub fn settings() -> Settings {
    Settings::from_config(&hyperme_core::config::Config::default())
}

/// A running engine: send events, read statuses.
pub struct Harness {
    pub events: mpsc::Sender<Event>,
    pub status: mpsc::UnboundedReceiver<Status>,
    pub log: Log,
}

impl Harness {
    pub fn start(log: &Log, speech: FakeSpeech, cleanup: FakeCleanup, settings: Settings) -> Self {
        let (events, events_rx) = mpsc::channel(256);
        let (status_tx, status) = mpsc::unbounded_channel();
        let engine = Engine::new(
            FakeMic { log: log.clone() },
            speech,
            cleanup,
            FakeInserter { log: log.clone() },
            FakeFocus,
            settings,
            status_tx,
        );
        // Spawned on the runtime, which proves the engine's future is Send.
        tokio::spawn(engine.run(events_rx));
        Harness {
            events,
            status,
            log: log.clone(),
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
