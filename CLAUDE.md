# HyperMe

HyperMe is a lightweight, open-source copilot for your daily work on the desktop. One
always-on-top overlay that takes voice in and gets work done ("voice to done"): dictation, quick
commands ("create a ticket for this in GitHub"), short answers, a daily task list, email drafts
and meeting notes. Mac first; Linux and Windows later.

This is a personal agent for daily work, not a coding assistant. It never does coding work
itself; a later version may hand a task to Claude Code ([docs/PRD.md](docs/PRD.md)).

## Names

- The program is **HyperMe**. Folder, repository and package names are lowercase: `hyperme`.
- The assistant is addressed as **"Hyper"** ("Hyper, create a ticket for this in GitHub").
  Use that in UI copy, examples and any future wake word.
- Always write the full name HyperMe in public text; plain "Hyper" is used by many other apps.

## Read first

- `docs/PRD.md`: what HyperMe is, why and for whom; users, features and priority, non-goals,
  success, and the single list of open questions. The source of truth for scope
- `docs/SPEC.md`: requirements, architecture, budgets and safety rules
- `docs/SPEED_TEST.md`: Phase 0, the first thing to build
- `docs/ROADMAP.md`: the phases and their gates, and the gap to Wispr Flow (adopted 25 November 2026)
- `docs/RELEASES.md`: version numbering, what each version (0.1 to 0.4) brings and when, and
  triage of new asks into versions (adopted 27 November 2026)
- `docs/plans/2026-11-26-phases-1-2.md`: phases 1 and 2 cut into chunks for parallel threads
- `docs/PITCH.md`: pitch, positioning against Wispr Flow, and the ledger of claims still to earn
- `specs/25-dictation-core/`: draft Phase 1 spec, design and slices (not yet approved)
- `specs/96-provider-settings/`: draft spec for choosing the speech and cleanup provider in Settings
- `specs/92-daily-tasks/`: draft phase 4 spec for the task list with dependencies, the morning card
  and the scheduler (core slices 1 to 6 approved to start, 26 November 2026)
- `specs/93-personal-dictionary/`: Phase 2 spec for the personal dictionary (approved 27 November 2026)
- `specs/151-voice-edits/`: phase 3 spec for voice edits of selected text ("Hyper, make this shorter";
  approved 27 November 2026, first work of phase 3)
- `specs/166-gmail-approvals/`: spec for approvals asked by connector servers (MCP form
  elicitation), first for Gmail (approved 27 November 2026)
- `specs/182-trace-store/`: phase 2 spec for local traces of every dictation (the `tracing`
  crate, a SQLite trace store, the timing log on spans; nothing leaves the Mac); implemented
- `specs/200-diagnostics/`: spec for the Diagnostics view in Settings, stage bars on History
  items, the Report a problem bundle the user previews and attaches to an issue, and local crash
  files (approved 27 November 2026)
- `docs/RELEASING.md`: how a release is built, signed, notarised and published, and how updates work
- `docs/FIELD.md`: existing projects and what to borrow from each
- `docs/research/2026-11-27-telemetry-and-tracing.md`: local tracing, what other users' copies may
  send, and backends (research; PRD Q-18 decided: no telemetry, a report bundle)
- `docs/UX.md`: UX and UI by phase (one shape at the bottom centre: dot, pill, card and quick sheet; the Home window; settings and menus), with mockups in `docs/ux/` (draft)
- `docs/RESEARCH.md`: verdict, product focus, pitch and field map from the research of
  24 November 2026
- `KICKOFF.md`: the first two sessions, step by step
- `CONTRIBUTING.md` and `specs/README.md`: how a change is sized, specified and tested

## Current phase

