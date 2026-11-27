//! Tasks slice 2: the task list on SQLite. Tests cite `specs/92-daily-tasks/spec.md`: AC-4,
//! AC-5 (data), AC-6 (data), AC-7 and AC-12 (the day log). Every store is a fresh file under the
//! target directory.

use std::path::{Path, PathBuf};

use hyperme_core::tasks::{
    DayRules, Due, Finish, Group, NewTask, Source, Status, TaskError, TaskId, TaskStore,
    TaskUpdate, order_today,
};
use hyperme_platform::task_store::{DayCounter, DayLog, SqliteTaskStore};
use time::{Date, Month, Time};

fn temp_db(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("task_store")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("hyperme.sqlite3")
}

fn open(name: &str) -> SqliteTaskStore {
    SqliteTaskStore::open(&temp_db(name)).unwrap()
}

fn day(d: u8) -> Date {
    Date::from_calendar_date(2026, Month::November, d).unwrap()
}
const WED: u8 = 25;
const THU: u8 = 26;
const FRI: u8 = 27;

/// The instant Thursday began, in Unix ms.
const THU_BEGAN: i64 = 1_000_000;

fn new(title: &str) -> NewTask {
    NewTask {
        title: title.into(),
        notes: None,
        day: None,
        due: None,
        source: Source::HyperMe,
        blocked_by: vec![],
        created_at: 0,
        arrived_day: day(WED),
    }
}

fn planned(title: &str, d: u8) -> NewTask {
    NewTask {
        day: Some(day(d)),
        ..new(title)
    }
}

fn issue(reference: &str, title: &str) -> NewTask {
    NewTask {
        source: Source::Mirror {
            name: "github".into(),
            account: Some("personal".into()),
            reference: reference.into(),
            url: Some(format!(
                "https://github.com/example/repo/issues/{reference}"
            )),
        },
        ..new(title)
    }
}

async fn today_titles(store: &SqliteTaskStore, d: u8) -> (Vec<String>, Vec<String>) {
    let snapshot = store.snapshot(day(d), THU_BEGAN).await.unwrap();
    let today = order_today(&snapshot, day(d), THU_BEGAN, &DayRules::default());
    (
        today.lines.iter().map(|l| l.task.title.clone()).collect(),
        today
            .waiting
            .iter()
            .map(|w| format!("{} ← {}", w.task.title, w.on.title))
            .collect(),
    )
}

// --- The file ------------------------------------------------------------------------------

/// design.md, "The store": WAL mode so cards never wait for a write, and the schema version in
/// `user_version`. Reopening keeps the tasks; a file from a newer HyperMe is refused.
#[tokio::test]
async fn the_store_uses_wal_and_a_schema_version_and_survives_reopening() {
    let path = temp_db("reopen");
    let store = SqliteTaskStore::open(&path).unwrap();
    let id = store.add(new("Call the dentist")).await.unwrap();
    drop(store);

    let db = rusqlite::Connection::open(&path).unwrap();
    let mode: String = db
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    let version: i64 = db
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    // 1 tasks, 2 jobs, 3 connector requests, 4 events, 5 mail, 6 sign-in state
    // (specs/97-google-connectors).
    assert_eq!(version, 6);

    let store = SqliteTaskStore::open(&path).unwrap();
    let snapshot = store.snapshot(day(WED), 0).await.unwrap();
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].id, id);
    drop(store);

    db.execute_batch("PRAGMA user_version = 99").unwrap();
    let err = SqliteTaskStore::open(&path).unwrap_err();
    assert!(err.to_string().contains("newer"), "{err}");
}

/// R-1: every field of a task survives the store: notes, day, due with a time, a mirrored
/// source with its link, creation and arrival.
#[tokio::test]
async fn a_task_round_trips_with_every_field() {
    let store = open("round_trip");
    let mut task = issue("105", "Review the slice");
    task.notes = Some("Before the standup".into());
    task.day = Some(day(THU));
    task.due = Some(Due {
        date: day(FRI),
        time: Some(Time::from_hms(10, 0, 0).unwrap()),
    });
    task.created_at = 1_234;
    let id = store.add(task.clone()).await.unwrap();

    let snapshot = store.snapshot(day(THU), THU_BEGAN).await.unwrap();
    let got = snapshot.iter().find(|t| t.id == id).unwrap();
    assert_eq!(got.title, task.title);
    assert_eq!(got.notes, task.notes);
    assert_eq!(got.status, Status::Open);
    assert_eq!(got.day, task.day);
    assert_eq!(got.due, task.due);
    assert_eq!(got.source, task.source);
    assert_eq!(got.created_at, 1_234);
    assert_eq!(got.arrived_day, day(WED));
    assert_eq!(got.rolled, 0);
    assert_eq!(got.finished_at, None);
    assert!(got.blocked_by.is_empty());
}

