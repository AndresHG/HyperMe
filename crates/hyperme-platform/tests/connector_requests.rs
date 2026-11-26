//! Connectors slice 3: the request counter on SQLite, table `connector_requests` (schema
//! version 3). Tests cite `specs/97-google-connectors/spec.md`, R-20.

use std::path::{Path, PathBuf};

use hyperme_core::connectors::{RequestLog, Usage};
use hyperme_platform::task_store::SqliteTaskStore;
use time::{Date, Month};

fn temp_db(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("connector_requests")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("hyperme.sqlite3")
}

fn day() -> Date {
    Date::from_calendar_date(2026, Month::November, 27).unwrap()
}

/// R-20: calls and units add up per account, day and server, and survive reopening.
#[test]
fn requests_add_up_per_account_day_and_server_and_persist() {
    let path = temp_db("add_up");
    let store = SqliteTaskStore::open(&path).unwrap();
    assert_eq!(
        store.usage("google:personal", day(), "gmail").unwrap(),
        Usage::default()
    );
    store.record("google:personal", day(), "gmail", 10).unwrap();
    store.record("google:personal", day(), "gmail", 1).unwrap();
    store
        .record("google:personal", day(), "calendar", 1)
        .unwrap();
    store.record("google:work", day(), "gmail", 1).unwrap();
    store
        .record("google:personal", day().next_day().unwrap(), "gmail", 1)
        .unwrap();
    drop(store);

    let store = SqliteTaskStore::open(&path).unwrap();
    assert_eq!(
        store.usage("google:personal", day(), "gmail").unwrap(),
        Usage {
            calls: 2,
            units: 11
        }
    );
    assert_eq!(
        store
            .usage("google:personal", day(), "calendar")
            .unwrap()
            .calls,
        1
    );
    assert_eq!(store.usage("google:work", day(), "gmail").unwrap().calls, 1);
    assert_eq!(
        store
            .usage("google:personal", day().next_day().unwrap(), "gmail")
            .unwrap()
            .calls,
        1
    );
}

/// design.md, "Storage": a file from before the counter gains the table on open and keeps its
/// other data.
#[test]
fn an_older_file_gains_the_table() {
    let path = temp_db("older");
    drop(SqliteTaskStore::open(&path).unwrap());
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("DROP TABLE connector_requests; PRAGMA user_version = 2;")
        .unwrap();
    drop(db);

    let store = SqliteTaskStore::open(&path).unwrap();
    store.record("google:personal", day(), "gmail", 1).unwrap();
    assert_eq!(
        store
            .usage("google:personal", day(), "gmail")
            .unwrap()
            .calls,
        1
    );
}
