# HyperMe roadmap

**Status: adopted by the owner on 25 November 2026.** What and why: [PRD.md](PRD.md); this file says when. It replaces the earlier phase table in
[SPEC.md](SPEC.md) section 5. The owner approved Phase 1 on 26 November 2026 (PRD Q-2), and its code is merged; its gate is
under way. On 26 November 2026 the owner also asked to start phase 2 work in parallel: see
[the plan for phases 1 and 2](plans/2026-11-26-phases-1-2.md). The first public release still waits for both gates.

## Where we stand against Wispr Flow

The owner's target is Wispr Flow's performance. Measured so far, on the owner's Mac (refreshed 26 November 2026):

| Measure | Wispr Flow | HyperMe today (`flowbench`, CLI only) | Gap |
| --- | --- | --- | --- |
| Key release to cleaned text | Published budget under 700 ms, a p99 target ([Wispr, Sept 2025](https://wisprflow.ai/post/technical-challenges)). Not yet timed on the owner's Mac ([#5](https://github.com/AndresHG/hyperme/issues/5)) | Median 426 ms, p90 587 ms: Speechmatics `melia-1` EU plus Groq `gpt-oss-20b`, prompt `v1`, wired, 38 of 50 samples ([results](results/2026-11-25-eu-speech.md)) | Ahead of Wispr's own budget on paper. Needs the full 50 samples, Wi-Fi, and Wispr Flow timed side by side |
| Language, Spanish and English | 21 languages; mixed-language behaviour not stated | 38 of 38 right | Even so far |
| Formatting quality, edits per 100 characters against the owner's gold texts (lower is better) | 11.6 on the owner's 50 samples, 4 of 50 exact ([results](results/2026-11-26-wispr-comparison.md)). Voice Arena reports 79.3% zero-edit, US English only ([voicearena.com](https://voicearena.com/dictation-bench)) | 7.6 with prompt `v3`, 5 of 50 exact; 6.5 and 8 of 50 exact with a dictionary learned from corrections ([results](results/2026-11-26-dictionary.md)) | Ahead on the same samples. Wispr still gets the owner's internal names right without help; the dictionary closes that |
| Personal dictionary | Yes | Substitution before cleanup works in flowbench and in the app's config; no way to learn or edit it in the app | Being designed in its own thread |
| Per-app style, cleanup levels | Yes | No | Phase 2 |
| Works in any app: hotkey, paste at cursor | Yes | Yes: the Phase 1 menu-bar app, hold Fn, paste at the cursor, history and Retry (PRs #47 to #91) | Live checks and the owner's week of use still to run |
| Key-down to recording | Not published | Median about 65 to 70 ms ([results](results/2026-11-26-mic-start-latency.md)) | Misses the 50 ms budget; a decision for the owner |
| Idle memory | Electron plus a Swift helper (third-party teardown, unverified) | 59.4 MB and 0% CPU, before any dictation ([results](results/2026-11-26-idle-usage-baseline.md)) | Within the 100 MB budget; still to measure after 20 dictations |

Speed and formatting are ahead of Wispr Flow on the owner's samples. What separates us now is
daily use of the app itself, the personal dictionary in the app, and a release others can install.

## Phases

| Phase | Goal | Scope | Gate to pass before the next phase |
| --- | --- | --- | --- |
| **0. Speed test** (done: the owner approved Phase 1 on 26 November 2026, PRD Q-2) | Prove Wispr Flow speed on the owner's voice | All 50 samples; full benchmark on Speechmatics EU; Wispr Flow timed on the same samples ([#4](https://github.com/AndresHG/hyperme/issues/4), [#5](https://github.com/AndresHG/hyperme/issues/5), [#15](https://github.com/AndresHG/hyperme/issues/15)) | Median at or under 800 ms, p90 at or under 1200 ms, language right on 50 of 50, formatting accepted beside Wispr Flow |
| **1. Dictation core** (now: code merged, gate under way; [spec](../specs/25-dictation-core/spec.md)) | Use it instead of Wispr Flow for daily dictation | Tauri 2 menu-bar app; hold-to-talk hotkey; connection warmed on key-down; Speechmatics streaming behind the speech trait; Groq cleanup with fallback to the raw transcript; paste at the cursor; a minimal pill that shows state; keys in the keychain | A full week without opening Wispr Flow (the earlier Phase 2 gate). Idle memory and CPU measured against the budgets |
| **2. Wispr-level quality and first release** | Match Wispr Flow on formatting, then let others install it | Gold set and zero-edit rate; prompt tuning; personal dictionary; cleanup levels (none, light, medium); tone per app; clean text in pieces while the user speaks; a second speech provider as fallback; import from Wispr Flow and Superwhisper; microphone handling (pause media, mic priority); signed and notarised build | **Proposed, for the owner to approve (#75):** on the owner's 50 samples, edits per 100 characters at or below Wispr Flow's 11.6 with the shipped app's own pipeline, every internal name in the owner's dictionary right, and a week of daily use with cleanup levels and the dictionary on. (The old rule, a zero-edit rate at or above Wispr Flow's, is already met on paper, 5 of 50 against 4 of 50, so it no longer separates good from not.) Then the first public release: "open-source dictation for your Mac, as fast as Wispr Flow" ([RESEARCH.md](RESEARCH.md) section 3). Licence decided: MIT OR Apache-2.0 |
| **3. Smart routing and actions** | One key for everything you say | The same key routes to dictation, a question or an action; command mode sends the transcript to a tool-calling model (how others do it: [voice commands research](research/2026-11-27-voice-commands.md)); one MCP client (`rmcp`); the first tracker connector; "speak, preview card, Enter" in under a second; short spoken questions answered in a card; voice edits of selected text; "Continue with ChatGPT" as an answers provider; HyperMe as an MCP server (phase 3 or 4) ([research](research/2026-11-26-competitors-and-plan-sign-in.md)); memory: "Hyper, remember …", approved facts in a Markdown folder ([PRD](PRD.md) 7.5) | Owner uses commands every day for two weeks. Covers items 1 and 2 of "what nobody ships" ([RESEARCH.md](RESEARCH.md) section 1.2) |
| **4. Daily tasks and email** | The copilot for the workday | Calendar connector, Google first; HyperMe's own task list with voice capture ("Hyper, remind me to …") and tasks blocked by other tasks; a morning card and "meeting now" nudges ([draft spec](../specs/92-daily-tasks/spec.md)); new items pulled into the task list from calendar, mail and tracker; scheduled jobs, each with a tool allowlist; email triage and replies, draft-only | Owner relies on it for a week of daily planning and mail drafts |
| **5. Meetings** | Meetings become tasks | Note-taker with live gray transcript and batch cleanup to white ([#20](https://github.com/AndresHG/hyperme/issues/20)), local-first: see [meetings on device](#meetings-on-device) below; proposed tickets with owner and quote | Tickets from a real meeting accepted with few edits |
| **Later** | | Memory grows: proposed facts, a graph view; handing a task to Claude Code by voice, nothing more (owner, 26 November 2026); spoken answers; local speech models for dictation; Linux and Windows, including a local meeting option there in place of Apple's recognizer (for example Whisper through whisper.cpp or sherpa-onnx, both behind the speech trait) | |

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
