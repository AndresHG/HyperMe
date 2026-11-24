# The field: what exists and what to borrow

Checked on 24 November 2026, second pass. Sources and detail are in
[RESEARCH.md](RESEARCH.md) and [research/2026-11-24/](research/2026-11-24/). Star counts
are from the GitHub API on that date. Re-check licences before copying any code, and
record what you take in `NOTICE.md`.

## Open source

| Layer | Project | Stack, licence | State | What to take from it |
| --- | --- | --- | --- | --- |
| Dictation | [Handy](https://github.com/cjpais/Handy) | Rust + Tauri 2, MIT | 40,050 stars, 193 contributors, a release every 1 to 3 weeks. Streams with local models since v0.9.0. No cloud speech | The base for the dictation core: `handy-keys`, `handy-recorder`, clipboard and paste code, the NSPanel recording pill. Replace its local transcription manager. The first pass noted that its name and logo are not reusable; not re-checked. |
| Dictation | [SpeakoFlow](https://github.com/AbhishekBarali/SpeakoFlow) | Tauri 2 + Rust, MIT | 407 stars, 6 contributors, built on Handy's core | Worked example of WebSocket streaming to cloud speech, with keys in the keychain |
| Dictation | [OpenTypeless](https://github.com/tover0314-w/opentypeless) | Rust + Tauri, MIT | 703 stars | App detection with tone per app; connection warm-up |
| Dictation | [Blurt](https://github.com/AssemblyAI/blurt) | Swift, MIT | 78.0% zero-edit rate on the one cross-vendor benchmark, against Wispr Flow's 79.3% and Aqua's 80.5% | Design only: pre-warm on key-down, upload while recording, narrow context policy |
| Dictation | [FreeFlow](https://github.com/zachlatta/freeflow) | Swift, MIT | 3,155 stars | Its published cleanup prompt |
| Dictation | VoiceInk, FluidVoice | Swift, GPL-3.0 | | Ideas only. No code. |
| All-in-one | [OpenWhispr](https://github.com/OpenWhispr/openwhispr) | Electron, MIT | 12,161 stars, five active committers, weekly releases. Dictation, voice-command hotkey, meetings with calendar, notes, API, MCP server, own API keys | The closest existing product to the whole plan, and the baseline to beat on speed and memory. The owner found an earlier version slow and dislikes the UI. |
| All-in-one | [Omi](https://github.com/BasedHardware/omi) desktop | Swift, MIT | 14,020 stars | Closest in shape: floating bar, push-to-talk, voice typing, agent pills, MCP client, nudges. Ideas translated from Swift: panel settings, the rule that only an explicit request takes focus, one push-to-talk state machine, pill states |
| Meetings | [Minutes](https://github.com/silverstein/minutes) | Rust core, Swift capture helper, TypeScript MCP server, MIT | 1,618 stars, eight months old | Meeting files with structured action items and a "restricted" flag. A meeting layer to read from |
| Meetings | [Anarlog](https://github.com/fastrepl/anarlog) (was Hyprnote) | Tauri v2, MIT outside `enterprise/` | 9,984 stars. The team's main product is now "char" | The "proposal stays pending until a human applies it" pattern. Capture crates only if capture is ever built; check each `PROVENANCE.md` |
| Meetings | [Meetily](https://github.com/Zackriya-Solutions/meetily) | Rust + Tauri, MIT community edition | 33,018 stars. Speaker labels, MCP, CLI and webhooks are in the closed PRO edition | The smallest readable `cidre` capture example |
| Agent | [goose](https://github.com/aaif-goose/goose) | Rust core, Apache-2.0 | 56,942 stars, now under the Linux Foundation | `rmcp` 3.x usage, recipe format, scheduler design. Not its runtime |
| Agent | [OpenClaw](https://github.com/openclaw/openclaw) | TypeScript, MIT | 395,155 stars. Has a Mac menu-bar app with voice wake | Skills as files, automation model. Too heavy to embed. |
| Agent | [Hermes Agent](https://github.com/NousResearch/hermes-agent) | Python, MIT | 265,870 stars. Has an Electron desktop app | Cron design, skills as files, memory |
| Agent | [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw) | Rust, Apache-2.0 | 33,606 stars | Approval gates, tool receipts |
| Agent | [IronClaw](https://github.com/nearai/ironclaw) | Rust, Apache-2.0 | Main branch idle since 10 September 2026 | Credential isolation, as a design reference |
| Overlay | [Thuki](https://github.com/quiet-node/thuki) | Tauri 2 + Rust, Apache-2.0 | 584 stars, maintenance mode, local models only | NSPanel above fullscreen apps, double-tap activator. Fork and own. |
| Coding agents | [Happy](https://github.com/slopus/happy) | MIT | 24,788 stars | Session model for several agents (Claude, Codex, Grok) |

Dropped since the first pass: Clicky (open repository frozen, product now closed) and
Teachy (4 stars).

**Do not copy code from:** VoiceInk and FluidVoice (GPL-3.0); Epicenter/Whispering,
OpenLess, Tambourine, VoiceTypr, Claude Squad and opcode (AGPL-3.0); screenpipe (commercial,
source-available).

## Closed products to measure against

| Product | What it is now | What it leaves open |
| --- | --- | --- |
| Wispr Flow | Dictation, bot-less notetaker, an MCP server for its meetings and notes, "Ask Wispr" drafts, own speech models. $12 to $15 a month | Linux, open source, own keys, local mode, scheduled jobs, agent dispatch. Its Command Mode takes no action in other apps |
| Claude desktop with Claude Code | Quick-entry overlay with dictation, connectors, local and cloud scheduled tasks, background coding sessions | No pill that shows state. Dictation only into Claude. Claude Code only |
| ChatGPT desktop | Chat, Work and Codex in one app, voice that drives them, scheduled tasks, live dictation | No overlay documented. Dictation only into ChatGPT. Codex only |
| Granola | Bot-less notes, MCP and API, free dictation since October 2026. Free, or $14 a month for Business | No push to a task tracker |
| Raycast Pro | Hotkey palette, dictation, "Dictate to AI", MCP client, own keys. $10 a month | Scheduling, nudges, coding agents, meeting notes |
| Circleback | Meeting notes with assigned action items sent to Linear and others | The closest product to "meeting ends, tasks appear". Jira is not listed |

## The reference for feel

Wispr Flow (closed, cloud). The owner uses it daily. Match its speed and its
formatting; its pill is the UX baseline for dictation.

## Building blocks

- Overlay: Tauri 2, [`tauri-nspanel`](https://github.com/ahkohd/tauri-nspanel)
  (Apache-2.0; a git dependency, so pin a commit)
- Audio: `cpal` for the microphone, and for macOS loopback from 0.18 (added in 0.17,
  silent captures fixed in 0.18); `cidre` for Core
  Audio taps. Taps need the `NSAudioCaptureUsageDescription` permission
- Speech, cloud: the candidates and prices are in [SPEED_TEST.md](SPEED_TEST.md)
- Speech, local (later): `transcribe-rs` with Parakeet v3 (25 European languages). From
  the first pass; not re-checked
- Connectors: `rmcp` 3.x, pinned (three major versions in five months). Linear works
  with dynamic registration or an API key; GitHub with a personal access token. Google
  Workspace is a preview that needs your own Cloud project; Slack bars unlisted apps;
  Microsoft 365 is tenant-gated
- Coding agents: `claude --bg` and `claude agents --json` for background sessions, and
  the Agent SDK; Devin through its MCP server or v3 API (it is not an ACP agent); Agent
  Client Protocol (ACP) for local agents
