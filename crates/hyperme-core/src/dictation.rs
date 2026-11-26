//! The dictation state machine (design.md, "The state machine").
//!
//! ```text
//! Idle -key down-> Recording -key up (held >= min) or max length-> Transcribing -final->
//!   Cleaning (skipped for short transcripts) -text or deadline-> Pasting -> Done -> Idle
//! Short hold or Esc before paste -> Cancelled -> Idle;  speech failure -> Failed -> Idle
//! ```
//!
//! One async task owns all state. Platform callbacks send it [`Event`]s over a channel; it
//! reports each [`Status`] for the pill over another. Only one dictation runs at a time.

use std::future::Future;
use std::pin::{Pin, pin};
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::{Instant, sleep_until, timeout};

use crate::config::Config;
use crate::traits::{
    CleanupProvider, Error, FocusInfo, Inserter, LiveSession, Microphone, SpeechProvider,
};

/// The numbers the state machine needs from the config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// R-5: a shorter hold is cancelled.
    pub min_hold: Duration,
    /// R-7: recording stops here and is processed.
    pub max_length: Duration,
    /// R-13: transcripts with fewer words skip cleanup.
    pub short_words: usize,
    /// R-12: after this, the raw transcript is used.
    pub cleanup_deadline: Duration,
}

impl Settings {
    pub fn from_config(config: &Config) -> Self {
        Settings {
            min_hold: Duration::from_millis(config.dictation.min_hold_ms),
            max_length: Duration::from_secs(config.dictation.max_length_s),
            short_words: config.dictation.short_dictation_words as usize,
            cleanup_deadline: Duration::from_millis(config.cleanup.deadline_ms),
        }
    }
}

/// What the platform glue tells the state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The dictation hotkey went down.
    KeyDown,
    /// The dictation hotkey went up.
    KeyUp,
    /// Esc: cancel (R-6).
    Esc,
    /// One captured frame of mono PCM16 at the speech provider's rate.
    Audio(Vec<i16>),
}

/// What the pill shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Idle,
    Recording,
    Transcribing,
    Cleaning,
    Pasting,
    /// The text was pasted. `raw` when it is the raw transcript after a cleanup failure or the
    /// deadline (R-12, R-20).
    Done {
        raw: bool,
    },
    /// Short hold or Esc: nothing pasted.
    Cancelled,
    /// R-14.
    NothingHeard,
    /// A key-down while a dictation is still being processed; it is ignored.
    StillWorking,
    /// Nothing pasted; the message says why (R-19).
    Failed(String),
}

pub struct Engine<M, S, C, I, F> {
    mic: M,
    speech: S,
    cleanup: C,
    inserter: I,
    focus: F,
    settings: Settings,
    status: mpsc::UnboundedSender<Status>,
}

impl<M, S, C, I, F> std::fmt::Debug for Engine<M, S, C, I, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

/// How a dictation ended, for the run loop.
#[derive(Debug, PartialEq, Eq)]
enum Flow {
    Ended,
    /// The event channel closed: the app is quitting.
    Closed,
}

/// The outcome of waiting on a future while still listening for Esc.
enum Waited<T> {
    Done(T),
    Cancelled,
    Closed,
}

