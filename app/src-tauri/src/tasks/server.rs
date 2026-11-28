//! Sync with connector servers: pulling events and mail through MCP, reviewing a server's tools,
//! and the alerts shown when a pull fails or needs the user's OK.

use super::*;

/// The "Job failed" alert for a job (R-36, docs/UX.md "Alerts"): what could not be reached,
/// and Fix opening that account's page in Settings → Connectors, or the connector's when it
/// has several accounts.
pub fn failed_alert(
    job: &str,
    registry: &hyperme_core::registry::Registry,
    connectors: &hyperme_core::config::Connectors,
) -> crate::pill::AlertView {
    use hyperme_core::connector_settings::link_for;
    let found = job.strip_prefix("sync:").and_then(|id| {
        registry.connectors().iter().find_map(|c| {
            c.servers
                .iter()
                .find(|s| format!("{}-{}", c.id, s.id) == id)
                .map(|s| (c, s))
        })
    });
    let Some((connector, server)) = found else {
        return crate::pill::AlertView {
            title: "Job failed".to_owned(),
            detail: format!("{job} failed twice today"),
            action: "Fix".to_owned(),
            link: None,
        };
    };
    let what = server
        .label
        .as_deref()
        .and_then(|l| l.split(':').next())
        .unwrap_or(&server.id)
        .to_lowercase();
    let accounts: Vec<String> = connectors
        .accounts
        .iter()
        .filter(|a| a.connector == connector.id && a.enabled)
        .map(|a| a.id())
        .collect();
    let link = match accounts.as_slice() {
        [one] => link_for(one),
        _ => link_for(&connector.id),
    };
    crate::pill::AlertView {
        title: "Job failed".to_owned(),
        detail: format!("Couldn't reach your {} {what} twice today", connector.name),
        action: "Fix".to_owned(),
        link: Some(link),
    }
}

/// The local day of `now` on this Mac.
fn local_day(now: UtcDateTime) -> time::Date {
    use hyperme_platform::presence::MacClock;
    now.to_offset(hyperme_core::schedule::Zone::offset_at(&MacClock, now))
        .date()
}

/// The account servers whose pulls the user is running now (Review, the setup check).
static CHECKING: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashSet<(String, String)>>,
> = std::sync::LazyLock::new(Default::default);

/// Marks one account's server as being checked, until dropped.
pub(super) struct Checking((String, String));

impl Checking {
    pub(super) fn start(account: &str, server: &str) -> Self {
        let key = (account.to_owned(), server.to_owned());
        CHECKING
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(key.clone());
        Checking(key)
    }

    pub(super) fn running(account: &str, server: &str) -> bool {
        CHECKING
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains(&(account.to_owned(), server.to_owned()))
    }
}

impl Drop for Checking {
    fn drop(&mut self) {
        CHECKING
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&self.0);
    }
}

/// One account's client for one remote server, with its keychain sign-in and request counter;
/// `None` for a server that is not remote.
fn remote_server(
    connector: &hyperme_core::registry::Connector,
    server: &hyperme_core::registry::Server,
    account: &str,
    store: &Arc<SqliteTaskStore>,
) -> Result<Option<hyperme_providers::mcp::RemoteServer>, String> {
    use hyperme_platform::keychain::KeychainStore;
    use hyperme_providers::mcp::{AccountAuth, RemoteServer};
    let hyperme_core::registry::Transport::Http(url) = &server.transport else {
        return Ok(None);
    };
    let keychain: Arc<dyn hyperme_core::secrets::SecretStore + Send + Sync> = Arc::new(
        KeychainStore::with_service(crate::connectors::KEYCHAIN_SERVICE),
    );
    let oauth = Arc::new(
        hyperme_providers::setup::oauth_client(connector, &*keychain).map_err(|e| e.to_string())?,
    );
    let auth = Arc::new(AccountAuth::new(oauth, keychain, account));
    Ok(Some(
        RemoteServer::new(
            &server.id,
            url,
            auth,
            store.clone(),
            Arc::new(|| local_day(UtcDateTime::now())),
        )
        .with_units(server.units.clone()),
    ))
}

/// What one server's pulls did for one account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerPull {
    /// How they ended together (specs/166, R-16a).
    pub end: hyperme_core::health::PullEnd,
    /// Failures for the job-failed alert: never a state that waits for the user (R-16b).
    pub failures: Vec<String>,
    /// Events or threads kept.
    pub items: usize,
}

