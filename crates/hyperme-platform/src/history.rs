//! The dictation history on disk (design.md, "History"; R-18, R-28, R-31): one WAV file per
//! dictation, written while it is captured, and a SQLite index. Portable code: the app passes
//! the folder (`~/Library/Application Support/HyperMe/history/` on macOS).
//!
//! WAV (24 kHz mono PCM16, about 2.8 MB a minute) needs no codec and is still readable after a
//! crash; 1 GB holds about six hours of dictation.
//!
//! Cleanup runs at open and after each dictation:
//! - history on: items past the retention period go, then the oldest items until the total is
//!   under the size limit, done items before failed ones;
//! - history off: a successful dictation is deleted at once, and a failed one after 24 hours.

use std::collections::HashMap;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use hyperme_core::config::{self, Retention};
use hyperme_core::traits::{DictationId, Error, ErrorKind, History};
use rusqlite::{Connection, OptionalExtension, params};

const SAMPLE_RATE: u32 = 24_000;
const HEADER_BYTES: u64 = 44;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// With history off, a failed dictation's audio waits this long for a retry (R-31).
const FAILED_KEPT_MS: i64 = DAY_MS;

/// What the history keeps, from the config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistorySettings {
    pub enabled: bool,
    pub retention: Retention,
    pub size_limit_bytes: u64,
}

impl HistorySettings {
    pub fn from_config(history: &config::History) -> Self {
        HistorySettings {
            enabled: history.enabled,
            retention: history.retention_days,
            size_limit_bytes: history.size_limit_mb * 1024 * 1024,
        }
    }
}

/// One dictation in the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: DictationId,
    pub started_ms: i64,
    pub app: Option<String>,
    pub language: Option<String>,
    pub text: Option<String>,
    /// `recording`, `done` or `failed`.
    pub state: String,
    pub audio_bytes: u64,
    /// The audio file is still on disk and holds more than a header, so the item can be
    /// retried (#98 R-16).
    pub has_audio: bool,
    /// Why a failed dictation failed (#98 R-11).
    pub message: Option<String>,
}

/// The columns [`item_from_row`] reads, in order.
const ITEM_COLUMNS: &str = "id, started_ms, app, language, text, state, audio_bytes, message";

/// An item from a row of [`ITEM_COLUMNS`], before its audio is checked.
fn item_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Item> {
    Ok(Item {
        id: r.get(0)?,
        started_ms: r.get(1)?,
        app: r.get(2)?,
        language: r.get(3)?,
        text: r.get(4)?,
        state: r.get(5)?,
        audio_bytes: r.get::<_, i64>(6)? as u64,
        has_audio: false,
        message: r.get(7)?,
    })
}

/// Lower case without accents, for search (#98 R-12): "Canción" and "CANCION" both fold to
/// "cancion". A small map of the Latin letters Spanish and English text meets; other letters
/// are only lower-cased.
fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' => 'a',
            'ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' | 'ē' => 'e',
            'ì' | 'í' | 'î' | 'ï' | 'ī' => 'i',
            'ñ' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' => 'o',
            'ù' | 'ú' | 'û' | 'ü' | 'ū' => 'u',
            'ý' | 'ÿ' => 'y',
            c => c,
        })
        .collect()
}

/// The history store.
pub struct SqliteHistory {
    dir: PathBuf,
    db: Connection,
    settings: HistorySettings,
    /// Open audio files of dictations still recording.
    writers: HashMap<DictationId, File>,
    now: Box<dyn Fn() -> i64 + Send>,
}

impl fmt::Debug for SqliteHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqliteHistory")
            .field("dir", &self.dir)
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

fn error(what: impl fmt::Display) -> Error {
    Error {
        kind: ErrorKind::Device,
        message: format!("history: {what}"),
    }
}

fn system_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// A WAV header for `data_bytes` of 24 kHz mono PCM16.
fn wav_header(data_bytes: u32) -> [u8; HEADER_BYTES as usize] {
    let mut h = [0u8; HEADER_BYTES as usize];
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&(36 + data_bytes).to_le_bytes());
    h[8..16].copy_from_slice(b"WAVEfmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes());
    h[20..22].copy_from_slice(&1u16.to_le_bytes()); // PCM
    h[22..24].copy_from_slice(&1u16.to_le_bytes()); // mono
    h[24..28].copy_from_slice(&SAMPLE_RATE.to_le_bytes());
    h[28..32].copy_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    h[32..34].copy_from_slice(&2u16.to_le_bytes());
    h[34..36].copy_from_slice(&16u16.to_le_bytes());
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&data_bytes.to_le_bytes());
    h
}