impl<M, S, C, I, F> Engine<M, S, C, I, F>
where
    M: Microphone + Send,
    S: SpeechProvider + Sync,
    C: CleanupProvider + Sync,
    I: Inserter + Send,
    F: FocusInfo + Send,
{
    pub fn new(
        mic: M,
        speech: S,
        cleanup: C,
        inserter: I,
        focus: F,
        settings: Settings,
        status: mpsc::UnboundedSender<Status>,
    ) -> Self {
        Engine {
            mic,
            speech,
            cleanup,
            inserter,
            focus,
            settings,
            status,
        }
    }

    /// Runs until the event channel closes.
    pub async fn run(mut self, mut events: mpsc::Receiver<Event>) {
        while let Some(event) = events.recv().await {
            if event != Event::KeyDown {
                continue;
            }
            let flow = self.dictate(&mut events).await;
            let _ = self.status.send(Status::Idle);
            if flow == Flow::Closed {
                return;
            }
        }
    }

    async fn dictate(&mut self, events: &mut mpsc::Receiver<Event>) -> Flow {
        let Engine {
            mic,
            speech,
            cleanup,
            inserter,
            focus,
            settings,
            status,
        } = self;
        let emit = |s: Status| {
            // The pill may be gone while the app quits; the dictation goes on regardless.
            let _ = status.send(s);
        };
        let start = Instant::now();

        // R-1, R-2: capture, the speech session and the cleanup warm-up start together; the
        // state is Recording before either connection completes (AC-1).
        if let Err(error) = mic.start() {
            emit(Status::Failed(format!("Microphone: {error}")));
            return Flow::Ended;
        }
        emit(Status::Recording);
        let mut open = pin!(speech.open());
        let mut opening = true;
        let mut warm = pin!(async {
            // A failed warm-up shows up again, with its message, in the cleanup call.
            let _ = cleanup.warm().await;
        });
        let mut warming = true;
        let mut session: Option<S::Session> = None;
        // Audio captured before the session opens, sent when it does.
        let mut buffer: Vec<i16> = Vec::new();
        let mut max_length = pin!(sleep_until(start + settings.max_length));

        loop {
            tokio::select! {
                event = events.recv() => match event {
                    None => {
                        mic.stop();
                        return Flow::Closed;
                    }
                    Some(Event::Audio(pcm)) => match session.as_mut() {
                        Some(live) => {
                            if let Err(error) = live.send_audio(&pcm).await {
                                mic.stop();
                                emit(speech_failed(&error));
                                return Flow::Ended;
                            }
                        }
                        None => buffer.extend_from_slice(&pcm),
                    },
                    // R-5: a tap is cancelled, and dropping the session closes it (AC-4).
                    Some(Event::KeyUp) if start.elapsed() < settings.min_hold => {
                        mic.stop();
                        emit(Status::Cancelled);
                        return Flow::Ended;
                    }
                    Some(Event::KeyUp) => break,
                    // R-6.
                    Some(Event::Esc) => {
                        mic.stop();
                        emit(Status::Cancelled);
                        return Flow::Ended;
                    }
                    Some(Event::KeyDown) => {}
                },
                result = &mut open, if opening => {
                    opening = false;
                    match result {
                        Ok(mut live) => {
                            if !buffer.is_empty() {
                                if let Err(error) = live.send_audio(&buffer).await {
                                    mic.stop();
                                    emit(speech_failed(&error));
                                    return Flow::Ended;
                                }
                                buffer.clear();
                            }
                            session = Some(live);
                        }
                        // R-19.
                        Err(error) => {
                            mic.stop();
                            emit(speech_failed(&error));
                            return Flow::Ended;
                        }
                    }
                }
                () = &mut warm, if warming => warming = false,
                // R-7: as if the key had been released (AC-5).
                () = &mut max_length => break,
            }
        }

        mic.stop();
        let app = focus.frontmost_app();
        emit(Status::Transcribing);

        let mut live = match session {
            Some(live) => live,
            None => match wait(open.as_mut(), events, warm.as_mut(), &mut warming, &emit).await {
                Waited::Done(Ok(live)) => live,
                Waited::Done(Err(error)) => {
                    emit(speech_failed(&error));
                    return Flow::Ended;
                }
                Waited::Cancelled => {
                    emit(Status::Cancelled);
                    return Flow::Ended;
                }
                Waited::Closed => return Flow::Closed,
            },
        };
        if !buffer.is_empty() {
            if let Err(error) = live.send_audio(&buffer).await {
                emit(speech_failed(&error));
                return Flow::Ended;
            }
        }

        // R-9, AC-6: the explicit finalize, then the final transcript.
        let finish = pin!(live.finish());
        let transcript = match wait(finish, events, warm.as_mut(), &mut warming, &emit).await {
            Waited::Done(Ok(transcript)) => transcript,
            Waited::Done(Err(error)) => {
                emit(speech_failed(&error));
                return Flow::Ended;
            }
            Waited::Cancelled => {
                emit(Status::Cancelled);
                return Flow::Ended;
            }
            Waited::Closed => return Flow::Closed,
        };

        let raw = transcript.text.trim();
        let words = raw.split_whitespace().count();
        // R-14.
        if words == 0 {
            emit(Status::NothingHeard);
            return Flow::Ended;
        }
        let (text, used_raw) = if words < settings.short_words {
            // R-13: no cleanup for short dictations.
            (format_short(raw), false)
        } else {
            emit(Status::Cleaning);
            let mut cleaned = String::new();
            let outcome = {
                let mut on_text = |piece: &str| cleaned.push_str(piece);
                let clean = pin!(timeout(
                    settings.cleanup_deadline,
                    cleanup.clean(raw, app.as_deref(), &mut on_text),
                ));
                wait(clean, events, warm.as_mut(), &mut warming, &emit).await
            };
            match outcome {
                Waited::Done(Ok(Ok(()))) if !cleaned.trim().is_empty() => {
                    (cleaned.trim().to_owned(), false)
                }
                // R-12 (deadline), R-20 (error) and an empty answer: the raw transcript.
                Waited::Done(_) => (raw.to_owned(), true),
                Waited::Cancelled => {
                    emit(Status::Cancelled);
                    return Flow::Ended;
                }
                Waited::Closed => return Flow::Closed,
            }
        };

        emit(Status::Pasting);
        if let Err(error) = inserter.paste(&text).await {
            emit(Status::Failed(format!("Paste: {error}")));
            return Flow::Ended;
        }
        emit(Status::Done { raw: used_raw });
        Flow::Ended
    }
}

fn speech_failed(error: &Error) -> Status {
    Status::Failed(format!("Speech: {error}"))
}

/// Waits for `fut` while still handling events: Esc cancels, a key-down says "Still working",
/// anything else is ignored. Keeps driving the cleanup warm-up so it is not dropped half-way.
async fn wait<T>(
    mut fut: Pin<&mut impl Future<Output = T>>,
    events: &mut mpsc::Receiver<Event>,
    mut warm: Pin<&mut impl Future<Output = ()>>,
    warming: &mut bool,
    emit: &impl Fn(Status),
) -> Waited<T> {
    loop {
        tokio::select! {
            out = &mut fut => return Waited::Done(out),
            () = &mut warm, if *warming => *warming = false,
            event = events.recv() => match event {
                None => return Waited::Closed,
                Some(Event::Esc) => return Waited::Cancelled,
                Some(Event::KeyDown) => emit(Status::StillWorking),
                Some(Event::KeyUp | Event::Audio(_)) => {}
            },
        }
    }
}

/// R-13: a short transcript with its first letter capitalised and a full stop added when it
/// ends without one.
pub fn format_short(raw: &str) -> String {
    let raw = raw.trim();
    let mut text = String::with_capacity(raw.len() + 1);
    let mut capitalised = false;
    for c in raw.chars() {
        if !capitalised && c.is_alphabetic() {
            text.extend(c.to_uppercase());
            capitalised = true;
        } else {
            text.push(c);
        }
    }
    if !text.ends_with(['.', '!', '?', '…']) {
        text.push('.');
    }
    text
}
