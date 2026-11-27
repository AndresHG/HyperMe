//! Connectors slice 7: the calendar cache on SQLite, table `events` (schema version 4). Tests
//! cite `specs/97-google-connectors/spec.md`, R-12, R-19 and AC-9.

use std::path::{Path, PathBuf};

use hyperme_core::connectors::AccountData;
use hyperme_core::sync::{Event, EventStore};
use hyperme_platform::task_store::SqliteTaskStore;
use time::{Date, Duration, Month, Time, UtcDateTime};

fn temp_db(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("events")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("hyperme.sqlite3")
}

fn utc(day: u8, h: u8) -> UtcDateTime {
    UtcDateTime::new(
        Date::from_calendar_date(2026, Month::November, day).unwrap(),
        Time::from_hms(h, 0, 0).unwrap(),
    )
}

fn event(account: &str, id: &str, start: UtcDateTime) -> Event {
    Event {
        account: account.to_owned(),
        id: id.to_owned(),
        calendar: Some("primary".to_owned()),
        title: format!("Synthetic {id}"),
        start,
        end: start + Duration::hours(1),
        all_day: false,
        url: Some(format!("https://meet.example/{id}")),
        status: Some("confirmed".to_owned()),
    }
}

/// R-19: events survive reopening, a pull replaces only its account's window, and the order is
/// by start.
#[test]
fn events_round_trip_and_replace_by_window() {
    let path = temp_db("round_trip");
    let store = SqliteTaskStore::open(&path).unwrap();
    store
        .replace(
            "google:work",
            utc(27, 0),
            utc(28, 0),
            &[event("google:work", "w", utc(27, 9))],
        )
        .unwrap();
    store
        .replace(
            "google:personal",
            utc(27, 0),
            utc(28, 0),
            &[
                event("google:personal", "b", utc(27, 15)),
                event("google:personal", "a", utc(27, 8)),
            ],
        )
        .unwrap();
    store
        .replace(
            "google:personal",
            utc(27, 0),
            utc(28, 0),
            &[event("google:personal", "c", utc(27, 10))],
        )
        .unwrap();
    drop(store);

    let store = SqliteTaskStore::open(&path).unwrap();
    let all = store.between(None, utc(27, 0), utc(28, 0)).unwrap();
    let ids: Vec<&str> = all.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["w", "c"]);
    assert_eq!(all[1], event("google:personal", "c", utc(27, 10)));
    assert_eq!(
        store
            .between(Some("google:work"), utc(27, 0), utc(28, 0))
            .unwrap()
            .len(),
        1
    );
}

/// R-19, AC-9: retention deletes what ended more than a day ago and what starts more than 8
/// days ahead.
#[test]
fn retention_deletes_outside_the_window() {
    let store = SqliteTaskStore::open(&temp_db("retention")).unwrap();
    let now = utc(27, 12);
    let far = now + Duration::days(9);
    store
        .replace(
            "google:work",
            utc(1, 0),
            far + Duration::days(1),
            &[
                event("google:work", "old", utc(25, 9)),
                event("google:work", "yesterday", utc(26, 13)),
                event("google:work", "soon", utc(29, 9)),
                event("google:work", "far", far),
            ],
        )
        .unwrap();
    assert_eq!(store.retain(now).unwrap(), 2);
    let ids: Vec<String> = store
        .between(None, utc(1, 0), far + Duration::days(1))
        .unwrap()
        .into_iter()
        .map(|e| e.id)
        .collect();
    assert_eq!(ids, ["yesterday", "soon"]);
}

/// R-12, AC-6: Disconnect deletes that account's events and leaves the others'.
#[test]
fn disconnect_deletes_the_accounts_events() {
    let store = SqliteTaskStore::open(&temp_db("disconnect")).unwrap();
    store
        .replace(
            "google:work",
            utc(27, 0),
            utc(28, 0),
            &[event("google:work", "w", utc(27, 9))],
        )
        .unwrap();
    store
        .replace(
            "google:personal",
            utc(27, 0),
            utc(28, 0),
            &[event("google:personal", "p", utc(27, 9))],
        )
        .unwrap();
    store.delete_account("google:work").unwrap();
    let left = store.between(None, utc(27, 0), utc(28, 0)).unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].account, "google:personal");
}