impl SqliteHistory {
    /// Opens or creates the history in `dir`.
    pub fn open(dir: &Path, settings: HistorySettings) -> Result<Self, Error> {
        Self::open_with_clock(dir, settings, system_now_ms)
    }

    /// As [`open`](Self::open), with a clock in milliseconds since the Unix epoch, for tests.
    pub fn open_with_clock(
        dir: &Path,
        settings: HistorySettings,
        now: impl Fn() -> i64 + Send + 'static,
    ) -> Result<Self, Error> {
        std::fs::create_dir_all(dir.join("audio"))
            .map_err(|e| error(format!("creating {}: {e}", dir.display())))?;
        let db = Connection::open(dir.join("history.sqlite3"))
            .map_err(|e| error(format!("opening the index: {e}")))?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS dictations (
                id INTEGER PRIMARY KEY,
                started_ms INTEGER NOT NULL,
                finished_ms INTEGER,
                app TEXT,
                language TEXT,
                text TEXT,
                state TEXT NOT NULL CHECK (state IN ('recording', 'done', 'failed')),
                message TEXT,
                audio_bytes INTEGER NOT NULL DEFAULT 0
            );",
        )
        .map_err(|e| error(format!("creating the index: {e}")))?;
        db.execute_batch(
            "CREATE INDEX IF NOT EXISTS dictations_started ON dictations (started_ms, id);",
        )
        .map_err(|e| error(format!("creating the index: {e}")))?;
        let mut store = SqliteHistory {
            dir: dir.to_owned(),
            db,
            settings,
            writers: HashMap::new(),
            now: Box::new(now),
        };
        store.recover()?;
        store.cleanup()?;
        Ok(store)
    }

    fn audio_path(&self, id: DictationId) -> PathBuf {
        self.dir.join("audio").join(format!("{id}.wav"))
    }

    fn sql<T>(result: rusqlite::Result<T>) -> Result<T, Error> {
        result.map_err(|e| error(format!("index: {e}")))
    }

    /// A dictation still `recording` from an earlier run was cut off by a crash or a quit: its
    /// audio is on disk, so it becomes a failed dictation that can be retried (R-31).
    fn recover(&mut self) -> Result<(), Error> {
        let ids: Vec<DictationId> = {
            let mut query = Self::sql(
                self.db
                    .prepare("SELECT id FROM dictations WHERE state = 'recording'"),
            )?;
            let rows = Self::sql(query.query_map([], |row| row.get(0)))?;
            Self::sql(rows.collect())?
        };
        for id in ids {
            self.finalize(id)?;
            let now = (self.now)();
            Self::sql(self.db.execute(
                "UPDATE dictations SET state = 'failed', finished_ms = ?2,
                 message = 'HyperMe stopped while recording' WHERE id = ?1",
                params![id, now],
            ))?;
        }
        Ok(())
    }

    /// Closes the audio file, writes the WAV sizes and records the file's size.
    fn finalize(&mut self, id: DictationId) -> Result<(), Error> {
        self.writers.remove(&id);
        let path = self.audio_path(id);
        let Ok(mut file) = OpenOptions::new().read(true).write(true).open(&path) else {
            return Ok(());
        };
        let len = file.metadata().map_err(error)?.len();
        let data = u32::try_from(len.saturating_sub(HEADER_BYTES)).unwrap_or(u32::MAX);
        file.seek(SeekFrom::Start(0)).map_err(error)?;
        file.write_all(&wav_header(data)).map_err(error)?;
        Self::sql(self.db.execute(
            "UPDATE dictations SET audio_bytes = ?2 WHERE id = ?1",
            params![id, len as i64],
        ))?;
        Ok(())
    }

    fn delete(&mut self, id: DictationId) -> Result<(), Error> {
        self.writers.remove(&id);
        match std::fs::remove_file(self.audio_path(id)) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(error(format!("deleting audio: {e}"))),
        }
        Self::sql(
            self.db
                .execute("DELETE FROM dictations WHERE id = ?1", params![id]),
        )?;
        Ok(())
    }

    fn ids(&self, sql: &str, value: i64) -> Result<Vec<DictationId>, Error> {
        let mut query = Self::sql(self.db.prepare(sql))?;
        let rows = Self::sql(query.query_map(params![value], |row| row.get(0)))?;
        Self::sql(rows.collect())
    }

    /// Applies the retention period, the size limit and, with history off, the 24 hours for
    /// failed audio.
    pub fn cleanup(&mut self) -> Result<(), Error> {
        let now = (self.now)();
        if !self.settings.enabled {
            let expired = self.ids(
                "SELECT id FROM dictations WHERE state = 'done'
                 OR (state = 'failed' AND finished_ms < ?1)",
                now - FAILED_KEPT_MS,
            )?;
            for id in expired {
                self.delete(id)?;
            }
            return Ok(());
        }
        if let Retention::Days(days) = self.settings.retention {
            let cutoff = now - i64::from(days) * DAY_MS;
            for id in self.ids(
                "SELECT id FROM dictations WHERE state != 'recording' AND started_ms < ?1",
                cutoff,
            )? {
                self.delete(id)?;
            }
        }
        // Oldest first, done before failed.
        let order = self.ids(
            "SELECT id FROM dictations WHERE state != 'recording' AND ?1 = ?1
             ORDER BY state = 'failed', started_ms, id",
            0,
        )?;
        for id in order {
            if self.total_bytes()? <= self.settings.size_limit_bytes {
                break;
            }
            self.delete(id)?;
        }
        Ok(())
    }

    /// The audio bytes on disk.
    pub fn total_bytes(&self) -> Result<u64, Error> {
        let total: i64 = Self::sql(self.db.query_row(
            "SELECT COALESCE(SUM(audio_bytes), 0) FROM dictations",
            [],
            |r| r.get(0),
        ))?;
        Ok(total as u64)
    }

    /// The text of the last successful dictation, for "Copy last dictation" and "Paste last
    /// dictation" (R-18). With history off there is none on disk.
    pub fn last_text(&self) -> Result<Option<String>, Error> {
        Self::sql(
            self.db
                .query_row(
                    "SELECT text FROM dictations WHERE state = 'done' ORDER BY started_ms DESC, id DESC LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .optional(),
        )
    }

    /// Every dictation, newest first.
    pub fn items(&self) -> Result<Vec<Item>, Error> {
        self.query_items(
            &format!("SELECT {ITEM_COLUMNS} FROM dictations ORDER BY started_ms DESC, id DESC"),
            [],
        )
    }

    fn query_items(&self, sql: &str, args: impl rusqlite::Params) -> Result<Vec<Item>, Error> {
        let mut query = Self::sql(self.db.prepare(sql))?;
        let rows = Self::sql(query.query_map(args, item_from_row))?;
        let items: Vec<Item> = Self::sql(rows.collect())?;
        Ok(items.into_iter().map(|i| self.with_audio(i)).collect())
    }

    /// Finished dictations older than `before` (start time, then id), newest first, that pass
    /// `keep`, up to `limit`. Keyset paging: a dictation added between two pages is newer than
    /// the cursor, so it never shifts a later page (#98 AC-7). Dictations still recording are
    /// left out.
    fn scan(
        &self,
        before: Option<(i64, DictationId)>,
        limit: usize,
        mut keep: impl FnMut(&Item) -> bool,
    ) -> Result<Vec<Item>, Error> {
        let (ms, id) = before.unwrap_or((i64::MAX, DictationId::MAX));
        let mut query = Self::sql(self.db.prepare(&format!(
            "SELECT {ITEM_COLUMNS} FROM dictations
             WHERE state != 'recording' AND (started_ms < ?1 OR (started_ms = ?1 AND id < ?2))
             ORDER BY started_ms DESC, id DESC"
        )))?;
        let mut rows = Self::sql(query.query(params![ms, id]))?;
        let mut items = Vec::new();
        while items.len() < limit {
            let Some(row) = Self::sql(rows.next())? else {
                break;
            };
            let item = Self::sql(item_from_row(row))?;
            if keep(&item) {
                items.push(self.with_audio(item));
            }
        }
        Ok(items)
    }

    /// A page of History: up to `limit` finished dictations older than `before`, the start
    /// time and id of the last item of the previous page (#98 R-10, AC-7).
    pub fn page(
        &self,
        before: Option<(i64, DictationId)>,
        limit: usize,
    ) -> Result<Vec<Item>, Error> {
        self.scan(before, limit, |_| true)
    }

    /// As [`page`](Self::page), only the dictations whose text holds every word of `query`,
    /// ignoring case and accents (#98 R-12, AC-8). In Rust over the texts: a 30-day history is
    /// a few thousand items, too few to need a full-text index.
    pub fn search(
        &self,
        query: &str,
        before: Option<(i64, DictationId)>,
        limit: usize,
    ) -> Result<Vec<Item>, Error> {
        let folded = fold(query);
        let words: Vec<&str> = folded.split_whitespace().collect();
        self.scan(before, limit, |item| {
            let text = fold(item.text.as_deref().unwrap_or_default());
            words.iter().all(|w| text.contains(w))
        })
    }

    fn with_audio(&self, mut item: Item) -> Item {
        item.has_audio = item.audio_bytes > HEADER_BYTES && self.audio_path(item.id).exists();
        item
    }

    /// One dictation by id (#98).
    pub fn item(&self, id: DictationId) -> Result<Option<Item>, Error> {
        let sql = format!("SELECT {ITEM_COLUMNS} FROM dictations WHERE id = ?1");
        Ok(self.query_items(&sql, params![id])?.pop())
    }

    /// Deletes a dictation's text and audio (#98 R-18, AC-12).
    pub fn delete_item(&mut self, id: DictationId) -> Result<(), Error> {
        self.delete(id)
    }

    /// Replaces a dictation's text after a retry from Home (#98 R-16). The audio stays. With
    /// history off, a successful retry leaves nothing behind, as [`History::done`] does
    /// (Phase 1 R-28).
    pub fn replace_text(
        &mut self,
        id: DictationId,
        text: &str,
        language: Option<&str>,
    ) -> Result<(), Error> {
        if !self.settings.enabled {
            return self.delete(id);
        }
        let now = (self.now)();
        let changed = Self::sql(self.db.execute(
            "UPDATE dictations SET state = 'done', text = ?2, language = ?3, finished_ms = ?4,
             message = NULL WHERE id = ?1 AND state != 'recording'",
            params![id, text, language, now],
        ))?;
        if changed == 0 {
            return Err(error(format!("no finished dictation {id}")));
        }
        Ok(())
    }

    /// The failed dictations that still have their audio, newest first: all History lists
    /// while history is off (#98 R-19, AC-13).
    pub fn failed_with_audio(&self) -> Result<Vec<Item>, Error> {
        let sql = format!(
            "SELECT {ITEM_COLUMNS} FROM dictations WHERE state = 'failed'
             ORDER BY started_ms DESC, id DESC"
        );
        let mut items = self.query_items(&sql, [])?;
        items.retain(|i| i.has_audio);
        Ok(items)
    }
}

