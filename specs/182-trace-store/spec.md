# Trace store: local traces of every dictation

**Status:** draft, for the owner's approval. One open question (TS-1).
**Issue:** [#182](https://github.com/AndresHG/hyperme/issues/182). **Size:** L (new crates, core
and platform, changes spec 25 R-30).
**Traces to:** [telemetry research](../../docs/research/2026-11-27-telemetry-and-tracing.md)
(T-1, T-3); [PRD](../../docs/PRD.md) Q-18 (no telemetry, a report bundle; owner, 27 November
2026); [ROADMAP](../../docs/ROADMAP.md) phase 2; spec [25](../25-dictation-core/spec.md) R-29,
R-30, AC-18, AC-20; spec [96](../96-provider-settings/spec.md) R-20 and spec
[98](../98-home-window/spec.md) (both want the user's median latency, which today needs the
timing log, off by default).
**Followed by:** a spec for the Diagnostics view, stage bars on history items and the Report a
problem bundle, which read this store.

## Problem

HyperMe has to be very fast, and the owner wants full traceability to keep it that way. Today the
only record is one optional line of stage timings per pasted dictation (`timing.tsv`, R-30):

- It is off by default, so the numbers spec 96 and spec 98 want to show do not exist for most
  users, and the owner has to remember to turn it on before a slow dictation happens.
- It records only dictations that pasted. A failed, cancelled or retried dictation leaves nothing.
- It cannot say *why* a stage was slow: how long the microphone took to start, how many bytes
  went to speech, whether cleanup retried, how long the dictionary pass took.
- It is not linked to the history item, so a slow dictation in the Home window cannot show its
  stages.
- Each stage is timed by hand with `Instant` fields in `dictation.rs`; every new feature (voice
  edits, commands, syncs) would add more of the same.

## Goal

Every dictation leaves a trace on the Mac: one root span with a child span per stage, numbers and
names only, stored in SQLite next to the history and linked to its history item. The timing log
keeps its columns and becomes one output of those spans. A developer build can also send the same
spans to Perfetto or to an OpenTelemetry viewer. A release build sends nothing anywhere, and the
idle and latency budgets do not move.

## Words used here

- **Span:** one timed piece of work with a name, a start, a duration, a parent and fields.
- **Trace:** the tree of spans of one dictation (later: one voice edit, command or sync job).
- **Field:** a value on a span: an integer, a boolean, or one of a fixed set of names (an enum
  value, a provider or model name from the config, a language code).
- **Trace store:** the SQLite tables that keep traces on the Mac.
- **Developer build:** a build with the `trace-chrome` or `otel` Cargo feature. Releases never
  have either.

## Requirements

EARS form. "The app" is HyperMe on macOS.

### Instrumentation

- R-1. The app shall record what it does through the `tracing` crate: the core creates spans; it
  does not know where they go.
- R-2. For each dictation, the app shall create one root span `dictation` and child spans for its
  stages: `mic_start` (key-down to the first captured sample), `capture` (key-down to key release),
  `speech` (release to the final transcript), `dictionary`, `cleanup` (with `first_token` as an
  event), `insert` (text ready to paste sent) and `history_write`. A retry (spec 25 R-31) is its
  own `dictation` root with `retry_of` set.
- R-3. The `dictation` span shall carry: the history id when history is on, the outcome
  (`pasted`, `copied`, `failed`, `cancelled`, `too_short`), the error kind on failure, the
  language code, the word count, the fallback flag, the speech and cleanup provider and model
  names, and the cleanup level.
- R-4. Child spans shall carry only what explains their time: bytes of audio sent and attempts
  (`speech`), entries applied (`dictionary`), tokens in and out and attempts (`cleanup`).
- R-5. No span or event shall carry text: no transcript, selection, cleaned text, dictionary
  entry, app name, window title, file path, account, URL or key. The trace store keeps only the
  fields declared for each span name and drops anything else.
- R-6. Instrumentation shall not change behaviour: with no output attached, the dictation flow
  shall run exactly as it does today.

### The trace store

- R-7. The app shall write each finished trace to the trace store, in the app's data folder next
  to the history, after the paste (or the failure) is shown, never on the path from key release to
  paste.
- R-8. The trace store shall keep every dictation's trace, whether or not it pasted, and link it
  to its history item when one exists.
- R-9. When a history item is deleted, the app shall delete its trace. Traces shall follow the
  history's retention days and be counted in its size limit. (With history off: TS-1.)
- R-10. The trace store shall offer, to the rest of the app, the spans of one trace by history id,
  and the median and 90th percentile of each stage over a time window, by provider and model.
- R-11. A write failure in the trace store shall never fail or slow a dictation; it shall be
  logged as a warning and the trace dropped.

### The timing log

- R-12. When timing is on (spec 25 R-30), the app shall write the same `timing.tsv` line as today,
  same columns and definitions, from the finished `dictation` span. flowbench comparisons and
  `summarise` shall keep working unchanged.
- R-13. The engine's hand-made stage timings (`Text::transcript_at`, `first_token_at`,
  `ready_at`, and the `TimingLine` assembly) shall be replaced by the spans; the `TimingLog` trait
  may stay as the timing log's writer.

### Warnings and errors

- R-14. The app shall keep a local file of warnings and errors (`~/Library/Logs/HyperMe/`), at most
  1 MB with one rotated copy, with the same no-text rule as R-5. The 9 `println!` and `eprintln!`
  calls in the app and core shall move to it.

### Developer builds

- R-15. With the `trace-chrome` feature, the app shall write a Perfetto-readable trace file of the
  session when `HYPERME_TRACE_CHROME=<path>` is set.
