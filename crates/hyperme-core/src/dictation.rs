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
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, sleep_until, timeout};

use crate::config::Config;
use crate::dictionary::Dictionary;
use crate::traits::{
    CleanupProvider, DictationId, Error, FocusInfo, History, Inserter, LiveSession, Microphone,
    Sound, Sounds, SpeechProvider, TimingLine, TimingLog,
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
    /// R-30: write one line of stage timings per dictation.
    pub timing_log: bool,
    /// Spec 93, R-1: the user's dictionary, applied to every final transcript. The app loads it
    /// from the `dictation.dictionary` file; empty by default.
    pub dictionary: Dictionary,
}

impl Settings {
    pub fn from_config(config: &Config) -> Self {
        Settings {
            min_hold: Duration::from_millis(config.dictation.min_hold_ms),
            max_length: Duration::from_secs(config.dictation.max_length_s),
            short_words: config.dictation.short_dictation_words as usize,
            cleanup_deadline: Duration::from_millis(config.cleanup.deadline_ms),
            sounds: config.dictation.sounds,
            timing_log: config.timing.log,
            dictionary: Dictionary::default(),
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
    /// Retry a dictation from its saved audio. `None`: the last failed dictation, from the
    /// pill or the menu (R-31); pasted if its app is still in front, else copied. `Some(id)`:
    /// that dictation, whatever its state, from Home (#98 R-16); always copied, since Home has
    /// focus, and its text replaced.
    Retry(Option<DictationId>),
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

/// The speech and cleanup providers one dictation uses.
#[derive(Debug)]
pub struct Providers<S, C> {
    pub speech: S,
    pub cleanup: C,
}

/// The providers in use, which Settings can replace while the app runs (specs/96-provider-
/// settings, R-8). The engine reads it once on key-down and keeps that pair for the whole
/// dictation, so a change never reaches one in progress (R-9).
pub type ProviderSwitch<S, C> = watch::Receiver<Arc<Providers<S, C>>>;

/// Providers that never change, for tools and tests with no Settings window.
pub fn fixed<S, C>(speech: S, cleanup: C) -> ProviderSwitch<S, C> {
    // The receiver keeps the last value after the sender is dropped.
    watch::channel(Arc::new(Providers { speech, cleanup })).1
}

/// Everything the state machine drives, one implementation per trait.
#[derive(Debug)]
pub struct Parts<M, S, C, I, F, H, N, T> {
    pub mic: M,
    pub providers: ProviderSwitch<S, C>,
    pub inserter: I,
    pub focus: F,
    pub history: H,
    pub sounds: N,
    pub timing: T,
}

pub struct Engine<M, S, C, I, F, H, N, T> {
    parts: Parts<M, S, C, I, F, H, N, T>,
    settings: Settings,
    status: mpsc::UnboundedSender<Status>,
    /// The last dictation that failed with its audio saved, for Retry (R-31).
    failed: Option<FailedDictation>,
    /// A dictionary saved in Settings, waiting for the next dictation (spec 93, R-10a).
    dictionary: DictionaryUpdates,
    /// The dictionary line of the last saved dictionary, given to the cleanup provider of each
    /// dictation, so it holds across a provider switch (spec 93, R-10a; specs/96, R-8).
    dictionary_line: Option<String>,
}

/// Hands a changed dictionary to a running engine (spec 93, R-10a). The newest one wins; the
/// engine picks it up when the next dictation or retry starts, so a dictation in flight keeps
/// the dictionary it started with.
#[derive(Debug, Clone, Default)]
pub struct DictionaryUpdates(Arc<Mutex<Option<Dictionary>>>);

impl DictionaryUpdates {
    pub fn send(&self, dictionary: Dictionary) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(dictionary);
    }

    fn take(&self) -> Option<Dictionary> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).take()
    }
}

