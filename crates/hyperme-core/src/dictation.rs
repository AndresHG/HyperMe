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
    CleanupProvider, DictationId, Error, FocusInfo, History, Inserter, LiveSession, Microphone,
    Sound, Sounds, SpeechProvider,
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
    /// R-32: the start and stop sounds.
    pub sounds: bool,
}

impl Settings {
    pub fn from_config(config: &Config) -> Self {
        Settings {
            min_hold: Duration::from_millis(config.dictation.min_hold_ms),
            max_length: Duration::from_secs(config.dictation.max_length_s),
            short_words: config.dictation.short_dictation_words as usize,
            cleanup_deadline: Duration::from_millis(config.cleanup.deadline_ms),
            sounds: config.dictation.sounds,
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
    /// Retry the last failed dictation from its saved audio (R-31), from the pill or the menu.
    Retry,
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
    /// A key-down while a password field has focus: nothing is recorded (R-17). The pill says
    /// "Not available in password fields".
    SecureInput,
    /// Nothing pasted; the message says why (R-19). With `retry`, the audio is saved and the
    /// pill and menu offer Retry (R-31).
    Failed {
        message: String,
        retry: bool,
    },
    /// A retry finished after the user left the app it was dictated into, so the text was
    /// copied to the clipboard instead of pasted (R-31).
    Copied {
        raw: bool,
    },
}

/// Everything the state machine drives, one implementation per trait.
#[derive(Debug)]
pub struct Parts<M, S, C, I, F, H, N> {
    pub mic: M,
    pub speech: S,
    pub cleanup: C,
    pub inserter: I,
    pub focus: F,
    pub history: H,
    pub sounds: N,
}

pub struct Engine<M, S, C, I, F, H, N> {
    parts: Parts<M, S, C, I, F, H, N>,
    settings: Settings,
    status: mpsc::UnboundedSender<Status>,
    /// The last dictation that failed with its audio saved, for Retry (R-31).
    failed: Option<FailedDictation>,
}

impl<M, S, C, I, F, H, N> std::fmt::Debug for Engine<M, S, C, I, F, H, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
struct FailedDictation {
    id: DictationId,
    /// The app that was in front when it was dictated.
    app: Option<String>,
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

/// Why transcription and cleanup ended without text.
enum NoText {
    Cancelled,
    NothingHeard,
    Speech(Error),
    Closed,
}

/// The text to insert.
struct Text {
    text: String,
    /// The raw transcript, after a cleanup failure or the deadline.
    raw: bool,
    language: Option<String>,
}

impl<M, S, C, I, F, H, N> Engine<M, S, C, I, F, H, N>
where
    M: Microphone + Send,
    S: SpeechProvider + Sync,
    C: CleanupProvider + Sync,
    I: Inserter + Send,
    F: FocusInfo + Send,
    H: History + Send,
    N: Sounds + Send,
{
    pub fn new(
        parts: Parts<M, S, C, I, F, H, N>,
        settings: Settings,
        status: mpsc::UnboundedSender<Status>,
    ) -> Self {
        Engine {
            parts,
            settings,
            status,
            failed: None,
        }
    }

    /// Runs until the event channel closes.
    pub async fn run(mut self, mut events: mpsc::Receiver<Event>) {
        while let Some(event) = events.recv().await {
            let flow = match event {
                Event::KeyDown => self.dictate(&mut events).await,
                Event::Retry => self.retry(&mut events).await,
                Event::KeyUp | Event::Esc | Event::Audio(_) => continue,
            };
            let _ = self.status.send(Status::Idle);
            if flow == Flow::Closed {
                return;
            }
        }
    }

    async fn dictate(&mut self, events: &mut mpsc::Receiver<Event>) -> Flow {
        let Engine {
            parts,
            settings,
            status,
            failed,
        } = self;
        let Parts {
            mic,
            speech,
            cleanup,
            inserter,
            focus,
            history,
            sounds,
        } = parts;
        let emit = |s: Status| {
            // The pill may be gone while the app quits; the dictation goes on regardless.
            let _ = status.send(s);
        };
        // R-17: never record while a password field has focus.
        if focus.secure_input() {
            emit(Status::SecureInput);
            return Flow::Ended;
        }
        let start = Instant::now();
        // R-11: the app the user is dictating into.
        let app = focus.frontmost_app();
        // R-18, R-31: audio goes to disk as it is captured. A history that cannot be written
        // does not stop the dictation; it only loses the retry.
        let id = history.begin(app.as_deref()).ok();
        let stop_mic = |mic: &mut M| {
            mic.stop();
            if settings.sounds {
                sounds.play(Sound::Stop);
            }
        };
        let mut fail_speech = |history: &mut H, error: &Error| {
            let retry = match id {
                Some(id) => {
                    let kept = history.failed(id, &error.message).is_ok();
                    if kept {
                        *failed = Some(FailedDictation {
                            id,
                            app: app.clone(),
                        });
                    }
                    kept
                }
                None => false,
            };
            emit(Status::Failed {
                message: format!("Speech: {error}"),
                retry,
            });
        };
        let discard = |history: &mut H| {
            if let Some(id) = id {
                let _ = history.discard(id);
            }
        };

        // R-1, R-2: capture, the speech session and the cleanup warm-up start together; the
        // state is Recording before either connection completes (AC-1).
        if let Err(error) = mic.start() {
            discard(history);
            emit(Status::Failed {
                message: format!("Microphone: {error}"),
                retry: false,
            });
            return Flow::Ended;
        }
        // R-32.
        if settings.sounds {
            sounds.play(Sound::Start);
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
                        stop_mic(mic);
                        discard(history);
                        return Flow::Closed;
                    }
                    Some(Event::Audio(pcm)) => {
                        if let Some(id) = id {
                            let _ = history.append(id, &pcm);
                        }
                        match session.as_mut() {
                            Some(live) => {
                                if let Err(error) = live.send_audio(&pcm).await {
                                    stop_mic(mic);
                                    fail_speech(history, &error);
                                    return Flow::Ended;
                                }
                            }
                            None => buffer.extend_from_slice(&pcm),
                        }
                    }
                    // R-5: a tap is cancelled, and dropping the session closes it (AC-4).
                    Some(Event::KeyUp) if start.elapsed() < settings.min_hold => {
                        stop_mic(mic);
                        discard(history);
                        emit(Status::Cancelled);
                        return Flow::Ended;
                    }
                    Some(Event::KeyUp) => break,
                    // R-6.
                    Some(Event::Esc) => {
                        stop_mic(mic);
                        discard(history);
                        emit(Status::Cancelled);
                        return Flow::Ended;
                    }
                    Some(Event::KeyDown | Event::Retry) => {}
                },
                result = &mut open, if opening => {
                    opening = false;
                    match result {
                        Ok(mut live) => {
                            if !buffer.is_empty() {
                                if let Err(error) = live.send_audio(&buffer).await {
                                    stop_mic(mic);
                                    fail_speech(history, &error);
                                    return Flow::Ended;
                                }
                                buffer.clear();
                            }
                            session = Some(live);
                        }
                        // R-19, R-31: the audio so far is kept for a retry.
                        Err(error) => {
                            stop_mic(mic);
                            fail_speech(history, &error);
                            return Flow::Ended;
                        }
                    }
                }
                () = &mut warm, if warming => warming = false,
                // R-7: as if the key had been released (AC-5).
                () = &mut max_length => break,
            }
        }

        stop_mic(mic);
        emit(Status::Transcribing);
        let live = match session {
            Some(live) => live,
            None => match wait(open.as_mut(), events, warm.as_mut(), &mut warming, &emit).await {
                Waited::Done(Ok(live)) => live,
                Waited::Done(Err(error)) => {
                    fail_speech(history, &error);
                    return Flow::Ended;
                }
                Waited::Cancelled => {
                    discard(history);
                    emit(Status::Cancelled);
                    return Flow::Ended;
                }
                Waited::Closed => return Flow::Closed,
            },
        };
        let text = transcribe_and_clean(
            live,
            &buffer,
            cleanup,
            settings,
            app.as_deref(),
            events,
            warm.as_mut(),
            &mut warming,
            &emit,
        )
        .await;
        let text = match text {
            Ok(text) => text,
            Err(NoText::Speech(error)) => {
                fail_speech(history, &error);
                return Flow::Ended;
            }
            Err(NoText::Cancelled | NoText::NothingHeard) => {
                discard(history);
                return Flow::Ended;
            }
            Err(NoText::Closed) => return Flow::Closed,
        };

        emit(Status::Pasting);
        if let Err(error) = inserter.paste(&text.text).await {
            emit(Status::Failed {
                message: format!("Paste: {error}"),
                retry: false,
            });
            return Flow::Ended;
        }
        if let Some(id) = id {
            let _ = history.done(id, &text.text, text.language.as_deref());
        }
        emit(Status::Done { raw: text.raw });
        Flow::Ended
    }

