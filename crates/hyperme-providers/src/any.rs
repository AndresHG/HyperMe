//! One client per protocol, chosen at run time (specs/96-provider-settings, design.md, "Enum
//! dispatch"). The traits return `impl Future`, so they cannot be trait objects without boxing
//! every call; an enum over the protocols costs one `match` per call and keeps the engine
//! generic. Adding a protocol adds a variant here; the engine and its tests do not change.
//!
//! [`resolve`] builds the providers a config names. The app uses it at launch, after each
//! change in Settings, and for Test, so Test and dictation cannot disagree.

use std::sync::Arc;

use hyperme_core::config::{self, Config, Protocol, Role};
use hyperme_core::dictation::Providers;
use hyperme_core::fallback::Fallback;
use hyperme_core::secrets::SecretStore;
use hyperme_core::traits::{
    CleanupProvider, Error, ErrorKind, LiveSession, SpeechProvider, Transcript, Via,
};

use crate::cleanup::ChatCleanup;
use crate::openai_rt::{OpenAiRealtime, OpenAiSession};
use crate::speechmatics::{Speechmatics, SpeechmaticsSession};

/// A speech provider of any protocol the app has.
#[derive(Debug)]
pub enum AnySpeech {
    Speechmatics(Speechmatics),
    OpenAi(OpenAiRealtime),
}

/// A live session of any speech protocol.
#[derive(Debug)]
pub enum AnySession {
    Speechmatics(SpeechmaticsSession),
    OpenAi(OpenAiSession),
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
            AnySpeech::OpenAi(p) => p.open().await.map(AnySession::OpenAi),
        }
    }
}

impl LiveSession for AnySession {
    async fn send_audio(&mut self, pcm: &[i16]) -> Result<(), Error> {
        match self {
            AnySession::Speechmatics(s) => s.send_audio(pcm).await,
            AnySession::OpenAi(s) => s.send_audio(pcm).await,
        }
    }

    async fn finish(self) -> Result<Transcript, Error> {
        match self {
            AnySession::Speechmatics(s) => s.finish().await,
            AnySession::OpenAi(s) => s.finish().await,
        }
    }
}

impl CleanupProvider for AnyCleanup {
    fn set_dictionary(&self, line: &str) {
        match self {
            AnyCleanup::Chat(p) => p.set_dictionary(line),
        }
    }

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

impl AnyCleanup {
    /// Settings' Test: the key and the chosen model together (R-13).
    pub async fn check(&self) -> Result<(), Error> {
        match self {
            AnyCleanup::Chat(p) => p.check().await,
        }
    }

    /// Adds the user's dictionary to the prompt (spec 93, R-4): `line` is
    /// `Dictionary::prompt_line()`.
    pub fn with_dictionary(self, line: &str) -> Self {
        self.set_dictionary(line);
        self
    }
}

/// The speech the engine uses: the role's provider, with its fallback when one is set and
/// ready (R-25, R-26, R-24a). Shared, so a session can open the fallback on release.
pub type Speech = Fallback<Arc<AnySpeech>>;

/// The cleanup the engine uses: the role's provider, with its fallback when one is set and
/// ready (R-24, R-24a).
pub type Cleanup = Fallback<AnyCleanup>;

/// The key a provider needs, or empty for one the catalogue marks as needing none (R-14). A
/// provider that shares another's key reads that one (D-2). A missing key that is needed
/// fails, naming the keychain entry.
fn key_for(config: &Config, provider: &str, keys: &dyn SecretStore) -> Result<String, Error> {
    let needed = config.provider(provider).is_none_or(|p| p.key_required);
    let provider = config.key_account(provider);
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
    speech_for(config, &config.speech, keys)
}

/// The speech fallback the config names, alone, for its Test (R-24a). Fails when none is set.
pub fn speech_fallback(config: &Config, keys: &dyn SecretStore) -> Result<AnySpeech, Error> {
    let s = config.speech_fallback().ok_or_else(|| Error {
        kind: ErrorKind::Other,
        message: "no speech fallback is set".to_owned(),
    })?;
    speech_for(config, &s, keys)
}

/// The role's speech with its fallback. A fallback that cannot be built, its key missing for
/// example, is skipped (R-24a).
pub fn speech_with_fallback(config: &Config, keys: &dyn SecretStore) -> Result<Speech, Error> {
    let main = Arc::new(speech(config, keys)?);
    let fallback = config.speech_fallback().and_then(|s| {
        let built = speech_for(config, &s, keys).ok()?;
        Some((Arc::new(built), via(config, s.provider, s.model)))
    });
    Ok(Fallback::new(main, fallback))
}

fn via(config: &Config, provider: String, model: String) -> Via {
    let name = config
        .provider(&provider)
        .map_or_else(|| provider.clone(), |p| p.name.clone());
    Via {
        provider,
        model,
        name,
    }
}

fn speech_for(
    config: &Config,
    s: &config::Speech,
    keys: &dyn SecretStore,
) -> Result<AnySpeech, Error> {
    let key = key_for(config, &s.provider, keys)?;
    match protocol(config, &s.provider)? {
        Protocol::SpeechmaticsRt => Ok(AnySpeech::Speechmatics(Speechmatics::new(s, key))),
        Protocol::OpenaiRealtime => Ok(AnySpeech::OpenAi(OpenAiRealtime::new(s, key))),
        other => Err(wrong_role(&s.provider, other, "speech")),
    }
}

/// The cleanup provider the config names, with its key.
pub fn cleanup(config: &Config, keys: &dyn SecretStore) -> Result<AnyCleanup, Error> {
    cleanup_for(config, &config.cleanup, keys)
}

/// The cleanup fallback the config names, alone, for its Test (R-24a). Fails when none is set.
pub fn cleanup_fallback(config: &Config, keys: &dyn SecretStore) -> Result<AnyCleanup, Error> {
    let c = config.cleanup_fallback().ok_or_else(|| Error {
        kind: ErrorKind::Other,
        message: "no cleanup fallback is set".to_owned(),
    })?;
    cleanup_for(config, &c, keys)
}

/// Whether a role's fallback is set, and if so whether it is ready (R-24a): `None` when none
/// is set; `Some(false)` when its key is missing, so the Setup check turns its row red.
pub fn fallback_ready(config: &Config, role: Role, keys: &dyn SecretStore) -> Option<bool> {
    let provider = match role {
        Role::Cleanup => &config.cleanup.fallback.as_ref()?.provider,
        Role::Speech => &config.speech.fallback.as_ref()?.provider,
    };
    Some(key_ready(config, provider, keys))
}

/// The role's cleanup with its fallback. A fallback that cannot be built, its key missing for
/// example, is skipped: it never blocks or slows a dictation (R-24a).
pub fn cleanup_with_fallback(config: &Config, keys: &dyn SecretStore) -> Result<Cleanup, Error> {
    let main = cleanup(config, keys)?;
    let fallback = config.cleanup_fallback().and_then(|c| {
        let built = cleanup_for(config, &c, keys).ok()?;
        Some((built, via(config, c.provider, c.model)))
    });
    Ok(Fallback::new(main, fallback))
}

fn cleanup_for(
    config: &Config,
    c: &config::Cleanup,
    keys: &dyn SecretStore,
) -> Result<AnyCleanup, Error> {
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
) -> Result<Providers<Speech, Cleanup>, Error> {
    Ok(Providers {
        speech: speech_with_fallback(config, keys)?,
        cleanup: cleanup_with_fallback(config, keys)?,
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
