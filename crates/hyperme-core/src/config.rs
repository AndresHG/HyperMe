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

/// The file written when there is none, and the source of every default.
pub const DEFAULT_CONFIG: &str = include_str!("default-config.toml");

/// The built-in provider catalogue (R-1). Entries in the user's file add to it or replace an
/// entry of the same name.
pub const DEFAULT_PROVIDERS: &str = include_str!("default-providers.toml");

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

/// How the app talks to a provider. The code knows protocols, not providers (R-2, D-7): adding
/// a provider that speaks one of these is a catalogue entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Protocol {
    /// Speechmatics' real-time WebSocket API.
    SpeechmaticsRt,
    /// An OpenAI-compatible `/chat/completions` API.
    OpenaiChat,
}

impl Protocol {
    pub const ALL: [Protocol; 2] = [Protocol::SpeechmaticsRt, Protocol::OpenaiChat];

    /// The name written in the catalogue.
    pub fn name(self) -> &'static str {
        match self {
            Protocol::SpeechmaticsRt => "speechmatics-rt",
            Protocol::OpenaiChat => "openai-chat",
        }
    }

    /// The roles a client of this protocol can fill.
    pub fn roles(self) -> &'static [Role] {
        match self {
            Protocol::SpeechmaticsRt => &[Role::Speech],
            Protocol::OpenaiChat => &[Role::Cleanup],
        }
    }

    /// The URL schemes its endpoint may use.
    fn schemes(self) -> &'static [&'static str] {
        match self {
            Protocol::SpeechmaticsRt => &["wss", "ws"],
            Protocol::OpenaiChat => &["https", "http"],
        }
    }
}

/// One job a model does (spec.md, "Requirements"). Answers and meeting cleanup join later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Speech,
    Cleanup,
}

impl Role {
    /// The config section that names this role's provider.
    pub fn section(self) -> &'static str {
        match self {
            Role::Speech => "speech",
            Role::Cleanup => "cleanup",
        }
    }
}

fn yes() -> bool {
    true
}

fn later_in_menus() -> u32 {
    500
}

/// Whether `endpoint` is a server on this Mac: its host is `127.0.0.1`, `localhost` or `[::1]`
/// (R-23).
pub fn on_this_mac(endpoint: &str) -> bool {
    let rest = endpoint.split_once("://").map_or(endpoint, |(_, r)| r);
    let authority = rest.split('/').next().unwrap_or("");
    let host = if authority.starts_with('[') {
        authority.split(']').next().map(|h| &h[1..]).unwrap_or("")
    } else {
        authority.split(':').next().unwrap_or("")
    };
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

/// What a provider is (R-1).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderEntry {
    /// As people write it: "Speechmatics", "Local server".
    pub name: String,
    pub protocol: Protocol,
    pub endpoint: String,
    /// Where "Get a key" goes.
    #[serde(default)]
    pub key_url: Option<String>,
    /// One line shown beside the key field while there is no key, such as a plan whose credits
    /// pay for it.
    #[serde(default)]
    pub key_note: Option<String>,
    /// The roles it is offered for; each must be one its protocol can fill.
    pub roles: Vec<Role>,
    /// False for a server that needs no key, such as one on this Mac (R-14).
    #[serde(default = "yes")]
    pub key_required: bool,
    /// The key can list the models it offers (R-15).
    #[serde(default)]
    pub list_models: bool,
    /// Listed models whose names contain any of these are left out of the menu: embeddings,
    /// speech and image models the role cannot use (R-15, design.md "Model listing").
    #[serde(default)]
    pub hide_models: Vec<String>,
    /// Credit new accounts get, for onboarding's line about it.
    #[serde(default)]
    pub free_credit_usd: Option<f64>,
    /// Place in a role's provider menu, lowest first, after the recommended one (R-7). Entries
    /// without one, such as a user's own, come after the built-in ones.
    #[serde(default = "later_in_menus")]
    pub order: u32,
    /// The models the catalogue knows, recommended first.
    #[serde(default)]
    pub models: Vec<ModelEntry>,
    /// Ports on this Mac where a server like it usually listens, tried in order when the user
    /// picks it (R-22).
    #[serde(default)]
    pub ports: Vec<u16>,
}

/// One model of a provider: its prices and the latency this project measured (R-1, R-19).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelEntry {
    pub id: String,
    /// Speech.
    #[serde(default)]
    pub price_per_hour_usd: Option<f64>,
    /// Cleanup.
    #[serde(default)]
    pub price_input_per_million_usd: Option<f64>,
    #[serde(default)]
    pub price_output_per_million_usd: Option<f64>,
    /// The day the prices were checked against the provider's page, "YYYY-MM-DD".
    #[serde(default)]
    pub prices_checked: Option<String>,
    /// The `reasoning_effort` values the model takes; empty means the field is never sent.
    #[serde(default)]
    pub reasoning_effort: Vec<String>,
    #[serde(default)]
    pub measured: Option<Measured>,
}

