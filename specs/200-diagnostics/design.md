# Diagnostics and Report a problem: design

How [spec.md](spec.md) fits the code that exists after spec 182.

## Where the parts live

| Part | Crate | What it does |
| --- | --- | --- |
| `diagnostics.rs` | `hyperme-core` | Pure: turns stored traces into the Diagnostics numbers (key release to text, stages, outcomes, failure kinds, too few, over budget). No SQLite, no clock: the caller passes traces and "now" |
| `trace_store.rs` (extended) | `hyperme-platform` | Two new reads on the store's reader connection: `traces_since(since_ms, limit)` (traces with their spans, newest first) and `outcome_counts(since_ms)`. `stage_percentiles` (spec 182 R-10) is reused |
| `report.rs` | `hyperme-core` | Pure: builds the bundle text from parts (system facts, allowlisted settings, numbers, traces, warning lines, crash texts) and applies the size cap. The allowlist lives here |
| `crash.rs` | `hyperme-platform` | The panic hook (writes, prunes), and on macOS the `.ips` finder. Behind a `CrashFiles` trait so Linux and Windows add their paths later |
| `diagnostics.rs` | `app/src-tauri` | Tauri commands: `diagnostics_view`, `history_trace(id)`, `report_build`, `report_save`, `report_open_issue`. Opens the browser and reveals the saved file in Finder with `open`, as `settings.rs` and `connectors.rs` already do, behind a small opener trait so tests record the calls |
| `settings.html` | `app/ui` | The Diagnostics section, and the preview sheet |
| `home.html` | `app/ui` | Stage bars in an expanded History item; Report a problem under Help and on failed items |

The core stays free of platform calls and network, as now: everything the bundle reads is handed
to `report.rs` as plain values.

## Diagnostics numbers

- **Key release to text** is not a span of its own. It is the end of `insert` minus the end of
  `capture` within one trace, the same arithmetic `TimingLine::from_trace` already does for the
  timing log's columns. Only traces with outcome pasted or copied count.
- Percentiles: nearest rank, as `stage_percentiles` does, so the two never disagree.
- "Too few" below 5 values (R-6); "over budget" when p90 of key release to text is over 800 ms
  (R-8), naming the stage with the largest p90 share.
- One read for the view: `traces_since(now − 7 days, 5,000)`, then everything is computed in
  memory. 5,000 traces of about 10 spans is well under a megabyte and a few milliseconds; AC-11
  checks it live.

## The bundle

Plain text, sections in this order, so a person reads it top to bottom and a maintainer can grep
it:

```
HyperMe problem report
Created: 2026-12-03 10:41 (local time zone offset only)

== Description (written by you) ==
== This Mac ==            version, build, macOS, chip family, memory
== Settings ==            allowlisted keys, one per line: dictation.hotkey = fn
== Last 7 days ==         the Diagnostics numbers
== Last 50 dictations ==  one line per trace: time offset, outcome, error kind, stages in ms, providers
== Warnings and errors == last 200 lines
== Crashes ==             each crash file, then up to two .ips excerpts
== Trimmed ==             what the size cap left out, if anything
```

- **Allowlist** (R-17): a `const` table of config paths and how each is shown (`as_is`,
  `on_off`, `default_or_custom`). The test for AC-6 adds an unknown key to a config fixture and
  checks it is absent.
- **Accounts**: the connector settings give accounts in order; the bundle numbers them per
  service. No account string reaches `report.rs`: the app passes counts.
- **Scrub as a second net**: every free-text line (warnings, crash message, `.ips`, the user's
  description) passes through `warnings::scrub` with the home folder. The user's own description
  is scrubbed too, and the preview shows the result, so they see what will be in the file.
- **Times**: offsets from the bundle's creation ("−2 h 13 min"), not wall-clock times, so a
  bundle does not show when someone works. The creation line keeps the date.

## Open issue

`https://github.com/AndresHG/hyperme/issues/new?template=bug_report.yml&title=…&what=…`, opened in
the default browser. GitHub issue forms take field values from the query string by field id
([docs](https://docs.github.com/en/communities/using-templates-to-encourage-useful-issues-and-pull-requests/syntax-for-issue-forms),
not rechecked for this draft). The `what` field gets version, macOS and outcome counts. The
repository's `bug_report.yml` gains a short line: "If HyperMe is running, use Report a problem in
Settings → Diagnostics and attach the file." The URL is a constant in the app, checked by a test
against the repository in `Cargo.toml`.

## Crash files

- The hook wraps the default one: write the file, then call the previous hook, so standard error
  and unwinding are unchanged (AC-9).
- `std::backtrace::Backtrace::force_capture()`; frames only, with file and line where the release
  build has them. No locals exist in a backtrace, so the only text risk is the message (R-22).
- File name `crash-<unix seconds>.txt`; pruning runs at start-up, not in the hook.
- The crash notice (R-25) is a marker file the hook writes beside the crash; the next start reads
  and removes it.

## Failure modes

| Failure | Behaviour |
| --- | --- |
| Trace store missing or locked | Diagnostics shows the R-7 line; the bundle says "no traces" |
| Warnings file missing | Section says "none" |
| Panic inside the panic hook (disk full) | The hook ignores its own write errors; the previous hook still runs |
| The user saves into a folder they cannot write | The save dialog's error; nothing else happens |
| No default browser | "Open issue" shows the URL to copy |

## Budgets

| Measure | Effect | Check |
| --- | --- | --- |
| Idle memory and CPU | None while Settings is closed: no timers, no background reads | AC-11, `scripts/measure-idle.sh` |
| Key release to text | None: nothing in the dictation path changes; the panic hook costs nothing until a panic | Spec 182's overhead test still passes |
| Binary size | No new crates expected: SQLite, serde_json and the opener are in the tree | `cargo tree`, `cargo deny check` |

## Test plan

- Core: `diagnostics.rs` and `report.rs` with fixture traces and settings (AC-1 to AC-3, AC-5,
  AC-6, AC-8).
- Platform: the new store reads against a temporary database; the panic hook in a spawned thread
  with a temporary folder (AC-9); `.ips` fixtures (AC-10).
- App: the commands with a fake opener and a recording network fake (AC-7); History stage bars
  from a fixture trace (AC-4).
- Live: AC-11 and AC-12 on the owner's Mac, numbers to `docs/results/`.
