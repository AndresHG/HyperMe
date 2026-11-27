//! A connector account's health between syncs (specs/97-google-connectors, slice 10): the
//! "Sign in again" state with its one alert card (R-10, AC-4), and slowing down near the daily
//! quota (R-20). The scheduler's own retries (#92, R-36) handle every other failure.

use std::collections::BTreeSet;
use std::future::Future;
use std::sync::Mutex;

use time::{Date, Duration, UtcDateTime};

use crate::connector_settings::link_for;
use crate::connectors::Usage;
use crate::sync::SyncError;

/// Pulls slow down from this share of the day's quota (R-20)...
const SLOW_DOWN_AT_PERCENT: u64 = 80;
/// ...to at most one in this long.
const SLOW_EVERY: Duration = Duration::hours(2);

/// Which accounts need a new sign-in. SQLite keeps it in the app; tests use memory.
pub trait AccountStatusStore: Send + Sync {
    fn sign_in_needed(&self, account: &str) -> Result<bool, String>;
    /// Marks the account; `true` when it was not marked before, so the alert is raised once.
    fn mark_sign_in_needed(&self, account: &str, at: UtcDateTime) -> Result<bool, String>;
    /// Clears the mark after a new sign-in.
    fn clear(&self, account: &str) -> Result<(), String>;
}

/// An in-memory [`AccountStatusStore`] for tests.
#[derive(Debug, Default)]
pub struct MemoryAccountStatus {
    marked: Mutex<BTreeSet<String>>,
}

impl AccountStatusStore for MemoryAccountStatus {
    fn sign_in_needed(&self, account: &str) -> Result<bool, String> {
        Ok(self
            .marked
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains(account))
    }

    fn mark_sign_in_needed(&self, account: &str, _at: UtcDateTime) -> Result<bool, String> {
        Ok(self
            .marked
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(account.to_owned()))
    }

    fn clear(&self, account: &str) -> Result<(), String> {
        self.marked
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(account);
        Ok(())
    }
}

/// The one alert card for an expired sign-in; it opens the account's row (R-6, AC-13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    pub account: String,
    pub title: String,
    pub link: String,
}

/// How a guarded sync went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncOutcome<R> {
    Done(R),
    /// The sign-in just expired: show this alert, once.
    Alert(Alert),
    /// The account is waiting for a new sign-in; nothing was tried (R-10).
    SignInNeeded,
    /// The server waits for the user (specs/166); not a failure (R-16b).
    NeedsUser(ServerState),
    Failed(String),
}

/// Runs one account's sync unless it waits for a new sign-in. A refused refresh token marks the
/// account and returns the alert the first time only; until the user signs in again the sync
/// is not tried (R-10, AC-4).
pub async fn guarded_sync<R, F, Fut>(
    status: &dyn AccountStatusStore,
    account: &str,
    title: &str,
    now: UtcDateTime,
    run: F,
) -> SyncOutcome<R>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<R, SyncError>>,
{
    match status.sign_in_needed(account) {
        Ok(true) => return SyncOutcome::SignInNeeded,
        Ok(false) => {}
        Err(error) => return SyncOutcome::Failed(error),
    }
    match run().await {
        Ok(result) => SyncOutcome::Done(result),
        Err(SyncError::SignInAgain) => match status.mark_sign_in_needed(account, now) {
            Ok(true) => SyncOutcome::Alert(Alert {
                account: account.to_owned(),
                title: format!("{title} needs you to sign in again"),
                link: link_for(account),
            }),
            Ok(false) => SyncOutcome::SignInNeeded,
            Err(error) => SyncOutcome::Failed(error),
        },
        Err(SyncError::NeedsUser(state)) => SyncOutcome::NeedsUser(state),
        Err(SyncError::Failed(reason)) => SyncOutcome::Failed(reason),
    }
}

/// A server's daily quota (R-20).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quota {
    pub daily_units: u32,
}

/// Whether a scheduled pull runs now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaDecision {
    Run,
    /// Over 80%: skip this one; the next runs once 2 hours have passed since the last.
    SlowDown,
    /// At the quota: no more pulls today.
    Stop,
}

/// R-20: a pull runs freely under 80% of the day's quota, at most every 2 hours from 80%, and
/// not at all at the quota. With no quota known it runs.
pub fn quota_allows(
    usage: Usage,
    quota: Option<Quota>,
    last_pull: Option<UtcDateTime>,
    now: UtcDateTime,
) -> QuotaDecision {
    let Some(quota) = quota else {
        return QuotaDecision::Run;
    };
    let used = u64::from(usage.units);
    let daily = u64::from(quota.daily_units);
    if used >= daily {
        QuotaDecision::Stop
    } else if used * 100 >= daily * SLOW_DOWN_AT_PERCENT
        && last_pull.is_some_and(|at| now - at < SLOW_EVERY)
    {
        QuotaDecision::SlowDown
    } else {
        QuotaDecision::Run
    }
}

/// The account page's line (R-20): "31 requests today · 310 of 5000 units".
pub fn requests_line(usage: Usage, quota: Option<Quota>) -> String {
    let requests = if usage.calls == 1 {
        "request"
    } else {
        "requests"
    };
    let mut line = format!("{} {requests} today", usage.calls);
    if let Some(quota) = quota {
        line.push_str(&format!(
            " · {} of {} units",
            usage.units, quota.daily_units
        ));
    }
    line
}