impl ModelEntry {
    /// A model the catalogue does not list, typed by hand (R-16): no prices, no measurement.
    pub fn named(id: &str) -> Self {
        ModelEntry {
            id: id.to_owned(),
            ..ModelEntry::default()
        }
    }
}

/// A latency this project measured, with the result it comes from (R-18, R-19).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measured {
    /// Median from key release to text.
    pub release_to_text_ms: u64,
    #[serde(default)]
    pub p90_ms: Option<u64>,
    /// The file under `docs/results/`.
    pub source: String,
}

/// The built-in catalogue file's shape.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalogue {
    providers: BTreeMap<String, ProviderEntry>,
}

/// The built-in catalogue, parsed and checked once.
fn built_in_providers() -> &'static BTreeMap<String, ProviderEntry> {
    static PROVIDERS: std::sync::OnceLock<BTreeMap<String, ProviderEntry>> =
        std::sync::OnceLock::new();
    PROVIDERS.get_or_init(|| {
        let path = Path::new("default-providers.toml");
        let catalogue: Catalogue =
            toml::from_str(DEFAULT_PROVIDERS).expect("the built-in catalogue parses");
        for (id, entry) in &catalogue.providers {
            validate_provider(id, entry, path).expect("the built-in catalogue is valid");
        }
        catalogue.providers
    })
}

