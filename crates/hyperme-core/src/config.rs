//! The config file: `config.toml` in the app's data folder (design.md, "Config").
//!
//! Provider and model names and endpoints come from this file, never from code (R-26); their
//! defaults live in `default-config.toml`, which is written on first launch. The file holds no
//! secrets: a key-like field is an error. Unknown keys are an error naming the file and line,
//! and out-of-range values name the key.

use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The file written when there is none, and the source of every default.
pub const DEFAULT_CONFIG: &str = include_str!("default-config.toml");

/// The only speech providers with a client. Speech protocols differ, so each needs its own code.
const SPEECH_PROVIDERS: &[&str] = &["speechmatics"];

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub dictation: Dictation,
    pub speech: Speech,
    pub cleanup: Cleanup,
    pub history: History,
    pub timing: Timing,
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
    /// R-13: fewer words than this skip cleanup.
    pub short_dictation_words: u32,
    /// R-32.
    pub sounds: bool,
    /// The user's terms, one per line.
    pub dictionary: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Speech {
    pub provider: String,
    pub model: String,
    pub endpoint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cleanup {
    /// Labels the keychain entry and error messages (R-21).
    pub provider: String,
    pub model: String,
    /// Base URL of an OpenAI-compatible chat completions API.
    pub endpoint: String,
    /// Sent as `reasoning_effort`; `off` leaves the field out.
    pub reasoning_effort: String,
    /// The cleanup prompt version.
    pub prompt: String,
    /// R-12: after this, the raw transcript is pasted.
    pub deadline_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct History {
    /// R-18.
    pub enabled: bool,
    pub retention_days: Retention,
    pub size_limit_mb: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timing {
    /// R-30: one line of stage timings per dictation, numbers only.
    pub log: bool,
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
        key: &'static str,
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
        let config: Config = toml::from_str(text).map_err(parse_error)?;
        config.validate(path)?;
        Ok(config)
    }

    fn validate(&self, path: &Path) -> Result<(), ConfigError> {
        let invalid = |key, message: String| {
            Err(ConfigError::Invalid {
                path: path.to_owned(),
                key,
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
        range(
            "dictation.short_dictation_words",
            d.short_dictation_words.into(),
            0,
            50,
        )?;

        let s = &self.speech;
        if !SPEECH_PROVIDERS.contains(&s.provider.as_str()) {
            invalid(
                "speech.provider",
                format!(
                    "is {:?}: supported providers are {}",
                    s.provider,
                    SPEECH_PROVIDERS.join(", ")
                ),
            )?;
        }
        named("speech.model", &s.model)?;
        scheme("speech.endpoint", &s.endpoint, &["wss", "ws"])?;

        let c = &self.cleanup;
        named("cleanup.provider", &c.provider)?;
        named("cleanup.model", &c.model)?;
        scheme("cleanup.endpoint", &c.endpoint, &["https", "http"])?;
        named("cleanup.reasoning_effort", &c.reasoning_effort)?;
        named("cleanup.prompt", &c.prompt)?;
        range("cleanup.deadline_ms", c.deadline_ms, 100, 10_000)?;

        let h = &self.history;
        if let Retention::Days(days) = h.retention_days {
            range("history.retention_days", days.into(), 1, 3650)?;
        }
        range("history.size_limit_mb", h.size_limit_mb, 10, 1_000_000)?;
        Ok(())
    }
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
fn looks_like_secret(name: &str) -> bool {
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
