# Trace store: design

For [spec.md](spec.md). Slices in [tasks.md](tasks.md).

## 1. Crates

| Crate | Where | Build | Why |
| --- | --- | --- | --- |
| `tracing` 0.1 | `hyperme-core`, `hyperme-platform` | release | Spans and events. Already in the tree through Tokio, Tauri and reqwest (`Cargo.lock`), so it adds no new code to the binary; `default-features = false`, `std` only |
| `tracing-subscriber` 0.3 | `hyperme-core` | release | The `Layer` trait and the `Registry` that tracks span parents. `default-features = false`, features `registry` and `std` only: no `fmt`, no `env-filter`, no regex |
| `tracing-chrome` | `app/src-tauri` | feature `trace-chrome` | Perfetto file (R-15) |
| `opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp`, `tracing-opentelemetry` | `app/src-tauri` | feature `otel` | OTLP/HTTP export (R-16). HTTP transport on the workspace `reqwest` with `native-tls`, never `tonic` or `rustls`; checked with `cargo deny` before the slice merges |

Versions are taken from crates.io when each slice starts, and each crate's reason goes in its
commit message (`CLAUDE.md`).

## 2. Where the code lives

```
hyperme-core/src/trace.rs        span names, the field schema, FinishedTrace, TraceLayer,
                                 TimingLine::from_trace, the TraceSink trait
hyperme-core/src/dictation.rs    creates the spans; no Instant bookkeeping for R-30
hyperme-platform/src/trace_store.rs   TraceSink on SQLite (tables in history.sqlite3)
hyperme-platform/src/timing.rs   FileTimingLog, unchanged, fed by a TraceSink adapter
hyperme-platform/src/warn_log.rs warnings and errors file (R-14)
app/src-tauri/src/tracing_setup.rs    builds the subscriber at start-up; dev features
```

The core owns the layer because it owns the schema and the timing definitions, and both need
fake-driven tests (AC-1, AC-7). The layer knows nothing about SQLite or files: it hands each
finished trace to a `TraceSink`.

## 3. Spans

One root per dictation, children created with an explicit parent, never entered across an
`.await`. A span's time runs from creation to close (drop), so a stage span is created where the
stage starts and dropped where it ends; the engine already has those points.

```
dictation  {history_id, outcome, error_kind, language, words, fallback,
            speech_provider, speech_model, cleanup_provider, cleanup_model, cleanup_level,
            retry_of}
├── mic_start      key-down → first captured sample
├── capture        key-down → key release
├── speech         release → final transcript   {audio_bytes, attempts}
├── dictionary     {entries_applied}
├── cleanup        {tokens_in, tokens_out, attempts}   event: first_token
├── insert         text ready → paste sent
└── history_write
```

Fields not known at creation are declared `tracing::field::Empty` and recorded later with
`span.record`. The provider and model names come from the `ProviderSwitch` names, which the
timing log already uses.

### Timestamps

The layer stamps `new_span`, `on_close` and events with `tokio::time::Instant::now()`. Under a
paused Tokio clock (the core's tests) that is the fake time, so traces are exact and
deterministic; in the app it is the monotonic clock. Offsets are stored in microseconds from the
root's start.

## 4. The no-text rule (R-5)

`trace.rs` holds a static schema:

```rust
pub enum Kind { U64, Bool, Name }
pub const SCHEMA: &[(&str /* span */, &[(&str /* field */, Kind)])] = &[ ... ];
```

The layer's visitor keeps a field only when its span name and field name are in the schema and
the value has the declared kind; everything else is dropped. `Name` fields take a `&str` and
are only the enums (`outcome`, `error_kind`, `cleanup_level`), the language code and the
provider and model names. A `Name` value longer than 64 bytes, or with a space or a line break
outside the provider and model fields, is dropped. The privacy grep (AC-3) is the backstop.

## 5. The layer and the sinks

```rust
pub struct FinishedTrace {
    pub kind: &'static str,          // "dictation"
    pub started_unix_ms: u64,
    pub spans: Vec<FinishedSpan>,    // root first, then by start offset
}
pub struct FinishedSpan {
    pub name: &'static str,
    pub parent: Option<usize>,       // index into spans
    pub start_us: u64,
    pub duration_us: u64,
    pub fields: Vec<(&'static str, FieldValue)>,
    pub events: Vec<(&'static str, u64)>,   // name, offset in µs
}
pub trait TraceSink: Send + Sync + 'static {
    fn finished(&self, trace: &FinishedTrace);
}
```

`TraceLayer` keeps open spans in the registry's extensions, and when a root closes it assembles
the `FinishedTrace` and calls every sink. Sinks must return quickly: the SQLite sink only sends
the trace over a channel to a writer thread (R-7, R-11). A trace whose root never closes (the
app quits mid-dictation) is dropped.

## 6. Timing log on spans (R-12, R-13)

`TimingLine::from_trace(&FinishedTrace) -> Option<TimingLine>` maps:

| Column | From |
| --- | --- |
| `first_audio_ms` | end of `mic_start` |
| `release_ms` | end of `capture` |
| `stt_ms` | end of `speech` − release |
| `first_ms` | `first_token` event − release |
| `total_ms` | start of `insert` − release (text ready) |
| `paste_ms` | end of `insert` − release |
| `language`, `words`, `fallback` | root fields |

It returns `None` unless the outcome is `pasted`, as today. A `TimingSink` adapter calls the
existing `TimingLog` when the timing switch (an `AtomicBool` shared with Settings) is on. The
engine loses its `T: TimingLog` parameter and `Text`'s three `Instant` fields; the existing
AC-20 tests move to a test subscriber and keep their exact numbers (AC-7).

## 7. The trace store (slice 2)

Tables in `history.sqlite3`, created by `SqliteHistory::open`:

```sql
CREATE TABLE IF NOT EXISTS trace (
  id INTEGER PRIMARY KEY, history_id INTEGER, kind TEXT NOT NULL,
  started_ms INTEGER NOT NULL, duration_us INTEGER NOT NULL, outcome TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS span (
  trace_id INTEGER NOT NULL REFERENCES trace(id) ON DELETE CASCADE,
  idx INTEGER NOT NULL, parent INTEGER, name TEXT NOT NULL,
  start_us INTEGER NOT NULL, duration_us INTEGER NOT NULL,
  fields TEXT NOT NULL,           -- compact key=value list, schema fields only
  PRIMARY KEY (trace_id, idx));
CREATE INDEX IF NOT EXISTS trace_history ON trace (history_id);
CREATE INDEX IF NOT EXISTS trace_started ON trace (started_ms);
```

A writer thread owns its own connection (WAL mode, which the history already tolerates), batches
inserts, and on the history's cleanup pass deletes traces by retention and by `history_id`. With
history off it keeps 30 days; with Keep timing traces off it writes nothing (R-9a). Queries for
R-10: `spans_for(history_id)` and `stage_percentiles(since, provider, model)`, computed in Rust
over at most a few thousand rows.

## 8. Start-up

`tracing_setup::init(config)` builds `Registry::default().with(TraceLayer::new(sinks))`, plus
`tracing-chrome` or the OTLP layer when their feature is compiled and their environment variable
is set, and installs it as the global default once. Nothing else in the app reads the
subscriber.

## 9. Measuring

Slice 5 runs `scripts/measure-idle.sh --after-dictations` and a 50-dictation replay on the
commit before slice 1 and after slice 2, and writes both to `docs/results/`.
