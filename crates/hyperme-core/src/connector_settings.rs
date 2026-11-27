//! Settings → Connectors as pure logic (specs/97-google-connectors, slice 5; docs/UX.md): the
//! list rows, the Setup check rows, the account page's tool switches, the Add connector sheet,
//! the `hyperme://settings/...` links, and writing the `[connectors]` part of the config. The
//! page only draws what this returns; nothing here touches the network or the keychain.

use std::path::Path;

use serde::{Deserialize, Serialize};
use time::UtcDateTime;

use crate::config::{Config, ConfigError, Connectors};
use crate::registry::{
    Connector, CustomAuth, CustomServer, Registry, RegistryError, ToolAccess, ToolLists,
};

/// A connected account in the config: which connector, and the label the user picked
/// ("personal", "work"). Its refresh token is the keychain item `<connector>:<label>`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AccountEntry {
    pub connector: String,
    pub label: String,
    /// Off keeps the account and its sign-in but stops using it (R-3).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub enabled: bool,
    /// The setup sheet's switches that were on at sign-in (`calendar`, `gmail.drafts`).
    /// Empty means the connector's defaults.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
}

fn yes() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

/// Lowercase letters, digits, `-` and `_`, starting with a letter.
fn is_slug(text: &str) -> bool {
    text.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

impl AccountEntry {
    /// `google:personal`: the keychain account and the link id.
    pub fn id(&self) -> String {
        format!("{}:{}", self.connector, self.label)
    }

    pub(crate) fn check(&self) -> Result<(), String> {
        if !is_slug(&self.connector) {
            return Err(format!(
                "connector {:?} is not a connector id",
                self.connector
            ));
        }
        if !is_slug(&self.label) {
            return Err(format!(
                "label {:?} must be lowercase letters, digits, - or _, starting with a letter",
                self.label
            ));
        }
        Ok(())
    }
}

/// How an account is doing, from the keychain and the last sync; the app works it out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountHealth {
    /// No refresh token in the keychain.
    NotSignedIn,
    /// The refresh token was refused (R-10).
    SignInAgain,
    Works {
        synced: Option<UtcDateTime>,
    },
}

/// The colour of a state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    Ok,
    Warn,
    Bad,
    Off,
}

/// One row of the list page or of Setup check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Row {
    /// `google:personal` for an account, the connector id otherwise.
    pub id: String,
    pub connector: String,
    pub title: String,
    pub state: String,
    pub tone: Tone,
    pub link: String,
}

/// The registry with the config's custom servers and the user's tool switches.
pub fn registry(
    presets_dir: Option<&Path>,
    connectors: &Connectors,
) -> Result<Registry, RegistryError> {
    Ok(Registry::load(presets_dir, &connectors.custom)?.with_user_tools(&connectors.tools))
}

/// "synced 3 min ago", or "not synced yet".
pub fn sync_age(now: UtcDateTime, synced: Option<UtcDateTime>) -> String {
    let Some(synced) = synced else {
        return "not synced yet".to_owned();
    };
    let minutes = (now - synced).whole_minutes().max(0);
    match minutes {
        0 => "synced just now".to_owned(),
        1..=59 => format!("synced {minutes} min ago"),
        60..=1439 => format!("synced {} h ago", minutes / 60),
        _ => format!("synced {} days ago", minutes / 1440),
    }
}

fn account_state(entry: &AccountEntry, health: AccountHealth, now: UtcDateTime) -> (String, Tone) {
    if !entry.enabled {
        return ("Off".to_owned(), Tone::Off);
    }
    match health {
        AccountHealth::NotSignedIn => ("Not signed in".to_owned(), Tone::Warn),
        AccountHealth::SignInAgain => ("Sign in again".to_owned(), Tone::Bad),
        AccountHealth::Works { synced } => (format!("Works · {}", sync_age(now, synced)), Tone::Ok),
    }
}

fn is_custom(connectors: &Connectors, id: &str) -> bool {
    connectors.custom.iter().any(|server| server.id == id)
}

fn account_rows(
    connector: &Connector,
    connectors: &Connectors,
    health: &dyn Fn(&str, &str) -> AccountHealth,
    now: UtcDateTime,
) -> Vec<Row> {
    connectors
        .accounts
        .iter()
        .filter(|entry| entry.connector == connector.id)
        .map(|entry| {
            let (state, tone) = account_state(entry, health(&entry.connector, &entry.label), now);
            Row {
                id: entry.id(),
                connector: connector.id.clone(),
                title: format!("{} · {}", connector.name, entry.label),
                state,
                tone,
                link: link_for(&entry.id()),
            }
        })
        .collect()
}

fn custom_row(
    connector: &Connector,
    health: &dyn Fn(&str, &str) -> AccountHealth,
    now: UtcDateTime,
) -> Row {
    let entry = AccountEntry {
        connector: connector.id.clone(),
        label: String::new(),
        enabled: true,
        features: Vec::new(),
    };
    let (state, tone) = account_state(&entry, health(&connector.id, ""), now);
    Row {
        id: connector.id.clone(),
        connector: connector.id.clone(),
        title: connector.name.clone(),
        state,
        tone,
        link: link_for(&connector.id),
    }
}