**Phase 1: dictation core.** The owner approved it on 26 November 2026 (`docs/PRD.md` Q-2); its
code is merged and its gate (a week without Wispr Flow, idle budgets) is under way. On the same
day the owner asked to start phase 2 work, and the design of the Google connector and daily tasks,
in parallel: `docs/plans/2026-11-26-phases-1-2.md` cuts that work into chunks. The first public
release still waits for the phase 1 and phase 2 gates. A week of daily use in a gate is an
important check, but development does not stop to wait for it (owner, 27 November 2026).
Otherwise, do not start a phase before the gate of the previous one has passed. When a gate
passes, update this section and record the evidence in `docs/results/`.

## Decisions already made

- Rust core inside Tauri 2, with a thin web UI. The overlay is an NSPanel on macOS.
- Cloud speech and language-model APIs first. No local models for dictation in version 1;
  meetings on device (below) are the decided exception.
- Every provider sits behind a trait. Model names live in config, never in code.
- Connectors go through MCP (one client, `rmcp`). No integration code per service.
- No agent framework runtime (OpenClaw, Hermes and similar). A small in-process
  tool-calling loop is enough.
- SQLite for storage. Secrets in the OS keychain, never in files, logs or commits.
- Dependencies must be MIT, Apache-2.0 or BSD. Never copy GPL code (VoiceInk,
  FluidVoice): read it for ideas only.
- MPL-2.0 is allowed only for the Tauri dependencies named in `deny.toml` (`option-ext`,
  `cssparser`, `cssparser-macros`, `selectors`, `dtoa-short`), unmodified; Zlib (`foldhash`) and
  ISC (`libloading`) likewise for those crates only (owner, 26 November 2026).
- Anything that creates, sends or posts outside the app shows a preview and waits for
  confirmation. Email is draft-only.
- Success means an open-source project other people install and use. What sets it apart
  is speed and lightness, with the full feature set (owner, 24 November 2026).
- The first gate is Wispr Flow performance, shown by a command-line tool on the owner's
  own utterances. No overlay is needed for it.
- Dictation is in Spanish and English, one language per dictation, detected
  automatically. Mixed-language sentences are not a requirement.
- Privacy boundary: audio, transcript, dictionary, app name and the text before the
  cursor may leave the machine, under the user's own keys. Window text or a screenshot
  only behind a visible switch that is off by default. Never password fields. No
  telemetry. Audio stays on the Mac: dictation history (audio and text) is on by default,
  kept 30 days or unlimited, under a size limit the user sets, so a dropped connection never
  loses a dictation (owner, 26 November 2026; this replaces "no audio kept on disk").
  The text the user selects may go to their own cleanup provider when they ask Hyper to edit it
  (owner, 27 November 2026, spec 151). Calendar and mail content from a connected account may go to the user's own language-model
  provider, only when a briefing, summary, triage or question needs it (owner, 26 November 2026).
