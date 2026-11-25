# HyperMe spec

Source: analysis of 24 November 2026.
Full write-up: https://claude.ai/artifact/HP3aScXAUcUVD5h7ExLwTL
Mind map of the field: https://claude.ai/artifact/KG73DFinZohHHeVQn9VEAZ

Reviewed the same day against [RESEARCH.md](RESEARCH.md). Section 5 carries the change;
sections 1 and 2 still describe the original six-part plan.

## 1. What it must do

| Capability | Detail | When |
| --- | --- | --- |
| Quick commands | "Create a ticket for this in Linear, in this project"; short spoken questions | Phase 1 |
| Crons | Daily tasks, items pending for tomorrow, "meeting now" nudges | Phase 1 |
| Connectors | Linear and calendar first; then email, Jira, GitHub Projects | Phase 1 onward |
| Dictation | Hold a key, speak, get formatted text in any app, at Wispr Flow speed | Phase 2 |
| Meetings | Record, transcribe, notes and action points, push tasks out | Phase 3 |
| Coding agents | Start and follow tasks in Claude Code or Devin | Phase 3 |
| Local speech models | Optional, for offline use | Phase 4 |
| Linux and Windows | Same app | Phase 4 |

Across all of it: very low idle footprint, instant response, cloud APIs are acceptable,
permissive open-source licence.

## 2. Architecture

One Rust process owns audio, hotkeys, scheduling and tools. A thin web view draws the
overlay. Models and connectors are remote.

```
+--------------------------------------------------------------------+
| Overlay (web view): pill, answer card, settings. No logic.         |
+--------------------------------------------------------------------+
                               ^  events / commands
                               v
+--------------------------------------------------------------------+
| Rust core (one process)                                            |
|  Hotkeys + paste | Audio capture | Scheduler      | Store (SQLite) |
|  Speech client   | Cleanup +     | MCP client     | Agent runner   |
|                  | command loop  |                |                |
+--------|-----------------|----------------|----------------|-------+
         v                 v                v                v
   Speech API        Language model    MCP servers     Coding agents
   (streaming)       (small, no        (Linear, Gmail, (Claude Code
                      reasoning)        GitHub, ...)    local, Devin)
```

| Component | Choice | Why |
| --- | --- | --- |
| Shell and overlay | Tauri 2; NSPanel on macOS via `tauri-nspanel` | Small footprint, floats over fullscreen apps, same code reaches Linux and Windows |
| Hotkeys and text insertion | Global shortcuts; clipboard paste | What Handy ships on three operating systems |
| Audio in | `cpal` for the microphone; Core Audio taps (`cidre`, or `cpal` 0.18 and later) for system audio on macOS | Taps need their own audio-capture permission, not Screen Recording |
| Speech | Trait with providers: streaming cloud first, file upload fallback, local Parakeet later | Swap provider after the speed test without touching the rest |
| Cleanup | Small model, reasoning off, streamed; prompt carries the personal dictionary and the active app name | This is where dictation quality is won |
| Command mode | Second hotkey; transcript goes to a tool-calling model | Voice in, action out |
| Tools | One MCP client (`rmcp`) to remote MCP servers | No per-service code |
| Coding agents | Claude Code through `claude -p` or the Agent SDK; Devin through its MCP server | Start, poll, surface the result in the overlay |
| Scheduler | Cron jobs in SQLite; each job is a self-contained prompt plus a tool allowlist | Jobs run with no memory of earlier chats |
| Storage | SQLite with full-text search | Retrieval layer can be swapped later (VFS) |
| Secrets | OS keychain | Keys never sit in config files |

### Why Wispr Flow is fast (and what to copy)

It is cloud-only with a budget of under 700 ms from end of speech to text. The speed
comes from wiring, not from the client language:

