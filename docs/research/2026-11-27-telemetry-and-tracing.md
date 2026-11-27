# Telemetry and traceability: what HyperMe measures, where it goes, who sees it

Research of 27 November 2026, asked by the owner: "full traceability of things happening, so it
is easier to improve the system and optimise", on his own Mac and for other people who install
HyperMe. Research and recommendation only: nothing here changes an earlier decision. The one
choice it needed from the owner, what other users' copies may send, was decided on 27 November
2026: nothing, plus a report bundle (section 7, T-2). "No telemetry" stands.

## 1. Summary

- **What we have today is a good start, but flat.** One optional line of stage timings per
  dictation (`timing.tsv`, spec 25 R-30), off by default, numbers only. It cannot show *why* a
  dictation was slow (a retry, a slow first byte, a long dictionary pass), it covers dictation
  only (not commands, tasks, connectors or the Home window), and the history does not keep it.
- **For the owner's Mac: adopt the `tracing` crate as the one way the app instruments itself.**
  One trace per dictation (and per command, sync job and morning card), with a span per stage.
  Spans are kept on the Mac in SQLite next to the history, so every history item can show where
  its time went. The timing log becomes one output of those spans. A developer-only build
  feature exports the same spans over OpenTelemetry to a local viewer or to Honeycomb.
- **Honeycomb is not overkill for the owner, but it must stay a developer tool.** At about ten
  spans a dictation, even 500 dictations a day is about 150,000 events a month, under 1% of the
  free plan's 20 million. It belongs in a developer build with the owner's own key, never in the
  release.
- **For other users: keep "no telemetry", and add a diagnostic bundle.** Their traces stay on
  their Mac and power a Diagnostics view they can read. When something goes wrong they press
  **Report a problem**, see exactly what would be shared (numbers, versions, settings, errors;
  never text, audio, keys or names), and attach it to a GitHub issue themselves. Nothing leaves
  the Mac on its own.
- **Opt-in anonymous metrics or crash reports are possible later, and are the owner's call.**
  They would change a public promise (PITCH, PRD sections 8 and 9). The recommendation is to
  wait until after the first release, and to revisit only if issues alone do not explain what
  goes wrong.
- **Cost against the budgets:** `tracing` with a SQLite sink adds a small, measurable amount; the
  OpenTelemetry exporter stays out of the release binary. Gate: idle memory grows by no more than
  2 MB and key release to text by no more than 1 ms, measured before merging.

## 2. What the repository has today

Checked on `master` at d67d2db, 27 November 2026.

| Piece | Where | What it records | Limits |
| --- | --- | --- | --- |
| Timing log (R-30, AC-20) | `crates/hyperme-platform/src/timing.rs`, written to `~/Library/Logs/HyperMe/timing.tsv` | One tab-separated line per pasted dictation: first audio, release, `stt_ms`, `first_ms`, `total_ms`, `paste_ms`, language, words, fallback, provider and model names | Off by default (`[timing] log = false`). Pasted dictations only: a failed or cancelled dictation leaves no line. No stage breakdown inside speech or cleanup. Not linked to the history item |
| `summarise` | same file | Median and 90th percentile for AC-10, with flowbench's definitions | Reads the one file |
| Dictation history (R-18) | `crates/hyperme-platform/src/history.rs` (SQLite) | Audio and text, kept 30 days by default | No timings |
| Phase 0 and replay results | `docs/results/speed-test-*.md`, `replay-*.md`, `2026-11-26-wispr-comparison.md` | Benchmark numbers from flowbench on recorded utterances | Offline runs, not live use |
| Idle and latency measurements | `docs/results/2026-11-26-idle-usage-baseline.md` and others | 59.4 MB idle, 0% CPU (no engine running); key-down and mic-start latencies | Measured by scripts, one at a time |
| Logging | none | 9 `println!`/`eprintln!` calls in the app and core; no `log` or `tracing` crate | Nothing to read after a problem |
| Crash reports | none of our own | macOS writes its own `.ips` reports | Unknown to the app |

