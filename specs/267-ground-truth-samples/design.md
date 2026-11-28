# Samples: design

For [spec.md](spec.md). Slices in [tasks.md](tasks.md).

## 1. The parts

```
hyperme-core/src/traits.rs            History::done gains the speech transcript (R-1)
hyperme-core/src/dictation.rs         passes transcript.text (before the dictionary) to done
hyperme-core/src/samples.rs           Sample, SampleFacts, the Samples trait (no I/O)
hyperme-platform/src/history.rs       two columns, first_text and transcript; a read for both
hyperme-platform/src/sample_store.rs  Samples on the sample folder (WAV + JSON, temp and rename)
app/src-tauri/src/samples.rs          correct() over the Samples trait; home_correct,
                                      home_delete_sample, home_samples_of, samples_view,
                                      samples_delete_all, samples_reveal
app/src-tauri/src/home.rs             the Correct action in actions() (slice 4, with the page)
app/web/src/home/HistoryRow.svelte    the editor, the sample mark (slice 4, after spec 257)
app/web/src/settings/Privacy.svelte   the samples row (slice 4)
crates/hyperme-quality                --set samples, the speech-transcript column
```

No new crate. The store uses `std::fs` and `serde_json`, both already in the platform crate.

## 2. History (R-1, R-2)

Two nullable columns, added with `ALTER TABLE ... ADD COLUMN` when `SqliteHistory::open` finds
them missing (as `task_store.rs` does for `day_log`; old rows stay `NULL`):

| Column | Written | Changed by |
| --- | --- | --- |
| `transcript` | `done`, from the speech provider's text before the dictionary | never |
| `first_text` | `done`, the same text as `text` | never |
| `text` | `done` | `replace_text` (Retry, Fix a word, Correct) |

`History::done(id, text, transcript, language)`: one more argument. `SqliteHistory` also gains
`benchmark_texts(id)` and `audio_wav(id)` (the file as it is on disk) for a sample. The engine has the speech
text at `transcript.text` before `settings.dictionary.substitute` (dictation.rs); it keeps it in
`Text` next to the substituted `transcript`. The speech transcript is taken before the dictionary
because the harness applies a dictionary itself (`--dictionary`), so a benchmark sees what the
speech model alone heard. A dictation pasted through the short path or the raw fallback stores the
same transcript.

With history off, `done` deletes the item as today, so nothing new is kept (R-2).

## 3. The sample folder (R-9, R-10, R-14)

The layout of a harness set, so `hyperme-quality` reads it with its own loader:

```
samples/
  es-h412.wav        the history's WAV, copied byte for byte (24 kHz, mono, 16-bit)
  gold.json          {"es-h412": "the corrected text", ...}           harness: gold texts
  raw.json           {"es-h412": "the speech transcript", ...}        harness: --reuse-raw
  apps.json          {"es-h412": "Slack", ...}                         harness: tone per app
  samples.json       {"es-h412": {facts below}, ...}                   the app's own record
```

`samples.json`, one object per sample:

```json
{ "history_id": 412, "saved_ms": 1764331200000, "dictated_ms": 1764330000000,
  "language": "es", "cleanup_level": "light",
  "speech": "speechmatics/enhanced", "cleanup": "groq/llama-3.3-70b",
  "first_text": "what HyperMe pasted the first time" }
```

The provider and model names come from the item's trace (`TraceStore::trace_for`), which already
keeps them as names (spec 182 R-3); when the trace is gone they are left out. `apps.json` holds the
app name only because the harness's tone per app needs it; it never leaves the folder.

Each JSON file is written whole to `<name>.tmp` and renamed over the old one; the WAV is written
first, then the JSON files, so a crash leaves at worst a WAV with no gold text, which the harness
scores as having none and the next save of that sample overwrites. One writer only: the store is
a single owner behind the app's runtime, like the history.

### The trait

```rust
pub struct SampleFacts { history_id, dictated_ms, language, app, cleanup_level,
                         speech, cleanup, transcript, first_text }
pub trait Samples {
    fn save(&mut self, facts: &SampleFacts, audio_wav: &[u8], corrected: &str) -> Result<(), Error>;
    fn delete(&mut self, history_id: DictationId) -> Result<(), Error>;
    fn delete_all(&mut self) -> Result<(), Error>;
    fn has(&self, history_id: DictationId) -> bool;
    fn summary(&self) -> Result<(usize, u64), Error>;   // count, bytes
}
```

`has` feeds the sample mark on History rows: the store keeps the set of ids in memory, read from
`samples.json` at open (a few hundred ids at most).

### Agreement (R-6)

An empty `.agreed` file in the sample folder, behind `Samples::agreed` and `Samples::agree`: it
records a choice, not a setting. Delete all samples keeps it; deleting the folder in Finder asks
again, which is the safe side. `home_correct` returns `NeedsAgreement` until it is set, as `fix_word` returns
`NeedsConfirm` today, and the page shows the sheet and calls again with `agreed: true`.

## 4. Correct, end to end

```
page: ⌘E → editor → ⌘↵
  └─ home_correct(id, text, agreed)                         app/src-tauri/src/samples.rs
       ├─ not agreed and never agreed → NeedsAgreement
       ├─ history.item(id): done, has audio, else error
       ├─ facts: history (transcript, first_text, app, language) + trace_for(id) names
       ├─ samples.save(facts, wav, text)                    off the main thread
       └─ history.replace_text(id, text) if it differs
     → Saved { text }
```

`correct()` is a plain function over `&mut dyn Samples`, tested with fakes
the way `fix_word` is (AC-2, AC-4).

## 5. The harness (R-14 to R-16)

- `--set samples` is a name, not a path: it resolves to the app's data folder (the `data_dir()` that
  `import` uses to find the history). The folder is outside any repository, so `check_location`
  passes.
- R-15: each language row also shows **Speech edits per 100**: the replay's own speech
  transcript (the one the harness already records per sample) against gold, with the same
  `score::against_gold` the pasted text uses. It costs nothing and helps every set, so it is on
  for all of them, not only samples.
- `import` refuses the sample folder (R-16).
- `import` stays as it is, for people who keep a set elsewhere.

## 6. Failure modes

| Failure | Behaviour |
| --- | --- |
| The history deleted the audio before Correct | Correct is not offered (`has_audio` is false), as Retry |
| Disk full while saving | The error is shown in the row; the row text is not changed; temp files are removed |
| The trace is gone | The sample is saved without provider names |
| An old row has no `transcript` (before this change) | Saved with an empty speech transcript; `raw.json` leaves it out, so `--reuse-raw` skips it |
| The user deletes the folder in Finder | The store recreates it at the next save; Settings shows 0 |

## 7. Tests

Lean, one per behaviour: AC-1 in the history tests and the engine tests (one more assertion each);
AC-2 and AC-3 as fake-driven `correct()` tests in `samples.rs` (AC-4 holds because `correct()` takes no dictionary), AC-5 in the store tests; AC-3 at the command; AC-6 and
AC-8 as one contract test in `crates/hyperme-quality/tests/` that writes a folder with the real
`SampleStore` in a temp dir and loads it with `set::load`; AC-7 holds by construction: the bundle is built from the trace store and
the warnings file only (spec 200), whose text-free rule spec 182's privacy test (AC-3) covers, and
nothing but `samples.rs` opens the sample folder; AC-10 in the page tests of spec 257.
