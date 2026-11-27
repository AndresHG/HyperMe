//! The task list on SQLite (specs/92-daily-tasks, design.md, "The store"): [`TaskStore`] on one
//! file, with the blocked-by links, the cycle check, the ready query, undo, mirror upserts and
//! the day log. Portable code: the app passes the path.
//!
//! WAL mode, so the cards' reads never wait for a write. The schema version lives in
//! `user_version`; each migration runs once, in a transaction. Days and due dates are local
//! civil dates as `YYYY-MM-DD` text, due times as `HH:MM`.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use hyperme_core::connectors::{AccountData, RequestLog, Usage};
use hyperme_core::health::AccountStatusStore;
use hyperme_core::schedule::Schedule;
use hyperme_core::scheduler::{Job, JobKind, JobStatus, JobStore};
use hyperme_core::sync::{Event, EventStore, MailMeta, MailStore};
use hyperme_core::tasks::{
    Due, Finish, NewTask, Source, Status, Task, TaskError, TaskId, TaskStore, TaskUpdate,
    WeekdaySet,
};
use hyperme_core::traits::{Error, ErrorKind};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use time::{Date, Month, SignedDuration, Time, UtcDateTime, Weekday};

/// One entry per schema version: entry n takes `user_version` from n to n + 1.
const MIGRATIONS: &[&str] = &[
    // 1: tasks, their links and the day log. The job table comes with the scheduler (slice 4).
    "CREATE TABLE task (
        id             INTEGER PRIMARY KEY,
        title          TEXT    NOT NULL,
        notes          TEXT,
        status         TEXT    NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'done', 'dropped')),
        day            TEXT,
        due_date       TEXT,
        due_time       TEXT,
        source         TEXT    NOT NULL DEFAULT 'hyperme',
        source_account TEXT,
        source_ref     TEXT,
        source_url     TEXT,
        created_at     INTEGER NOT NULL,
        arrived_day    TEXT    NOT NULL,
        rolled         INTEGER NOT NULL DEFAULT 0,
        finished_at    INTEGER,
        UNIQUE (source, source_account, source_ref)
    );
    CREATE TABLE task_block (
        blocked INTEGER NOT NULL REFERENCES task (id) ON DELETE CASCADE,
        blocker INTEGER NOT NULL REFERENCES task (id) ON DELETE CASCADE,
        PRIMARY KEY (blocked, blocker)
    ) WITHOUT ROWID;
    CREATE INDEX task_block_blocker ON task_block (blocker);
    CREATE INDEX task_open ON task (day, due_date) WHERE status = 'open';
    CREATE INDEX task_finished ON task (finished_at) WHERE status != 'open';
    CREATE TABLE day_log (
        day TEXT PRIMARY KEY,
        captured INTEGER NOT NULL DEFAULT 0, done INTEGER NOT NULL DEFAULT 0,
        moved INTEGER NOT NULL DEFAULT 0,    dropped INTEGER NOT NULL DEFAULT 0,
        morning_at INTEGER, evening_at INTEGER, closed_at INTEGER
    ) WITHOUT ROWID;",
    // 2: the scheduler's jobs (R-31). The schedule is text: `at HH:MM 1111100`,
    // `every 30` or `presence HH:MM HH:MM 1111100`, weekdays Monday first.
    "CREATE TABLE job (
        name         TEXT PRIMARY KEY,
        kind         TEXT NOT NULL,
        schedule     TEXT NOT NULL,
        enabled      INTEGER NOT NULL DEFAULT 1,
        catch_up_min INTEGER NOT NULL,
        allowlist    TEXT,
        prompt       TEXT,
        last_run     INTEGER,
        last_status  TEXT,
        fails_today  INTEGER NOT NULL DEFAULT 0,
        fails_day    TEXT
    ) WITHOUT ROWID;",
    // 3: the connector request counter (specs/97-google-connectors, design.md, "Storage").
    "CREATE TABLE connector_requests (
        account TEXT    NOT NULL,
        day     TEXT    NOT NULL,
        server  TEXT    NOT NULL,
        calls   INTEGER NOT NULL DEFAULT 0,
        units   INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (account, day, server)
    ) WITHOUT ROWID;",
    // 4: the calendar cache (specs/97-google-connectors, design.md, "Storage"). Times are
    // Unix seconds, UTC.
    "CREATE TABLE events (
        account   TEXT    NOT NULL,
        id        TEXT    NOT NULL,
        calendar  TEXT,
        title     TEXT    NOT NULL,
        starts_at INTEGER NOT NULL,
        ends_at   INTEGER NOT NULL,
        all_day   INTEGER NOT NULL DEFAULT 0,
        url       TEXT,
        status    TEXT,
        PRIMARY KEY (account, id)
    ) WITHOUT ROWID;
    CREATE INDEX events_start ON events (starts_at);",
    // 5: inbox metadata (specs/97-google-connectors, design.md, "Storage"): never a body
    // (R-18). Labels are one per line; received_at is Unix seconds, UTC.
    "CREATE TABLE mail_meta (
        account     TEXT    NOT NULL,
        id          TEXT    NOT NULL,
        subject     TEXT    NOT NULL,
        sender      TEXT    NOT NULL,
        snippet     TEXT    NOT NULL,
        received_at INTEGER NOT NULL,
        labels      TEXT    NOT NULL,
        to_answer   INTEGER NOT NULL,
        PRIMARY KEY (account, id)
    ) WITHOUT ROWID;",
    // 6: accounts waiting for a new sign-in (specs/97-google-connectors, R-10).
    "CREATE TABLE connector_sign_in (
        account TEXT    PRIMARY KEY,
        since   INTEGER NOT NULL
    ) WITHOUT ROWID;",
    // 7: the last time the user saw Today, which clears "new" (specs/92-daily-tasks, R-21).
    "ALTER TABLE day_log ADD COLUMN seen_at INTEGER;",
];

