//! Settings → Connectors (specs/97-google-connectors, slice 5): the commands behind the page.
//! The rows, links and switches come from `hyperme_core::connector_settings`; this file reads
//! the config and the keychain, and saves the `[connectors]` part of the config. Tokens are
//! never sent to the page: only whether one is there.

use std::path::PathBuf;
use std::sync::Arc;

use hyperme_core::config::{Config, Connectors};
use hyperme_core::connector_settings::{
    self as cs, AccountHealth, Choice, CustomForm, Row, ToolRow,
};
use hyperme_core::connector_setup as setup_core;
use hyperme_core::registry::{Auth, Registry, ToolAccess};
use hyperme_core::secrets::SecretStore;
use hyperme_platform::keychain::KeychainStore;
use hyperme_providers::setup::{self, ClientCredentials, SetupError};
use serde::Serialize;
use tauri::AppHandle;
use time::UtcDateTime;

/// The keychain service of connector tokens (design.md, "Signing in").
pub const KEYCHAIN_SERVICE: &str = "ai.hyperme.connector";

/// Preset files the user added, beside the config.
fn presets_dir(app: &AppHandle) -> Option<PathBuf> {
    let dir = crate::runtime::data_dir(app).join("presets");
    dir.is_dir().then_some(dir)
}

/// One account's or custom server's page.
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename = "ConnectorPage")]
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
    /// Signs in with OAuth, so the page offers Sign in again and Disconnect.
    oauth: bool,
    /// The account's label, for Sign in again.
    label: String,
    /// The switches that were on at sign-in.
    features: Vec<String>,
    /// "31 requests today · 310 of 5000 units", per server (R-20).
    requests: Vec<String>,
    /// "Mail · Needs your OK", "Mail · Google held back 12 items", per server waiting for the
    /// user or holding items back (specs/166, R-8, R-15 to R-17).
    approvals: Vec<ApprovalLine>,
}

/// The setup sheet for one OAuth preset.
#[derive(Debug, Serialize, specta::Type)]
pub struct SetupSheet {
    connector: String,
    name: String,
    /// A client ID is in the keychain (the secret is never sent).
    client_saved: bool,
    client_id: Option<String>,
    client_url: Option<String>,
    production_url: Option<String>,
    commands: Vec<String>,
    features: Vec<setup_core::Feature>,
}

/// One server's state on the account page, with the key that resolves it (specs/166, A-3).
#[derive(Debug, Serialize, PartialEq, Eq, specta::Type)]
pub struct ApprovalLine {
    server: String,
    text: String,
    /// "Review" (needs your OK, or items held back) or "Ask again" (not allowed); none for an
    /// admin block, which only the admin can lift.
    action: Option<String>,
}

/// What the Connectors section and its Setup check rows show.
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename = "ConnectorsView")]
pub struct View {
    rows: Vec<Row>,
    setup: Vec<Row>,
    choices: Vec<Choice>,
    pages: Vec<Page>,
    sheets: Vec<SetupSheet>,
    privacy: setup_core::Privacy,
    error: Option<String>,
}

/// What the app keeps about accounts besides the keychain: the "Sign in again" marks and the
/// request counts, both in the app's SQLite file.
pub struct Local<'a> {
    status: &'a dyn hyperme_core::health::AccountStatusStore,
    log: &'a dyn hyperme_core::connectors::RequestLog,
    /// Why a server waits for the user, and what it held back (specs/166).
    servers: &'a dyn hyperme_core::health::ServerStatusStore,
}

/// Works out an account's health: "Sign in again" when its refresh was refused (R-10), else
/// from the keychain. Sync times arrive with the pulls' wiring in the app.
fn health(
    registry: &Registry,
    store: &dyn SecretStore,
    local: &Local<'_>,
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
    if matches!(local.status.sign_in_needed(&key), Ok(true)) {
        return AccountHealth::SignInAgain;
    }
    if !needs_token || matches!(store.get(&key), Ok(Some(_))) {
        AccountHealth::Works { synced: None }
    } else {
        AccountHealth::NotSignedIn
    }
}

