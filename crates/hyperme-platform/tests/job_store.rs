//! Tasks slice 4: the job table on SQLite (schema version 2). Tests cite
//! `specs/92-daily-tasks/spec.md`, R-31 and R-35.

use std::path::{Path, PathBuf};

use hyperme_core::schedule::Schedule;
use hyperme_core::scheduler::{Job, JobKind, JobStatus, JobStore};
use hyperme_core::tasks::{NewTask, Source, TaskStore, WeekdaySet};
use hyperme_platform::task_store::SqliteTaskStore;
use time::{Date, Month, SignedDuration, Time, UtcDateTime, Weekday};

fn temp_db(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("job_store")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("hyperme.sqlite3")
}

fn day() -> Date {
    Date::from_calendar_date(2026, Month::November, 26).unwrap()
}

fn hm(h: u8, m: u8) -> Time {
    Time::from_hms(h, m, 0).unwrap()
}

/// R-31: a job keeps its name, kind, schedule, switch, catch-up window, tool allowlist,
/// prompt, last run and result, and today's failures. Every schedule shape and kind survives.
#[tokio::test]
async fn jobs_round_trip_with_every_field() {
    let store = SqliteTaskStore::open(&temp_db("round_trip")).unwrap();
    assert!(store.jobs().await.unwrap().is_empty());

    let mut prompt = Job::new(
        "weekly review",
        JobKind::Prompt,
        Schedule::At {
            time: hm(17, 0),
            weekdays: WeekdaySet::from_array([Weekday::Friday, Weekday::Sunday]),
        },
    );
    prompt.allowlist = vec!["github.search_issues".into(), "calendar.list_events".into()];
    prompt.prompt = Some("List what I closed this week".into());
    prompt.catch_up = SignedDuration::hours(3);
    prompt.enabled = false;
    prompt.last_run = Some(UtcDateTime::new(
        day(),
        Time::from_hms_milli(16, 0, 1, 250).unwrap(),
    ));
    prompt.last_status = Some(JobStatus::Failed("no network".into()));
    prompt.fails_today = 2;
    prompt.fails_day = Some(day());

    let mut sync = Job::new(
        "sync:github",
        JobKind::Sync("github".into()),
        Schedule::EveryWhilePresent { minutes: 30 },
    );
    sync.last_status = Some(JobStatus::Ok);
    let morning = Job::new(
        "morning",
        JobKind::Morning,
        Schedule::OnPresenceAfter {
            from: hm(6, 0),
            latest: hm(9, 30),
            weekdays: WeekdaySet::ALL,
        },
    );
    let others = [
        Job::new(
            "rollover",
            JobKind::RollOver,
            Schedule::EveryWhilePresent { minutes: 0 },
        ),
        Job::new(
            "evening",
            JobKind::Evening,
            Schedule::EveryWhilePresent { minutes: 1 },
        ),
        Job::new(
            "nudge",
            JobKind::Nudge,
            Schedule::EveryWhilePresent { minutes: 2 },
        ),
    ];

    let mut all = vec![prompt, sync, morning];
    all.extend(others);
    for job in &all {
        store.save_job(job).await.unwrap();
    }
    let mut got = store.jobs().await.unwrap();
    got.sort_by(|a, b| a.name.cmp(&b.name));
    all.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(got, all);
}

/// R-35: saving a job again replaces its row, so the last run is never counted twice.
#[tokio::test]
async fn saving_a_job_again_replaces_it() {
    let store = SqliteTaskStore::open(&temp_db("replace")).unwrap();
    let mut job = Job::new(
        "sync:github",
        JobKind::Sync("github".into()),
        Schedule::EveryWhilePresent { minutes: 30 },
    );
    store.save_job(&job).await.unwrap();
    job.last_run = Some(UtcDateTime::new(day(), hm(9, 0)));
    store.save_job(&job).await.unwrap();
    assert_eq!(store.jobs().await.unwrap(), vec![job]);
}

/// A version 1 file (tasks, no job table) is brought to version 2 and keeps its tasks.
#[tokio::test]
async fn a_version_1_file_gains_the_job_table_and_keeps_its_tasks() {
    let path = temp_db("migrate");
    let store = SqliteTaskStore::open(&path).unwrap();
    store
        .add(NewTask {
            title: "Call the dentist".into(),
            notes: None,
            day: None,
            due: None,
            source: Source::HyperMe,
            blocked_by: vec![],
            created_at: 0,
            arrived_day: day(),
        })
        .await
        .unwrap();
    drop(store);
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("DROP TABLE job; DROP TABLE connector_requests; DROP TABLE events; DROP TABLE mail_meta; PRAGMA user_version = 1;")
        .unwrap();
    drop(db);

    let store = SqliteTaskStore::open(&path).unwrap();
    assert!(store.jobs().await.unwrap().is_empty());
    assert_eq!(store.snapshot(day(), 0).await.unwrap().len(), 1);
}
