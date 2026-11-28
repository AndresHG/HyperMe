# Samples: slices

Each slice is one pull request, checked on the owner's Mac (`cargo fmt`, `cargo clippy
--all-targets -- -D warnings`, `cargo test`, and `npm run check` and the page tests for slice 4).
Tests first; each cites its AC.

| Slice | What | ACs | Touches |
| --- | --- | --- | --- |
| 1 | History keeps the speech transcript and the first text; `History::done` gains the transcript | AC-1 | core traits and engine, platform history |
| 2 | `Samples` trait and `SampleStore` on the folder; contract test with the harness loader | AC-6 (store side), AC-8 | core, platform, quality tests |
| 3 | `correct()` and the commands (`home_correct`, `home_delete_sample`, `home_samples_of`, `samples_view`, `samples_delete_all`, `samples_reveal`); harness `--set samples` and the speech column | AC-2 to AC-7 | app commands, quality |
| 4 | The Correct action in `actions()`, the editor, the first-save sheet, the sample mark and the delete wording on the History page; the samples row in Privacy; typed bindings in `ipc.ts` | AC-10 | Svelte pages. Starts after spec 257's History page has merged |
| 5 | Live: 10 samples from the owner's history, one harness run, save time | AC-9 | `docs/results/` (numbers only) |

Slices 1 to 3 touch no page, so they can run while spec 257 finishes.

Done in PR #268: slices 1 to 4 (28 November 2026). Left: slice 5, the live check.