/// The account page's lines for one server: its state and what its last pull held back
/// (specs/166, R-8, R-15 to R-17).
fn approval_lines(
    connector: &hyperme_core::registry::Connector,
    server: &hyperme_core::registry::Server,
    local: &Local<'_>,
    account: &str,
) -> Vec<ApprovalLine> {
    use hyperme_core::health::ServerState;
    let status = local
        .servers
        .status(account, &server.id)
        .unwrap_or_default();
    let what = server
        .label
        .as_deref()
        .and_then(|l| l.split(':').next())
        .unwrap_or(&server.id)
        .to_owned();
    let noun = match connector
        .pulls
        .iter()
        .find(|p| p.server == server.id)
        .map(|p| p.into.as_str())
    {
        Some("mail") => "threads",
        Some("events") => "events",
        _ => "items",
    };
    let line = |text: String, action: Option<&str>| ApprovalLine {
        server: server.id.clone(),
        text,
        action: action.map(str::to_owned),
    };
    let mut lines = Vec::new();
    if let Some(state) = status.state {
        let action = match state {
            ServerState::NeedsOk => Some("Review"),
            ServerState::NotAllowed => Some("Ask again"),
            ServerState::Blocked => None,
        };
        lines.push(line(format!("{what} · {}", state.words()), action));
    }
    if status.held_back > 0 {
        let action = status.state.is_none().then_some("Review");
        lines.push(line(
            format!(
                "{what} · {} held back {} {noun}",
                connector.name, status.held_back
            ),
            action,
        ));
    }
    lines
}

fn build_view(
    connectors: &Connectors,
    registry: &Registry,
    store: &dyn SecretStore,
    local: &Local<'_>,
    now: UtcDateTime,
) -> View {
    let health = |connector: &str, label: &str| health(registry, store, local, connector, label);
    let rows = cs::list_rows(registry, connectors, &health, now);
    let pages = rows
        .iter()
        .filter_map(|row| {
            let connector = registry.get(&row.connector)?;
            let entry = connectors.accounts.iter().find(|a| a.id() == row.id);
            Some(Page {
                oauth: matches!(connector.auth, Auth::OAuth(_)),
                label: entry.map(|a| a.label.clone()).unwrap_or_default(),
                features: entry
                    .map(|a| setup_core::features_on(connector, a))
                    .unwrap_or_default(),
                id: row.id.clone(),
                title: row.title.clone(),
                state: row.state.clone(),
                tone: row.tone,
                account: entry.is_some(),
                enabled: entry.is_none_or(|a| a.enabled),
                tools_verified: connector.tools_verified,
                tools: cs::tool_rows(connector),
                approvals: connector
                    .servers
                    .iter()
                    .flat_map(|server| approval_lines(connector, server, local, &row.id))
                    .collect(),
                requests: connector
                    .servers
                    .iter()
                    .filter_map(|server| {
                        let usage = local.log.usage(&row.id, now.date(), &server.id).ok()?;
                        let quota = server
                            .daily_units
                            .map(|daily_units| hyperme_core::health::Quota { daily_units });
                        let label = server.label.as_deref().unwrap_or(&server.id);
                        Some(format!(
                            "{label} · {}",
                            hyperme_core::health::requests_line(usage, quota)
                        ))
                    })
                    .collect(),
            })
        })
        .collect();
    let sheets = registry
        .connectors()
        .iter()
        .filter(|c| matches!(c.auth, Auth::OAuth(_)) && c.setup.is_some())
        .map(|c| {
            let client = setup::load_client(store, &c.id).ok().flatten();
            let help = c.setup.clone().unwrap_or_default();
            SetupSheet {
                connector: c.id.clone(),
                name: c.name.clone(),
                client_saved: client.is_some(),
                client_id: client.map(|c| c.client_id),
                client_url: Some(help.client_url),
                production_url: help.production_url,
                commands: help.commands,
                features: setup_core::features(c),
            }
        })
        .collect();
    View {
        setup: cs::setup_rows(registry, connectors, &health, now),
        choices: cs::add_choices(registry, connectors),
        privacy: setup_core::privacy(registry, connectors),
        sheets,
        rows,
        pages,
        error: None,
    }
}

