//! The warnings and errors file on disk (specs/182-trace-store, R-14): appended a line at a
//! time, and moved to `<name>.1` once it would pass its limit, so at most two files are kept.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use hyperme_core::warnings::LineSink;

/// The most a warnings file holds before it rotates (R-14).
pub const LIMIT: u64 = 1024 * 1024;

/// A log file that rotates at a size limit, keeping one older copy.
#[derive(Debug)]
pub struct RotatingFile {
    path: PathBuf,
    limit: u64,
    open: Mutex<Option<(File, u64)>>,
}

impl RotatingFile {
    /// A file at `path`, created with its folder on the first line.
    pub fn new(path: PathBuf, limit: u64) -> Self {
        Self {
            path,
            limit,
            open: Mutex::new(None),
        }
    }

    fn open(&self) -> Option<(File, u64)> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).ok()?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .ok()?;
        let size = file.metadata().map(|m| m.len()).unwrap_or(0);
        Some((file, size))
    }

    fn rotated(&self) -> PathBuf {
        let mut name = self.path.as_os_str().to_owned();
        name.push(".1");
        PathBuf::from(name)
    }
}

impl LineSink for RotatingFile {
    fn write_line(&self, line: &str) {
        let bytes = line.len() as u64 + 1;
        let mut open = self.open.lock().unwrap_or_else(PoisonError::into_inner);
        if open.is_none() {
            *open = self.open();
        }
        if open
            .as_ref()
            .is_some_and(|(_, size)| size + bytes > self.limit)
        {
            *open = None;
            let _ = std::fs::rename(&self.path, self.rotated());
            *open = self.open();
        }
        if let Some((file, size)) = open.as_mut()
            && writeln!(file, "{line}").is_ok()
        {
            *size += bytes;
        }
    }
}