fn show_while_idle() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dictation {
    /// R-8.
    pub hotkey: Hotkey,
    /// R-5: a shorter hold pastes nothing.
    pub min_hold_ms: u64,
    /// R-7.
    pub max_length_s: u64,
    /// #167: a key-down this soon after a tap locks recording (hands-free). Optional, so an
    /// older file still loads.
    #[serde(default = "default_double_tap_ms")]
    pub double_tap_ms: u64,
    /// R-13: fewer words than this skip cleanup.
    pub short_dictation_words: u32,
    /// R-32.
    pub sounds: bool,
    /// The bead at the bottom centre while idle (U-13, brought forward by the owner). Optional,
    /// so a config written before it existed still loads, with the bead on.
    #[serde(default = "show_while_idle")]
    pub show_while_idle: bool,
    /// Minutes of talking a day, for the cost estimates (U-11). Onboarding saves the user's
    /// choice here. Optional: an older file takes the default file's value.
    #[serde(default = "default_talking_minutes")]
    pub talking_minutes_per_day: u32,
    /// The user's terms, one per line.
    pub dictionary: Option<PathBuf>,
    /// Spec 93, R-5: up to this many entries the cleanup prompt carries them all; above it, only
    /// the ones relevant to the dictation. Optional: an older file takes the default.
    #[serde(default = "default_prompt_cap")]
    pub dictionary_prompt_cap: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Speech {
    /// A provider in the catalogue whose protocol can fill the speech role.
    pub provider: String,
    pub model: String,
    /// The provider's endpoint, from the catalogue. A file written before the catalogue sets it
    /// here, and then it overrides the catalogue's (R-4).
    #[serde(default)]
    pub endpoint: String,
    /// Onboarding's "Get a key" page, from the catalogue unless an older file sets it.
    #[serde(default)]
    key_url: Option<String>,
    /// Older files only: overrides the catalogue model's price (R-4).
    #[serde(default)]
    price_per_hour_usd: Option<f64>,
    /// From the catalogue unless an older file sets it.
    #[serde(default)]
    free_credit_usd: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cleanup {
    /// A provider in the catalogue whose protocol can fill the cleanup role. Labels the keychain
    /// entry and error messages (R-21).
    pub provider: String,
    pub model: String,
    /// Base URL of an OpenAI-compatible chat completions API, from the catalogue. A file written
    /// before the catalogue sets it here, and then it overrides the catalogue's (R-4).
    #[serde(default)]
    pub endpoint: String,
    /// Sent as `reasoning_effort`; `off` leaves the field out.
    pub reasoning_effort: String,
    /// How much the model may change the transcript (P2-2). Optional: an older file takes the
    /// default file's level.
    #[serde(default = "default_cleanup_level")]
    pub level: CleanupLevel,
    /// A named prompt version that replaces the level's prompt, for experiments. A file written
    /// before levels has `prompt = "v4"`, the old default, with no level; that is read as no
    /// override, so it gets the default level.
    #[serde(default)]
    pub prompt: Option<String>,
    /// R-12: after this, the raw transcript is pasted.
    pub deadline_ms: u64,
    /// Onboarding's "Get a key" page, from the catalogue unless an older file sets it.
    #[serde(default)]
    key_url: Option<String>,
    /// Older files only: override the catalogue model's prices (R-4).
    #[serde(default)]
    price_input_per_million_usd: Option<f64>,
    #[serde(default)]
    price_output_per_million_usd: Option<f64>,
    /// What an hour of talking sends to cleanup, for the estimate. Optional: an older file
    /// takes the default file's values, so no estimate is ever written in code.
    #[serde(default)]
    estimate_dictations_per_hour: Option<f64>,
    #[serde(default)]
    estimate_input_tokens: Option<f64>,
    #[serde(default)]
    estimate_output_tokens: Option<f64>,
}

/// The default file, for keys an older file leaves out.
fn defaults() -> &'static Config {
    static DEFAULTS: std::sync::OnceLock<Config> = std::sync::OnceLock::new();
    DEFAULTS.get_or_init(Config::default)
}

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

impl Speech {
    /// The provider's "Get a key" page, or empty when it has none.
    pub fn key_url(&self) -> String {
        self.key_url.clone().unwrap_or_default()
    }
    /// Credit new accounts get, or 0.
    pub fn free_credit_usd(&self) -> f64 {
        self.free_credit_usd.unwrap_or_default()
    }
}

impl Cleanup {
    /// The provider's "Get a key" page, or empty when it has none.
    pub fn key_url(&self) -> String {
        self.key_url.clone().unwrap_or_default()
    }
    pub fn estimate_dictations_per_hour(&self) -> f64 {
        or_default!(self, cleanup, estimate_dictations_per_hour)
    }
    pub fn estimate_input_tokens(&self) -> f64 {
        or_default!(self, cleanup, estimate_input_tokens)
    }
    pub fn estimate_output_tokens(&self) -> f64 {
        or_default!(self, cleanup, estimate_output_tokens)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct History {
    /// R-18.
    pub enabled: bool,
    pub retention_days: Retention,
    pub size_limit_mb: u64,
    /// Spec 182, R-9a (TS-1): keep each dictation's timing trace, numbers only, even with
    /// history off. A file written before traces has no such key: on.
    #[serde(default = "keep_traces_default")]
    pub keep_traces: bool,
}

fn keep_traces_default() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timing {
    /// R-30: one line of stage timings per dictation, numbers only.
    pub log: bool,
}

/// The task list's day, workdays and stale mark, and the morning card's times
/// (specs/92-daily-tasks). Times are "HH:MM"; weekdays are `mon` to `sun`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tasks {
    pub day_start: String,
    pub workdays: Vec<String>,
    pub stale_after: u32,
    pub morning_from: String,
    pub morning_latest: String,
}

fn default_double_tap_ms() -> u64 {
    defaults().dictation.double_tap_ms
}

/// How much the cleanup model may change the transcript (P2-2). None of them rephrase,
/// translate or answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CleanupLevel {
    /// No model call: the transcript after dictionary substitution, as a short dictation.
    None,
    /// Fillers, punctuation, capitals and clear mishearings; every other word kept.
    Light,
    /// Light, plus repetitions, false starts, small grammar slips and spoken formatting.
    Medium,
}

fn default_cleanup_level() -> CleanupLevel {
    defaults().cleanup.level
}

fn default_talking_minutes() -> u32 {
    defaults().dictation.talking_minutes_per_day
}

fn default_prompt_cap() -> u32 {
    defaults().dictation.dictionary_prompt_cap
}

fn default_tasks() -> Tasks {
    defaults().tasks.clone()
}

/// "HH:MM", 00:00 to 23:59.
fn clock_time(text: &str) -> Option<Time> {
    let (h, m) = text.trim().split_once(':')?;
    if h.len() != 2 || m.len() != 2 {
        return None;
    }
    Time::from_hms(h.parse().ok()?, m.parse().ok()?, 0).ok()
}

fn weekday(name: &str) -> Option<Weekday> {
    Some(match name.trim().to_lowercase().as_str() {
        "mon" => Weekday::Monday,
        "tue" => Weekday::Tuesday,
        "wed" => Weekday::Wednesday,
        "thu" => Weekday::Thursday,
        "fri" => Weekday::Friday,
        "sat" => Weekday::Saturday,
        "sun" => Weekday::Sunday,
        _ => return None,
    })
}

impl Tasks {
    /// The day start, workdays and stale threshold. A value the file got wrong (validation
    /// refuses it, so only a hand-built config) falls back to the default.
    pub fn day_rules(&self) -> DayRules {
        let fallback = DayRules::default();
        let mut workdays = WeekdaySet::EMPTY;
        for name in &self.workdays {
            if let Some(day) = weekday(name) {
                workdays.insert(day);
            }
        }
        DayRules {
            day_start: clock_time(&self.day_start).unwrap_or(fallback.day_start),
            workdays: if workdays.is_empty() {
                fallback.workdays
            } else {
                workdays
            },
            stale_after: self.stale_after,
        }
    }

    /// The morning card's schedule (R-15, R-16).
    pub fn morning(&self) -> Schedule {
        let at = |text: &str, h, m| {
            clock_time(text).unwrap_or(Time::from_hms(h, m, 0).expect("a valid time"))
        };
        Schedule::OnPresenceAfter {
            from: at(&self.morning_from, 6, 0),
            latest: at(&self.morning_latest, 9, 30),
            weekdays: self.day_rules().workdays,
        }
    }
}

/// How long history keeps a dictation (R-18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RetentionValue")]
pub enum Retention {
    Days(u32),
    Unlimited,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RetentionValue {
    Days(u32),
    Word(String),
}

impl TryFrom<RetentionValue> for Retention {
    type Error = String;

    fn try_from(value: RetentionValue) -> Result<Self, String> {
        match value {
            RetentionValue::Days(days) => Ok(Retention::Days(days)),
            RetentionValue::Word(word) if word == "unlimited" => Ok(Retention::Unlimited),
            RetentionValue::Word(word) => Err(format!(
                "history.retention_days is {word:?}: write a number of days or \"unlimited\""
            )),
        }
    }
}

/// The dictation hotkey (R-8).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub enum Hotkey {
    /// Hold Fn (Globe), the default.
    Fn,
    RightOption,
    /// One or more modifiers and one other key, such as `ctrl+option+space`. The key is
    /// lowercase.
    Chord {
        modifiers: Vec<Modifier>,
        key: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    Cmd,
    Ctrl,
    Option,
    Shift,
    Fn,
}

impl TryFrom<String> for Hotkey {
    type Error = String;

    fn try_from(value: String) -> Result<Self, String> {
        let invalid = |why: &str| {
            format!(
                "dictation.hotkey is {value:?}: {why}. Write \"fn\", \"right_option\" or a chord \
                 such as \"ctrl+option+space\""
            )
        };
        let lower = value.trim().to_lowercase();
        match lower.as_str() {
            "fn" => return Ok(Hotkey::Fn),
            "right_option" => return Ok(Hotkey::RightOption),
            _ => {}
        }
        let parts: Vec<&str> = lower.split('+').map(str::trim).collect();
        let Some((key, modifiers)) = parts.split_last() else {
            return Err(invalid("it is empty"));
        };
        if modifiers.is_empty() {
            return Err(invalid("a chord needs at least one modifier"));
        }
        let mut seen = Vec::with_capacity(modifiers.len());
        for name in modifiers {
            let modifier = Modifier::named(name).ok_or_else(|| {
                invalid(&format!("{name:?} is not cmd, ctrl, option, shift or fn"))
            })?;
            if seen.contains(&modifier) {
                return Err(invalid(&format!("{name:?} appears twice")));
            }
            seen.push(modifier);
        }
        if key.is_empty() {
            return Err(invalid("the chord has no key after its modifiers"));
        }
        if Modifier::named(key).is_some() {
            return Err(invalid("the chord ends in a modifier, not a key"));
        }
        Ok(Hotkey::Chord {
            modifiers: seen,
            key: (*key).to_owned(),
        })
    }
}

impl Modifier {
    fn named(name: &str) -> Option<Self> {
        Some(match name {
            "cmd" => Modifier::Cmd,
            "ctrl" => Modifier::Ctrl,
            "option" => Modifier::Option,
            "shift" => Modifier::Shift,
            "fn" => Modifier::Fn,
            _ => return None,
        })
    }
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

    /// A provider in the catalogue.
    pub fn provider(&self, id: &str) -> Option<&ProviderEntry> {
        self.providers.get(id)
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

    fn validate(&self, path: &Path) -> Result<(), ConfigError> {
        let invalid = |key: &str, message: String| {
            Err(ConfigError::Invalid {
                path: path.to_owned(),
                key: key.to_owned(),
                message,
            })
        };
        let range = |key, value: u64, min: u64, max: u64| {
            if (min..=max).contains(&value) {
                Ok(())
            } else {
                invalid(key, format!("is {value}: it must be from {min} to {max}"))
            }
        };
        let named = |key, value: &str| {
            if value.trim().is_empty() {
                invalid(key, "is empty".to_owned())
            } else {
                Ok(())
            }
        };
        let scheme = |key, value: &str, schemes: &[&str]| {
            if schemes
                .iter()
                .any(|s| value.starts_with(&format!("{s}://")))
            {
                Ok(())
            } else {
                invalid(
                    key,
                    format!(
                        "is {value:?}: it must start with {}://",
                        schemes.join(":// or ")
                    ),
                )
            }
        };

        let d = &self.dictation;
        range("dictation.min_hold_ms", d.min_hold_ms, 50, 2000)?;
        range("dictation.max_length_s", d.max_length_s, 5, 600)?;
        range("dictation.double_tap_ms", d.double_tap_ms, 100, 1000)?;
        range(
            "dictation.short_dictation_words",
            d.short_dictation_words.into(),
            0,
            50,
        )?;
        range(
            "dictation.dictionary_prompt_cap",
            d.dictionary_prompt_cap.into(),
            10,
            1000,
        )?;
        range(
            "dictation.talking_minutes_per_day",
            d.talking_minutes_per_day.into(),
            1,
            600,
        )?;

        let s = &self.speech;
        let speech = self.role_provider(Role::Speech, "speech.provider", &s.provider, path)?;
        named("speech.model", &s.model)?;
        if !s.endpoint.is_empty() {
            scheme("speech.endpoint", &s.endpoint, speech.protocol.schemes())?;
        }

        let c = &self.cleanup;
        let cleanup = self.role_provider(Role::Cleanup, "cleanup.provider", &c.provider, path)?;
        named("cleanup.model", &c.model)?;
        if !c.endpoint.is_empty() {
            scheme("cleanup.endpoint", &c.endpoint, cleanup.protocol.schemes())?;
        }
        named("cleanup.reasoning_effort", &c.reasoning_effort)?;
        if let Some(prompt) = &c.prompt {
            named("cleanup.prompt", prompt)?;
        }
        range("cleanup.deadline_ms", c.deadline_ms, 100, 10_000)?;

        if let Some(url) = &s.key_url {
            scheme("speech.key_url", url, &["https"])?;
        }
        if let Some(url) = &c.key_url {
            scheme("cleanup.key_url", url, &["https"])?;
        }
        let not_negative = |key, value: Option<f64>| match value {
            Some(v) if !(v.is_finite() && v >= 0.0) => {
                invalid(key, format!("is {v}: it must be 0 or more"))
            }
            _ => Ok(()),
        };
        not_negative("speech.price_per_hour_usd", s.price_per_hour_usd)?;
        not_negative("speech.free_credit_usd", s.free_credit_usd)?;
        not_negative(
            "cleanup.price_input_per_million_usd",
            c.price_input_per_million_usd,
        )?;
        not_negative(
            "cleanup.price_output_per_million_usd",
            c.price_output_per_million_usd,
        )?;
        not_negative(
            "cleanup.estimate_dictations_per_hour",
            c.estimate_dictations_per_hour,
        )?;
        not_negative("cleanup.estimate_input_tokens", c.estimate_input_tokens)?;
        not_negative("cleanup.estimate_output_tokens", c.estimate_output_tokens)?;

        let h = &self.history;
        if let Retention::Days(days) = h.retention_days {
            range("history.retention_days", days.into(), 1, 3650)?;
        }
        range("history.size_limit_mb", h.size_limit_mb, 10, 1_000_000)?;

        let t = &self.tasks;
        let time_of = |key, value: &str| match clock_time(value) {
            Some(time) => Ok(time),
            None => invalid(
                key,
                format!("is {value:?}: write a time as \"HH:MM\", such as \"04:00\""),
            )
            .map(|()| Time::MIDNIGHT),
        };
        time_of("tasks.day_start", &t.day_start)?;
        let from = time_of("tasks.morning_from", &t.morning_from)?;
        let latest = time_of("tasks.morning_latest", &t.morning_latest)?;
        if latest <= from {
            invalid(
                "tasks.morning_latest",
                format!(
                    "is {:?}: it must be later than tasks.morning_from ({:?})",
                    t.morning_latest, t.morning_from
                ),
            )?;
        }
        if t.workdays.is_empty() {
            invalid(
                "tasks.workdays",
                "is empty: name at least one day".to_owned(),
            )?;
        }
        for name in &t.workdays {
            if weekday(name).is_none() {
                invalid(
                    "tasks.workdays",
                    format!("has {name:?}: write mon, tue, wed, thu, fri, sat or sun"),
                )?;
            }
        }
        range("tasks.stale_after", t.stale_after.into(), 1, 30)?;

        for server in &self.connectors.custom {
            if let Err(message) = server.check() {
                invalid("connectors.custom", message)?;
            }
        }
        let mut accounts = HashSet::new();
        for account in &self.connectors.accounts {
            if let Err(message) = account.check() {
                invalid("connectors.accounts", message)?;
            }
            if !accounts.insert((&account.connector, &account.label)) {
                invalid(
                    "connectors.accounts",
                    format!("has {}:{} twice", account.connector, account.label),
                )?;
            }
        }
        Ok(())
    }
}

impl Config {
    /// The catalogue entry a role names, which must exist and fill that role (R-3, R-7).
    fn role_provider(
        &self,
        role: Role,
        key: &'static str,
        id: &str,
        path: &Path,
    ) -> Result<&ProviderEntry, ConfigError> {
        let invalid = |message| ConfigError::Invalid {
            path: path.to_owned(),
            key: key.to_owned(),
            message,
        };
        let fitting = || {
            self.providers_for(role)
                .map(|(id, _)| id)
                .collect::<Vec<_>>()
                .join(", ")
        };
        match self.providers.get(id) {
            Some(entry) if entry.roles.contains(&role) => Ok(entry),
            Some(_) => Err(invalid(format!(
                "is {id:?}, which cannot do {}: providers that can are {}",
                role.section(),
                fitting()
            ))),
            None => Err(invalid(format!(
                "is {id:?}, which is not in the catalogue: providers for {} are {}",
                role.section(),
                fitting()
            ))),
        }
    }
}

/// Checks one catalogue entry: names, endpoint scheme for its protocol, roles its protocol can
/// fill, and prices of 0 or more. Keys name the entry, such as `providers.groq.endpoint`.
fn validate_provider(id: &str, entry: &ProviderEntry, path: &Path) -> Result<(), ConfigError> {
    let invalid = |key: String, message: String| {
        Err(ConfigError::Invalid {
            path: path.to_owned(),
            key,
            message,
        })
    };
    let key = |field: &str| format!("providers.{id}.{field}");
    if entry.name.trim().is_empty() {
        return invalid(key("name"), "is empty".to_owned());
    }
    let schemes = entry.protocol.schemes();
    if !schemes
        .iter()
        .any(|s| entry.endpoint.starts_with(&format!("{s}://")))
    {
        return invalid(
            key("endpoint"),
            format!(
                "is {:?}: a {} endpoint must start with {}://",
                entry.endpoint,
                entry.protocol.name(),
                schemes.join(":// or ")
            ),
        );
    }
    let secure = format!("{}://", "https");
    if let Some(url) = &entry.key_url
        && !url.starts_with(&secure)
    {
        return invalid(
            key("key_url"),
            format!("is {url:?}: it must start with {secure}"),
        );
    }
    if entry.roles.is_empty() {
        return invalid(key("roles"), "is empty: name at least one role".to_owned());
    }
    for role in &entry.roles {
        if !entry.protocol.roles().contains(role) {
            return invalid(
                key("roles"),
                format!(
                    "has {:?}, which the {} protocol cannot do",
                    role.section(),
                    entry.protocol.name()
                ),
            );
        }
    }
    let not_negative = |field: &str, value: Option<f64>| match value {
        Some(v) if !(v.is_finite() && v >= 0.0) => {
            invalid(field.to_owned(), format!("is {v}: it must be 0 or more"))
        }
        _ => Ok(()),
    };
    not_negative(&key("free_credit_usd"), entry.free_credit_usd)?;
    for model in &entry.models {
        if model.id.trim().is_empty() {
            return invalid(key("models"), "has a model with an empty id".to_owned());
        }
        let field = |name: &str| key(&format!("models.{}.{name}", model.id));
        not_negative(&field("price_per_hour_usd"), model.price_per_hour_usd)?;
        not_negative(
            &field("price_input_per_million_usd"),
            model.price_input_per_million_usd,
        )?;
        not_negative(
            &field("price_output_per_million_usd"),
            model.price_output_per_million_usd,
        )?;
    }
    Ok(())
}

/// R-26: fails on the first key-like field anywhere in the file.
fn reject_secrets(
    table: &toml::Table,
    prefix: &str,
    text: &str,
    path: &Path,
) -> Result<(), ConfigError> {
    for (name, value) in table {
        let key = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}.{name}")
        };
        if looks_like_secret(name) {
            return Err(ConfigError::Secret {
                path: path.to_owned(),
                key,
                line: line_of_key(text, name),
            });
        }
        if let toml::Value::Table(inner) = value {
            reject_secrets(inner, &key, text, path)?;
        }
    }
    Ok(())
}

/// The 1-based line and full name of the first key-like key set in `text`, read line by line
/// so it works on text that is not valid TOML: `api_key = ...`, `cleanup.token=...`,
/// `"secret" = ...`. Comments are skipped; section headers set the prefix.
fn find_secret_line(text: &str) -> Option<(usize, String)> {
    let mut section = String::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim_start();
        if line.starts_with('#') {
            continue;
        }
        if let Some(header) = line.strip_prefix('[') {
            let header = header.trim_start_matches('[');
            section = header
                .split(']')
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned();
            continue;
        }
        let Some((key, _)) = line.split_once('=') else {
            continue;
        };
        let key: String = key
            .chars()
            .filter(|c| !matches!(c, '"' | '\'') && !c.is_whitespace())
            .collect();
        if key.rsplit('.').next().is_some_and(looks_like_secret) {
            let full = if section.is_empty() {
                key
            } else {
                format!("{section}.{key}")
            };
            return Some((index + 1, full));
        }
    }
    None
}

