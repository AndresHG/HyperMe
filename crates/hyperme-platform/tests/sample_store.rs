//! Spec 267, slice 2: the sample folder. Tests cite the spec's acceptance criteria.

use std::path::{Path, PathBuf};

use hyperme_core::samples::{SampleFacts, Samples};
use hyperme_platform::sample_store::SampleStore;

fn temp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("samples")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn facts(id: i64, language: &str) -> SampleFacts {
    SampleFacts {
        history_id: id,
        dictated_ms: 1_000,
        language: Some(language.to_owned()),
        app: Some("TextEdit".to_owned()),
        cleanup_level: Some("light".to_owned()),
        speech: Some("speech/model".to_owned()),
        cleanup: None,
        transcript: Some("send the sun trix report".to_owned()),
        first_text: Some("Send the sun trix report.".to_owned()),
    }
}

fn json(dir: &Path, file: &str) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir.join(file)).unwrap()).unwrap()
}

/// AC-2 (store side, R-7, R-9, R-10): a save writes the WAV byte for byte and the texts; a
/// second save replaces the corrected text and keeps one sample; a reopened store knows it.
#[test]
fn a_sample_is_written_and_replaced() {
    let dir = temp_dir("write");
    let mut store = SampleStore::open(&dir).unwrap();
    assert!(!store.has(7));
    let wav = b"RIFF fake wav bytes".to_vec();
    store
        .save(&facts(7, "en"), &wav, "Send the Zentrix report.")
        .unwrap();
    assert_eq!(std::fs::read(dir.join("en-h7.wav")).unwrap(), wav);
    assert_eq!(json(&dir, "gold.json")["en-h7"], "Send the Zentrix report.");
    assert_eq!(json(&dir, "raw.json")["en-h7"], "send the sun trix report");
    assert_eq!(json(&dir, "apps.json")["en-h7"], "TextEdit");
    let record = &json(&dir, "samples.json")["en-h7"];
    assert_eq!(record["history_id"], 7);
    assert_eq!(record["speech"], "speech/model");
    assert_eq!(record["first_text"], "Send the sun trix report.");
    assert!(record.get("cleanup").is_none());

    store
        .save(&facts(7, "en"), &wav, "Send the Zentrix reports.")
        .unwrap();
    assert_eq!(json(&dir, "gold.json").as_object().unwrap().len(), 1);
    assert_eq!(
        json(&dir, "gold.json")["en-h7"],
        "Send the Zentrix reports."
    );
    let reopened = SampleStore::open(&dir).unwrap();
    assert!(reopened.has(7));
    assert_eq!(reopened.summary().unwrap().0, 1);
}

/// AC-5 (store side, R-12): Delete sample and Delete all samples remove the files, and only the
/// folder's own files; the agreement to keep samples stays (R-6).
#[test]
fn samples_are_deleted_one_or_all() {
    let dir = temp_dir("delete");
    let mut store = SampleStore::open(&dir).unwrap();
    assert!(!store.agreed());
    store.agree().unwrap();
    store.save(&facts(1, "es"), b"one", "Uno.").unwrap();
    store.save(&facts(2, "en"), b"two", "Two.").unwrap();
    std::fs::write(dir.join("notes.txt"), "mine").unwrap();
    let (count, bytes) = store.summary().unwrap();
    assert_eq!(count, 2);
    assert!(bytes > 6);

    store.delete(1).unwrap();
    assert!(!store.has(1) && store.has(2));
    assert!(!dir.join("es-h1.wav").exists());
    assert!(json(&dir, "gold.json").get("es-h1").is_none());

    store.delete_all().unwrap();
    assert_eq!(store.summary().unwrap(), (0, 0));
    assert!(
        dir.join("notes.txt").exists(),
        "a file the user put there stays"
    );
    assert!(store.agreed(), "the agreement survives Delete all (R-6)");
    assert!(!SampleStore::open(&dir).unwrap().has(2));
}

/// AC-8 (R-9): a temporary file left by an interrupted write is ignored: the JSON files stay
/// readable and the next save goes through.
#[test]
fn a_left_over_temporary_file_breaks_nothing() {
    let dir = temp_dir("interrupted");
    let mut store = SampleStore::open(&dir).unwrap();
    store.save(&facts(1, "en"), b"one", "One.").unwrap();
    std::fs::write(dir.join("gold.json.tmp"), "{ half a fi").unwrap();
    let mut store = SampleStore::open(&dir).unwrap();
    assert!(store.has(1));
    store.save(&facts(2, "en"), b"two", "Two.").unwrap();
    assert_eq!(json(&dir, "gold.json").as_object().unwrap().len(), 2);
    assert!(!dir.join("gold.json.tmp").exists());
}
