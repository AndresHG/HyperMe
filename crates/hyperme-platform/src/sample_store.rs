//! The sample folder (spec 267, design.md section 3): corrected dictations in the layout of a
//! quality-harness set, so `hyperme-quality run --set` reads it as it is. Portable code: the
//! app passes the folder (`~/Library/Application Support/HyperMe/samples/` on macOS).
//!
//! ```text
//! samples/es-h412.wav    the history's WAV, byte for byte
//! samples/gold.json      {"es-h412": "the corrected text"}       harness: gold texts
//! samples/raw.json       {"es-h412": "the speech transcript"}    harness: --reuse-raw
//! samples/apps.json      {"es-h412": "Slack"}                     harness: tone per app
//! samples/samples.json   {"es-h412": {facts}}                     the app's own record
//! ```
//!
//! Every file is written to `<name>.tmp` and renamed, the WAV first, so a reader never sees half
//! a file and a crash leaves at worst a WAV with no gold text (R-9, AC-8). One writer only.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use hyperme_core::samples::{SampleFacts, Samples, sample_name};
use hyperme_core::traits::{DictationId, Error, ErrorKind};
use serde_json::{Map, Value, json};

pub const GOLD: &str = "gold.json";
pub const RAW: &str = "raw.json";
pub const APPS: &str = "apps.json";
pub const FACTS: &str = "samples.json";
const JSON_FILES: [&str; 4] = [GOLD, RAW, APPS, FACTS];
/// Marks that the user agreed to keep samples (R-6). It survives Delete all samples; deleting
/// the folder in Finder asks again.
const AGREED: &str = ".agreed";

fn error(what: impl std::fmt::Display) -> Error {
    Error {
        kind: ErrorKind::Device,
        message: format!("samples: {what}"),
    }
}

/// The samples in one folder.
#[derive(Debug)]
pub struct SampleStore {
    dir: PathBuf,
    /// The names in `samples.json`, by history id, read at open.
    names: BTreeMap<DictationId, String>,
}

impl SampleStore {
    /// Opens the folder, which need not exist yet: it is made at the first save.
    pub fn open(dir: &Path) -> Result<Self, Error> {
        let mut store = SampleStore {
            dir: dir.to_owned(),
            names: BTreeMap::new(),
        };
        for (name, facts) in store.read(FACTS)? {
            if let Some(id) = facts.get("history_id").and_then(Value::as_i64) {
                store.names.insert(id, name);
            }
        }
        Ok(store)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn name_of(&self, id: DictationId) -> Option<String> {
        self.names.get(&id).cloned()
    }

    fn read(&self, file: &str) -> Result<Map<String, Value>, Error> {
        let path = self.dir.join(file);
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                serde_json::from_str(&text).map_err(|e| error(format!("{}: {e}", path.display())))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
            Err(e) => Err(error(format!("{}: {e}", path.display()))),
        }
    }

    /// Writes `bytes` to `<file>.tmp` and renames it over `file`.
    fn replace(&self, file: &str, bytes: &[u8]) -> Result<(), Error> {
        let path = self.dir.join(file);
        let tmp = self.dir.join(format!("{file}.tmp"));
        std::fs::write(&tmp, bytes)
            .and_then(|()| std::fs::rename(&tmp, &path))
            .map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                error(format!("writing {}: {e}", path.display()))
            })
    }

    /// Sets (`Some`) or removes (`None`) `name` in one JSON file.
    fn update(&self, file: &str, name: &str, value: Option<Value>) -> Result<(), Error> {
        let mut map = self.read(file)?;
        let changed = match value {
            Some(value) => map.insert(name.to_owned(), value.clone()) != Some(value),
            None => map.remove(name).is_some(),
        };
        if !changed {
            return Ok(());
        }
        let text = serde_json::to_string_pretty(&map).map_err(error)? + "\n";
        self.replace(file, text.as_bytes())
    }

    fn remove_wav(&self, name: &str) -> Result<(), Error> {
        match std::fs::remove_file(self.dir.join(format!("{name}.wav"))) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(error(format!("deleting {name}.wav: {e}"))),
        }
    }

    /// The folder's own files: samples and the JSON files, nothing else a user put there.
    fn own_files(&self) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                let file = p.file_name().and_then(|f| f.to_str()).unwrap_or_default();
                let wav = p.extension().is_some_and(|x| x == "wav")
                    && self.names.values().any(|n| file == format!("{n}.wav"));
                wav || JSON_FILES
                    .iter()
                    .any(|j| file == *j || file == format!("{j}.tmp"))
            })
            .collect()
    }
}

impl Samples for SampleStore {
    fn save(
        &mut self,
        facts: &SampleFacts,
        audio_wav: &[u8],
        corrected: &str,
    ) -> Result<(), Error> {
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| error(format!("creating {}: {e}", self.dir.display())))?;
        let name = sample_name(facts.history_id, facts.language.as_deref());
        // A sample whose language changed keeps one name: the old one goes.
        if self
            .name_of(facts.history_id)
            .is_some_and(|old| old != name)
        {
            self.delete(facts.history_id)?;
        }
        self.replace(&format!("{name}.wav"), audio_wav)?;
        self.update(GOLD, &name, Some(Value::String(corrected.to_owned())))?;
        let transcript = facts.transcript.as_deref().filter(|t| !t.trim().is_empty());
        self.update(RAW, &name, transcript.map(|t| Value::String(t.to_owned())))?;
        self.update(APPS, &name, facts.app.clone().map(Value::String))?;
        let mut record = json!({
            "history_id": facts.history_id,
            "saved_ms": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as i64),
            "dictated_ms": facts.dictated_ms,
        });
        for (key, value) in [
            ("language", &facts.language),
            ("cleanup_level", &facts.cleanup_level),
            ("speech", &facts.speech),
            ("cleanup", &facts.cleanup),
            ("first_text", &facts.first_text),
        ] {
            if let Some(value) = value {
                record[key] = Value::String(value.clone());
            }
        }
        self.update(FACTS, &name, Some(record))?;
        self.names.insert(facts.history_id, name);
        Ok(())
    }

    fn delete(&mut self, history_id: DictationId) -> Result<(), Error> {
        let Some(name) = self.name_of(history_id) else {
            return Ok(());
        };
        // The record goes first: a crash after it leaves files no sample points to, which
        // the next save of the same dictation overwrites.
        for file in [FACTS, GOLD, RAW, APPS] {
            self.update(file, &name, None)?;
        }
        self.remove_wav(&name)?;
        self.names.remove(&history_id);
        Ok(())
    }

    fn delete_all(&mut self) -> Result<(), Error> {
        for path in self.own_files() {
            std::fs::remove_file(&path)
                .map_err(|e| error(format!("deleting {}: {e}", path.display())))?;
        }
        self.names.clear();
        Ok(())
    }

    fn has(&self, history_id: DictationId) -> bool {
        self.name_of(history_id).is_some()
    }

    fn summary(&self) -> Result<(usize, u64), Error> {
        let bytes = self
            .own_files()
            .iter()
            .filter_map(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .sum();
        Ok((self.names.len(), bytes))
    }

    fn agreed(&self) -> bool {
        self.dir.join(AGREED).is_file()
    }

    fn agree(&mut self) -> Result<(), Error> {
        std::fs::create_dir_all(&self.dir)
            .and_then(|()| std::fs::write(self.dir.join(AGREED), ""))
            .map_err(|e| error(format!("creating {}: {e}", self.dir.display())))
    }
}