/// `api_key`, `key`, `groq_key`, `token`, `secret`, `password` and the like. Not `hotkey`.
pub(crate) fn looks_like_secret(name: &str) -> bool {
    let name = name.to_lowercase();
    ["key", "apikey", "token", "secret", "password", "passwd"]
        .iter()
        .any(|word| {
            name == *word
                || name.ends_with(&format!("_{word}"))
                || name.ends_with(&format!("-{word}"))
        })
}

/// The 1-based line where `name = ...` is set, for the error message.
fn line_of_key(text: &str, name: &str) -> Option<usize> {
    text.lines()
        .position(|line| {
            line.trim_start()
                .strip_prefix(name)
                .is_some_and(|rest| rest.trim_start().starts_with('='))
        })
        .map(|index| index + 1)
}

/// Sets one value in config text, as the Settings window does, keeping every other line and
/// comment. `value` is a TOML value (`2000`, `"right_option"`, `true`). A key that is commented
/// out in the file (`# dictionary = ...`) is switched on. The result is validated like the file
/// itself, so an out-of-range value, an unknown key or a key-like field (R-26) is refused and
/// nothing changes.
pub fn set_value(text: &str, section: &str, key: &str, value: &str) -> Result<String, ConfigError> {
    let path = Path::new("config.toml");
    let full = format!("{section}.{key}");
    if looks_like_secret(key) {
        return Err(ConfigError::Secret {
            path: path.to_owned(),
            key: full,
            line: None,
        });
    }
    let unknown = |what: String| ConfigError::Parse {
        path: path.to_owned(),
        message: what,
    };
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let header = format!("[{section}]");
    let start = lines
        .iter()
        .position(|l| l.trim() == header)
        .ok_or_else(|| unknown(format!("there is no [{section}] section")))?;
    let end = lines[start + 1..]
        .iter()
        .position(|l| l.trim_start().starts_with('['))
        .map_or(lines.len(), |i| start + 1 + i);
    let sets_key = |line: &str| {
        line.trim_start()
            .strip_prefix(key)
            .is_some_and(|rest| rest.trim_start().starts_with('='))
    };
    let index = lines[start + 1..end]
        .iter()
        .position(|l| sets_key(l))
        .or_else(|| {
            lines[start + 1..end]
                .iter()
                .position(|l| l.trim_start().strip_prefix('#').is_some_and(&sets_key))
        })
        .map(|i| start + 1 + i)
        .ok_or_else(|| unknown(format!("{full} is not a setting")))?;
    lines[index] = format!("{key} = {value}");
    let mut out = lines.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    Config::parse(&out, path)?;
    Ok(out)
}