/// R-4: a change sets the fields it names and leaves the rest; `Some(None)` clears.
#[tokio::test]
async fn an_update_changes_only_the_fields_it_names() {
    let store = open("update");
    let mut task = planned("Report", THU);
    task.notes = Some("Q4".into());
    let id = store.add(task).await.unwrap();

    store
        .update(
            id,
            TaskUpdate {
                title: Some("The report".into()),
                notes: Some(None),
                day: Some(Some(day(FRI))),
                ..TaskUpdate::default()
            },
        )
        .await
        .unwrap();
    let snapshot = store.snapshot(day(FRI), THU_BEGAN).await.unwrap();
    let got = &snapshot[0];
    assert_eq!(got.title, "The report");
    assert_eq!(got.notes, None);
    assert_eq!(got.day, Some(day(FRI)));

    assert_eq!(
        store.update(99, TaskUpdate::default()).await,
        Err(TaskError::NotFound(99))
    );
}

// --- Dependencies --------------------------------------------------------------------------

/// AC-4, R-7: linking A blocked by B when B is already blocked by A is refused naming both; a
/// three-task loop is refused naming all three; nothing is written.
#[tokio::test]
async fn a_link_that_would_make_a_loop_is_refused_with_its_path() {
    let store = open("cycle");
    let a = store.add(new("A")).await.unwrap();
    let b = store
        .add(NewTask {
            blocked_by: vec![a],
            ..new("B")
        })
        .await
        .unwrap();
    let c = store.add(new("C")).await.unwrap();
    store.block(c, b).await.unwrap();

    assert_eq!(store.block(a, b).await, Err(TaskError::Cycle(vec![b, a])));
    assert_eq!(
        store.block(a, c).await,
        Err(TaskError::Cycle(vec![c, b, a]))
    );
    assert_eq!(store.block(a, a).await, Err(TaskError::Cycle(vec![a])));
    assert_eq!(store.block(a, 99).await, Err(TaskError::NotFound(99)));
    assert_eq!(store.block(99, a).await, Err(TaskError::NotFound(99)));
    assert_eq!(
        store
            .add(NewTask {
                blocked_by: vec![99],
                ..new("D")
            })
            .await,
        Err(TaskError::NotFound(99))
    );

    let snapshot = store.snapshot(day(WED), 0).await.unwrap();
    let blockers = |id: TaskId| {
        snapshot
            .iter()
            .find(|t| t.id == id)
            .unwrap()
            .blocked_by
            .clone()
    };
    assert!(blockers(a).is_empty(), "no loop was written");
    assert_eq!(blockers(c), vec![b]);
    // Linking twice is harmless; unlinking removes it.
    store.block(c, b).await.unwrap();
    store.unblock(c, b).await.unwrap();
    let snapshot = store.snapshot(day(WED), 0).await.unwrap();
    assert!(
        snapshot
            .iter()
            .all(|t| t.id != c || t.blocked_by.is_empty())
    );
}

