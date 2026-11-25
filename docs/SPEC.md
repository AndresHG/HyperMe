# HyperMe spec

Source: analysis of 24 November 2026.
Full write-up: https://claude.ai/artifact/HP3aScXAUcUVD5h7ExLwTL
Mind map of the field: https://claude.ai/artifact/KG73DFinZohHHeVQn9VEAZ

Reviewed the same day against [RESEARCH.md](RESEARCH.md). The phases in section 1 follow
[ROADMAP.md](ROADMAP.md), adopted on 25 November 2026; section 2 still describes the full
architecture.

## 1. What it must do

| Capability | Detail | When |
| --- | --- | --- |
| Quick commands | "Create a ticket for this in GitHub, in this project"; short spoken questions | Phase 3 |
| Crons | Daily tasks, items pending for tomorrow, "meeting now" nudges | Phase 4 |
| Connectors | GitHub first, then calendar and email; Linear and Jira later | Phase 3 onward |
| Dictation | Hold a key, speak, get formatted text in any app, at Wispr Flow speed | Phases 1 and 2 |
| Meetings | Record, transcribe, notes and action points, push tasks out | Phase 5 |
| Coding agents | Start and follow tasks in Claude Code or Devin | Phase 5 |
| Local speech models | Optional, for offline use | Later |
| Linux and Windows | Same app | Later |

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
| Storage | SQLite with full-text search | Retrieval layer can be swapped later |
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

The roadmap is [ROADMAP.md](ROADMAP.md), adopted by the owner on 25 November 2026. It replaces
the earlier six-part plan that this section used to hold (command layer first, dictation
second). Each phase starts only after the gate of the one before it passes. Phase 1 is
specified in [specs/25-dictation-core](../specs/25-dictation-core/spec.md).

## 6. Risks

| Risk | Mitigation |
| --- | --- |
| Formatting quality falls short of Wispr Flow | Keep a test set of real dictations; iterate on the prompt; owner stays on Wispr until the Phase 2 gate |
| Provider and model churn | Provider trait; model names in config |
| macOS permissions and signing | Sign and notarise from the first shared build |
| Agent acts on text it reads | Previews, draft-only email, allowlists |
| Scope | Gates between phases |
