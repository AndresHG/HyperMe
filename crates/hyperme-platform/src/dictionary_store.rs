//! The personal dictionary on SQLite (spec 93, design.md, "Data model"; slice D2): entries and
//! their heard forms in `dictionary.sqlite`, apart from the history so that no retention setting
//! ever deletes a word. Portable code: the app passes the path.
//!
//! The list is small (at most 1,000 entries), so a save replaces it whole in one transaction,
//! keeping each row that stays: its source, its date and its use counts. The suggestion and
//! dismissal tables are for learning (slice D6) and stay empty until then.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use hyperme_core::dictionary::{Dictionary, Term, key};
use rusqlite::{Connection, OptionalExtension, params};

/// One entry per schema version: entry n takes `user_version` from n to n + 1.
const MIGRATIONS: &[&str] = &[
    // 1: entries, heard forms, and the learning tables for slice D6.
    "CREATE TABLE entry (
        id INTEGER PRIMARY KEY,
        written TEXT NOT NULL,
        position INTEGER NOT NULL,
        source TEXT NOT NULL,
        created_ms INTEGER NOT NULL,
        uses INTEGER NOT NULL DEFAULT 0,
        last_used_ms INTEGER
    );
    CREATE TABLE heard (
        id INTEGER PRIMARY KEY,
        entry_id INTEGER NOT NULL REFERENCES entry(id) ON DELETE CASCADE,
        heard TEXT NOT NULL,
        position INTEGER NOT NULL,
        source TEXT NOT NULL,
        created_ms INTEGER NOT NULL,
        uses INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX heard_entry ON heard(entry_id);
    CREATE TABLE suggestion (
        entry_written TEXT NOT NULL,
        heard TEXT NOT NULL,
        seen INTEGER NOT NULL,
        first_seen_ms INTEGER NOT NULL,
        last_app TEXT,
        PRIMARY KEY (entry_written, heard)
    );
    CREATE TABLE dismissed (
        entry_written TEXT NOT NULL,
        heard TEXT NOT NULL,
        PRIMARY KEY (entry_written, heard)
    );
    CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
];

/// Where a word or heard form came from (design.md, "Data model"). Learned ones get a mark on
/// the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Manual,
    HistoryFix,
    Learned,
    Imported,
}

impl Source {
    fn as_str(self) -> &'static str {
        match self {
            Source::Manual => "manual",
            Source::HistoryFix => "history_fix",
            Source::Learned => "learned",
            Source::Imported => "imported",
        }
    }
}

/// The meta key set once the text file has been imported (R-14).
const TEXT_IMPORTED: &str = "text_imported";

