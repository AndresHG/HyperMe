//! The personal dictionary in the app (spec 93): kept in `dictionary.sqlite` in the app's data
//! folder (slice D2), edited in Settings → Dictionary (D1b), added to by Fix a word in Home's
//! History (D5). A change applies from the next dictation, with no restart (R-10a).
//!
//! The dictionary text file from before (`dictation.dictionary` in the config, or
//! `dictionary.txt`) is imported once into an empty store (R-14) and then left alone.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use hyperme_core::config::Config;
use hyperme_core::dictionary::{Corrected, Dictionary, Term, is_common_word};
use hyperme_core::traits::{self, Added, Correction, Error, ErrorKind, Refusal};
use hyperme_platform::dictionary_store::{DictionaryStore, Source};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// The dictionary text file from before the store: the one named in the config, or
/// `dictionary.txt` in the app's data folder. Only read for the one-time import (R-14).
pub fn file(dir: &Path, config: Option<&Config>) -> PathBuf {
    config
        .and_then(|c| c.dictation.dictionary.clone())
        .unwrap_or_else(|| dir.join("dictionary.txt"))
}

/// The store, or why it could not be opened.
struct State {
    path: PathBuf,
    store: Result<DictionaryStore, String>,
}

type Shared = Mutex<State>;

/// Opens the store and imports the old text file into it once (R-14). Called once at setup,
/// before dictation starts. A store that cannot be opened leaves dictation working without a
/// dictionary; Settings shows why.
pub fn init(app: &AppHandle) {
    let dir = crate::runtime::data_dir(app);
    let path = dir.join("dictionary.sqlite");
    let config = Config::load_or_create(&dir.join("config.toml")).ok();
    let store = DictionaryStore::open(&path).and_then(|mut store| {
        import_once(&mut store, &file(&dir, config.as_ref()))?;
        Ok(store)
    });
    if let Err(why) = &store {
        eprintln!("HyperMe: {why}");
    }
    app.manage::<Shared>(Mutex::new(State { path, store }));
}

/// The one-time import (R-14, AC-11). A missing file counts as imported-empty.
fn import_once(store: &mut DictionaryStore, file: &Path) -> Result<bool, String> {
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{} could not be read: {e}", file.display())),
    };
    store.import_text_once(&text)
}

fn with_store<T>(
    app: &AppHandle,
    f: impl FnOnce(&mut DictionaryStore, &Path) -> Result<T, String>,
) -> Result<T, String> {
    let shared = app.state::<Shared>();
    let mut state = shared.lock().unwrap_or_else(PoisonError::into_inner);
    let State { path, store } = &mut *state;
    match store {
        Ok(store) => f(store, path),
        Err(why) => Err(why.clone()),
    }
}

/// The dictionary for dictation: empty if the store could not be read.
pub fn current(app: &AppHandle) -> Dictionary {
    with_store(app, |store, _| store.dictionary()).unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    written: String,
    heard: Vec<String>,
}

/// What the page shows.
#[derive(Debug, Serialize)]
pub struct View {
    path: String,
    entries: Vec<Entry>,
    error: Option<String>,
}

fn view(path: &Path, dictionary: &Dictionary, error: Option<String>) -> View {
    View {
        path: path.display().to_string(),
        entries: dictionary
            .terms()
            .iter()
            .map(|t| Entry {
                written: t.written.clone(),
                heard: t.heard.clone(),
            })
            .collect(),
        error,
    }
}

fn terms(entries: Vec<Entry>) -> Vec<Term> {
    entries
        .into_iter()
        .map(|e| Term {
            written: e.written,
            heard: e.heard,
        })
        .collect()
}

/// Saves a changed list, hands it to the running dictation (R-10a) and returns the page's view.
fn saved(
    app: &AppHandle,
    change: impl FnOnce(&mut DictionaryStore) -> Result<Dictionary, String>,
) -> Result<View, String> {
    let (dictionary, shown) = with_store(app, |store, path| {
        let dictionary = change(store)?;
        let shown = view(path, &dictionary, None);
        Ok((dictionary, shown))
    })?;
    crate::runtime::update_dictionary(app, dictionary);
    Ok(shown)
}

