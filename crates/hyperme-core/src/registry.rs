//! The connector registry (specs/97-google-connectors, slice 4): presets as data plus the
//! user's custom MCP servers, each a [`Connector`] with its servers, how it signs in, its tool
//! defaults and its pulls. Nothing here is network code or knows a service: Google is
//! `presets/google.toml`, and a new preset is a new file (R-1, R-2, AC-1).
//!
//! Tool defaults (R-5): each tool is read, ask before, or off. Lists in the preset name tools
//! or `*` patterns; a tool no list names asks before, and when lists overlap the strictest
//! wins. A job's allowlist may hold read tools only (R-22, AC-10).

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::looks_like_secret;

/// The presets that ship with HyperMe, as (file name, text).
const BUILT_IN: &[(&str, &str)] = &[("google.toml", include_str!("../presets/google.toml"))];

/// Why a preset or a custom server was refused. Names the file or entry and the reason; never
/// carries a secret's value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryError(pub String);

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for RegistryError {}

/// What a tool may do without the user (R-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolAccess {
    /// Runs on its own, and may be in a job's allowlist.
    Read,
    /// Shows a preview and waits for Enter.
    Ask,
    /// Never runs.
    Off,
}

/// Tool names or `*` patterns for each access level.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToolLists {
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub read: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ask: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub off: Vec<String>,
}

impl ToolLists {
    pub fn is_empty(&self) -> bool {
        self.read.is_empty() && self.ask.is_empty() && self.off.is_empty()
    }

    /// The access for `tool`: the strictest list that matches it, or ask before.
    pub fn access(&self, tool: &str) -> ToolAccess {
        let matches = |list: &[String]| list.iter().any(|pattern| glob(pattern, tool));
        if matches(&self.off) {
            ToolAccess::Off
        } else if matches(&self.ask) {
            ToolAccess::Ask
        } else if matches(&self.read) {
            ToolAccess::Read
        } else {
            ToolAccess::Ask
        }
    }
}

/// `*` matches any run of characters, including none; everything else matches itself.
pub(crate) fn glob(pattern: &str, name: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or_default();
    let Some(mut rest) = name.strip_prefix(first) else {
        return false;
    };
    let parts: Vec<&str> = parts.collect();
    let Some((last, middle)) = parts.split_last() else {
        return rest.is_empty();
    };
    for part in middle {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.len() >= last.len() && rest.ends_with(last)
}

/// A declarative pull (G-5): call `tool` on `server` with `args`, keep these fields, store them
/// in `into`. Run without a model by the sync jobs of #92.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pull {
    pub server: String,
    pub tool: String,
    /// Arguments; `{today_start}`, `{today_plus_7}` and `{now}` are filled in at run time.
    #[serde(default)]
    pub args: BTreeMap<String, String>,
    /// Paths of the fields kept from each item, for documentation and review.
    #[serde(default)]
    pub keep: Vec<String>,
    /// Where the items are in the tool's answer (`items`); empty when it is the list itself.
    #[serde(default)]
    pub list: String,
    /// Each stored field and the path it comes from; `a|b` takes the first that is there.
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
    /// Also mirror each item as a task (R-27).
    #[serde(default)]
    pub mirror: bool,
    /// Mail: items with any of these labels are not counted or mirrored (R-26).
    #[serde(default)]
    pub exclude_labels: Vec<String>,
    /// Mail: the label on the last message that means the account wrote last (R-26, R-27).
    pub done_label: Option<String>,
    /// A link back to the item, with `{id}` filled in, when the answer has none.
    pub link: Option<String>,
    /// Mail: the read tool that fetches a body on demand, its id argument and the path of the
    /// text in its answer (R-18).
    pub body_tool: Option<String>,
    pub body_arg: Option<String>,
    pub body_path: Option<String>,
    pub into: String,
}

/// How a connector's server is reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
    /// A remote server over streamable HTTP.
    Http(String),
    /// A local server started as a command, over standard input and output.
    Command { program: String, args: Vec<String> },
}

/// One MCP server of a connector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub id: String,
    pub transport: Transport,
    /// The OAuth scopes this server always needs.
    pub scopes: Vec<String>,
    /// Scopes asked for only when the named feature is switched on, such as `drafts`.
    pub optional_scopes: BTreeMap<String, Vec<String>>,
    /// The switch that turns this server on in the setup sheet, in plain words.
    pub label: Option<String>,
    /// The switch for each optional scope, by feature.
    pub optional_labels: BTreeMap<String, String>,
    /// The provider's daily quota for this server, in units (R-20).
    pub daily_units: Option<u32>,
    /// What each tool costs in units; a tool not named costs 1.
    pub units: BTreeMap<String, u32>,
}

