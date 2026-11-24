# Research: should HyperMe be built, and as what?

Date: 24 November 2026.

Sources: four research reports in [`research/2026-11-24/`](research/2026-11-24/) and a
nine-round interview with the owner. The reports mark every claim as verified at a
primary source that day, or not. Nothing was installed or measured: every latency figure
is a vendor claim until Phase 0 replaces it.

| Report | Covers |
| --- | --- |
| [01-dictation](research/2026-11-24/01-dictation.md) | Dictation products, Wispr Flow in depth, the sub-second recipe, speech and cleanup APIs with prices |
| [02-meetings](research/2026-11-24/02-meetings.md) | Bot-less notetakers, Granola in depth, system-audio capture, cost, consent rules in Spain |
| [03-agents-overlays](research/2026-11-24/03-agents-overlays.md) | Desktop agents, overlays, coding-agent dispatch, MCP connectors, platform risk |
| [04-redteam](research/2026-11-24/04-redteam.md) | The case against building, base rates, demand signals, kill criteria, cheapest experiments |

## 1. Verdict

- **Do not build the six-part product in `SPEC.md` as it stands.** Its premise, "every
  piece exists open source; nobody ships the whole", stopped being true during 2026.
- **Do run the validation test (Phase 0).** It takes one to two days, it is the owner's
  chosen gate, and its provider code is reused later.
- **Expect it to pass, and do not read a pass as "build the product".** The test answers
  "can we match Wispr Flow". Two cheaper checks (section 1.3) answer "should we".
- **If it goes ahead:** a small dictation core built on Handy's MIT crates and released on
  its own, then the capabilities nobody ships (section 1.2). Not its own meeting capture,
  and not a scheduler first.

### 1.1 What changed since the first analysis

| First analysis said | Research found |
| --- | --- |
| No open-source project combines the parts | OpenWhispr (MIT, 12,161 stars, five active committers) has dictation, a voice-command hotkey, meetings with calendar, notes, an API, an MCP server and bring-your-own-key. As far as its README shows, it lacks scheduled jobs and coding-agent dispatch. Omi desktop (MIT, Swift) has the floating bar with agent pills. |
| Wispr Flow is a dictation app | It added a bot-less notetaker (August 2026), an MCP server, "Ask Wispr" drafts for Gmail, Slack and Notion, and its own speech models, on $361M raised to build a "voice OS". |
| Start with commands and crons, because nobody offers them | Crons and proactive nudges are the most crowded part: Anthropic, OpenAI and Microsoft shipped them in 2026, and Google and Meta are reported to have. Claude desktop alone has a quick-entry overlay with dictation, connectors, scheduled tasks and background Claude Code sessions. |
| Formatting quality is the largest risk | On the one cross-vendor benchmark (US English) a small MIT client scores 78.0% zero-edit rate against Wispr Flow's 79.3% and Aqua's 80.5%. English quality is close to a commodity. |
| Handy transcribes after key release | Handy streams since v0.9.0 (July 2026), with local models only. It still has no cloud speech, and cleanup is an optional extra step. |

### 1.2 What nobody ships

1. One push-to-talk key that routes to dictation, a previewed action or an agent, with a
   pill that shows state. Omi's "type ..." wake word is the only shipped example.
2. A sub-second "speak, preview card, Enter". The Claude path opens a chat turn that takes
   seconds.
3. Meetings turned into proposed tickets with owner and exact quote. Granola and Wispr
   Flow stop at notes, and neither creates tickets. Circleback comes closest.
4. Starting and following coding agents from several vendors by voice. Each vendor
   dispatches only its own agent.
5. A light native client with cloud streaming and good cleanup. Handy is light but local
   only. OpenWhispr is Electron. The Rust + Tauri projects that stream to the cloud are
   small: SpeakoFlow (407 stars), OpenTypeless (703 stars).

### 1.3 The two checks that can say no

The validation test will most likely pass, so it cannot be the only gate.

1. **Existing tools.** One day each on today's Handy, OpenWhispr and Omi desktop.
   OpenWhispr has shipped 98 releases and now streams. Write down exactly what is slow or
   disliked: that list is HyperMe's requirement. If one of them is enough, contribute
   there.
2. **Baseline.** Measure Wispr Flow and OpenWhispr on the same Mac: latency on the same
   utterances, and idle memory. "Faster and lighter" needs a number to beat.

The owner has not adopted these as gates (interview, round 5). They are recommendations.

### 1.4 Reasons against the full product

- **Coverage.** Wispr Flow Pro plus Claude Pro, both already paid, cover four of the five
  original pillars at no extra cost.
- **Upkeep.** Version 1 takes days to weeks in every comparable project. The cost is
  after it: Handy has shipped 69 releases in 21 months, is still 0.x, and has 200 issues
  about paste alone.