#[tauri::command]
pub fn dictionary_view(app: AppHandle) -> View {
    let shared = app.state::<Shared>();
    let state = shared.lock().unwrap_or_else(PoisonError::into_inner);
    match &state.store {
        Ok(store) => match store.dictionary() {
            Ok(d) => view(&state.path, &d, None),
            Err(why) => view(&state.path, &Dictionary::default(), Some(why)),
        },
        Err(why) => view(&state.path, &Dictionary::default(), Some(why.clone())),
    }
}

/// Saves the list from the page (R-11, R-15).
#[tauri::command]
pub fn dictionary_save(app: AppHandle, entries: Vec<Entry>) -> Result<View, String> {
    saved(&app, |store| store.save(terms(entries), Source::Manual))
}

/// The add dialog's warning (R-17): a common misspelling would be replaced everywhere.
#[tauri::command]
pub fn dictionary_is_common(word: String) -> bool {
    is_common_word(&word)
}

/// Reads an imported file: CSV by its name, otherwise the text format (R-13).
fn parse_import(name: &str, text: &str) -> Result<Vec<Term>, String> {
    if name.to_lowercase().ends_with(".csv") {
        Dictionary::parse_csv(text)
    } else {
        Ok(Dictionary::parse(text).terms().to_vec())
    }
}

/// Import (R-13): a text or CSV file the page read, merged into the list.
#[tauri::command]
pub fn dictionary_import(app: AppHandle, name: String, text: String) -> Result<View, String> {
    let incoming = parse_import(&name, &text)?;
    saved(&app, |store| {
        let merged = store.dictionary()?.merged(&incoming)?;
        store.save(merged.terms().to_vec(), Source::Imported)
    })
}

/// The export's file name and contents for `format`: `csv`, or the text format.
fn export(dictionary: &Dictionary, format: &str) -> (&'static str, String) {
    if format == "csv" {
        ("HyperMe dictionary.csv", dictionary.to_csv())
    } else {
        ("HyperMe dictionary.txt", dictionary.to_text())
    }
}

/// Export (R-13): writes the list to the Downloads folder and shows it in Finder.
#[tauri::command]
pub fn dictionary_export(app: AppHandle, format: String) -> Result<String, String> {
    let dictionary = with_store(&app, |store, _| store.dictionary())?;
    let dir = app
        .path()
        .download_dir()
        .map_err(|e| format!("No Downloads folder: {e}"))?;
    let (name, contents) = export(&dictionary, &format);
    let path = dir.join(name);
    std::fs::write(&path, contents).map_err(|e| format!("{}: {e}", path.display()))?;
    let _ = std::process::Command::new("open")
        .arg("-R")
        .arg(&path)
        .spawn();
    Ok(path.display().to_string())
}

/// Applies a fix (R-12, R-17, AC-12): `None` when the heard form is common and needs a yes.
fn correct(store: &mut DictionaryStore, c: &Correction) -> Result<Option<Dictionary>, String> {
    match store
        .dictionary()?
        .corrected(&c.heard, &c.wanted, c.confirmed_common)?
    {
        Corrected::Common => Ok(None),
        Corrected::Added(fixed) => store
            .save(fixed.terms().to_vec(), Source::HistoryFix)
            .map(Some),
    }
}

/// The dictionary as Home's Fix a word sees it (spec 98, R-20).
pub struct Fixer {
    app: AppHandle,
}

