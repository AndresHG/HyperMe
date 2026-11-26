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

use hyperme_core::tasks::{
    Due, Finish, NewTask, Source, Status, Task, TaskError, TaskId, TaskStore, TaskUpdate,
};
use hyperme_core::traits::{Error, ErrorKind};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use time::{Date, Month, Time};

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