const TASK_COLUMNS: &str = "id, title, notes, status, day, due_date, due_time, source, \
     source_account, source_ref, source_url, created_at, arrived_day, rolled, finished_at";

/// The ids a snapshot needs: open tasks that belong to the day (?1), tasks finished since day
/// start (?2), and the blockers of those.
const WANTED: &str = "WITH today(id) AS (
        SELECT id FROM task
        WHERE (status = 'open'
               AND (day <= ?1 OR due_date <= ?1 OR (day IS NULL AND arrived_day = ?1)))
           OR (status != 'open' AND finished_at >= ?2)
    ),
    wanted(id) AS (
        SELECT id FROM today
        UNION SELECT blocker FROM task_block WHERE blocked IN (SELECT id FROM today)
    )";

/// A counter of the day log (R-29).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayCounter {
    Captured,
    Done,
    Moved,
    Dropped,
}

impl DayCounter {
    fn column(self) -> &'static str {
        match self {
            DayCounter::Captured => "captured",
            DayCounter::Done => "done",
            DayCounter::Moved => "moved",
            DayCounter::Dropped => "dropped",
        }
    }
}

/// One day's line of the day log.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DayLog {
    pub captured: u32,
    pub done: u32,
    pub moved: u32,
    pub dropped: u32,
}

/// The task list in one SQLite file.
pub struct SqliteTaskStore {
    db: Mutex<Connection>,
}

impl fmt::Debug for SqliteTaskStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqliteTaskStore").finish_non_exhaustive()
    }
}

fn store_error(what: impl fmt::Display) -> TaskError {
    TaskError::Store(Error {
        kind: ErrorKind::Device,
        message: format!("tasks: {what}"),
    })
}

fn sql<T>(result: rusqlite::Result<T>) -> Result<T, TaskError> {
    result.map_err(store_error)
}

fn date_text(d: Date) -> String {
    format!("{:04}-{:02}-{:02}", d.year(), u8::from(d.month()), d.day())
}

fn time_text(t: Time) -> String {
    format!("{:02}:{:02}", t.hour(), t.minute())
}

fn parse_date(text: &str) -> Result<Date, TaskError> {
    let bad = || store_error(format!("bad date {text:?}"));
    let mut parts = text.splitn(3, '-');
    let mut next = || parts.next().ok_or_else(bad);
    let year: i32 = next()?.parse().map_err(|_| bad())?;
    let month: u8 = next()?.parse().map_err(|_| bad())?;
    let day: u8 = next()?.parse().map_err(|_| bad())?;
    let month = Month::try_from(month).map_err(|_| bad())?;
    Date::from_calendar_date(year, month, day).map_err(|_| bad())
}

fn parse_time(text: &str) -> Result<Time, TaskError> {
    let bad = || store_error(format!("bad time {text:?}"));
    let (h, m) = text.split_once(':').ok_or_else(bad)?;
    let h: u8 = h.parse().map_err(|_| bad())?;
    let m: u8 = m.parse().map_err(|_| bad())?;
    Time::from_hms(h, m, 0).map_err(|_| bad())
}

fn status_text(status: Status) -> &'static str {
    match status {
        Status::Open => "open",
        Status::Done => "done",
        Status::Dropped => "dropped",
    }
}

/// The source columns: name, account, reference, link.
type SourceColumns<'a> = (&'a str, Option<&'a str>, Option<&'a str>, Option<&'a str>);

fn source_columns(source: &Source) -> SourceColumns<'_> {
    match source {
        Source::HyperMe => ("hyperme", None, None, None),
        Source::Mirror {
            name,
            account,
            reference,
            url,
        } => (
            name,
            account.as_deref(),
            Some(reference.as_str()),
            url.as_deref(),
        ),
    }
}

/// A task row as text, before its dates are parsed.
struct RawTask {
    id: TaskId,
    title: String,
    notes: Option<String>,
    status: String,
    day: Option<String>,
    due_date: Option<String>,
    due_time: Option<String>,
    source: String,
    source_account: Option<String>,
    source_ref: Option<String>,
    source_url: Option<String>,
    created_at: i64,
    arrived_day: String,
    rolled: u32,
    finished_at: Option<i64>,
}

impl RawTask {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<RawTask> {
        Ok(RawTask {
            id: row.get(0)?,
            title: row.get(1)?,
            notes: row.get(2)?,
            status: row.get(3)?,
            day: row.get(4)?,
            due_date: row.get(5)?,
            due_time: row.get(6)?,
            source: row.get(7)?,
            source_account: row.get(8)?,
            source_ref: row.get(9)?,
            source_url: row.get(10)?,
            created_at: row.get(11)?,
            arrived_day: row.get(12)?,
            rolled: row.get(13)?,
            finished_at: row.get(14)?,
        })
    }