/// Why a connector server waits for the user (specs/166-gmail-approvals, R-8, R-15, R-16).
/// The order is the precedence: a later state replaces an earlier one, never the reverse
/// (R-16a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ServerState {
    /// The server asked for an approval and nobody answered: "Needs your OK".
    NeedsOk,
    /// The user refused: "Not allowed".
    NotAllowed,
    /// The server's admin policy blocks it: "Blocked by your admin".
    Blocked,
}

impl ServerState {
    /// The words the account page shows (R-8, R-15, R-16).
    pub fn words(self) -> &'static str {
        match self {
            ServerState::NeedsOk => "Needs your OK",
            ServerState::NotAllowed => "Not allowed",
            ServerState::Blocked => "Blocked by your admin",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ServerState::NeedsOk => "needs_ok",
            ServerState::NotAllowed => "not_allowed",
            ServerState::Blocked => "blocked",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "needs_ok" => Some(ServerState::NeedsOk),
            "not_allowed" => Some(ServerState::NotAllowed),
            "blocked" => Some(ServerState::Blocked),
            _ => None,
        }
    }
}

/// What the app keeps about one account's server between pulls (specs/166).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerStatus {
    pub state: Option<ServerState>,
    /// The local day of the last unattended pull tried while "Needs your OK" (R-8).
    pub last_try: Option<Date>,
    /// The local day of the last "Needs your OK" alert (R-8).
    pub last_alert: Option<Date>,
    /// Items the last pull's answer listed as held back (`omittedContent`, R-17).
    pub held_back: u32,
}

/// The server states, per account and server. SQLite keeps it in the app; tests use memory.
pub trait ServerStatusStore: Send + Sync {
    fn status(&self, account: &str, server: &str) -> Result<ServerStatus, String>;
    fn save(&self, account: &str, server: &str, status: &ServerStatus) -> Result<(), String>;
    /// Forgets every server of the account: a new sign-in or a disconnect (R-16a).
    fn clear_account(&self, account: &str) -> Result<(), String>;
}

/// An in-memory [`ServerStatusStore`] for tests.
#[derive(Debug, Default)]
pub struct MemoryServerStatus {
    rows: Mutex<std::collections::BTreeMap<(String, String), ServerStatus>>,
}

impl ServerStatusStore for MemoryServerStatus {
    fn status(&self, account: &str, server: &str) -> Result<ServerStatus, String> {
        let rows = self.rows.lock().unwrap_or_else(|p| p.into_inner());
        Ok(rows
            .get(&(account.to_owned(), server.to_owned()))
            .cloned()
            .unwrap_or_default())
    }

    fn save(&self, account: &str, server: &str, status: &ServerStatus) -> Result<(), String> {
        let mut rows = self.rows.lock().unwrap_or_else(|p| p.into_inner());
        rows.insert((account.to_owned(), server.to_owned()), status.clone());
        Ok(())
    }

    fn clear_account(&self, account: &str) -> Result<(), String> {
        let mut rows = self.rows.lock().unwrap_or_else(|p| p.into_inner());
        rows.retain(|(a, _), _| a != account);
        Ok(())
    }
}

/// Whether an unattended pull of this server runs `today` (R-8, R-15, R-16): never while "Not
/// allowed" or "Blocked by your admin" (until Review, Ask again or a new sign-in); once a day
/// while "Needs your OK", since only the user can change it; otherwise always.
pub fn pull_runs(status: &ServerStatus, today: Date) -> bool {
    match status.state {
        None => true,
        Some(ServerState::NeedsOk) => status.last_try != Some(today),
        Some(ServerState::NotAllowed | ServerState::Blocked) => false,
    }
}

/// How an unattended pull of one server ended, for [`after_pull`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullEnd {
    /// The pull worked; it held back this many items (R-17).
    Done { held_back: u32 },
    /// The server waits for the user.
    NeedsUser(ServerState),
    /// Any other failure: the state is left as it is.
    Failed,
}

/// Records how a pull ended and says whether the "Needs your OK" alert is due: a pull that
/// works clears the state (R-16a); a higher state replaces a lower one; the alert comes at most
/// once a day per account and server (R-8), and never for "Not allowed" or "Blocked" (R-15,
/// R-16).
pub fn after_pull(
    store: &dyn ServerStatusStore,
    account: &str,
    server: &str,
    end: PullEnd,
    today: Date,
) -> Result<bool, String> {
    let mut status = store.status(account, server)?;
    let alert = match end {
        PullEnd::Done { held_back } => {
            status = ServerStatus {
                held_back,
                ..ServerStatus::default()
            };
            false
        }
        PullEnd::NeedsUser(state) => {
            if status.state.is_none_or(|old| state >= old) {
                status.state = Some(state);
            }
            status.last_try = Some(today);
            let due =
                status.state == Some(ServerState::NeedsOk) && status.last_alert != Some(today);
            if due {
                status.last_alert = Some(today);
            }
            due
        }
        PullEnd::Failed => return Ok(false),
    };
    store.save(account, server, &status)?;
    Ok(alert)
}
