//! Settings → Connectors (specs/97-google-connectors, slice 5): the commands behind the page.
//! The rows, links and switches come from `hyperme_core::connector_settings`; this file reads
//! the config and the keychain, and saves the `[connectors]` part of the config. Tokens are
//! never sent to the page: only whether one is there.

use std::path::{Path, PathBuf};

use hyperme_core::config::{Config, Connectors};
use hyperme_core::connector_settings::{
    self as cs, AccountHealth, Choice, CustomForm, Row, ToolRow,
};
use hyperme_core::registry::{Auth, Registry, ToolAccess};
use hyperme_core::secrets::SecretStore;
use hyperme_platform::keychain::KeychainStore;
use serde::Serialize;
use tauri::AppHandle;
use time::UtcDateTime;

/// The keychain service of connector tokens (design.md, "Signing in").
pub const KEYCHAIN_SERVICE: &str = "ai.hyperme.connector";

fn config_path(app: &AppHandle) -> PathBuf {
    crate::runtime::data_dir(app).join("config.toml")
}

/// Preset files the user added, beside the config.
fn presets_dir(app: &AppHandle) -> Option<PathBuf> {
    let dir = crate::runtime::data_dir(app).join("presets");
    dir.is_dir().then_some(dir)
}

/// One account's or custom server's page.
#[derive(Debug, Serialize)]
pub struct Page {
    id: String,
    title: String,
    state: String,
    tone: cs::Tone,
    /// Accounts can be switched off; custom servers are removed in the config.
    account: bool,
    enabled: bool,
    tools_verified: bool,
    tools: Vec<ToolRow>,
}

/// What the Connectors section and its Setup check rows show.
#[derive(Debug, Serialize)]
pub struct View {
    rows: Vec<Row>,
    setup: Vec<Row>,
    choices: Vec<Choice>,
    pages: Vec<Page>,
    error: Option<String>,
}

/// Works out an account's health from the keychain. Sync times arrive with the pulls
/// (slice 7), and "Sign in again" with the refresh alerts (slice 10).
fn health(
    registry: &Registry,
    store: &dyn SecretStore,
    connector: &str,
    label: &str,
) -> AccountHealth {
    let needs_token = registry
        .get(connector)
        .is_none_or(|c| !matches!(c.auth, Auth::None));
    let key = if label.is_empty() {
        connector.to_owned()
    } else {
        format!("{connector}:{label}")
    };
    if !needs_token || matches!(store.get(&key), Ok(Some(_))) {
        AccountHealth::Works { synced: None }
    } else {
        AccountHealth::NotSignedIn
    }
}

fn build_view(
    connectors: &Connectors,
    registry: &Registry,
    store: &dyn SecretStore,
    now: UtcDateTime,
) -> View {
    let health = |connector: &str, label: &str| health(registry, store, connector, label);
    let rows = cs::list_rows(registry, connectors, &health, now);
    let pages = rows
        .iter()
        .filter_map(|row| {
            let connector = registry.get(&row.connector)?;
            let entry = connectors.accounts.iter().find(|a| a.id() == row.id);
            Some(Page {
                id: row.id.clone(),
                title: row.title.clone(),
                state: row.state.clone(),
                tone: row.tone,
                account: entry.is_some(),
                enabled: entry.is_none_or(|a| a.enabled),
                tools_verified: connector.tools_verified,
                tools: cs::tool_rows(connector),
            })
        })
        .collect();
    View {
        setup: cs::setup_rows(registry, connectors, &health, now),
        choices: cs::add_choices(registry, connectors),
        rows,
        pages,
        error: None,
    }
}

fn load(path: &Path) -> Result<(String, Config), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let config = Config::parse(&text, path).map_err(|e| e.to_string())?;
    Ok((text, config))
}

#[tauri::command]
pub fn connectors_view(app: AppHandle) -> View {
    let empty = |error: String| View {
        rows: Vec::new(),
        setup: Vec::new(),
        choices: Vec::new(),
        pages: Vec::new(),
        error: Some(error),
    };
    let (_, config) = match load(&config_path(&app)) {
        Ok(loaded) => loaded,
        Err(error) => return empty(error),
    };
    match cs::registry(presets_dir(&app).as_deref(), &config.connectors) {
        Ok(registry) => build_view(
            &config.connectors,
            &registry,
            &KeychainStore::with_service(KEYCHAIN_SERVICE),
            UtcDateTime::now(),
        ),
        Err(error) => empty(error.to_string()),
    }
}

/// Loads the config, lets `change` edit its connectors, and saves the result whole.
fn save(
    app: &AppHandle,
    change: impl FnOnce(&mut Connectors, &Registry) -> Result<(), String>,
) -> Result<(), String> {
    let path = config_path(app);
    let (text, mut config) = load(&path)?;
    let registry =
        cs::registry(presets_dir(app).as_deref(), &config.connectors).map_err(|e| e.to_string())?;
    change(&mut config.connectors, &registry)?;
    let updated = cs::write_connectors(&text, &config.connectors).map_err(|e| e.to_string())?;
    // Write beside the file, then rename, so a crash never leaves half a config.
    let temp = path.with_extension("toml.new");
    std::fs::write(&temp, updated).map_err(|e| format!("{}: {e}", temp.display()))?;
    std::fs::rename(&temp, &path).map_err(|e| format!("{}: {e}", path.display()))
}