    fn into_task(self) -> Result<Task, TaskError> {
        let status = match self.status.as_str() {
            "open" => Status::Open,
            "done" => Status::Done,
            "dropped" => Status::Dropped,
            other => return Err(store_error(format!("bad status {other:?}"))),
        };
        let due = match self.due_date {
            Some(date) => Some(Due {
                date: parse_date(&date)?,
                time: self.due_time.as_deref().map(parse_time).transpose()?,
            }),
            None => None,
        };
        let source = if self.source == "hyperme" {
            Source::HyperMe
        } else {
            Source::Mirror {
                name: self.source,
                account: self.source_account,
                reference: self.source_ref.unwrap_or_default(),
                url: self.source_url,
            }
        };
        Ok(Task {
            id: self.id,
            title: self.title,
            notes: self.notes,
            status,
            day: self.day.as_deref().map(parse_date).transpose()?,
            due,
            source,
            created_at: self.created_at,
            arrived_day: parse_date(&self.arrived_day)?,
            rolled: self.rolled,
            finished_at: self.finished_at,
            blocked_by: Vec::new(),
        })
    }
}

fn exists(db: &Connection, id: TaskId) -> Result<(), TaskError> {
    let found = sql(db
        .query_row("SELECT 1 FROM task WHERE id = ?1", [id], |_| Ok(()))
        .optional())?;
    found.ok_or(TaskError::NotFound(id))
}

/// Inserts a task and its links. Every blocker must exist.
fn insert(tx: &Transaction<'_>, task: &NewTask) -> Result<TaskId, TaskError> {
    for &blocker in &task.blocked_by {
        exists(tx, blocker)?;
    }
    let (source, account, reference, url) = source_columns(&task.source);
    sql(tx.execute(
        "INSERT INTO task (title, notes, day, due_date, due_time, source, source_account,
                           source_ref, source_url, created_at, arrived_day)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            task.title,
            task.notes,
            task.day.map(date_text),
            task.due.map(|d| date_text(d.date)),
            task.due.and_then(|d| d.time).map(time_text),
            source,
            account,
            reference,
            url,
            task.created_at,
            date_text(task.arrived_day),
        ],
    ))?;
    let id = tx.last_insert_rowid();
    for &blocker in &task.blocked_by {
        sql(tx.execute(
            "INSERT OR IGNORE INTO task_block (blocked, blocker) VALUES (?1, ?2)",
            params![id, blocker],
        ))?;
    }
    Ok(id)
}

