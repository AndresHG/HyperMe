//! The trace store (specs/182-trace-store, slice 2): every dictation's trace, numbers and names
//! only, in two tables of the history's SQLite file, written on a thread of its own so a
//! dictation never waits for the disk (R-7, R-11).
//!
//! Retention (R-9, R-9a): with history on, a trace goes with its history item, and after the
//! history's retention days; with history off, after 30 days. With Keep timing traces off,
//! nothing is written.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, SyncSender, channel, sync_channel};
use std::sync::{Arc, Mutex, PoisonError};

use hyperme_core::config::Retention;
use hyperme_core::trace::{FieldValue, FinishedTrace, TraceSink};
use hyperme_core::traits::{Error, ErrorKind};
use rusqlite::{Connection, params};
use serde_json::{Map, Value};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// With history off, traces are kept this many days (TS-1).
const DAYS_WITHOUT_HISTORY: i64 = 30;

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS trace (
        id INTEGER PRIMARY KEY,
        history_id INTEGER,
        kind TEXT NOT NULL,
        started_ms INTEGER NOT NULL,
        duration_us INTEGER NOT NULL,
        outcome TEXT NOT NULL,
        speech_provider TEXT,
        speech_model TEXT,
        cleanup_provider TEXT,
        cleanup_model TEXT,
        cleanup_level TEXT
    );
    CREATE TABLE IF NOT EXISTS span (
        trace_id INTEGER NOT NULL REFERENCES trace(id) ON DELETE CASCADE,
        idx INTEGER NOT NULL,
        parent INTEGER,
        name TEXT NOT NULL,
        start_us INTEGER NOT NULL,
        duration_us INTEGER NOT NULL,
        fields TEXT NOT NULL,
        events TEXT NOT NULL,
        PRIMARY KEY (trace_id, idx)
    );
    CREATE INDEX IF NOT EXISTS trace_history ON trace (history_id);
    CREATE INDEX IF NOT EXISTS trace_started ON trace (started_ms);";

/// The providers, models and cleanup level the app is set to, recorded with each trace. Names
/// from the config only, never text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Labels {
    pub speech_provider: String,
    pub speech_model: String,
    pub cleanup_provider: String,
    pub cleanup_model: String,
    pub cleanup_level: String,
}

/// What the store keeps, from the config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceSettings {
    /// `history.keep_traces` (R-9a).
    pub keep: bool,
    /// `history.enabled`.
    pub history: bool,
    /// `history.retention_days`.
    pub retention: Retention,
}

impl TraceSettings {
    pub fn from_config(history: &hyperme_core::config::History) -> Self {
        TraceSettings {
            keep: history.keep_traces,
            history: history.enabled,
            retention: history.retention_days,
        }
    }
}

/// One stored span.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredSpan {
    pub name: String,
    pub parent: Option<usize>,
    pub start_us: u64,
    pub duration_us: u64,
    pub fields: Map<String, Value>,
    pub events: Map<String, Value>,
}

/// One stored trace, its root first.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredTrace {
    pub history_id: Option<i64>,
    pub started_ms: i64,
    pub outcome: String,
    pub labels: Labels,
    pub spans: Vec<StoredSpan>,
}

/// The median and 90th percentile of one stage, for one provider and model where the stage has
/// one (speech and cleanup), over the traces of a window (R-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageStat {
    pub stage: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub count: usize,
    pub p50_us: u64,
    pub p90_us: u64,
}

enum Job {
    Write(Box<FinishedTrace>, Labels, i64, TraceSettings),
    Cleanup(TraceSettings, i64),
    Flush(SyncSender<()>),
}

#[derive(Debug, Clone)]
struct State {
    labels: Labels,
    settings: TraceSettings,
}

/// The trace store: a [`TraceSink`] that hands each trace to a writer thread, and the queries
/// Diagnostics and Settings read.
pub struct TraceStore {
    path: PathBuf,
    jobs: Mutex<Sender<Job>>,
    state: Mutex<State>,
    now: Arc<dyn Fn() -> i64 + Send + Sync>,
}

impl fmt::Debug for TraceStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TraceStore")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

fn error(what: impl fmt::Display) -> Error {
    Error {
        kind: ErrorKind::Device,
        message: format!("traces: {what}"),
    }
}

fn system_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

fn connect(path: &Path) -> Result<Connection, Error> {
    let db = Connection::open(path).map_err(|e| error(format!("opening: {e}")))?;
    db.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")
        .map_err(|e| error(format!("opening: {e}")))?;
    db.busy_timeout(std::time::Duration::from_secs(2))
        .map_err(|e| error(format!("opening: {e}")))?;
    db.execute_batch(SCHEMA)
        .map_err(|e| error(format!("creating tables: {e}")))?;
    Ok(db)
}

impl TraceStore {
    /// Opens the store in the history's SQLite file at `path`, creating its tables.
    pub fn open(path: &Path, settings: TraceSettings) -> Result<Self, Error> {
        Self::open_with_clock(path, settings, system_now_ms)
    }

