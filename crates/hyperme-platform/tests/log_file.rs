//! Spec 182, R-14: the warnings file stays under its size, with one rotated copy.

use hyperme_core::warnings::LineSink;
use hyperme_platform::log_file::RotatingFile;
use std::path::{Path, PathBuf};

fn temp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("log_file")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// R-14: past the limit the file moves to `.1`, replacing an older copy, and a new file starts.
#[test]
fn r14_rotates_at_the_limit_keeping_one_copy() {
    let dir = temp_dir("rotates");
    let path = dir.join("logs").join("warnings.log");
    let file = RotatingFile::new(path.clone(), 100);
    let line = "x".repeat(39); // 40 bytes with its newline
    for _ in 0..7 {
        file.write_line(&line);
    }
    let current = std::fs::read_to_string(&path).unwrap();
    let older = std::fs::read_to_string(path.with_extension("log.1")).unwrap();
    assert!(current.len() <= 100 && older.len() <= 100);
    assert_eq!(current.lines().count(), 1, "{current}");
    assert_eq!(older.lines().count(), 2, "{older}");
    assert!(!dir.join("logs").join("warnings.log.2").exists());
}

/// R-14: a file left by an earlier run counts toward the limit.
#[test]
fn r14_an_existing_file_counts() {
    let dir = temp_dir("existing");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("warnings.log");
    std::fs::write(&path, "y".repeat(90) + "\n").unwrap();
    RotatingFile::new(path.clone(), 100).write_line("new line here");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new line here\n");
}