- R-16. With the `otel` feature, the app shall export spans over OTLP/HTTP when
  `HYPERME_OTLP_ENDPOINT` is set, with headers from `HYPERME_OTLP_HEADERS` (for example
  Honeycomb's team key). Both come from the environment only, never the config file, and are never
  logged.
- R-17. A release build shall contain neither feature's crates, and shall open no network
  connection for tracing. R-29 of spec 25 holds unchanged.

### Budgets

- R-18. Idle memory with the trace store shall be no more than 2 MB above the same build without
  it, and idle CPU shall stay near 0%.
- R-19. Key release to text ready shall be no more than 1 ms slower at the 90th percentile.

## Acceptance criteria

*(fake)*: runs in CI with fakes. *(live)*: run on the owner's Mac, numbers to `docs/results/`.

- AC-1 *(fake)*: A dictation through fakes produces one `dictation` trace with the child spans of
  R-2, in order, nested under the root, with the fields of R-3 and R-4. (R-1 to R-4)
- AC-2 *(fake)*: A failed, a cancelled, a too-short and a retried dictation each leave a trace with
  the right outcome; the retry names the trace it retries. (R-2, R-3, R-8)
- AC-3 *(fake)*: Privacy grep. Fixtures with a transcript, a dictionary entry, an app name, an
  email address, a home-folder path and a fake key go through a dictation, a failure and a retry;
  the trace store's database file, the timing log and the warnings file contain none of them.
  A span created with an undeclared text field stores nothing for it. Extends spec 25 AC-18.
  (R-5, R-14)
- AC-4 *(fake)*: Deleting a history item deletes its trace; retention and the size limit remove
  traces with their items. (R-9)
- AC-5 *(fake)*: The trace store returns one trace's spans by history id, and the median and p90
  per stage over a window, matching a hand-computed fixture. (R-10)
- AC-6 *(fake)*: With the store failing every write, dictations still paste and the failure is a
  warning. (R-11)
- AC-7 *(fake)*: With timing on, the `timing.tsv` line for a fixture dictation is byte-identical to
  the line today's code writes for it (same columns, same numbers); with timing off, nothing is
  written. Replaces spec 25 AC-20's test. (R-12, R-13)
- AC-8 *(fake)*: With no output attached, the existing dictation tests pass unchanged. (R-6)
- AC-9 *(fake)*: `cargo tree -e normal` of the release app contains no `opentelemetry*` and no
  `tracing-chrome`; with `--features otel` against a local test server, spans arrive as OTLP/HTTP
  with the configured headers, and the headers appear in no log. (R-15 to R-17)
- AC-10 *(live)*: `scripts/measure-idle.sh --after-dictations` before and after: idle memory
  within 2 MB, idle CPU near 0%. (R-18)
- AC-11 *(live)*: A flowbench replay, or 50 live dictations, before and after: key release to text
  ready within 1 ms at p90. (R-19)
- AC-12 *(fake)*: Tests make no network call outside localhost and need no key (`CLAUDE.md`).

## Design sketch

Details go in `design.md` after approval.

- **Crates.** Release: `tracing` in `hyperme-core` (spans only, `default-features = false`, with
  `release_max_level_info` if measurement favours it); `tracing-subscriber` with the `registry`
  feature only, in `hyperme-platform`. Developer features: `tracing-chrome`; `opentelemetry`,
  `opentelemetry_sdk`, `opentelemetry-otlp` (HTTP, native TLS, matching the app's TLS rule) and
  `tracing-opentelemetry`. Each checked against `deny.toml` and justified in its commit.
- **Store layer.** A `tracing-subscriber` layer in `hyperme-platform` collects a trace's spans in
  memory while it runs, keeps only the declared fields per span name (R-5), and hands the finished
  trace to a writer on a background task. It writes into `history.sqlite3`, using the history's
  connection and its retention pass, so there is no second database or timer: tables `trace` (id,
  history id, kind, started, duration, outcome) and `span` (trace id, name, parent, offset,
  duration, declared fields).
- **Timing log.** A second small layer turns a finished `dictation` span into today's
  `TimingLine` and calls `FileTimingLog`.
- **Order of work.** Slices: (1) spans in the engine with a test layer, timing log on spans (AC-1,
  AC-7, AC-8); (2) store layer and tables (AC-2 to AC-6); (3) warnings file (R-14); (4) developer
  features (AC-9); (5) live budgets (AC-10, AC-11).

## Out of scope

- The Diagnostics view, stage bars on history items, median latency on Home and in provider rows,
  and the Report a problem bundle: the follow-up spec.
- Spans for voice edits, commands, connector syncs, the morning card and the Home window: each
  feature adds its own in its spec, using this store.
- Any telemetry or crash report sent from a user's Mac (PRD Q-18; roadmap Later).
- Crash capture (a panic hook writing a local file): the follow-up spec, with the bundle.

## Open question for the owner

| ID | Question | Recommendation |
| --- | --- | --- |
| TS-1 | With history off, does the app still keep traces? | Yes: traces hold no text, only numbers and provider names, and Diagnostics needs them. Keep 30 days, with the history's size limit, and a switch "Keep timing traces" (on) under Settings → History, so a user who wants nothing kept can turn it off |

## Risks

| Risk | Mitigation |
| --- | --- |
| Text slips into a span field through a later change | Fields are declared per span name and anything else is dropped (R-5); the privacy grep (AC-3) runs on every PR |
| Tracing costs memory or latency | Store writes after the paste on a background task; budgets as acceptance criteria (AC-10, AC-11) |
| OpenTelemetry crates pull a second TLS stack or a licence outside policy | Developer features only, never in a release (AC-9); `cargo deny` before adding |
| Changing R-30 breaks Phase 0 comparisons | Byte-identical timing line test (AC-7) |