/// Runs every pull of `server` for `account` through `caller`, keeping what they bring (G-5).
/// A refused sign-in marks the account (spec 97, R-10).
pub async fn pull_server<C: hyperme_core::sync::ToolCaller>(
    caller: &C,
    store: &Arc<SqliteTaskStore>,
    connector: &hyperme_core::registry::Connector,
    server: &hyperme_core::registry::Server,
    account: &str,
    now: UtcDateTime,
) -> ServerPull {
    use hyperme_core::health::{AccountStatusStore, PullEnd};
    use hyperme_platform::presence::MacClock;
    let mut pulled = ServerPull {
        end: PullEnd::Done { held_back: 0 },
        failures: Vec::new(),
        items: 0,
    };
    for pull in connector.pulls.iter().filter(|p| p.server == server.id) {
        let result = match pull.into.as_str() {
            "events" => {
                sync::pull_events(caller, &**store, &**store, account, pull, now, &MacClock)
                    .await
                    .map(|report| (report.events, 0))
            }
            "mail" => sync::pull_mail(caller, &**store, &**store, account, pull, now, &MacClock)
                .await
                .map(|report| {
                    let held = u32::try_from(report.held_back).unwrap_or(u32::MAX);
                    (report.threads, held)
                }),
            _ => Ok((0, 0)),
        };
        if let Err(sync::SyncError::SignInAgain) = &result {
            let _ = store.mark_sign_in_needed(account, now);
            pulled
                .failures
                .push(format!("{account} needs a new sign-in"));
        } else if let Err(e @ sync::SyncError::Failed(_)) = &result {
            pulled.failures.push(format!("{account}: {e}"));
        }
        // "Needs your OK" and the like wait for the user (specs/166): never a failure for the
        // job-failed alert (R-16b); the account page shows the state.
        pulled.end = pulled.end.and(match result {
            Ok((items, held_back)) => {
                pulled.items += items;
                PullEnd::Done { held_back }
            }
            Err(sync::SyncError::NeedsUser(state)) => PullEnd::NeedsUser(state),
            Err(_) => PullEnd::Failed,
        });
    }
    pulled
}

/// Runs one account's server pulls now with `approver` answering what the server asks: the
/// setup sheet's check, and Review or Ask again on the account page (specs/166, R-8, R-14,
/// A-3). It runs whatever the server's state, and records how it ended; returns the check line.
pub async fn review(
    app: &tauri::AppHandle,
    account: &str,
    server_id: &str,
    approver: Arc<dyn hyperme_core::approval::Approver>,
) -> Result<String, String> {
    use hyperme_core::health::after_review;
    use hyperme_providers::mcp::McpCaller;
    use tauri::Manager;
    let store = app
        .try_state::<Tasks>()
        .map(|t| t.store.clone())
        .ok_or("daily tasks are off")?;
    let config = config(app)?;
    let registry = registry(app, &config)?;
    let entry = config
        .connectors
        .accounts
        .iter()
        .find(|a| a.id() == account)
        .ok_or_else(|| format!("no account {account}"))?;
    let connector = registry
        .get(&entry.connector)
        .ok_or_else(|| format!("no connector {}", entry.connector))?;
    let server = connector
        .server(server_id)
        .ok_or_else(|| format!("no server {server_id}"))?;
    let Some(remote) = remote_server(connector, server, account, &store)? else {
        return Err(format!("{server_id} is not a remote server"));
    };
    let allowlist: Vec<String> = connector
        .pulls
        .iter()
        .filter(|p| p.server == server.id)
        .map(|p| p.tool.clone())
        .collect();
    let caller = McpCaller::new(vec![remote], allowlist).with_approver(approver);
    let now = UtcDateTime::now();
    // While the user answers, the scheduler leaves this server alone, so it cannot raise
    // "Needs your OK" over the card or overwrite the outcome (specs/166, R-14).
    let _checking = Checking::start(account, &server.id);
    let pulled = pull_server(&caller, &store, connector, server, account, now).await;
    // A Review never raises the alert: the user is already looking.
    after_review(&*store, account, &server.id, pulled.end, local_day(now))?;
    Ok(check_line(connector, server, &pulled))
}

/// The line a check or Review shows for one server (specs/166, AC-13).
pub fn check_line(
    connector: &hyperme_core::registry::Connector,
    server: &hyperme_core::registry::Server,
    pulled: &ServerPull,
) -> String {
    use hyperme_core::health::{PullEnd, ServerState};
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
    let lower = what.to_lowercase();
    match pulled.end {
        PullEnd::Done { .. } => format!("{what} · works, {} {noun}", pulled.items),
        PullEnd::NeedsUser(ServerState::NeedsOk) => {
            format!("{what} · not approved yet: {lower} pulls wait for your OK")
        }
        PullEnd::NeedsUser(ServerState::NotAllowed) => {
            format!("{what} · not allowed: {lower} pulls are skipped")
        }
        PullEnd::NeedsUser(ServerState::Blocked) => format!("{what} · blocked by your admin"),
        PullEnd::Failed => format!(
            "{what} · {}",
            pulled
                .failures
                .first()
                .map_or("could not be reached", String::as_str)
        ),
    }
}