impl<M, S, C, I, F, H, N, T> std::fmt::Debug for Engine<M, S, C, I, F, H, N, T> {
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

/// The text to insert, and when each stage finished (R-30).
struct Text {
    text: String,
    /// The raw transcript, after a cleanup failure or the deadline.
    raw: bool,
    language: Option<String>,
    words: usize,
    transcript_at: Instant,
    first_token_at: Option<Instant>,
    ready_at: Instant,
}

impl<M, S, C, I, F, H, N, T> Engine<M, S, C, I, F, H, N, T>
where
    M: Microphone + Send,
    S: SpeechProvider + Send + Sync,
    C: CleanupProvider + Send + Sync,
    I: Inserter + Send,
    F: FocusInfo + Send,
    H: History + Send,
    N: Sounds + Send,
    T: TimingLog + Send,
{
    pub fn new(
        parts: Parts<M, S, C, I, F, H, N, T>,
        settings: Settings,
        status: mpsc::UnboundedSender<Status>,
    ) -> Self {
        Engine {
            parts,
            settings,
            status,
            failed: None,
            dictionary: DictionaryUpdates::default(),
            dictionary_line: None,
        }
    }

    /// Where Settings sends a changed dictionary (spec 93, R-10a).
    pub fn dictionary_updates(&self) -> DictionaryUpdates {
        self.dictionary.clone()
    }

    /// Uses the newest saved dictionary, if one is waiting: substitution and the cleanup prompt.
    fn apply_dictionary_update(&mut self) {
        if let Some(dictionary) = self.dictionary.take() {
            self.dictionary_line = Some(dictionary.prompt_line());
            self.settings.dictionary = dictionary;
        }
    }

    /// Runs until the event channel closes.
    pub async fn run(mut self, mut events: mpsc::Receiver<Event>) {
        while let Some(event) = events.recv().await {
            if matches!(event, Event::KeyDown | Event::Retry(_)) {
                self.apply_dictionary_update();
            }
            let flow = match event {
                Event::KeyDown => self.dictate(&mut events).await,
                Event::Retry(None) => self.retry(&mut events).await,
                Event::Retry(Some(id)) => self.retry_by_id(id, &mut events).await,
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
            dictionary_line,
            ..
        } = self;
        let Parts {
            mic,
            providers,
            inserter,
            focus,
            history,
            sounds,
            timing,
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
        // R-8, R-9: the providers set at key-down serve this dictation start to end.
        let current = Arc::clone(&providers.borrow());
        let Providers { speech, cleanup } = &*current;
        if let Some(line) = dictionary_line.as_deref() {
            cleanup.set_dictionary(line);
        }
        // R-1, R-2: capture, the speech session and the cleanup warm-up start together; the
        // state is Recording before either connection completes (AC-1). Capture starts before
        // the app lookup and the history entry, so neither adds to key-down latency (AC-2);
        // frames wait in the event channel until the loop below appends them.
        if let Err(error) = mic.start() {
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
        let mut first_audio: Option<Instant> = None;

        loop {
            tokio::select! {
                event = events.recv() => match event {
                    None => {
                        stop_mic(mic);
                        discard(history);
                        return Flow::Closed;
                    }
                    Some(Event::Audio(pcm)) => {
                        first_audio.get_or_insert_with(Instant::now);
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
                    Some(Event::KeyDown | Event::Retry(_)) => {}
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

        let released = Instant::now();
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
        let pasted = Instant::now();
        if let Some(id) = id {
            let _ = history.done(id, &text.text, text.language.as_deref());
        }
        // R-30: numbers only, and only when the timing log is on.
        if settings.timing_log {
            let ms =
                |from: Instant, to: Instant| to.saturating_duration_since(from).as_millis() as u64;
            timing.record(&TimingLine {
                first_audio_ms: first_audio.map(|at| ms(start, at)),
                release_ms: ms(start, released),
                stt_ms: ms(released, text.transcript_at),
                first_ms: text.first_token_at.map(|at| ms(released, at)),
                total_ms: ms(released, text.ready_at),
                paste_ms: ms(released, pasted),
                language: text.language.clone(),
                words: text.words,
                fallback: text.raw,
            });
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
            dictionary_line,
            ..
        } = self;
        let emit = |s: Status| {
            let _ = status.send(s);
        };
        let Parts {
            providers,
            inserter,
            focus,
            history,
            ..
        } = parts;
        // R-9: this retry uses the providers set now, start to end.
        let current = Arc::clone(&providers.borrow());
        let Providers { speech, cleanup } = &*current;
        if let Some(line) = dictionary_line.as_deref() {
            cleanup.set_dictionary(line);
        }
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

impl<M, S, C, I, F, H, N, T> Engine<M, S, C, I, F, H, N, T>
where
    M: Microphone + Send,
    S: SpeechProvider + Sync,
    C: CleanupProvider + Sync,
    I: Inserter + Send,
    F: FocusInfo + Send,
    H: History + Send,
    N: Sounds + Send,
    T: TimingLog + Send,
{
    /// #98 R-16: sends one dictation's saved audio again, copies the new text and replaces the
    /// item's text. On any failure the old text stays. The last failed dictation, which the
    /// pill and menu retry, is left alone, unless this was that dictation and it succeeded.
    async fn retry_by_id(&mut self, id: DictationId, events: &mut mpsc::Receiver<Event>) -> Flow {
        let Engine {
            parts,
            settings,
            status,
            failed: slot,
            dictionary_line,
            ..
        } = self;
        let emit = |s: Status| {
            let _ = status.send(s);
        };
        let Parts {
            providers,
            inserter,
            history,
            ..
        } = parts;
        // specs/96, R-9: this retry uses the providers set now, start to end.
        let current = Arc::clone(&providers.borrow());
        let Providers { speech, cleanup } = &*current;
        if let Some(line) = dictionary_line.as_deref() {
            cleanup.set_dictionary(line);
        }
        // The pill and menu keep offering Retry for the last failed dictation, if any.
        let fail = |message: String| {
            emit(Status::Failed {
                message,
                retry: slot.is_some(),
            });
        };
        let (audio, app) = match history.audio(id).and_then(|a| Ok((a, history.app(id)?))) {
            Ok(found) => found,
            Err(error) => {
                fail(format!("Retry: {error}"));
                return Flow::Ended;
            }
        };
        emit(Status::Transcribing);
        let mut warm = pin!(std::future::ready(()));
        let mut warming = false;
        let open = pin!(speech.open());
        let live = match wait(open, events, warm.as_mut(), &mut warming, &emit).await {
            Waited::Done(Ok(live)) => live,
            Waited::Done(Err(error)) => {
                fail(format!("Speech: {error}"));
                return Flow::Ended;
            }
            Waited::Cancelled => {
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
                fail(format!("Speech: {error}"));
                return Flow::Ended;
            }
            // Nothing heard on a retry keeps the item as it was: it is not discarded.
            Err(NoText::Cancelled | NoText::NothingHeard) => return Flow::Ended,
            Err(NoText::Closed) => return Flow::Closed,
        };
        emit(Status::Pasting);
        if let Err(error) = inserter.copy(&text.text).await {
            fail(format!("Copy: {error}"));
            return Flow::Ended;
        }
        let _ = history.replace_text(id, &text.text, text.language.as_deref());
        if slot.as_ref().is_some_and(|f| f.id == id) {
            *slot = None;
        }
        emit(Status::Copied { raw: text.raw });
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
    let transcript_at = Instant::now();
    let language = transcript.language.clone();
    // Spec 93, R-1: the dictionary fixes the raw transcript once, so the short path, the
    // cleanup and the raw fallback all get the user's words.
    let (substituted, _) = settings.dictionary.apply(transcript.text.trim());
    let raw = substituted.as_str();
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
            words,
            transcript_at,
            first_token_at: None,
            ready_at: Instant::now(),
        });
    }
    emit(Status::Cleaning);
    let mut cleaned = String::new();
    let mut first_token_at: Option<Instant> = None;
    let outcome = {
        let mut on_text = |piece: &str| {
            if !piece.is_empty() {
                first_token_at.get_or_insert_with(Instant::now);
            }
            cleaned.push_str(piece);
        };
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
            words,
            transcript_at,
            first_token_at,
            ready_at: Instant::now(),
        }),
        // R-12 (deadline), R-20 (error) and an empty answer: the raw transcript.
        Waited::Done(_) => Ok(Text {
            text: raw.to_owned(),
            raw: true,
            language,
            words,
            transcript_at,
            first_token_at: None,
            ready_at: Instant::now(),
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
                Some(Event::KeyUp | Event::Audio(_) | Event::Retry(_)) => {}
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