impl SqliteTaskStore {
    /// Opens or creates the store at `path`, bringing its schema up to date. A file written by a
    /// newer HyperMe is refused rather than changed.
    pub fn open(path: &Path) -> Result<Self, TaskError> {
        let mut db = sql(Connection::open(path))?;
        let mode: String = sql(db.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0)))?;
        if !mode.eq_ignore_ascii_case("wal") {
            return Err(store_error(format!("journal mode is {mode}, not WAL")));
        }
        sql(db.execute_batch("PRAGMA foreign_keys = ON"))?;
        let version: i64 = sql(db.query_row("PRAGMA user_version", [], |r| r.get(0)))?;
        let version = usize::try_from(version).map_err(store_error)?;
        if version > MIGRATIONS.len() {
            return Err(store_error(format!(
                "the file is from a newer HyperMe (schema {version}, this one knows {})",
                MIGRATIONS.len()
            )));
        }
        for (n, migration) in MIGRATIONS.iter().enumerate().skip(version) {
            let tx = sql(db.transaction())?;
            sql(tx.execute_batch(migration))?;
            sql(tx.execute_batch(&format!("PRAGMA user_version = {}", n + 1)))?;
            sql(tx.commit())?;
        }
        Ok(SqliteTaskStore { db: Mutex::new(db) })
    }

    fn db(&self) -> MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// "Ready": open, and no blocker still open.
    pub fn ready(&self, id: TaskId) -> Result<bool, TaskError> {
        let ready = sql(self
            .db()
            .query_row(
                "SELECT status = 'open' AND NOT EXISTS (
                     SELECT 1 FROM task_block b JOIN task t ON t.id = b.blocker
                     WHERE b.blocked = task.id AND t.status = 'open')
                 FROM task WHERE id = ?1",
                [id],
                |r| r.get(0),
            )
            .optional())?;
        ready.ok_or(TaskError::NotFound(id))
    }

    /// Adds `n` to one counter of `day`'s line in the day log.
    pub fn count(&self, day: Date, counter: DayCounter, n: u32) -> Result<(), TaskError> {
        let column = counter.column();
        sql(self.db().execute(
            &format!(
                "INSERT INTO day_log (day, {column}) VALUES (?1, ?2)
                 ON CONFLICT (day) DO UPDATE SET {column} = {column} + excluded.{column}"
            ),
            params![date_text(day), n],
        ))?;
        Ok(())
    }

    /// Records when `day`'s morning card showed (Unix ms); a later write that day keeps the
    /// first (R-15).
    pub fn mark_morning(&self, day: Date, at_ms: i64) -> Result<(), TaskError> {
        sql(self.db().execute(
            "INSERT INTO day_log (day, morning_at) VALUES (?1, ?2)
             ON CONFLICT (day) DO UPDATE SET morning_at = coalesce(morning_at, excluded.morning_at)",
            params![date_text(day), at_ms],
        ))?;
        Ok(())
    }

    /// When `day`'s morning card showed, Unix ms.
    pub fn morning_at(&self, day: Date) -> Result<Option<i64>, TaskError> {
        let at = sql(self
            .db()
            .query_row(
                "SELECT morning_at FROM day_log WHERE day = ?1",
                [date_text(day)],
                |r| r.get::<_, Option<i64>>(0),
            )
            .optional())?;
        Ok(at.flatten())
    }

    /// Records that the user saw Today at `at_ms` (Unix ms); the latest look wins (R-21).
    pub fn mark_seen(&self, day: Date, at_ms: i64) -> Result<(), TaskError> {
        sql(self.db().execute(
            "INSERT INTO day_log (day, seen_at) VALUES (?1, ?2)
             ON CONFLICT (day) DO UPDATE SET seen_at = max(coalesce(seen_at, 0), excluded.seen_at)",
            params![date_text(day), at_ms],
        ))?;
        Ok(())
    }

    /// When the user last saw `day`'s Today, Unix ms.
    pub fn seen_at(&self, day: Date) -> Result<Option<i64>, TaskError> {
        let at = sql(self
            .db()
            .query_row(
                "SELECT seen_at FROM day_log WHERE day = ?1",
                [date_text(day)],
                |r| r.get::<_, Option<i64>>(0),
            )
            .optional())?;
        Ok(at.flatten())
    }

    /// "Keep" on a stale task: plans it for `today` and starts its roll-over count again, so
    /// the question is asked once (R-14).
    pub fn keep(&self, id: TaskId, today: Date) -> Result<(), TaskError> {
        let changed = sql(self.db().execute(
            "UPDATE task SET day = ?2, rolled = 0 WHERE id = ?1",
            params![id, date_text(today)],
        ))?;
        if changed == 0 {
            return Err(TaskError::NotFound(id));
        }
        Ok(())
    }

    /// `day`'s line of the day log; zeros for a day with none.
    pub fn day_log(&self, day: Date) -> Result<DayLog, TaskError> {
        let log = sql(self
            .db()
            .query_row(
                "SELECT captured, done, moved, dropped FROM day_log WHERE day = ?1",
                [date_text(day)],
                |r| {
                    Ok(DayLog {
                        captured: r.get(0)?,
                        done: r.get(1)?,
                        moved: r.get(2)?,
                        dropped: r.get(3)?,
                    })
                },
            )
            .optional())?;
        Ok(log.unwrap_or_default())
    }

    fn add_now(&self, task: &NewTask) -> Result<TaskId, TaskError> {
        let mut db = self.db();
        let tx = sql(db.transaction())?;
        let id = insert(&tx, task)?;
        sql(tx.commit())?;
        Ok(id)
    }

    fn update_now(&self, id: TaskId, change: &TaskUpdate) -> Result<(), TaskError> {
        let mut db = self.db();
        let tx = sql(db.transaction())?;
        exists(&tx, id)?;
        if let Some(title) = &change.title {
            sql(tx.execute(
                "UPDATE task SET title = ?2 WHERE id = ?1",
                params![id, title],
            ))?;
        }
        if let Some(notes) = &change.notes {
            sql(tx.execute(
                "UPDATE task SET notes = ?2 WHERE id = ?1",
                params![id, notes],
            ))?;
        }
        if let Some(day) = change.day {
            sql(tx.execute(
                "UPDATE task SET day = ?2 WHERE id = ?1",
                params![id, day.map(date_text)],
            ))?;
        }
        if let Some(due) = change.due {
            sql(tx.execute(
                "UPDATE task SET due_date = ?2, due_time = ?3 WHERE id = ?1",
                params![
                    id,
                    due.map(|d| date_text(d.date)),
                    due.and_then(|d| d.time).map(time_text)
                ],
            ))?;
        }
        sql(tx.commit())
    }

    fn finish_now(&self, id: TaskId, how: Finish, at_ms: i64) -> Result<(), TaskError> {
        let status = match how {
            Finish::Done => Status::Done,
            Finish::Dropped => Status::Dropped,
        };
        let changed = sql(self.db().execute(
            "UPDATE task SET status = ?2, finished_at = ?3 WHERE id = ?1",
            params![id, status_text(status), at_ms],
        ))?;
        if changed == 0 {
            return Err(TaskError::NotFound(id));
        }
        Ok(())
    }

    fn remove_now(&self, id: TaskId) -> Result<(), TaskError> {
        let mut db = self.db();
        let tx = sql(db.transaction())?;
        sql(tx.execute(
            "DELETE FROM task_block WHERE blocked = ?1 OR blocker = ?1",
            [id],
        ))?;
        if sql(tx.execute("DELETE FROM task WHERE id = ?1", [id]))? == 0 {
            return Err(TaskError::NotFound(id));
        }
        sql(tx.commit())
    }

    fn upsert_mirror_now(
        &self,
        task: &NewTask,
        closed_at: Option<i64>,
    ) -> Result<TaskId, TaskError> {
        let mut db = self.db();
        let tx = sql(db.transaction())?;
        let id = match source_columns(&task.source) {
            (_, _, None, _) => insert(&tx, task)?,
            (source, account, Some(reference), url) => {
                let found: Option<TaskId> = sql(tx
                    .query_row(
                        "SELECT id FROM task
                         WHERE source = ?1 AND source_account IS ?2 AND source_ref = ?3",
                        params![source, account, reference],
                        |r| r.get(0),
                    )
                    .optional())?;
                match found {
                    Some(id) => {
                        sql(tx.execute(
                            "UPDATE task SET title = ?2, notes = ?3, due_date = ?4,
                                             due_time = ?5, source_url = ?6
                             WHERE id = ?1",
                            params![
                                id,
                                task.title,
                                task.notes,
                                task.due.map(|d| date_text(d.date)),
                                task.due.and_then(|d| d.time).map(time_text),
                                url,
                            ],
                        ))?;
                        id
                    }
                    None => insert(&tx, task)?,
                }
            }
        };
        if let Some(at) = closed_at {
            sql(tx.execute(
                "UPDATE task SET status = 'done', finished_at = ?2
                 WHERE id = ?1 AND status = 'open'",
                params![id, at],
            ))?;
        }
        sql(tx.commit())?;
        Ok(id)
    }

    fn block_now(&self, blocked: TaskId, blocker: TaskId) -> Result<(), TaskError> {
        let mut db = self.db();
        let tx = sql(db.transaction())?;
        exists(&tx, blocker)?;
        exists(&tx, blocked)?;
        // Walk the blockers of `blocker`; reaching `blocked` means the new link closes a loop.
        // The graph has no loop yet, so the walk ends.
        let path: Option<String> = sql(tx
            .query_row(
                "WITH RECURSIVE walk(id, path) AS (
                     SELECT ?2, CAST(?2 AS TEXT)
                     UNION ALL
                     SELECT b.blocker, walk.path || ',' || b.blocker
                     FROM walk JOIN task_block b ON b.blocked = walk.id
                     WHERE walk.id != ?1
                 )
                 SELECT path FROM walk WHERE id = ?1 LIMIT 1",
                params![blocked, blocker],
                |r| r.get(0),
            )
            .optional())?;
        if let Some(path) = path {
            let ids = path
                .split(',')
                .map(|id| id.parse().map_err(store_error))
                .collect::<Result<Vec<TaskId>, TaskError>>()?;
            return Err(TaskError::Cycle(ids));
        }
        sql(tx.execute(
            "INSERT OR IGNORE INTO task_block (blocked, blocker) VALUES (?1, ?2)",
            params![blocked, blocker],
        ))?;
        sql(tx.commit())
    }

    fn unblock_now(&self, blocked: TaskId, blocker: TaskId) -> Result<(), TaskError> {
        let db = self.db();
        exists(&db, blocked)?;
        sql(db.execute(
            "DELETE FROM task_block WHERE blocked = ?1 AND blocker = ?2",
            params![blocked, blocker],
        ))?;
        Ok(())
    }

    fn roll_over_now(&self, today: Date) -> Result<Vec<TaskId>, TaskError> {
        let db = self.db();
        let mut query = sql(db.prepare(
            "UPDATE task SET rolled = rolled + 1
             WHERE status = 'open' AND day < ?1 RETURNING id",
        ))?;
        let rows = sql(query.query_map([date_text(today)], |r| r.get(0)))?;
        let mut ids: Vec<TaskId> = sql(rows.collect())?;
        ids.sort_unstable();
        Ok(ids)
    }

    fn snapshot_now(&self, today: Date, day_began_ms: i64) -> Result<Vec<Task>, TaskError> {
        let db = self.db();
        let today = date_text(today);
        let mut query = sql(db.prepare(&format!(
            "{WANTED} SELECT {TASK_COLUMNS} FROM task WHERE id IN (SELECT id FROM wanted)
             ORDER BY id"
        )))?;
        let raw: Vec<RawTask> =
            sql(sql(query.query_map(params![today, day_began_ms], RawTask::from_row))?.collect())?;
        let mut tasks = raw
            .into_iter()
            .map(RawTask::into_task)
            .collect::<Result<Vec<Task>, TaskError>>()?;

        let mut links = sql(db.prepare(&format!(
            "{WANTED} SELECT blocked, blocker FROM task_block
             WHERE blocked IN (SELECT id FROM wanted) ORDER BY blocked, blocker"
        )))?;
        let pairs: Vec<(TaskId, TaskId)> = sql(sql(
            links.query_map(params![today, day_began_ms], |r| Ok((r.get(0)?, r.get(1)?)))
        )?
        .collect())?;
        let mut blocked_by: HashMap<TaskId, Vec<TaskId>> = HashMap::new();
        for (blocked, blocker) in pairs {
            blocked_by.entry(blocked).or_default().push(blocker);
        }
        for task in &mut tasks {
            task.blocked_by = blocked_by.remove(&task.id).unwrap_or_default();
        }
        Ok(tasks)
    }
}