impl Fixer {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl traits::Dictionary for Fixer {
    fn add_correction(&mut self, c: Correction) -> Result<Added, Error> {
        let fixed =
            with_store(&self.app, |store, _| correct(store, &c)).map_err(|message| Error {
                kind: ErrorKind::Other,
                message,
            })?;
        Ok(match fixed {
            None => Added::NeedsConfirm(Refusal::CommonWord),
            Some(dictionary) => {
                crate::runtime::update_dictionary(&self.app, dictionary);
                Added::Added
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("hyperme-dictionary-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn fix(heard: &str, wanted: &str, confirmed_common: bool) -> Correction {
        Correction {
            heard: heard.to_owned(),
            wanted: wanted.to_owned(),
            dictation: None,
            language: None,
            confirmed_common,
        }
    }

    /// Spec 93, D1b and R-14: the configured file wins; otherwise dictionary.txt in the data
    /// folder.
    #[test]
    fn the_file_is_the_configured_one_or_the_default() {
        let dir = Path::new("/data/HyperMe");
        assert_eq!(file(dir, None), dir.join("dictionary.txt"));
        let mut config = Config::default();
        config.dictation.dictionary = Some(PathBuf::from("/Users/you/words.txt"));
        assert_eq!(
            file(dir, Some(&config)),
            PathBuf::from("/Users/you/words.txt")
        );
    }

    /// Spec 93, AC-11 (R-14): the old text file goes into an empty store once; a missing file
    /// imports nothing and is not read again.
    #[test]
    fn the_old_file_is_imported_once() {
        let dir = temp_dir("import");
        let mut store = DictionaryStore::open(&dir.join("dictionary.sqlite")).unwrap();
        std::fs::write(dir.join("dictionary.txt"), "Zentrix: zen trix\n").unwrap();
        assert_eq!(
            import_once(&mut store, &dir.join("dictionary.txt")),
            Ok(true)
        );
        assert_eq!(
            import_once(&mut store, &dir.join("dictionary.txt")),
            Ok(false)
        );
        assert_eq!(store.dictionary().unwrap().terms().len(), 1);

        let mut empty = DictionaryStore::open(&dir.join("other.sqlite")).unwrap();
        assert_eq!(import_once(&mut empty, &dir.join("missing.txt")), Ok(true));
        assert!(empty.dictionary().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Spec 93, AC-10 (R-13): a file is read as CSV by its name, otherwise as text; the export
    /// reads back as the same list.
    #[test]
    fn import_and_export_round_trip() {
        let d = Dictionary::parse("Zentrix: zen trix, sun trix\nLangChain\n");
        let (name, csv) = export(&d, "csv");
        assert_eq!(name, "HyperMe dictionary.csv");
        assert_eq!(Dictionary::new(parse_import(name, &csv).unwrap()), d);
        let (name, text) = export(&d, "text");
        assert_eq!(name, "HyperMe dictionary.txt");
        assert_eq!(Dictionary::new(parse_import(name, &text).unwrap()), d);
    }

    /// Spec 93, AC-12 (R-12, R-17): a fix is saved at once and keeps what was there; a common
    /// heard form saves nothing until confirmed; a refusal changes nothing.
    #[test]
    fn a_fix_is_saved_in_the_store() {
        let dir = temp_dir("fix");
        let mut store = DictionaryStore::open(&dir.join("dictionary.sqlite")).unwrap();
        store
            .save(
                vec![Term {
                    written: "Zentrix".to_owned(),
                    heard: vec!["zen trix".to_owned()],
                }],
                Source::Manual,
            )
            .unwrap();

        let fixed = correct(&mut store, &fix("cloud", "Claude", false))
            .unwrap()
            .unwrap();
        assert_eq!(store.dictionary().unwrap(), fixed);
        assert_eq!(fixed.apply("ask cloud").0, "ask Claude");

        assert_eq!(correct(&mut store, &fix("mesa", "Mesa", false)), Ok(None));
        assert_eq!(store.dictionary().unwrap(), fixed);
        assert!(
            correct(&mut store, &fix("mesa", "Mesa", true))
                .unwrap()
                .is_some()
        );

        let before = store.dictionary().unwrap();
        assert!(correct(&mut store, &fix("x", "a:b", false)).is_err());
        assert_eq!(store.dictionary().unwrap(), before);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