/// The list page (R-3): one row per account of each connector, then one per custom server.
/// A preset with no account is not listed; Add connector offers it.
pub fn list_rows(
    registry: &Registry,
    connectors: &Connectors,
    health: &dyn Fn(&str, &str) -> AccountHealth,
    now: UtcDateTime,
) -> Vec<Row> {
    let mut rows = Vec::new();
    for connector in registry.connectors() {
        if is_custom(connectors, &connector.id) {
            rows.push(custom_row(connector, health, now));
        } else {
            rows.extend(account_rows(connector, connectors, health, now));
        }
    }
    rows
}

/// Setup check (R-6, AC-13): a row per connector in use, then a row per account, each linking
/// to its place in Connectors.
pub fn setup_rows(
    registry: &Registry,
    connectors: &Connectors,
    health: &dyn Fn(&str, &str) -> AccountHealth,
    now: UtcDateTime,
) -> Vec<Row> {
    let mut rows = Vec::new();
    for connector in registry.connectors() {
        if is_custom(connectors, &connector.id) {
            rows.push(custom_row(connector, health, now));
            continue;
        }
        let accounts = account_rows(connector, connectors, health, now);
        if accounts.is_empty() {
            continue;
        }
        let needs = accounts
            .iter()
            .filter(|row| matches!(row.tone, Tone::Bad | Tone::Warn))
            .count();
        let total = accounts.len();
        let plural = if total == 1 { "account" } else { "accounts" };
        let (state, tone) = match needs {
            0 if total == 1 => ("1 account works".to_owned(), Tone::Ok),
            0 => (format!("All {total} accounts work"), Tone::Ok),
            n => (format!("{n} of {total} {plural} needs you"), Tone::Bad),
        };
        rows.push(Row {
            id: connector.id.clone(),
            connector: connector.id.clone(),
            title: connector.name.clone(),
            state,
            tone,
            link: link_for(&connector.id),
        });
        rows.extend(accounts);
    }
    rows
}

/// Where a `hyperme://settings/<section>[/<item>]` link goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub section: String,
    pub item: Option<String>,
}

impl Link {
    /// From the Settings page's hash, `connectors/google:work`.
    pub fn from_hash(hash: &str) -> Option<Link> {
        let hash = hash.strip_prefix('#').unwrap_or(hash);
        let (section, item) = match hash.split_once('/') {
            Some((section, item)) => (section, Some(item)),
            None => (hash, None),
        };
        let section_ok =
            !section.is_empty() && section.chars().all(|c| c.is_ascii_lowercase() || c == '-');
        let item_ok = item.is_none_or(|item| {
            !item.is_empty()
                && item.chars().all(|c| {
                    c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_' | ':')
                })
        });
        (section_ok && item_ok).then(|| Link {
            section: section.to_owned(),
            item: item.map(str::to_owned),
        })
    }

    /// The Settings page's hash for this link.
    pub fn hash(&self) -> String {
        match &self.item {
            Some(item) => format!("{}/{item}", self.section),
            None => self.section.clone(),
        }
    }
}

/// The link to a connector or account row (UX rule 5, R-6).
pub fn link_for(id: &str) -> String {
    format!("hyperme://settings/connectors/{id}")
}

/// Reads a `hyperme://settings/...` link; anything else is `None`.
pub fn parse_link(url: &str) -> Option<Link> {
    Link::from_hash(url.strip_prefix("hyperme://settings/")?)
}

/// One tool on the account page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolRow {
    pub name: String,
    pub access: ToolAccess,
    pub default: ToolAccess,
    /// The user set this tool's switch.
    pub changed: bool,
}