impl TaskStore for SqliteTaskStore {
    async fn add(&self, task: NewTask) -> Result<TaskId, TaskError> {
        self.add_now(&task)
    }

    async fn update(&self, id: TaskId, change: TaskUpdate) -> Result<(), TaskError> {
        self.update_now(id, &change)
    }

    async fn finish(&self, id: TaskId, how: Finish, at_ms: i64) -> Result<(), TaskError> {
        self.finish_now(id, how, at_ms)
    }

    async fn remove(&self, id: TaskId) -> Result<(), TaskError> {
        self.remove_now(id)
    }

    async fn upsert_mirror(
        &self,
        task: NewTask,
        closed_at: Option<i64>,
    ) -> Result<TaskId, TaskError> {
        self.upsert_mirror_now(&task, closed_at)
    }

    async fn block(&self, blocked: TaskId, blocker: TaskId) -> Result<(), TaskError> {
        self.block_now(blocked, blocker)
    }

    async fn unblock(&self, blocked: TaskId, blocker: TaskId) -> Result<(), TaskError> {
        self.unblock_now(blocked, blocker)
    }

    async fn roll_over(&self, today: Date) -> Result<Vec<TaskId>, TaskError> {
        self.roll_over_now(today)
    }

    async fn snapshot(&self, today: Date, day_began_ms: i64) -> Result<Vec<Task>, TaskError> {
        self.snapshot_now(today, day_began_ms)
    }
}

