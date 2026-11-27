# Personal dictionary: slices

Each slice is one pull request, starts from failing tests that cite its `AC-n`, and passes fmt,
clippy and tests before it merges. Open one issue per slice when the spec is approved.

| # | Slice | Proves | Size | Depends on | Can start |
| --- | --- | --- | --- | --- | --- |
| D1 | `hyperme-core::dictionary`: port flowbench's matcher with accent folding and implicit forms; load the `dictation.dictionary` text file at start and on Settings save; substitute once on the raw transcript for every path (cleanup, short dictation, raw fallback); entries line in the cleanup prompt before the app line; substitution count in the timing log | AC-1, AC-2, AC-3, AC-4, AC-8, AC-9 | M | none | Now, if the owner agrees: it makes the existing config key do what it says and matches C-9 in the PRD ("the pipeline now uses dictionary substitution") |
| D1b | **Settings → Dictionary**: the page in the Settings window (list, search, add, edit, remove, heard forms, import a text file) and "Dictionary…" in the menu-bar menu; saves to the dictionary text file (the configured one, or `dictionary.txt` in the app's data folder) and applies from the next dictation with no restart | R-10, R-10a, R-11, AC-8 | M | D1 | Next, at the owner's request |
| D2 | `dictionary.sqlite` store in `hyperme-platform`: entries, heard forms, suggestions, dismissals, use counts; one-time import of the text file; import and export of text and CSV; limits | AC-10, AC-11 | M | D1 | Now, at the owner's request (27 November 2026), with the add sheet changed to Wispr Flow's misspelling switch |
| D3 | Prompt cap and relevance filter; speed test over 1,000 entries | AC-5, AC-6 | S | D1 | Done: `dictation.dictionary_prompt_cap` (default 150); above it the warm-up prefix carries no dictionary and each dictation gets its relevant entries; 0.5 to 0.8 ms in release on the owner's Mac for 1,000 entries and 2,000 words |
| D4 | The same Dictionary page in Home's sidebar, plus export, use counts and the learning switch; `dictionary_*` Tauri commands | R-10, R-13 | S | D1b, D2 and the Home window shell (Home window thread) | Now, at the owner's request (27 November 2026): the page in Home shares dictionary.js with Settings; use counts and the learning switch come with D3 and D6 |
| D5 | The `Dictionary` trait's `add_correction` for History → Fix, and Paste again | AC-12 | S | History (Home window thread) | Now, at the coordinator's request once Home merged: it writes the dictionary text file until D2 moves it to SQLite. Home's Dictionary section opens Settings → Dictionary until D4 |
| D6 | Candidate finder and learning policy in `hyperme-core`: alignment, filters, raw mapping, common words from the built-in list and from history, twice-then-add, dismissals; the Suggested block and the Home page line | AC-13, AC-14 | M | D2 | Done: `hyperme-core::learning` (finder and policy, fixtures in `tests/fixtures/dictionary/edits.txt`), `DictionaryStore::learn`, use counts after each paste, the Suggested block, the learned mark with Undo, the switch and the Home line. Nothing calls `learn` until D7's watcher |
| D7 | macOS `FieldWatcher`: focused element, secure checks, `AXObserver`, 60 s window; per-app check; idle and active CPU and memory | AC-15, AC-16, AC-17 | M | D6 | Done in code: `hyperme-core::field_watch` (the rules, AC-15 with fakes), `MacFieldWatcher` in `hyperme-platform` (a thread and an `AXObserver` only during the 60 s after a paste), wired after each paste and stopped at the next recording. AC-16 idle measured; the typing part and AC-17 wait for the owner (checklist below, #214) |
| D8 | Measure: the full pipeline on the 50 samples with 0, 150 and 1,000 entries; Groq cached tokens with the warm-up prefix; then the experiment of Speechmatics `melia-1` + `multi` + `additional_vocab` (accepted? names? edits per 100 characters? session start?) | AC-7, R-7 | S | D1, D3; the dictation quality tests thread's gold set | Done for the replays ([results](../../docs/results/2026-11-27-dictionary-sizes.md)): 30 of 30 names and 6.1 to 6.6 edits per 100 characters with 10, 150 or 1,000 entries; the cleanup's median stays near 360 ms. Groq cached tokens and the Speechmatics `additional_vocab` experiment are not measured |

D1 and D3 are pure core work and touch no screen. D4 and D5 wait for the Home window and
History, which another thread is building; the hand-off between the two is the `Dictionary` trait
in `hyperme_core::traits` (design.md, "UX").

## AC-16 and AC-17: checks for the V0.1 week of daily use

These need the owner's hands; record the answers in `docs/results/` (numbers and yes or no
only, no text).

1. With "Learn from my edits" on, dictate a sentence with a name the dictionary does not know
   into each app below, fix the name where it was pasted, and wait a few seconds or click
   elsewhere. Then open Dictionary: is the fix under **Suggested**?
   - TextEdit · Notes · Slack · Mail · Chrome (a text area on a web page) · VS Code
2. Do the same fix a second time in one app where it worked: is the word now in the list with
   the **learned** mark, and is it right in the next dictation?
3. In an app where nothing appeared, check that nothing broke and that Fix from History still
   adds the word.
4. AC-16, typing: dictate into TextEdit, then type for 60 s in the same field while Activity
   Monitor shows HyperMe's CPU. The average should stay under 1%.
5. Never in a password field: dictation is refused there already (secure input), and the watcher
   reads nothing when secure input is on.
