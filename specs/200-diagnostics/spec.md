# Diagnostics and Report a problem

**Status:** approved by the owner, 27 November 2026 (DG-1 to DG-3 decided).
**Issue:** [#200](https://github.com/AndresHG/hyperme/issues/200). **Size:** L (a new Settings
section, a change to History, a bundle that a user may take off the Mac, crash capture).
**Traces to:** [PRD](../../docs/PRD.md) Q-18 (no telemetry; a Diagnostics view and a Report a
problem bundle; owner, 27 November 2026);
[telemetry research](../../docs/research/2026-11-27-telemetry-and-tracing.md) section 5.1;
[RELEASES](../../docs/RELEASES.md) 0.1 ("local traces and Report a problem");
spec [182](../182-trace-store/spec.md) (the trace store this reads, and its out-of-scope list);
spec [98](../98-home-window/spec.md) (Home and History); spec [96](../96-provider-settings/spec.md)
R-20 (a provider row's median latency); [UX](../../docs/UX.md) "The settings plan".

## Problem

Spec 182 gave every dictation a local trace, and HyperMe a warnings file. Nobody can see them yet:

- A user whose dictations feel slow cannot tell whether speech, cleanup or the paste is the slow
  part, or whether another provider would be faster. They would have to open SQLite.
- When something goes wrong on someone else's Mac, the project hears about it only through a
  GitHub issue, and the issue rarely says the version, the macOS, the providers or what failed.
  HyperMe sends no telemetry (PRD Q-18), so that issue is the only channel there is.
- A crash leaves nothing HyperMe can show: a Rust panic goes to standard error, which an app
  opened from Finder throws away.
- History shows what was said, but not why one dictation took two seconds.

## Goal

A user can answer "why was that slow, and what failed?" on their own Mac, and when they want help,
hand the project one file they have read in full. HyperMe itself still sends nothing.

## Words used here

- **Stage:** one of the timed parts of a dictation in a trace: microphone start, speech,
  dictionary, cleanup, insert, history write (spec 182 R-2).
- **Key release to text:** from releasing the key to the text sent for pasting; the budget is
  800 ms (`CLAUDE.md`).
- **Bundle:** the text file Report a problem builds.

## Requirements

### Diagnostics in Settings

- R-1. Settings shall have a **Diagnostics** section, after Privacy, shown once the trace store
  exists (UX: a section appears only once its feature exists).
- R-2. Diagnostics shall show key release to text, median and 90th percentile, over the last
  24 hours and the last 7 days, with the number of dictations each covers.
- R-3. Diagnostics shall show the median and 90th percentile of each stage over the last 7 days,
  speech and cleanup per provider and model, so a slow provider stands out next to another one
  used in the same week.
- R-4. Diagnostics shall show the outcomes of the last 7 days (pasted, copied, failed, cancelled,
  nothing heard) as counts, and the failures by kind (`error_kind` of spec 182 R-3).
- R-5. Diagnostics shall show the last 20 warnings and errors from the warnings file, newest
  first, as written (they are already scrubbed, spec 182 R-14).
- R-6. When there are fewer than 5 dictations in a window, Diagnostics shall say so in place of
  percentiles, rather than show a percentile of one value.
- R-7. When the trace store is off (Keep timing traces off, spec 182 R-9a, with history off) or
  empty, Diagnostics shall say so in one line and link to the switch.
- R-8. A number over its budget (key release to text over 800 ms at p90) shall be marked, with
  one line naming the slowest stage.
- R-9. Diagnostics shall read the store when the section opens and when the user presses
  Refresh. It shall do no background work (idle CPU near 0%).

### Stage bars in History

- R-10. When a History item is expanded, it shall show its trace as one bar per stage on a shared
  time axis from key-down, with each stage's duration in milliseconds, the providers and models,
  and the outcome.
- R-11. An item without a trace (traces off, or older than the trace's retention) shall show
  nothing extra.

### Report a problem

- R-12. **Report a problem…** shall be reachable from Diagnostics, from Help at the bottom of the
  Home sidebar, and from the failure line of a failed dictation in History. It shall not be added
  to the menu-bar menu (UX: at most 8 rows, actions used daily).
- R-13. When the user chooses Report a problem, HyperMe shall build the bundle on the Mac and show
  it in full as readable text before anything else happens.
- R-14. The bundle shall hold:
  - HyperMe's version and build, macOS version, chip family and memory size;
  - the settings from an allowlist (R-17);
  - Diagnostics' numbers (R-2 to R-4);
  - the last 50 traces, numbers and names only (spec 182 R-5);
  - the last 200 lines of the warnings file;
  - HyperMe's crash files from the last 30 days (R-21 to R-23);
  - an optional description the user types, shown in the preview like the rest.
- R-15. The preview shall offer **Save…**, which writes the bundle to a file the user picks, and
  **Open issue**, which saves the bundle, opens a new GitHub issue in the browser with a title and
  a body prefilled with the issue context (R-15a), and shows the saved file in Finder so the user
  can attach it. **Cancel** discards it.
- R-15a. The prefilled body shall hold the version and build, macOS, chip and memory, the
  allowlisted settings (R-17), Diagnostics' numbers per stage, and the stage timings and outcome
  codes of the last traces (the failed item's trace first when the report starts from one), newest
  first, until the issue URL reaches 6,000 characters. It shall hold only numbers, names from the
  allowlist and outcome codes: warning lines, crash files and the user's description go in the
  attached file only, since they are free text. The browser shows the body before the user
  submits it.
- R-16. HyperMe shall send nothing itself: no request to GitHub or anywhere else is made by the
  app. Opening the issue page is a link the browser opens; the bundle travels only if the user
  attaches it.
- R-17. Settings shall enter the bundle only from an allowlist: provider names, model names,
  cleanup level, switches (on or off), numbers (deadlines, limits, retention), the hotkey, and for
  each endpoint only whether it is the default or custom. Anything else is left out, so a new
  setting is out of the bundle until it is added to the list.
- R-18. The bundle shall hold no key or token, no dictated or edited text, no dictionary entry,
  no app or window name, no email address, no account name, no file path outside the bundle's own
  folder names, and no network address of a custom endpoint. Connector accounts appear as their
  service and a number ("Google, account 1"). The home folder becomes `~`.
- R-19. The bundle shall be one UTF-8 text file of at most 1 MB, trimming the oldest traces,
  warnings and crash files first, and saying what was trimmed.
- R-20. Building the bundle shall take under 1 second on the owner's Mac with a full trace store.

### Crashes

- R-21. HyperMe shall install a panic hook that writes a crash file to
  `~/Library/Logs/HyperMe/crashes/` with the time, version, thread name, panic location and a
  symbolised backtrace, then lets the panic go on as before.
- R-22. The panic message shall pass through the warnings file's scrub (spec 182 R-14) and be cut
  to 200 characters. No variable values other than the message are written.
- R-23. HyperMe shall keep at most 10 crash files and none older than 30 days.
- R-24. When macOS has written its own crash reports for HyperMe (`~/Library/Logs/
  DiagnosticReports/hyperme*.ips`), the bundle shall include the newest two from the last 30 days,
  scrubbed of the home folder, each cut to 64 KB, and the preview shall show them like the rest,
  with a switch "Include macOS crash reports", on by default, that takes them out of the bundle.
- R-25. After a crash, the next start shall show "HyperMe quit unexpectedly · Report a problem"
  in the menu's status row until the user opens it or dismisses it, and only once per crash.

## Acceptance criteria

- AC-1 *(fake)*: With a fixture store of known durations, Diagnostics' numbers match a
  hand-computed table: key release to text p50 and p90 over 24 hours and 7 days, per stage, per
  provider and model, outcome counts and failure kinds. (R-2 to R-4)
- AC-2 *(fake)*: With 4 dictations in a window, the window says there are too few; with traces off
  or an empty store, the one-line notice shows. (R-6, R-7)
- AC-3 *(fake)*: A fixture with p90 at 950 ms marks the number and names the slowest stage.
  (R-8)
- AC-4 *(fake)*: An expanded History item with a trace shows one bar per stage with offsets and
  durations from the trace; one without a trace shows none. (R-10, R-11)
- AC-5 *(fake)*: Privacy grep. Fixtures with a transcript, an edited text, a dictionary entry, an
  app name, an email address, a connector account, a custom endpoint URL, a home-folder path, a
  panic message holding a key and a fake key in the keychain fake go through dictations, a
  failure, a crash file and a bundle; the bundle contains none of them, and contains
  "Google, account 1" and `~`. (R-17, R-18, R-22)
- AC-6 *(fake)*: A new setting added to the config fixture does not appear in the bundle.
  (R-17)
- AC-7 *(fake)*: Report a problem makes no network request: the bundle is built, saved and the
  issue link opened with every outbound call recorded by a fake, and none is made by the app.
  The issue link's body holds the context of R-15a, stays under 6,000 characters with 10,000
  traces, and holds no warning line, crash text or description; a report started from a failed
  item puts that item's trace first. (R-15, R-15a, R-16)
- AC-8 *(fake)*: A store with 10,000 traces and a 5 MB warnings file give a bundle under 1 MB
  that says what was trimmed. (R-19)
- AC-9 *(fake)*: A panic in a test thread writes one crash file with location and backtrace, a
  scrubbed message under 200 characters, and the panic still unwinds; with 12 crash files, the
  oldest 2 are deleted. (R-21 to R-23)
- AC-10 *(fake)*: `.ips` fixtures named for HyperMe and for another app: only HyperMe's newest
  two from 30 days enter the bundle, scrubbed and cut to 64 KB; with the switch off, none do.
  (R-24)
- AC-11 *(live)*: On the owner's Mac with 30 days of traces, opening Diagnostics shows its numbers
  in under 300 ms, building the bundle takes under 1 s, and idle memory and CPU are unchanged with
  Settings closed. (R-9, R-20)
- AC-12 *(live)*: The owner builds a bundle, reads it in the preview, and opens an issue with it
  on the repository; the issue holds the prefilled lines and the attached file, and nothing else
  was sent. (R-13 to R-16)

## Non-goals

- Any telemetry, crash upload, or "send automatically" option: PRD Q-18. The opt-in signals of
  the research (section 5.2) are on the roadmap's Later row and need their own decision.
- Median latency on Home and in provider rows (spec 96 R-20, spec 98): they can read the same
  query (design.md) in their own change.
- Charts over time, or comparing two weeks: a table and stage bars are enough for "why was that
  slow".
- Traces of commands, voice edits and syncs: they get spans with their own specs; Diagnostics
  shows what the store holds.
- Linux and Windows: the view and the bundle are portable; crash and `.ips` paths are macOS
  modules behind a trait.

## Pre-mortem

It is three months later and this failed. Why?

| Failure | Answer in this spec |
| --- | --- |
| A bundle attached to a public issue leaked someone's email, a customer's name in an app title, or a key | Allowlist, not a blocklist (R-17); what cannot be scrubbed is never collected (R-18); the privacy grep (AC-5) and the new-setting test (AC-6); the user reads all of it first (R-13) |
| Users never found Report a problem, and issues still said "it doesn't work" | Three ways in, including the failed item itself (R-12); the crash notice points to it (R-25); the issue template asks for the bundle (design.md) |
| Diagnostics showed confident numbers from two dictations | Too few is said, not computed (R-6) |
| Diagnostics made HyperMe heavier at idle | No background work; reads on open and Refresh (R-9); AC-11 checks idle |
| A crash file held the transcript through a panic message | The message is scrubbed and cut, nothing else is written (R-22, AC-5) |
| The bundle grew past what GitHub accepts as an attachment | 1 MB cap with trimming (R-19); GitHub allows files up to 25 MB on issues ([docs](https://docs.github.com/en/get-started/writing-on-github/working-with-advanced-formatting/attaching-files), not rechecked for this draft) |
| "Open issue" quietly became an API call that needed a token | R-16: a browser link only; AC-7 records every outbound call |

## Decisions

| ID | Question | Recommendation | Status |
| --- | --- | --- | --- |
| DG-1 | Where Diagnostics lives | A Settings section after Privacy, not a Home section: it is looked at rarely, and Home keeps five sections at most (UX). Stage bars go on History items, where a slow dictation is noticed | Decided: Settings (owner, 27 November 2026) |
| DG-2 | What "Open issue" prefills | Title "Problem report: HyperMe <version>", and a body with version, macOS, outcome counts and the issue template's questions. Never traces or warnings in the URL: the user attaches the file | Decided otherwise: prefill all the context that is not private, traces included as numbers (R-15a); free text stays in the file (owner, 27 November 2026) |
| DG-3 | Whether to include macOS `.ips` crash reports | Yes, the newest two, scrubbed and cut (R-24): a native crash (WebKit, a system framework) leaves no Rust panic file, and the `.ips` is the only record | Decided: include them, with a switch in the preview to leave them out (R-24); the owner left the choice open (27 November 2026) |
