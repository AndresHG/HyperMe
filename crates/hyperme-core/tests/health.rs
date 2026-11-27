//! Slice 10 of specs/97-google-connectors: an expired sign-in raises one alert and stops the
//! account's syncs until the user signs in again (R-10, AC-4); pulls slow down at 80% of the
//! daily quota (R-20). Tests cite `specs/97-google-connectors/spec.md`.

use std::sync::atomic::{AtomicUsize, Ordering};

use hyperme_core::config::Connectors;
use hyperme_core::connector_settings as cs;
use hyperme_core::connectors::Usage;
use hyperme_core::health::{
    self, AccountStatusStore, MemoryAccountStatus, Quota, QuotaDecision, SyncOutcome,
};
use hyperme_core::sync::SyncError;
use time::{Date, Duration, Month, Time, UtcDateTime};

fn now() -> UtcDateTime {
    UtcDateTime::new(
        Date::from_calendar_date(2026, Month::November, 27).unwrap(),
        Time::from_hms(10, 0, 0).unwrap(),
    )
}

/// R-10, AC-4: a refused refresh token marks the account "Sign in again" and raises exactly one
/// alert, which opens the account's row; later syncs do not run, so nothing retries, until a
/// new sign-in clears the mark.
#[tokio::test]
async fn an_expired_sign_in_raises_one_alert_and_stops_syncing() {
    let status = MemoryAccountStatus::default();
    let runs = AtomicUsize::new(0);
    let refused = || async {
        runs.fetch_add(1, Ordering::SeqCst);
        Err::<(), _>(SyncError::SignInAgain)
    };

    let first = health::guarded_sync(&status, "google:work", "Google · work", now(), refused).await;
    let SyncOutcome::Alert(alert) = first else {
        panic!("{first:?}");
    };
    assert_eq!(alert.title, "Google · work needs you to sign in again");
    assert_eq!(alert.link, "hyperme://settings/connectors/google:work");
    assert!(status.sign_in_needed("google:work").unwrap());

    for _ in 0..3 {
        let later =
            health::guarded_sync(&status, "google:work", "Google · work", now(), refused).await;
        assert!(matches!(later, SyncOutcome::SignInNeeded), "{later:?}");
    }
    assert_eq!(
        runs.load(Ordering::SeqCst),
        1,
        "no retry until the user signs in"
    );

    status.clear("google:work").unwrap();
    let ok = health::guarded_sync(&status, "google:work", "Google · work", now(), || async {
        Ok(7)
    })
    .await;
    assert!(matches!(ok, SyncOutcome::Done(7)), "{ok:?}");
}

/// Other failures neither mark the account nor raise the sign-in alert; the scheduler's own
/// retry rules (R-36) handle them.
#[tokio::test]
async fn other_failures_are_not_a_sign_in() {
    let status = MemoryAccountStatus::default();
    let outcome = health::guarded_sync(&status, "google:work", "Google · work", now(), || async {
        Err::<(), _>(SyncError::Failed("timed out".to_owned()))
    })
    .await;
    assert!(
        matches!(&outcome, SyncOutcome::Failed(reason) if reason == "timed out"),
        "{outcome:?}"
    );
    assert!(!status.sign_in_needed("google:work").unwrap());
}

/// R-20: under 80% of the day's quota a pull runs; from 80% it runs at most every 2 hours;
/// at the quota it stops until tomorrow. With no quota known it always runs.
#[test]
fn pulls_slow_down_at_eighty_percent() {
    let quota = Some(Quota { daily_units: 1000 });
    let used = |units| Usage {
        calls: units,
        units,
    };
    let half_hour_ago = Some(now() - Duration::minutes(30));
    let three_hours_ago = Some(now() - Duration::hours(3));

    assert_eq!(
        health::quota_allows(used(799), quota, half_hour_ago, now()),
        QuotaDecision::Run
    );
    assert_eq!(
        health::quota_allows(used(800), quota, half_hour_ago, now()),
        QuotaDecision::SlowDown
    );
    assert_eq!(
        health::quota_allows(used(800), quota, three_hours_ago, now()),
        QuotaDecision::Run
    );
    assert_eq!(
        health::quota_allows(used(800), quota, None, now()),
        QuotaDecision::Run
    );
    assert_eq!(
        health::quota_allows(used(1000), quota, three_hours_ago, now()),
        QuotaDecision::Stop
    );
    assert_eq!(
        health::quota_allows(used(99_999), None, half_hour_ago, now()),
        QuotaDecision::Run
    );
}

/// R-20: the Google preset says each server's daily quota and what each tool costs, so the
/// counter adds real units (reported numbers, unverified until slice 1).
#[test]
fn the_google_preset_has_quotas() {
    let registry = cs::registry(None, &Connectors::default()).unwrap();
    let google = registry.get("google").unwrap();
    let gmail = google.server("gmail").unwrap();
    assert!(gmail.daily_units.is_some());
    assert_eq!(gmail.units.get("search_threads"), Some(&10));
    assert!(google.server("calendar").unwrap().daily_units.is_some());
}

/// The account page's line: requests today, and units against the quota when it is known.
#[test]
fn the_requests_line_reads_plainly() {
    let usage = Usage {
        calls: 31,
        units: 310,
    };
    assert_eq!(
        health::requests_line(usage, Some(Quota { daily_units: 5000 })),
        "31 requests today · 310 of 5000 units"
    );
    assert_eq!(health::requests_line(usage, None), "31 requests today");
    assert_eq!(
        health::requests_line(Usage { calls: 1, units: 1 }, None),
        "1 request today"
    );
}
