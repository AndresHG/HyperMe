# Trace store: slices

Each slice is one pull request, starts from failing tests that cite its `AC-n`, and passes fmt,
clippy and tests on the owner's Mac before it merges (GitHub CI is blocked by billing).

| # | Slice | Proves | Size | Depends on |
| --- | --- | --- | --- | --- |
| 1 | `trace.rs`: schema, `TraceLayer`, `FinishedTrace`, `TimingLine::from_trace`, `TimingSink`; spans in `dictation.rs` (dictate, retry, retry by id); the engine loses `T: TimingLog` and the `Instant` fields; the app installs the subscriber with the timing sink | AC-1, AC-7, AC-8, and the schema half of AC-3 | M | none |
| 2 | `trace_store.rs`: tables, writer thread, retention with the history, Keep timing traces switch, `spans_for` and `stage_percentiles` | AC-2, AC-3, AC-4, AC-5, AC-6 | M | 1 |
| 3 | Warnings and errors file; the `println!` and `eprintln!` calls move to `tracing` events | AC-3 (warnings file) | S | 1 |
| 4 | Developer features `trace-chrome` and `otel`; `cargo deny`; README lines in `CONTRIBUTING.md` (Developer settings) | AC-9 | S | 1 |
| 5 | Live: idle memory and CPU, and replay latency, before and after; numbers to `docs/results/` | AC-10, AC-11 | S | 2 |

Slices 2, 3 and 4 can run in parallel once slice 1 has merged.
