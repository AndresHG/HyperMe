//! AC-4 (the set folder) and AC-5 (privacy), issue 99.

use std::path::{Path, PathBuf};

use hyperme_quality::set::{
    HistoryItem, Lang, check_location_with, import, load, read_history, read_raw, read_wav,
    wav_bytes, write_raw,
};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hyperme-quality-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// AC-4: 24 kHz mono 16-bit, extra chunks before `data` skipped (afconvert writes `FLLR`).
#[test]
fn wav_files_read_back_and_other_formats_are_refused() {
    let pcm = vec![0i16, 1, -1, i16::MAX, i16::MIN];
    assert_eq!(read_wav(&wav_bytes(&pcm)).unwrap(), pcm);

    let mut padded = wav_bytes(&pcm);
    let data_at = 36;
    let filler: Vec<u8> = [b"FLLR".as_slice(), &4u32.to_le_bytes(), &[0, 0, 0, 0]].concat();
    padded.splice(data_at..data_at, filler);
    assert_eq!(read_wav(&padded).unwrap(), pcm);

    let mut stereo = wav_bytes(&pcm);
    stereo[22] = 2;
    assert!(read_wav(&stereo).unwrap_err().contains("mono"));
    let mut rate = wav_bytes(&pcm);
    rate[24..28].copy_from_slice(&16_000u32.to_le_bytes());
    assert!(read_wav(&rate).unwrap_err().contains("24000"));
    assert!(read_wav(b"not a wav").is_err());
}

