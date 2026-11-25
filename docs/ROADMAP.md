# HyperMe roadmap

**Status: proposal of 25 November 2026, not yet adopted by the owner.** It replaces the table in
[SPEC.md](SPEC.md) section 5, which has been under review since 24 November. Phase 0 is still the
only committed work. Each later phase starts only after the gate of the one before it passes
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
| **2. Wispr-level quality and first release** | Match Wispr Flow on formatting, then let others install it | Gold set and zero-edit rate; prompt tuning; personal dictionary; cleanup levels (none, light, medium); tone per app; clean text in pieces while the user speaks; a second speech provider as fallback; signed and notarised build | Zero-edit rate on the owner's samples at or above Wispr Flow's on the same samples. Then the first public release: "open-source dictation for your Mac, as fast as Wispr Flow" ([RESEARCH.md](RESEARCH.md) section 3). Licence decided: MIT OR Apache-2.0 |
| **3. Smart routing and actions** | One key for everything you say | The same key routes to dictation, a question or an action; command mode sends the transcript to a tool-calling model; one MCP client (`rmcp`); the first tracker connector; "speak, preview card, Enter" in under a second; short spoken questions answered in the pill | Owner uses commands every day for two weeks. Covers items 1 and 2 of "what nobody ships" ([RESEARCH.md](RESEARCH.md) section 1.2) |
| **4. Daily tasks and email** | The copilot for the workday | Calendar connector; crons and "meeting now" nudges, each job with a tool allowlist; email triage and replies, draft-only | Owner relies on it for a week of daily planning and mail drafts |
| **5. Meetings and coding agents** | Meetings become tasks; agents take orders | Note-taker with live gray transcript and batch cleanup to white ([#20](https://github.com/AndresHG/hyperme/issues/20)); proposed tickets with owner and quote; start and follow Claude Code and Devin tasks by voice | Tickets from a real meeting accepted with few edits; one agent task started and reported back from the pill |
| **Later** | | Memory and learning ("knows you better every day"); local speech models; Linux and Windows | |

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

1. ~~What comes after dictation~~: decided 25 November 2026, smart routing and actions (phase 3).
   Still open: whether email comes before meetings.
2. ~~First tracker connector~~: decided 25 November 2026, GitHub (this project's own tickets live there).
3. Mail and calendar provider for phase 4: Google or Microsoft 365.
4. ~~Licence~~: decided 25 November 2026, MIT OR Apache-2.0 ([#7](https://github.com/AndresHG/hyperme/issues/7)).
5. Whether to adopt the trial of existing tools ([#6](https://github.com/AndresHG/hyperme/issues/6))
   and the Wispr Flow baseline ([#5](https://github.com/AndresHG/hyperme/issues/5)) as gates.
6. The first outside user.

## Not checked

Whether a roadmap exists on a GitHub Projects board: this session can read the repository and
its issues but not the account's Projects. No milestones exist in the repository.
