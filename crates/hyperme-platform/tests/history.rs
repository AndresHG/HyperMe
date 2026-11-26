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

// Home window and History (#98), slice 1: the store half of History and the Home page.

fn fail(store: &mut SqliteHistory, message: &str, ms: usize) -> i64 {
    let id = store.begin(Some("Mail")).unwrap();
    store.append(id, &tone(ms)).unwrap();
    store.failed(id, message).unwrap();
    id
}

/// #98 R-11, R-16: an item says whether its audio is still there, and why it failed.
#[test]
fn items_carry_audio_flag_and_message() {
    let dir = temp_dir("item-fields");
    let (mut store, _) = open_store(&dir, on(Retention::Days(30), 1024));
    let done = dictate(&mut store, "Done.", 100);
    let failed = fail(&mut store, "connection dropped", 100);
    let gone = dictate(&mut store, "Audio gone.", 100);
    std::fs::remove_file(dir.join("audio").join(format!("{gone}.wav"))).unwrap();

    let done = store.item(done).unwrap().unwrap();
    assert!(done.has_audio);
    assert_eq!(done.message, None);
    let failed = store.item(failed).unwrap().unwrap();
    assert!(failed.has_audio);
    assert_eq!(failed.message.as_deref(), Some("connection dropped"));
    assert!(!store.item(gone).unwrap().unwrap().has_audio);
    assert_eq!(store.item(9999).unwrap(), None);
}

/// #98 AC-12 (R-18): Delete removes the row's text and its audio file.
#[test]
fn delete_item_removes_text_and_audio() {
    let dir = temp_dir("delete-item");
    let (mut store, _) = open_store(&dir, on(Retention::Days(30), 1024));
    let keep = dictate(&mut store, "Keep.", 100);
    let gone = dictate(&mut store, "Delete me.", 100);
    store.delete_item(gone).unwrap();
    assert_eq!(store.item(gone).unwrap(), None);
    assert!(!dir.join("audio").join(format!("{gone}.wav")).exists());
    assert_eq!(audio_files(&dir), 1);
    assert!(store.item(keep).unwrap().is_some());
}

/// #98 AC-10, store half (R-16): a retry replaces the text and language and keeps the audio;
/// an unknown id is an error.
#[test]
fn replace_text_after_a_retry() {
    let dir = temp_dir("replace-text");
    let (mut store, _) = open_store(&dir, on(Retention::Days(30), 1024));
    let id = dictate(&mut store, "Old text.", 100);
    store.replace_text(id, "Texto nuevo.", Some("es")).unwrap();
    let item = store.item(id).unwrap().unwrap();
    assert_eq!(item.text.as_deref(), Some("Texto nuevo."));
    assert_eq!(item.language.as_deref(), Some("es"));
    assert_eq!(item.state, "done");
    assert!(item.has_audio);

    let failed = fail(&mut store, "dropped", 100);
    store
        .replace_text(failed, "Recovered.", Some("en"))
        .unwrap();
    let item = store.item(failed).unwrap().unwrap();
    assert_eq!(item.state, "done");
    assert_eq!(item.message, None);

    assert!(store.replace_text(9999, "Nothing.", None).is_err());
}

/// #98 R-16, Phase 1 R-28: with history off, a successful retry leaves nothing behind.
#[test]
fn replace_text_with_history_off_deletes_the_item() {
    let dir = temp_dir("replace-off");
    let (mut store, _) = open_store(&dir, off());
    let id = fail(&mut store, "dropped", 100);
    store.replace_text(id, "Private.", Some("en")).unwrap();
    assert_eq!(store.item(id).unwrap(), None);
    assert_eq!(audio_files(&dir), 0);
}

/// #98 AC-13, store half (R-19): with history off, History lists only the failed dictations
/// that still have their audio, newest first.
#[test]
fn failed_with_audio_lists_retryable_items() {
    let dir = temp_dir("failed-with-audio");
    let (mut store, clock) = open_store(&dir, off());
    let older = fail(&mut store, "dropped", 100);
    clock.fetch_add(1000, Ordering::SeqCst);
    dictate(&mut store, "Private.", 100);
    clock.fetch_add(1000, Ordering::SeqCst);
    let newer = fail(&mut store, "timed out", 100);
    clock.fetch_add(1000, Ordering::SeqCst);
    let no_audio = fail(&mut store, "dropped", 100);
    std::fs::remove_file(dir.join("audio").join(format!("{no_audio}.wav"))).unwrap();

    let ids: Vec<_> = store
        .failed_with_audio()
        .unwrap()
        .iter()
        .map(|i| i.id)
        .collect();
    assert_eq!(ids, [newer, older]);
}

/// Fills a store with `n` done items straight in the index, one second apart from `start_ms`,
/// with the text `text(i)`. Faster than dictating them, for the paging and timing tests.
fn fill(dir: &Path, n: usize, start_ms: i64, text: impl Fn(usize) -> String) {
    let db = rusqlite::Connection::open(dir.join("history.sqlite3")).unwrap();
    db.execute_batch("BEGIN").unwrap();
    for i in 0..n {
        db.execute(
            "INSERT INTO dictations (started_ms, finished_ms, app, language, text, state)
             VALUES (?1, ?1, 'Notes', 'es', ?2, 'done')",
            rusqlite::params![start_ms + i as i64 * 1000, text(i)],
        )
        .unwrap();
    }
    db.execute_batch("COMMIT").unwrap();
}