fn sql<T>(r: rusqlite::Result<T>) -> Result<T, String> {
    r.map_err(|e| format!("The dictionary store failed: {e}"))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

#[derive(Debug)]
pub struct DictionaryStore {
    db: Connection,
}

impl DictionaryStore {
    /// Opens or creates the store at `path`, bringing its schema up to date. A file written by a
    /// newer HyperMe is refused rather than changed.
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let mut db = sql(Connection::open(path))?;
        sql(db.execute_batch("PRAGMA foreign_keys = ON"))?;
        let version: i64 = sql(db.query_row("PRAGMA user_version", [], |r| r.get(0)))?;
        let version = usize::try_from(version).unwrap_or(usize::MAX);
        if version > MIGRATIONS.len() {
            return Err(format!(
                "{} is from a newer HyperMe (schema {version}, this one knows {})",
                path.display(),
                MIGRATIONS.len()
            ));
        }
        for (n, migration) in MIGRATIONS.iter().enumerate().skip(version) {
            let tx = sql(db.transaction())?;
            sql(tx.execute_batch(migration))?;
            sql(tx.execute_batch(&format!("PRAGMA user_version = {}", n + 1)))?;
            sql(tx.commit())?;
        }
        Ok(Self { db })
    }

    /// The whole list, in its order, with each entry's use count (R-5).
    pub fn dictionary(&self) -> Result<Dictionary, String> {
        let mut entries = sql(self
            .db
            .prepare("SELECT id, written, uses FROM entry ORDER BY position"))?;
        let mut heard = sql(self
            .db
            .prepare("SELECT heard FROM heard WHERE entry_id = ?1 ORDER BY position"))?;
        let rows: Vec<(i64, String, u32)> =
            sql(sql(entries.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))))?.collect())?;
        let mut terms = Vec::with_capacity(rows.len());
        let uses = rows.iter().map(|r| r.2).collect();
        for (id, written, _) in rows {
            let forms: Vec<String> = sql(sql(heard.query_map([id], |r| r.get(0)))?.collect())?;
            terms.push(Term {
                written,
                heard: forms,
            });
        }
        Ok(Dictionary::new(terms).with_uses(uses))
    }

    /// Replaces the list with `terms` (R-11, R-15): checked first, then written in one
    /// transaction. Entries and heard forms that stay keep their source, date and counts; new
    /// ones get `source`. Entries match by [`key`], so a change of case keeps the row.
    pub fn save(&mut self, terms: Vec<Term>, source: Source) -> Result<Dictionary, String> {
        let terms: Vec<Term> = terms
            .into_iter()
            .map(|t| Term {
                written: t.written.trim().to_owned(),
                heard: t
                    .heard
                    .iter()
                    .map(|h| h.trim())
                    .filter(|h| !h.is_empty())
                    .map(str::to_owned)
                    .collect(),
            })
            .collect();
        Dictionary::check(&terms)?;
        let now = now_ms();
        let tx = sql(self.db.transaction())?;
        {
            let existing: Vec<(i64, String)> = {
                let mut q = sql(tx.prepare("SELECT id, written FROM entry"))?;
                sql(sql(q.query_map([], |r| Ok((r.get(0)?, r.get(1)?))))?.collect())?
            };
            let mut kept: Vec<i64> = Vec::with_capacity(terms.len());
            for (position, term) in terms.iter().enumerate() {
                let position = i64::try_from(position).unwrap_or(i64::MAX);
                let id = match existing.iter().find(|(_, w)| key(w) == key(&term.written)) {
                    Some((id, _)) => {
                        sql(tx.execute(
                            "UPDATE entry SET written = ?2, position = ?3 WHERE id = ?1",
                            params![id, term.written, position],
                        ))?;
                        *id
                    }
                    None => {
                        sql(tx.execute(
                            "INSERT INTO entry (written, position, source, created_ms)
                             VALUES (?1, ?2, ?3, ?4)",
                            params![term.written, position, source.as_str(), now],
                        ))?;
                        tx.last_insert_rowid()
                    }
                };
                kept.push(id);
                let forms: Vec<(i64, String)> = {
                    let mut q = sql(tx.prepare("SELECT id, heard FROM heard WHERE entry_id = ?1"))?;
                    sql(sql(q.query_map([id], |r| Ok((r.get(0)?, r.get(1)?))))?.collect())?
                };
                let mut kept_forms: Vec<i64> = Vec::with_capacity(term.heard.len());
                for (at, heard) in term.heard.iter().enumerate() {
                    let at = i64::try_from(at).unwrap_or(i64::MAX);
                    match forms.iter().find(|(_, h)| key(h) == key(heard)) {
                        Some((form, _)) => {
                            sql(tx.execute(
                                "UPDATE heard SET heard = ?2, position = ?3 WHERE id = ?1",
                                params![form, heard, at],
                            ))?;
                            kept_forms.push(*form);
                        }
                        None => {
                            sql(tx.execute(
                                "INSERT INTO heard (entry_id, heard, position, source, created_ms)
                                 VALUES (?1, ?2, ?3, ?4, ?5)",
                                params![id, heard, at, source.as_str(), now],
                            ))?;
                            kept_forms.push(tx.last_insert_rowid());
                        }
                    }
                }
                for (form, _) in &forms {
                    if !kept_forms.contains(form) {
                        sql(tx.execute("DELETE FROM heard WHERE id = ?1", [form]))?;
                    }
                }
            }
            for (id, _) in &existing {
                if !kept.contains(id) {
                    sql(tx.execute("DELETE FROM entry WHERE id = ?1", [id]))?;
                }
            }
        }
        sql(tx.commit())?;
        self.dictionary()
    }

    /// The one-time import of the dictionary text file (R-14, AC-11): into an empty store, the
    /// first time only. Returns whether it imported. After the first call, later ones do
    /// nothing, even if the store is empty again.
    pub fn import_text_once(&mut self, text: &str) -> Result<bool, String> {
        let done: Option<String> = sql(self
            .db
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                [TEXT_IMPORTED],
                |r| r.get(0),
            )
            .optional())?;
        if done.is_some() {
            return Ok(false);
        }
        let empty = self.dictionary()?.is_empty();
        if empty {
            let terms = Dictionary::parse(text).terms().to_vec();
            self.save(terms, Source::Imported)?;
        }
        sql(self.db.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)",
            params![TEXT_IMPORTED, now_ms().to_string()],
        ))?;
        Ok(empty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("hyperme-dict-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("dictionary.sqlite")
    }

    fn term(written: &str, heard: &[&str]) -> Term {
        Term {
            written: written.to_owned(),
            heard: heard.iter().map(|h| (*h).to_owned()).collect(),
        }
    }

    fn sources(store: &DictionaryStore) -> Vec<(String, String)> {
        let mut q = store
            .db
            .prepare(
                "SELECT e.written || ':' || e.source, coalesce(h.heard || ':' || h.source, '')
                 FROM entry e LEFT JOIN heard h ON h.entry_id = e.id ORDER BY e.position, h.position",
            )
            .unwrap();
        q.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    /// AC-10 (R-11, R-15): a save writes what a reopen reads back, in order; rows that stay
    /// keep their source; a bad list is refused and changes nothing.
    #[test]
    fn a_save_keeps_rows_that_stay() {
        let path = temp("save");
        let mut store = DictionaryStore::open(&path).unwrap();
        assert!(store.dictionary().unwrap().is_empty());

        store
            .save(
                vec![term("Zentrix", &["zen trix"]), term("LangChain", &[])],
                Source::Manual,
            )
            .unwrap();
        let fixed = store
            .save(
                vec![
                    term(" zentrix ", &["zen trix", "sun trix"]),
                    term("Claude", &["cloud"]),
                ],
                Source::HistoryFix,
            )
            .unwrap();
        assert_eq!(
            sources(&store),
            [
                ("zentrix:manual".to_owned(), "zen trix:manual".to_owned()),
                (
                    "zentrix:manual".to_owned(),
                    "sun trix:history_fix".to_owned()
                ),
                (
                    "Claude:history_fix".to_owned(),
                    "cloud:history_fix".to_owned()
                ),
            ]
        );
        drop(store);

        let mut store = DictionaryStore::open(&path).unwrap();
        assert_eq!(store.dictionary().unwrap(), fixed);
        assert!(
            store
                .save(vec![term("A", &[]), term("a", &[])], Source::Manual)
                .unwrap_err()
                .contains("twice")
        );
        assert_eq!(store.dictionary().unwrap(), fixed);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// AC-11 (R-14): an empty store imports the text file once; a second start does not, even
    /// after the list was emptied; a store that already has words is not touched.
    #[test]
    fn the_text_file_is_imported_once() {
        let path = temp("import");
        let mut store = DictionaryStore::open(&path).unwrap();
        assert!(
            store
                .import_text_once("Zentrix: zen trix\nLangChain\n")
                .unwrap()
        );
        assert_eq!(store.dictionary().unwrap().terms().len(), 2);
        assert_eq!(sources(&store)[0].0, "Zentrix:imported");
        store.save(Vec::new(), Source::Manual).unwrap();
        drop(store);

        let mut store = DictionaryStore::open(&path).unwrap();
        assert!(!store.import_text_once("Zentrix\n").unwrap());
        assert!(store.dictionary().unwrap().is_empty());

        let other = temp("import-full");
        let mut full = DictionaryStore::open(&other).unwrap();
        full.save(vec![term("Claude", &[])], Source::Manual)
            .unwrap();
        assert!(!full.import_text_once("Zentrix\n").unwrap());
        assert_eq!(full.dictionary().unwrap().terms().len(), 1);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
        let _ = std::fs::remove_dir_all(other.parent().unwrap());
    }
}
