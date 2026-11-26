//! A quality set (AC-4): a folder of WAV files with the owner's corrected texts. It holds
//! private speech, so it lives outside the repository or in a git-ignored folder (AC-5).
//!
//! ```text
//! <set>/es-01.wav       24 kHz, mono, 16-bit; the name's prefix (es-, en-) is the language
//! <set>/gold.json       {"es-01": "the corrected text", ...}
//! <set>/names.txt       optional: internal names, in the dictionary format
//! <set>/gold-draft.json written by the tool for the owner to correct into gold.json
//! ```
//!
//! `spikes/flowbench/samples/` has this layout already.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use rusqlite::{Connection, OpenFlags, params};
use serde_json::{Map, Value};

use crate::score::parse_names;

/// The rate the app records and the history stores at.
pub const SAMPLE_RATE: u32 = 24_000;

/// The language of a sample, from its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Es,
    En,
}

impl Lang {
    /// `es-01` and `es-h12` are Spanish, `en-...` English, anything else unknown.
    pub fn of_sample(name: &str) -> Option<Lang> {
        match name.split('-').next() {
            Some("es") => Some(Lang::Es),
            Some("en") => Some(Lang::En),
            _ => None,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::Es => "es",
            Lang::En => "en",
        }
    }
}

/// One recorded dictation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sample {
    /// The file name without `.wav`.
    pub name: String,
    /// Mono PCM16 at [`SAMPLE_RATE`].
    pub pcm: Vec<i16>,
}

/// A loaded set.
#[derive(Debug, Clone)]
pub struct Set {
    pub dir: PathBuf,
    pub samples: Vec<Sample>,
    /// The owner's corrected texts, by sample. May be empty.
    pub gold: BTreeMap<String, String>,
    /// Internal names to score. May be empty.
    pub names: Vec<String>,
}

/// A WAV file of mono PCM16 at 24 kHz, like the history's.
pub fn wav_bytes(pcm: &[i16]) -> Vec<u8> {
    let data = (pcm.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    out.extend(pcm.iter().flat_map(|s| s.to_le_bytes()));
    out
}

/// Reads mono PCM16 at 24 kHz, walking the chunks so extra ones (`FLLR`, `LIST`) are skipped.
/// A `data` size larger than the file, as a history file cut by a crash has, reads what is
/// there.
pub fn read_wav(bytes: &[u8]) -> Result<Vec<i16>, String> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a WAV file".to_owned());
    }
    let u16_at = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
    let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap_or_default());
    let mut at = 12;
    let mut format_ok = false;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32_at(at + 4) as usize;
        let body = at + 8;
        if id == b"fmt " {
            if body + 16 > bytes.len() {
                return Err("the fmt chunk is cut short".to_owned());
            }
            let (format, channels, rate, bits) = (
                u16_at(body),
                u16_at(body + 2),
                u32_at(body + 4),
                u16_at(body + 14),
            );
            if format != 1 || bits != 16 {
                return Err("expected 16-bit PCM".to_owned());
            }
            if channels != 1 {
                return Err(format!("expected mono, found {channels} channels"));
            }
            if rate != SAMPLE_RATE {
                return Err(format!("expected {SAMPLE_RATE} Hz, found {rate} Hz"));
            }
            format_ok = true;
        } else if id == b"data" {
            if !format_ok {
                return Err("no fmt chunk before the data".to_owned());
            }
            let end = body.saturating_add(size).min(bytes.len());
            return Ok(bytes[body..end]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&pair| i16::from_le_bytes(pair))
                .collect());
        }
        // Chunks are padded to an even size.
        at = body.saturating_add(size + size % 2);
    }
    Err("no data chunk".to_owned())
}

/// A JSON object of sample name to text.
fn read_texts(path: &Path) -> Result<BTreeMap<String, String>, String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(BTreeMap::new());
    };
    let map: Map<String, Value> = serde_json::from_str(&text).map_err(|e| {
        format!(
            "{}: expected a JSON object of name to text: {e}",
            path.display()
        )
    })?;
    Ok(map
        .into_iter()
        .filter_map(|(name, text)| text.as_str().map(|t| (name, t.to_owned())))
        .collect())
}

fn write_texts(path: &Path, texts: &BTreeMap<String, String>) -> Result<(), String> {
    let map: Map<String, Value> = texts
        .iter()
        .map(|(name, text)| (name.clone(), Value::String(text.clone())))
        .collect();
    let json = serde_json::to_string_pretty(&map).map_err(|e| e.to_string())? + "\n";
    std::fs::write(path, json).map_err(|e| format!("writing {}: {e}", path.display()))
}

/// Loads the set in `dir`: every `.wav` file, sorted by name, or only those in `only`.
/// `names` overrides the set's own `names.txt`.
pub fn load(dir: &Path, only: &[String], names: Option<&Path>) -> Result<Set, String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("reading the set {}: {e}", dir.display()))?;
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "wav"))
        .collect();
    paths.sort();
    let mut samples = Vec::new();
    for path in paths {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !only.is_empty() && !only.contains(&name) {
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let pcm = read_wav(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        samples.push(Sample { name, pcm });
    }
    if samples.is_empty() {
        return Err(format!("no samples (.wav files) in {}", dir.display()));
    }
    let names_path = names.map_or_else(|| dir.join("names.txt"), Path::to_owned);
    let names = match std::fs::read_to_string(&names_path) {
        Ok(text) => parse_names(&text),
        Err(_) if names.is_none() => Vec::new(),
        Err(e) => return Err(format!("{}: {e}", names_path.display())),
    };
    Ok(Set {
        dir: dir.to_owned(),
        samples,
        gold: read_texts(&dir.join("gold.json"))?,
        names,
    })
}

/// The root of the git work tree holding `dir`, if any.
fn repo_root(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|a| a.join(".git").exists())
        .map(Path::to_owned)
}