/// Help for setting up a connector's own OAuth client (G-1), from the preset.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    /// The page where the user creates their client.
    pub client_url: String,
    /// The page where the user sets the client's project to "In production" (R-10a).
    pub production_url: Option<String>,
    /// Shell commands for the "Use gcloud instead" path, copied as text, never run.
    #[serde(default)]
    pub commands: Vec<String>,
}

/// The OAuth endpoints and consent parameters of a connector (RFC 8252).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthEndpoints {
    pub auth_url: String,
    pub token_url: String,
    pub revoke_url: Option<String>,
    pub extra_auth_params: Vec<(String, String)>,
    /// A custom server's client id, when the user gave one. A preset's comes from its setup.
    pub client_id: Option<String>,
}

/// How a connector signs in. Tokens and client secrets live in the keychain, never here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Auth {
    OAuth(OAuthEndpoints),
    /// A bearer token the user pasted, kept in the keychain.
    Bearer,
    None,
}

/// A preset or a custom server, ready for the rest of the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connector {
    pub id: String,
    pub name: String,
    /// Shipped with HyperMe rather than added by the user.
    pub builtin: bool,
    /// False until the tool names were checked against the live servers.
    pub tools_verified: bool,
    pub auth: Auth,
    pub servers: Vec<Server>,
    pub tools: ToolLists,
    /// The user's switches from Settings (exact tool names), which win over `tools`.
    pub user_tools: ToolLists,
    pub pulls: Vec<Pull>,
    /// Scopes this connector never asks for, whatever is switched on (R-14).
    pub forbidden_scopes: Vec<String>,
    pub setup: Option<Setup>,
    /// Tool patterns that never run, whatever the user switches (Google: anything that sends
    /// mail, since email is draft-only).
    pub forbidden_tools: Vec<String>,
    /// How each "ask before" tool is previewed (R-23).
    pub actions: Vec<Action>,
}

/// The preview of an "ask before" tool (R-23), from the preset.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub tool: String,
    /// The card's title, such as "New draft".
    pub title: String,
    /// The Enter button's words, such as "Save draft".
    pub enter: String,
    /// The setup switch the tool needs, such as `gmail.drafts`.
    pub needs: Option<String>,
    /// The arguments shown, in order, each with its label: `[["to", "To"], ...]`.
    #[serde(default)]
    pub fields: Vec<[String; 2]>,
}

impl Connector {
    pub fn server(&self, id: &str) -> Option<&Server> {
        self.servers.iter().find(|server| server.id == id)
    }

    /// The access for `tool`: the user's switch if there is one, else the default (R-5).
    /// A tool in `forbidden_tools` is always off, whatever the user switched.
    pub fn access(&self, tool: &str) -> ToolAccess {
        if self.forbidden_tools.iter().any(|p| glob(p, tool)) {
            return ToolAccess::Off;
        }
        self.user_access(tool)
            .unwrap_or_else(|| self.default_access(tool))
    }

    /// The preset's or custom server's default for `tool`.
    pub fn default_access(&self, tool: &str) -> ToolAccess {
        self.tools.access(tool)
    }

    /// The user's switch for `tool`, if they set one. Exact names only; the strictest wins.
    pub fn user_access(&self, tool: &str) -> Option<ToolAccess> {
        let named = |list: &[String]| list.iter().any(|name| name == tool);
        let user = &self.user_tools;
        if named(&user.off) {
            Some(ToolAccess::Off)
        } else if named(&user.ask) {
            Some(ToolAccess::Ask)
        } else if named(&user.read) {
            Some(ToolAccess::Read)
        } else {
            None
        }
    }

    /// The allowlist for a job that asks for `tools`: every one must be a read tool (R-22,
    /// AC-10); otherwise the error names the first that is not.
    pub fn job_allowlist(&self, tools: &[String]) -> Result<Vec<String>, RegistryError> {
        for tool in tools {
            let access = self.access(tool);
            if access != ToolAccess::Read {
                return Err(RegistryError(format!(
                    "{}: a job may use read tools only, and {tool} is {}",
                    self.id,
                    match access {
                        ToolAccess::Ask => "ask before",
                        _ => "off",
                    }
                )));
            }
        }
        Ok(tools.to_vec())
    }
}

