//! The config file: `config.toml` in the app's data folder (design.md, "Config").
//!
//! Provider and model names and endpoints come from this file, never from code (R-26); their
//! defaults live in `default-config.toml`, which is written on first launch. What each provider
//! is (protocol, endpoint, key page, models and prices) lives in the catalogue,
//! `default-providers.toml`, built into the app; the user's file may add or replace entries
//! (specs/96-provider-settings, R-1 to R-5). The file holds no
//! secrets: a key-like field is an error. Unknown keys are an error naming the file and line,
//! and out-of-range values name the key.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use time::{Time, Weekday};

use crate::connector_settings::AccountEntry;
use crate::registry::{CustomServer, ToolLists};
use crate::schedule::Schedule;
use crate::tasks::{DayRules, WeekdaySet};

/// A field's value, or the default file's when the user's file leaves it out.
macro_rules! or_default {
    ($self:ident, $section:ident, $field:ident) => {
        $self
            .$field
            .clone()
            .or_else(|| defaults().$section.$field.clone())
            .unwrap_or_default()
    };
}

mod edit;
mod providers;
mod sections;
mod validate;

pub use edit::*;
pub use providers::*;
pub use sections::*;
pub(crate) use validate::looks_like_secret;
use validate::{find_secret_line, reject_secrets, validate_provider};

/// The file written when there is none, and the source of every default.
pub const DEFAULT_CONFIG: &str = include_str!("default-config.toml");

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub dictation: Dictation,
    pub speech: Speech,
    pub cleanup: Cleanup,
    pub history: History,
    pub timing: Timing,
    /// The task list and the morning card. Optional: an older file takes the defaults.
    #[serde(default = "default_tasks")]
    pub tasks: Tasks,
    /// The provider catalogue: the built-in one with the user's entries over it (R-1, R-5).
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderEntry>,
    /// The user's own MCP servers (specs/97-google-connectors, R-4). Optional: an older file
    /// has none.
    #[serde(default)]
    pub connectors: Connectors,
}

/// `[connectors]`: custom MCP servers, connected accounts and the user's tool switches.
/// Presets are data files, not config. Settings writes this part of the file whole
/// (`connector_settings::write_connectors`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Connectors {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom: Vec<CustomServer>,
    /// Signed-in accounts, by connector and label (R-11). Their tokens are in the keychain.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accounts: Vec<AccountEntry>,
    /// The user's tool switches per connector id, exact tool names (R-5).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tools: BTreeMap<String, ToolLists>,
}

/// The default file, for keys an older file leaves out.
fn defaults() -> &'static Config {
    static DEFAULTS: std::sync::OnceLock<Config> = std::sync::OnceLock::new();
    DEFAULTS.get_or_init(Config::default)
}