/// AC-5 (data), R-10, R-11: a task with an open blocker waits on it; finishing the blocker
/// makes it ready and "unblocked" in Today; dropping the blocker says so.
#[tokio::test]
async fn finishing_a_blocker_unblocks_its_task() {
    let store = open("unblock");
    let call = store.add(planned("Call the insurance", THU)).await.unwrap();
    let send = store
        .add(NewTask {
            blocked_by: vec![call],
            ..planned("Send Ana the numbers", THU)
        })
        .await
        .unwrap();
    let room = store.add(new("Book the room")).await.unwrap();
    let invite = store
        .add(NewTask {
            blocked_by: vec![room],
            ..planned("Send the invite", THU)
        })
        .await
        .unwrap();

    assert!(store.ready(call).unwrap());
    assert!(!store.ready(send).unwrap());
    assert_eq!(
        today_titles(&store, THU).await.1,
        vec![
            "Send Ana the numbers ← Call the insurance",
            "Send the invite ← Book the room"
        ]
    );

    store
        .finish(call, Finish::Done, THU_BEGAN + 10)
        .await
        .unwrap();
    store
        .finish(room, Finish::Dropped, THU_BEGAN + 20)
        .await
        .unwrap();
    assert!(store.ready(send).unwrap());
    assert!(!store.ready(call).unwrap(), "a done task is not ready");

    let snapshot = store.snapshot(day(THU), THU_BEGAN).await.unwrap();
    let today = order_today(&snapshot, day(THU), THU_BEGAN, &DayRules::default());
    let groups: Vec<(TaskId, Group)> = today.lines.iter().map(|l| (l.task.id, l.group)).collect();
    assert_eq!(
        groups,
        vec![
            (
                send,
                Group::Unblocked {
                    blocker_dropped: false
                }
            ),
            (
                invite,
                Group::Unblocked {
                    blocker_dropped: true
                }
            ),
        ]
    );
    let finished = snapshot.iter().find(|t| t.id == room).unwrap();
    assert_eq!(finished.status, Status::Dropped);
    assert_eq!(finished.finished_at, Some(THU_BEGAN + 20));
    assert_eq!(
        store.finish(99, Finish::Done, 0).await,
        Err(TaskError::NotFound(99))
    );
}

/// AC-6 (data), R-12: a local task blocked by a mirrored GitHub issue becomes ready after a sync
/// that sees the issue closed. The mirror is found by source, account and reference, so a sync
/// updates it rather than adding a second one, and keeps the user's day.
#[tokio::test]
async fn a_mirrored_blocker_closed_at_its_source_unblocks_the_local_task() {
    let store = open("mirror");
    let pr = store
        .upsert_mirror(issue("7", "Review the PR"), None)
        .await
        .unwrap();
    store
        .update(
            pr,
            TaskUpdate {
                day: Some(Some(day(THU))),
                ..TaskUpdate::default()
            },
        )
        .await
        .unwrap();
    let release = store
        .add(NewTask {
            blocked_by: vec![pr],
            ..planned("Cut the release", THU)
        })
        .await
        .unwrap();
    assert!(!store.ready(release).unwrap());

    // The next sync: renamed at the source, still open.
    let again = store
        .upsert_mirror(issue("7", "Review the release PR"), None)
        .await
        .unwrap();
    assert_eq!(again, pr);
    // The same reference on another account is another item.
    let mut other = issue("7", "Someone else's 7");
    if let Source::Mirror { account, .. } = &mut other.source {
        *account = Some("work".into());
    }
    let other = store.upsert_mirror(other, None).await.unwrap();
    assert_ne!(other, pr);

    // The sync after that sees it closed.
    store
        .upsert_mirror(issue("7", "Review the release PR"), Some(THU_BEGAN + 5))
        .await
        .unwrap();
    assert!(store.ready(release).unwrap());

    let snapshot = store.snapshot(day(THU), THU_BEGAN).await.unwrap();
    let mirrored = snapshot.iter().find(|t| t.id == pr).unwrap();
    assert_eq!(mirrored.title, "Review the release PR");
    assert_eq!(mirrored.day, Some(day(THU)), "the user's day is kept");
    assert_eq!(mirrored.status, Status::Done);
    let today = order_today(&snapshot, day(THU), THU_BEGAN, &DayRules::default());
    assert_eq!(today.lines[0].task.id, release);
}

// --- Roll-over -----------------------------------------------------------------------------

/// AC-7, R-13, R-14: at day start an open task planned for yesterday stays in Today with a
/// roll-over count of 1 and its original day; after three roll-overs Today marks it.
#[tokio::test]
async fn roll_over_counts_and_keeps_the_day() {
    let store = open("roll_over");
    let dentist = store.add(planned("Call the dentist", 23)).await.unwrap();
    let done = store.add(planned("Done already", 23)).await.unwrap();
    store.finish(done, Finish::Done, 0).await.unwrap();
    store.add(planned("For Friday", FRI)).await.unwrap();

    assert_eq!(store.roll_over(day(24)).await.unwrap(), vec![dentist]);
    assert_eq!(store.roll_over(day(WED)).await.unwrap(), vec![dentist]);
    let snapshot = store.snapshot(day(WED), THU_BEGAN).await.unwrap();
    let task = snapshot.iter().find(|t| t.id == dentist).unwrap();
    assert_eq!(task.rolled, 2);
    assert!(!DayRules::default().is_stale(task));

    store.roll_over(day(THU)).await.unwrap();
    let snapshot = store.snapshot(day(THU), THU_BEGAN).await.unwrap();
    let task = snapshot.iter().find(|t| t.id == dentist).unwrap();
    assert_eq!(task.rolled, 3);
    assert_eq!(task.day, Some(day(23)), "the original day is kept");
    let today = order_today(&snapshot, day(THU), THU_BEGAN, &DayRules::default());
    assert_eq!(today.lines[0].task.id, dentist);
    assert_eq!(today.lines[0].group, Group::RolledOver);
    assert!(today.lines[0].stale);
}