    /// R-31: sends the last failed dictation's saved audio again; pastes the result, or copies
    /// it when the app it was dictated into is no longer in front.
    async fn retry(&mut self, events: &mut mpsc::Receiver<Event>) -> Flow {
        let Some(failed) = self.failed.take() else {
            return Flow::Ended;
        };
        let Engine {
            parts,
            settings,
            status,
            failed: slot,
        } = self;
        let emit = |s: Status| {
            let _ = status.send(s);
        };
        let Parts {
            speech,
            cleanup,
            inserter,
            focus,
            history,
            ..
        } = parts;
        let mut keep_for_retry = |error: &Error| {
            *slot = Some(failed.clone());
            emit(Status::Failed {
                message: format!("Speech: {error}"),
                retry: true,
            });
        };
        let audio = match history.audio(failed.id) {
            Ok(audio) => audio,
            Err(error) => {
                emit(Status::Failed {
                    message: format!("Retry: {error}"),
                    retry: false,
                });
                return Flow::Ended;
            }
        };
        emit(Status::Transcribing);
        // No warm-up to drive on a retry.
        let mut warm = pin!(std::future::ready(()));
        let mut warming = false;
        let open = pin!(speech.open());
        let live = match wait(open, events, warm.as_mut(), &mut warming, &emit).await {
            Waited::Done(Ok(live)) => live,
            Waited::Done(Err(error)) => {
                keep_for_retry(&error);
                return Flow::Ended;
            }
            Waited::Cancelled => {
                *slot = Some(failed.clone());
                emit(Status::Cancelled);
                return Flow::Ended;
            }
            Waited::Closed => return Flow::Closed,
        };
        let text = transcribe_and_clean(
            live,
            &audio,
            cleanup,
            settings,
            failed.app.as_deref(),
            events,
            warm.as_mut(),
            &mut warming,
            &emit,
        )
        .await;
        let text = match text {
            Ok(text) => text,
            Err(NoText::Speech(error)) => {
                keep_for_retry(&error);
                return Flow::Ended;
            }
            Err(NoText::Cancelled) => {
                *slot = Some(failed.clone());
                return Flow::Ended;
            }
            Err(NoText::NothingHeard) => {
                let _ = history.discard(failed.id);
                return Flow::Ended;
            }
            Err(NoText::Closed) => return Flow::Closed,
        };
        let same_app = failed.app.is_some() && focus.frontmost_app() == failed.app;
        emit(Status::Pasting);
        let inserted = if same_app {
            inserter.paste(&text.text).await
        } else {
            inserter.copy(&text.text).await
        };
        if let Err(error) = inserted {
            *slot = Some(failed.clone());
            emit(Status::Failed {
                message: format!("Paste: {error}"),
                retry: true,
            });
            return Flow::Ended;
        }
        let _ = history.done(failed.id, &text.text, text.language.as_deref());
        emit(if same_app {
            Status::Done { raw: text.raw }
        } else {
            Status::Copied { raw: text.raw }
        });
        Flow::Ended
    }
}