// --- The job table ----------------------------------------------------------------------------

const WEEK: [Weekday; 7] = [
    Weekday::Monday,
    Weekday::Tuesday,
    Weekday::Wednesday,
    Weekday::Thursday,
    Weekday::Friday,
    Weekday::Saturday,
    Weekday::Sunday,
];

fn weekdays_text(days: WeekdaySet) -> String {
    WEEK.iter()
        .map(|d| if days.contains(*d) { '1' } else { '0' })
        .collect()
}

fn parse_weekdays(text: &str) -> Result<WeekdaySet, TaskError> {
    if text.len() != 7 || !text.bytes().all(|b| b == b'0' || b == b'1') {
        return Err(store_error(format!("bad weekdays {text:?}")));
    }
    let mut days = WeekdaySet::EMPTY;
    for (day, bit) in WEEK.iter().zip(text.bytes()) {
        if bit == b'1' {
            days.insert(*day);
        }
    }
    Ok(days)
}

fn schedule_text(schedule: &Schedule) -> String {
    match *schedule {
        Schedule::At { time, weekdays } => {
            format!("at {} {}", time_text(time), weekdays_text(weekdays))
        }
        Schedule::EveryWhilePresent { minutes } => format!("every {minutes}"),
        Schedule::OnPresenceAfter {
            from,
            latest,
            weekdays,
        } => format!(
            "presence {} {} {}",
            time_text(from),
            time_text(latest),
            weekdays_text(weekdays)
        ),
    }
}

fn parse_schedule(text: &str) -> Result<Schedule, TaskError> {
    let parts: Vec<&str> = text.split(' ').collect();
    match parts.as_slice() {
        ["at", time, days] => Ok(Schedule::At {
            time: parse_time(time)?,
            weekdays: parse_weekdays(days)?,
        }),
        ["every", minutes] => Ok(Schedule::EveryWhilePresent {
            minutes: minutes.parse().map_err(store_error)?,
        }),
        ["presence", from, latest, days] => Ok(Schedule::OnPresenceAfter {
            from: parse_time(from)?,
            latest: parse_time(latest)?,
            weekdays: parse_weekdays(days)?,
        }),
        _ => Err(store_error(format!("bad schedule {text:?}"))),
    }
}

fn kind_text(kind: &JobKind) -> String {
    match kind {
        JobKind::RollOver => "rollover".into(),
        JobKind::Morning => "morning".into(),
        JobKind::Evening => "evening".into(),
        JobKind::Nudge => "nudge".into(),
        JobKind::Sync(connector) => format!("sync:{connector}"),
        JobKind::Prompt => "prompt".into(),
    }
}

fn parse_kind(text: &str) -> Result<JobKind, TaskError> {
    Ok(match text {
        "rollover" => JobKind::RollOver,
        "morning" => JobKind::Morning,
        "evening" => JobKind::Evening,
        "nudge" => JobKind::Nudge,
        "prompt" => JobKind::Prompt,
        other => match other.strip_prefix("sync:") {
            Some(connector) => JobKind::Sync(connector.to_owned()),
            None => return Err(store_error(format!("bad job kind {other:?}"))),
        },
    })
}

fn status_text_of(status: &JobStatus) -> String {
    match status {
        JobStatus::Ok => "ok".into(),
        JobStatus::Failed(message) => format!("failed: {message}"),
    }
}

fn parse_status(text: &str) -> JobStatus {
    match text.strip_prefix("failed: ") {
        Some(message) => JobStatus::Failed(message.to_owned()),
        None => JobStatus::Ok,
    }
}

fn unix_ms(at: UtcDateTime) -> i64 {
    (at.unix_timestamp_nanos() / 1_000_000) as i64
}

fn from_unix_ms(ms: i64) -> Result<UtcDateTime, TaskError> {
    UtcDateTime::from_unix_timestamp_nanos(i128::from(ms) * 1_000_000).map_err(store_error)
}