impl History for SqliteHistory {
    fn begin(&mut self, app: Option<&str>) -> Result<DictationId, Error> {
        let now = (self.now)();
        Self::sql(self.db.execute(
            "INSERT INTO dictations (started_ms, app, state) VALUES (?1, ?2, 'recording')",
            params![now, app],
        ))?;
        let id = self.db.last_insert_rowid();
        let mut file =
            File::create(self.audio_path(id)).map_err(|e| error(format!("creating audio: {e}")))?;
        file.write_all(&wav_header(0)).map_err(error)?;
        self.writers.insert(id, file);
        Ok(id)
    }

    fn append(&mut self, id: DictationId, pcm: &[i16]) -> Result<(), Error> {
        let file = self
            .writers
            .get_mut(&id)
            .ok_or_else(|| error("no dictation is recording"))?;
        let bytes: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
        // Unbuffered: each frame reaches the OS at once, so a crash loses at most one frame.
        file.write_all(&bytes)
            .map_err(|e| error(format!("writing audio: {e}")))
    }

    fn done(&mut self, id: DictationId, text: &str, language: Option<&str>) -> Result<(), Error> {
        if !self.settings.enabled {
            // R-28: with history off, a successful dictation leaves nothing behind.
            return self.delete(id);
        }
        self.finalize(id)?;
        let now = (self.now)();
        Self::sql(self.db.execute(
            "UPDATE dictations SET state = 'done', text = ?2, language = ?3, finished_ms = ?4,
             message = NULL WHERE id = ?1",
            params![id, text, language, now],
        ))?;
        self.cleanup()
    }

    fn failed(&mut self, id: DictationId, message: &str) -> Result<(), Error> {
        self.finalize(id)?;
        let now = (self.now)();
        Self::sql(self.db.execute(
            "UPDATE dictations SET state = 'failed', message = ?2, finished_ms = ?3 WHERE id = ?1",
            params![id, message, now],
        ))?;
        self.cleanup()
    }

    fn discard(&mut self, id: DictationId) -> Result<(), Error> {
        self.delete(id)
    }

    fn audio(&self, id: DictationId) -> Result<Vec<i16>, Error> {
        let mut file = File::open(self.audio_path(id))
            .map_err(|e| error(format!("no audio for dictation {id}: {e}")))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(error)?;
        // The data runs to the end of the file, whatever the header says: a crash may have left
        // the sizes unwritten.
        let data = bytes.get(HEADER_BYTES as usize..).unwrap_or_default();
        Ok(data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes(*b))
            .collect())
    }
}
