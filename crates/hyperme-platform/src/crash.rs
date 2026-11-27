//! Crash files (specs/200-diagnostics, slice 4): a panic hook that leaves a file HyperMe can show,
//! since an app opened from Finder throws standard error away (R-21 to R-23), the notice the next
//! start reads (R-25), and on macOS the system's own crash reports for HyperMe (R-24).
//!
//! Only the panic's message is free text, and it is scrubbed and cut; a backtrace holds frames,
//! never values.

use std::backtrace::Backtrace;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hyperme_core::warnings::scrub;

/// Crash files kept at most (R-23).
pub const KEEP: usize = 10;
/// Crash files and system reports older than this are left out (R-23, R-24).
const MAX_AGE_S: u64 = 30 * 24 * 60 * 60;
/// The longest panic message written, in characters (R-22).
const MESSAGE_MAX: usize = 200;
/// System reports included at most, and the most of each that is kept (R-24).
const REPORTS: usize = 2;
const REPORT_MAX: usize = 64 * 1024;
const NOTICE: &str = "crashed";

/// One crash text for the bundle: the file's name and its contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashText {
    pub name: String,
    pub text: String,
}

/// The crash files the bundle reads. macOS adds its system reports; Linux and Windows add
/// theirs here later.
pub trait CrashFiles {
    /// HyperMe's own crash files from the last 30 days, newest first.
    fn crashes(&self) -> Vec<CrashText>;
    /// The system's crash reports for HyperMe, newest first, scrubbed and cut.
    fn system_reports(&self) -> Vec<CrashText>;
}

/// HyperMe's crash folder, `~/Library/Logs/HyperMe/crashes/` on macOS.
#[derive(Debug, Clone)]
pub struct CrashFolder {
    dir: PathBuf,
    home: Option<String>,
    version: String,
}

impl CrashFolder {
    pub fn new(dir: PathBuf, home: Option<String>, version: &str) -> Self {
        CrashFolder {
            dir,
            home,
            version: version.to_owned(),
        }
    }

    /// Installs the panic hook: write the crash file, then run the hook that was there before,
    /// so standard error and unwinding are unchanged (R-21).
    pub fn install(self) {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            self.record(info);
            previous(info);
        }));
    }

    fn record(&self, info: &PanicHookInfo<'_>) {
        let payload = info.payload();
        let message = payload
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(not text)".to_owned());
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown".to_owned());
        let thread = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_owned();
        let backtrace = Backtrace::force_capture().to_string();
        // A failed write must not panic inside the hook (design.md, failure modes).
        let _ = self.write(&message, &location, &thread, &backtrace, now_s());
    }

    fn write(
        &self,
        message: &str,
        location: &str,
        thread: &str,
        backtrace: &str,
        now: u64,
    ) -> std::io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let home = self.home.as_deref();
        let message: String = scrub(message, home).chars().take(MESSAGE_MAX).collect();
        let text = format!(
            "HyperMe {}\ntime: {now}\nthread: {thread}\nlocation: {}\nmessage: {}\nbacktrace:\n{}\n",
            self.version,
            scrub(location, home),
            message.replace('\n', " "),
            scrub(backtrace, home),
        );
        let mut file = self.new_file(now)?;
        file.write_all(text.as_bytes())?;
        fs::write(self.dir.join(NOTICE), now.to_string())
    }

    fn new_file(&self, now: u64) -> std::io::Result<File> {
        let mut n = 1;
        loop {
            let name = if n == 1 {
                format!("crash-{now}.txt")
            } else {
                format!("crash-{now}-{n}.txt")
            };
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(self.dir.join(name))
            {
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && n < 100 => n += 1,
                other => return other,
            }
        }
    }

    /// Deletes crash files older than 30 days, then all but the newest 10 (R-23). Run at start.
    pub fn prune(&self, now: u64) {
        let mut files = self.files();
        files.retain(|(at, path)| {
            let old = now.saturating_sub(*at) > MAX_AGE_S;
            if old {
                let _ = fs::remove_file(path);
            }
            !old
        });
        for (_, path) in files.iter().skip(KEEP) {
            let _ = fs::remove_file(path);
        }
    }

    /// Whether HyperMe crashed since the notice was last taken; taking it clears it, so the
    /// notice shows once per crash (R-25).
    pub fn take_notice(&self) -> bool {
        fs::remove_file(self.dir.join(NOTICE)).is_ok()
    }

    /// The crash files of the last 30 days, newest first.
    pub fn recent(&self, now: u64) -> Vec<CrashText> {
        self.files()
            .into_iter()
            .filter(|(at, _)| now.saturating_sub(*at) <= MAX_AGE_S)
            .filter_map(|(_, path)| {
                Some(CrashText {
                    name: path.file_name()?.to_string_lossy().into_owned(),
                    text: fs::read_to_string(&path).ok()?,
                })
            })
            .collect()
    }

    /// The crash files with the time in their names, newest first.
    fn files(&self) -> Vec<(u64, PathBuf)> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut files: Vec<(u64, PathBuf)> = entries
            .filter_map(|e| {
                let path = e.ok()?.path();
                let name = path.file_name()?.to_str()?;
                let stem = name.strip_prefix("crash-")?.strip_suffix(".txt")?;
                let at = stem.split('-').next()?.parse().ok()?;
                Some((at, path))
            })
            .collect();
        files.sort_by(|a, b| b.cmp(a));
        files
    }
}