/// #98 AC-7 (R-10): History pages in fifties, newest first, and a dictation that arrives
/// between pages neither repeats nor skips an item.
#[test]
fn pages_do_not_repeat_or_skip_when_a_dictation_arrives() {
    let dir = temp_dir("paging");
    let (store, clock) = open_store(&dir, on(Retention::Days(30), 1024));
    drop(store);
    fill(&dir, 120, 99 * DAY_MS, |i| format!("Item {i}"));
    let (mut store, _) = open_store(&dir, on(Retention::Days(30), 1024));
    clock.store(100 * DAY_MS, Ordering::SeqCst);

    let first = store.page(None, 50).unwrap();
    assert_eq!(first.len(), 50);
    assert_eq!(first[0].text.as_deref(), Some("Item 119"));
    // Two items with the same start time are ordered by id.
    let last = first.last().unwrap();
    let new = dictate(&mut store, "Arrived between pages.", 100);

    let mut seen: Vec<i64> = first.iter().map(|i| i.id).collect();
    let mut cursor = Some((last.started_ms, last.id));
    while let Some(before) = cursor {
        let page = store.page(Some(before), 50).unwrap();
        cursor = page.last().map(|i| (i.started_ms, i.id));
        seen.extend(page.iter().map(|i| i.id));
    }
    assert_eq!(seen.len(), 120, "every old item once");
    let mut unique = seen.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), 120);
    assert!(
        !seen.contains(&new),
        "the new item is on top, not inside a later page"
    );
    assert_eq!(store.page(None, 1).unwrap()[0].id, new);
}

/// #98 AC-7: items that share a start time page by id, without loss.
#[test]
fn pages_break_ties_by_id() {
    let dir = temp_dir("paging-ties");
    let (store, _) = open_store(&dir, on(Retention::Days(30), 1024));
    drop(store);
    fill(&dir, 5, 99 * DAY_MS, |i| format!("Tie {i}"));
    let db = rusqlite::Connection::open(dir.join("history.sqlite3")).unwrap();
    db.execute("UPDATE dictations SET started_ms = ?1", [99 * DAY_MS])
        .unwrap();
    drop(db);
    let (store, _) = open_store(&dir, on(Retention::Days(30), 1024));
    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = store.page(cursor, 2).unwrap();
        let Some(last) = page.last() else { break };
        cursor = Some((last.started_ms, last.id));
        seen.extend(page.iter().map(|i| i.id));
    }
    assert_eq!(seen, [5, 4, 3, 2, 1]);
}

/// #98 AC-8 (R-12): every word typed must appear, ignoring case and accents.
#[test]
fn search_matches_every_word_ignoring_case_and_accents() {
    let dir = temp_dir("search");
    let (mut store, clock) = open_store(&dir, on(Retention::Days(30), 1024));
    let mut ids = Vec::new();
    for text in [
        "Canción para la Reunión de mañana.",
        "La reunión empieza a las diez.",
        "CANCION nueva para el coche.",
        "Song for the meeting.",
        "Después de la REUNION, la cancion.",
        "Ünïcödé àccents: Ñandú, façade, Øresund.",
    ] {
        clock.fetch_add(1000, Ordering::SeqCst);
        ids.push(dictate(&mut store, text, 100));
    }
    let found = |query: &str| -> Vec<i64> {
        store
            .search(query, None, 50)
            .unwrap()
            .iter()
            .map(|i| i.id)
            .collect()
    };
    assert_eq!(found("cancion reunion"), [ids[4], ids[0]]);
    assert_eq!(found("  Canción   REUNIÓN "), [ids[4], ids[0]]);
    assert_eq!(found("meeting"), [ids[3]]);
    assert_eq!(found("unicode nandu facade oresund"), [ids[5]]);
    assert!(found("cancion meeting").is_empty());
    // No words: every item, as a page.
    assert_eq!(found("   ").len(), 6);
    // The search pages like History.
    let first = store.search("la", None, 2).unwrap();
    assert_eq!(
        first.iter().map(|i| i.id).collect::<Vec<_>>(),
        [ids[4], ids[1]]
    );
    let last = first.last().unwrap();
    let rest: Vec<_> = store
        .search("la", Some((last.started_ms, last.id)), 50)
        .unwrap()
        .iter()
        .map(|i| i.id)
        .collect();
    assert_eq!(rest, [ids[0]]);
}

/// #98 AC-8: search over 10,000 items takes under 50 ms in a release build on the owner's
/// Mac. A debug build is far slower, so it gets a generous bound here; the 50 ms check is
/// `cargo test --release -p hyperme-platform --test history search_is_fast`.
#[test]
fn search_is_fast_on_ten_thousand_items() {
    let dir = temp_dir("search-speed");
    let (store, _) = open_store(&dir, on(Retention::Days(30), 1024));
    drop(store);
    let words = [
        "la", "reunión", "de", "mañana", "the", "meeting", "notes", "para", "proyecto", "email",
    ];
    fill(&dir, 10_000, 90 * DAY_MS, |i| {
        let mut text: Vec<&str> = (0..30)
            .map(|k| words[(i * 7 + k * 3) % words.len()])
            .collect();
        if i % 1000 == 0 {
            text.push("Canción");
        }
        text.join(" ")
    });
    let (store, _) = open_store(&dir, on(Retention::Days(30), 1024));

    let start = std::time::Instant::now();
    let found = store.search("cancion reunion", None, 50).unwrap();
    let elapsed = start.elapsed();
    println!("search over 10,000 items: {elapsed:?}");
    assert_eq!(found.len(), 10);
    let bound_ms = if cfg!(debug_assertions) { 500 } else { 50 };
    assert!(
        elapsed.as_millis() < bound_ms,
        "search took {elapsed:?}, bound {bound_ms} ms"
    );
}
