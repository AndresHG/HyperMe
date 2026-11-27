//! Slice 2 of specs/166-gmail-approvals: the server states on SQLite, table
//! `connector_server_status` (schema version 8). Tests cite `specs/166-gmail-approvals`.

use std::path::PathBuf;

use hyperme_core::connectors::AccountData;
use hyperme_core::health::{PullEnd, ServerState, ServerStatusStore, after_pull, pull_runs};
use hyperme_platform::task_store::SqliteTaskStore;
use time::{Date, Month};

fn temp_db(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "hyperme-server-status-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("tasks.sqlite")
}

fn day(n: u8) -> Date {
    Date::from_calendar_date(2026, Month::November, n).unwrap()
}

/// AC-3, AC-5, AC-14 on SQLite: states, the alert day and the held-back count survive a
/// reopen; "Not allowed" skips pulls.
#[test]
fn server_states_are_kept_across_restarts() {
    let path = temp_db("kept");
    {
        let store = SqliteTaskStore::open(&path).unwrap();
        let needs = PullEnd::NeedsUser(ServerState::NeedsOk);
        assert!(after_pull(&store, "google:personal", "gmail", needs, day(27)).unwrap());
        after_pull(
            &store,
            "google:personal",
            "calendar",
            PullEnd::NeedsUser(ServerState::NotAllowed),
            day(27),
        )
        .unwrap();
        after_pull(
            &store,
            "google:work",
            "gmail",
            PullEnd::Done { held_back: 12 },
            day(27),
        )
        .unwrap();
    }
    let store = SqliteTaskStore::open(&path).unwrap();
    let gmail = store.status("google:personal", "gmail").unwrap();
    assert_eq!(gmail.state, Some(ServerState::NeedsOk));
    assert_eq!(gmail.last_alert, Some(day(27)));
    assert!(!pull_runs(&gmail, day(27)));
    assert!(
        !after_pull(
            &store,
            "google:personal",
            "gmail",
            PullEnd::NeedsUser(ServerState::NeedsOk),
            day(27)
        )
        .unwrap()
    );
    let calendar = store.status("google:personal", "calendar").unwrap();
    assert_eq!(calendar.state, Some(ServerState::NotAllowed));
    assert!(!pull_runs(&calendar, day(28)));
    assert_eq!(store.status("google:work", "gmail").unwrap().held_back, 12);
    assert_eq!(store.status("google:none", "gmail").unwrap().state, None);
}

/// R-16a, spec 97 R-12: a new sign-in clears the account's servers; a disconnect deletes them.
#[test]
fn sign_in_and_disconnect_clear_the_account() {
    let store = SqliteTaskStore::open(&temp_db("clear")).unwrap();
    let needs = PullEnd::NeedsUser(ServerState::Blocked);
    after_pull(&store, "google:personal", "gmail", needs, day(27)).unwrap();
    after_pull(&store, "google:work", "gmail", needs, day(27)).unwrap();
    store.clear_account("google:personal").unwrap();
    assert_eq!(
        store.status("google:personal", "gmail").unwrap().state,
        None
    );
    store.delete_account("google:work").unwrap();
    assert_eq!(store.status("google:work", "gmail").unwrap().state, None);
}