/// AC-5: refuses a set folder inside a git work tree unless git ignores it. `ignored` is asked
/// with the work tree's root and the folder, both canonical.
pub fn check_location_with(
    dir: &Path,
    ignored: impl Fn(&Path, &Path) -> bool,
) -> Result<(), String> {
    let dir = dir
        .canonicalize()
        .map_err(|e| format!("the set {}: {e}", dir.display()))?;
    match repo_root(&dir) {
        None => Ok(()),
        Some(root) if ignored(&root, &dir) => Ok(()),
        Some(root) => Err(format!(
            "the set {} is inside the git repository {} and is not git-ignored. Its audio and \
             texts are private: move it out of the repository, or into a git-ignored folder \
             such as spikes/flowbench/samples/",
            dir.display(),
            root.display()
        )),
    }
}

/// AC-5 with git itself as the judge: `git check-ignore` succeeds for an ignored path.
pub fn check_location(dir: &Path) -> Result<(), String> {
    check_location_with(dir, |root, dir| {
        Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["check-ignore", "-q"])
            .arg(dir)
            .status()
            .is_ok_and(|status| status.success())
    })
}

/// One done dictation in the app's history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryItem {
    pub id: i64,
    pub language: Option<String>,
    /// The text that was pasted.
    pub text: String,
    pub audio: PathBuf,
}

/// The done dictations in the history at `dir` (the app's `history/` folder), newest first,
/// started at or after `since_ms` when given. Opens the index read-only: the app's own
/// retention cleanup runs when the app opens it, never from here.
pub fn read_history(dir: &Path, since_ms: Option<i64>) -> Result<Vec<HistoryItem>, String> {
    let path = dir.join("history.sqlite3");
    let db = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("opening {}: {e}", path.display()))?;
    let mut query = db
        .prepare(
            "SELECT id, language, text FROM dictations
             WHERE state = 'done' AND text IS NOT NULL AND started_ms >= ?1
             ORDER BY started_ms DESC, id DESC",
        )
        .map_err(|e| format!("reading {}: {e}", path.display()))?;
    let rows = query
        .query_map(params![since_ms.unwrap_or(i64::MIN)], |row| {
            let id: i64 = row.get(0)?;
            Ok(HistoryItem {
                id,
                language: row.get(1)?,
                text: row.get(2)?,
                audio: dir.join("audio").join(format!("{id}.wav")),
            })
        })
        .map_err(|e| format!("reading {}: {e}", path.display()))?;
    rows.collect::<Result<_, _>>()
        .map_err(|e| format!("reading {}: {e}", path.display()))
}

/// Adds texts to the set's `gold-draft.json` for samples it does not have yet, so the owner's
/// edits there are never overwritten. Returns how many were added.
pub fn add_to_draft(set: &Path, texts: &BTreeMap<String, String>) -> Result<usize, String> {
    let path = set.join("gold-draft.json");
    let mut draft = read_texts(&path)?;
    let before = draft.len();
    for (name, text) in texts {
        draft.entry(name.clone()).or_insert_with(|| text.clone());
    }
    let added = draft.len() - before;
    if added > 0 {
        write_texts(&path, &draft)?;
    }
    Ok(added)
}

/// What an import did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Imported {
    pub added: usize,
    /// Already in the set.
    pub skipped: usize,
}

/// The sample name for a history item: `es-h12`, `en-h12`, or `xx-h12` when the language is
/// not known.
fn history_name(item: &HistoryItem) -> String {
    let lang = match item.language.as_deref().map(str::to_lowercase) {
        Some(code) if code.starts_with("es") => "es",
        Some(code) if code.starts_with("en") => "en",
        _ => "xx",
    };
    format!("{lang}-h{}", item.id)
}

/// AC-4: copies each item's audio into the set and adds its pasted text to `gold-draft.json`,
/// for the owner to correct into `gold.json`. The pasted text is the app's own output, so it
/// never goes into `gold.json` directly.
pub fn import(set: &Path, items: &[HistoryItem]) -> Result<Imported, String> {
    std::fs::create_dir_all(set).map_err(|e| format!("creating {}: {e}", set.display()))?;
    let mut draft = BTreeMap::new();
    let mut done = Imported {
        added: 0,
        skipped: 0,
    };
    for item in items {
        let name = history_name(item);
        let target = set.join(format!("{name}.wav"));
        if target.exists() {
            done.skipped += 1;
            continue;
        }
        let bytes =
            std::fs::read(&item.audio).map_err(|e| format!("{}: {e}", item.audio.display()))?;
        // Check it reads, so a set never holds a file the replay would refuse.
        read_wav(&bytes).map_err(|e| format!("{}: {e}", item.audio.display()))?;
        std::fs::write(&target, bytes).map_err(|e| format!("{}: {e}", target.display()))?;
        draft.insert(name, item.text.clone());
        done.added += 1;
    }
    add_to_draft(set, &draft)?;
    Ok(done)
}
