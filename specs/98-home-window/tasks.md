# Home window and History: slices

Each slice is one pull request into `master`, test first, with the `AC-n` it proves. Order is the
order of the list; slices 2 and 3 can run in parallel once slice 1 has merged.

| # | Slice | Proves | Touches |
| --- | --- | --- | --- |
| 1 | **History queries and stats.** `SqliteHistory`: page of items (keyset on `started_ms, id`), search with case and accent folding, item by id, delete, replace text after a retry, failed-with-audio list; `stats(now, tz)` for words this week and time saved. `Item` gains `has_audio` and `message`. Pure Rust, portable | AC-6, AC-7, AC-8, AC-12, AC-13 (store half) | `crates/hyperme-platform/src/history.rs` and its tests |
| 2 | **Core: retry by id and the Dictionary trait.** `Event::Retry` gains an optional id, so any dictation with audio can be retried; a retry from Home copies instead of pasting. `traits::Dictionary`, `Correction`, `Added`, `Refusal`, and a test fake | AC-10, AC-11 (core half) | `crates/hyperme-core/src/{dictation,traits}.rs` and tests |
| 3 | **The window shell.** `home.rs`: create on open, destroy on close, Dock icon while open, size and position remembered, section and block lists, `open(app, target)`, `home_attention`, the push event. "Open HyperMe" in the menu; Reopen opens Home once the walkthrough is done. `home.html` with the sidebar, the Home page and an empty History | AC-1, AC-3, AC-4, AC-5, AC-14 | `app/src-tauri/src/{home,main,runtime}.rs`, `app/ui/home.html` |
| 4 | **History and Recent in the page.** Rows, day groups, paging, search, the actions and their keys, Paste again through the recorded front app, Retry rows, Delete confirm, Fix a word behind the optional dictionary, history-off line | AC-9, AC-10, AC-11, AC-13, AC-15 | `app/src-tauri/src/home.rs`, `app/ui/home.html` |
| 5 | **Measure.** Footprint before, open on 1,000 items, after close, five cycles; record in `docs/results/` | AC-2 | `docs/results/` |