Privacy rules that bound this work: `CLAUDE.md` ("No telemetry"), spec 25 R-29 (network traffic
only to the providers the user configured) and R-30 (timings are numbers only, never text), the
risk table in spec 25 ("a key leaked into a log or a crash report": AC-18 greps outputs), spec 97
(no token in logs or crash reports), PRD section 9 ("Never: telemetry; any destination the user
did not configure") and PRD section 10 (adoption measured only from the outside: downloads, stars
and weekly users who report in issues).

## 3. Local traceability on the owner's Mac

### 3.1 The instrument: `tracing`

`tracing` (MIT, from the Tokio project) is the Rust standard for spans and structured events. The
core already runs on Tokio, so it fits the state machine without a new runtime. What it gives:

- **Spans with fields.** `dictation{id, language, words, fallback}` with children `capture`,
  `speech{provider, model, bytes, attempt}`, `speech.first_byte`, `dictionary{hits}`,
  `cleanup{provider, model, level, tokens_in, tokens_out}`, `cleanup.first_token`, `insert`,
  `paste`. The same shape later for `command{tool}`, `sync{connector}`, `brief`, `home.open`.
- **Near-zero cost when off.** A disabled callsite is a cached check of a static flag; levels can
  be compiled out with the crate's `max_level_*` features. To confirm by measurement, not assume.
- **Swappable outputs ("subscribers" and "layers").** The code says *what* happened once; what is
  done with it is chosen at start-up. This is the modularity the project wants: platform and
  sink code stay out of the core.

Rule that keeps privacy by construction: **span fields are typed numbers, enums, provider and
model names, and error kinds. No field takes free text.** A transcript, a selection, an email
subject or a file path cannot be recorded because no field accepts one. A test greps every sink's
output for fixture text and key shapes, extending AC-18.

### 3.2 Where the spans go

| Output | Build | Purpose | New crates |
| --- | --- | --- | --- |
| **Trace store** in SQLite, next to the history (`trace` and `span` tables, keyed by the dictation id) | Release | Every history item shows its stage bars ("speech 410 ms, cleanup 290 ms, paste 12 ms"); Diagnostics shows medians and p90 per provider; kept as long as the history and deleted with it | `tracing`, `tracing-subscriber` (both MIT) |
| **Timing log** (`timing.tsv`, unchanged columns) | Release | Kept so flowbench comparisons and `summarise` keep working; written from the dictation span when it closes | none |
| **Warnings and errors file**, rotated, a few hundred KB | Release | What went wrong, without text: "speech provider returned 429", "paste target lost" | none, or `tracing-appender` (MIT) |
| **Perfetto file** (`tracing-chrome`, MIT) | Developer feature `trace-chrome` | Deep dives: open in `ui.perfetto.dev` and see every span of a session on a timeline | `tracing-chrome` |
| **OpenTelemetry export** (OTLP over HTTP) | Developer feature `otel`, never in a release | Query across weeks: "p90 cleanup by model since Monday", "every dictation over 1.2 s and what they share" | `opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp`, `tracing-opentelemetry` (Apache-2.0 or MIT); their transitive tree to check against `deny.toml` and the native-tls rule before adding |

The trace store is what makes "full traceability" useful day to day: the owner opens a slow
dictation in the Home window and sees which stage took the time, without any tool running. It
also replaces scattered `Instant` bookkeeping in `dictation.rs` with one mechanism.

### 3.3 Viewers for the developer build

