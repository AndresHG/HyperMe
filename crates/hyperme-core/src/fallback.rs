//! A fallback provider behind the main one (specs/96-provider-settings, design.md, "Fallback").
//! A wrapper, so the engine stays as it is: it calls one provider, and asks it afterwards
//! whether a fallback answered (R-27).

use std::sync::{Mutex, PoisonError};

use crate::traits::{CleanupProvider, Error, Via};

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
