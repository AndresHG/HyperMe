# The field: what exists and what to borrow

Checked on 24 November 2026. Re-check licences before copying any code, and record
what you take in `NOTICE.md`.

| Layer | Project | Stack, licence | What to take from it |
| --- | --- | --- | --- |
| Dictation | [Handy](https://github.com/cjpais/handy) | Rust + Tauri, MIT | Dictation core, recording pill, hotkey and paste handling on Mac, Windows and Linux. It transcribes after key release, not while speaking. Name and logo are not reusable. |
| Dictation | VoiceInk | Mac only, GPLv3 | Ideas only. No code. |
| Meetings | [Meetily](https://github.com/Zackriya-Solutions/meetily) | Rust + Tauri, MIT | Mixing microphone and system audio without a bot |
| Meetings | [Anarlog](https://github.com/fastrepl/anarlog) (was Hyprnote) | Tauri v2, MIT | Core Audio tap capture crates, plugin SDK, CLI that is also an MCP server. Enterprise parts have a separate licence. |
| Meetings | [Minutes](https://github.com/silverstein/minutes) | Rust, MIT | Meetings as Markdown files, exposed to agents over MCP |
| All-in-one | [OpenWhispr](https://github.com/OpenWhispr/openwhispr) | Electron, MIT | Feature checklist only. The owner found it slow and dislikes the UI. |
| All-in-one | [SpeakoFlow](https://github.com/AbhishekBarali/SpeakoFlow) | Tauri 2 + Rust, MIT | Closest existing code to the target, built on Handy's core. Young. |
| All-in-one | [Omi](https://github.com/BasedHardware/omi) | Swift, MIT | Floating bar design: push-to-talk, agent pills, proactive nudges |
| Agent | [Hermes Agent](https://hermes-agent.nousresearch.com/docs) | Python, MIT | Cron design, skills as files, memory |
| Agent | [OpenClaw](https://github.com/openclaw/openclaw) | TypeScript, MIT | Ideas on channels and skills. Too heavy to embed. |
| Agent | [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw), IronClaw | Rust, Apache-2.0 | Small agent runtimes; credential isolation |
| Agent | [goose](https://github.com/aaif-goose/goose) | Rust core, Apache-2.0 | MCP client usage (`rmcp`), recipes, scheduler |
| Overlay | [Thuki](https://github.com/quiet-node/thuki) | Tauri + Rust | NSPanel overlay above fullscreen apps |
| Overlay | [Clicky](https://github.com/farzaa/clicky), [Teachy](https://github.com/VedSoni-dev/teachy) | Swift | Companion beside the cursor |

## The reference for feel

Wispr Flow (closed, cloud). The owner uses it daily. Match its speed and its
formatting; its pill is the UX baseline for dictation.

## Building blocks

- Overlay: Tauri 2, `tauri-nspanel`
- Audio: `cpal` (microphone), `cidre` (Core Audio taps on macOS)
- Speech, cloud: OpenAI realtime transcription; Deepgram and ElevenLabs Scribe v2
  Realtime as alternatives
- Speech, local (later): `transcribe-rs` with Parakeet v3 (25 European languages)
- Connectors: `rmcp`; remote MCP servers for Linear, Atlassian, GitHub, Gmail, Calendar, Devin
- Coding agents: `claude -p` and the Agent SDK; Devin's MCP server; Agent Client
  Protocol (ACP) as a possible common layer later