    /// As [`open`](Self::open), with a clock in milliseconds since the Unix epoch, for tests.
    pub fn open_with_clock(
        path: &Path,
        settings: TraceSettings,
        now: impl Fn() -> i64 + Send + Sync + 'static,
    ) -> Result<Self, Error> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| error(format!("creating folder: {e}")))?;
        }
        let db = connect(path)?;
        let (jobs, rx) = channel();
        std::thread::Builder::new()
            .name("hyperme-traces".to_owned())
            .spawn(move || write_loop(db, rx))
            .map_err(|e| error(format!("starting the writer: {e}")))?;
        let store = TraceStore {
            path: path.to_owned(),
            jobs: Mutex::new(jobs),
            state: Mutex::new(State {
                labels: Labels::default(),
                settings,
            }),
            now: Arc::new(now),
        };
        store.cleanup();
        Ok(store)
    }

    fn send(&self, job: Job) {
        // A stopped writer only loses traces; the dictation never notices (R-11).
        let _ = self
            .jobs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .send(job);
    }

    fn state(&self) -> State {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The providers and models recorded with the next traces, after a provider switch.
    pub fn set_labels(&self, labels: Labels) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .labels = labels;
    }

    /// New settings from the config; applies retention at once.
    pub fn set_settings(&self, settings: TraceSettings) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .settings = settings;
        self.cleanup();
    }

    /// Applies retention (R-9): call after history items are deleted.
    pub fn cleanup(&self) {
        let settings = self.state().settings;
        self.send(Job::Cleanup(settings, (self.now)()));
    }

    /// Waits until every trace handed over so far is written.
    pub fn flush(&self) {
        let (done, wait) = sync_channel(1);
        self.send(Job::Flush(done));
        let _ = wait.recv();
    }

    fn reader(&self) -> Result<Connection, Error> {
        connect(&self.path)
    }

    /// The trace of one history item, if it has one (R-10).
    pub fn trace_for(&self, history_id: i64) -> Result<Option<StoredTrace>, Error> {
        let db = self.reader()?;
        let row = db.query_row(
            "SELECT id, started_ms, outcome, speech_provider, speech_model, cleanup_provider,
                    cleanup_model, cleanup_level
             FROM trace WHERE history_id = ?1 ORDER BY id DESC LIMIT 1",
            params![history_id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    labels_from(r, 3)?,
                ))
            },
        );
        let (id, started_ms, outcome, labels) = match row {
            Ok(found) => found,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(error(e)),
        };
        let mut query = db
            .prepare(
                "SELECT name, parent, start_us, duration_us, fields, events FROM span
                 WHERE trace_id = ?1 ORDER BY idx",
            )
            .map_err(error)?;
        let spans = query
            .query_map(params![id], |r| {
                Ok(StoredSpan {
                    name: r.get(0)?,
                    parent: r.get::<_, Option<i64>>(1)?.map(|p| p as usize),
                    start_us: r.get::<_, i64>(2)? as u64,
                    duration_us: r.get::<_, i64>(3)? as u64,
                    fields: json_map(&r.get::<_, String>(4)?),
                    events: json_map(&r.get::<_, String>(5)?),
                })
            })
            .map_err(error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?;
        Ok(Some(StoredTrace {
            history_id: Some(history_id),
            started_ms,
            outcome,
            labels,
            spans,
        }))
    }

    /// The median and p90 of each stage over the traces started at or after `since_ms`, by
    /// provider and model for speech and cleanup (R-10). Stages: `mic_start`, `speech`,
    /// `cleanup`, `insert`, and `total` (key release, or a retry's start, to text ready) for
    /// pasted dictations.
    pub fn stage_percentiles(&self, since_ms: i64) -> Result<Vec<StageStat>, Error> {
        let db = self.reader()?;
        let mut query = db
            .prepare(
                "SELECT s.name, s.duration_us, t.speech_provider, t.speech_model,
                        t.cleanup_provider, t.cleanup_model
                 FROM span s JOIN trace t ON t.id = s.trace_id
                 WHERE t.started_ms >= ?1 AND s.name IN ('mic_start', 'speech', 'cleanup', 'insert')",
            )
            .map_err(error)?;
        let rows = query
            .query_map(params![since_ms], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)? as u64,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                ))
            })
            .map_err(error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?;
        let mut groups: std::collections::BTreeMap<
            (String, Option<String>, Option<String>),
            Vec<u64>,
        > = Default::default();
        for (stage, us, sp, sm, cp, cm) in rows {
            let (provider, model) = match stage.as_str() {
                "speech" => (sp, sm),
                "cleanup" => (cp, cm),
                _ => (None, None),
            };
            groups.entry((stage, provider, model)).or_default().push(us);
        }
        Ok(groups
            .into_iter()
            .map(|((stage, provider, model), mut values)| {
                values.sort_unstable();
                StageStat {
                    stage,
                    provider,
                    model,
                    count: values.len(),
                    p50_us: percentile(&values, 50),
                    p90_us: percentile(&values, 90),
                }
            })
            .collect())
    }

    /// How many traces the store holds.
    pub fn count(&self) -> Result<usize, Error> {
        let db = self.reader()?;
        db.query_row("SELECT COUNT(*) FROM trace", [], |r| r.get::<_, i64>(0))
            .map(|n| n as usize)
            .map_err(error)
    }
}