/// A preset file, as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PresetFile {
    id: String,
    name: String,
    kind: Kind,
    #[serde(default)]
    tools_verified: bool,
    auth_url: Option<String>,
    token_url: Option<String>,
    revoke_url: Option<String>,
    #[serde(default)]
    extra_auth_params: BTreeMap<String, String>,
    servers: Vec<ServerFile>,
    #[serde(default)]
    tools: ToolLists,
    #[serde(default)]
    pull: Vec<Pull>,
    #[serde(default)]
    forbidden_scopes: Vec<String>,
    setup: Option<Setup>,
    #[serde(default)]
    forbidden_tools: Vec<String>,
    #[serde(default)]
    actions: Vec<Action>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Kind {
    /// Remote MCP servers signed in with OAuth.
    OauthMcp,
    /// Remote MCP servers with no sign-in.
    Mcp,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ServerFile {
    id: String,
    url: String,
    #[serde(default)]
    scopes: Vec<String>,
    #[serde(default)]
    optional_scopes: BTreeMap<String, Vec<String>>,
    label: Option<String>,
    #[serde(default)]
    optional_labels: BTreeMap<String, String>,
    daily_units: Option<u32>,
    #[serde(default)]
    units: BTreeMap<String, u32>,
}

/// How a custom server signs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CustomAuth {
    Oauth,
    Bearer,
    None,
}

/// A custom MCP server from the config's `[[connectors.custom]]` (R-4). A bearer token or
/// client secret is entered in Settings and kept in the keychain; the file never holds one.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CustomServer {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    pub auth: CustomAuth,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revoke_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub extra_auth_params: BTreeMap<String, String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "ToolLists::is_empty")]
    pub tools: ToolLists,
}

impl CustomServer {
    /// Checks the entry on its own; the config calls this when it loads.
    pub(crate) fn check(&self) -> Result<(), String> {
        check_id(&self.id)?;
        if self.name.trim().is_empty() {
            return Err(format!("{}: the name is empty", self.id));
        }
        match (&self.url, &self.command) {
            (Some(url), None) => check_url(url).map_err(|e| format!("{}: {e}", self.id))?,
            (None, Some(command)) if !command.trim().is_empty() => {}
            _ => {
                return Err(format!(
                    "{}: give a url or a command, not both and not neither",
                    self.id
                ));
            }
        }
        if self.auth == CustomAuth::Oauth {
            for (name, value) in [("auth_url", &self.auth_url), ("token_url", &self.token_url)] {
                match value {
                    Some(url) => check_url(url).map_err(|e| format!("{}: {name} {e}", self.id))?,
                    None => return Err(format!("{}: OAuth needs {name}", self.id)),
                }
            }
        }
        Ok(())
    }

    fn connector(&self) -> Connector {
        let transport = match (&self.url, &self.command) {
            (Some(url), _) => Transport::Http(url.clone()),
            (None, command) => Transport::Command {
                program: command.clone().unwrap_or_default(),
                args: self.args.clone(),
            },
        };
        let auth = match self.auth {
            CustomAuth::Oauth => Auth::OAuth(OAuthEndpoints {
                auth_url: self.auth_url.clone().unwrap_or_default(),
                token_url: self.token_url.clone().unwrap_or_default(),
                revoke_url: self.revoke_url.clone(),
                extra_auth_params: pairs(&self.extra_auth_params),
                client_id: self.client_id.clone(),
            }),
            CustomAuth::Bearer => Auth::Bearer,
            CustomAuth::None => Auth::None,
        };
        Connector {
            id: self.id.clone(),
            name: self.name.clone(),
            builtin: false,
            tools_verified: false,
            auth,
            servers: vec![Server {
                id: self.id.clone(),
                transport,
                scopes: self.scopes.clone(),
                optional_scopes: BTreeMap::new(),
                label: None,
                optional_labels: BTreeMap::new(),
                daily_units: None,
                units: BTreeMap::new(),
            }],
            tools: self.tools.clone(),
            user_tools: ToolLists::default(),
            pulls: Vec::new(),
            forbidden_scopes: Vec::new(),
            setup: None,
            forbidden_tools: Vec::new(),
            actions: Vec::new(),
        }
    }
}

