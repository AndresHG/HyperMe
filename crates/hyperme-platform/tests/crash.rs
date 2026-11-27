//! Spec 200 (Diagnostics), slice 4: crash files from the panic hook, their pruning and notice,
//! and the macOS crash reports the bundle includes. Every folder is fresh under the target
//! directory. Only one test installs the hook, since it is process-wide.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hyperme_platform::crash::{CrashFiles, CrashFolder, LocalCrashFiles, system_reports};

const DAY_S: u64 = 24 * 60 * 60;

fn temp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("crash")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn now_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn crash_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("crash-"))
        .collect();
    names.sort();
    names
}

/// AC-9 (R-21, R-22, R-25): a panic in a thread writes one crash file with the version, the
/// thread, the location and a backtrace; the message is scrubbed and cut to 200 characters; the
/// panic still unwinds; the next start sees the notice once.
#[test]
fn a_panic_writes_one_scrubbed_crash_file_and_still_unwinds() {
    let dir = temp_dir("hook");
    let folder = CrashFolder::new(dir.clone(), Some("/Users/someone".into()), "9.9.9");
    folder.clone().install();

    let long = "x ".repeat(300);
    let result = std::thread::Builder::new()
        .name("dictation".into())
        .spawn(move || {
            panic!("could not read /Users/someone/notes.txt for ana@example.com {long}");
        })
        .unwrap()
        .join();
    assert!(result.is_err(), "the panic still unwinds");

    let names = crash_names(&dir);
    assert_eq!(names.len(), 1, "{names:?}");
    let text = fs::read_to_string(dir.join(&names[0])).unwrap();
    assert!(text.contains("HyperMe 9.9.9"), "{text}");
    assert!(text.contains("thread: dictation"), "{text}");
    assert!(text.contains("tests/crash.rs:"), "{text}");
    assert!(text.contains("backtrace:"), "{text}");
    assert!(!text.contains("/Users/someone"), "{text}");
    assert!(!text.contains("ana@example.com"), "{text}");
    assert!(text.contains("~/notes.txt"), "{text}");
    let message = text
        .lines()
        .find_map(|l| l.strip_prefix("message: "))
        .unwrap();
    assert!(message.chars().count() <= 200, "{}", message.len());

    assert!(folder.take_notice(), "the next start shows the notice");
    assert!(!folder.take_notice(), "only once per crash");
}

/// AC-9 (R-23): at most 10 crash files and none older than 30 days are kept; the newest stay.
#[test]
fn pruning_keeps_ten_crash_files_from_the_last_thirty_days() {
    let dir = temp_dir("prune");
    let now = now_s();
    for i in 0..12u64 {
        fs::write(dir.join(format!("crash-{}.txt", now - 12 + i)), "x").unwrap();
    }
    fs::write(dir.join(format!("crash-{}.txt", now - 31 * DAY_S)), "old").unwrap();
    fs::write(dir.join("notes.txt"), "not a crash").unwrap();
    let folder = CrashFolder::new(dir.clone(), None, "9.9.9");
    folder.prune(now);
    let names = crash_names(&dir);
    assert_eq!(names.len(), 10);
    assert_eq!(
        names[0],
        format!("crash-{}.txt", now - 10),
        "the oldest 2 went"
    );
    assert!(dir.join("notes.txt").exists());
}

/// R-14, R-21: the crash files of the last 30 days, newest first, for the bundle.
#[test]
fn recent_crash_files_are_read_newest_first() {
    let dir = temp_dir("recent");
    let now = now_s();
    fs::write(dir.join(format!("crash-{}.txt", now - 100)), "older").unwrap();
    fs::write(dir.join(format!("crash-{}.txt", now - 10)), "newer").unwrap();
    fs::write(dir.join(format!("crash-{}.txt", now - 40 * DAY_S)), "gone").unwrap();
    let folder = CrashFolder::new(dir, None, "9.9.9");
    let texts: Vec<String> = folder.recent(now).into_iter().map(|c| c.text).collect();
    assert_eq!(texts, ["newer", "older"]);
}

fn ips(dir: &Path, name: &str, body: &str, age_days: u64) {
    let path = dir.join(name);
    fs::write(&path, body).unwrap();
    let at = SystemTime::now() - Duration::from_secs(age_days * DAY_S);
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(at)
        .unwrap();
}

/// AC-10 (R-24): of the macOS crash reports, only HyperMe's newest two from the last 30 days are
/// read, scrubbed of the home folder and cut to 64 KB.
#[test]
fn only_hypermes_newest_two_system_reports_are_read_scrubbed_and_cut() {
    let dir = temp_dir("ips");
    let big = format!("path /Users/someone/x {}", "y ".repeat(50 * 1024));
    ips(&dir, "HyperMe-2026-11-20-100000.ips", "oldest", 7);
    ips(&dir, "HyperMe-2026-11-25-100000.ips", &big, 2);
    ips(
        &dir,
        "hyperme-2026-11-26-100000.ips",
        "newest /Users/someone/y",
        1,
    );
    ips(&dir, "HyperMe-2026-10-01-100000.ips", "too old", 40);
    ips(&dir, "OtherApp-2026-11-27-100000.ips", "not ours", 0);
    ips(&dir, "HyperMe-2026-11-27-100000.txt", "not a report", 0);

    let reports = system_reports(&dir, Some("/Users/someone"), SystemTime::now());
    assert_eq!(reports.len(), 2);
    assert!(
        reports[0].text.starts_with("newest ~/y"),
        "{}",
        reports[0].text
    );
    assert!(reports[1].text.starts_with("path ~/x"));
    assert!(reports[1].text.len() <= 64 * 1024);
    assert!(reports.iter().all(|r| !r.text.contains("not ours")));
    assert!(reports.iter().all(|r| !r.text.contains("oldest")));
    assert_eq!(reports[0].name, "hyperme-2026-11-26-100000.ips");

    assert!(system_reports(&dir.join("missing"), None, SystemTime::now()).is_empty());
}

/// The sources the bundle reads, behind the trait Linux and Windows will implement.
#[test]
fn local_crash_files_combine_both_sources() {
    let dir = temp_dir("local");
    let reports = temp_dir("local-reports");
    let now = now_s();
    fs::write(dir.join(format!("crash-{now}.txt")), "panic").unwrap();
    ips(&reports, "HyperMe-1.ips", "native", 0);
    let files = LocalCrashFiles::new(CrashFolder::new(dir, None, "9.9.9"), Some(reports), None);
    assert_eq!(files.crashes().len(), 1);
    assert_eq!(files.system_reports().len(), 1);
}