/// The system's crash reports for HyperMe in `dir` (`~/Library/Logs/DiagnosticReports` on
/// macOS): the newest two `hyperme*.ips` from the last 30 days, any case, scrubbed of the home
/// folder and cut to 64 KB (R-24).
pub fn system_reports(dir: &Path, home: Option<&str>, now: SystemTime) -> Vec<CrashText> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let oldest = now - Duration::from_secs(MAX_AGE_S);
    let mut found: Vec<(SystemTime, PathBuf, String)> = entries
        .filter_map(|e| {
            let entry = e.ok()?;
            let name = entry.file_name().to_str()?.to_owned();
            let lower = name.to_ascii_lowercase();
            if !lower.starts_with("hyperme") || !lower.ends_with(".ips") {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            (modified >= oldest).then_some((modified, entry.path(), name))
        })
        .collect();
    found.sort_by_key(|f| std::cmp::Reverse(f.0));
    found
        .into_iter()
        .take(REPORTS)
        .filter_map(|(_, path, name)| {
            let bytes = fs::read(&path).ok()?;
            let text = scrub(&String::from_utf8_lossy(&bytes), home);
            Some(CrashText {
                name,
                text: cut(text, REPORT_MAX),
            })
        })
        .collect()
}

/// Cuts text to at most `max` bytes, on a character boundary.
fn cut(mut text: String, max: usize) -> String {
    if text.len() > max {
        let mut end = max;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    text
}

/// HyperMe's crash folder and, where the system keeps them, its crash reports.
#[derive(Debug, Clone)]
pub struct LocalCrashFiles {
    folder: CrashFolder,
    reports: Option<PathBuf>,
    home: Option<String>,
}

impl LocalCrashFiles {
    pub fn new(folder: CrashFolder, reports: Option<PathBuf>, home: Option<String>) -> Self {
        LocalCrashFiles {
            folder,
            reports,
            home,
        }
    }

    /// The folders for this system and home folder: on macOS, `~/Library/Logs/HyperMe/crashes`
    /// and `~/Library/Logs/DiagnosticReports`.
    pub fn for_home(home: &Path, version: &str) -> Self {
        let logs = home.join("Library").join("Logs");
        let home_text = Some(home.to_string_lossy().into_owned());
        let reports = cfg!(target_os = "macos").then(|| logs.join("DiagnosticReports"));
        LocalCrashFiles::new(
            CrashFolder::new(
                logs.join("HyperMe").join("crashes"),
                home_text.clone(),
                version,
            ),
            reports,
            home_text,
        )
    }

    pub fn folder(&self) -> &CrashFolder {
        &self.folder
    }
}

impl CrashFiles for LocalCrashFiles {
    fn crashes(&self) -> Vec<CrashText> {
        self.folder.recent(now_s())
    }

    fn system_reports(&self) -> Vec<CrashText> {
        self.reports.as_deref().map_or_else(Vec::new, |dir| {
            system_reports(dir, self.home.as_deref(), SystemTime::now())
        })
    }
}

fn now_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