fn pairs(map: &BTreeMap<String, String>) -> Vec<(String, String)> {
    map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

/// A connector id: lowercase letters, digits, `-` and `_`, starting with a letter. It names
/// the keychain items and the `hyperme://settings/connectors/<id>` link.
fn check_id(id: &str) -> Result<(), String> {
    let valid = id.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if valid {
        Ok(())
    } else {
        Err(format!(
            "id {id:?} must be lowercase letters, digits, - or _, starting with a letter"
        ))
    }
}

/// `https://`, or plain `http://` only to this Mac (a local server or a test).
fn check_url(url: &str) -> Result<(), String> {
    if url.starts_with("https://") {
        return Ok(());
    }
    if let Some(rest) = url.strip_prefix("http://") {
        let host = if rest.starts_with("[::1]") {
            "[::1]"
        } else {
            rest.split(['/', ':', '?']).next().unwrap_or_default()
        };
        if matches!(host, "127.0.0.1" | "localhost" | "[::1]") {
            return Ok(());
        }
    }
    Err(format!(
        "{url:?} must use https:// (plain http:// only to 127.0.0.1 or localhost)"
    ))
}

/// The first key-like key in a preset, which never holds a secret.
fn secret_key(table: &toml::Table, prefix: &str) -> Option<String> {
    table.iter().find_map(|(name, value)| {
        let key = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}.{name}")
        };
        if looks_like_secret(name) {
            return Some(key);
        }
        match value {
            toml::Value::Table(inner) => secret_key(inner, &key),
            toml::Value::Array(items) => items.iter().find_map(|item| match item {
                toml::Value::Table(inner) => secret_key(inner, &key),
                _ => None,
            }),
            _ => None,
        }
    })
}

/// Parses and checks one preset. `source` names it in errors.
fn parse_preset(text: &str, source: &str, builtin: bool) -> Result<Connector, RegistryError> {
    let fail = |message: String| RegistryError(format!("{source}: {message}"));
    let table: toml::Table = toml::from_str(text).map_err(|e| fail(e.to_string()))?;
    if let Some(key) = secret_key(&table, "") {
        return Err(fail(format!(
            "{key} looks like a secret; a preset never holds one, the keychain does"
        )));
    }
    let file: PresetFile = toml::from_str(text).map_err(|e| fail(e.to_string()))?;
    check_id(&file.id).map_err(fail)?;
    if file.name.trim().is_empty() {
        return Err(fail("the name is empty".to_owned()));
    }
    let auth = match file.kind {
        Kind::OauthMcp => {
            let mut urls = Vec::new();
            for (name, value) in [("auth_url", &file.auth_url), ("token_url", &file.token_url)] {
                let url = value
                    .clone()
                    .ok_or_else(|| fail(format!("an oauth-mcp preset needs {name}")))?;
                check_url(&url).map_err(|e| fail(format!("{name} {e}")))?;
                urls.push(url);
            }
            if let Some(url) = &file.revoke_url {
                check_url(url).map_err(|e| fail(format!("revoke_url {e}")))?;
            }
            let token_url = urls.pop().unwrap_or_default();
            let auth_url = urls.pop().unwrap_or_default();
            Auth::OAuth(OAuthEndpoints {
                auth_url,
                token_url,
                revoke_url: file.revoke_url.clone(),
                extra_auth_params: pairs(&file.extra_auth_params),
                client_id: None,
            })
        }
        Kind::Mcp => Auth::None,
    };
    if file.servers.is_empty() {
        return Err(fail("a preset needs at least one server".to_owned()));
    }
    let mut seen = HashSet::new();
    let mut servers = Vec::new();
    for server in file.servers {
        check_id(&server.id).map_err(|e| fail(format!("server {e}")))?;
        if !seen.insert(server.id.clone()) {
            return Err(fail(format!("server {:?} appears twice", server.id)));
        }
        check_url(&server.url).map_err(|e| fail(format!("server {}: url {e}", server.id)))?;
        let asks = server
            .scopes
            .iter()
            .chain(server.optional_scopes.values().flatten());
        for scope in asks {
            if file.forbidden_scopes.contains(scope) {
                return Err(fail(format!(
                    "server {} asks for {scope}, which forbidden_scopes rules out",
                    server.id
                )));
            }
        }
        servers.push(Server {
            id: server.id,
            transport: Transport::Http(server.url),
            scopes: server.scopes,
            optional_scopes: server.optional_scopes,
            label: server.label,
            optional_labels: server.optional_labels,
            daily_units: server.daily_units,
            units: server.units,
        });
    }
    for pull in &file.pull {
        if !seen.contains(&pull.server) {
            return Err(fail(format!(
                "a pull names server {:?}, which it has not",
                pull.server
            )));
        }
        if let Some(tool) = &pull.body_tool
            && file.tools.access(tool) != ToolAccess::Read
        {
            return Err(fail(format!("body_tool {tool} is not a read tool")));
        }
        if pull.into == "mail" {
            for field in ["id", "labels"] {
                if !pull.fields.contains_key(field) {
                    return Err(fail(format!("a pull into mail needs fields.{field}")));
                }
            }
        }
        if pull.into == "events" {
            for field in ["id", "title", "start", "end"] {
                if !pull.fields.contains_key(field) {
                    return Err(fail(format!("a pull into events needs fields.{field}")));
                }
            }
        }
        if file.tools.access(&pull.tool) != ToolAccess::Read {
            return Err(fail(format!(
                "a pull calls {}, which is not a read tool; pulls may only read",
                pull.tool
            )));
        }
    }
    Ok(Connector {
        id: file.id,
        name: file.name,
        builtin,
        tools_verified: file.tools_verified,
        auth,
        servers,
        tools: file.tools,
        user_tools: ToolLists::default(),
        pulls: file.pull,
        forbidden_scopes: file.forbidden_scopes,
        setup: file.setup,
        forbidden_tools: file.forbidden_tools,
        actions: file.actions,
    })
}

