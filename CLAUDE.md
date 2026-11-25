# HyperMe

HyperMe is a lightweight, open-source copilot for your daily work on the desktop. One
always-on-top overlay that takes voice in and gets work done: quick commands ("create a ticket for
this in Linear"), crons, dictation, meeting notes, and starting and following tasks in
coding agents. Mac first; Linux and Windows later.

This is a personal agent, not a coding assistant. It hands coding work to Claude Code
or Devin and reports back.

## Names

- The program is **HyperMe**. Folder, repository and package names are lowercase: `hyperme`.
- The assistant is addressed as **"Hyper"** ("Hyper, create a ticket for this in Linear").
  Use that in UI copy, examples and any future wake word.
- Always write the full name HyperMe in public text; plain "Hyper" is used by many other apps.

## Read first

- `docs/SPEC.md`: requirements, architecture, budgets, safety rules, roadmap and gates
- `docs/SPEED_TEST.md`: Phase 0, the first thing to build
- `docs/ROADMAP.md`: proposed phases after Phase 0 and the gap to Wispr Flow (not yet adopted)
- `docs/FIELD.md`: existing projects and what to borrow from each
- `docs/RESEARCH.md`: verdict, product focus, pitch and field map from the research of
  24 November 2026
- `KICKOFF.md`: the first two sessions, step by step

## Current phase

**Phase 0: speed test.** It is the only committed work: the owner's gate is to show Wispr
Flow performance first. What follows is decided once its results are in. Working
assumption: dictation next, then the other parts as capabilities.
Do not start a phase before the gate of the previous one has passed. When a gate
passes, update this section and record the evidence in `docs/results/`.

## Decisions already made

- Rust core inside Tauri 2, with a thin web UI. The overlay is an NSPanel on macOS.
- Cloud speech and language-model APIs first. No local models in version 1.
- Every provider sits behind a trait. Model names live in config, never in code.
- Connectors go through MCP (one client, `rmcp`). No integration code per service.
- No agent framework runtime (OpenClaw, Hermes and similar). A small in-process
  tool-calling loop is enough.
- SQLite for storage. Secrets in the OS keychain, never in files, logs or commits.
- Dependencies must be MIT, Apache-2.0 or BSD. Never copy GPL code (VoiceInk,
  FluidVoice): read it for ideas only.
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
  telemetry. No audio kept on disk.
- Memory and learning ("knows you better every day") is part of the vision, not of a
  first version.
- This project's own tickets live in GitHub Projects.
- After dictation comes smart routing and actions; the first tracker connector is GitHub,
  not Linear (owner, 25 November 2026). See `docs/ROADMAP.md`.

## Budgets (targets to design against)

| Measure | Target |
| --- | --- |
| Idle memory | under 100 MB |
| Idle CPU | near 0% |
| Key-down to recording | under 50 ms |
| Key release to cleaned text | under 800 ms |

## Open decisions: ask the owner, do not assume

- Licence: MIT or Apache-2.0. Do not add a LICENSE file until this is decided.
- Mail and calendar provider: Google or Microsoft 365?
- Ticket creation: wait for Enter (default) or fire at once?
- Order of the parts after dictation, and who the first outside user is.
- Whether to adopt the two extra checks in `docs/RESEARCH.md` section 1.3 as gates.

## Working rules

- Check external API shapes against the provider's current docs before coding against
  them. Speech and model names changed more than once in 2026.
- API keys come from environment variables or the keychain. Never print, log or commit them.
- Measure before optimising. Write numbers to `docs/results/` with date, machine and
  network.
- Keep dependencies few. Say why each new crate is needed in the commit message.
- When borrowing code, record the source repository, commit and licence in `NOTICE.md`.
- `cargo fmt`, `cargo clippy -- -D warnings` and tests pass before every commit.
- Changed files end in a pull request into `master` with its `## Lessons` filled in: never
  a commit on `master`, never left uncommitted. An uncommitted plan was missing for the
  session that needed it, and a clean-context check found six errors (PR 1).
- Rename an auto-created `claude/*` branch before the first push. GitHub cannot re-point
  a pull request's head.
- A claim about the field or a competitor in `docs/` names its source or says it is
  unverified. The first analysis took its own field list as checked and its headline was
  wrong (PR 1).
- Small commits with clear messages. Open a GitHub issue for anything deferred.
- Work data (bank mail, bank meetings) is out of scope until the owner says otherwise.
