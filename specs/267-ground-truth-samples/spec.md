# Samples: correct a dictation and keep it as ground truth

**Status:** approved, 28 November 2026 (owner: "spec 267 approved, V0.1 with the first-save sheet").
**Issue:** [#267](https://github.com/AndresHG/hyperme/issues/267). **Size:** L (changes what is
kept on disk, a core trait, a new platform store, the harness and the History page).
**Version:** 0.1 (GT-1): it turns daily use into the gate's measure.
**Traces to:** the owner's ask of 28 November 2026; [RELEASES](../../docs/RELEASES.md) 0.1 gate
(edits per 100 characters at or below Wispr Flow's 11.6); spec [98](../98-home-window/spec.md)
R-13, R-16, R-17 (History's actions); spec [182](../182-trace-store/spec.md) (traces, provider
and model per dictation); [hyperme-quality](../../crates/hyperme-quality/README.md) (sets, `import`,
`run`, `--reuse-raw`); spec [257](../257-ui-framework/spec.md) (the Svelte History page this
builds on).

## Problem

The 0.1 gate is a number, edits per 100 characters, and it is only as good as the corrected texts
it is scored against. Today they come from one path: `hyperme-quality import` copies history
audio into a set and writes the pasted text to `gold-draft.json`, and the owner edits JSON by
hand. In practice:

- The moment a dictation comes out wrong is in the app, not in a terminal days later. By then the
  owner no longer remembers what they said, and the history may have deleted the audio
  (30 days, or the size limit).
- **Fix a word** is the only way to say "this was wrong" in the app, and it does something else:
  it teaches the dictionary one word and overwrites the row's text. A sentence that came out wrong
  in three places, or with wrong punctuation, has nowhere to go.
- Both Fix a word and Retry overwrite the history's text, and the history never keeps the speech
  transcript at all. So the app cannot tell what the speech model heard from what cleanup made of
  it, which is what a benchmark of a new speech model or prompt needs.

## Goal

From History, the user marks a dictation as wrong and types the whole text as it should have been.
HyperMe keeps, on the Mac, the audio, the speech transcript, the text it pasted and the correction,
with the providers that produced it, as a **sample**. Samples sit in a folder that
`hyperme-quality run --set` reads as it is, so any future speech model, prompt or approach is
scored against everything the user has corrected, and `--reuse-raw` replays cleanup alone. Nothing
leaves the Mac.

## Words used here

- **Sample:** one corrected dictation: its audio (WAV), the speech transcript, the pasted text,
  the corrected text and a few facts about how it was made.
- **Sample folder:** `~/Library/Application Support/HyperMe/samples/`, in the layout of a
  harness set (design.md, section 3).
- **Correct:** the new History action. **Fix a word** stays as it is.

## Requirements

### Keeping what a benchmark needs

- R-1. When a dictation finishes, the app shall store in the history, next to its text, the speech
  transcript as the speech provider returned it (before the dictionary) and the text first pasted
  or copied. Retry and Fix a word change the shown text only; these two stay as they were.
- R-2. The speech transcript and the first text shall follow the history's rules: deleted with the
  item, by retention and by the size limit, and never kept while history is off.

### Correcting

- R-3. For a selected done row with audio, History shall offer **Correct** (⌘E) next to Fix a word.
  It is offered whether or not a dictionary exists.
- R-4. When the user chooses Correct, History shall open the row's text in an editor in place, the
  whole text selected, with **Save as sample** (⌘↵) and **Cancel** (Esc).
- R-5. When the user saves a text that differs from the row's text (apart from spacing), the app
  shall write a sample and show the corrected text in the row with a sample mark. Saving the same
  text keeps it and still writes a sample (a dictation that was right is a useful sample too).
- R-6. The first time the user saves a sample, History shall explain under the row, before
  anything is written (as it asks before Delete): what a sample keeps (audio and texts), that it stays on this Mac until they
  delete it, even after the history deletes the dictation, and that only their own providers hear
  it, and only when they run a benchmark. **Keep samples** (↵) writes it and does not ask again;
  **Not now** (Esc) writes nothing. (GT-2; the owner chose "ask on first save". The ask sits in the
  row like the other confirmations there, not in a separate sheet.)
- R-7. When the user corrects a row that already has a sample, the app shall replace that sample's
  corrected text and keep one sample per dictation.
- R-8. Correct shall never change the personal dictionary. (Turning a correction into dictionary
  suggestions is a later step, non-goals.)

### The sample folder

- R-9. The app shall write each sample to the sample folder as `<lang>-h<id>.wav` (the history
  item's id, `xx` when the language is unknown) and add its texts to the folder's JSON files, each
  written to a temporary file and renamed, so a harness run never reads half a file.
- R-10. A sample shall carry: the history id, the date, the language, the app it went into, the
  cleanup level, the speech and cleanup provider and model names (from its trace, spec 182), the
  speech transcript, the first pasted text and the corrected text.
- R-11. Samples shall not follow the history's retention or size limit. Deleting the history item
  shall keep its sample; the delete confirmation says so in one line.
- R-12. Settings → Privacy, under the history settings, shall show the number of samples and their size, with **Show in
  Finder** and **Delete all samples** (asks once). A sample row in History has **Delete sample**.
- R-13. The Report a problem bundle (spec 200) and the trace store shall never read the sample
  folder. Samples go nowhere unless the user runs the harness on them.

### The harness

- R-14. `hyperme-quality run --set samples` shall resolve to the app's sample folder and replay it
  like any other set; `--reuse-raw <folder>/raw.json` shall replay cleanup alone over the samples'
  speech transcripts.
- R-15. A run on samples shall also report, per language, edits per 100 characters of the **speech
  transcript** against the correction, so a speech model can be compared without cleanup.
- R-16. The harness shall read the sample folder only; it never writes to it, except the private
  outputs it already writes into a set folder (texts, raw transcripts of a live run).

### Budgets

- R-17. Saving a sample shall take under 200 ms for a one-minute dictation on the owner's Mac and
  run off the main thread. Idle memory and the dictation path do not change (R-1 adds two text
  columns to a write the history already does).

## Acceptance criteria

*(fake)*: runs in CI with fakes. *(live)*: on the owner's Mac.

- AC-1 *(fake)*: A dictation through fakes stores its speech transcript and first text; a retry
  and a Fix a word change the text and leave both unchanged; with history off, none is kept.
  (R-1, R-2)
- AC-2 *(fake)*: Correct on a done row with audio writes `<lang>-h<id>.wav` (byte-identical to the
  history's audio) and the texts of R-10 to the sample folder; the row shows the corrected text.
  A second Correct replaces the text and leaves one sample. Correct with no audio is not offered.
  (R-3 to R-5, R-7, R-9, R-10)
- AC-3 *(fake)*: Before the first sample, nothing is written until Keep samples; Not now writes
  nothing and asks again next time. (R-6)
- AC-4 *(fake)*: Correct sends nothing to the dictionary. (R-8)
- AC-5 *(fake)*: The history's retention, size limit and Delete remove history items and leave
  their samples; Delete sample and Delete all samples remove them. (R-11, R-12)
- AC-6 *(fake)*: A folder written by the sample store loads with `hyperme_quality::set::load`,
  with the corrections as gold texts and the speech transcripts as a `--reuse-raw` file; a
  replay through fakes scores it, with the speech-transcript column of R-15. (R-9, R-14, R-15)
- AC-7 *(fake)*: Privacy: the trace store's database, the warnings file and a Report a problem
  bundle built after saving samples contain none of a sample's texts. (R-13)
- AC-8 *(fake)*: An interrupted write (the temporary file left behind) leaves the folder's JSON
  files readable and the sample either fully there or absent. (R-9)
- AC-9 *(live)*: Save 10 samples from the owner's history; `hyperme-quality run --set samples`
  scores them; saving takes under 200 ms each. Numbers only to `docs/results/`. (R-14, R-17)
- AC-10 *(fake)*: Page tests (spec 257's harness): Correct opens the editor with the text selected,
  ⌘↵ saves, Esc cancels, and the first-save ask appears once. (R-4, R-6)

## Non-goals

- Turning a correction into dictionary suggestions (a word-level diff fed to spec 93's
  suggestions). A good next step once samples exist; its own issue.
- Correcting text the user edited after pasting it in another app (watching the field is spec 93's
  learning, not this).
- Sharing, exporting or syncing samples; any upload. A user who wants to send a sample sends the
  file themselves.
- A benchmark screen in the app. The harness stays a command-line tool for now.
- Samples while history is off: with no history item there is nothing to correct.

## Pre-mortem

It is three months later and this failed. Why?

| Failure | Answer |
| --- | --- |
| The user did not know audio was being kept after the history deleted it | R-6 (one sheet before the first sample), R-11 (delete says the sample stays), R-12 (count, size, delete all) |
| Samples were scored against the cleaned text of a later retry, not what was first pasted | R-1 keeps the first text and the speech transcript apart from the shown text |
| The harness and the app disagreed on the folder format | AC-6: a contract test loads a store-written folder with the harness's own loader |
| The sample set filled up with right dictations and said nothing | R-5 keeps them (they measure regressions); the report counts zero-edit samples apart |
| A half-written JSON broke a benchmark run | R-9 (temp file and rename), AC-8 |
| Text from a sample ended up in a bug report | R-13, AC-7 |
| UI work collided with the Svelte move | Slices 1 to 3 touch no page; slice 4 starts after spec 257's History page has merged (tasks.md) |

## Decisions

| ID | Question | Recommendation | Answer |
| --- | --- | --- | --- |
| GT-1 | Which version? | 0.1. It does not move the date: slices 1 to 3 are core, platform and harness; slice 4 is one page action after spec 257. And it is what makes the gate's number come from daily use instead of hand-edited JSON | Decided 28 November 2026, as recommended: 0.1 |
| GT-2 | How does the user agree to keep audio past the history's retention? | The first Save as sample shows one sheet (R-6): each sample is a deliberate act, so a Settings switch that is off by default would only add a step. Rejected: keeping samples inside the history with a "pinned" flag, which ties a benchmark set to a store the user clears, and makes "delete my history" not mean what it says | Decided 28 November 2026, as recommended: the one-time sheet at the first save |
| GT-3 | Does a correction change the row's shown text? | Yes, the row shows what the user said was right, with a sample mark; the first text is kept for the benchmark (R-1) | decided in the draft; the owner did not object, 28 November 2026 |
| GT-4 | One folder in the harness's set layout, or a table in SQLite? | The folder: the harness is the only reader, it already reads that layout, and the user can see and delete it in Finder. Samples are few and written one at a time; SQLite stays the history's store | decided in this draft |