- Memory ("Hyper, remember …", proposed facts approved by the user, kept in a folder of Markdown
  files on the user's Mac), self-learning facts, the graph and contexts come after 1.0. The near
  versions are dictation (0.1), voice commands and the to-do list (0.2), email (0.3), meetings
  (0.4) (owner, 27 November 2026; replaces "memory starts in phase 3"). See `docs/RELEASES.md` and
  `docs/PRD.md` section 7.5.
- This project's own tickets live in GitHub Projects.
- After dictation comes smart routing and actions; the first tracker connector is GitHub,
  not Linear (owner, 25 November 2026). See `docs/ROADMAP.md`.
- Licence: MIT OR Apache-2.0, dual, at the user's option (owner, 25 November 2026).
- No Apple Developer account for now: macOS releases ship unsigned (the local identity, not
  notarised), as VFS does, and Developer ID is decided later. Linux and Windows matter more than
  notarisation (owner, 27 November 2026). See `docs/RELEASING.md`.
- This repository is the one that goes public. It was recreated on 26 November 2026 with its
  history rewritten so that internal project names never appear; the private original is the
  archive (owner, 26 November 2026). See `docs/LAUNCH.md`.
- `docs/ROADMAP.md` is the adopted plan; it replaces the old phase table in `docs/SPEC.md`
  (owner, 25 November 2026).
- Code-of-conduct reports go through GitHub private reporting, like security reports; no
  personal email in the repository (owner, 25 November 2026).
- Public positioning leads with the full copilot ("the open-source voice copilot for your
  workday"), and says plainly that it starts with dictation. The name HyperMe stays for the
  public launch (owner, 25 November 2026). See `docs/PITCH.md`.
- Notes and contexts (owner, 27 November 2026): when a call is detected HyperMe asks "Transcribe?"
  every time and never records without a yes; tags are two-level contexts (Customers > Amazon),
  each a file in the memory folder; facts and to-dos link to meetings only through contexts. See
  `docs/ROADMAP.md`, "Notes, memory and contexts".
- Paid accounts (owner, 27 November 2026): no sign-in replaces the speech key. "Continue with ChatGPT"
  is the only plan sign-in; there is no Claude sign-in, so Claude subscribers use an Anthropic key, paid
  by their plan's monthly API credits on Max, Ultra and Team. Launching a Claude Code or Codex task by
  voice uses their own command line on the user's login, never an embedded interface; it stays in Later, after the first-wave commands. See
  `docs/research/2026-11-27-paid-accounts-and-agent-sessions.md`.
- Meetings run on device first: Apple's on-device recognizer for the live transcript, and a
  cleanup setting of cloud (Groq, default), local model, or off (owner, 26 November 2026). See
  `docs/ROADMAP.md`, "Meetings on device". Dictation stays on cloud speech.

## Budgets (targets to design against)

| Measure | Target |
| --- | --- |
| Idle memory | under 100 MB |
| Idle CPU | near 0% |
| Key-down to recording | under 50 ms |
| Key release to cleaned text | under 800 ms |

## Open decisions: ask the owner, do not assume

The single list is `docs/PRD.md` section 12. The interview of 26 November 2026 settled the
earlier ones here: Google first for mail and calendar, actions wait for Enter, daily tasks and
email before meetings, first users are people in tech companies (developers first), and the
research checks are dropped.

## Spec first, test first

- Size every change S, M or L before coding (`CONTRIBUTING.md`). L gets
  `specs/<issue>-<slug>/spec.md` with `AC-n` criteria, approved before production code; M keeps
  its `AC-n` in the issue; S starts from a failing test.
- Write the failing test before the code. Each test cites the `AC-n` it proves; a bug fix starts
  with a test that reproduces it.
- Tests never call a paid API or need a key: use local fakes and test servers.
- CI (`.github/workflows/`) runs fmt, clippy, tests, a full-history secret scan and the licence
  and advisory policy in `deny.toml`. A red check is fixed, never skipped or allowlisted to pass.
- Keep platform code behind a trait or a `cfg(target_os)` module, so Linux and Windows can follow
  macOS without touching the core.

## Working rules

- Check external API shapes against the provider's current docs before coding against
  them. Speech and model names changed more than once in 2026.
- API keys come from environment variables or the keychain. Never print, log or commit them.
- Measure before optimising. Write numbers to `docs/results/` with date, machine and
  network.
- Keep dependencies few. Say why each new crate is needed in the commit message.
- When borrowing code, record the source repository, commit and licence in `NOTICE.md`.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and tests pass before every commit.
- Changed files end in a pull request into `master` with its `## Lessons` filled in: never
  a commit on `master`, never left uncommitted. An uncommitted plan was missing for the
  session that needed it, and a clean-context check found six errors (PR 1).
- Rename an auto-created `claude/*` branch before the first push. GitHub cannot re-point
  a pull request's head.
- A claim about the field or a competitor in `docs/` names its source or says it is
  unverified. The first analysis took its own field list as checked and its headline was
  wrong (PR 1).
- Small commits with clear messages. Open a GitHub issue for anything deferred.
- Work data from the owner's employer is out of scope until the owner says otherwise.
- The repository is meant to be public. Never commit audio, transcripts, meeting notes, personal
  or family details, or anything naming the owner's employer, clients or other projects.
  `docs/results/` holds numbers only.
