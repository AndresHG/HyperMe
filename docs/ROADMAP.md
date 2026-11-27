# HyperMe roadmap

**Status: adopted by the owner on 25 November 2026.** What and why: [PRD.md](PRD.md); this file says when. It replaces the earlier phase table in
[SPEC.md](SPEC.md) section 5. Phase 0 is still the only work under way. Each later phase starts only after the gate of the one before it passes
(`CLAUDE.md`).

## Where we stand against Wispr Flow

The owner's target is Wispr Flow's performance. Measured so far, on the owner's Mac:

| Measure | Wispr Flow | HyperMe today (`flowbench`, CLI only) | Gap |
| --- | --- | --- | --- |
| Key release to cleaned text | Published budget under 700 ms, a p99 target ([Wispr, Sept 2025](https://wisprflow.ai/post/technical-challenges)). Not yet timed on the owner's Mac ([#5](https://github.com/AndresHG/hyperme/issues/5)) | Median 426 ms, p90 587 ms: Speechmatics `melia-1` EU plus Groq `gpt-oss-20b`, prompt `v1`, wired, 38 of 50 samples ([results](results/2026-11-25-eu-speech.md)) | Ahead of Wispr's own budget on paper. Needs the full 50 samples, Wi-Fi, and Wispr Flow timed side by side |
| Language, Spanish and English | 21 languages; mixed-language behaviour not stated | 38 of 38 right | Even so far |
| Formatting quality | 79.3% zero-edit rate on Voice Arena, US English only ([voicearena.com](https://voicearena.com/dictation-bench)) | Not measured. Claude's read: fewer misrecognitions than OpenAI, some dropped words, names wrong ("long chain"), fillers sometimes kept | **The real unknown.** Needs `samples/gold.json` corrected by the owner and a zero-edit rate on the same samples |
| Personal dictionary, per-app style, cleanup levels | Yes | No | Not started |
| Works in any app: hotkey, paste at cursor | Yes | No, CLI only | Not started |
| Idle memory | Electron plus a Swift helper (third-party teardown, unverified) | No app yet | Budget under 100 MB |

Speed, the hardest part on paper, looks solved. What separates us from Wispr Flow now is
formatting quality and the app itself.

## Phases

| Phase | Goal | Scope | Gate to pass before the next phase |
| --- | --- | --- | --- |
| **0. Speed test** (now) | Prove Wispr Flow speed on the owner's voice | All 50 samples; full benchmark on Speechmatics EU; Wispr Flow timed on the same samples ([#4](https://github.com/AndresHG/hyperme/issues/4), [#5](https://github.com/AndresHG/hyperme/issues/5), [#15](https://github.com/AndresHG/hyperme/issues/15)) | Median at or under 800 ms, p90 at or under 1200 ms, language right on 50 of 50, formatting accepted beside Wispr Flow |
| **1. Dictation core** ([draft spec](../specs/25-dictation-core/spec.md)) | Use it instead of Wispr Flow for daily dictation | Tauri 2 menu-bar app; hold-to-talk hotkey; connection warmed on key-down; Speechmatics streaming behind the speech trait; Groq cleanup with fallback to the raw transcript; paste at the cursor; a minimal pill that shows state; keys in the keychain | A full week without opening Wispr Flow (the earlier Phase 2 gate). Idle memory and CPU measured against the budgets |
| **2. Wispr-level quality and first release** | Match Wispr Flow on formatting, then let others install it | Gold set and zero-edit rate; prompt tuning; personal dictionary; cleanup levels (none, light, medium); tone per app; clean text in pieces while the user speaks; a second speech provider as fallback; import from Wispr Flow and Superwhisper; microphone handling (pause media, mic priority); signed and notarised build | Zero-edit rate on the owner's samples at or above Wispr Flow's on the same samples. Then the first public release: "open-source dictation for your Mac, as fast as Wispr Flow" ([RESEARCH.md](RESEARCH.md) section 3). Licence decided: MIT OR Apache-2.0 |
| **3. Smart routing and actions** | One key for everything you say | The same key routes to dictation, a question or an action; command mode sends the transcript to a tool-calling model (how others do it: [voice commands research](research/2026-11-27-voice-commands.md)); one MCP client (`rmcp`); the first tracker connector; "speak, preview card, Enter" in under a second; short spoken questions answered in a card; voice edits of selected text; "Continue with ChatGPT" as an answers provider; HyperMe as an MCP server (phase 3 or 4) ([research](research/2026-11-26-competitors-and-plan-sign-in.md)); memory: "Hyper, remember …", approved facts in a Markdown folder ([PRD](PRD.md) 7.5); contexts (tags) in two levels, such as Customers > Amazon, each a file in that folder ([notes, memory and contexts](#notes-memory-and-contexts)) | Owner uses commands every day for two weeks. Covers items 1 and 2 of "what nobody ships" ([RESEARCH.md](RESEARCH.md) section 1.2) |
| **4. Daily tasks and email** | The copilot for the workday | Calendar connector, Google first; HyperMe's own task list with voice capture ("Hyper, remind me to …") and tasks blocked by other tasks; a morning card and "meeting now" nudges ([draft spec](../specs/92-daily-tasks/spec.md)); new items pulled into the task list from calendar, mail and tracker; scheduled jobs, each with a tool allowlist; email triage and replies, draft-only; contexts on tasks ("remind me to send the deck for Amazon") and a filter by context | Owner relies on it for a week of daily planning and mail drafts |
| **5. Meetings** | Meetings become tasks | Note-taker with live gray transcript and batch cleanup to white ([#20](https://github.com/AndresHG/hyperme/issues/20)), local-first: see [meetings on device](#meetings-on-device) below; proposed tickets with owner and quote; a "Transcribe?" card when a call is detected; one Meetings page for transcripts with or without a calendar event; a meeting takes contexts from its attendees or title ([notes, memory and contexts](#notes-memory-and-contexts)) | Tickets from a real meeting accepted with few edits |
| **Later** | | Memory grows: proposed facts, a graph view; evaluate again, with an interview of the owner first, an embeddings index and an AI knowledge graph (owner, 27 November 2026; see [notes, memory and contexts](#notes-memory-and-contexts)); a direct link from a fact or to-do to a meeting, only if contexts prove not enough; handing a task to Claude Code by voice, nothing more (owner, 26 November 2026); spoken answers; local speech models for dictation; Linux and Windows, including a local meeting option there in place of Apple's recognizer (for example Whisper through whisper.cpp or sherpa-onnx, both behind the speech trait) | |

## Meetings on device

**Adopted by the owner on 26 November 2026.** Meetings do not need dictation's
speed, so they can run on the Mac instead of paying Speechmatics by the minute. That would
avoid USD 10 to 16 a month at 8 hours of meetings a week. Measured on the owner's Mac
([results](results/2026-11-26-local-meetings.md)):

| Step | Proposal | Why | Load on the Mac |
| --- | --- | --- | --- |
| Live gray transcript | Apple's on-device recognizer (SpeechAnalyzer, macOS 26), behind the speech trait in a `cfg(target_os = "macos")` module with a small Swift bridge | Spanish and English, 15% of words wrong on the samples against 13% (es) and 11% (en) for Wispr Flow's engine. Free, nothing to download or ship | About 1 to 2.5% of one CPU core, runs on the Neural Engine, no GPU |
| White text, in batches of sentences | The cleanup trait, as in dictation, with a setting of three choices (owner, 26 November 2026): **cloud** (Groq `gpt-oss-20b`, the default), **local** (a model through any OpenAI-compatible server: Ollama, LM Studio, `mlx_lm.server`), or **off** (the transcript only, no cleanup, nothing leaves the Mac) | Groq cleans clearly better (7.2 edits per 100 characters against 9.8 for local Qwen3 4B) and costs about USD 0.2 to 0.35 a month for 8 hours a week | Groq: none. Local Qwen3 4B: GPU busy about 15% of the meeting, 2.3 GB of memory |
| Who spoke | Two tracks to start: the microphone is "me", system audio is "them", named from the calendar invite. Real diarization later, after the meeting, with FluidAudio (Apache-2.0, on the Neural Engine) | No extra model for the first version | None for two tracks |
| After the meeting | Notes, action points and proposed tickets from the cleaned transcript, with the same language model provider | A 4-minute transcript summarised locally in 10 s; the cloud is quicker | Short burst |

Not chosen: NVIDIA Parakeet v3 switched to English on meeting-length Spanish audio; Whisper small
made too many Spanish errors; Whisper large-v3-turbo is accurate but keeps the GPU busy, so it is
kept only as an optional re-transcription after the meeting. For the settings screen and the
meeting view, look first at how Wispr Flow and OpenWhispr present them; check the licence before
borrowing any code and record it in `NOTICE.md`. Still to measure before phase 5:
battery drain, Apple's live mode on far-field meeting audio, and diarization.

Apple's recognizer exists only on macOS. Windows and Linux get their own local option later
(owner, 26 November 2026); see the Later row.

This does not change the dictation core, which stays on cloud speech for speed.

## Notes, memory and contexts

**Adopted by the owner on 27 November 2026** after the [research](research/2026-11-27-notes-memory-tags.md) and an interview (N-1 to N-4 in
[PRD.md](PRD.md) section 12). Not for the current version.

| Piece | Decision | Phase |
| --- | --- | --- |
| Meeting prompt | When a call app holds the microphone, or a calendar meeting with other attendees starts, the dot shows "In a meeting. Transcribe?". Enter starts; Esc or 20 seconds dismisses; mute per app. Nothing records without a yes, as in Wispr Flow and Granola. Noticed through a macOS "microphone in use" event, so no polling and no idle CPU | 5 |
| Meetings page | One list for every transcript. A calendar event is attached when the times overlap (within 15 minutes, as Granola) and can be changed by hand; without one, the title is generated after the meeting | 5 |
| Contexts (tags) | Two levels: a group (Customers, Family) holds contexts (Amazon, Mother). Each context is a Markdown file in the memory folder (`customers/amazon.md`) with its facts and aliases ("AWS", "mamá"); groups are folders. Tasks, facts and meetings carry context ids in the SQLite index | 3 (memory), 4 (tasks), 5 (meetings) |
| Linking | Local and deterministic: names and aliases matched in Rust, no extra model call. The command call that already parses "remember …" may pick the context. A new context or a fact that replaces an old one goes to the approval list | 3 |
| Fact format | One fact per line with its date, `[[links]]` to other files, aliases in the file's properties, readable in Obsidian. A fact can be marked as replaced by a newer one | 3 |
| Meetings and to-dos | No direct link. "For Amazon" puts a to-do or fact under Amazon, and the Amazon meeting shows beside it through the shared context | 5 |

Not now, but to evaluate again when there is bandwidth (owner, 27 November 2026): a local
embeddings index for fuzzy recall ("what did we say about pricing?"), and an AI knowledge graph
that extracts entities and facts with a model, as Zep and Mem0 do. The owner likes both. Before
either is planned, interview the owner to settle the product questions: what it may learn
without approval, the cost per capture, where the index lives and its memory budget. Then pick
the best approach. Not chosen: free-depth tags, which Tana itself calls experimental and slow.

## How this differs from the earlier plan

- Dictation moves from Phase 2 to Phase 1, because the owner's gate is Wispr Flow performance
  first and the speed test now passes on paper.
- Quality gets its own phase. Speed is close to done; formatting is the open risk, and it is
  what the owner will notice every day.
- The first public release comes after dictation alone, not after the full product. Research
  found single-purpose launches do far better than all-in-one ones ([RESEARCH.md](RESEARCH.md)
  section 1.4).
- Crons move behind commands. Research found them the most crowded part of the field.
- Meetings come last and start from the live note-taker, since bot-less capture alone is a
  commodity ([research](research/2026-11-24/02-meetings.md)).

## Open decisions for the owner

The single list of open questions is now [PRD.md](PRD.md) section 12. Earlier items:

1. ~~What comes after dictation~~: decided 25 November 2026, smart routing and actions (phase 3).
   Email before meetings: decided 26 November 2026 (PRD Q-6).
2. ~~First tracker connector~~: decided 25 November 2026, GitHub (this project's own tickets live there).
3. ~~Mail and calendar provider~~: decided 26 November 2026, Google first (PRD Q-7).
4. ~~Licence~~: decided 25 November 2026, MIT OR Apache-2.0 ([#7](https://github.com/AndresHG/hyperme/issues/7)).
5. ~~Trial and baseline as gates~~: dropped 26 November 2026 (PRD Q-13); #5 and #6 closed.
6. ~~The first outside user~~: decided 26 November 2026, people in tech companies, developers
   first (PRD Q-4).
7. ~~Where meeting cleanup runs~~: decided 26 November 2026, a setting with three choices: cloud
   (Groq, default), local model, or off. See [meetings on device](#meetings-on-device).

## Not checked

Whether a roadmap exists on a GitHub Projects board: this session can read the repository and
its issues but not the account's Projects. No milestones exist in the repository.
