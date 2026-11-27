//! The personal dictionary on SQLite (spec 93, design.md, "Data model"; slice D2): entries and
//! their heard forms in `dictionary.sqlite`, apart from the history so that no retention setting
//! ever deletes a word. Portable code: the app passes the path.
//!
//! The list is small (at most 1,000 entries), so a save replaces it whole in one transaction,
//! keeping each row that stays: its source, its date and its use counts. The suggestion and
//! dismissal tables hold what learning from the user's edits found (slice D6).

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use hyperme_core::dictionary::{Corrected, Dictionary, Term, key};
use hyperme_core::learning::{self, Known, Step};
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
/// The meta key for "Learn from my edits" (R-16): `off`, or on when missing.
const LEARNING: &str = "learning";

/// A fix seen in the user's edits, waiting for a yes (R-18).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub written: String,
    pub heard: String,
    /// Times seen.
    pub seen: u32,
    /// The app it was last fixed in.
    pub last_app: Option<String>,
}

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
                for (form, heard) in &forms {
                    if !kept_forms.contains(form) {
                        // R-19: a learned heard form the user deletes never comes back.
                        sql(tx.execute(
                            "INSERT OR IGNORE INTO dismissed (entry_written, heard)
                             SELECT ?2, ?3 FROM heard WHERE id = ?1 AND source = 'learned'",
                            params![form, term.written, heard],
                        ))?;
                        sql(tx.execute("DELETE FROM heard WHERE id = ?1", [form]))?;
                    }
                }
            }
            for (id, written) in &existing {
                if !kept.contains(id) {
                    sql(tx.execute(
                        "INSERT OR IGNORE INTO dismissed (entry_written, heard)
                         SELECT ?2, heard FROM heard WHERE entry_id = ?1 AND source = 'learned'
                         UNION SELECT ?2, ?2 FROM entry WHERE id = ?1 AND source = 'learned'",
                        params![id, written],
                    ))?;
                    sql(tx.execute("DELETE FROM entry WHERE id = ?1", [id]))?;
                }
            }
        }
        sql(tx.commit())?;
        self.dictionary()
    }

    /// Counts one dictation's use of each entry it used (design.md, "Data model"): orders the
    /// prompt's relevant entries (R-5). Names that are no longer entries are skipped.
    pub fn count_uses(&mut self, used: &[String]) -> Result<(), String> {
        if used.is_empty() {
            return Ok(());
        }
        let now = now_ms();
        let tx = sql(self.db.transaction())?;
        {
            let entries: Vec<(i64, String)> = {
                let mut q = sql(tx.prepare("SELECT id, written FROM entry"))?;
                sql(sql(q.query_map([], |r| Ok((r.get(0)?, r.get(1)?))))?.collect())?
            };
            for name in used {
                if let Some((id, _)) = entries.iter().find(|(_, w)| key(w) == key(name)) {
                    sql(tx.execute(
                        "UPDATE entry SET uses = uses + 1, last_used_ms = ?2 WHERE id = ?1",
                        params![id, now],
                    ))?;
                }
            }
        }
        sql(tx.commit())
    }

    /// The suggestions waiting for the user (R-18, R-22), oldest first.
    pub fn suggestions(&self) -> Result<Vec<Suggestion>, String> {
        let mut q = sql(self.db.prepare(
            "SELECT entry_written, heard, seen, last_app FROM suggestion ORDER BY first_seen_ms",
        ))?;
        sql(sql(q.query_map([], |r| {
            Ok(Suggestion {
                written: r.get(0)?,
                heard: r.get(1)?,
                seen: r.get(2)?,
                last_app: r.get(3)?,
            })
        }))?
        .collect())
    }

    /// The suggestion for this fix, matched as the matcher compares, if one waits.
    fn suggestion_row(&self, written: &str, heard: &str) -> Result<Option<Suggestion>, String> {
        Ok(self
            .suggestions()?
            .into_iter()
            .find(|s| key(&s.written) == key(written) && key(&s.heard) == key(heard)))
    }

    /// How many times this fix was seen, 0 if it is not waiting as a suggestion.
    pub fn seen(&self, written: &str, heard: &str) -> Result<u32, String> {
        Ok(self.suggestion_row(written, heard)?.map_or(0, |s| s.seen))
    }

    /// Notes a sighting of a fix (R-18) and returns how many times it was seen before this one.
    pub fn note_suggestion(
        &mut self,
        written: &str,
        heard: &str,
        app: Option<&str>,
    ) -> Result<u32, String> {
        match self.suggestion_row(written, heard)? {
            Some(s) => {
                sql(self.db.execute(
                    "UPDATE suggestion SET seen = seen + 1, last_app = ?3
                     WHERE entry_written = ?1 AND heard = ?2",
                    params![s.written, s.heard, app],
                ))?;
                Ok(s.seen)
            }
            None => {
                sql(self.db.execute(
                    "INSERT INTO suggestion (entry_written, heard, seen, first_seen_ms, last_app)
                     VALUES (?1, ?2, 1, ?3, ?4)",
                    params![written, heard, now_ms(), app],
                ))?;
                Ok(0)
            }
        }
    }

    /// Takes a suggestion off the list: it was added, or the user dismissed it.
    pub fn drop_suggestion(&mut self, written: &str, heard: &str) -> Result<(), String> {
        if let Some(s) = self.suggestion_row(written, heard)? {
            sql(self.db.execute(
                "DELETE FROM suggestion WHERE entry_written = ?1 AND heard = ?2",
                params![s.written, s.heard],
            ))?;
        }
        Ok(())
    }

    /// The user dismissed a suggestion: it never comes back (R-19).
    pub fn dismiss(&mut self, written: &str, heard: &str) -> Result<(), String> {
        self.drop_suggestion(written, heard)?;
        sql(self.db.execute(
            "INSERT OR IGNORE INTO dismissed (entry_written, heard) VALUES (?1, ?2)",
            params![written, heard],
        ))?;
        Ok(())
    }

    /// Whether the user dismissed this fix, or deleted it once learned (R-19).
    pub fn is_dismissed(&self, written: &str, heard: &str) -> Result<bool, String> {
        let mut q = sql(self
            .db
            .prepare("SELECT entry_written, heard FROM dismissed"))?;
        let rows: Vec<(String, String)> =
            sql(sql(q.query_map([], |r| Ok((r.get(0)?, r.get(1)?))))?.collect())?;
        Ok(rows
            .iter()
            .any(|(w, h)| key(w) == key(written) && key(h) == key(heard)))
    }

    /// The learned rows, for the page's mark and Undo: `(written, heard)`, with `heard` empty
    /// for an entry learned with no heard form.
    pub fn learned(&self) -> Result<Vec<(String, String)>, String> {
        let mut q = sql(self.db.prepare(
            "SELECT e.written, h.heard FROM heard h JOIN entry e ON e.id = h.entry_id
             WHERE h.source = 'learned'
             UNION ALL SELECT written, '' FROM entry WHERE source = 'learned'",
        ))?;
        sql(sql(q.query_map([], |r| Ok((r.get(0)?, r.get(1)?))))?.collect())
    }

    /// Whether "Learn from my edits" is on (R-16). On unless the user turned it off.
    pub fn learning(&self) -> Result<bool, String> {
        let value: Option<String> = sql(self
            .db
            .query_row("SELECT value FROM meta WHERE key = ?1", [LEARNING], |r| {
                r.get(0)
            })
            .optional())?;
        Ok(value.as_deref() != Some("off"))
    }

    pub fn set_learning(&mut self, on: bool) -> Result<(), String> {
        sql(self.db.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![LEARNING, if on { "on" } else { "off" }],
        ))?;
        Ok(())
    }

    /// The watch of a pasted dictation ended with `edited` in its place (R-16 to R-19): each fix
    /// of a misheard word becomes a suggestion, or is learned on its second sighting. `pasted`
    /// and `transcript` are what was pasted and the transcript it came from; `dictations_with`
    /// counts the dictations in the history that hold some words (R-17). Returns the new
    /// dictionary when something was learned. The field text is not kept, only the fix (R-21).
    pub fn learn(
        &mut self,
        pasted: &str,
        transcript: &str,
        edited: &str,
        app: Option<&str>,
        dictations_with: impl Fn(&str) -> u32,
    ) -> Result<Option<Dictionary>, String> {
        let mut changed = None;
        for candidate in learning::candidates(pasted, edited, transcript) {
            let (Some(heard), Some(pasted_form)) =
                (candidate.heard.first(), candidate.heard.last())
            else {
                continue;
            };
            let dictionary = self.dictionary()?;
            let seen_before = self.seen(&candidate.wanted, heard)?;
            let known = Known {
                seen_before,
                dismissed: self.is_dismissed(&candidate.wanted, heard)?,
                // This dictation and the earlier sightings were corrected.
                uncorrected: dictations_with(pasted_form).saturating_sub(seen_before + 1),
            };
            match learning::step(&candidate, &dictionary, known) {
                Step::Nothing => {}
                Step::Suggest => {
                    self.note_suggestion(&candidate.wanted, heard, app)?;
                }
                Step::Learn => {
                    let mut fixed = dictionary;
                    for form in &candidate.heard {
                        if let Corrected::Added(next) =
                            fixed.corrected(form, &candidate.wanted, true)?
                        {
                            fixed = next;
                        }
                    }
                    self.drop_suggestion(&candidate.wanted, heard)?;
                    changed = Some(self.save(fixed.terms().to_vec(), Source::Learned)?);
                }
            }
        }
        Ok(changed)
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

    /// AC-14 (R-18, R-19): a sighting counts up; a dismissal takes the suggestion off and
    /// sticks; a learned heard form or entry that the user deletes is dismissed too; use counts
    /// add up; the learning switch is on until turned off.
    #[test]
    fn suggestions_dismissals_and_use_counts_persist() {
        let path = temp("learn");
        let mut store = DictionaryStore::open(&path).unwrap();
        assert_eq!(
            store.note_suggestion("Zentrix", "sun trix", Some("Slack")),
            Ok(0)
        );
        assert_eq!(
            store.note_suggestion("zentrix", "Sun Trix", Some("Mail")),
            Ok(1)
        );
        let waiting = store.suggestions().unwrap();
        assert_eq!(waiting.len(), 1);
        assert_eq!(
            (waiting[0].seen, waiting[0].last_app.as_deref()),
            (2, Some("Mail"))
        );

        store.note_suggestion("Quorbit", "corbit", None).unwrap();
        store.dismiss("Quorbit", "corbit").unwrap();
        assert!(store.is_dismissed("quorbit", "Corbit").unwrap());
        assert_eq!(store.suggestions().unwrap().len(), 1);

        store
            .save(
                vec![term("Zentrix", &["sun trix"]), term("Kubectl", &[])],
                Source::Learned,
            )
            .unwrap();
        assert_eq!(store.learned().unwrap().len(), 3);
        store
            .save(vec![term("Zentrix", &[])], Source::Manual)
            .unwrap();
        assert!(store.is_dismissed("Zentrix", "sun trix").unwrap());
        assert!(store.is_dismissed("Kubectl", "kubectl").unwrap());

        store
            .count_uses(&["zentrix".to_owned(), "Gone".to_owned()])
            .unwrap();
        store.count_uses(&["Zentrix".to_owned()]).unwrap();
        let uses: u32 = store
            .db
            .query_row("SELECT uses FROM entry", [], |r| r.get(0))
            .unwrap();
        assert_eq!(uses, 2);

        assert!(store.learning().unwrap());
        store.set_learning(false).unwrap();
        assert!(!store.learning().unwrap());
        store.set_learning(true).unwrap();
        assert!(store.learning().unwrap());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// AC-14 (R-16 to R-19), end to end on the store: a fix seen once waits as a suggestion,
    /// seen twice it is learned and used; a fix the user dismissed never returns; a heard form
    /// the user says often without fixing it stays a suggestion.
    #[test]
    fn fixes_become_suggestions_then_learned_words() {
        let path = temp("learn-flow");
        let mut store = DictionaryStore::open(&path).unwrap();
        let (pasted, raw) = (
            "Send the sun trix report today.",
            "send the sun trix report today",
        );
        let edited = "Send the Zentrix report today.";
        let never = |_: &str| 0;

        assert_eq!(
            store.learn(pasted, raw, edited, Some("Slack"), never),
            Ok(None)
        );
        assert_eq!(store.suggestions().unwrap().len(), 1);
        let learned = store
            .learn(pasted, raw, edited, Some("Slack"), never)
            .unwrap();
        let learned = learned.expect("learned on the second sighting");
        assert_eq!(learned.apply("the sun trix build").0, "the Zentrix build");
        assert!(store.suggestions().unwrap().is_empty());
        assert_eq!(store.learned().unwrap().len(), 2);
        // Already fixed by the dictionary: nothing new.
        assert_eq!(store.learn(pasted, raw, edited, None, never), Ok(None));
        assert!(store.suggestions().unwrap().is_empty());

        let (pasted, raw) = ("The corbit page is down.", "the corbit page is down");
        let edited = "The Quorbit page is down.";
        store.learn(pasted, raw, edited, None, never).unwrap();
        store.dismiss("Quorbit", "corbit").unwrap();
        for _ in 0..3 {
            assert_eq!(store.learn(pasted, raw, edited, None, never), Ok(None));
        }
        assert!(store.suggestions().unwrap().is_empty());

        // Said in five dictations, fixed in two: common, so it stays a suggestion.
        let (pasted, raw) = ("Ping the dock team.", "ping the dock team");
        let edited = "Ping the Dokk team.";
        let often = |_: &str| 5;
        for _ in 0..2 {
            assert_eq!(store.learn(pasted, raw, edited, None, often), Ok(None));
        }
        assert_eq!(store.seen("Dokk", "dock").unwrap(), 2);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
