# Track 01: Dictation and voice input

Research date: 24 November 2026. All GitHub numbers were read from the GitHub API on that date.

## How to read the labels

- **[V]** VERIFIED: I opened a primary source today (vendor page, vendor docs, GitHub API, Hacker News thread). For vendor pages this verifies that the vendor *says* it, not that it is true.
- **[S]** UNVERIFIED (secondary, read in full today): press or a third-party write-up.
- **[U]** UNVERIFIED: search snippet, competitor blog, or memory.
- **Inference:** my own reasoning. Kept in separate paragraphs or marked inline.

---

## Summary

1. **English dictation quality is close to a commodity.** On the one cross-vendor benchmark I found (Voice Arena Dictation Bench, US English, macOS, 759 clips), an MIT-licensed thin client over a turnkey API (Blurt, by AssemblyAI) scores 78.0% zero-edit rate against Wispr Flow's 79.3% and Aqua's 80.5% [V: [voicearena.com/dictation-bench](https://voicearena.com/dictation-bench)]. The benchmark does not measure latency yet.
2. **Wispr Flow has moved well past dictation.** It now has a bot-less Notetaker (Aug 2026), an MCP server, tasks, "Ask Wispr" that drafts Gmail, Slack and Notion items behind a Send button, its own speech models (Canto-1 in Sept, Canto-2 on 23 Nov 2026), Android and iOS, and $361M raised at a $2B valuation. It still has no Linux client, no bring-your-own-key, no open source and no coding-agent dispatch.
3. **The sub-second recipe is public.** Stream audio while the key is held, pre-warm the connection, finalize on key release, run a small LLM with a tight budget, paste. Wispr's own budget is ASR under 200 ms, LLM under 200 ms, network under 200 ms, total under 700 ms.
4. **Streaming is no longer rare in open source.** OpenWhispr, VoiceInk, FreeFlow, SpeakoFlow, OpenTypeless and Tambourine all stream to cloud STT; Handy, Hex and FluidVoice stream with local models.
5. **Spanish-English mixing in one utterance is the real risk.** Whisper-family models tend to translate instead of transcribe. Provider choice matters more here than anywhere else.
6. **Handy is the codebase to borrow from** (MIT, Rust + Tauri 2, 40,050 stars, 193 contributors, released today). It has no cloud STT; SpeakoFlow (MIT, built on Handy) shows how to add WebSocket streaming on top.

---

## A. Product landscape

### A1. Commercial products

| Product | Platforms | Price | Cloud / local | Beyond dictation | Latency claim | Stack, funding, users |
| --- | --- | --- | --- | --- | --- | --- |
| **Wispr Flow** | Mac, Windows, iPhone, Android. No Linux. [V: [pricing](https://wisprflow.ai/pricing)] | Free (weekly word cap, number not shown); Pro $15/mo or $12/mo annual; Growth and Enterprise above. Students 50% off. [V] | Cloud only: "Transcription always occurs on the cloud" [V: [data-controls](https://wisprflow.ai/data-controls)] | Command Mode, Notetaker, MCP server, tasks, Ask Wispr drafts, Transforms, Scratchpad, snippets, per-app Styles, dictionary, context awareness. See section B. | Under 700 ms end of speech to text, as a budget [V: [canto-1-results](https://wisprflow.ai/canto-1-results)] | $280M Series B at $2B, Aug 2026; $361M total [S: [TechCrunch](https://techcrunch.com/2026/08/17/wispr-raises-280m-at-2b-valuation-as-it-looks-beyond-dictation/)]. 1 billion words dictated per month in Sept 2025 [V: [technical-challenges](https://wisprflow.ai/post/technical-challenges)]. Desktop is Electron plus a Swift helper [S: [third-party teardown](https://www.wensenwu.com/thoughts/wispr-flow-investigation)]. |
| **Superwhisper** | Mac, Windows, iOS, Android, Linux listed in site footer [V: [superwhisper.com](https://superwhisper.com/)] | Free (local Whisper, two modes, no AI processing); Pro $8.49/mo, $84.99/yr, $249.99 lifetime [V: [plans](https://superwhisper.com/docs/billing/plans.md)] | Both. Local Whisper offline; cloud models on Pro; bring your own API keys on Pro [V] | Modes (Message, Email, custom prompts), "Super Mode" that adapts to the screen, custom vocabulary, meeting assistant (Mac), file transcription, coding-agent integrations (Cursor, Claude Code, Codex) [V] | None published [V] | "Hundreds of thousands" of users [V]. Funding not stated. |
| **Aqua Voice** | Mac, Windows, iPhone, Android [V: [aquavoice.com](https://aquavoice.com/)] | Free 2,000 words; Pro $12/mo ($10 annual); Max $24/mo ($19 annual); Business $24/seat [V: [pricing](https://aquavoice.com/pricing)] | Cloud. Own model, Avalon 1.5 [V] | Edit Mode on selected text, Deep Context (reads names on screen), Notetaker, voice commands and Realtime Mode on Max, MCP, CLI and a public API [V] | None on site. Its own blog reportedly measured 965 ms for Aqua, 1,399 ms for Wispr Flow, 2,407 ms for Superwhisper on an M3 Max [U: search snippet; page returned 404 today] | #1 on Voice Arena at 80.5% zero-edit rate [V]. Funding not stated on site. |
| **Willow Voice** | Mac, Windows, iPhone; Android waitlist [V: [willowvoice.com](https://willowvoice.com/)] | Free (limited); Pro $15/mo ($12 annual); Business $35 ($28) [V: [pricing](https://willowvoice.com/pricing)] | Cloud, plus an offline mode "powered entirely by your device" [V] | Scribe (rough description to polished text), style matching per app, auto-learning dictionary, shortcuts, whisper mode [V] | "As little as 200ms" [V] | "150,000+ professionals" [V]. Reported to run its LLM step on Groq [U: case-study title only]. |
| **Typeless** | macOS, Windows, Linux, iOS, Android [V: [typeless.com](https://www.typeless.com/)] | Free (limited); Pro $30/mo or $12/mo billed yearly; Ultra $200/mo [V: [pricing](https://www.typeless.com/pricing)] | Cloud; says audio is "never stored in the cloud" and history stays on device [V] | Edit by voice, ask about selected text, "quick answers and actions" (search sites, open pages with details filled in), translation, per-app tone, dictionary [V] | None in ms; "220 wpm" [V] | Not stated. |
| **Monologue** (by Every) | Mac, iPhone, iPad, Apple Watch [V: [monologue.to](https://www.monologue.to/)] | Trial 1,000 words; Pro $15/mo or $144/yr [V] | Cloud formatting; local transcription models on Mac [V] | Four modes that switch by app, custom instructions, dictionary, uses surrounding code for names, bot-free meeting notes, notes reachable by agents through MCP, API and CLI [V] | None [V] | "500M+ words dictated" [V]. |
| **MacWhisper** | Mac | Reported at about EUR 59 to 64 one-time on Gumroad; App Store subscription separately [U] | Local first | File transcription is the core; basic push-to-talk dictation; meeting recording in beta [U] | None | Solo developer product [U]. |
| **Spokenly** | macOS, Windows, Linux, iPhone, Android [V: [spokenly.app](https://spokenly.app/)] | Local free; own API keys free; Local+ $39 once; Pro $15/mo ($12 annual) [V] | Both, with bring-your-own-key [V] | Agentic Actions on macOS, MCP voice input for Claude Code, Codex and Cursor, AI modes, meeting recording, CLI [V] | None [V] | Closed source [V]. 73.8% on Voice Arena [V]. |
| **Granola Voice Typing** | Mac (Windows, mobile "coming") | Free in every Granola plan | Cloud | Hold a key, dictate into any app. Launched 29 Oct 2026. [U: search snippet of docs.granola.ai] | None | 71.9% on Voice Arena [V]. |
| **Apple** | iOS 27 keyboard; macOS built-in dictation | Free | Not stated | iOS 27 adds systemwide dictation that fixes spelling, punctuation and capitalization, on a model "created based on Google's Gemini" [S: [TechCrunch, 8 Jun 2026](https://techcrunch.com/2026/06/08/apple-introduces-systemwide-dictation)] | None | Current macOS Apple Dictation scores 34.5% on Voice Arena, last of 14 [V]. |
| **Google Gemini app for macOS** | Mac | Not stated | Cloud | "Speak naturally into any window on your desktop"; removes fillers, handles mid-sentence corrections, drops text at the cursor. 25 Aug 2026. [V: [blog.google](https://blog.google/innovation-and-ai/products/gemini-app/enable-intelligent-dictation-macos/)] | "Instantly" | "Gemini 3.5 Transcribe" scores 75.8% on Voice Arena [V]. |
| **ChatGPT desktop** | Mac, Windows | In Plus, Pro, Business, Edu, Enterprise | Cloud | Voice mode on GPT-Live since 23 Jul 2026. OpenAI's wording: "Control your computer and direct multiple agents running in ChatGPT Work or Codex, using just your voice." Dictation is inside the app, not into other apps. [S: [9to5Mac](https://9to5mac.com/2026/07/23/openai-updating-chatgpt-desktop-app-with-gpt-voice-for-talking-through-work/)] | None | "ChatGPT Dictation" scores 64.3% on Voice Arena [V]. |
| **Claude desktop** | Web, mobile, desktop | All plans | Cloud | Dictation into the message box; voice mode is separate [V: [Claude tutorial](https://academy.claude.com/tutorials/how-to-choose-between-voice-mode-and-dictation)]. A Mac "quick entry" voice shortcut and Claude Code hold-to-talk exist [U]. No dictation into other apps found. | None | "Claude Dictation" scores 64.0% on Voice Arena [V]. |

Also on the Voice Arena board and launched or relaunched in 2026: Blurt (AssemblyAI), Kivi, Meta AI, Perplexity Dictation [V]. I did not research Kivi, Meta AI or Perplexity further.

**Most common complaints**

| Product | Complaints | Source |
| --- | --- | --- |
| Wispr Flow | Multi-day outages in late May and June 2026 (Wispr published post-mortems); Auto Cleanup changed words users did not ask it to touch; the Flow Bar covered app controls; 700+ people answered a public call for criticism in July 2026 | [V: [whats-new](https://wisprflow.ai/whats-new)], [S: [Digital Trends](https://www.digitaltrends.com/computing/wispr-flow-asked-its-haters-what-was-wrong-and-more-than-700-people-answered/)] |
| Wispr Flow | Privacy: see section B | |
| Wispr Flow | High idle memory, around 800 MB | [U: competitor blogs] |
| Superwhisper | "Too clever", too complex | [V: [HN thread](https://news.ycombinator.com/item?id=47040375)] |
| Subscriptions in general | Preference for one-time purchase or free local tools | [V: same thread] |

**One thing most worth copying from each**

| Product | Copy this |
| --- | --- |
| Wispr Flow | A hard latency budget split by stage, and four cleanup levels (None, Light, Medium, High) after over-aggressive cleanup hurt trust |
| Aqua | Deep Context: read names on screen to fix spelling of proper nouns |
| Superwhisper | Modes as user-editable prompts with a model choice per mode |
| Willow | Dictionary that learns from corrections without manual entry |
| Typeless | "Keeps your final wording after self-corrections" as an explicit cleanup rule |
| Monologue | Mode switches automatically by frontmost app |
| Spokenly | MCP voice input: a coding agent can ask the user a question and receive a spoken answer |

### A2. Open source

| Project | Stack and licence | Stars | Contributors | Last commit | Releases | Cloud / local | Beyond dictation |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **[cjpais/Handy](https://github.com/cjpais/Handy)** | Rust + Tauri 2, MIT | 40,050 | 193 | 24 Nov 2026 | 69; v0.10.2 on 24 Nov; every 1 to 3 weeks | Local only for STT. Optional LLM post-processing through OpenAI-compatible or Anthropic-format endpoints. | Transcription hook script, file transcription, CLI remote control, Raycast extension. Author calls it "roughly feature complete". |
| **[OpenWhispr/openwhispr](https://github.com/OpenWhispr/openwhispr)** | Electron (JavaScript, Node 24), MIT | 12,161 | 126 | 24 Nov 2026 | 98; v1.12.1 on 24 Nov; about weekly | Both; BYOK; paid cloud plans | Named voice agent, assistant hotkey with optional screenshot, meeting transcription with diarization, notes, team spaces, public API and MCP server, mobile app |
| **[altic-dev/FluidVoice](https://github.com/altic-dev/FluidVoice)** | Swift, GPL-3.0 | 13,244 | 58 | 24 Nov 2026 | 66; v1.6.13 on 13 Nov | Local first; optional cloud enhancement | Own speech models (Fluid Speech Pro, 0.63B parameters), local cleanup model, Command Mode (launch apps, run shortcuts), Edit Mode, per-app prompts, meeting transcripts |
| **[Beingpax/VoiceInk](https://github.com/Beingpax/VoiceInk)** | Swift, GPL-3.0 (LICENSE file read) | 7,207 | 50 | 11 Nov 2026 | 135; v2.27 on 11 Nov | Both | Per-app modes, screen context, dictionary, assistant mode. Paid binary: $25, $39 or $49 once [V: [tryvoiceink.com](https://tryvoiceink.com/)] |
| **[EpicenterHQ/epicenter](https://github.com/EpicenterHQ/epicenter)** (Whispering) | TypeScript + Tauri, **AGPL-3.0-or-later** (LICENSE file read) | 4,840 | 35 | 24 Nov 2026 | 72; last Whispering app release v7.11.0 on 27 Dec 2025 | Both, BYOK | Now one app inside a broader workspace project; repo was renamed and moved from `braden-w/whispering` (archived) |
| **[AbhishekBarali/SpeakoFlow](https://github.com/AbhishekBarali/SpeakoFlow)** | Rust + Tauri 2, MIT, dictation core from Handy | 407 | 6 | 31 Oct 2026 | 14; v2.2.0 on 21 Oct | Local default; cloud STT optional | Ask on selected text, opt-in screen vision, web search, reminders, spoken conversation, meetings (beta) |
| **[AssemblyAI/blurt](https://github.com/AssemblyAI/blurt)** | Swift (AppKit + SwiftUI), MIT | 83 | 18 | 24 Nov 2026 | 44; v0.1.71 on 19 Nov | Cloud only (AssemblyAI Dictation API, own key) | Styles, key terms, text shortcuts. Deliberately small. |
| **[zachlatta/freeflow](https://github.com/zachlatta/freeflow)** | Swift, MIT | 3,155 | n/a | 23 Nov 2026 | n/a | Cloud (Groq by default; any OpenAI-compatible URL) | Context-aware cleanup, Edit Mode, custom vocabulary |
| **[anomalyco/hex](https://github.com/anomalyco/hex)** | Rust, MIT (rewrite of the archived Swift Hex) | 8,538 | 25 | 24 Nov 2026 | 180 tags incl. previews | Local | Per-app output steps, local API and TypeScript SDK, screen annotation, meetings lab |
| **[tover0314-w/opentypeless](https://github.com/tover0314-w/opentypeless)** | Rust + Tauri, MIT | 703 | 5 | 23 Nov 2026 | 64; v1.1.64 on 11 Oct | Cloud first (Deepgram, AssemblyAI, Whisper-compatible, others) | App detector with per-app style mappings, Ask Anything, translation |
| **[Open-Less/openless](https://github.com/Open-Less/openless)** | Rust + Tauri, AGPL-3.0 | 4,165 | 71 | 24 Nov 2026 | 122 | Cloud, mostly Chinese realtime ASR providers | Streams polished text to the cursor character by character |
| **[kstonekuan/tambourine-voice](https://github.com/kstonekuan/tambourine-voice)** | Rust + Tauri client, Python Pipecat server, AGPL-3.0 | 396 | n/a | 21 Nov 2026 | n/a | Cloud or local | Pluggable STT and LLM providers |
| **[voquill/voquill](https://github.com/voquill/voquill)** | TypeScript + Tauri, licence not detected by GitHub | 1,032 | n/a | 22 Nov 2026 | n/a | Both | Writing styles, glossary, assistant |

All rows [V] via the GitHub API and each README on 24 Nov 2026. Contributor counts include anonymous contributors.

**Complaints from issues and Hacker News**

- Handy: "records words exactly as is" with no grammar cleanup; cut-off first words; no paragraph breaks; "too buggy" (one commenter). Most-reacted open issues: shortcut not working on Ubuntu (#102), "Pastes clipboard instead of spoken text" (#502). 162 open issues and pull requests (GitHub counts them together). [V: [HN 47040375](https://news.ycombinator.com/item?id=47040375), [HN 49809670](https://news.ycombinator.com/item?id=49809670), GitHub API]
- OpenWhispr: 514 open issues and pull requests. Most-reacted requests: Android, dual-language hotkey profiles (#233), a way to cancel a push-to-talk dictation (#1400), support for `gpt-live-transcribe` (#1427). [V: GitHub API]
- FluidVoice: default model has "a few other quirks" [V: HN 49809670].
- OpenSuperWhisper: "long startup time" [V: HN 49809670].
- Local pipelines with an LLM step: FreeFlow's author says a fully local version took 5 to 10 seconds per transcription against under 1 second with Groq; Handy's developer says end-to-end with post-processing "can still be under 3 seconds easily" [V: HN 47040375].

**One thing worth copying:** Handy's separation into small MIT crates (`handy-keys`, `handy-recorder`); Blurt's pre-warm plus upload-while-recording and its narrow context policy; FreeFlow's published cleanup prompt; OpenTypeless's app detector with per-app style mappings; OpenLess's streaming insertion with an automatic one-shot-paste fallback.

---

## B. Wispr Flow as of November 2026

### Feature set

| Area | Status | Source |
| --- | --- | --- |
| Dictation | Mac, Windows, iPhone, Android; 100+ languages claimed; 20-minute sessions; hands-free mode; language picker in the Flow Bar | [V: [whats-new](https://wisprflow.ai/whats-new)] |
| Own speech model | Canto-1 rolled out to everyone on 17 Sept 2026, replacing a setup that ran several recognizers per dictation and picked the best. Canto-2 published 23 Nov 2026, 10 times larger; Spanish WER 5.9% against 7.7% for Canto-1 on Wispr's own evaluation | [V: [canto-1-results](https://wisprflow.ai/canto-1-results), [canto-2](https://wisprflow.ai/canto-2)] |
| Voice commands | Command Mode: speak an instruction to rewrite selected text, draft at the cursor, or open a Perplexity search. Mac and Windows, paid plans, off by default under Settings, Experimental. No actions in other apps. | [V: [docs](https://docs.wisprflow.ai/articles/4816967992-how-to-use-command-mode)] |
| Transforms | AI rewrites of selected text (Polish, Prompt Engineer), beta since 1 May 2026 | [V: whats-new] |
| Meeting notes | Notetaker, no bot joins. Mac 5 Aug 2026, Windows 15 Sept, iPhone 19 Nov (in-person only). 21 languages. Imports from Otter and Granola. Android "on the roadmap". | [V: whats-new, pricing] |
| Calendar | Google Calendar and Outlook, used for meeting detection and speaker names | [V: whats-new] |
| Ask Wispr | Chat over meetings and notes. Drafts Gmail emails, Slack messages and Notion pages; "nothing is sent until you select Send". Reusable Markdown skills since 30 Oct 2026. | [V: [docs](https://docs.wisprflow.ai/articles/5694642921-ask-wispr-chat-with-your-meetings-and-notes-beta), whats-new] |
| MCP server | Launched with Notetaker. One-click setup for Claude, ChatGPT, Gemini. Connector lists 16 tools: meetings, calendar events, scratchpad notes, tasks; two write tools, `add_task` and `update_task`. "MCP grants access to meeting transcripts only, not dictations." | [V: whats-new, [claude.com/connectors/wispr-flow](https://claude.com/connectors/wispr-flow)] |
| Developer API | Documented REST and WebSocket transcription endpoints, API-key and client-token auth, a warm-up endpoint. Pricing and access terms not on the index page. | [V: [api-docs index](https://api-docs.wisprflow.ai/llms.txt)] |
| Per-app tone, dictionary, snippets | Styles by app, starred dictionary with auto-add, rich-text snippets, org-wide dictionary sharing | [V: whats-new] |
| Context awareness | App name, surrounding textbox content, and "relevant text data from the active app window" when Context Awareness is on | [V: [data-controls](https://wisprflow.ai/data-controls)] |
| Scheduled tasks, nudges | None found | [V: absent from whats-new and pricing] |
| Coding-agent dispatch | None. Terminal dictation for Claude Code and Codex, and file tagging in Cursor and Windsurf, are dictation features. | [V: whats-new, [developers](https://wisprflow.ai/developers)] |

### Direction

- November 2025 funding post: "build the voice OS", a "voice-native computing system" on its own foundation models [V: [new-funding](https://wisprflow.ai/new-funding)].
- August 2026: $280M led by Menlo Ventures at $2B; a research lab; a hardware partnership (a ring for quiet dictation); expansion "into new areas, such as meetings" [S: TechCrunch, link above].
- Canto-2 is framed as a step toward output that is "ready-to-send" [V].

Inference: Wispr is building from dictation toward a meeting-anchored assistant. Actions today are drafts that start from a meeting. There is no general "do this in Linear" command and nothing scheduled.

### Privacy and data handling

- Default: audio, transcripts and edits "may be used to evaluate, train, or improve AI models" if data sharing is allowed; the page does not state the default [V: data-controls]. Competitor write-ups say training is on by default for individual accounts [U].
- "Zero data retention agreements with all third-party AI providers", with an exception for some Notetaker features [V: data-controls].
- A third-party teardown of Mac version 1.4.752 (observed 4 April 2026) reports: audio streamed over gRPC during transcription together with app name, bundle ID, URL, Accessibility-tree text and textbox contents; a 694 MB local SQLite file with about 198 MB of audio; hourly metadata uploads continuing with data sharing off; Baseten inference endpoints; PostHog, Sentry, Segment and Datadog telemetry. No company response on the page. [S: [teardown](https://www.wensenwu.com/thoughts/wispr-flow-investigation)]
- Earlier episode: a user who reported screenshots being uploaded was banned, followed by a public apology from the company; dates conflict across sources [U].
- March 2026: Wispr's compliance vendor was accused of templated SOC 2 reports; Wispr later announced a new independent audit [U: competitor page citing TechCrunch and Wispr posts].

### Where it clearly does not go

| Gap | Evidence |
| --- | --- |
| Linux | Not on the pricing or platform lists [V] |
| Open source | Closed; terms reportedly prohibit reverse engineering [U] |
| Bring your own key, choice of model | No such option on pricing or docs pages I opened [V: absence] |
| Local or offline transcription | "Transcription always occurs on the cloud" [V] |
| Coding-agent dispatch, scheduled jobs | Not present [V: absence] |
| General actions through arbitrary MCP connectors | Wispr is an MCP *server*; I found no sign it is an MCP *client* for third-party tools [V: absence] |

---

## C. The technical recipe for sub-second dictation

### C1. What is published

- **Wispr's budget.** Text "within 700ms of when they stop speaking"; ASR inference under 200 ms; LLM inference under 200 ms; network at most 200 ms from anywhere [V: [technical-challenges, 11 Sept 2025](https://wisprflow.ai/post/technical-challenges)].
- **Wispr's earlier formatter.** Fine-tuned Llama on Baseten dedicated deployments with TensorRT-LLM; 100+ tokens in under 250 ms; the 700 ms figure is a p99 target [V: [Baseten case study](https://www.baseten.co/resources/customers/wispr-flow/)]. The speech side is now Canto [V]. Whether Llama on Baseten still does the formatting is not stated; the April 2026 teardown still saw Baseten endpoints [S].
- **Wispr streams while you speak.** Audio goes over gRPC during the dictation, with context attached [S: teardown].
- **Wispr's results.** Share of dictations taking over one second fell 54% on desktop and 37% on mobile after Canto-1; latency "down 30%" since the start of 2026 [V: canto-1-results, whats-new].
- **AssemblyAI's reference architecture.** Capture on key-down, pre-warm DNS, TCP and TLS, upload while the user is still speaking, transcribe and rewrite in one call, 5-second rewrite deadline with fallback to the verbatim transcript [V: [dictation-app-architecture](https://www.assemblyai.com/use-cases/dictation-app-architecture)]. A chart shows 0.36 s for short clips [V: [dictation-api](https://www.assemblyai.com/blog/dictation-api)].
- **Blurt** implements that: "pre-warms the HTTPS connection while you're still speaking and flips to 'transcribing' at key-up"; context is recent dictations plus the text just before the cursor [V: README].
- **LLM cleanup benchmark (vendor-run, synthetic data).** For a 30-second dictation: Gemini 2.5 Flash Lite scores 97 at 0.70 s median; Qwen3.5 4B Fast scores 81 at 0.28 s; Gemini 4 Argon scores 100 at 2.00 s [V: [AssemblyAI](https://www.assemblyai.com/blog/best-llms-dictation-cleanup)].

Inference, the recipe for push-to-talk:

1. Key release is the end-of-speech signal. Server-side turn detection (Deepgram Flux reports end-of-turn p90 of 1 s) is the wrong tool; send an explicit finalize or commit instead.
2. Open and warm the socket on key-down. Stream 16 kHz PCM while the key is held. At key-up only the last few hundred milliseconds remain to decode.
3. Send the final transcript to a small LLM with a short, cached system prompt: cleanup rules, dictionary, app name, text before the cursor. About 100 output tokens at 500 to 3,000 tokens per second is 30 to 200 ms of generation; time to first token dominates.
4. Skip the LLM for very short utterances. Fall back to the raw transcript on timeout.
5. Paste through the clipboard with save and restore; type instead of paste where secure input or the app requires it.

### C2. Streaming speech-to-text APIs

| Provider, model | Transport | End of speech to final transcript | Price | Spanish, English, mixing |
| --- | --- | --- | --- | --- |
| **OpenAI `gpt-live-transcribe`** (released 28 Jul 2026 [U]) | WebSocket `wss://api.openai.com/v1/realtime?intent=transcription`, or WebRTC. Client does VAD and sends `input_audio_buffer.commit`; server VAD not supported. | No ms figure; a `delay` setting from `minimal` to `xhigh` trades speed for accuracy | $0.017/min, estimated | `languages` array of hints; no detected-language output; mixing not documented. `keywords` and `prompt` biasing. [V: [guide](https://developers.openai.com/api/docs/guides/realtime-transcription), [pricing](https://developers.openai.com/api/docs/pricing)] |
| OpenAI `gpt-transcribe` | Same session type; transcribes after commit | Not stated | $0.0045/min; `gpt-4o-mini-transcribe` $0.003/min | Returns detected languages [V] |
| **Deepgram Nova-3** | WebSocket `/listen` | Not verified today | Streaming $0.0048/min monolingual, $0.0058/min multilingual; keyterm prompting +$0.0013/min; $200 free credit | Mixing with `language=multi`, hints such as `en`, `es`; docs recommend `endpointing=100`. 17.13% code-switched WER in AssemblyAI's test. [V: [pricing](https://deepgram.com/pricing), [docs](https://developers.deepgram.com/docs/multilingual-code-switching)] |
| Deepgram Flux | WebSocket `/v2/listen` | End-of-turn p90 1 s, p95 1.5 s (built for voice agents) | $0.0065/min English, $0.0078/min multilingual | 15.43% in the same test [V: [Flux post](https://deepgram.com/learn/introducing-flux-conversational-speech-recognition)] |
| **ElevenLabs Scribe v2 Realtime** (`scribe_v2_realtime`) | WebSocket | "Median of about 150 ms to a committed transcript" | $0.22/h; keyterms +$0.05/h | 90+ languages, automatic recognition. Mid-sentence switching is a feature of **Scribe v3 Live** (`multilang_mode=true`), which the official SDKs do not support yet; its price was not on the pages I opened. Scribe v3 Live: 8.32% in AssemblyAI's test. [V: [models](https://elevenlabs.io/docs/overview/models), [pricing](https://elevenlabs.io/pricing/api)] |
| **AssemblyAI-STT-3.7-Realtime** | WebSocket | Agent variant: final transcript "86 ms after the caller stops speaking" | $0.30/h, billed on how long the socket stays open; keyterms included (100 terms) | 6.67% code-switched WER on English paired with Spanish, German, French, Portuguese, Italian, in its own test [V: [pricing](https://www.assemblyai.com/pricing), [post, 19 Nov 2026](https://www.assemblyai.com/blog/real-time-transcription-code-switches-multilingual-speakers)] |
| AssemblyAI Dictation API | HTTP multipart to `dictation.assemblyai.com/v1/transcribe/live`, body read as it arrives | "Typically under a second" including cleanup | $0.62/h all in | 32 languages incl. Spanish; 120 s per call; Blurt's README says "code-switch mid-sentence" in 18 languages [V: [docs](https://www.assemblyai.com/docs/dictation.md)] |
| **Cartesia Ink-2** (released 13 Oct 2026) | Streaming with turn events | Not on the docs page; the 0.09 to 0.21 s figure is [U] | 3 credits per second, "$0.39 per hour on the Scale plan"; plans $0, $5, $49, $299 a month | English, French, Hindi, Japanese, Spanish; `ink-preview` has 44 languages; mixing not mentioned [V: [docs](https://docs.cartesia.ai/build-with-cartesia/stt-models/latest), [pricing](https://cartesia.ai/pricing)] |
| **Soniox `stt-rt-v6`** | WebSocket; final and non-final tokens; manual `finalize` message | No ms figure in docs; "sub-200ms" on marketing pages [U] | $0.12/h | `language_hints: ["en","es"]`, per-token language ID, context terms. Marketing claims mid-conversation Spanish-English switching [U]. 10.08% in AssemblyAI's test. [V: [docs](https://docs.soniox.com/stt/rt/real-time-transcription), [pricing](https://soniox.com/pricing)] |
| **Speechmatics** | Real-time API with partials and "Force end of utterance" | No numeric claim | Melia 1 $0.40/h; Oak 1 $0.46/h; Linden 1 $0.30/h (introductory); $100 free credit | Melia 1 transcribes 55+ languages without choosing one and "switches mid-sentence" [V: [pricing](https://www.speechmatics.com/pricing)] |
| **Groq Whisper** | HTTP upload after recording [U] | Not stated | `whisper-large-v3-turbo` $0.04/h; `whisper-large-v3` $0.111/h | Whisper "defaults to translation" on code-switched audio and ranks last of seven [V price: [Groq docs](https://console.groq.com/docs/models)], [S: ServiceNow benchmark below] |
| **Google Gemini 3.5 Transcribe Live** | Live API | Not stated | About $0.005/min audio in plus $0.004/min text out | 10.16% in AssemblyAI's test [V: [pricing](https://aistudio.google.com/docs/pricing)] |
| **Mistral Voxtral Realtime** | Streaming; open weights | "Sub-200ms" | $0.006/min; a later realtime model at $0.008/min | 13 languages [U: all snippets; Mistral's pricing page showed no speech rates today] |
| Aqua Avalon API | Synchronous HTTP; streaming not mentioned | Not stated | Dictation $0.49/h; transcription $0.39/h | 50 languages for dictation, best in 13 incl. Spanish [V: [avalon-api](https://aquavoice.com/avalon-api)] |

**Independent evidence on Spanish-English mixing.** A June 2026 benchmark of seven models with a 259-record Spanish-English set: ElevenLabs Scribe v2 and AssemblyAI Universal-3 Pro "tied on Spanish-English" at the top; Whisper Large V3 Turbo last, because it tends to translate into English unless a language is forced; errors concentrate on the English segments [S: [ServiceNow on Hugging Face](https://huggingface.co/blog/ServiceNow-AI/code-switching)]. It does not separate streaming from batch and does not include Soniox, Speechmatics or OpenAI.

Inference: for mixed Spanish and English, shortlist AssemblyAI realtime, ElevenLabs (Scribe v3 Live), Soniox and Speechmatics Melia 1, and test with your own voice. Avoid Whisper-family models, including Groq-hosted Whisper, for this input. Every cross-vendor number above except the ServiceNow one is published by a competitor.

### C3. Small, fast LLMs for cleanup

| Host, model | Tokens per second | Price per million tokens, in / out | Source |
| --- | --- | --- | --- |
| Groq `openai/gpt-oss-20b` | 1,000 | $0.075 / $0.30 | [V: [Groq docs](https://console.groq.com/docs/models)] |
| Groq `openai/gpt-oss-120b` | 500 | $0.15 / $0.60 | [V] |
| Groq `llama-3.1-8b-instant` | 560 | "Contact Sales" as rendered today | [V] |
| Cerebras `gpt-oss-120b` | about 3,000 | $0.35 / $0.75 | Speed [V: [Cerebras docs](https://inference-docs.cerebras.ai/models/overview)]; price [U] |
| Cerebras `qwen-3.8-27b` | about 1,850 | $0.99 / $1.49 | Speed [V]; price [U] |
| OpenAI `gpt-5-nano` | not verified | $0.05 / $0.40 | [V: [pricing](https://developers.openai.com/api/docs/pricing)] |
| OpenAI `gpt-6-luna` | not verified | $0.10 / $0.50; fast tier $0.20 / $1.00 | [V] |
| Anthropic Claude Haiku 5.5 | not verified | $0.10 / $0.50 up to 100k-token prompts; cache read $0.01 | [V: [pricing](https://platform.claude.com/docs/en/about-claude/pricing)] |
| Google Gemini 2.5 Flash-Lite | not verified | $0.10 / $0.40 | [V: [pricing](https://aistudio.google.com/docs/pricing)] |
| Google Gemini 3.1 Flash-Lite | not verified | $0.25 / $1.50 | [V] |

Time to first token: I could not verify a figure for any provider today. This is the number the Phase 0 speed test has to measure, from the user's own network.

---

## D. Who streams while the user is speaking

| Project | Streams during speech? | Evidence |
| --- | --- | --- |
| OpenWhispr | Yes, cloud. Streaming helpers for Deepgram, AssemblyAI, ElevenLabs, Corti; changelog entries for Gemini Live and OpenAI Realtime; its own cloud model cleans up "in the same streaming session" | [V: repo tree and CHANGELOG] |
| VoiceInk | Yes, cloud. `StreamingTranscriptionService.swift` plus streaming providers for AssemblyAI, Cartesia, Deepgram; ElevenLabs and Soniox providers | [V: repo tree] |
| FreeFlow | Yes. `RealtimeTranscriptionService.swift` | [V: repo tree] |
| SpeakoFlow | Yes. "ElevenLabs, Deepgram, and Amazon Transcribe stream text while you talk"; `stt_cloud_stream.rs` on `tokio-tungstenite` | [V: README, Cargo.toml] |
| OpenTypeless | Yes. Deepgram and AssemblyAI modules; "connection reuse and warm-up" | [V: README, repo tree] |
| Tambourine | Yes, through a Pipecat server over WebRTC | [V: README] |
| OpenLess | Yes, to Chinese realtime ASR providers; also streams polished text into the cursor | [V: README] |
| Blurt | Uploads the HTTP body while recording; not a WebSocket stream | [V: AssemblyAI docs, README] |
| Handy | Local streaming only. "Streaming model support" since v0.9.0 (1 Jul 2026); `managers/transcription/live.rs` decodes as audio arrives when the chosen model supports it. No cloud STT. | [V: release notes, repo tree] |
| Hex, FluidVoice | Local streaming with streaming-capable models ("writes while you talk"; "Live Preview") | [V: READMEs] |
| Whispering (Epicenter) | No streaming files found by name in `apps/whispering` | [V: repo tree; weak evidence] |

**Does any open-source project match Wispr Flow on both speed and formatting, according to users?** I found no user claiming that. In the two Hacker News threads I read (277 points and 24 points), no commenter compares any open-source app directly with Wispr Flow on speed or formatting [V]. The nearest evidence is the benchmark: Blurt at 78.0% zero-edit rate against Wispr's 79.3%, on US English, with no latency column [V]. Blurt is a client for a paid API, so it is "open-source client, closed model".

Inference: OpenWhispr streams and has cleanup, yet the developer found it slow. Streaming alone does not deliver the feel; the default provider, the cleanup step, Electron overhead and paste path all count. Measure each stage separately.

---

## E. Best codebase to borrow from

**Recommendation: Handy as the base, SpeakoFlow and OpenTypeless as references for the cloud path, Blurt for the latency tricks.**

| Need | Where it lives | Licence |
| --- | --- | --- |
| Global hotkeys, hold and toggle, lone modifiers | `handy-keys` crate ([handy-computer/handy-keys](https://github.com/handy-computer/handy-keys)); Handy `src-tauri/src/shortcut/` | MIT |
| Microphone capture | `handy-recorder` crate; Handy `managers/audio.rs` | MIT |
| Paste into the active app | Handy `clipboard.rs` (42 KB), `paste_tx/macos.rs`, `input.rs`, `secure_input.rs`; uses `enigo` and a `rdev` fork | MIT |
| Recording pill as an NSPanel | Handy `overlay.rs`; uses [`tauri-nspanel`](https://github.com/ahkohd/tauri-nspanel) | MIT; Apache-2.0 |
| LLM cleanup client | Handy `llm_client.rs` (OpenAI-compatible and Anthropic Messages formats) | MIT |
| WebSocket streaming to cloud STT | SpeakoFlow `stt_cloud_stream.rs`, `speech_stream.rs`; keys in the OS keychain through `keyring` | MIT |
| App detection and per-app tone | OpenTypeless `app_detector/`, `llm/prompt.rs`, `pipeline.rs` | MIT |
| Pre-warm, upload while recording, context policy | Blurt's `BlurtEngine` (Swift; read for design) | MIT |
| Cleanup prompt | FreeFlow README | MIT |

All [V] from the GitHub API and repo trees.

Why Handy: MIT; the same stack as planned (Rust, Tauri 2.11, SQLite through `rusqlite`); 193 contributors and a release every 1 to 3 weeks; the hard platform edges are already handled (secure input, Bluetooth microphones, clamshell mode, non-QWERTY paste, Accessibility permission loss). Caveats: it is local-first, so its transcription manager is built around on-device models and is the part to replace, not reuse; `lib.rs`, `actions.rs` and `settings.rs` are 50 to 80 KB files; it depends on a patched Tauri branch according to SpeakoFlow's Cargo comments.

SpeakoFlow is young (created 17 June 2026, 6 contributors, no push since 31 October) and vendors a modified `handy-keys`. Treat it as a worked example, not a dependency.

**Do not copy code from:** VoiceInk and FluidVoice (GPL-3.0), Epicenter/Whispering (AGPL-3.0-or-later), OpenLess, Tambourine, VoiceTypr (AGPL-3.0). Voquill's licence is unclear.

---

## Decision questions

### 1. Is dictation a commodity, or is Wispr-level quality still a moat?

Facts: three independent stacks sit within 2.5 points of each other at the top of the only cross-vendor benchmark, and one of them is a small MIT client over a public API. Cleaned dictation is sold as an API at $0.39 to $0.62 per audio hour. Apple, Google, OpenAI, Anthropic, Granola, Perplexity and Meta now ship dictation at no extra charge.

Inference: for one English-speaking user on a Mac, quality is a commodity. What remains hard is at the edges: p99 latency and uptime at scale, models conditioned on personal context (Canto-2 reports hard-term recall of 69.2% against 41.2% for Canto-1), mixed-language speech, mobile keyboards, and compliance. Only the mixed-language edge matters for this project. Dictation should be treated as table stakes for the copilot, not as its differentiator.

### 2. What can a solo developer reach in 3 to 4 weeks part-time?

Inference, reachable:
- Hold-to-talk hotkey, NSPanel pill, streaming cloud STT, finalize on key-up, small-LLM cleanup, clipboard-safe paste, built on Handy's crates.
- Personal dictionary passed as key terms; per-app tone from the frontmost app; text-before-cursor context.
- Median end-of-speech to text of roughly 500 to 900 ms in English on a good connection. This is an estimate from the component claims above, not a measurement.
- Idle memory under 100 MB is plausible on Tauri without local models.

Inference, out of reach in that time:
- Wispr-level consistency at p99 and its long tail of app-specific paste fixes (Handy still has 162 open issues and pull requests after 21 months).
- Learning from corrections, and context-conditioned speech recognition.
- Reliable Spanish-English mixing inside one sentence on every utterance.
- Mobile, Windows and Linux.

### 3. Monthly API cost for about one hour of dictation per day

Assumptions (mine): 30 audio hours a month; about 120 utterances per hour; about 600 prompt tokens and 100 output tokens per utterance, so roughly 2.5M input and 0.36M output tokens a month. Prices are the [V] list prices above.

| Pipeline | STT per month | LLM per month | Total |
| --- | --- | --- | --- |
| Soniox `stt-rt-v6` + Groq `gpt-oss-20b` | $3.60 | about $0.30 | **about $4** |
| ElevenLabs Scribe v2 Realtime with keyterms + Claude Haiku 5.5 | $8.10 | about $0.45 | about $8.50 |
| AssemblyAI-STT-3.7-Realtime + Gemini 2.5 Flash-Lite | $9.00 (socket-open time is billed) | about $0.40 | about $9.50 |
| Deepgram Nova-3 multilingual with keyterms + `gpt-5-nano` | $12.80 | about $0.30 | about $13 |
| AssemblyAI Dictation API, one call | $18.60 | included | about $19 |
| OpenAI `gpt-live-transcribe` + `gpt-6-luna` | $30.60 | about $0.45 | about $31 |
| Groq Whisper turbo + Groq LLM (upload after release; weak on mixed language) | $1.20 | about $0.30 | about $1.50 |

Subscriptions for comparison: Wispr Flow Pro $15 ($12 annual); Aqua Pro $12 ($10); Willow Pro $15 ($12); Monologue $15 ($12); Typeless Pro $30 ($12 annual); Superwhisper Pro $8.49; VoiceInk $25 once.

Caveats: whether the cheapest row meets the latency target is unmeasured, since Soniox publishes no finalization figure. The LLM cost is negligible in every row, so choose the cleanup model on latency and quality alone. One hour of actual speech a day is heavy use; cost scales linearly with audio.

### 4. Top 3 to copy, top 3 traps

**Copy**

1. **Stream while the key is held, pre-warm on key-down, finalize on key-up**, with a per-stage budget (Wispr: 200 + 200 + 200 ms). Log every stage per dictation, as AssemblyAI's `request_time_ms` does.
2. **Narrow, explicit context**: text before the cursor, app name, personal dictionary as key terms, per-app tone. Blurt's policy (never read password fields; nothing about the window beyond the preceding text) gets most of the benefit with little privacy cost.
3. **Never lose or silently change the user's words**: cleanup levels including "none", a hard LLM deadline with fallback to the verbatim transcript, and drafts that wait for Send before anything leaves the app (Ask Wispr's pattern, and already this project's rule).

**Traps**

1. **Licence contamination.** VoiceInk and FluidVoice are GPL-3.0; Epicenter/Whispering, OpenLess, Tambourine and VoiceTypr are AGPL-3.0. Read for ideas only.
2. **Whisper for mixed Spanish and English, and over-eager cleanup.** Whisper translates code-switched speech; Wispr's Auto Cleanup incident shows users notice immediately when words change.
3. **Letting dictation become the product.** OpenWhispr grew into agent, meetings, notes, teams, API and mobile and carries 514 open issues and pull requests. Wispr's privacy trouble came from collecting broadly (Accessibility trees, URLs, stored audio). Keep dictation small, collect little, and spend the time on commands, schedules and agent dispatch, where Wispr is absent.

---

## Corrections to the "already believed" list

| Belief | Finding |
| --- | --- |
| Handy "transcribes after key release" | Out of date. Streaming model support since v0.9.0 (July 2026), with a live preview. Still local-only for STT. 40,050 stars. [V] |
| SpeakoFlow about 349 stars | 407 stars. It adds cloud streaming (ElevenLabs, Deepgram, Amazon Transcribe), an assistant and meetings on top of Handy's core. [V] |
| Wispr Flow pipeline is ASR plus fine-tuned Llama on Baseten | That is the older description. Since Sept 2026 the speech model is Wispr's own Canto-1; Canto-2 was announced 23 Nov 2026. Whether Llama on Baseten still formats is not stated. The 700 ms figure is a budget and p99 target. [V] |
| Wispr Flow is dictation only | It has Notetaker, MCP server, tasks, Ask Wispr drafts, a developer API, Android and iOS. [V] |
| ElevenLabs Scribe v2 Realtime about 0.14 s | Docs say a median of about 150 ms to a committed transcript. Mid-sentence language switching belongs to the newer Scribe v3 Live. [V] |
| Cartesia Ink-2 at 0.09 to 0.21 s | Ink-2 exists (13 Oct 2026) but covers five languages in the stable model; I could not verify the latency figure. [V / U] |
| OpenAI `gpt-live-transcribe` as a streaming candidate | Confirmed, but it is the most expensive option at $0.017/min and needs client-side VAD and commit. `gpt-transcribe` at $0.0045/min transcribes after commit. [V] |
| Groq as a fast LLM host | Still fast (1,000 tokens/s on `gpt-oss-20b`), but the Llama models show "Contact Sales" on its models page today. [V] |
| Whispering/Epicenter | Repo moved to `EpicenterHQ/epicenter`; licence is now AGPL-3.0-or-later; last Whispering app release was December 2025. [V] |
| VoiceInk Mac only, GPLv3 | Confirmed GPL-3.0; the site also lists an iOS version. [V] |
| OpenWhispr Electron, MIT | Confirmed; 12,161 stars. [V] |

## Not covered or not resolved

- No independent latency benchmark exists; Voice Arena lists latency as "coming soon". Every latency number in this report is a vendor claim.
- Time to first token for the small LLMs was not verified.
- Kivi, Meta AI and Perplexity dictation were not researched beyond their benchmark rows.
- MacWhisper details are all from secondary sources.
- Voice Arena's independence is asserted by vendors that rank well on it; its scoring uses LLM judges and covers US English on macOS only.
