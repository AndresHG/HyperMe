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
- `docs/FIELD.md`: existing open-source projects and what to borrow from each
- `KICKOFF.md`: the first two sessions, step by step

## Current phase

**Phase 0: speed test.** Next is Phase 1 (command layer).
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

## Budgets (targets to design against)

| Measure | Target |
| --- | --- |
| Idle memory | under 100 MB |
| Idle CPU | near 0% |
| Key-down to recording | under 50 ms |
| Key release to cleaned text | under 800 ms |

## Open decisions: ask the owner, do not assume

- Licence: MIT or Apache-2.0. Do not add a LICENSE file until this is decided.
- Tracker for Phase 1: Linear is assumed. Jira or GitHub Projects from day one?
- Mail and calendar provider: Google or Microsoft 365?
- Ticket creation: wait for Enter (default) or fire at once?
- Dictation languages: Spanish and English are assumed, possibly mixed in one dictation.

## Working rules

- Check external API shapes against the provider's current docs before coding against
  them. Speech and model names changed more than once in 2026.
- API keys come from environment variables or the keychain. Never print, log or commit them.
- Measure before optimising. Write numbers to `docs/results/` with date, machine and
  network.
- Keep dependencies few. Say why each new crate is needed in the commit message.
- When borrowing code, record the source repository, commit and licence in `NOTICE.md`.
- `cargo fmt`, `cargo clippy -- -D warnings` and tests pass before every commit.
- Small commits with clear messages. Open a GitHub issue for anything deferred.
- Work data (bank mail, bank meetings) is out of scope until the owner says otherwise.
