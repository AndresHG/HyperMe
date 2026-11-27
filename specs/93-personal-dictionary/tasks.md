# Personal dictionary: slices

Each slice is one pull request, starts from failing tests that cite its `AC-n`, and passes fmt,
clippy and tests before it merges. Open one issue per slice when the spec is approved.

| # | Slice | Proves | Size | Depends on | Can start |
| --- | --- | --- | --- | --- | --- |
| D1 | `hyperme-core::dictionary`: port flowbench's matcher with accent folding and implicit forms; load the `dictation.dictionary` text file at start and on Settings save; substitute once on the raw transcript for every path (cleanup, short dictation, raw fallback); entries line in the cleanup prompt before the app line; substitution count in the timing log | AC-1, AC-2, AC-3, AC-4, AC-8, AC-9 | M | none | Now, if the owner agrees: it makes the existing config key do what it says and matches C-9 in the PRD ("the pipeline now uses dictionary substitution") |
| D1b | **Settings → Dictionary**: the page in the Settings window (list, search, add, edit, remove, heard forms, import a text file) and "Dictionary…" in the menu-bar menu; saves to the dictionary text file (the configured one, or `dictionary.txt` in the app's data folder) and applies from the next dictation with no restart | R-10, R-10a, R-11, AC-8 | M | D1 | Next, at the owner's request |
| D2 | `dictionary.sqlite` store in `hyperme-platform`: entries, heard forms, suggestions, dismissals, use counts; one-time import of the text file; import and export of text and CSV; limits | AC-10, AC-11 | M | D1 | Now, at the owner's request (27 November 2026), with the add sheet changed to Wispr Flow's misspelling switch |
| D3 | Prompt cap and relevance filter; speed test over 1,000 entries | AC-5, AC-6 | S | D1 | With D2 |
| D4 | The same Dictionary page in Home's sidebar, plus export, use counts and the learning switch; `dictionary_*` Tauri commands | R-10, R-13 | S | D1b, D2 and the Home window shell (Home window thread) | Now, at the owner's request (27 November 2026): the page in Home shares dictionary.js with Settings; use counts and the learning switch come with D3 and D6 |
| D5 | The `Dictionary` trait's `add_correction` for History → Fix, and Paste again | AC-12 | S | History (Home window thread) | Now, at the coordinator's request once Home merged: it writes the dictionary text file until D2 moves it to SQLite. Home's Dictionary section opens Settings → Dictionary until D4 |
| D6 | Candidate finder and learning policy in `hyperme-core`: alignment, filters, raw mapping, common words from the built-in list and from history, twice-then-add, dismissals; the Suggested block and the Home page line | AC-13, AC-14 | M | D2 | Phase 2 |
| D7 | macOS `FieldWatcher`: focused element, secure checks, `AXObserver`, 60 s window; per-app check; idle and active CPU and memory | AC-15, AC-16, AC-17 | M | D6 | Phase 2 |
| D8 | Measure: the full pipeline on the 50 samples with 0, 150 and 1,000 entries; Groq cached tokens with the warm-up prefix; then the experiment of Speechmatics `melia-1` + `multi` + `additional_vocab` (accepted? names? edits per 100 characters? session start?) | AC-7, R-7 | S | D1, D3; the dictation quality tests thread's gold set | After D3 |

D1 and D3 are pure core work and touch no screen. D4 and D5 wait for the Home window and
History, which another thread is building; the hand-off between the two is the `Dictionary` trait
in `hyperme_core::traits` (design.md, "UX").
