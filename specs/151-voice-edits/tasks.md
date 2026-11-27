# Voice edits of selected text: slices

Each slice is one pull request, starts from failing tests that cite its `AC-n`, and passes fmt,
clippy and tests before it merges. Open one issue per slice when the spec is approved. Build
starts as the first work of phase 3 (owner, 27 November 2026, E-2 in [spec.md](spec.md)).

| # | Slice | Proves | Size | Depends on |
| --- | --- | --- | --- | --- |
| 1 | The edit itself: `TextEditor`, the `edit-v1` prompt on `ChatCleanup`, forwarding through `AnyCleanup` and the cleanup fallback; `edit.rs` with prefix matching, limits, deadline, marker, quote stripping and white-space restore; `[edit]` config; edit samples in `hyperme-quality` and the first score in `docs/results/` | AC-3, AC-7, AC-8, AC-9, AC-10, AC-14 | M | none |
| 2 | Reading the selection: `Selection` trait, the copy fallback and `paste_restoring` over the existing pasteboard fakes; the macOS Accessibility probe and focus check (shared `ax` module), secure fields refused, messaging timeout | AC-4, AC-5, AC-6 | M | none |
| 3 | The engine and the pill: probe on key-down off the recording path, the route after the transcript, `Editing`, `Replaced`, the Copy card, Esc, deadline, errors and Retry; the Privacy section line | AC-1, AC-2, AC-11, AC-12 (fake half), AC-13, AC-16 | M | 1, 2 |
| 4 | History and timing: `kind`, `instruction` and `original` columns, Home row with Copy original and Copy result, the timing fields | AC-15 | S | 3 |
| 5 | Live check and measurement on the owner's Mac: TextEdit, Notes, Mail, Safari, Chrome, Slack, VS Code, Google Docs and Terminal (which read path, one ⌘Z undoes, clipboard restored); 30 timed edits; idle and peak memory before and after; numbers to `docs/results/` | AC-12, AC-17, AC-18 | S | 3 |

Slices 1 and 2 can run in parallel. Nothing here needs MCP or the phase 3 router; when the router
lands, the prefix list moves to it and the not-an-edit marker hands over to it (R-4, R-16).
