//! A fallback provider behind the main one (specs/96-provider-settings, design.md, "Fallback").
//! A wrapper, so the engine stays as it is: it calls one provider, and asks it afterwards
//! whether a fallback answered (R-27).

use std::sync::{Arc, Mutex, PoisonError};

use crate::traits::{CleanupProvider, Error, LiveSession, SpeechProvider, Transcript, Via};

/// The main provider, and a fallback with its name if one is set and ready (R-24a: a fallback
/// whose key is missing is never built, so it can neither block nor slow a dictation).
#[derive(Debug)]
pub struct Fallback<P> {
    main: P,
    fallback: Option<(P, Via)>,
    /// The fallback answered the last call.
    used: Mutex<Option<Via>>,
}

impl<P> Fallback<P> {
    pub fn new(main: P, fallback: Option<(P, Via)>) -> Self {
        Fallback {
            main,
            fallback,
            used: Mutex::new(None),
        }
    }

    /// No fallback: the main provider alone.
    pub fn alone(main: P) -> Self {
        Self::new(main, None)
    }

    pub fn main(&self) -> &P {
        &self.main
    }

    /// The fallback's name, when one is set.
    pub fn fallback(&self) -> Option<&Via> {
        self.fallback.as_ref().map(|(_, via)| via)
    }

    fn mark(&self, via: Option<Via>) {
        *self.used.lock().unwrap_or_else(PoisonError::into_inner) = via;
    }
}

impl<C: CleanupProvider + Sync> Fallback<C> {
    /// Adds the user's dictionary to both prompts (spec 93, R-4).
    pub fn with_dictionary(self, line: &str) -> Self {
        self.set_dictionary(line);
        self
    }
}

impl<C: CleanupProvider + Sync> CleanupProvider for Fallback<C> {
    fn set_dictionary(&self, line: &str) {
        self.main.set_dictionary(line);
        if let Some((fallback, _)) = &self.fallback {
            fallback.set_dictionary(line);
        }
    }

    /// Only the main provider is warmed: the fallback is called only when needed (R-30).
    async fn warm(&self) -> Result<(), Error> {
        self.main.warm().await
    }

    /// R-24: an error from the main provider before its first piece of text sends the
    /// transcript to the fallback with the same `on_text`. The engine's deadline wraps this
    /// whole call, so the raw transcript stays the last resort (Phase 1 R-12, D-6).
    async fn clean(
        &self,
        raw: &str,
        app: Option<&str>,
        on_text: &mut (dyn FnMut(&str) + Send),
    ) -> Result<(), Error> {
        self.mark(None);
        let mut started = false;
        let result = {
            let mut first = |piece: &str| {
                if !piece.is_empty() {
                    started = true;
                }
                on_text(piece);
            };
            self.main.clean(raw, app, &mut first).await
        };
        match (result, &self.fallback) {
            (Err(_), Some((fallback, via))) if !started => {
                self.mark(Some(via.clone()));
                fallback.clean(raw, app, on_text).await
            }
            (result, _) => result,
        }
    }

    fn fallback_used(&self) -> Option<Via> {
        self.used
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// A shared speech provider, so a fallback session can open the fallback itself on release
/// (R-26) without borrowing the providers it came from.
impl<S: SpeechProvider + Send + Sync> SpeechProvider for Arc<S> {
    type Session = S::Session;

    fn open(&self) -> impl std::future::Future<Output = Result<S::Session, Error>> + Send {
        (**self).open()
    }
}

impl<S> SpeechProvider for Fallback<S>
where
    S: SpeechProvider + Clone + Send + Sync,
    S::Session: Send,
{
    type Session = FallbackSession<S>;

    /// R-25: if the main session cannot open, the fallback opens at once. The engine buffers
    /// the audio captured meanwhile and sends it to whichever session it gets, so none is lost.
    /// If both fail, the main provider's error is the one shown (Phase 1 R-19).
    async fn open(&self) -> Result<FallbackSession<S>, Error> {
        let spare = self.fallback.clone();
        match self.main.open().await {
            Ok(session) => Ok(FallbackSession {
                state: State::Main(session),
                saved: Vec::new(),
                spare,
            }),
            Err(error) => match spare {
                Some((fallback, via)) => match fallback.open().await {
                    Ok(session) => Ok(FallbackSession {
                        state: State::Fallback(session, via),
                        saved: Vec::new(),
                        spare: None,
                    }),
                    Err(_) => Err(error),
                },
                None => Err(error),
            },
        }
    }
}

enum State<L> {
    Main(L),
    /// The main session dropped mid-dictation; the audio is saved for the fallback (R-26).
    Dropped(Error),
    Fallback(L, Via),
}

/// One dictation on the main speech provider, or on its fallback.
pub struct FallbackSession<S: SpeechProvider> {
    state: State<S::Session>,
    /// Every frame sent since key-down, kept only while a fallback is waiting: bounded by the
    /// maximum dictation length (design.md, "Budgets").
    saved: Vec<i16>,
    spare: Option<(S, Via)>,
}

impl<S: SpeechProvider> std::fmt::Debug for FallbackSession<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FallbackSession")
            .field("saved", &self.saved.len())
            .finish_non_exhaustive()
    }
}