/// Every connector the app knows: built-in presets, preset files, then custom servers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    connectors: Vec<Connector>,
}

impl Registry {
    /// Loads the built-in presets, every `*.toml` in `presets_dir` (by file name), and the
    /// config's custom servers. Any broken entry or repeated id fails the whole load, naming
    /// it, so nothing is half loaded.
    pub fn load(
        presets_dir: Option<&Path>,
        custom: &[CustomServer],
    ) -> Result<Registry, RegistryError> {
        let mut connectors = Vec::new();
        for (name, text) in BUILT_IN {
            connectors.push(parse_preset(text, &format!("built-in {name}"), true)?);
        }
        if let Some(dir) = presets_dir {
            let entries = std::fs::read_dir(dir)
                .map_err(|e| RegistryError(format!("{}: {e}", dir.display())))?;
            let mut files: Vec<_> = entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
                .collect();
            files.sort();
            for path in files {
                let source = path.display().to_string();
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| RegistryError(format!("{source}: {e}")))?;
                connectors.push(parse_preset(&text, &source, false)?);
            }
        }
        for server in custom {
            server
                .check()
                .map_err(|e| RegistryError(format!("connectors.custom: {e}")))?;
            connectors.push(server.connector());
        }
        let mut ids = HashSet::new();
        for connector in &connectors {
            if !ids.insert(connector.id.as_str()) {
                return Err(RegistryError(format!(
                    "the connector id {:?} is used twice; each preset and custom server needs \
                     its own",
                    connector.id
                )));
            }
        }
        Ok(Registry { connectors })
    }

    pub fn connectors(&self) -> &[Connector] {
        &self.connectors
    }

    /// Applies the user's tool switches from the config, by connector id. Switches for a
    /// connector that is not loaded are kept in the file but have no effect.
    pub fn with_user_tools(mut self, switches: &BTreeMap<String, ToolLists>) -> Registry {
        for connector in &mut self.connectors {
            if let Some(lists) = switches.get(&connector.id) {
                connector.user_tools = lists.clone();
            }
        }
        self
    }

    pub fn get(&self, id: &str) -> Option<&Connector> {
        self.connectors.iter().find(|connector| connector.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs_match_whole_names() {
        assert!(glob("trash_*", "trash_thread"));
        assert!(!glob("trash_*", "untrash_thread"));
        assert!(glob("mark_*_spam", "mark_thread_spam"));
        assert!(!glob("mark_*_spam", "mark_thread_spam_x"));
        assert!(glob("*", "anything"));
        assert!(glob("create_label", "create_label"));
        assert!(!glob("create_label", "create_labels"));
        assert!(glob("a*a", "aa"));
        assert!(!glob("a*a", "a"));
    }

    #[test]
    fn urls_need_https_except_to_this_mac() {
        assert!(check_url("https://example.com/mcp").is_ok());
        assert!(check_url("http://127.0.0.1:8080/mcp").is_ok());
        assert!(check_url("http://localhost/mcp").is_ok());
        assert!(check_url("http://[::1]:9/mcp").is_ok());
        assert!(check_url("http://example.com/mcp").is_err());
        assert!(check_url("http://127.0.0.1.example.com/mcp").is_err());
        assert!(check_url("ftp://example.com").is_err());
    }
}
