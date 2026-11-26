//! Connector bookkeeping that is not network code: the request count per account, day and
//! server (specs/97-google-connectors, R-20). The SQLite table `connector_requests` implements
//! [`RequestLog`] in `hyperme-platform`; [`MemoryRequestLog`] stands in for tests.

use std::collections::BTreeMap;
use std::sync::Mutex;

use time::Date;

/// Calls and quota units for one account, day and server.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub calls: u32,
    pub units: u32,
}

/// Counts the requests sent to each connector server, so the account page can show them and
/// syncing can slow down near the daily quota (R-20).
pub trait RequestLog: Send + Sync {
    /// Adds one call costing `units` to `account`'s count for `day` on `server`.
    fn record(&self, account: &str, day: Date, server: &str, units: u32) -> Result<(), String>;
    /// The count so far for `account`, `day` and `server`; zero when there is none.
    fn usage(&self, account: &str, day: Date, server: &str) -> Result<Usage, String>;
}

/// An in-memory [`RequestLog`] for tests.
#[derive(Debug, Default)]
pub struct MemoryRequestLog {
    counts: Mutex<BTreeMap<(String, Date, String), Usage>>,
}

impl RequestLog for MemoryRequestLog {
    fn record(&self, account: &str, day: Date, server: &str, units: u32) -> Result<(), String> {
        let mut counts = self.counts.lock().unwrap_or_else(|p| p.into_inner());
        let usage = counts
            .entry((account.to_owned(), day, server.to_owned()))
            .or_default();
        usage.calls += 1;
        usage.units += units;
        Ok(())
    }

    fn usage(&self, account: &str, day: Date, server: &str) -> Result<Usage, String> {
        let counts = self.counts.lock().unwrap_or_else(|p| p.into_inner());
        Ok(counts
            .get(&(account.to_owned(), day, server.to_owned()))
            .copied()
            .unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    /// R-20: counts are kept apart by account, day and server.
    #[test]
    fn counts_are_per_account_day_and_server() {
        let log = MemoryRequestLog::default();
        let day = Date::from_calendar_date(2026, Month::November, 27).unwrap();
        let next = day.next_day().unwrap();
        log.record("google:personal", day, "gmail", 10).unwrap();
        log.record("google:personal", day, "gmail", 1).unwrap();
        log.record("google:personal", next, "gmail", 1).unwrap();
        log.record("google:work", day, "gmail", 1).unwrap();
        log.record("google:personal", day, "calendar", 1).unwrap();
        assert_eq!(
            log.usage("google:personal", day, "gmail").unwrap(),
            Usage {
                calls: 2,
                units: 11
            }
        );
        assert_eq!(
            log.usage("google:personal", next, "gmail").unwrap().calls,
            1
        );
        assert_eq!(log.usage("google:work", day, "gmail").unwrap().calls, 1);
        assert_eq!(
            log.usage("google:other", day, "gmail").unwrap(),
            Usage::default()
        );
    }
}