- **Attention.** All-in-one launches scored 1 to 15 points on Hacker News in 2026.
  Single-purpose "open-source alternative to one app" launches scored 200 to 300.
- **Distribution.** Other people cannot easily connect mail, calendar or chat: Slack bars
  unlisted apps, Google Workspace MCP is a preview that needs your own Cloud project,
  Microsoft 365 is tenant-gated, and third parties may not offer claude.ai login.
- **Money is not an argument either way.** The build saves at most $15 a month. API cost
  for heavy use runs from about $9 to $100 a month depending on model choice.
- **Time is the unquantified risk.** The owner's time is limited and has no fixed budget.
  An open-source project with users is a release treadmill.

## 2. Product focus (owner's decisions, 24 November 2026)

| Question | Decision |
| --- | --- |
| What makes it worth building in 12 months | An open-source project that other people install and use |
| What it is at its core | One surface that replaces four apps |
| Why someone picks it | Speed and lightness, with the full feature set, open source |
| First target | Demonstrate Wispr Flow performance. Everything else is capability added afterwards |
| Finish line for the first target | Numbers from a CLI on the owner's own utterances, plus trying it by voice in the CLI. No pill or overlay needed |
| Languages | Spanish and English, one language per dictation, never mixed in a sentence |
| Memory and learning | Part of the vision and the pitch ("knows you better every day", a knowledge base in the style of Obsidian). Not in any first version |
| Privacy boundary | Audio, transcript, dictionary, app name and text before the cursor may leave the machine, under the owner's own keys. Window text or a screenshot only behind a visible switch, off by default. No telemetry. No audio kept on disk |
| Tracker for this project | GitHub Projects |
| Harness and project setup | Later, once the product is clearer. Captured as tickets (section 7) |

Not decided: the first outside user, the order of capabilities after dictation, an
adoption measure, the licence, the mail and calendar provider.

Working assumption for the first outside user: people who dictate all day into coding
agents and chat apps and want a tool that is open and light. In the one thread with usage
reports, commenters described thousands of dictated prompts, mostly into coding agents.

## 3. Elevator pitch

**One line, for after the gate passes:**
HyperMe is open-source dictation for your Mac: as fast as Wispr Flow, and light enough to
forget it is running.

**The vision, in two sentences:**
HyperMe is one small pill for everything you say at work. Hold a key and your words land
as clean text in any app; your meetings become tasks, your agents take orders, and it
knows you better every day, on your own keys and your own disk.

**What each claim has to earn before it goes in a README:**

| Claim | Evidence needed |
| --- | --- |
| As fast as Wispr Flow | Phase 0 numbers on the owner's utterances, beside Wispr Flow's |
| Light | Idle memory measured against Wispr Flow and OpenWhispr on the same Mac |
| Does things others do not | At least one item from section 1.2 working |
| Knows you better every day | Not before a memory design exists. Keep it out of the README until then |

## 4. Map of the field

| Layer | Closed leaders | Open source | State in November 2026 |
| --- | --- | --- | --- |
| Dictation | Wispr Flow, Aqua, Superwhisper, Willow, Typeless, Monologue; free from Granola, Apple, Google, ChatGPT | Handy, OpenWhispr, FluidVoice, VoiceInk, FreeFlow, Hex, SpeakoFlow, OpenTypeless, Blurt | Commodity in English. Table stakes, not a differentiator |
| Meeting notes | Granola, Wispr Flow Notetaker, Notion, ChatGPT Record, Krisp, Circleback, Zoom | Meetily, Anarlog, Minutes, Omi, Quill | Commodity. The open step is transcript to proposed tickets |
| Crons and nudges | Claude (scheduled tasks, Duet), ChatGPT (tasks, dots), Gemini Spark (reported), Copilot Autopilot, Poke, Lindy, Littlebird | OpenClaw, Hermes Agent, goose | Saturated. Call into what exists |
| Commands through connectors | Claude connectors, ChatGPT plugins, Raycast AI extensions | OpenWhispr, Omi | Crowded, but no sub-second preview card |
| Coding-agent dispatch | Claude Code agent view, Codex app, Conductor | Happy, Nimbalyst, Paradev | Single vendor is solved. Several vendors by voice is open |
| Overlay | Claude Quick Entry, Raycast, Cluely | Omi desktop, Thuki (frozen), OpenClicky | Omi has the shape. OpenAI is likely to ship one |
| Memory | Claude and ChatGPT memory, Littlebird | Omi, Minutes (Markdown files) | Vendor memory is closed. Plain files the user owns are rare |

## 5. What to borrow