/// AC-4: a flowbench samples folder loads as it is, sorted, with gold and names.
#[test]
fn a_set_loads_samples_gold_and_names() {
    let dir = temp_dir("load");
    std::fs::write(dir.join("es-01.wav"), wav_bytes(&[1, 2])).unwrap();
    std::fs::write(dir.join("en-01.wav"), wav_bytes(&[3])).unwrap();
    std::fs::write(dir.join("notes.txt"), "ignored").unwrap();
    std::fs::write(
        dir.join("gold.json"),
        r#"{"es-01": "Hola.", "en-01": "Hi."}"#,
    )
    .unwrap();
    std::fs::write(dir.join("names.txt"), "Zentrix: zen trix\n").unwrap();

    let set = load(&dir, &[], None).unwrap();
    let names: Vec<&str> = set.samples.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["en-01", "es-01"]);
    assert_eq!(set.samples[1].pcm, [1, 2]);
    assert_eq!(set.gold["es-01"], "Hola.");
    assert_eq!(set.names, ["Zentrix"]);

    let only = load(&dir, &["es-01".to_owned()], None).unwrap();
    assert_eq!(only.samples.len(), 1);

    // A names file given explicitly wins over the set's own.
    let other = dir.join("other-names.txt");
    std::fs::write(&other, "Quorbit\n").unwrap();
    assert_eq!(load(&dir, &[], Some(&other)).unwrap().names, ["Quorbit"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// AC-4: the language comes from the sample's name.
#[test]
fn the_language_comes_from_the_name() {
    assert_eq!(Lang::of_sample("es-01"), Some(Lang::Es));
    assert_eq!(Lang::of_sample("en-h12"), Some(Lang::En));
    assert_eq!(Lang::of_sample("xx-h3"), None);
}

/// AC-5: outside a repository is fine; inside one only when git ignores the folder.
#[test]
fn a_set_inside_the_repository_must_be_ignored() {
    let outside = temp_dir("outside");
    assert!(check_location_with(&outside, |_, _| false).is_ok());

    let repo = temp_dir("repo");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    let inside = repo.join("samples");
    std::fs::create_dir_all(&inside).unwrap();
    let seen = std::cell::RefCell::new(None::<(PathBuf, PathBuf)>);
    let refused = check_location_with(&inside, |root: &Path, dir: &Path| {
        *seen.borrow_mut() = Some((root.to_owned(), dir.to_owned()));
        false
    });
    assert!(refused.unwrap_err().contains("git-ignored"));
    let (root, _) = seen.borrow().clone().unwrap();
    assert_eq!(root, repo.canonicalize().unwrap());
    assert!(check_location_with(&inside, |_, _| true).is_ok());
    std::fs::remove_dir_all(&outside).unwrap();
    std::fs::remove_dir_all(&repo).unwrap();
}

/// AC-4: done dictations come from the history's index read-only, newest first, failed ones
/// and ones with no text left out.
#[test]
fn done_dictations_are_read_from_the_history() {
    let dir = temp_dir("history");
    std::fs::create_dir_all(dir.join("audio")).unwrap();
    let db = rusqlite::Connection::open(dir.join("history.sqlite3")).unwrap();
    db.execute_batch(
        "CREATE TABLE dictations (id INTEGER PRIMARY KEY, started_ms INTEGER NOT NULL,
            finished_ms INTEGER, app TEXT, language TEXT, text TEXT, state TEXT NOT NULL,
            message TEXT, audio_bytes INTEGER NOT NULL DEFAULT 0);
         INSERT INTO dictations (id, started_ms, language, text, state) VALUES
            (1, 1000, 'es', 'Hola equipo.', 'done'),
            (2, 2000, NULL, NULL, 'failed'),
            (3, 3000, 'en', 'Hi team.', 'done'),
            (4, 500, 'en', 'Old one.', 'done');",
    )
    .unwrap();
    drop(db);
    let items = read_history(&dir, Some(900)).unwrap();
    let ids: Vec<i64> = items.iter().map(|i| i.id).collect();
    assert_eq!(ids, [3, 1]);
    assert_eq!(items[1].language.as_deref(), Some("es"));
    assert_eq!(items[1].text, "Hola equipo.");
    assert_eq!(items[1].audio, dir.join("audio").join("1.wav"));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// AC-4: import copies the audio as `<lang>-h<id>.wav` and adds the pasted text to the gold
/// draft, never to `gold.json`; a sample already in the set is skipped.
#[test]
fn import_copies_audio_and_drafts_gold() {
    let history = temp_dir("import-history");
    let set = temp_dir("import-set");
    std::fs::write(history.join("1.wav"), wav_bytes(&[5, 6])).unwrap();
    std::fs::write(history.join("2.wav"), wav_bytes(&[7])).unwrap();
    std::fs::write(set.join("gold-draft.json"), r#"{"es-01": "kept"}"#).unwrap();
    let items = vec![
        HistoryItem {
            id: 1,
            language: Some("es".to_owned()),
            text: "Hola.".to_owned(),
            audio: history.join("1.wav"),
        },
        HistoryItem {
            id: 2,
            language: None,
            text: "Mystery.".to_owned(),
            audio: history.join("2.wav"),
        },
    ];
    let first = import(&set, &items).unwrap();
    assert_eq!((first.added, first.skipped), (2, 0));
    assert_eq!(
        read_wav(&std::fs::read(set.join("es-h1.wav")).unwrap()).unwrap(),
        [5, 6]
    );
    assert!(set.join("xx-h2.wav").exists());
    let draft: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(set.join("gold-draft.json")).unwrap())
            .unwrap();
    assert_eq!(draft["es-01"], "kept");
    assert_eq!(draft["es-h1"], "Hola.");
    assert!(!set.join("gold.json").exists());

    let again = import(&set, &items).unwrap();
    assert_eq!((again.added, again.skipped), (0, 2));
    std::fs::remove_dir_all(&history).unwrap();
    std::fs::remove_dir_all(&set).unwrap();
}

/// Slice A, AC-3: a live run's raw transcripts are saved as `{sample: raw}` and read back for
/// `--reuse-raw`; a missing file is an error, not an empty set.
#[test]
fn raw_transcripts_are_saved_and_read_back() {
    let dir = temp_dir("raw");
    let path = dir.join("raw-2026-11-27.json");
    let raw: std::collections::BTreeMap<String, String> = [("es-01", "hola"), ("en-01", "hi")]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    write_raw(&path, &raw).unwrap();
    assert_eq!(read_raw(&path).unwrap(), raw);
    let error = read_raw(&dir.join("none.json")).unwrap_err();
    assert!(error.contains("none.json"), "{error}");
}
