//! A connector account's health between syncs (specs/97-google-connectors, slice 10): the
//! "Sign in again" state with its one alert card (R-10, AC-4), and slowing down near the daily
//! quota (R-20). The scheduler's own retries (#92, R-36) handle every other failure.

use std::collections::BTreeSet;
use std::future::Future;
use std::sync::Mutex;

use time::{Duration, UtcDateTime};

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