/// The tools a connector names (its lists without patterns, then the user's), with their
/// access. The full list from the server's `tools/list` joins it once an account is connected.
pub fn tool_rows(connector: &Connector) -> Vec<ToolRow> {
    let mut names: Vec<&String> = Vec::new();
    let lists = [&connector.tools, &connector.user_tools];
    for lists in lists {
        for name in lists.read.iter().chain(&lists.ask).chain(&lists.off) {
            if !name.contains('*') && !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names
        .into_iter()
        .map(|name| ToolRow {
            name: name.clone(),
            access: connector.access(name),
            default: connector.default_access(name),
            changed: connector.user_access(name).is_some(),
        })
        .collect()
}

/// Sets the user's switch for `tool` on `connector` (R-5).
pub fn set_tool(
    connectors: &mut Connectors,
    connector: &str,
    tool: &str,
    access: ToolAccess,
) -> Result<(), String> {
    let known = registry(None, connectors).map_err(|e| e.to_string())?;
    if known.get(connector).is_none() {
        return Err(format!("{connector} is not a connector"));
    }
    let valid_tool = !tool.is_empty()
        && tool
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if !valid_tool {
        return Err(format!("{tool:?} is not a tool name"));
    }
    let lists = connectors.tools.entry(connector.to_owned()).or_default();
    for list in [&mut lists.read, &mut lists.ask, &mut lists.off] {
        list.retain(|name| name != tool);
    }
    match access {
        ToolAccess::Read => lists.read.push(tool.to_owned()),
        ToolAccess::Ask => lists.ask.push(tool.to_owned()),
        ToolAccess::Off => lists.off.push(tool.to_owned()),
    }
    Ok(())
}

/// Turns an account (`google:personal`) on or off without disconnecting it (R-3).
pub fn set_enabled(
    connectors: &mut Connectors,
    account: &str,
    enabled: bool,
) -> Result<(), String> {
    let entry = connectors
        .accounts
        .iter_mut()
        .find(|entry| entry.id() == account)
        .ok_or_else(|| format!("{account} is not a connected account"))?;
    entry.enabled = enabled;
    Ok(())
}

/// One choice in the Add connector sheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Choice {
    pub id: String,
    pub name: String,
    /// Can be added from the sheet now.
    pub ready: bool,
    pub note: String,
}

/// The Add connector sheet: the presets, then "Custom MCP server".
pub fn add_choices(registry: &Registry, connectors: &Connectors) -> Vec<Choice> {
    let mut choices: Vec<Choice> = registry
        .connectors()
        .iter()
        .filter(|connector| !is_custom(connectors, &connector.id))
        .map(|connector| {
            let oauth = matches!(connector.auth, crate::registry::Auth::OAuth(_));
            Choice {
                id: connector.id.clone(),
                name: connector.name.clone(),
                ready: oauth,
                note: if oauth {
                    "Your own OAuth client, then sign in".to_owned()
                } else {
                    "Added in config.toml for now".to_owned()
                },
            }
        })
        .collect();
    choices.push(Choice {
        id: "custom".to_owned(),
        name: "Custom MCP server".to_owned(),
        ready: true,
        note: "By URL, with a bearer token or none".to_owned(),
    });
    choices
}

/// The Custom MCP server form.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CustomForm {
    pub name: String,
    pub url: String,
    pub command: String,
    /// `none` or `bearer`.
    pub auth: String,
}

/// An id from a name: "Team wiki" becomes `team-wiki`, kept unique.
fn new_id(name: &str, taken: &dyn Fn(&str) -> bool) -> String {
    let mut base = String::new();
    for c in name.trim().to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            base.push(c);
        } else if !base.is_empty() && !base.ends_with('-') {
            base.push('-');
        }
    }
    let mut base = base.trim_end_matches('-').to_owned();
    if !base.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
        base.insert_str(0, "mcp-");
    }
    let base = base.trim_end_matches('-').to_owned();
    if !taken(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|id| !taken(id))
        .unwrap_or(base)
}

/// Adds a custom server from the sheet (R-4) and returns its id. A server started by a command
/// is refused for now: the client speaks streamable HTTP only.
pub fn add_custom(
    connectors: &mut Connectors,
    registry: &Registry,
    form: &CustomForm,
) -> Result<String, String> {
    if !form.command.trim().is_empty() {
        return Err(
            "Servers started by a command are not supported yet; add one by URL".to_owned(),
        );
    }
    let auth = match form.auth.as_str() {
        "none" => CustomAuth::None,
        "bearer" => CustomAuth::Bearer,
        other => return Err(format!("{other:?} sign-in is set in config.toml for now")),
    };
    let taken = |id: &str| {
        registry.get(id).is_some() || connectors.custom.iter().any(|server| server.id == id)
    };
    let id = new_id(&form.name, &taken);
    let server = CustomServer {
        id: id.clone(),
        name: form.name.trim().to_owned(),
        url: Some(form.url.trim().to_owned()),
        command: None,
        args: Vec::new(),
        auth,
        auth_url: None,
        token_url: None,
        revoke_url: None,
        client_id: None,
        scopes: Vec::new(),
        extra_auth_params: Default::default(),
        tools: ToolLists::default(),
    };
    server.check()?;
    connectors.custom.push(server);
    Ok(id)
}

/// Replaces the `[connectors]` part of the config text with `connectors`, keeping every other
/// line as it was, and checks the result loads.
pub fn write_connectors(text: &str, connectors: &Connectors) -> Result<String, ConfigError> {
    let mut kept = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') {
            let name = trimmed
                .trim_start_matches('[')
                .split(']')
                .next()
                .unwrap_or_default()
                .trim();
            inside = name == "connectors" || name.starts_with("connectors.");
        }
        if !inside {
            kept.push(line);
        }
    }
    let mut out = kept.join("\n").trim_end().to_owned();
    out.push('\n');
    if *connectors != Connectors::default() {
        #[derive(Serialize)]
        struct File<'a> {
            connectors: &'a Connectors,
        }
        let table = toml::to_string(&File { connectors }).map_err(|e| ConfigError::Parse {
            path: "config.toml".into(),
            message: e.to_string(),
        })?;
        out.push('\n');
        out.push_str(&table);
    }
    Config::parse(&out, Path::new("config.toml"))?;
    Ok(out)
}
