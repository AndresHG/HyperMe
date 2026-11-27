//! Settings → Dictionary (spec 93, slice D1b): the user's words, edited in the Settings window
//! and saved to the dictionary text file. A save applies from the next dictation, with no
//! restart (R-10a). Fix a word in Home's History adds to the same file (D5, R-12).

use std::path::{Path, PathBuf};

use hyperme_core::config::Config;
use hyperme_core::dictionary::{Corrected, Dictionary, Term};
use hyperme_core::traits::{self, Added, Correction, Error, ErrorKind, Refusal};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

/// The dictionary file: the one named in the config, or `dictionary.txt` in the app's data
/// folder.
pub fn file(dir: &Path, config: Option<&Config>) -> PathBuf {
    config
        .and_then(|c| c.dictation.dictionary.clone())
        .unwrap_or_else(|| dir.join("dictionary.txt"))
}

/// Reads the dictionary file. A missing file is an empty dictionary; an unreadable one is an
/// empty dictionary and the reason.
pub fn read(path: &Path) -> (Dictionary, Option<String>) {
    match std::fs::read_to_string(path) {
        Ok(text) => (Dictionary::parse(&text), None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Dictionary::default(), None),
        Err(e) => (
            Dictionary::default(),
            Some(format!("{} could not be read: {e}", path.display())),
        ),
    }
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

fn path_for(app: &AppHandle) -> PathBuf {
    let dir = crate::runtime::data_dir(app);
    let config = Config::load_or_create(&dir.join("config.toml")).ok();
    file(&dir, config.as_ref())
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

/// The entries from the page as terms: trimmed, with empty heard forms dropped.
fn terms(entries: Vec<Entry>) -> Vec<Term> {
    entries
        .into_iter()
        .map(|e| Term {
            written: e.written.trim().to_owned(),
            heard: e
                .heard
                .iter()
                .map(|h| h.trim())
                .filter(|h| !h.is_empty())
                .map(str::to_owned)
                .collect(),
        })
        .collect()
}

/// Checks and writes the whole list, beside the file and then renamed, so a crash never leaves
/// half a dictionary.
fn save(path: &Path, entries: Vec<Entry>) -> Result<Dictionary, String> {
    let terms = terms(entries);
    Dictionary::check(&terms)?;
    let dictionary = Dictionary::new(terms);
    write(path, &dictionary)?;
    Ok(dictionary)
}

fn write(path: &Path, dictionary: &Dictionary) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let temp = path.with_extension("txt.new");
    std::fs::write(&temp, dictionary.to_text()).map_err(|e| format!("{}: {e}", temp.display()))?;
    std::fs::rename(&temp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Applies a fix to the file (R-12, R-17, AC-12): read fresh, so an edit in Settings is never
/// lost; written only when the fix is added. Returns the new dictionary when it changed.
fn correct(path: &Path, c: &Correction) -> Result<Option<Dictionary>, String> {
    let (dictionary, error) = read(path);
    if let Some(error) = error {
        return Err(error);
    }
    match dictionary.corrected(&c.heard, &c.wanted, c.confirmed_common)? {
        Corrected::Common => Ok(None),
        Corrected::Added(fixed) => {
            write(path, &fixed)?;
            Ok(Some(fixed))
        }
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
        let fixed = correct(&path_for(&self.app), &c).map_err(|message| Error {
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

#[tauri::command]
pub fn dictionary_view(app: AppHandle) -> View {
    let path = path_for(&app);
    let (dictionary, error) = read(&path);
    view(&path, &dictionary, error)
}

/// Saves the list from the page and hands it to the running dictation (R-10a).
#[tauri::command]
pub fn dictionary_save(app: AppHandle, entries: Vec<Entry>) -> Result<View, String> {
    let path = path_for(&app);
    let dictionary = save(&path, entries)?;
    let shown = view(&path, &dictionary, None);
    crate::runtime::update_dictionary(&app, dictionary);
    Ok(shown)
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

    fn entry(written: &str, heard: &[&str]) -> Entry {
        Entry {
            written: written.to_owned(),
            heard: heard.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    /// Spec 93, D1b: the configured file wins; otherwise dictionary.txt in the data folder.
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

    /// Spec 93, AC-10 (R-11, R-15): a save writes the file that a read gives back, trims what
    /// the page sent, and refuses a bad list without touching the file.
    #[test]
    fn save_writes_what_read_gives_back() {
        let dir = temp_dir("save");
        let path = dir.join("dictionary.txt");
        assert_eq!(read(&path), (Dictionary::default(), None));

        let saved = save(
            &path,
            vec![
                entry(" Zentrix ", &["zen trix", " ", "sun trix "]),
                entry("LangChain", &[]),
            ],
        )
        .unwrap();
        let (again, error) = read(&path);
        assert_eq!(error, None);
        assert_eq!(again, saved);
        assert_eq!(
            view(&path, &again, None).entries,
            vec![
                entry("Zentrix", &["zen trix", "sun trix"]),
                entry("LangChain", &[])
            ]
        );

        let refused = save(&path, vec![entry("Zentrix", &[]), entry("zentrix", &[])]);
        assert!(refused.unwrap_err().contains("twice"));
        assert_eq!(read(&path).0, saved);
        let _ = std::fs::remove_dir_all(&dir);
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

    /// Spec 93, AC-12 (R-12, R-17): a fix is written to the file at once and keeps what was
    /// there; a common heard form writes nothing until confirmed; a refusal leaves the file.
    #[test]
    fn a_fix_is_written_to_the_file() {
        let dir = temp_dir("fix");
        let path = dir.join("dictionary.txt");
        save(&path, vec![entry("Zentrix", &["zen trix"])]).unwrap();

        let fixed = correct(&path, &fix("cloud", "Claude", false))
            .unwrap()
            .unwrap();
        assert_eq!(read(&path).0, fixed);
        assert_eq!(
            view(&path, &fixed, None).entries,
            vec![entry("Zentrix", &["zen trix"]), entry("Claude", &["cloud"])]
        );

        assert_eq!(correct(&path, &fix("mesa", "Mesa", false)), Ok(None));
        assert_eq!(read(&path).0, fixed);
        assert!(
            correct(&path, &fix("mesa", "Mesa", true))
                .unwrap()
                .is_some()
        );

        let before = read(&path).0;
        assert!(correct(&path, &fix("x", "a:b", false)).is_err());
        assert_eq!(read(&path).0, before);

        // A missing file is an empty dictionary: the first fix makes it.
        let fresh = dir.join("new.txt");
        assert!(
            correct(&fresh, &fix("lang chain", "LangChain", false))
                .unwrap()
                .is_some()
        );
        assert_eq!(read(&fresh).0.terms().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
