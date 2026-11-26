//! One client per protocol, chosen at run time (specs/96-provider-settings, design.md, "Enum
//! dispatch"). The traits return `impl Future`, so they cannot be trait objects without boxing
//! every call; an enum over the protocols costs one `match` per call and keeps the engine
//! generic. Adding a protocol adds a variant here; the engine and its tests do not change.
//!
//! [`resolve`] builds the providers a config names. The app uses it at launch, after each
//! change in Settings, and for Test, so Test and dictation cannot disagree.

use hyperme_core::config::{Config, Protocol};
use hyperme_core::dictation::Providers;
use hyperme_core::secrets::SecretStore;
use hyperme_core::traits::{
    CleanupProvider, Error, ErrorKind, LiveSession, SpeechProvider, Transcript,
};

use crate::cleanup::ChatCleanup;
use crate::speechmatics::{Speechmatics, SpeechmaticsSession};

/// A speech provider of any protocol the app has.
#[derive(Debug)]
pub enum AnySpeech {
    Speechmatics(Speechmatics),
}

/// A live session of any speech protocol.
#[derive(Debug)]
pub enum AnySession {
    Speechmatics(SpeechmaticsSession),
}

/// A cleanup provider of any protocol the app has. Every cleanup host today speaks the
/// OpenAI-compatible chat protocol.
#[derive(Debug)]
pub enum AnyCleanup {
    Chat(ChatCleanup),
}

impl SpeechProvider for AnySpeech {
    type Session = AnySession;

    async fn open(&self) -> Result<AnySession, Error> {
        match self {
            AnySpeech::Speechmatics(p) => p.open().await.map(AnySession::Speechmatics),
        }
    }
}

impl LiveSession for AnySession {
    async fn send_audio(&mut self, pcm: &[i16]) -> Result<(), Error> {
        match self {
            AnySession::Speechmatics(s) => s.send_audio(pcm).await,
        }
    }

    async fn finish(self) -> Result<Transcript, Error> {
        match self {
            AnySession::Speechmatics(s) => s.finish().await,
        }
    }
}

impl CleanupProvider for AnyCleanup {
    async fn warm(&self) -> Result<(), Error> {
        match self {
            AnyCleanup::Chat(p) => p.warm().await,
        }
    }

    async fn clean(
        &self,
        raw: &str,
        app: Option<&str>,
        on_text: &mut (dyn FnMut(&str) + Send),
    ) -> Result<(), Error> {
        match self {
            AnyCleanup::Chat(p) => p.clean(raw, app, on_text).await,
        }
    }
}

/// The key a provider needs, or `None` for one the catalogue marks as needing none (R-14).
/// A missing key that is needed fails, naming the provider.
fn key_for(config: &Config, provider: &str, keys: &dyn SecretStore) -> Result<String, Error> {
    let needed = config.provider(provider).is_none_or(|p| p.key_required);
    match keys.get(provider) {
        Ok(Some(key)) => Ok(key),
        Ok(None) if !needed => Ok(String::new()),
        Ok(None) => Err(Error {
            kind: ErrorKind::Auth,
            message: format!("No {provider} key yet"),
        }),
        Err(e) => Err(Error {
            kind: ErrorKind::Device,
            message: e.to_string(),
        }),
    }
}

/// Whether `provider` can be used now: it has a key, or its catalogue entry says it needs none
/// (R-12, R-14). Settings switches a role only when this holds, so dictation never loses a
/// working provider to one without a key.
pub fn key_ready(config: &Config, provider: &str, keys: &dyn SecretStore) -> bool {
    key_for(config, provider, keys).is_ok()
}

/// The speech provider the config names, with its key.
pub fn speech(config: &Config, keys: &dyn SecretStore) -> Result<AnySpeech, Error> {
    let s = &config.speech;
    let key = key_for(config, &s.provider, keys)?;
    match protocol(config, &s.provider)? {
        Protocol::SpeechmaticsRt => Ok(AnySpeech::Speechmatics(Speechmatics::new(s, key))),
        other => Err(wrong_role(&s.provider, other, "speech")),
    }
}

/// The cleanup provider the config names, with its key.
pub fn cleanup(config: &Config, keys: &dyn SecretStore) -> Result<AnyCleanup, Error> {
    let c = &config.cleanup;
    let key = key_for(config, &c.provider, keys)?;
    match protocol(config, &c.provider)? {
        Protocol::OpenaiChat => ChatCleanup::new(c, &key).map(AnyCleanup::Chat),
        other => Err(wrong_role(&c.provider, other, "cleanup")),
    }
}

/// Both providers the config names. Fails without building either if one cannot be built, so
/// the app keeps the providers it has (R-12).
pub fn resolve(
    config: &Config,
    keys: &dyn SecretStore,
) -> Result<Providers<AnySpeech, AnyCleanup>, Error> {
    Ok(Providers {
        speech: speech(config, keys)?,
        cleanup: cleanup(config, keys)?,
    })
}

fn protocol(config: &Config, provider: &str) -> Result<Protocol, Error> {
    // A parsed config always names providers in its catalogue; this guards hand-built ones.
    config
        .provider(provider)
        .map(|p| p.protocol)
        .ok_or_else(|| Error {
            kind: ErrorKind::Other,
            message: format!("{provider} is not in the provider catalogue"),
        })
}

fn wrong_role(provider: &str, protocol: Protocol, role: &str) -> Error {
    Error {
        kind: ErrorKind::Other,
        message: format!(
            "{provider} speaks {}, which cannot do {role}",
            protocol.name()
        ),
    }
}