/// The keys an older file may set in a role's section that belong to its provider: they are
/// commented out when the role changes provider (R-4).
const PROVIDER_OVERRIDES: &[&str] = &[
    "endpoint",
    "key_url",
    "price_per_hour_usd",
    "price_input_per_million_usd",
    "price_output_per_million_usd",
    "free_credit_usd",
];

/// A TOML string, quoted and escaped.
fn quoted(value: &str) -> String {
    toml::Value::String(value.to_owned()).to_string()
}

/// The body of `[section]`: the first line after the header and the line after the last.
fn section_body(lines: &[String], section: &str) -> Option<(usize, usize)> {
    let header = format!("[{section}]");
    let start = lines.iter().position(|l| l.trim() == header)? + 1;
    let end = lines[start..]
        .iter()
        .position(|l| l.trim_start().starts_with('['))
        .map_or(lines.len(), |i| start + i);
    Some((start, end))
}

/// Whether `line` sets `key`, as `key = ...`.
fn sets(line: &str, key: &str) -> bool {
    line.trim_start()
        .strip_prefix(key)
        .is_some_and(|rest| rest.trim_start().starts_with('='))
}

/// Sets `key = value` in the section body: the line that sets it, else a commented-out one,
/// else a new line after the header.
fn put(lines: &mut Vec<String>, (start, end): (usize, usize), key: &str, value: String) {
    let line = format!("{key} = {value}");
    let body = &lines[start..end];
    let found = body.iter().position(|l| sets(l, key)).or_else(|| {
        body.iter().position(|l| {
            l.trim_start()
                .strip_prefix('#')
                .is_some_and(|r| sets(r, key))
        })
    });
    match found {
        Some(i) => lines[start + i] = line,
        None => lines.insert(start, line),
    }
}