/// The "Needs your OK" alert (specs/166, R-8, docs/UX.md "Alerts"): which account's server
/// waits for an approval, and Review opening that account's page in Settings → Connectors.
pub fn needs_ok_alert(
    connector: &hyperme_core::registry::Connector,
    server: &hyperme_core::registry::Server,
    label: &str,
    account: &str,
) -> crate::pill::AlertView {
    let what = server
        .label
        .as_deref()
        .and_then(|l| l.split(':').next())
        .unwrap_or(&server.id)
        .to_lowercase();
    crate::pill::AlertView {
        title: "Needs your OK".to_owned(),
        detail: format!(
            "{} {what} wants your OK before HyperMe reads it for {label}",
            connector.name
        ),
        action: "Review".to_owned(),
        link: Some(hyperme_core::connector_settings::link_for(account)),
    }
}

/// The pulls through each account's remote MCP servers, with the job's allowlist (G-5, R-22).
pub(super) struct McpSyncer {
    pub(super) app: tauri::AppHandle,
    pub(super) store: Arc<SqliteTaskStore>,
}

impl Syncer for McpSyncer {
    async fn sync(&self, id: &str, allowlist: &[String], now: UtcDateTime) -> Result<(), String> {
        use hyperme_core::health::{AccountStatusStore, ServerStatusStore, after_pull, pull_runs};
        use hyperme_providers::mcp::McpCaller;

        let config = config(&self.app)?;
        let registry = registry(&self.app, &config)?;
        let Some((connector, server)) = registry.connectors().iter().find_map(|c| {
            c.servers
                .iter()
                .find(|s| format!("{}-{}", c.id, s.id) == id)
                .map(|s| (c, s))
        }) else {
            return Ok(()); // The connector went away; the next rebuild drops the job.
        };
        let today = local_day(now);
        let mut failures = Vec::new();
        for entry in config.connectors.accounts.iter().filter(|a| {
            a.enabled
                && a.connector == connector.id
                && hyperme_core::connector_setup::features_on(connector, a).contains(&server.id)
        }) {
            let account = entry.id();
            if self.store.sign_in_needed(&account).unwrap_or(false) {
                continue; // Settings shows "Sign in again" (specs/97, R-10).
            }
            if Checking::running(&account, &server.id) {
                continue; // The user is answering its approval card now (specs/166).
            }
            // "Not allowed" and "Blocked" wait for the user; "Needs your OK" is tried once a
            // day (specs/166, R-8, R-15, R-16).
            let status = self.store.status(&account, &server.id).unwrap_or_default();
            if !pull_runs(&status, today) {
                continue;
            }
            let Some(remote) = remote_server(connector, server, &account, &self.store)? else {
                return Ok(());
            };
            let caller = McpCaller::new(vec![remote], allowlist.to_vec());
            let pulled = pull_server(&caller, &self.store, connector, server, &account, now).await;
            failures.extend(pulled.failures);
            match after_pull(&*self.store, &account, &server.id, pulled.end, today) {
                Ok(true) => crate::runtime::show_card(
                    &self.app,
                    crate::pill::Card::Alert(needs_ok_alert(
                        connector,
                        server,
                        &entry.label,
                        &account,
                    )),
                    false,
                ),
                Ok(false) => {}
                Err(e) => tracing::warn!("server state: {e}"),
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("; "))
        }
    }

    fn feeds(&self) -> Feeds {
        let mut feeds = Feeds::default();
        let Ok(config) = config(&self.app) else {
            return feeds;
        };
        let Ok(registry) = registry(&self.app, &config) else {
            return feeds;
        };
        for job in sync::sync_jobs(&registry, &config.connectors) {
            let JobKind::Sync(id) = &job.kind else {
                continue;
            };
            for c in registry.connectors() {
                for pull in &c.pulls {
                    if format!("{}-{}", c.id, pull.server) != *id {
                        continue;
                    }
                    let list = match pull.into.as_str() {
                        "events" => &mut feeds.events,
                        "mail" => &mut feeds.mail,
                        _ => continue,
                    };
                    if !list.contains(&job.name) {
                        list.push(job.name.clone());
                    }
                }
            }
        }
        feeds
    }
}