/// A job row as stored, before its text is parsed.
type RawJob = (
    String,
    String,
    String,
    bool,
    i64,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<String>,
    u32,
    Option<String>,
);

fn job_from_raw(raw: RawJob) -> Result<Job, TaskError> {
    let (
        name,
        kind,
        schedule,
        enabled,
        catch_up_min,
        allowlist,
        prompt,
        last_run,
        status,
        fails,
        day,
    ) = raw;
    Ok(Job {
        name,
        kind: parse_kind(&kind)?,
        schedule: parse_schedule(&schedule)?,
        enabled,
        catch_up: SignedDuration::minutes(catch_up_min),
        allowlist: allowlist
            .filter(|a| !a.is_empty())
            .map(|a| a.split(',').map(str::to_owned).collect())
            .unwrap_or_default(),
        prompt,
        last_run: last_run.map(from_unix_ms).transpose()?,
        last_status: status.as_deref().map(parse_status),
        fails_today: fails,
        fails_day: day.as_deref().map(parse_date).transpose()?,
    })
}

impl SqliteTaskStore {
    fn jobs_now(&self) -> Result<Vec<Job>, TaskError> {
        let db = self.db();
        let mut query = sql(db.prepare(
            "SELECT name, kind, schedule, enabled, catch_up_min, allowlist, prompt, last_run,
                    last_status, fails_today, fails_day
             FROM job ORDER BY name",
        ))?;
        let raw: Vec<RawJob> = sql(sql(query.query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get(8)?,
                r.get(9)?,
                r.get(10)?,
            ))
        }))?
        .collect())?;
        raw.into_iter().map(job_from_raw).collect()
    }

    fn save_job_now(&self, job: &Job) -> Result<(), TaskError> {
        sql(self.db().execute(
            "INSERT OR REPLACE INTO job (name, kind, schedule, enabled, catch_up_min, allowlist,
                                         prompt, last_run, last_status, fails_today, fails_day)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                job.name,
                kind_text(&job.kind),
                schedule_text(&job.schedule),
                job.enabled,
                job.catch_up.whole_minutes(),
                job.allowlist.join(","),
                job.prompt,
                job.last_run.map(unix_ms),
                job.last_status.as_ref().map(status_text_of),
                job.fails_today,
                job.fails_day.map(date_text),
            ],
        ))?;
        Ok(())
    }
}

impl JobStore for SqliteTaskStore {
    async fn jobs(&self) -> Result<Vec<Job>, TaskError> {
        self.jobs_now()
    }

    async fn save_job(&self, job: &Job) -> Result<(), TaskError> {
        self.save_job_now(job)
    }
}

/// The request counter of connectors (R-20), in the same file as the task list.
impl RequestLog for SqliteTaskStore {
    fn record(&self, account: &str, day: Date, server: &str, units: u32) -> Result<(), String> {
        self.db()
            .execute(
                "INSERT INTO connector_requests (account, day, server, calls, units)
                 VALUES (?1, ?2, ?3, 1, ?4)
                 ON CONFLICT (account, day, server)
                 DO UPDATE SET calls = calls + 1, units = units + excluded.units",
                params![account, date_text(day), server, units],
            )
            .map(drop)
            .map_err(|error| format!("connector requests: {error}"))
    }

    fn usage(&self, account: &str, day: Date, server: &str) -> Result<Usage, String> {
        self.db()
            .query_row(
                "SELECT calls, units FROM connector_requests
                 WHERE account = ?1 AND day = ?2 AND server = ?3",
                params![account, date_text(day), server],
                |row| {
                    Ok(Usage {
                        calls: row.get(0)?,
                        units: row.get(1)?,
                    })
                },
            )
            .optional()
            .map(Option::unwrap_or_default)
            .map_err(|error| format!("connector requests: {error}"))
    }
}

/// Disconnect deletes every row kept from the account (R-12, AC-6).
impl AccountData for SqliteTaskStore {
    fn delete_account(&self, account: &str) -> Result<(), String> {
        let db = self.db();
        for table in [
            "connector_requests",
            "events",
            "mail_meta",
            "connector_sign_in",
        ] {
            db.execute(
                &format!("DELETE FROM {table} WHERE account = ?1"),
                params![account],
            )
            .map_err(|error| format!("{table}: {error}"))?;
        }
        Ok(())
    }
}

fn unix(at: UtcDateTime) -> i64 {
    (at - UtcDateTime::UNIX_EPOCH).whole_seconds()
}

fn from_unix(seconds: i64) -> rusqlite::Result<UtcDateTime> {
    UtcDateTime::from_unix_timestamp(seconds).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Integer, Box::new(e))
    })
}

fn events_error(error: rusqlite::Error) -> String {
    format!("events: {error}")
}