fn joined(lines: &[String], like: &str) -> String {
    let mut out = lines.join("\n");
    if like.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Sets a role's provider and model in one write, as Settings does when the user picks them
/// (R-8). Keeps every other line and comment. When the provider changes, an older file's
/// endpoint, key page and prices for the old provider are commented out. Cleanup's reasoning
/// effort becomes one the model takes: on the same provider the current one if it may, else the
/// model's first (the catalogue lists the fastest first), or "off" when the catalogue lists none
/// for it or does not know the model on another provider.
/// The result is validated like the file itself, so a provider that cannot fill the role is
/// refused and nothing changes.
pub fn choose_provider(
    text: &str,
    role: Role,
    provider: &str,
    model: &str,
) -> Result<String, ConfigError> {
    choose(text, role, provider, model, false, None)
}

fn choose(
    text: &str,
    role: Role,
    provider: &str,
    model: &str,
    clear_overrides: bool,
    effort: Option<&str>,
) -> Result<String, ConfigError> {
    let path = Path::new("config.toml");
    let config = Config::parse(text, path)?;
    let (current, current_effort) = match role {
        Role::Speech => (&config.speech.provider, None),
        Role::Cleanup => (
            &config.cleanup.provider,
            Some(config.cleanup.reasoning_effort.as_str()),
        ),
    };
    let same_provider = current == provider;
    let section = role.section();
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let range = section_body(&lines, section).ok_or_else(|| ConfigError::Parse {
        path: path.to_owned(),
        message: format!("there is no [{section}] section"),
    })?;
    if clear_overrides || !same_provider {
        for line in &mut lines[range.0..range.1] {
            if PROVIDER_OVERRIDES.iter().any(|key| sets(line, key)) {
                *line = format!("# {}", line.trim_start());
            }
        }
    }
    put(&mut lines, range, "provider", quoted(provider));
    put(&mut lines, range, "model", quoted(model));
    if let Some(current_effort) = current_effort {
        let listed = config
            .provider(provider)
            .and_then(|p| p.models.iter().find(|m| m.id == model))
            .map(|m| &m.reasoning_effort);
        let chosen = match (effort, listed) {
            (Some(effort), _) => effort.to_owned(),
            (None, Some(values)) if same_provider && values.iter().any(|v| v == current_effort) => {
                current_effort.to_owned()
            }
            (None, Some(values)) => values.first().map_or("off", String::as_str).to_owned(),
            (None, None) if same_provider => current_effort.to_owned(),
            (None, None) => "off".to_owned(),
        };
        put(&mut lines, range, "reasoning_effort", quoted(&chosen));
    }
    let out = joined(&lines, text);
    Config::parse(&out, path)?;
    Ok(out)
}

/// Puts every role back to the recommended provider and model, with the recommended reasoning
/// effort and no endpoint or price overrides (R-10). Keys live in the keychain and providers the
/// user added stay in the file.
pub fn reset_to_recommended(text: &str) -> Result<String, ConfigError> {
    let (provider, model) = Config::recommended(Role::Speech);
    let text = choose(text, Role::Speech, provider, model, true, None)?;
    let (provider, model) = Config::recommended(Role::Cleanup);
    let effort = defaults().cleanup.reasoning_effort.as_str();
    choose(&text, Role::Cleanup, provider, model, true, Some(effort))
}

/// Adds a cleanup provider at `endpoint` speaking the OpenAI-compatible protocol, as Settings'
/// "Add a custom endpoint…" does (design.md, "Settings page"; R-5). The entry is appended to the
/// user's file under a name made from `name` and not yet taken. Returns the new text and that
/// name. The result is validated, so a bad address is refused and nothing changes.
pub fn add_provider(
    text: &str,
    name: &str,
    endpoint: &str,
) -> Result<(String, String), ConfigError> {
    let path = Path::new("config.toml");
    let invalid = |key: &str, message: &str| ConfigError::Invalid {
        path: path.to_owned(),
        key: key.to_owned(),
        message: message.to_owned(),
    };
    let name = name.trim();
    if name.is_empty() {
        return Err(invalid("name", "is empty"));
    }
    let config = Config::parse(text, path)?;
    let mut slug = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let base = match slug.trim_end_matches('-') {
        "" => "custom".to_owned(),
        s => s.to_owned(),
    };
    let id = (1..)
        .map(|n| {
            if n == 1 {
                base.clone()
            } else {
                format!("{base}-{n}")
            }
        })
        .find(|id| config.provider(id).is_none())
        .expect("some name is free");
    let mut out = text.to_owned();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!(
        "\n# Added in Settings: an OpenAI-compatible cleanup endpoint.\n[providers.{id}]\n\
         name = {}\nprotocol = \"openai-chat\"\nendpoint = {}\nroles = [\"cleanup\"]\n\
         list_models = true\n",
        quoted(name),
        quoted(endpoint.trim().trim_end_matches('/'))
    ));
    Config::parse(&out, path)?;
    Ok((out, id))
}