/// The nearest-rank percentile of sorted values.
fn percentile(sorted: &[u64], p: usize) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (p * sorted.len()).div_ceil(100).max(1);
    sorted[rank.min(sorted.len()) - 1]
}

fn labels_from(r: &rusqlite::Row<'_>, from: usize) -> rusqlite::Result<Labels> {
    let text = |i: usize| -> rusqlite::Result<String> {
        Ok(r.get::<_, Option<String>>(from + i)?.unwrap_or_default())
    };
    Ok(Labels {
        speech_provider: text(0)?,
        speech_model: text(1)?,
        cleanup_provider: text(2)?,
        cleanup_model: text(3)?,
        cleanup_level: text(4)?,
    })
}

fn json_map(text: &str) -> Map<String, Value> {
    match serde_json::from_str(text) {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    }
}

impl TraceSink for TraceStore {
    fn finished(&self, trace: &FinishedTrace) {
        let State { labels, settings } = self.state();
        if !settings.keep {
            return;
        }
        let now = (self.now)();
        self.send(Job::Write(Box::new(trace.clone()), labels, now, settings));
    }
}

fn write_loop(mut db: Connection, jobs: Receiver<Job>) {
    while let Ok(job) = jobs.recv() {
        let result = match job {
            Job::Write(trace, labels, now, settings) => {
                write(&mut db, &trace, &labels, now).and_then(|()| retain(&db, now, settings))
            }
            Job::Cleanup(settings, now) => retain(&db, now, settings),
            Job::Flush(done) => {
                let _ = done.send(());
                Ok(())
            }
        };
        // R-11: a failed write is a warning in the warnings file, never a failed dictation;
        // the trace is dropped.
        if let Err(e) = result {
            tracing::warn!("trace store: {}", e.message);
        }
    }
}

fn write(
    db: &mut Connection,
    trace: &FinishedTrace,
    labels: &Labels,
    now: i64,
) -> Result<(), Error> {
    let root = trace.root();
    let history_id = root.u64("history_id").map(|v| v as i64);
    let outcome = root.name("outcome").unwrap_or("cancelled").to_owned();
    let started_ms = now - (root.duration_us / 1000) as i64;
    let tx = db.transaction().map_err(error)?;
    tx.execute(
        "INSERT INTO trace (history_id, kind, started_ms, duration_us, outcome, speech_provider,
             speech_model, cleanup_provider, cleanup_model, cleanup_level)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            history_id,
            root.name,
            started_ms,
            root.duration_us as i64,
            outcome,
            labels.speech_provider,
            labels.speech_model,
            labels.cleanup_provider,
            labels.cleanup_model,
            labels.cleanup_level,
        ],
    )
    .map_err(error)?;
    let id = tx.last_insert_rowid();
    for (idx, span) in trace.spans.iter().enumerate() {
        let fields: Map<String, Value> = span
            .fields
            .iter()
            .map(|(name, value)| {
                let value = match value {
                    FieldValue::U64(v) => Value::from(*v),
                    FieldValue::Bool(v) => Value::from(*v),
                    FieldValue::Name(v) => Value::from(v.as_str()),
                };
                ((*name).to_owned(), value)
            })
            .collect();
        let events: Map<String, Value> = span
            .events
            .iter()
            .map(|(name, at)| ((*name).to_owned(), Value::from(*at)))
            .collect();
        tx.execute(
            "INSERT INTO span (trace_id, idx, parent, name, start_us, duration_us, fields, events)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                idx as i64,
                span.parent.map(|p| p as i64),
                span.name,
                span.start_us as i64,
                span.duration_us as i64,
                Value::Object(fields).to_string(),
                Value::Object(events).to_string(),
            ],
        )
        .map_err(error)?;
    }
    tx.commit().map_err(error)
}

/// R-9, R-9a: drops traces past their time, and, with history on, traces whose history item was
/// deleted. A cancelled dictation's item is discarded at once, so its trace is kept by age (R-8).
fn retain(db: &Connection, now: i64, settings: TraceSettings) -> Result<(), Error> {
    let days = if settings.history {
        match settings.retention {
            Retention::Days(days) => Some(i64::from(days)),
            Retention::Unlimited => None,
        }
    } else {
        Some(DAYS_WITHOUT_HISTORY)
    };
    if let Some(days) = days {
        db.execute(
            "DELETE FROM trace WHERE started_ms < ?1",
            params![now - days * DAY_MS],
        )
        .map_err(error)?;
    }
    if settings.history {
        // The dictations table belongs to the history, in the same file.
        let has_history: bool = db
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'dictations'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map_err(error)?
            > 0;
        if has_history {
            db.execute(
                "DELETE FROM trace WHERE history_id IS NOT NULL
                 AND outcome IN ('pasted', 'copied', 'failed')
                 AND history_id NOT IN (SELECT id FROM dictations)",
                [],
            )
            .map_err(error)?;
        }
    }
    Ok(())
}