| From | Licence | Take |
| --- | --- | --- |
| Handy | MIT | The base for dictation: `handy-keys`, `handy-recorder`, clipboard and paste code, the NSPanel overlay. Replace its local transcription manager |
| SpeakoFlow, OpenTypeless | MIT | Worked examples: WebSocket streaming to cloud speech, keys in the keychain, app detection with per-app tone |
| Blurt | MIT | Design only (Swift): pre-warm on key-down, upload while recording, narrow context policy |
| FreeFlow | MIT | Its published cleanup prompt |
| Omi desktop | MIT | Ideas, translated from Swift: non-activating panel settings, the rule that only an explicit request may take focus, one push-to-talk state machine, agent pill states |
| Thuki, `tauri-nspanel` | Apache-2.0 | Tauri 2 panel above fullscreen apps, double-tap activator. Thuki is in maintenance mode: fork and own |
| Minutes | MIT | Meeting file format with structured action items and a "restricted" flag |
| Anarlog | MIT outside `enterprise/` | The "proposal stays pending until a human applies it" pattern. Capture crates only if capture is ever built; check each `PROVENANCE.md` |
| goose | Apache-2.0 | `rmcp` 3.x usage, recipe format. Do not adopt its runtime |
| IronClaw | Apache-2.0 | Credential injection at the host boundary, as a design |

**Do not copy code from:** VoiceInk and FluidVoice (GPL-3.0); Epicenter/Whispering,
OpenLess, Tambourine, VoiceTypr, Claude Squad and opcode (AGPL-3.0); screenpipe (commercial,
source-available).

**Consume instead of building:** meeting capture (read an existing notetaker over MCP or
watch Minutes' files), scheduling (a Claude scheduled task or an OS timer).

## 6. Corrections to `FIELD.md` and `SPEC.md`

- Handy now streams with local models. It has 40,050 stars and 193 contributors.
- SpeakoFlow has 407 stars and already streams to cloud speech.
- Meetily: speaker labels, MCP, CLI and webhooks are in the closed PRO edition.
- Anarlog: the team's main product is now "char". Some files are relicensed ports.
- Minutes: capture is a Swift helper and the MCP server is TypeScript. Eight months old.
- Thuki is in maintenance mode and runs local models only.
- Clicky's open repository is frozen. Teachy has 4 stars.
- IronClaw's main branch has been idle since 10 September 2026.
- OpenClaw has a Mac menu-bar app with voice wake. Hermes Agent has an Electron desktop
  app. Neither is only a chat gateway now.
- Devin is not an Agent Client Protocol agent. Use its MCP server or v3 API.
- Core Audio taps need their own permission (`NSAudioCaptureUsageDescription`). `cpal`
  also does macOS loopback: added in 0.17, silent captures fixed in 0.18.
- Wispr Flow's speech model is its own Canto-1 and Canto-2. Llama on Baseten is the older
  description.
- `gpt-live-transcribe` is the most expensive streaming option at about $0.017 a minute.
- `rmcp` is at 3.11.0 after three major versions in five months. Pin it.

## 7. Next steps

### Validation test: changes to `SPEED_TEST.md`

- 50 utterances: 25 Spanish and 25 English, one language each. Check that language is
  detected with no manual switch and nothing is translated.
- Providers to compare, since no single one is known to win: ElevenLabs Scribe v2
  Realtime, Soniox, AssemblyAI realtime, Deepgram Nova-3 multilingual, OpenAI
  `gpt-live-transcribe`; Groq Whisper as the cheap upload baseline.
- Cleanup models to compare: Groq `gpt-oss-20b`, Cerebras `gpt-oss-120b`, `gpt-6-luna`,
  Claude Haiku 5.5, Gemini 2.5 Flash-Lite. Time to first token is unverified for all of
  them and decides the result.
- Recipe: open and warm the connection on key-down, stream while the key is held, send an
  explicit finalize on release (no server-side turn detection), skip cleanup for very
  short utterances, fall back to the raw transcript after a deadline.
- Log every stage for every utterance.
- Live microphone mode, so the owner can try it by voice.
- Side by side with Wispr Flow on the same utterances.

### Tickets to open later (GitHub Projects)

Validation:
1. Update `SPEED_TEST.md` from this research and build `spikes/flowbench`.
2. Record the 50 samples.
3. Baseline: Wispr Flow and OpenWhispr latency and idle memory on the same Mac.
4. Trial: Handy, OpenWhispr and Omi desktop, one day each, with notes.
5. Decide the licence. Handy's crates are MIT; `tauri-nspanel` and `rmcp` are Apache-2.0.

Harness, once the product is clearer:
6. Ignore `.omc/` state in git; keep specs in the repository.
7. `CLAUDE.md`: branch and pull-request rules, ticket ids in branches and commits.
8. Pull-request template with a "Lessons" section.
9. Git guard hook with tests (no push from `claude/*` branches, no bare `git stash`).
10. `kb/` with index, log and an integrity test.
11. A `/spec` skill with a pre-mortem file seeded from the traps in these reports.
12. A review rubric and a clean-context reviewer before pull requests.
