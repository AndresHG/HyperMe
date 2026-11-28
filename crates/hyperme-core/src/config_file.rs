//! `config.toml` as one shared service: read and parsed once, then again only when the file
//! changes on disk (its size or modified time), and rewritten whole through one function that
//! keeps the cache in step. The app holds one of these instead of reading the file in every
//! command.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

use crate::config::{Config, ConfigError};

/// What tells one version of the file from the next without reading it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    modified: Option<SystemTime>,
    len: u64,
}

#[derive(Debug)]
struct Cached {
    stamp: Stamp,
    text: Arc<str>,
    config: Arc<Config>,
}

/// The config file at one path, cached.
#[derive(Debug)]
pub struct ConfigFile {
    path: PathBuf,
    cache: Mutex<Option<Cached>>,
}

impl ConfigFile {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            cache: Mutex::new(None),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The config, created with the defaults if the file is missing.
    pub fn load(&self) -> Result<Arc<Config>, ConfigError> {
        self.load_with_text().map(|(_, config)| config)
    }

    /// The file's text as it was parsed, with the config.
    pub fn load_with_text(&self) -> Result<(Arc<str>, Arc<Config>), ConfigError> {
        self.load_locked(&mut self.lock())
    }

    fn load_locked(
        &self,
        cache: &mut Option<Cached>,
    ) -> Result<(Arc<str>, Arc<Config>), ConfigError> {
        let stamp = match self.stamp() {
            Some(stamp) => stamp,
            None => {
                // Missing: write the defaults, then read them back like any other file.
                Config::load_or_create(&self.path)?;
                self.stamp()
                    .ok_or_else(|| self.io(std::io::ErrorKind::NotFound.into()))?
            }
        };
        if let Some(cached) = cache.as_ref().filter(|c| c.stamp == stamp) {
            return Ok((cached.text.clone(), cached.config.clone()));
        }
        let text = std::fs::read_to_string(&self.path).map_err(|e| self.io(e))?;
        let config = Arc::new(Config::parse(&text, &self.path)?);
        let text: Arc<str> = text.into();
        *cache = Some(Cached {
            stamp,
            text: text.clone(),
            config: config.clone(),
        });
        Ok((text, config))
    }

    /// Rewrites the file with `change`, which gets the current text and returns the new one. The
    /// result must parse, or nothing is written. Writes beside the file, then renames, so a crash
    /// never leaves half a config.
    pub fn rewrite(
        &self,
        change: impl FnOnce(&str) -> Result<String, ConfigError>,
    ) -> Result<Arc<Config>, ConfigError> {
        // Held from the read to the write, so two rewrites never lose each other's change.
        let mut cache = self.lock();
        let (text, _) = self.load_locked(&mut cache)?;
        let updated = change(&text)?;
        let config = Arc::new(Config::parse(&updated, &self.path)?);
        let temp = self.path.with_extension("toml.new");
        std::fs::write(&temp, &updated).map_err(|source| ConfigError::Io {
            path: temp.clone(),
            source,
        })?;
        std::fs::rename(&temp, &self.path).map_err(|e| self.io(e))?;
        *cache = self.stamp().map(|stamp| Cached {
            stamp,
            text: updated.into(),
            config: config.clone(),
        });
        Ok(config)
    }

    fn stamp(&self) -> Option<Stamp> {
        let meta = std::fs::metadata(&self.path).ok()?;
        Some(Stamp {
            modified: meta.modified().ok(),
            len: meta.len(),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Option<Cached>> {
        self.cache.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn io(&self, source: std::io::Error) -> ConfigError {
        ConfigError::Io {
            path: self.path.clone(),
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{self, DEFAULT_CONFIG};

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("hyperme-config-file-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("config.toml")
    }

    /// A missing file is created with the defaults, as `Config::load_or_create` does.
    #[test]
    fn a_missing_file_gets_the_defaults() {
        let path = temp("missing");
        let file = ConfigFile::new(&path);
        assert_eq!(*file.load().unwrap(), Config::default());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), DEFAULT_CONFIG);
    }

    /// The file is parsed once while it is unchanged: the same `Arc` comes back.
    #[test]
    fn an_unchanged_file_is_read_once() {
        let path = temp("once");
        std::fs::write(&path, DEFAULT_CONFIG).unwrap();
        let file = ConfigFile::new(&path);
        let first = file.load().unwrap();
        assert!(Arc::ptr_eq(&first, &file.load().unwrap()));
    }

    /// An edit made outside the app (a different size) is seen on the next load.
    #[test]
    fn an_edit_on_disk_is_seen() {
        let path = temp("edit");
        std::fs::write(&path, DEFAULT_CONFIG).unwrap();
        let file = ConfigFile::new(&path);
        let before = file.load().unwrap();
        let edited =
            config::set_value(DEFAULT_CONFIG, "cleanup", "model", "\"edited-model\"").unwrap();
        std::fs::write(&path, &edited).unwrap();
        let after = file.load().unwrap();
        assert!(!Arc::ptr_eq(&before, &after));
        assert_eq!(after.cleanup.model, "edited-model");
    }

    /// A rewrite saves the file and the next load returns it without reading the disk again.
    #[test]
    fn a_rewrite_saves_and_refreshes() {
        let path = temp("rewrite");
        std::fs::write(&path, DEFAULT_CONFIG).unwrap();
        let file = ConfigFile::new(&path);
        let saved = file
            .rewrite(|text| config::set_value(text, "cleanup", "model", "\"saved-model\""))
            .unwrap();
        assert_eq!(saved.cleanup.model, "saved-model");
        assert!(Arc::ptr_eq(&saved, &file.load().unwrap()));
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("saved-model")
        );
        assert!(!path.with_extension("toml.new").exists());
    }

    /// A change whose result does not parse writes nothing.
    #[test]
    fn a_bad_rewrite_writes_nothing() {
        let path = temp("bad");
        std::fs::write(&path, DEFAULT_CONFIG).unwrap();
        let file = ConfigFile::new(&path);
        assert!(file.rewrite(|_| Ok("not = [valid".to_string())).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), DEFAULT_CONFIG);
    }

    /// Two rewrites at once both land: neither works from text the other is replacing.
    #[test]
    fn concurrent_rewrites_keep_both_changes() {
        let path = temp("concurrent");
        std::fs::write(&path, DEFAULT_CONFIG).unwrap();
        let file = ConfigFile::new(&path);
        std::thread::scope(|s| {
            s.spawn(|| {
                file.rewrite(|t| config::set_value(t, "cleanup", "model", "\"a\""))
                    .unwrap()
            });
            s.spawn(|| {
                file.rewrite(|t| config::set_value(t, "speech", "model", "\"b\""))
                    .unwrap()
            });
        });
        let config = file.load().unwrap();
        assert_eq!(
            (config.cleanup.model.as_str(), config.speech.model.as_str()),
            ("a", "b")
        );
    }
}
