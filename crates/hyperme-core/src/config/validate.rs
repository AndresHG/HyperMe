//! Checks on a parsed config: values in range, providers that exist and fill their role, and no
//! secrets in the file. Errors name the key, and the file and line where they can.

use super::*;

impl Config {
    pub(super) fn validate(&self, path: &Path) -> Result<(), ConfigError> {
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

        for (id, entry) in &self.providers {
            if let Some(from) = &entry.key_from
                && (from == id || !self.providers.contains_key(from))
            {
                invalid(
                    &format!("providers.{id}.key_from"),
                    format!("is {from:?}: it must name another provider in the catalogue"),
                )?;
            }
        }

        let s = &self.speech;
        let speech = self.role_provider(Role::Speech, "speech.provider", &s.provider, path)?;
        named("speech.model", &s.model)?;
        if let Some(f) = &s.fallback {
            self.role_provider(Role::Speech, "speech.fallback", &f.provider, path)?;
            named("speech.fallback", &f.model)?;
        }
        if !s.endpoint.is_empty() {
            scheme("speech.endpoint", &s.endpoint, speech.protocol.schemes())?;
        }

        let c = &self.cleanup;
        let cleanup = self.role_provider(Role::Cleanup, "cleanup.provider", &c.provider, path)?;
        named("cleanup.model", &c.model)?;
        if let Some(f) = &c.fallback {
            self.role_provider(Role::Cleanup, "cleanup.fallback", &f.provider, path)?;
            named("cleanup.fallback", &f.model)?;
        }
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
pub(super) fn validate_provider(
    id: &str,
    entry: &ProviderEntry,
    path: &Path,
) -> Result<(), ConfigError> {
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
pub(super) fn reject_secrets(
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
pub(super) fn find_secret_line(text: &str) -> Option<(usize, String)> {
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
