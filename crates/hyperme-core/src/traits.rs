//! The traits that keep platform and network code out of the core (design.md, "Traits").
//! Real implementations live in `hyperme-providers` and `hyperme-platform`; tests use fakes.
//!
//! Async methods return `impl Future + Send`, so the state machine's own future is `Send` and
//! the app can spawn it. Implementations may write them as `async fn`.

use std::fmt;
use std::future::Future;

/// What went wrong in a provider or a device. The message never contains a key (R-25).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// The provider rejected the key (R-21).
    Auth,
    /// Could not connect, or the connection dropped (R-19).
    Network,
    /// A local device or OS service failed: microphone, clipboard.
    Device,
    /// Anything else the provider answered.
    Other,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

/// The final transcript of one dictation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcript {
    pub text: String,
    /// The language the provider detected, when it says (R-10).
    pub language: Option<String>,
}

/// A streaming speech-to-text provider. It is never told which language to expect (R-10).
pub trait SpeechProvider {
    type Session: LiveSession + Send;

    /// Opens and configures a streaming session, on key-down.
    fn open(&self) -> impl Future<Output = Result<Self::Session, Error>> + Send;
}

/// One dictation being streamed. Dropping it closes the session.
pub trait LiveSession {
    /// Sends one chunk of mono PCM16 at the provider's rate.
    fn send_audio(&mut self, pcm: &[i16]) -> impl Future<Output = Result<(), Error>> + Send;

    /// Key release: sends the provider's explicit finalize message and waits for the final
    /// transcript (R-9).
    fn finish(self) -> impl Future<Output = Result<Transcript, Error>> + Send;
}

/// A language model that rewrites a raw transcript into clean text.
pub trait CleanupProvider {
    /// Opens the connection ahead of time, on key-down (R-2).
    fn warm(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Streams the cleaned text of `raw`, calling `on_text` with each piece. `app` is the
    /// frontmost app's name (R-11).
    fn clean(
        &self,
        raw: &str,
        app: Option<&str>,
        on_text: &mut (dyn FnMut(&str) + Send),
    ) -> impl Future<Output = Result<(), Error>> + Send;
}

/// Microphone capture. While started, the platform glue sends each captured frame to the
/// state machine as `Event::Audio`.
pub trait Microphone {
    fn start(&mut self) -> Result<(), Error>;
    fn stop(&mut self);
}

/// Puts text at the cursor of the frontmost app (R-15).
pub trait Inserter {
    fn paste(&mut self, text: &str) -> impl Future<Output = Result<(), Error>> + Send;
}

/// What the user is typing into.
pub trait FocusInfo {
    /// The frontmost app's name, for the cleanup prompt (R-11).
    fn frontmost_app(&self) -> Option<String>;

    /// Whether secure input is on, as it is while a password field has focus (R-17).
    fn secure_input(&self) -> bool;
}