| Viewer | What it is | Cost | Data leaves the Mac |
| --- | --- | --- | --- |
| `otel-desktop-viewer` | One binary with an OpenTelemetry collector, DuckDB and a browser UI on `localhost:8000`; OTLP on 4317 and 4318; Apache-2.0; Homebrew cask ([pkg.go.dev](https://pkg.go.dev/github.com/CtrlSpice/otel-desktop-viewer)) | Free | No |
| Perfetto UI | Opens the `tracing-chrome` file in the browser ([tracing-chrome README](https://github.com/thoren-d/tracing-chrome)) | Free | No (runs in the browser) |
| Honeycomb | Hosted; queries, heatmaps, BubbleUp ("what is different about the slow ones"). Free plan: up to 20 million events a month, 2 triggers, no SLOs; Pro from $150 a month ([pricing](https://www.honeycomb.io/pricing)); Pro events cost $3.00 per million from 1 July 2026 ([plan changes](https://docs.honeycomb.io/get-started/honeycomb/2026-pro-plan-changes)). Retention is not stated on the pricing page | Free at this volume | Yes: span fields (numbers and names) go to Honeycomb under the owner's key |
| Jaeger, Grafana Tempo, SigNoz | Self-hosted trace stores | Free, but a server to run | No, if run on the Mac; unverified for this note, listed for completeness |

**Recommendation for the owner:** `otel-desktop-viewer` for daily development, Honeycomb when he
wants to look across weeks of real use. Use a personal Honeycomb team created for HyperMe, kept
apart from any work account. Since spans carry no text, what reaches Honeycomb is timing, sizes,
languages and provider names: his own data, sent by his own choice, from a build only he runs.

Volume check: about 10 spans a dictation, 120 dictations an hour of talking (the cost estimate in
`default-config.toml`), a heavy day of 500 dictations, is about 5,000 events a day and 150,000 a
month: under 1% of Honeycomb's free plan.

## 4. How privacy-first apps handle telemetry for other users

| App | What it does | Default | What the user sees |
| --- | --- | --- | --- |
| Zed (editor) | Usage metrics and crash reports (minidumps), each with its own switch; server-side data only for hosted AI and collaboration ([docs](https://zed.dev/docs/telemetry)) | Not stated on the page | **A telemetry log**: Help → View Telemetry Log shows every event sent |
| Aptabase (analytics for Tauri and other apps) | Only events the app sends, plus OS, app version and locale; "no device IDs, cookies or fingerprinting" ([for Tauri](https://aptabase.com/for-tauri)). Server AGPL-3.0, SDKs MIT; self-hostable; cloud free up to 20,000 events a month ([repository](https://github.com/aptabase/aptabase)) | Up to the app | Up to the app |
| Sentry (crash and error reports) | Free developer plan: 1 user, 5,000 errors, 30-day lookback; Team from $26 a month ([pricing](https://sentry.io/pricing/?utm_category=ai)) | Up to the app | Up to the app |
| Wispr Flow (the bar for dictation) | A third-party teardown reports PostHog, Sentry, Segment and Datadog telemetry, and metadata uploads continuing with data sharing off ([teardown](https://www.wensenwu.com/thoughts/wispr-flow-investigation), cited in `docs/research/2026-11-24/01-dictation.md`; no company response) | On | Unclear |

Patterns worth copying, from the above and common open-source practice:

1. **Off by default, or nothing at all.** Privacy-led apps either send nothing or ask first.
   HyperMe's public promise is "no telemetry"; that is a feature against Wispr Flow (PITCH).
2. **Show exactly what would be sent.** Zed's telemetry log is the model: the user can read every
   byte. A preview before sending also matches HyperMe's own rule for anything that leaves the app.
3. **Anonymise by not collecting, not by hashing.** A hashed user id or device id is still a
   stable identifier (pseudonymous, not anonymous). Safer: no ids at all, coarse values only
   (app version, macOS major version, chip family, provider names, rounded latencies, error
   kinds), and no IP kept by whatever receives it.
4. **A diagnostic bundle the user attaches to an issue.** Common in developer tools ("copy system
   info", "generate a bug report"): the user chooses when, sees what, and sends it through a
   channel they already trust.

## 5. Recommendation for other users

### 5.1 Version 1: no telemetry, and good self-service diagnostics

1. **Same local traces as the owner.** Every user's Mac keeps its own trace store, under the
   history's retention and size limit, deleted with the history.
2. **Diagnostics view** in Settings or Home: median and p90 from key release to text, per stage
   and per provider, over the last day and week; recent errors by kind. A user with slow
   dictations can see "speech took 900 ms" and switch provider without filing anything.
3. **Report a problem**, in the menu and the Diagnostics view:
   - Builds a bundle on the Mac: app version and build, macOS version, chip, the config with
     every secret and personal value removed (providers, models, levels, switches), the last 50
     dictation traces (numbers only), the warnings and errors file, and HyperMe crash reports.
   - Shows the bundle as readable text before anything else happens.
   - **Save** writes it to a file; **Open issue** opens a prefilled GitHub issue (app version,
     macOS, a short summary of failures) in the browser, and the user attaches the file. The app
     itself sends nothing.
   - Scrubbing: no text field exists to scrub (section 3.1); the home folder becomes `~`;
     connector accounts become "Google account 1"; keys never enter the bundle. A test builds a
     bundle from fixtures holding text, emails and fake keys, and greps for them.
4. **Crashes:** a panic hook writes a local crash file (thread, backtrace symbols, app version;
   no field values), and the bundle includes it and any macOS `.ips` report named `HyperMe`.

This keeps every current promise, costs no service, and needs no account or key from anyone.

### 5.2 Later, if the owner chooses: opt-in anonymous signals

Only after the first release, and only as an explicit decision recorded in the PRD:

- **Opt-in crash reports** through Sentry's free plan or a self-hosted, Sentry-compatible server.
  Adds a third party and an SDK; the first-run question and a "what was sent" log are required.
- **Opt-in weekly summary**: one small payload a week (version, macOS major, chip family,
  provider names, p50 and p90 per stage, error counts), shown in full before the first send and
  in a log afterwards, sent to an endpoint the project runs (Aptabase self-hosted, or a tiny
  receiver). No ids, no IPs kept. This would also measure adoption better than stars (PRD
  section 10).

This is common practice in open-source desktop apps that collect anything at all (section 4),
and is on the roadmap's Later row (owner, 27 November 2026).

Both change the public line "no telemetry" to "nothing leaves your Mac unless you turn it on, and
you can see every byte". The recommendation is to start with 5.1 and decide on 5.2 with real
issues in hand.

## 6. Cost against the budgets

| Measure | Expected effect | How to check |
| --- | --- | --- |
| Idle memory (under 100 MB; 59.4 MB today) | `tracing` and a SQLite sink on the connection the history already opens: small. The OTLP exporter and its HTTP stack are not in the release | `scripts/measure-idle.sh` before and after; gate +2 MB |
| Idle CPU (near 0%) | No background work: spans are written when a dictation ends, retention runs with the history's | Same script |
| Key release to text (under 800 ms) | Span bookkeeping is microseconds; the store writes after the paste | Replay with flowbench before and after; gate +1 ms at p90 |
| Disk | About 10 rows of numbers per dictation; trivial next to the audio | Size of the history folder |
| Binary size and dependencies | Two to three small MIT crates in the release | `cargo tree`, `cargo deny check` |

## 7. Decisions

| ID | Question | Recommendation | Status |
| --- | --- | --- | --- |
| T-1 | Adopt `tracing` with a local trace store as the app's one instrumentation, and OpenTelemetry export as a developer-only feature | Yes: an M-sized change after the phase 1 gate's code is stable; it rebuilds R-30 on spans and adds stage bars to history items | Recommended; engineering choice within existing rules |
| T-2 | What other users' copies may send | No telemetry in version 1; a local Diagnostics view and a **Report a problem** bundle the user attaches to an issue. Opt-in signals (5.2) reconsidered after the first release | Decided 27 November 2026: as recommended (PRD Q-18) |
| T-3 | Where the owner looks at his own traces | `otel-desktop-viewer` locally; Honeycomb's free plan in a personal team for weeks of data | Recommended; his choice of tool |

## 8. Next steps

1. A spec (M, `AC-n` in the issue) for the trace store and the move of R-30 onto spans, with
   the privacy grep test and the budget gates of section 6.
2. A small spec for Diagnostics and Report a problem, with the bundle format and its scrub test.
3. A `docs/results/` entry with idle memory and replay latency before and after.
