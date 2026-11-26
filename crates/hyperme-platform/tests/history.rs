//! Slice 10b (non-UI half): the dictation history on disk. Tests cite the spec's requirements
//! and acceptance criteria. Audio is synthetic; every store lives in a fresh folder under the
//! target directory.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use hyperme_core::config::Retention;
use hyperme_core::traits::History;
use hyperme_platform::history::{HistorySettings, SqliteHistory};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

fn temp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("history")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A synthetic tone, `ms` long at 24 kHz.
fn tone(ms: usize) -> Vec<i16> {
    (0..ms * 24)
        .map(|i| ((i as f32 * 0.05).sin() * 8000.0) as i16)
        .collect()
}

fn on(retention: Retention, size_limit_mb: u64) -> HistorySettings {
    HistorySettings {
        enabled: true,
        retention,
        size_limit_bytes: size_limit_mb * 1024 * 1024,
    }
}

fn off() -> HistorySettings {
    HistorySettings {
        enabled: false,
        ..on(Retention::Days(30), 1024)
    }
}

/// A store on a manual clock, starting at day 100.
fn open_store(dir: &Path, settings: HistorySettings) -> (SqliteHistory, Arc<AtomicI64>) {
    let clock = Arc::new(AtomicI64::new(100 * DAY_MS));
    let now = clock.clone();
    let store =
        SqliteHistory::open_with_clock(dir, settings, move || now.load(Ordering::SeqCst)).unwrap();
    (store, clock)
}

fn audio_files(dir: &Path) -> usize {
    std::fs::read_dir(dir.join("audio")).map_or(0, |entries| entries.count())
}

fn dictate(store: &mut SqliteHistory, text: &str, ms: usize) -> i64 {
    let id = store.begin(Some("TextEdit")).unwrap();
    for frame in tone(ms).chunks(480) {
        store.append(id, frame).unwrap();
    }
    store.done(id, text, Some("en")).unwrap();
    id
}

/// AC-15 (R-18): with history on, a dictation's audio and text are kept, and "Copy last
/// dictation" returns its text.
#[test]
fn history_keeps_audio_and_text() {
    let dir = temp_dir("keeps");
    let (mut store, _) = open_store(&dir, on(Retention::Days(30), 1024));
    let first = dictate(&mut store, "First note.", 1000);
    let second = dictate(&mut store, "Second note.", 500);
    assert_eq!(store.audio(first).unwrap(), tone(1000));
    assert_eq!(store.audio(second).unwrap(), tone(500));
    assert_eq!(store.last_text().unwrap().as_deref(), Some("Second note."));
    let items = store.items().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].text.as_deref(), Some("Second note."));
    assert_eq!(items[0].app.as_deref(), Some("TextEdit"));
    assert_eq!(items[0].language.as_deref(), Some("en"));
    assert_eq!(audio_files(&dir), 2);
}

/// R-18, R-31: audio reaches the disk while it is captured, so a crash keeps it; at the next
/// launch an interrupted dictation becomes a failed one that can be retried.
#[test]
fn audio_survives_a_crash_while_recording() {
    let dir = temp_dir("crash");
    let id = {
        let (mut store, _) = open_store(&dir, on(Retention::Days(30), 1024));
        let id = store.begin(None).unwrap();
        store.append(id, &tone(200)).unwrap();
        // Dropped without done or failed: the app died.
        id
    };
    let (store, _) = open_store(&dir, on(Retention::Days(30), 1024));
    assert_eq!(store.audio(id).unwrap(), tone(200));
    assert_eq!(store.items().unwrap()[0].state, "failed");
}

/// AC-15 (R-18): items older than the retention period are deleted; "unlimited" keeps them.
#[test]
fn retention_deletes_old_items() {
    let dir = temp_dir("retention");
    let (mut store, clock) = open_store(&dir, on(Retention::Days(30), 1024));
    dictate(&mut store, "Old.", 100);
    clock.fetch_add(20 * DAY_MS, Ordering::SeqCst);
    dictate(&mut store, "Newer.", 100);
    clock.fetch_add(11 * DAY_MS, Ordering::SeqCst);
    store.cleanup().unwrap();
    let texts: Vec<_> = store
        .items()
        .unwrap()
        .into_iter()
        .filter_map(|i| i.text)
        .collect();
    assert_eq!(texts, ["Newer."]);
    assert_eq!(audio_files(&dir), 1);

    let dir = temp_dir("unlimited");
    let (mut store, clock) = open_store(&dir, on(Retention::Unlimited, 1024));
    dictate(&mut store, "Kept.", 100);
    clock.fetch_add(3000 * DAY_MS, Ordering::SeqCst);
    store.cleanup().unwrap();
    assert_eq!(store.items().unwrap().len(), 1);
}