fn load(app: &AppHandle) -> Result<(Arc<str>, Arc<Config>), String> {
    crate::runtime::config_file(app)
        .load_with_text()
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn connectors_view(app: AppHandle) -> View {
    let empty = |error: String| View {
        rows: Vec::new(),
        setup: Vec::new(),
        choices: Vec::new(),
        pages: Vec::new(),
        sheets: Vec::new(),
        privacy: setup_core::privacy(
            &cs::registry(None, &Connectors::default()).expect("built-in presets load"),
            &Connectors::default(),
        ),
        error: Some(error),
    };
    let (_, config) = match load(&app) {
        Ok(loaded) => loaded,
        Err(error) => return empty(error),
    };
    match cs::registry(presets_dir(&app).as_deref(), &config.connectors) {
        Ok(registry) => match account_data(&app) {
            Ok(data) => build_view(
                &config.connectors,
                &registry,
                &KeychainStore::with_service(KEYCHAIN_SERVICE),
                &Local {
                    status: &data,
                    log: &data,
                    servers: &data,
                },
                UtcDateTime::now(),
            ),
            Err(error) => empty(error),
        },
        Err(error) => empty(error.to_string()),
    }
}

/// Loads the config, lets `change` edit its connectors, and saves the result whole.
fn save(
    app: &AppHandle,
    change: impl FnOnce(&mut Connectors, &Registry) -> Result<(), String>,
) -> Result<(), String> {
    let (_, config) = load(app)?;
    let mut connectors = config.connectors.clone();
    let registry =
        cs::registry(presets_dir(app).as_deref(), &connectors).map_err(|e| e.to_string())?;
    change(&mut connectors, &registry)?;
    crate::runtime::config_file(app)
        .rewrite(|text| cs::write_connectors(text, &connectors))
        .map_err(|e| e.to_string())?;
    // An account added, removed or switched brings or takes its sync jobs (#92).
    crate::tasks::jobs_changed(app);
    Ok(())
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
#[specta::specta]
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
#[specta::specta]
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
#[specta::specta]
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
#[specta::specta]
pub fn connectors_set_token(
    app: AppHandle,
    connector: String,
    token: String,
) -> Result<(), String> {
    let (_, config) = load(&app)?;
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

/// The connector store: the app's SQLite file, which holds the request counts and, with the
/// pulls, the cached events and mail.
fn account_data(app: &AppHandle) -> Result<hyperme_platform::task_store::SqliteTaskStore, String> {
    let path = crate::runtime::data_dir(app).join("hyperme.sqlite3");
    hyperme_platform::task_store::SqliteTaskStore::open(&path).map_err(|e| e.to_string())
}

fn connector_of(app: &AppHandle, id: &str) -> Result<hyperme_core::registry::Connector, String> {
    let (_, config) = load(app)?;
    let registry =
        cs::registry(presets_dir(app).as_deref(), &config.connectors).map_err(|e| e.to_string())?;
    registry
        .get(id)
        .cloned()
        .ok_or_else(|| format!("{id} is not a connector"))
}

/// Step 1 of the sheet: saves the user's client in the keychain and checks it against the
/// provider, without a sign-in. Returns the time the check took in ms.
#[tauri::command]
#[specta::specta]
pub async fn connectors_save_client(
    app: AppHandle,
    connector: String,
    client_id: String,
    client_secret: String,
) -> Result<u64, String> {
    let preset = connector_of(&app, &connector)?;
    let store = KeychainStore::with_service(KEYCHAIN_SERVICE);
    let secret = Some(client_secret).filter(|s| !s.trim().is_empty());
    setup::save_client(
        &store,
        &connector,
        &ClientCredentials {
            client_id,
            client_secret: secret,
        },
    )
    .map_err(|e| e.to_string())?;
    let started = std::time::Instant::now();
    setup::oauth_client(&preset, &store)
        .map_err(|e| e.to_string())?
        .check_client()
        .await
        .map_err(|e| e.to_string())?;
    Ok(started.elapsed().as_millis() as u64)
}

/// What a finished sign-in shows (the sheet's Done step).
#[derive(Debug, Serialize, specta::Type)]
pub struct SignInResult {
    account: String,
    /// One line per server: "Calendar: read your events · 12 tools" or the error.
    checks: Vec<String>,
}

/// Why a sign-in stopped, for the sheet: `testing` shows the fix step.
#[derive(Debug, Serialize, specta::Type)]
pub struct SignInError {
    message: String,
    testing: bool,
    fix_url: Option<String>,
}

fn sign_in_error(error: SetupError) -> SignInError {
    match error {
        SetupError::ClientInTesting { fix_url } => SignInError {
            message: SetupError::ClientInTesting { fix_url: None }.to_string(),
            testing: true,
            fix_url,
        },
        other => SignInError {
            message: other.to_string(),
            testing: false,
            fix_url: None,
        },
    }
}

/// The sign-in waiting in the browser, so Cancel can stop it.
static SIGN_IN: std::sync::Mutex<Option<tauri::async_runtime::JoinHandle<()>>> =
    std::sync::Mutex::new(None);

/// Step 2: signs the account in through the system browser (R-7), with only the switched-on
/// scopes, and adds it to the config. Then calls `tools/list` on each switched-on server.
#[tauri::command]
#[specta::specta]
pub async fn connectors_sign_in(
    app: AppHandle,
    connector: String,
    label: String,
    features: Vec<String>,
) -> Result<SignInResult, SignInError> {
    let fail = |message: String| SignInError {
        message,
        testing: false,
        fix_url: None,
    };
    // The label is settled before the browser opens, so a sign-in is never stored under an
    // empty or unusable label (#216).
    let label = setup_core::account_label(&label).map_err(fail)?;
    let preset = connector_of(&app, &connector).map_err(fail)?;
    let store = std::sync::Arc::new(KeychainStore::with_service(KEYCHAIN_SERVICE));
    let oauth = std::sync::Arc::new(setup::oauth_client(&preset, &*store).map_err(sign_in_error)?);
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let task = {
        let (preset, store, oauth, label, features) = (
            preset.clone(),
            store.clone(),
            oauth.clone(),
            label.clone(),
            features.clone(),
        );
        tauri::async_runtime::spawn(async move {
            let signed = setup::sign_in_account(
                &oauth,
                &preset,
                &hyperme_providers::oauth::SystemBrowser,
                &*store,
                &label,
                &features,
                hyperme_providers::oauth::DEFAULT_LISTEN_TIMEOUT,
            )
            .await;
            let _ = sender.send(signed);
        })
    };
    *SIGN_IN.lock().unwrap_or_else(|p| p.into_inner()) = Some(task);
    let token = match receiver.await {
        Ok(signed) => signed.map_err(sign_in_error)?,
        Err(_) => return Err(fail("Sign-in cancelled".to_owned())),
    };
    let mut account = String::new();
    save(&app, |connectors, _| {
        account = setup_core::add_account(connectors, &connector, &label, &features)?;
        Ok(())
    })
    .map_err(fail)?;
    // A new sign-in clears "Sign in again" (R-10).
    let data = account_data(&app).map_err(fail)?;
    hyperme_core::health::AccountStatusStore::clear(&data, &account).map_err(fail)?;
    // A new sign-in starts the account's servers afresh (specs/166, R-16a).
    hyperme_core::health::ServerStatusStore::clear_account(&data, &account).map_err(fail)?;

    let auth = std::sync::Arc::new(
        hyperme_providers::mcp::AccountAuth::new(oauth, store, &account).with_token(token),
    );
    let mut checks = Vec::new();
    for feature in setup_core::features(&preset) {
        let Some(server) = preset.server(&feature.id) else {
            continue;
        };
        if !features.contains(&feature.id) {
            continue;
        }
        let hyperme_core::registry::Transport::Http(url) = &server.transport else {
            continue;
        };
        // A server with a pull is checked by running it, with the approval card answering
        // what the server asks, so background updates work from the start (specs/166, R-14).
        let tasks_on = {
            use tauri::Manager;
            app.try_state::<crate::tasks::Tasks>().is_some()
        };
        if tasks_on && preset.pulls.iter().any(|p| p.server == server.id) {
            let line = match ui_approver(&app, &account, &server.id) {
                Ok(approver) => crate::tasks::review(&app, &account, &server.id, approver).await,
                Err(e) => Err(e),
            };
            checks.push(line.unwrap_or_else(|e| format!("{} · {e}", feature.label)));
            continue;
        }
        let remote = hyperme_providers::mcp::RemoteServer::new(
            &server.id,
            url,
            auth.clone(),
            std::sync::Arc::new(hyperme_core::connectors::MemoryRequestLog::default()),
            std::sync::Arc::new(|| UtcDateTime::now().date()),
        );
        checks.push(match remote.list_tools().await {
            Ok(tools) => format!("{} · works, {} tools", feature.label, tools.len()),
            Err(error) => format!("{} · {error}", feature.label),
        });
    }
    Ok(SignInResult { account, checks })
}

/// Review, or Ask again, on the account page: runs the server's pulls now, with the approval
/// card answering what the server asks (specs/166, R-8, A-3). Returns the check line.
#[tauri::command]
#[specta::specta]
pub async fn connectors_review(
    app: AppHandle,
    account: String,
    server: String,
) -> Result<String, String> {
    let approver = ui_approver(&app, &account, &server)?;
    crate::tasks::review(&app, &account, &server, approver).await
}

/// The card's approver for one account's server, named the way the card shows it.
fn ui_approver(
    app: &AppHandle,
    account: &str,
    server: &str,
) -> Result<std::sync::Arc<dyn hyperme_core::approval::Approver>, String> {
    let (connector_id, label) = account.split_once(':').unwrap_or((account, ""));
    let connector = connector_of(app, connector_id)?;
    let what = connector
        .server(server)
        .and_then(|s| s.label.as_deref())
        .and_then(|l| l.split(':').next())
        .unwrap_or(server)
        .to_owned();
    Ok(std::sync::Arc::new(crate::approvals::UiApprover {
        app: app.clone(),
        who: format!("{} · {label}", connector.name),
        what,
        read: true,
    }))
}

/// Cancel in the "Finish in your browser" step: stops waiting and closes the listener.
#[tauri::command]
#[specta::specta]
pub fn connectors_cancel_sign_in() {
    if let Some(task) = SIGN_IN.lock().unwrap_or_else(|p| p.into_inner()).take() {
        task.abort();
    }
}

/// Disconnect (R-12): revokes the token at the provider, removes it from the keychain,
/// deletes what HyperMe kept from the account and removes it from the config. When the
/// provider cannot be reached the account is still removed here, and the error says so.
#[tauri::command]
#[specta::specta]
pub async fn connectors_disconnect(app: AppHandle, account: String) -> Result<(), String> {
    let connector = account.split(':').next().unwrap_or_default().to_owned();
    let preset = connector_of(&app, &connector)?;
    let store = KeychainStore::with_service(KEYCHAIN_SERVICE);
    let data = account_data(&app)?;
    let revoked = match setup::oauth_client(&preset, &store) {
        Ok(oauth) => setup::disconnect(&oauth, &store, &data, &account).await,
        Err(_) => {
            // No client any more: nothing to revoke with, so forget the token and the data.
            let _ = store.delete(&account);
            hyperme_core::connectors::AccountData::delete_account(&data, &account)
                .map_err(SetupError::Data)
        }
    };
    save(&app, |connectors, _| {
        setup_core::remove_account(connectors, &account)
    })?;
    revoked.map_err(|e| {
        format!("Removed from HyperMe, but {e}; check your Google account's connected apps")
    })
}

/// Opens a page the setup sheet links to (the client setup or the production setting).
#[tauri::command]
#[specta::specta]
pub fn connectors_open_url(app: AppHandle, connector: String, which: String) -> Result<(), String> {
    let preset = connector_of(&app, &connector)?;
    let help = preset.setup.ok_or("no setup help for this connector")?;
    let url = match which.as_str() {
        "client" => Some(help.client_url),
        "production" => help.production_url,
        _ => None,
    }
    .ok_or_else(|| format!("{which} is not a setup page"))?;
    std::process::Command::new("open")
        .arg(url)
        .status()
        .map(drop)
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
#[specta::specta]
pub fn connectors_open_link(app: AppHandle, url: String) -> Result<(), String> {
    open_link(&app, &url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperme_core::config::DEFAULT_CONFIG;
    use hyperme_core::secrets::MemorySecretStore;

    fn local() -> Local<'static> {
        static STATUS: std::sync::LazyLock<hyperme_core::health::MemoryAccountStatus> =
            std::sync::LazyLock::new(Default::default);
        static LOG: std::sync::LazyLock<hyperme_core::connectors::MemoryRequestLog> =
            std::sync::LazyLock::new(Default::default);
        static SERVERS: std::sync::LazyLock<hyperme_core::health::MemoryServerStatus> =
            std::sync::LazyLock::new(Default::default);
        Local {
            status: &*STATUS,
            log: &*LOG,
            servers: &*SERVERS,
        }
    }

    fn connectors(extra: &str) -> Connectors {
        Config::parse(
            &format!("{DEFAULT_CONFIG}\n{extra}"),
            std::path::Path::new("c.toml"),
        )
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

        let view = build_view(&connectors, &registry, &store, &local(), now);
        assert_eq!(view.rows[0].state, "Not signed in");
        assert_eq!(
            view.setup[1].link,
            "hyperme://settings/connectors/google:personal"
        );

        store.set("google:personal", "fake-refresh-token").unwrap();
        let view = build_view(&connectors, &registry, &store, &local(), now);
        assert_eq!(view.rows[0].state, "Works · not synced yet");
        assert_eq!(view.pages[0].id, "google:personal");
        assert!(view.pages[0].account);
        assert!(view.pages[0].tools_verified);
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
            &local(),
            UtcDateTime::now(),
        );
        let states: Vec<&str> = view.rows.iter().map(|r| r.state.as_str()).collect();
        assert_eq!(states, ["Works · not synced yet", "Not signed in"]);
        assert!(!view.pages[0].account);
    }

    /// G-1, R-24: the Google sheet says whether a client is saved (never the secret), the
    /// account page offers Sign in again and Disconnect, and Privacy lists the account.
    #[test]
    fn the_view_has_the_setup_sheet_and_privacy() {
        let connectors = connectors(
            "[[connectors.accounts]]\nconnector = \"google\"\nlabel = \"work\"\nfeatures = [\"calendar\"]\n",
        );
        let registry = cs::registry(None, &connectors).unwrap();
        let store = MemorySecretStore::default();
        let view = build_view(&connectors, &registry, &store, &local(), UtcDateTime::now());
        assert!(!view.sheets[0].client_saved);

        setup::save_client(
            &store,
            "google",
            &ClientCredentials {
                client_id: "fake-id.apps.example".to_owned(),
                client_secret: Some("fake-client-secret".to_owned()),
            },
        )
        .unwrap();
        let view = build_view(&connectors, &registry, &store, &local(), UtcDateTime::now());
        let sheet = &view.sheets[0];
        assert!(sheet.client_saved);
        assert_eq!(sheet.client_id.as_deref(), Some("fake-id.apps.example"));
        assert_eq!(sheet.features.len(), 3);
        assert!(view.pages[0].oauth);
        assert_eq!(view.pages[0].label, "work");
        assert_eq!(view.pages[0].features, ["calendar"]);
        assert_eq!(
            view.privacy.accounts[0].reads,
            ["Calendar: read your events"]
        );
        // specs/166 AC-17, R-21: one line per account about approvals.
        assert_eq!(
            view.privacy.accounts[0].approvals,
            "Google may ask for your OK before HyperMe reads some of it. Google keeps your \
             choice, not HyperMe."
        );
        let shown = serde_json::to_string(&view).unwrap();
        assert!(!shown.contains("fake-client-secret"), "{shown}");
    }

    /// R-10a: a client in Testing reaches the sheet as its own case, with the fix's page.
    #[test]
    fn a_client_in_testing_shows_the_fix() {
        let error = sign_in_error(SetupError::ClientInTesting {
            fix_url: Some("https://example.test/audience".to_owned()),
        });
        assert!(error.testing);
        assert_eq!(
            error.fix_url.as_deref(),
            Some("https://example.test/audience")
        );
        assert!(!sign_in_error(SetupError::NoClient).testing);
    }

    #[test]
    fn access_names_come_from_the_page() {
        assert_eq!(access("read").unwrap(), ToolAccess::Read);
        assert_eq!(access("ask").unwrap(), ToolAccess::Ask);
        assert_eq!(access("off").unwrap(), ToolAccess::Off);
        assert!(access("on").is_err());
    }

    /// specs/166 AC-3, AC-5, AC-9, AC-14 (page part): each server's state and held-back count
    /// show on the account page.
    #[test]
    fn server_states_show_on_the_page() {
        use hyperme_core::health::{
            MemoryAccountStatus, MemoryServerStatus, PullEnd, ServerState, after_pull,
        };
        let connectors =
            connectors("[[connectors.accounts]]\nconnector = \"google\"\nlabel = \"work\"\n");
        let registry = cs::registry(None, &connectors).unwrap();
        let store = MemorySecretStore::default();
        store.set("google:work", "fake-refresh-token").unwrap();
        let (status, log, servers) = (
            MemoryAccountStatus::default(),
            hyperme_core::connectors::MemoryRequestLog::default(),
            MemoryServerStatus::default(),
        );
        let now = UtcDateTime::now();
        let gmail = PullEnd::NeedsUser(ServerState::NeedsOk);
        after_pull(&servers, "google:work", "gmail", gmail, now.date()).unwrap();
        let calendar = PullEnd::Done { held_back: 12 };
        after_pull(&servers, "google:work", "calendar", calendar, now.date()).unwrap();
        let local = Local {
            status: &status,
            log: &log,
            servers: &servers,
        };
        let view = build_view(&connectors, &registry, &store, &local, now);
        let lines: Vec<(&str, Option<&str>)> = view.pages[0]
            .approvals
            .iter()
            .map(|l| (l.text.as_str(), l.action.as_deref()))
            .collect();
        assert_eq!(
            lines,
            [
                ("Calendar · Google held back 12 events", Some("Review")),
                ("Mail · Needs your OK", Some("Review"))
            ]
        );
        // A-3: "Not allowed" offers Ask again; an admin block offers nothing.
        let deny = PullEnd::NeedsUser(ServerState::NotAllowed);
        after_pull(&servers, "google:work", "gmail", deny, now.date()).unwrap();
        let block = PullEnd::NeedsUser(ServerState::Blocked);
        after_pull(&servers, "google:work", "calendar", block, now.date()).unwrap();
        let view = build_view(&connectors, &registry, &store, &local, now);
        let lines: Vec<(&str, Option<&str>)> = view.pages[0]
            .approvals
            .iter()
            .map(|l| (l.text.as_str(), l.action.as_deref()))
            .collect();
        assert_eq!(
            lines,
            [
                ("Calendar · Blocked by your admin", None),
                // The count from the last pull that worked stays; Review is offered by the state.
                ("Calendar · Google held back 12 events", None),
                ("Mail · Not allowed", Some("Ask again"))
            ]
        );
    }

    /// R-10, R-20: an account whose refresh was refused shows "Sign in again"; its page shows
    /// the day's requests against each server's quota.
    #[test]
    fn sign_in_again_and_requests_show_on_the_page() {
        use hyperme_core::connectors::{MemoryRequestLog, RequestLog};
        use hyperme_core::health::{AccountStatusStore, MemoryAccountStatus};

        let connectors =
            connectors("[[connectors.accounts]]\nconnector = \"google\"\nlabel = \"work\"\n");
        let registry = cs::registry(None, &connectors).unwrap();
        let store = MemorySecretStore::default();
        store.set("google:work", "fake-refresh-token").unwrap();
        let status = MemoryAccountStatus::default();
        let log = MemoryRequestLog::default();
        let now = UtcDateTime::now();
        log.record("google:work", now.date(), "gmail", 10).unwrap();
        status.mark_sign_in_needed("google:work", now).unwrap();
        let servers = hyperme_core::health::MemoryServerStatus::default();
        let local = Local {
            status: &status,
            log: &log,
            servers: &servers,
        };
        let view = build_view(&connectors, &registry, &store, &local, now);
        assert_eq!(view.rows[0].state, "Sign in again");
        assert_eq!(view.rows[0].tone, cs::Tone::Bad);
        assert!(
            view.pages[0]
                .requests
                .contains(&"Mail: read your inbox · 1 request today · 10 of 5000 units".to_owned())
        );
    }
}