// --- Snapshot ------------------------------------------------------------------------------

/// The snapshot holds what Today needs: the tasks that belong to the day, the ones finished
/// since day start, and the blockers of those even when they belong to another day.
#[tokio::test]
async fn the_snapshot_holds_today_and_the_blockers_it_needs() {
    let store = open("snapshot");
    let later_blocker = store
        .add(planned("Blocker for next week", 30))
        .await
        .unwrap();
    let today_task = store
        .add(NewTask {
            blocked_by: vec![later_blocker],
            ..planned("Today, blocked", THU)
        })
        .await
        .unwrap();
    let later = store.add(planned("Friday", FRI)).await.unwrap();
    let finished_today = store.add(planned("Done today", THU)).await.unwrap();
    store
        .finish(finished_today, Finish::Done, THU_BEGAN + 1)
        .await
        .unwrap();
    let finished_before = store.add(planned("Done yesterday", WED)).await.unwrap();
    store
        .finish(finished_before, Finish::Done, THU_BEGAN - 1)
        .await
        .unwrap();

    let snapshot = store.snapshot(day(THU), THU_BEGAN).await.unwrap();
    let mut ids: Vec<TaskId> = snapshot.iter().map(|t| t.id).collect();
    ids.sort();
    assert_eq!(ids, vec![later_blocker, today_task, finished_today]);
    assert!(!ids.contains(&later));
    assert_eq!(
        today_titles(&store, THU).await.1,
        vec!["Today, blocked ← Blocker for next week"]
    );
}

// --- Undo ----------------------------------------------------------------------------------

/// R-3: ⌘Z right after a voice capture deletes the new task and its links in one go.
#[tokio::test]
async fn undo_removes_a_task_and_its_links() {
    let store = open("undo");
    let call = store.add(planned("Call the insurance", THU)).await.unwrap();
    let send = store
        .add(NewTask {
            blocked_by: vec![call],
            ..planned("Send Ana the numbers", THU)
        })
        .await
        .unwrap();
    let after = store
        .add(NewTask {
            blocked_by: vec![send],
            ..planned("File the receipt", THU)
        })
        .await
        .unwrap();

    store.remove(send).await.unwrap();
    let snapshot = store.snapshot(day(THU), THU_BEGAN).await.unwrap();
    assert!(snapshot.iter().all(|t| t.id != send));
    let filed = snapshot.iter().find(|t| t.id == after).unwrap();
    assert!(filed.blocked_by.is_empty(), "its links went with it");
    assert!(store.ready(after).unwrap());
    assert_eq!(store.remove(send).await, Err(TaskError::NotFound(send)));
}

// --- The day log ---------------------------------------------------------------------------

/// AC-12 (data half), R-29: one line per day of counters, for "Yesterday" and the week view.
#[tokio::test]
async fn the_day_log_keeps_one_line_of_counters_per_day() {
    let store = open("day_log");
    assert_eq!(store.day_log(day(THU)).unwrap(), DayLog::default());

    store.count(day(THU), DayCounter::Captured, 2).unwrap();
    store.count(day(THU), DayCounter::Done, 1).unwrap();
    store.count(day(THU), DayCounter::Moved, 2).unwrap();
    store.count(day(THU), DayCounter::Dropped, 1).unwrap();
    store.count(day(THU), DayCounter::Done, 1).unwrap();
    store.count(day(FRI), DayCounter::Done, 4).unwrap();

    assert_eq!(
        store.day_log(day(THU)).unwrap(),
        DayLog {
            captured: 2,
            done: 2,
            moved: 2,
            dropped: 1
        }
    );
    assert_eq!(store.day_log(day(FRI)).unwrap().done, 4);
}