/// The calendar cache (specs/97-google-connectors, R-19), in the app's one SQLite file.
impl EventStore for SqliteTaskStore {
    fn replace(
        &self,
        account: &str,
        from: UtcDateTime,
        to: UtcDateTime,
        events: &[Event],
    ) -> Result<(), String> {
        let mut db = self.db();
        let tx = db.transaction().map_err(events_error)?;
        tx.execute(
            "DELETE FROM events WHERE account = ?1 AND starts_at >= ?2 AND starts_at < ?3",
            params![account, unix(from), unix(to)],
        )
        .map_err(events_error)?;
        for e in events {
            tx.execute(
                "INSERT OR REPLACE INTO events
                 (account, id, calendar, title, starts_at, ends_at, all_day, url, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    account,
                    e.id,
                    e.calendar,
                    e.title,
                    unix(e.start),
                    unix(e.end),
                    e.all_day,
                    e.url,
                    e.status
                ],
            )
            .map_err(events_error)?;
        }
        tx.commit().map_err(events_error)
    }

    fn between(
        &self,
        account: Option<&str>,
        from: UtcDateTime,
        to: UtcDateTime,
    ) -> Result<Vec<Event>, String> {
        let db = self.db();
        let mut statement = db
            .prepare(
                "SELECT account, id, calendar, title, starts_at, ends_at, all_day, url, status
                 FROM events
                 WHERE (?1 IS NULL OR account = ?1) AND starts_at >= ?2 AND starts_at < ?3
                 ORDER BY starts_at, id",
            )
            .map_err(events_error)?;
        let rows = statement
            .query_map(params![account, unix(from), unix(to)], |row| {
                Ok(Event {
                    account: row.get(0)?,
                    id: row.get(1)?,
                    calendar: row.get(2)?,
                    title: row.get(3)?,
                    start: from_unix(row.get(4)?)?,
                    end: from_unix(row.get(5)?)?,
                    all_day: row.get(6)?,
                    url: row.get(7)?,
                    status: row.get(8)?,
                })
            })
            .map_err(events_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(events_error)
    }

    fn retain(&self, now: UtcDateTime) -> Result<usize, String> {
        self.db()
            .execute(
                "DELETE FROM events WHERE ends_at < ?1 OR starts_at >= ?2",
                params![
                    unix(now - hyperme_core::sync::KEEP_BEHIND),
                    unix(now + hyperme_core::sync::KEEP_AHEAD)
                ],
            )
            .map_err(events_error)
    }
}

fn mail_error(error: rusqlite::Error) -> String {
    format!("mail: {error}")
}

/// The inbox metadata cache (specs/97-google-connectors, R-18, R-19).
impl MailStore for SqliteTaskStore {
    fn upsert(&self, threads: &[MailMeta]) -> Result<(), String> {
        let mut db = self.db();
        let tx = db.transaction().map_err(mail_error)?;
        for t in threads {
            tx.execute(
                "INSERT OR REPLACE INTO mail_meta
                 (account, id, subject, sender, snippet, received_at, labels, to_answer)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    t.account,
                    t.id,
                    t.subject,
                    t.sender,
                    t.snippet,
                    unix(t.received_at),
                    t.labels.join("\n"),
                    t.to_answer
                ],
            )
            .map_err(mail_error)?;
        }
        tx.commit().map_err(mail_error)
    }

    fn all(&self, account: Option<&str>) -> Result<Vec<MailMeta>, String> {
        let db = self.db();
        let mut statement = db
            .prepare(
                "SELECT account, id, subject, sender, snippet, received_at, labels, to_answer
                 FROM mail_meta WHERE (?1 IS NULL OR account = ?1)
                 ORDER BY received_at DESC, id",
            )
            .map_err(mail_error)?;
        let rows = statement
            .query_map(params![account], |row| {
                let labels: String = row.get(6)?;
                Ok(MailMeta {
                    account: row.get(0)?,
                    id: row.get(1)?,
                    subject: row.get(2)?,
                    sender: row.get(3)?,
                    snippet: row.get(4)?,
                    received_at: from_unix(row.get(5)?)?,
                    labels: labels.lines().map(str::to_owned).collect(),
                    to_answer: row.get(7)?,
                })
            })
            .map_err(mail_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(mail_error)
    }

    fn retain(&self, now: UtcDateTime) -> Result<usize, String> {
        self.db()
            .execute(
                "DELETE FROM mail_meta WHERE received_at < ?1",
                params![unix(now - hyperme_core::sync::KEEP_MAIL)],
            )
            .map_err(mail_error)
    }
}

/// Accounts marked "Sign in again" (specs/97-google-connectors, R-10, AC-4).
impl AccountStatusStore for SqliteTaskStore {
    fn sign_in_needed(&self, account: &str) -> Result<bool, String> {
        self.db()
            .query_row(
                "SELECT 1 FROM connector_sign_in WHERE account = ?1",
                params![account],
                |_| Ok(()),
            )
            .optional()
            .map(|found| found.is_some())
            .map_err(|error| format!("sign-in state: {error}"))
    }

    fn mark_sign_in_needed(&self, account: &str, at: UtcDateTime) -> Result<bool, String> {
        self.db()
            .execute(
                "INSERT OR IGNORE INTO connector_sign_in (account, since) VALUES (?1, ?2)",
                params![account, unix(at)],
            )
            .map(|inserted| inserted == 1)
            .map_err(|error| format!("sign-in state: {error}"))
    }

    fn clear(&self, account: &str) -> Result<(), String> {
        self.db()
            .execute(
                "DELETE FROM connector_sign_in WHERE account = ?1",
                params![account],
            )
            .map(drop)
            .map_err(|error| format!("sign-in state: {error}"))
    }
}