impl<S> LiveSession for FallbackSession<S>
where
    S: SpeechProvider + Send + Sync,
    S::Session: Send,
{
    async fn send_audio(&mut self, pcm: &[i16]) -> Result<(), Error> {
        if self.spare.is_some() {
            self.saved.extend_from_slice(pcm);
        }
        match &mut self.state {
            State::Main(session) => match session.send_audio(pcm).await {
                Ok(()) => Ok(()),
                // R-26: keep listening; the saved audio goes to the fallback on release.
                Err(error) if self.spare.is_some() => {
                    self.state = State::Dropped(error);
                    Ok(())
                }
                Err(error) => Err(error),
            },
            State::Dropped(_) => Ok(()),
            State::Fallback(session, _) => session.send_audio(pcm).await,
        }
    }

    /// R-26: a main session that dropped, or failed on finish, hands the saved audio to the
    /// fallback as one stream. If the fallback fails too, the main error is the one shown.
    async fn finish(self) -> Result<Transcript, Error> {
        let FallbackSession {
            state,
            saved,
            spare,
        } = self;
        let error = match state {
            State::Fallback(session, via) => {
                return session.finish().await.map(|t| Transcript {
                    via: Some(via),
                    ..t
                });
            }
            State::Main(session) => match session.finish().await {
                Ok(transcript) => return Ok(transcript),
                Err(error) => error,
            },
            State::Dropped(error) => error,
        };
        let Some((fallback, via)) = spare else {
            return Err(error);
        };
        let replay = async {
            let mut session = fallback.open().await?;
            session.send_audio(&saved).await?;
            session.finish().await
        };
        match replay.await {
            Ok(transcript) => Ok(Transcript {
                via: Some(via),
                ..transcript
            }),
            Err(_) => Err(error),
        }
    }
}

/// Counts fallbacks in a row per role, from finished dictation traces, for the notice that the
/// main provider keeps failing (R-28). It lives in the app, not the engine, and never changes a
/// setting.
#[derive(Debug, Default)]
pub struct FallbackStreak {
    speech: u32,
    cleanup: u32,
}

/// How many fallbacks in a row bring the notice.
pub const STREAK: u32 = 3;

impl FallbackStreak {
    /// Feeds one finished dictation or retry. Returns the roles whose fallback has just been
    /// used [`STREAK`] times in a row, once per streak.
    pub fn observe(&mut self, trace: &crate::trace::FinishedTrace) -> Vec<crate::config::Role> {
        use crate::config::Role;
        let root = trace.root();
        if !matches!(root.name("outcome"), Some("pasted" | "copied")) {
            return Vec::new();
        }
        let mut notice = Vec::new();
        let mut count = |n: &mut u32, used: Option<bool>, role| match used {
            Some(true) => {
                *n += 1;
                if *n == STREAK {
                    notice.push(role);
                }
            }
            Some(false) => *n = 0,
            None => {}
        };
        // Every pasted dictation had speech; cleanup only when it ran and did not end in the raw
        // transcript with no fallback, which says nothing about the main provider.
        count(
            &mut self.speech,
            Some(root.name("speech_via").is_some()),
            Role::Speech,
        );
        let cleanup = if root.name("cleanup_via").is_some() {
            Some(true)
        } else if trace.span(crate::trace::CLEANUP).is_some() && root.bool("fallback") != Some(true)
        {
            Some(false)
        } else {
            None
        };
        count(&mut self.cleanup, cleanup, Role::Cleanup);
        notice
    }
}