/// From an open session to the text to insert: sends the audio not yet sent, finalizes
/// (R-9, AC-6), and cleans the transcript within the deadline (R-11 to R-14, R-20).
#[allow(clippy::too_many_arguments)]
async fn transcribe_and_clean<L: LiveSession, C: CleanupProvider>(
    mut live: L,
    unsent: &[i16],
    cleanup: &C,
    settings: &Settings,
    app: Option<&str>,
    events: &mut mpsc::Receiver<Event>,
    mut warm: Pin<&mut impl Future<Output = ()>>,
    warming: &mut bool,
    emit: &impl Fn(Status),
) -> Result<Text, NoText> {
    if !unsent.is_empty() {
        live.send_audio(unsent).await.map_err(NoText::Speech)?;
    }
    let finish = pin!(live.finish());
    let transcript = match wait(finish, events, warm.as_mut(), warming, emit).await {
        Waited::Done(Ok(transcript)) => transcript,
        Waited::Done(Err(error)) => return Err(NoText::Speech(error)),
        Waited::Cancelled => {
            emit(Status::Cancelled);
            return Err(NoText::Cancelled);
        }
        Waited::Closed => return Err(NoText::Closed),
    };
    let language = transcript.language.clone();
    let raw = transcript.text.trim();
    let words = raw.split_whitespace().count();
    // R-14.
    if words == 0 {
        emit(Status::NothingHeard);
        return Err(NoText::NothingHeard);
    }
    if words < settings.short_words {
        // R-13: no cleanup for short dictations.
        return Ok(Text {
            text: format_short(raw),
            raw: false,
            language,
        });
    }
    emit(Status::Cleaning);
    let mut cleaned = String::new();
    let outcome = {
        let mut on_text = |piece: &str| cleaned.push_str(piece);
        let clean = pin!(timeout(
            settings.cleanup_deadline,
            cleanup.clean(raw, app, &mut on_text),
        ));
        wait(clean, events, warm, warming, emit).await
    };
    match outcome {
        Waited::Done(Ok(Ok(()))) if !cleaned.trim().is_empty() => Ok(Text {
            text: cleaned.trim().to_owned(),
            raw: false,
            language,
        }),
        // R-12 (deadline), R-20 (error) and an empty answer: the raw transcript.
        Waited::Done(_) => Ok(Text {
            text: raw.to_owned(),
            raw: true,
            language,
        }),
        Waited::Cancelled => {
            emit(Status::Cancelled);
            Err(NoText::Cancelled)
        }
        Waited::Closed => Err(NoText::Closed),
    }
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
                Some(Event::KeyUp | Event::Audio(_) | Event::Retry) => {}
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