/// Why the config could not be loaded. Every variant names the file.
#[derive(Debug)]
pub enum ConfigError {
    /// The file could not be read or written.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// Not valid TOML, an unknown or missing key, or a wrong type. The message has the line.
    Parse { path: PathBuf, message: String },
    /// A value out of range or not allowed.
    Invalid {
        path: PathBuf,
        key: String,
        message: String,
    },
    /// A key-like field: the config file holds no secrets (R-26). Never carries the value.
    Secret {
        path: PathBuf,
        key: String,
        line: Option<usize>,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io { path, source } => write!(f, "{}: {source}", path.display()),
            ConfigError::Parse { path, message } => write!(f, "{}: {message}", path.display()),
            ConfigError::Invalid { path, key, message } => {
                write!(f, "{}: {key} {message}", path.display())
            }
            ConfigError::Secret { path, key, line } => {
                write!(f, "{}", path.display())?;
                if let Some(line) = line {
                    write!(f, ", line {line}")?;
                }
                write!(
                    f,
                    ": {key} looks like a key. The config file holds no secrets: remove it and \
                     enter the key in Settings, which keeps it in the macOS keychain"
                )
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::parse(DEFAULT_CONFIG, Path::new("default-config.toml"))
            .expect("the built-in default config is valid")
    }
}

impl Config {
    /// Reads the config at `path`, or writes the default file there (creating its folder) and
    /// returns the defaults when there is no file.
    pub fn load_or_create(path: &Path) -> Result<Config, ConfigError> {
        let io = |source| ConfigError::Io {
            path: path.to_owned(),
            source,
        };
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text, path),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir).map_err(io)?;
                }
                // create_new: never overwrite a file that appeared in the meantime.
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .map_err(io)?;
                file.write_all(DEFAULT_CONFIG.as_bytes()).map_err(io)?;
                Ok(Config::default())
            }
            Err(err) => Err(io(err)),
        }
    }

    /// Parses and validates config text. `path` is only used in error messages.
    pub fn parse(text: &str, path: &Path) -> Result<Config, ConfigError> {
        let parse_error = |err: toml::de::Error| ConfigError::Parse {
            path: path.to_owned(),
            message: err.to_string().trim_end().to_owned(),
        };
        // Secrets first, so a key-like field gets its own message rather than "unknown field",
        // and line by line before TOML parsing, so a syntax error never echoes a key's line.
        if let Some((line, key)) = find_secret_line(text) {
            return Err(ConfigError::Secret {
                path: path.to_owned(),
                key,
                line: Some(line),
            });
        }
        let table: toml::Table = toml::from_str(text).map_err(parse_error)?;
        reject_secrets(&table, "", text, path)?;
        let mut config: Config = toml::from_str(text).map_err(parse_error)?;
        // P2-2: `prompt = "v4"` with no level is the old default line, not a choice.
        let cleanup = table.get("cleanup").and_then(toml::Value::as_table);
        if cleanup.is_some_and(|c| !c.contains_key("level"))
            && config.cleanup.prompt.as_deref() == Some("v4")
        {
            config.cleanup.prompt = None;
        }
        for (id, entry) in &config.providers {
            validate_provider(id, entry, path)?;
        }
        let mine = std::mem::take(&mut config.providers);
        config.providers = built_in_providers().clone();
        config.providers.extend(mine);
        config.validate(path)?;
        config.resolve_from_catalogue();
        Ok(config)
    }

    /// The cleanup section for the fallback, when one is set (R-24): the main one's prompt,
    /// level and deadline, with the fallback's provider, model and endpoint, and the fastest
    /// reasoning effort the catalogue lists for the model ("off" when it lists none).
    pub fn cleanup_fallback(&self) -> Option<Cleanup> {
        let f = self.cleanup.fallback.as_ref()?;
        let entry = self.provider(&f.provider)?;
        let model = self.catalogue_model(&f.provider, &f.model);
        Some(Cleanup {
            provider: f.provider.clone(),
            model: f.model.clone(),
            endpoint: entry.endpoint.clone(),
            reasoning_effort: model
                .reasoning_effort
                .first()
                .map_or_else(|| "off".to_owned(), Clone::clone),
            key_url: entry.key_url.clone(),
            price_input_per_million_usd: None,
            price_output_per_million_usd: None,
            fallback: None,
            ..self.cleanup.clone()
        })
    }

    /// The speech section for the fallback, when one is set (R-25): its provider, model and
    /// catalogue endpoint.
    pub fn speech_fallback(&self) -> Option<Speech> {
        let f = self.speech.fallback.as_ref()?;
        let entry = self.provider(&f.provider)?;
        Some(Speech {
            provider: f.provider.clone(),
            model: f.model.clone(),
            endpoint: entry.endpoint.clone(),
            key_url: entry.key_url.clone(),
            price_per_hour_usd: None,
            free_credit_usd: entry.free_credit_usd,
            fallback: None,
        })
    }

    /// Every provider dictation can send to, fallbacks included, in the order the Privacy
    /// section lists them (R-29).
    pub fn destinations(&self) -> Vec<Destination> {
        let name = |id: &str| {
            self.provider(id)
                .map_or_else(|| id.to_owned(), |p| p.name.clone())
        };
        let mut out = Vec::new();
        let roles = [
            (
                Role::Speech,
                &self.speech.provider,
                &self.speech.model,
                &self.speech.endpoint,
                self.speech_fallback()
                    .map(|s| (s.provider, s.model, s.endpoint)),
            ),
            (
                Role::Cleanup,
                &self.cleanup.provider,
                &self.cleanup.model,
                &self.cleanup.endpoint,
                self.cleanup_fallback()
                    .map(|c| (c.provider, c.model, c.endpoint)),
            ),
        ];
        for (role, provider, model, endpoint, fallback) in roles {
            out.push(Destination {
                role,
                name: name(provider),
                model: model.clone(),
                fallback_for: None,
                on_this_mac: on_this_mac(endpoint),
            });
            if let Some((f_provider, f_model, f_endpoint)) = fallback {
                out.push(Destination {
                    role,
                    name: name(&f_provider),
                    model: f_model,
                    fallback_for: Some(name(provider)),
                    on_this_mac: on_this_mac(&f_endpoint),
                });
            }
        }
        out
    }

    /// A provider in the catalogue.
    pub fn provider(&self, id: &str) -> Option<&ProviderEntry> {
        self.providers.get(id)
    }

    /// The keychain entry that holds `provider`'s key: its own, or the one it shares (D-2).
    pub fn key_account<'a>(&'a self, provider: &'a str) -> &'a str {
        self.provider(provider)
            .and_then(|p| p.key_from.as_deref())
            .unwrap_or(provider)
    }

    /// The providers that can fill `role`, in menu order: the recommended one first, then by
    /// each entry's `order`, then by name (R-7).
    pub fn providers_for(&self, role: Role) -> impl Iterator<Item = (&str, &ProviderEntry)> {
        let recommended = Config::recommended(role).0;
        let mut fitting: Vec<(&str, &ProviderEntry)> = self
            .providers
            .iter()
            .filter(|(_, entry)| entry.roles.contains(&role))
            .map(|(id, entry)| (id.as_str(), entry))
            .collect();
        fitting.sort_by_key(|(id, entry)| (*id != recommended, entry.order, *id));
        fitting.into_iter()
    }

    /// The recommended provider and model for `role`: the default file's (R-6, R-10).
    pub fn recommended(role: Role) -> (&'static str, &'static str) {
        let d = defaults();
        match role {
            Role::Speech => (&d.speech.provider, &d.speech.model),
            Role::Cleanup => (&d.cleanup.provider, &d.cleanup.model),
        }
    }

    /// The speech model in use: the catalogue's entry, or a bare one for a model typed by hand,
    /// with an older file's price over it (R-4).
    pub fn speech_model(&self) -> ModelEntry {
        let mut model = self.catalogue_model(&self.speech.provider, &self.speech.model);
        if let Some(price) = self.speech.price_per_hour_usd {
            model.price_per_hour_usd = Some(price);
        }
        model
    }

    /// The cleanup model in use, like [`Config::speech_model`].
    pub fn cleanup_model(&self) -> ModelEntry {
        let c = &self.cleanup;
        let mut model = self.catalogue_model(&c.provider, &c.model);
        if let Some(price) = c.price_input_per_million_usd {
            model.price_input_per_million_usd = Some(price);
        }
        if let Some(price) = c.price_output_per_million_usd {
            model.price_output_per_million_usd = Some(price);
        }
        model
    }

    fn catalogue_model(&self, provider: &str, model: &str) -> ModelEntry {
        self.provider(provider)
            .and_then(|entry| entry.models.iter().find(|m| m.id == model))
            .cloned()
            .unwrap_or_else(|| ModelEntry::named(model))
    }

    /// Fills each role's endpoint, key page and credit from its catalogue entry where the file
    /// does not set them (R-4). Runs after validation, so every role's provider exists.
    fn resolve_from_catalogue(&mut self) {
        if let Some(entry) = self.providers.get(&self.speech.provider) {
            let s = &mut self.speech;
            if s.endpoint.is_empty() {
                s.endpoint.clone_from(&entry.endpoint);
            }
            if s.key_url.is_none() {
                s.key_url.clone_from(&entry.key_url);
            }
            if s.free_credit_usd.is_none() {
                s.free_credit_usd = entry.free_credit_usd;
            }
        }
        if let Some(entry) = self.providers.get(&self.cleanup.provider) {
            let c = &mut self.cleanup;
            if c.endpoint.is_empty() {
                c.endpoint.clone_from(&entry.endpoint);
            }
            if c.key_url.is_none() {
                c.key_url.clone_from(&entry.key_url);
            }
        }
    }
}