| Step | Slow wiring | Fast wiring (do this) |
| --- | --- | --- |
| Audio | Upload the whole file after key release | Stream small chunks over a WebSocket while speaking |
| Recognition | Starts after upload | Runs during speech; only the tail remains at release |
| Cleanup | General model, reasoning on, wait for full reply | Small model, reasoning off, fast inference, streamed |
| Connections | New TLS handshake per request | Opened at key-down, kept warm |

## 3. Budgets

| Measure | Target |
| --- | --- |
| Idle memory | under 100 MB |
| Idle CPU | near 0% |
| Key-down to recording | under 50 ms |
| Key release to final transcript | 150 to 300 ms |
| Key release to first cleaned word | 300 to 600 ms |
| Key release to full cleaned text (about 100 tokens) | under 800 ms |

These are targets and estimates. Phase 0 replaces the latency rows with measurements.

## 4. Safety rules

- Anything that creates, sends or posts outside the app shows a preview and waits for
  Enter. This can be relaxed per tool later.
- Email is draft-only.
- Text read from email, tickets or web pages is data, never instructions to the agent.
- Each cron job has an explicit tool allowlist.
- Work and personal profiles keep separate connectors and storage.
- Meeting capture always shows a visible indicator.

## 5. Roadmap and gates

**Under review since 24 November 2026.** A replacement is proposed in [ROADMAP.md](ROADMAP.md). Only Phase 0 is committed. The owner's gate is to
show Wispr Flow performance first, so dictation moves ahead of the command layer and the
other parts follow as capabilities, in an order decided after Phase 0. Research found
crons and nudges to be the most crowded part, and meeting capture a commodity best read
from an existing notetaker: see [RESEARCH.md](RESEARCH.md). The table below is the earlier
plan, kept until it is rewritten.

| Phase | Scope | Estimate | Gate to pass before the next phase |
| --- | --- | --- | --- |
| 0. Speed test | CLI that measures upload against streaming on 50 of the owner's dictations | 1 to 2 days | Median key release to cleaned text at or under 800 ms, 90th percentile at or under 1.2 s, formatting accepted beside Wispr Flow |
| 1. Command layer | Overlay pill, command hotkey, Linear and calendar over MCP, crons | 2 to 3 weeks | Owner uses it every day for two weeks |
| 2. Dictation | Streaming speech, cleanup prompt, personal dictionary, tone per app | 3 to 4 weeks | A full week without opening Wispr Flow |
| 3. Meetings and coding agents | Capture, notes, action points to Linear; Claude Code and Devin tasks | 4 to 6 weeks | First public open-source release |
| 4. Later | Local speech models, Linux, Windows, Jira, GitHub Projects | open-ended | |

Estimates assume part-time work with Claude Code. If Phase 0 fails on every variant,
Phase 2 is dropped and Wispr Flow stays for dictation.

### Phase 1 milestones

1. Tauri 2 app with a floating, non-activating pill that stays above fullscreen apps.
   No Dock icon; menu bar item instead.
2. Global hotkey: hold to record, release to stop. Recording state shown in the pill.
3. Streaming transcription of the command; transcript shown in the pill.
4. Tool-calling loop with one MCP server (Linear): "create a ticket ..." produces a
   preview card; Enter confirms, Esc cancels.
5. OAuth and keychain storage for the connector; a minimal connectors page.
6. SQLite store: command history and cron jobs.
7. Scheduler: create, list, pause and delete jobs by voice or in settings; results
   appear as nudges in the pill.
8. Calendar connector and the "meeting now" nudge.
9. Measure idle memory and CPU against the budgets; fix before closing the phase.

## 6. Risks

| Risk | Mitigation |
| --- | --- |
| Formatting quality falls short of Wispr Flow | Keep a test set of real dictations; iterate on the prompt; owner stays on Wispr until the Phase 2 gate |
| Provider and model churn | Provider trait; model names in config |
| macOS permissions and signing | Sign and notarise from the first shared build |
| Agent acts on text it reads | Previews, draft-only email, allowlists |
| Scope | Gates between phases |