fn access(text: &str) -> Result<ToolAccess, String> {
    match text {
        "read" => Ok(ToolAccess::Read),
        "ask" => Ok(ToolAccess::Ask),
        "off" => Ok(ToolAccess::Off),
        other => Err(format!("{other:?} is not read, ask or off")),
    }
}

#[tauri::command]
pub fn connectors_set_tool(
    app: AppHandle,
    connector: String,
    tool: String,
    access: String,
) -> Result<(), String> {
    let access = self::access(&access)?;
    save(&app, |connectors, _| {
        cs::set_tool(connectors, &connector, &tool, access)
    })
}

#[tauri::command]
pub fn connectors_set_enabled(
    app: AppHandle,
    account: String,
    enabled: bool,
) -> Result<(), String> {
    save(&app, |connectors, _| {
        cs::set_enabled(connectors, &account, enabled)
    })
}

/// Adds a custom server and returns its id; its bearer token, if any, is entered next.
#[tauri::command]
pub fn connectors_add_custom(app: AppHandle, form: CustomForm) -> Result<String, String> {
    let mut id = String::new();
    save(&app, |connectors, registry| {
        id = cs::add_custom(connectors, registry, &form)?;
        Ok(())
    })?;
    Ok(id)
}

/// A custom server's bearer token goes straight to the keychain; it is never sent back.
#[tauri::command]
pub fn connectors_set_token(
    app: AppHandle,
    connector: String,
    token: String,
) -> Result<(), String> {
    let (_, config) = load(&config_path(&app))?;
    let bearer = config.connectors.custom.iter().any(|server| {
        server.id == connector && server.auth == hyperme_core::registry::CustomAuth::Bearer
    });
    if !bearer {
        return Err(format!("{connector} does not take a bearer token"));
    }
    KeychainStore::with_service(KEYCHAIN_SERVICE)
        .set(&connector, &token)
        .map_err(|e| e.to_string())
}

/// Opens a `hyperme://settings/...` link, such as an expired account's alert or a Setup check
/// row (UX rule 5, R-6). Anything else is refused.
pub fn open_link(app: &AppHandle, url: &str) -> Result<(), String> {
    let link =
        cs::parse_link(url).ok_or_else(|| format!("{url} is not a HyperMe settings link"))?;
    crate::settings::open(app, &link.hash());
    Ok(())
}

#[tauri::command]
pub fn connectors_open_link(app: AppHandle, url: String) -> Result<(), String> {
    open_link(&app, &url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperme_core::config::DEFAULT_CONFIG;
    use hyperme_core::secrets::MemorySecretStore;

    fn connectors(extra: &str) -> Connectors {
        Config::parse(&format!("{DEFAULT_CONFIG}\n{extra}"), Path::new("c.toml"))
            .unwrap()
            .connectors
    }

    /// AC-13, R-9: an account without a token shows "Not signed in" and links to its row; with
    /// its token in the keychain it works. The view never holds the token.
    #[test]
    fn the_view_reads_tokens_only_to_say_they_are_there() {
        let connectors =
            connectors("[[connectors.accounts]]\nconnector = \"google\"\nlabel = \"personal\"\n");
        let registry = cs::registry(None, &connectors).unwrap();
        let store = MemorySecretStore::default();
        let now = UtcDateTime::now();

        let view = build_view(&connectors, &registry, &store, now);
        assert_eq!(view.rows[0].state, "Not signed in");
        assert_eq!(
            view.setup[1].link,
            "hyperme://settings/connectors/google:personal"
        );

        store.set("google:personal", "fake-refresh-token").unwrap();
        let view = build_view(&connectors, &registry, &store, now);
        assert_eq!(view.rows[0].state, "Works · not synced yet");
        assert_eq!(view.pages[0].id, "google:personal");
        assert!(view.pages[0].account);
        assert!(!view.pages[0].tools_verified);
        let shown = serde_json::to_string(&view).unwrap();
        assert!(!shown.contains("fake-refresh-token"), "{shown}");
    }

    /// R-4: a custom server with no sign-in works without a token; one with a bearer token
    /// needs it.
    #[test]
    fn custom_servers_need_a_token_only_with_bearer() {
        let connectors = connectors(
            "[[connectors.custom]]\nid = \"open\"\nname = \"Open\"\nurl = \"https://o.example/mcp\"\nauth = \"none\"\n\n[[connectors.custom]]\nid = \"wiki\"\nname = \"Wiki\"\nurl = \"https://w.example/mcp\"\nauth = \"bearer\"\n",
        );
        let registry = cs::registry(None, &connectors).unwrap();
        let view = build_view(
            &connectors,
            &registry,
            &MemorySecretStore::default(),
            UtcDateTime::now(),
        );
        let states: Vec<&str> = view.rows.iter().map(|r| r.state.as_str()).collect();
        assert_eq!(states, ["Works · not synced yet", "Not signed in"]);
        assert!(!view.pages[0].account);
    }

    #[test]
    fn access_names_come_from_the_page() {
        assert_eq!(access("read").unwrap(), ToolAccess::Read);
        assert_eq!(access("ask").unwrap(), ToolAccess::Ask);
        assert_eq!(access("off").unwrap(), ToolAccess::Off);
        assert!(access("on").is_err());
    }
}
