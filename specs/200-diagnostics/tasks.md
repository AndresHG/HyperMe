# Diagnostics and Report a problem: slices

Each slice is one pull request, starts from failing tests that cite its `AC-n`, and passes fmt,
clippy, tests and `cargo deny` on the owner's Mac before it merges.

| # | Slice | Proves | Size | Depends on |
| --- | --- | --- | --- | --- |
| 1 | `diagnostics.rs` (core) and the store reads `traces_since`, `outcome_counts` | AC-1, AC-2, AC-3 | M | none |
| 2 | Settings → Diagnostics section: the view, Refresh, the too-few and off notices, the warnings list | AC-2, AC-3 (UI), AC-11 (open time) | M | 1 |
| 3 | Stage bars on expanded History items | AC-4 | S | 1 |
| 4 | `crash.rs`: panic hook, pruning, crash notice, `.ips` finder | AC-9, AC-10 | M | none |
| 5 | `report.rs`: allowlist, sections, scrub, cap; the preview sheet, Save, Open issue; the issue template line | AC-5, AC-6, AC-7, AC-8 | M | 1, 4 |
| 6 | Live: Diagnostics and bundle timings, a real issue from a bundle; numbers to `docs/results/` | AC-11, AC-12 | S | 2, 5 |

Slices 1 and 4 can start in parallel. 2, 3 and 5 follow 1.
