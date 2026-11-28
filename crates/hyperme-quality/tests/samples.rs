//! Spec 267, AC-6 (store side): the app's sample folder is a harness set. A folder written by
//! the real `SampleStore` loads with the harness's own loader, with the corrections as gold
//! texts and the speech transcripts as a `--reuse-raw` file.

use hyperme_core::samples::{SampleFacts, Samples};
use hyperme_platform::sample_store::SampleStore;
use hyperme_quality::set::{load, read_raw, wav_bytes};

#[test]
fn a_sample_folder_loads_as_a_set() {
    let dir = std::env::temp_dir().join(format!("hyperme-samples-set-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let pcm: Vec<i16> = (0..2400).map(|i| (i % 300) as i16).collect();
    let mut store = SampleStore::open(&dir).unwrap();
    for (id, language, corrected, transcript) in [
        (
            4,
            "es",
            "Envía el informe de Zentrix.",
            Some("envía el informe de sun trix"),
        ),
        (9, "en", "Call Ana at five.", None),
    ] {
        let facts = SampleFacts {
            history_id: id,
            language: Some(language.to_owned()),
            transcript: transcript.map(str::to_owned),
            ..SampleFacts::default()
        };
        store.save(&facts, &wav_bytes(&pcm), corrected).unwrap();
    }

    let set = load(&dir, &[], None).unwrap();
    let names: Vec<&str> = set.samples.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["en-h9", "es-h4"]);
    assert!(set.samples.iter().all(|s| s.pcm == pcm));
    assert_eq!(set.gold["es-h4"], "Envía el informe de Zentrix.");
    assert_eq!(set.gold["en-h9"], "Call Ana at five.");
    assert!(!set.text_only);

    let raw = read_raw(&dir.join("raw.json")).unwrap();
    assert_eq!(
        raw.len(),
        1,
        "a sample with no transcript is left out of raw.json"
    );
    assert_eq!(raw["es-h4"], "envía el informe de sun trix");
}
