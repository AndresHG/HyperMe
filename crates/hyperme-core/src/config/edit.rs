//! Edits to the config's text, line by line, so the user's comments and layout survive: one
//! value, a provider choice, a fallback, a reset, or a new provider. Each result is parsed
//! before it is returned.

use super::*;

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
        .map(|i| start + 1 + i);
    match index {
        Some(index) => lines[index] = format!("{key} = {value}"),
        // A setting added after the file was written (#221): it goes at the end of its section,
        // and the parse below refuses a key that is not a setting at all.
        None => {
            let last = lines[start..end]
                .iter()
                .rposition(|l| !l.trim().is_empty())
                .map_or(start, |i| start + i);
            lines.insert(last + 1, format!("{key} = {value}"));
        }
    }
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

/// Sets or clears a role's fallback (R-24a: Settings calls this only after the fallback's Test
/// passed). Keeps every other line; the result is validated, so a provider that cannot fill
/// the role is refused and nothing changes.
pub fn set_fallback(
    text: &str,
    role: Role,
    choice: Option<(&str, &str)>,
) -> Result<String, ConfigError> {
    let path = Path::new("config.toml");
    let section = role.section();
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let range = section_body(&lines, section).ok_or_else(|| ConfigError::Parse {
        path: path.to_owned(),
        message: format!("there is no [{section}] section"),
    })?;
    match choice {
        Some((provider, model)) => put(
            &mut lines,
            range,
            "fallback",
            format!(
                "{{ provider = {}, model = {} }}",
                quoted(provider),
                quoted(model)
            ),
        ),
        None => {
            let (start, end) = range;
            let kept: Vec<String> = lines[start..end]
                .iter()
                .filter(|l| !sets(l, "fallback"))
                .cloned()
                .collect();
            lines.splice(start..end, kept);
        }
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
