//! Speech-to-text behind one trait, so providers can be compared and swapped.

pub mod openai;

use anyhow::Result;

pub struct Transcript {
    pub text: String,
    /// Language codes the provider says it detected. Empty when it reports none.
    pub languages: Vec<String>,
}

/// A speech provider. `label` names the dictation in errors and logs; it never tells a provider
/// which language to expect.
pub trait SpeechProvider {
    type Live: LiveSession;

    /// Pipeline A: transcribe a complete WAV recording, uploaded after key release.
    async fn transcribe_file(&self, label: &str, wav: Vec<u8>) -> Result<Transcript>;

    /// Pipeline B: open and configure a streaming session before the audio starts.
    async fn open_live(&self, label: &str) -> Result<Self::Live>;
}

/// One dictation being streamed.
pub trait LiveSession {
    /// Sends one chunk of 24 kHz mono PCM16.
    async fn send_audio(&mut self, pcm: &[i16]) -> Result<()>;

    /// Key release: tells the provider the speech has ended, with its explicit commit or
    /// finalize message, and waits for the final transcript.
    async fn finish(self) -> Result<Transcript>;
}
