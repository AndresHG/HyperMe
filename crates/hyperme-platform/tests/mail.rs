//! Connectors slice 8: inbox metadata on SQLite, table `mail_meta` (schema version 5), with the
//! mail pull run against it. Tests cite `specs/97-google-connectors/spec.md`, R-12, R-18, R-19,
//! AC-9 and AC-15.

use std::path::{Path, PathBuf};

use hyperme_core::config::Connectors;
use hyperme_core::connector_settings as cs;
use hyperme_core::connectors::AccountData;
use hyperme_core::schedule::Zone;
use hyperme_core::sync::{self, MailStore, SyncError, ToolCaller};
use hyperme_platform::task_store::SqliteTaskStore;
use serde_json::{Value, json};
use time::{Date, Duration, Month, Time, UtcDateTime, UtcOffset};

struct Plus1;

impl Zone for Plus1 {
    fn offset_at(&self, _at: UtcDateTime) -> UtcOffset {
        UtcOffset::from_hms(1, 0, 0).unwrap()
    }
}

const BODY_MARKER: &str = "SYNTHETIC-BODY-THAT-MUST-NOT-BE-STORED";

/// The inbox search, with a body field in each thread that the pull must not keep.
struct FakeGmail;

impl ToolCaller for FakeGmail {
    async fn call(&self, _: &str, _: &str, _: Value) -> Result<Value, SyncError> {
        let thread = |id: &str, last: &str| {
            json!({
                "id": id, "subject": format!("Subject {id}"), "from": "sender@example.test",
                "snippet": "a short snippet", "date": "2026-11-26T18:00:00Z",
                "labels": ["INBOX"], "lastMessage": { "labels": [last] },
                "body": BODY_MARKER,
            })
        };
        Ok(json!({ "threads": [thread("t1", "INBOX"), thread("t2", "SENT")] }))
    }
}

fn temp_db(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("mail")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("hyperme.sqlite3")
}

fn now() -> UtcDateTime {
    UtcDateTime::new(
        Date::from_calendar_date(2026, Month::November, 27).unwrap(),
        Time::from_hms(8, 0, 0).unwrap(),
    )
}

/// R-18, AC-9, AC-15: a pull keeps metadata only, on disk no body; the count to answer and the
/// mirrored task come from the same file; retention clears it after 14 days; Disconnect
/// deletes the account's rows.
#[tokio::test]
async fn the_mail_pull_keeps_metadata_only() {
    let path = temp_db("pull");
    let store = SqliteTaskStore::open(&path).unwrap();
    let registry = cs::registry(None, &Connectors::default()).unwrap();
    let pull = registry
        .get("google")
        .unwrap()
        .pulls
        .iter()
        .find(|p| p.into == "mail")
        .unwrap()
        .clone();
    let report = sync::pull_mail(
        &FakeGmail,
        &store,
        &store,
        "google:work",
        &pull,
        now(),
        &Plus1,
    )
    .await
    .unwrap();
    assert_eq!((report.threads, report.to_answer), (2, 1));
    assert_eq!(sync::threads_to_answer(&store, now()).unwrap(), 1);
    let cached = store.all(Some("google:work")).unwrap();
    assert_eq!(cached.len(), 2);
    assert_eq!(cached[0].labels, ["INBOX"]);
    drop(store);

    // The whole data folder: the file and its WAL hold no body.
    for entry in std::fs::read_dir(path.parent().unwrap()).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        assert!(
            !bytes
                .windows(BODY_MARKER.len())
                .any(|w| w == BODY_MARKER.as_bytes()),
            "a body reached the disk"
        );
    }

    let store = SqliteTaskStore::open(&path).unwrap();
    assert_eq!(store.retain(now() + Duration::days(13)).unwrap(), 0);
    store.delete_account("google:work").unwrap();
    assert!(store.all(None).unwrap().is_empty());
    sync::pull_mail(
        &FakeGmail,
        &store,
        &store,
        "google:work",
        &pull,
        now(),
        &Plus1,
    )
    .await
    .unwrap();
    assert_eq!(store.retain(now() + Duration::days(15)).unwrap(), 2);
}