/// AC-15 (R-18): past the size limit the oldest items go first, and failed items last.
#[test]
fn size_limit_deletes_oldest_and_failed_last() {
    let dir = temp_dir("size");
    // 1 MB limit; each 10 s dictation is about 469 KB of audio.
    let (mut store, clock) = open_store(&dir, on(Retention::Unlimited, 1));
    let failed = store.begin(None).unwrap();
    store.append(failed, &tone(10_000)).unwrap();
    store.failed(failed, "connection dropped").unwrap();
    for text in ["One.", "Two.", "Three."] {
        clock.fetch_add(1000, Ordering::SeqCst);
        dictate(&mut store, text, 10_000);
    }
    let items = store.items().unwrap();
    let kept: Vec<_> = items
        .iter()
        .map(|i| i.text.clone().unwrap_or_else(|| i.state.clone()))
        .collect();
    // The failed item is the oldest but is kept; "One." and "Two." made room.
    assert_eq!(kept, ["Three.", "failed"]);
    assert!(store.total_bytes().unwrap() <= 1024 * 1024);
    assert_eq!(audio_files(&dir), 2);
}

/// AC-15 (R-28): with history off, no audio or text remains after a successful dictation, and
/// nothing remains of a cancelled one.
#[test]
fn history_off_leaves_nothing_after_success() {
    let dir = temp_dir("off");
    let (mut store, _) = open_store(&dir, off());
    dictate(&mut store, "Private.", 500);
    let tap = store.begin(None).unwrap();
    store.append(tap, &tone(100)).unwrap();
    store.discard(tap).unwrap();
    assert!(store.items().unwrap().is_empty());
    assert_eq!(audio_files(&dir), 0);
    assert_eq!(store.last_text().unwrap(), None);
}

/// AC-23 (R-31): with history off, a failed dictation keeps its audio for a retry; it is gone
/// after a successful retry, and after 24 hours when not retried.
#[test]
fn history_off_keeps_failed_audio_for_24_hours() {
    let dir = temp_dir("off-failed");
    let (mut store, clock) = open_store(&dir, off());

    let retried = store.begin(None).unwrap();
    store.append(retried, &tone(300)).unwrap();
    store.failed(retried, "connection dropped").unwrap();
    assert_eq!(store.audio(retried).unwrap(), tone(300));
    store.done(retried, "Retried text.", Some("en")).unwrap();
    assert!(store.audio(retried).is_err());
    assert_eq!(audio_files(&dir), 0);

    let abandoned = store.begin(None).unwrap();
    store.append(abandoned, &tone(300)).unwrap();
    store.failed(abandoned, "connection dropped").unwrap();
    clock.fetch_add(23 * 60 * 60 * 1000, Ordering::SeqCst);
    store.cleanup().unwrap();
    assert!(store.audio(abandoned).is_ok(), "kept for 24 hours");
    clock.fetch_add(2 * 60 * 60 * 1000, Ordering::SeqCst);
    store.cleanup().unwrap();
    assert!(store.audio(abandoned).is_err(), "gone after 24 hours");
    assert!(store.items().unwrap().is_empty());
}

/// R-31: discarding a failed dictation deletes its audio.
#[test]
fn discarding_a_failed_dictation_deletes_its_audio() {
    let dir = temp_dir("discard-failed");
    let (mut store, _) = open_store(&dir, on(Retention::Days(30), 1024));
    let id = store.begin(None).unwrap();
    store.append(id, &tone(100)).unwrap();
    store.failed(id, "dropped").unwrap();
    store.discard(id).unwrap();
    assert!(store.items().unwrap().is_empty());
    assert_eq!(audio_files(&dir), 0);
}

/// The audio files are standard WAV, so the user can play them.
#[test]
fn audio_files_are_wav() {
    let dir = temp_dir("wav");
    let (mut store, _) = open_store(&dir, on(Retention::Days(30), 1024));
    let id = dictate(&mut store, "Wave.", 100);
    let bytes = std::fs::read(dir.join("audio").join(format!("{id}.wav"))).unwrap();
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(&bytes[8..16], b"WAVEfmt ");
    assert_eq!(
        u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
        24_000
    );
    assert_eq!(
        u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize,
        100 * 24 * 2
    );
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize,
        bytes.len() - 8
    );
}
