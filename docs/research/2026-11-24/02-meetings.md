# 02 · Meeting capture, notes and ambient memory: market and technical research

Date of research: 24 November 2026. All web pages and GitHub API calls were made on this date.

**How to read the marks**

- **[V]** = verified today at the linked primary source (vendor page, vendor docs, Apple docs, GitHub API or repository file).
- **[U]** = unverified: search snippet, secondary source, competitor blog, or memory. Treat as a lead, not a fact.
- "Inference:" marks my own reasoning. Everything else is reported fact.
- Prices are list prices seen today. Several pages showed Black Friday banners, which I ignored.

---

## 0. Short version

1. Bot-less meeting notes on a Mac is a commodity in November 2026. About ten commercial products and five MIT-licensed projects (plus source-available screenpipe) do the same capture: microphone plus system audio, no bot. Granola itself says notes are becoming a commodity and value is moving to actions ([TechCrunch, 25 Mar 2026](https://techcrunch.com/2026/03/25/granola-raises-125m-hits-1-5b-valuation-as-it-expands-from-meeting-notetaker-to-enterprise-ai-app/)) [V].
2. The step the copilot actually needs, "transcript in, proposed tickets with owner and quote out", is not solved well by anyone. Granola has no native tracker push; Wispr Flow's MCP is read-only; the open-source tools stop at a list of action items in a file.
3. Capture is easy to start and hard to make reliable. The three MIT codebases all carry large amounts of code for silent failure, device changes, echo and clock drift.
4. Cloud transcription with speaker labels costs USD 0.10 to 0.36 per meeting hour after the meeting, and 0.12 to 1.02 in real time. Money is not the constraint; engineering time and legal exposure are.
5. The Spanish regulator (AEPD) published guidance in January and April 2026 that asks for information before the session, a visible indicator during it and consent per session. Bot-less capture is invisible by design, so this lands on the person who records.

---

## 1. Corrections to the "already believed" list

| Belief | Status | Correction or addition |
| --- | --- | --- |
| Meetily: Rust + Tauri, MIT, mixes mic and system audio without a bot | Correct [V] | Speaker identification, MCP, CLI, webhooks, PDF/DOCX export and custom templates are **not** in the MIT edition. They are in Meetily PRO, which is commercial and closed ([meetily.ai/pro](https://meetily.ai/pro/), [README](https://github.com/Zackriya-Solutions/meetily)) [V]. Tauri 2.6.2; capture uses `cidre` pinned to a git revision, not a crates.io release ([Cargo.toml](https://github.com/Zackriya-Solutions/meetily/blob/main/frontend/src-tauri/Cargo.toml)) [V]. |
| Anarlog (ex-Hyprnote): Tauri v2, MIT core, separate enterprise licence, Core Audio tap crates, plugin SDK, CLI that is also an MCP server | Correct [V] | Three additions. (a) The team's main product is now **char** ([char.com](https://char.com), private beta); the README says Anarlog "is not being retired" ([README](https://github.com/fastrepl/anarlog)) [V]. (b) Some files are ports from the char codebase, which is GPL-3.0-or-later; Fastrepl authorised them under MIT on 19 Nov 2026 and records this in per-crate `PROVENANCE.md` files ([crates/audio/PROVENANCE.md](https://github.com/fastrepl/anarlog/blob/main/crates/audio/PROVENANCE.md)) [V]. (c) Licensing is by path: everything under `enterprise/` is commercial ([LICENSING.md](https://github.com/fastrepl/anarlog/blob/main/LICENSING.md)) [V]. |
| Minutes: Rust, MIT, Markdown files, MCP | Mostly correct [V] | The core is Rust, but macOS system-audio capture is a **Swift** helper ([system_audio_tap.swift](https://github.com/silverstein/minutes/blob/main/tauri/src-tauri/src/system_audio_tap.swift)) and the MCP server is **TypeScript** run with `npx minutes-mcp` ([llms.txt](https://useminutes.app/llms.txt)) [V]. The stand-alone CLI cannot capture system audio by itself: the docs say it "needs BlackHole and a Multi-Output Device" ([docs/features.md](https://github.com/silverstein/minutes/blob/main/docs/features.md)) [V]. The repository is only eight months old (created 18 Mar 2026) [V]. |
| Omi: Swift Mac app, MIT, cloud backend | Correct [V] | The repository's largest language is Python (backend); the Mac app is Swift/SwiftUI plus a Python desktop backend, and the quick start connects to Omi's cloud ([README](https://github.com/BasedHardware/omi)) [V]. It captures system audio with Core Audio taps, macOS 14.4+ ([SystemAudioCaptureService.swift](https://github.com/BasedHardware/omi/blob/main/desktop/macos/Desktop/Sources/SystemAudioCaptureService.swift)) [V]. |
| Core Audio process taps capture system audio without the Screen Recording permission; `cidre` wraps them | Correct, with caveats | Taps still need their own permission: the `NSAudioCaptureUsageDescription` key and a system prompt on first use ([Apple docs](https://developer.apple.com/documentation/coreaudio/capturing-system-audio-with-core-audio-taps)) [V]. If the user refuses, the API returns success and delivers silence, and there is no public API to check the state ([Apple forum thread 771864](https://developer.apple.com/forums/thread/771864)) [V, the second half is a developer's statement, not Apple's]. `cidre` is not the only Rust route: `cpal` gained macOS loopback capture in 0.17.0 (20 Dec 2025) and fixed three silent-failure bugs in 0.18.0 ([CHANGELOG](https://github.com/RustAudio/cpal/blob/master/CHANGELOG.md)) [V]. |

Renamed, acquired or shut down:

- **Hyprnote → Anarlog** (the old GitHub path redirects) [V].
- **Rewind / Limitless: effectively gone for a user in Spain.** Acquired by Meta; Pendant sales ended 5 Dec 2025; service ended in the EU and UK on 5 Dec 2025; Rewind's screen and audio capture was disabled from 19 Dec 2025 ([limitless.ai](https://www.limitless.ai/)) [V].
- **screenpipe is not open source.** `LICENSE.md` is the "Screenpipe Commercial License": free for personal non-commercial use, 7-day evaluation otherwise ([LICENSE.md](https://github.com/screenpipe/screenpipe/blob/main/LICENSE.md)) [V]. The repository moved from `mediar-ai/screenpipe` to `screenpipe/screenpipe` [V]. Do not borrow code from it.
- **Fathom** moved from fathom.video to fathom.ai [V].

---

## A. Product landscape

### A1. Bot-less commercial products

| Product | Capture on macOS; permissions | Platforms | Price (list) | Transcription | Languages; mixed | Where output goes | Complaints | Worth copying |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **Granola** | Mic + system audio, no bot. Needs Microphone and "Screen & System Audio Recording" ([docs](https://docs.granola.ai/help-center/troubleshooting/transcription-issues)) [V] | macOS, Windows, iOS, Android (1 Jul 2026), Apple Watch (28 Jul 2026) ([updates](https://www.granola.ai/updates)) [V] | Basic $0 (30-day history), Business $14, Enterprise $35 per user/month ([pricing](https://granola.ai/pricing)) [V] | Cloud; audio deleted after transcription ([llms.txt](https://granola.ai/llms.txt)) [V] | 31 desktop, 17 mobile; desktop "Auto" handles several languages in one meeting; mobile picks one ([docs](https://docs.granola.ai/help-center/customising-granola/multi-language)) [V] | Slack, Notion, HubSpot, Affinity, Attio, Zapier; remote MCP; REST API on Business+ ([MCP docs](https://docs.granola.ai/help-center/sharing/integrations/mcp), [API docs](https://docs.granola.ai/introduction)) [V]. No native Linear/Jira push [U] | No audio playback; "Me/Them" labels outside Meet and Zoom-on-Mac; recordings that stop silently; Feb 2026 free-plan change [U] | Your own typed notes steer the summary; chat answers carry citations |
| **Wispr Flow Notetaker** | "Without a bot joining the call"; "if your computer can play it, Notetaker can capture it" ([notetaker page](https://wisprflow.ai/notetaker)) [V] | Mac, Windows, iPhone; Android "in 2027" [V] | Included in Flow. Free (weekly meeting limit, 30-day history), Pro $15 or $12 annual ([pricing](https://wisprflow.ai/pricing)) [V] | Cloud; audio "stored only temporarily"; "does not create voiceprints" [V] | 21 languages including Spanish; mixed-language behaviour not stated [V] | Remote MCP, **read-only**; no native tracker push ([MCP docs](https://docs.wisprflow.ai/articles/9551372685-connect-an-mcp-client-to-wispr-flow-remote-mcp-server)) [V] | Not researched | Speaker names from the calendar invite and personal dictionary; a second AI pass over the transcript after the meeting |
| **Notion AI Meeting Notes** | Desktop app: mic + system audio. Browser: mic only ([help](https://www.notion.com/help/ai-meeting-notes)) [V] | macOS 13+, Windows, mobile, web [V] | Business €19.50 per member/month; Free and Plus get a limited trial ([pricing](https://www.notion.com/pricing)) [V] | Cloud; local audio storage off by default; 10 h/day cap [V] | Speaker labels in 23 languages [V] | Notion page; built-in "Create follow-up tasks" skill [V] | Browser mode misses the other side when on headphones [U] | **Consent message** as a product feature: chat text, voice message, or forced by the workspace owner |
| **ChatGPT Record** | Mic + system audio; needs Microphone and "Screen & System Audio Recording" ([help](https://help.openai.com/en/articles/11487532-chatgpt-record)) [V] | macOS desktop app only [V] | Included in Plus, Pro, Business, Enterprise, Edu [V] | Cloud; audio deleted after transcription; 4 h cap per session [V] | "Works best in English today" [V] | A canvas in chat history [V] | English-centred; no tracker push [V/inference] | Nothing specific |
| **Krisp** | Bot-free (also offers a bot) ([pricing](https://krisp.ai/pricing/)) [V]; installs virtual mic and speaker devices [U] | macOS, Windows, mobile [U] | Core $16 or $8 annual; Advanced $30 or $15 annual; 7-day trial, no permanent free tier [V] | English on-device, others in cloud [U] | "16+ langs" [V] | MCP (Claude, ChatGPT, Cursor), API, webhooks, HubSpot, Slack, Notion, Zapier [V] | Not researched | Noise removal and notes share one audio pipeline |
| **Jamie** | Device capture, no bot [U] | macOS, Windows, iOS [U] | Free €0 (10 notes/month, 30 min each), Plus €21, Pro €39 annual, Team €33 per seat ([pricing](https://www.meetjamie.ai/pricing)) [V] | Cloud, "hosted in the EU" [V] | "99+ languages" [V] | MCP, API and webhooks only on Pro; Notion, Google Docs, OneNote on lower plans [V] | Low caps on free and Plus [V] | EU hosting as the headline; also sells live diarisation as an API at $0.04 per audio hour [V] |
| **Circleback** | Bot **and** a bot-free desktop app; records video too ([site](https://circleback.ai/)) [V] | macOS, Windows [V] | Free $0, Pro $15, Business $25, Enterprise $35 ([pricing](https://circleback.ai/pricing)) [V] | Cloud | "Over 100 languages" [V] | Action items "captured, assigned, and organized"; automations to Linear, Notion, Slack, HubSpot, Asana and others; API, MCP and CLI on the free plan [V]. Jira not listed [V] | Not researched | **The closest product to "meeting ends, assigned action items go to the tracker"** |
| **Fathom** | Bot by default; bot-free capture in beta ([pricing](https://fathom.ai/pricing)) [V] | macOS, Windows | Free forever; Premium $20 or $16 annual [V] | Cloud | Count not stated | MCP on Premium+; CRM sync on Business [V] | Not researched | Generous free tier |
| **Zoom My Notes** | "Zoom, Teams, Google Meet, and other third-party meeting platforms, no bots required" ([Zoom](https://www.zoom.com/en/products/ai-assistant/features/ai-note-taking/)) [V] | Zoom Workplace app | Included in paid Zoom Workplace; free tier 3 uses/month [V] | Cloud | Not stated | Zoom Workflows: recap email, create tasks [V] | Not researched | Shows that platform vendors now ship bot-less capture for competitors' calls |
| **Talat** | Core Audio taps [U, from TechCrunch's description of the developer's earlier work] | Mac, Apple Silicon | One-time $49 pre-release, $99 at 1.0 ([TechCrunch, 24 Mar 2026](https://techcrunch.com/2026/03/24/talats-ai-meeting-notes-stay-on-your-machine-not-in-the-cloud/)) [V] | Local: FluidAudio with Parakeet; local Qwen summary [V] | Not stated | Obsidian export, webhook on meeting end, MCP server [V] | Not researched | **Webhook when the meeting finishes**; about 20 MB app |

### A2. Open source (GitHub API, 24 Nov 2026) [V]

| Project | Stars | Contributors | Last commit | Latest release | Licence | Stack | Capture on macOS |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [Meetily](https://github.com/Zackriya-Solutions/meetily) | 33,018 | 19 | 14 Nov 2026 | v0.5.0, 14 Nov 2026 | MIT (community) + closed PRO | Rust, Tauri 2.6, Next.js | Global mono Core Audio tap through `cidre` ([core_audio.rs](https://github.com/Zackriya-Solutions/meetily/blob/main/frontend/src-tauri/src/audio/capture/core_audio.rs)) |
| [Anarlog](https://github.com/fastrepl/anarlog) | 9,984 | 43 | 23 Nov 2026 | desktop v1.4.35, 15 Nov 2026 | MIT, `enterprise/` commercial | Rust, Tauri v2, React | Global mono tap + private aggregate device through `cidre` ([speaker/macos.rs](https://github.com/fastrepl/anarlog/blob/main/crates/audio-actual/src/speaker/macos.rs)); own echo canceller |
| [Minutes](https://github.com/silverstein/minutes) | 1,618 | 43 | 24 Nov 2026 | v0.34.2, 23 Nov 2026 | MIT | Rust core, Tauri, Swift helper, TypeScript MCP | Swift helper: tap first, ScreenCaptureKit as fallback and as a check |
| [Omi](https://github.com/BasedHardware/omi) | 14,020 | 350 | 24 Nov 2026 | macOS v0.12.582, 24 Nov 2026 | MIT | Swift app, Python and Go backends, Flutter mobile | Core Audio taps, macOS 14.4+ |
| [screenpipe](https://github.com/screenpipe/screenpipe) | 22,530 | 166 | 24 Nov 2026 | app v2.7.153, 23 Nov 2026 | **Commercial, source-available** | Rust, Tauri | ScreenCaptureKit plus a process-tap module |
| [Quill](https://github.com/humanitas-labs/quill) | 3,968 | not counted | 27 Sep 2026 | not checked | MIT | Swift | Core Audio taps, macOS 15+; Parakeet v2, English only ([README](https://github.com/humanitas-labs/quill/blob/master/macos/README.md)) |

Per project:

- **Meetily.** Local Whisper or Parakeet (the README credits the Parakeet TDT 0.6b v3 ONNX conversion); summaries through Ollama, Claude, Groq, OpenRouter or any OpenAI-compatible endpoint [V]. PRO is $10 per user/month billed annually [V]. Languages are not listed [V]. Worth copying: the single-file `cidre` capture, which is the easiest to read.
- **Anarlog.** Free plan includes local API, CLI, MCP and webhooks; Pro is $14 per person/month and adds cloud transcription, sync and "Better Speaker Identification" ([pricing](https://anarlog.so/pricing)) [V]. Managed cloud prefers Soniox for transcription and Claude Sonnet through OpenRouter for summaries [V]. MCP tools include `get_meeting` (returns action items), `get_meeting_transcript`, `export_meeting`, and `propose_summary_edit` / `propose_memo_edit`, where "the proposal stays pending until a human applies it" ([apps/cli/src/mcp.rs](https://github.com/fastrepl/anarlog/blob/main/apps/cli/src/mcp.rs)) [V]. Worth copying: that **proposal pattern**, and the echo canceller.
- **Minutes.** Meetings are Markdown with YAML frontmatter that already holds `action_items` with `assignee`, `task`, `due`, `status`, plus `decisions` ([README](https://github.com/silverstein/minutes)) [V]. 34 MCP tools, including `track_commitments`, `get_meeting_insights`, and the resource `minutes://actions/open` [V]. Meetings marked `sensitivity: restricted` are excluded from every agent surface by default, with a logged override ([consent-enforcement.md](https://github.com/silverstein/minutes/blob/main/docs/architecture/consent-enforcement.md)) [V]. Local diarisation uses pyannote-rs, pyannote segmentation-3.0 and WeSpeaker CAM++ ([NOTICE](https://github.com/silverstein/minutes/blob/main/NOTICE)) [V]. Worth copying: the file format and the restricted flag.
- **Omi.** Wearable first; the Mac app is one client of a cloud backend with speech-to-text and a GPU diariser [V]. Has an MCP server directory [V]. Prices not verified: third-party pages disagree ($19 and $29.99 per month for unlimited) [U].
- **screenpipe.** Free, Basic $21, Business $42 per seat/month ([pricing](https://screenpipe.com/pricing)) [V]. Its own README states 0.5 to 3 GB RAM and 5 to 20% CPU for capture [V], which is far from a 100 MB idle budget. Worth copying as an idea only: an agent-facing local API over captured history.

### A3. Bot-based products (brief)

| Product | Bot-less option | Price | Agent access | Source |
| --- | --- | --- | --- | --- |
| **Otter** | Not shown on the pricing page; reported as bot-first [U] | Basic free (300 min/month, 30 min per conversation); Pro $16.99 or $8.33 annual; Business $30 or $19.99 annual | "Otter MCP server" listed; API and webhooks on Enterprise | [pricing](https://otter.ai/pricing) [V] |
| **Fireflies** | Desktop app and "Chrome extension without Fireflies bot" listed | Prices not on the fetched page | API access listed | [pricing](https://fireflies.ai/pricing) [V] |
| **tl;dv** | Not stated on the pricing page | Pro €29 or €18 annual; Business €39 or €29 annual | Not stated | [pricing](https://tldv.io/pricing/) [V] |
| **Claude Meetings** (Anthropic) | **No.** Claude joins Google Meet "as a visible participant" and posts a chat notice | Beta, included in Team and Enterprise | Notes, transcript, action items in claude.ai; no audio kept | [help article](https://support.claude.com/en/articles/18325154-use-claude-meetings) [V] |
| **Google "Take notes for me"** | In-person capture since Aug 2026; expansion to Zoom/Teams calls reported but not confirmed | Workspace Business/Enterprise Standard and Plus, Google AI Pro and Ultra | Google Doc in Drive | [Workspace Updates, 13 Aug 2026](https://workspaceupdates.googleblog.com/2026/08/take-notes-with-me-for-in-person-meetings-is-now-available.html) [V]; Zoom/Teams part [U] |

Claude desktop equivalent: none shipped. A Mac recorder codenamed "Parka" was reported from leaked build files and is not public ([Reworked](https://www.reworked.co/digital-workplace/anthropic-explores-meeting-recorder-that-assigns-tasks-to-claude-agents/)) [U]. Claude Meetings, launched in beta on 10 to 12 November 2026, is bot-based, Google Meet only, and Team/Enterprise only [V].

### A4. Apple built-in

- macOS 26 can record calls in the **Phone app and FaceTime audio** and shows summaries in Notes under "Call Recordings"; it needs Apple Intelligence and is "not available in all countries and regions" ([Mac User Guide](https://support.apple.com/guide/mac-help/mchld69597ca/26/mac/26)) [V]. It does not record Zoom, Meet or Teams. Inference: this is not a meeting notetaker.
- `SpeechAnalyzer` and `SpeechTranscriber` are public APIs from macOS 26.0 ([Apple docs](https://developer.apple.com/documentation/speech/speechanalyzer)) [V]. Anarlog already has a `transcribe-speechanalyzer` crate with a Swift bridge [V]. Whether they provide speaker labels: not confirmed [U].

### A5. Ambient capture and memory

| Product | Status in Nov 2026 | Source |
| --- | --- | --- |
| Rewind / Limitless | Acquired by Meta; shut in the EU and UK | [limitless.ai](https://www.limitless.ai/) [V] |
| screenpipe | Active, YC S26, repositioned as "finds the work worth automating"; commercial licence | [README](https://github.com/screenpipe/screenpipe) [V] |
| Omi | Active, MIT, hardware + apps + cloud | [README](https://github.com/BasedHardware/omi) [V] |
| Bee (Amazon) | New hardware on 15 Oct 2026 from $74.99; "Bee Pro" at $11.99/month reportedly adds desktop meeting capture | [TechCrunch](https://techcrunch.com/2026/10/15/amazons-bee-launches-two-new-personal-AI-devices-starting-at-75) [U, not opened] |
| Recall.ai Desktop Recording SDK | A paid building block: $0.50 per recording hour, plus $0.15 per hour for transcription | [pricing](https://recall.ai/pricing) [V] |

Inference: always-on ambient capture is the wrong fit here. It conflicts with the idle budgets (screenpipe's own numbers), and with the AEPD's requirement that recording stops when the session ends (section D).

---

## B. Granola in November 2026

**Funding.** Series C of $125M at a $1.5B valuation, led by Index Ventures with Kleiner Perkins; total raised $192M; previous round $43M at $250M in May 2025 ([TechCrunch, 25 Mar 2026](https://techcrunch.com/2026/03/25/granola-raises-125m-hits-1-5b-valuation-as-it-expands-from-meeting-notetaker-to-enterprise-ai-app/)) [V]. Granola's own `llms.txt` still says $67.2M, so that file is stale [V].

**Pricing.** Basic $0 with 30-day history, Business $14, Enterprise $35 ([pricing](https://granola.ai/pricing)) [V]. Third-party pages say a February 2026 change retired the old individual plan and introduced the 30-day limit on free, with user backlash [U].

**2026 timeline** ([updates page](https://www.granola.ai/updates)) [V]:

| Date | Release |
| --- | --- |
| 15 Jan | Sign in with Microsoft |
| 28 Jan | Delete parts of a transcript |
| 4 Feb | Granola MCP |
| 21 Apr | Chat rebuilt as "agentic", with search across notes, Team Space and shared items, and inline citations |
| 20 May | Briefs (pre-meeting summary) on Mac and Windows |
| 1 Jul | Android |
| 28 Jul | Apple Watch |
| 2 Sep | 32 languages |
| 11 Nov | Agent brief: a nightly summary of your work for connected agents |

**MCP** ([docs](https://docs.granola.ai/help-center/sharing/integrations/mcp)) [V]: remote server at `https://mcp.granola.ai/mcp`, Streamable HTTP, OAuth 2.0 with dynamic client registration, no API keys. Read tools include `search_meetings`, `get_meetings`, `get_meeting_transcript`, `search_transcripts`, `query_granola_meetings`. Paid plans also get **write** tools (`update_summary`, `update_private_notes`, `create_folder`, `generate_summary`). Transcript tools are paid only. Free is limited to the last 30 days.

**API** ([docs](https://docs.granola.ai/introduction)) [V]: `https://public-api.granola.ai/v1`, Bearer key, `GET /v1/notes`, `GET /v1/notes/{id}/transcript`, `POST /v1/notes/transcripts/search`; Business and Enterprise; 5 requests per second. No webhooks mentioned.

**Direction.** Granola presents itself as the context layer for other agents (MCP, API, Agent brief) and not as the thing that files tickets. TechCrunch reports the company's view that notes are becoming a commodity and value is shifting to actions [V]. There is no changelog entry in 2026 about tasks or trackers [V]. Inference: Granola is leaving "create the ticket" to whoever consumes its MCP, which is the gap this copilot would sit in.

**A warning from Granola's year.** Users complained that Granola locked down its local database and broke their local agent workflows; the co-founder said the cache "was not designed for AI workflows" and promised bulk APIs ([TechCrunch](https://techcrunch.com/2026/03/25/granola-raises-125m-hits-1-5b-valuation-as-it-expands-from-meeting-notetaker-to-enterprise-ai-app/)) [V]. Inference: reading a vendor's local cache is not a stable integration; use the documented MCP or API.

**How commoditised is bot-less capture?** Verified today as shipping bot-less capture: Granola, Wispr Flow, Notion, ChatGPT, Krisp, Circleback, Fathom (beta), Zoom My Notes, Talat, plus Meetily, Anarlog, Minutes, Omi and Quill in open source. Free tiers exist at Granola, Wispr Flow, Circleback, Fathom, Jamie and Zoom. Inference: capture and a summary are worth about zero as a differentiator; the entry price for a good one is $0 to $15 per month.

---

## C. Technical

### C1. System audio on macOS: the two routes

| | Core Audio process tap | ScreenCaptureKit (SCK) audio |
| --- | --- | --- |
| Minimum macOS | 14.2 per Apple's sample; projects use 14.4 as the floor (Omi, Minutes, Meetily, AudioCap) [V] | 13.0 for audio [U] |
| Info.plist | `NSAudioCaptureUsageDescription` ([Apple](https://developer.apple.com/documentation/coreaudio/capturing-system-audio-with-core-audio-taps)) [V] | Screen capture usage description [U] |
| Permission | Its own audio-only grant, shown as "System Audio Recording Only" ([diarize PR](https://github.com/elien666/diarize/pull/3), [SystemAudioKit](https://github.com/pieralukasz/SystemAudioKit)) [V as third-party statements]. Apple's user guide confirms apps can be allowed "to record both your screen and audio, or just your audio" ([Apple](https://support.apple.com/en-ph/guide/mac-help/control-access-screen-system-audio-recording-mchld6aa7d23/26/mac/26)) [V] | Full "Screen & System Audio Recording" |
| When the prompt appears | "The first time it starts recording from an aggregate device that contains a Core Audio tap" (Apple engineer, [forum](https://developer.apple.com/forums/thread/771864)) [V] | On first capture |
| If refused | Calls succeed, buffers are zeros; no public API to query the state [V, developer reports] | Capture fails or is empty [U] |
| Indicator | Quill's README says "macOS shows the purple recording indicator" while it records with a tap ([README](https://github.com/humanitas-labs/quill/blob/master/macos/README.md)) [V as a third-party statement]. Apple's pages do not document it [V]. **Test this on the target macOS.** | Screen-recording indicator in the menu bar [U]; periodic re-approval prompts on recent macOS [U, memory] |
| Hears FaceTime and iPhone calls on the Mac | Yes | **No**: those are rendered by system daemons, and SCK "delivers frames that are digitally silent for the whole call" (comment in [Minutes source](https://github.com/silverstein/minutes/blob/main/tauri/src-tauri/src/system_audio_tap.swift)) [V] |

The microphone is a separate permission with the orange indicator in both cases [U].

Granola, Notion and ChatGPT all tell users to enable "Screen & System Audio Recording" [V]. Which API they call underneath is not documented.

### C2. Pitfalls found in real code and docs

1. **Silence that looks like success.** A refused tap returns zeros. Minutes runs SCK in parallel as a "witness" and switches source if SCK hears audio the tap does not [V]. cpal 0.18.0 fixed "loopback capture returning silence due to disabled tap auto-start" [V]. One developer blog lists further silent cases: changing `isExclusive` after construction, an empty sub-device list, and a nil dispatch queue on macOS 26 ([DGR Labs, Apr 2026](https://dgrlabs.co/blog/2026-04-25-capturing-system-audio-on-macos-in-2026.html)) [V as a third-party report].
2. **OS upgrades.** One project reported that after moving to macOS 27 its tap stopped working until the audio-capture permission was declared and granted again ([diarize PR, 26 Oct 2026](https://github.com/elien666/diarize/pull/3)) [V as a single report]. Apple's support site lists "macOS 27 Golden Gate" as current [V].
3. **Unsigned builds do not trigger the prompt**, so capture cannot be tested end to end without signing (DGR Labs) [V as a third-party report].
4. **Device changes.** The tap must be rebuilt when the default output changes, for example when headphones connect; SystemAudioKit pads the gap with silence to keep tracks aligned [V].
5. **Echo.** With loudspeakers, the microphone hears the call and every remote sentence is transcribed twice. Granola says it "normally filters this echo out" and still fails with the Teams desktop app on Windows [V]. Apple's voice processing removes echo but "ducks every other app's audio, the call included" (comment in [Anarlog echo.rs](https://github.com/fastrepl/anarlog/blob/main/crates/audio-actual/src/native_meeting/echo.rs)) [V]. Anarlog therefore ships its own ONNX echo canceller with reference alignment, adding 24 ms of delay [V].
6. **Bluetooth headsets.** "They can't handle high-quality audio output and microphone input at the same time"; Granola suggests the built-in microphone as input ([Granola docs](https://docs.granola.ai/help-center/troubleshooting/transcription-issues)) [V].
7. **Sample rate and drift.** Tap and microphone run on different clocks and often different rates. Anarlog has separate crates for resampling and drift (`resampler`, `audio-sync`); both Anarlog and Meetily re-read the tap's sample rate during capture [V]. Anarlog and SystemAudioKit place both tracks on the host clock [V].
8. **A global tap records everything**, including notification sounds and music (Quill README) [V]. Per-process taps avoid that but must include browser helper processes (SystemAudioKit) [V].
9. **Mute sync.** Granola stops transcribing your microphone when you mute in Zoom or Teams, and warns this breaks hybrid meetings [V].

### C3. Rust crates

| Crate | What it is | State today | Licence |
| --- | --- | --- | --- |
| [cidre](https://github.com/yury/cidre) | Bindings to Apple frameworks, including Core Audio taps. Used by Anarlog and Meetily [V] | crates.io 0.30.1 (13 Nov 2026); 226 stars, 12 contributors, last commit 23 Nov 2026; no GitHub releases [V]. Inference: one main author and pre-1.0, so pin a version. | MIT |
| [cpal](https://github.com/RustAudio/cpal) | Cross-platform audio I/O. "Support for loopback recording … on macOS > 14.6" since 0.17.0 [V] | 0.18.2 (16 Aug 2026); 4,031 stars [V] | Apache-2.0 |
| [screencapturekit](https://github.com/doom-fish/screencapturekit-rs) | SCK bindings | crates.io 12.0.0 (21 Nov 2026); 245 stars [V]. Inference: twelve major versions means frequent breaking changes. | Apache-2.0 |
| [flexaudio](https://github.com/Studio-Sadola/flexaudio) | Mic, system loopback and per-process capture behind one API | 0.8.2 (24 Nov 2026), 10 stars [V]. Too young to depend on. | MIT |
| Swift references | [AudioCap](https://github.com/insidegui/AudioCap) (BSD-2-Clause, 513 stars, last push Aug 2025), [SystemAudioKit](https://github.com/pieralukasz/SystemAudioKit) (MIT, new) [V] | Read for the permission and aggregate-device logic | |

Inference: for a Rust core with few dependencies, `cpal` ≥ 0.18 for both microphone and loopback is the smallest surface. `cidre` gives more control (mono global tap, exclusions, private aggregate device) and has two production users to read.

### C4. Speaker labels

The cheapest option is not diarisation. Keep microphone and system audio as two tracks: microphone is "me", system is "them". Quill calls this "free two-party diarization" [V]; Granola shows green and gray bubbles on the same basis [V]. Then name "them" from the calendar invite, as Wispr Flow and Notion do [V]. This is enough for one-to-one calls and wrong for three or more remote speakers.

| Option | Where | State and price | Licence |
| --- | --- | --- | --- |
| Soniox | Cloud | Diarisation and language identification "bundled into the hourly rate" ([pricing](https://soniox.com/pricing)) [V] | |
| Deepgram | Cloud | Included for pre-recorded; +$0.0020/min for streaming ([pricing](https://deepgram.com/pricing)) [V] | |
| AssemblyAI | Cloud | +$0.02/h async; +$0.12/h streaming ([pricing](https://www.assemblyai.com/pricing)) [V] | |
| OpenAI `gpt-4o-transcribe-diarize` | Cloud | $0.006/min ([pricing](https://developers.openai.com/api/docs/pricing)) [V] | |
| pyannote community-1 | Local | Better than 3.1 on most benchmarks; runs offline; gated download ([model card](https://huggingface.co/pyannote/speaker-diarization-community-1)) [V] | CC-BY-4.0 |
| [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) | Local | 16,201 stars, active; diarisation, VAD, STT on ONNX Runtime [V] | Apache-2.0 |
| [FluidAudio](https://github.com/FluidInference/FluidAudio) | Local, Swift/CoreML | 3,330 stars, active; used by Talat and Quill [V] | Apache-2.0 |
| [pyannote-rs](https://github.com/thewh1teagle/pyannote-rs) | Local, Rust | 131 stars, last push Sep 2025 [V]. Stale. | MIT |

Note that stored voice profiles ("voiceprints") change the legal category of the data (section D). Wispr Flow advertises that it creates none [V].

### C5. Cost per meeting hour on cloud APIs (list prices today) [V]

| Provider and model | After the meeting | Real time | Speaker labels | Mixed Spanish/English |
| --- | --- | --- | --- | --- |
| [Soniox](https://soniox.com/pricing) | $0.10 | $0.12 | Included | Yes: each token carries a language tag, and it handles speech that "mixes multiple languages" ([docs](https://docs.soniox.com/stt/concepts/language-identification)) |
| [ElevenLabs Scribe](https://elevenlabs.io/pricing/api) | $0.20 | $0.22 | Not stated on the page | Not checked |
| [AssemblyAI STT-3.7](https://www.assemblyai.com/pricing) | $0.21 + $0.02 = $0.23 | $0.30 + $0.12 = $0.42 | Add-on | Not checked |
| [Deepgram Nova-3 multilingual](https://deepgram.com/pricing) | $0.31 | $0.35 + $0.12 = $0.47 | Included batch, add-on streaming | `language=multi` on Nova-3, 55 languages ([docs](https://developers.deepgram.com/docs/multilingual-code-switching)) |
| [OpenAI gpt-4o-transcribe-diarize](https://developers.openai.com/api/docs/pricing) | $0.36 | $1.02 (gpt-live-transcribe, no diarisation stated) | Included in the diarize model | Not checked |
| [OpenAI gpt-4o-mini-transcribe](https://developers.openai.com/api/docs/pricing) | $0.18 | n/a | No | Not checked |

Deepgram's page labels its prices as promotional [V]. Hourly figures are my arithmetic from the per-minute prices.

At 40 meeting hours a month, transcription after the meeting costs about $4 (Soniox) to $14 (OpenAI). The summary and extraction call is small: Soniox's own reference is about 15,000 text tokens per hour of speech [V]. Inference: doing it after the meeting is 9 to 65% cheaper than streaming, depending on the provider, and removes the hardest real-time problems; streaming is only needed for live features.

---

## D. Legal pointers (not legal advice)

**Spain and the EU**

- **AEPD, 14 Jan and 20 Apr 2026**, two blog posts on AI voice transcription ([part II](https://www.aepd.es/prensa-y-comunicacion/blog/transcripcion-de-voz-con-ia-ii-responsabilidad-derechos-y-transparencia), [part I](https://www.aepd.es/prensa-y-comunicacion/blog/transcripcion-de-voz-con-ai)) [V for part II]. Points made:
  - Voice recordings and transcripts are personal data. Whoever adopts the tool is the controller.
  - Participants must be informed **before** the session starts.
  - A **visible, active indicator** must show recording throughout.
  - Consent is per session; "consentimiento genérico o indefinido para grabaciones futuras" is not valid, and "joining means you consent" does not meet the standard.
  - Recording must stop automatically when the session ends.
  - Retention periods must be stated; transcription errors trigger the duty to rectify.
  - The post does not discuss the household exemption or legitimate interest.
- **One-party recording.** Secondary sources say a participant may record their own conversation in Spain (Constitutional Court ruling 114/1984) and that the criminal risk is in recording others' conversations or disclosing recordings (Penal Code art. 197) [U]. This is about secrecy of communications. It does not remove GDPR duties once the recording is used for work.
- **Biometrics.** Voiceprints or speaker-identification templates may be biometric data needing explicit consent ([Mayer Brown, 3 Jun 2026](https://www.mayerbrown.com/en/insights/publications/2026/06/ai-notetakers-productivity-tool-or-emerging-legal-risk)) [V as a law-firm note].
- **Works councils.** EU member states may require works-council involvement for processing employee data (Mayer Brown) [V as a law-firm note].
- **EU AI Act.** Emotion inference in the workplace is prohibited [U, memory]. Do not add sentiment or "coaching" analysis of colleagues.

**A user in a regulated workplace**

- The user is not the controller of work meetings; the employer is. Recording internal meetings with a personal tool creates a second, unapproved copy of the employer's data on personal infrastructure or a personal cloud account. Inference from the AEPD and Mayer Brown points.
- Mayer Brown's recommended company policy: only approved notetakers, an escalation path for shadow IT, and default non-use for legal advice, HR matters, regulated clients, trade secrets and board discussions [V as a law-firm note].
- A vendor poll reported that 68% of financial-services compliance attendees restrict AI meeting tools and 20% ban them [U, vendor blog].
- DORA's ICT third-party rules mean a bank must register and assess providers that process its data [U, memory]. A personal API key to a speech provider is such a provider.
- The project brief already keeps employer meetings out of scope. This research supports keeping it that way, and suggests enforcing it in software (calendar-domain or account allowlist) rather than by habit.

**Litigation signal (US).** A federal judge let wiretap and biometric claims against Otter proceed on 13 Aug 2026; two BIPA suits against Fireflies are pending [U, legal-news snippets].

**What products do about consent**

| Product | Mechanism | Source |
| --- | --- | --- |
| Granola | Optional automated chat message; "Granola Watermark" on your video; admin-wide enforcement; pre-call emails. States that customers "remain responsible for determining what notice or consent is required" | [docs](https://docs.granola.ai/help-center/consent-security-privacy/transparency-solutions/introduction) [V]; pre-call emails [U] |
| Notion | Consent message as chat text or a voice message played through speakers; workspace owners can force it; with the Meet extension it pins a chat message and shows an indicator | [help](https://www.notion.com/help/ai-meeting-notes) [V] |
| Wispr Flow | "Get everyone's consent before you record"; no automatic notice; enterprise consent acknowledgment "available on request" | [page](https://wisprflow.ai/notetaker) [V] |
| ChatGPT Record | "Always get the right consents before recording others" | [help](https://help.openai.com/en/articles/11487532-chatgpt-record) [V] |
| Claude Meetings | Visible participant plus a chat notice; external guests off by default on Enterprise | [help](https://support.claude.com/en/articles/18325154-use-claude-meetings) [V] |
| Minutes | "Consent reminders, acknowledgement, and provenance"; a restricted flag; auto-record is off by default and a consent requirement forces manual start | [README](https://github.com/silverstein/minutes), [call-auto-record.md](https://github.com/silverstein/minutes/blob/main/docs/call-auto-record.md) [V] |
| Apple call recording | Audible alert to both sides | TWiT hands-on [U] |

Inference: every bot-less vendor puts the duty on the user, and the features that exist (chat message, watermark) are opt-in. None satisfies the AEPD's "visible indicator throughout" on the other participants' screens except a bot or a watermark.

---

## E. Integrate instead of rebuild

### Options as the meeting layer

| Layer | How the copilot would consume it | What you get | What you lose |
| --- | --- | --- | --- |
| **Wispr Flow MCP** (already in use) | Remote MCP with browser OAuth. The connector attached to this research session lists 16 tools, among them `search_meetings`, `get_meeting`, `get_meeting_by_calendar_id`, `get_meeting_participants_enriched`, `list_upcoming_meetings`, `search_calendar_events`, `get_scratchpad_note` (names observed in the session's tool list; no tool was called) | Summaries, notes, transcripts, attendees, corrected speaker names, Google and Microsoft calendar events ([docs](https://docs.wisprflow.ai/articles/9551372685-connect-an-mcp-client-to-wispr-flow-remote-mcp-server)) [V]. Zero capture code. | Read-only [V]. No "meeting ended" event, so polling. Search covers titles and summaries, not transcripts [V]. No dedicated action-item tool in the docs [V]. No audio, no live transcript. Rate limits unspecified [V]. Mac and Windows only [V]. Data stays in a vendor cloud. A 30-day history limit on the free plan [V]. |
| **Minutes** | Watch `~/meetings/*.md` for new files, or `npx minutes-mcp` | Local files with `action_items` (assignee, task, due) already extracted [V]; restricted flag; MIT; local transcription | A second app to install and keep updated. System audio needs its desktop app [V]. Eight-month-old project [V]. Local models use memory while processing. Node needed for MCP. |
| **Anarlog** | Local CLI and MCP (Rust, stdio) over SQLite | Action items in `get_meeting`; transcript paging; proposal tools [V]; MIT; active team | Heavy app. The team's focus has moved to char [V]. Cloud transcription and better speaker ID are paid [V]. |
| **Meetily community** | None | MIT capture and local transcription | No MCP, CLI, webhooks or speaker labels without PRO [V]. Not usable as an agent-facing layer for free. |
| **Granola MCP / API** | Remote MCP or REST | Best documented; transcript tools; write tools | Paid for transcripts ($14) [V]; duplicates Wispr Flow. |

### What is lost by not owning capture

- Live features during the meeting (nudges, live recap). Wispr Flow has a mid-call recap inside its own app only [V].
- One permission story and one indicator in one overlay.
- Control of language handling for mixed Spanish and English.
- Independence from a vendor changing limits, as Granola did with its local cache and free plan.

What is gained: no audio code, no echo canceller, no macOS-upgrade breakage, no recordings stored by the copilot.

---

## Decision questions

### 1. Is meeting capture worth building for a solo developer?

**No, not now.** It is a solved layer for the note itself and should be consumed through MCP or files.

- Fact: capture plus summary is free or under $15 a month from about ten vendors and five MIT projects (A1, A2).
- Fact: the reliable version needs silent-failure detection, tap rebuilds on device change, echo cancellation, clock alignment and re-testing on each macOS release (C2).
- Fact: the owner already pays for and uses a notetaker with an MCP server.
- Inference: the part with no good product is the step after the transcript: proposed tickets with owner, quote and confirmation. That is where a personal copilot adds something and where the existing "preview and confirm" rule already fits.

Revisit building capture only if one of these becomes true: live in-meeting features are wanted; the vendor MCP proves too slow or limited; or meetings must stay fully on-device.

### 2. Cheapest path to "meeting ends, action items appear as proposed tickets with owner and quote"

1. **Trigger.** A scheduled job (already planned as a feature) polls the meeting layer every few minutes, or fires a few minutes after a calendar event ends. With Minutes, a file watcher on `~/meetings/` replaces polling.
2. **Fetch.** Get the meeting with transcript, attendee names and speaker labels.
3. **Extract.** One LLM call with a strict schema: `title`, `owner`, `due`, `quote`, `timestamp`, `confidence`. About 15,000 tokens of input per meeting hour [V].
4. **Check.** Reject any item whose `quote` is not an exact substring of the transcript. Map `owner` to a tracker user only when the name matches an attendee; otherwise leave it empty.
5. **Propose.** Show cards in the overlay; nothing is created until confirmed. Then create the issue through the tracker's MCP server.

No audio code, no new permissions, no recordings. Inference: this is days of work, against weeks for reliable capture. Step 5 is the same code path as the spoken "create a ticket for this" command.

### 3. Which codebase to borrow capture code from, if capture is built later

| Rank | Source | Why | Licence caveats |
| --- | --- | --- | --- |
| 1 | **Anarlog** `crates/audio-actual`, `crates/aec`, `crates/audio-sync`, `crates/resampler` | Most complete Rust implementation: tap through `cidre`, mic, echo cancellation, drift, recovery; Windows and Linux backends for later | MIT outside `enterprise/`. Check each crate's `PROVENANCE.md`: some files are ports of GPL-3.0-or-later char code released under MIT by the copyright holder on 19 Nov 2026 [V]. Record repository, commit and that authorisation in `NOTICE.md`. The origin and licence of the ONNX echo-cancellation models was not verified. Crates depend on internal `anlg-*` crates, so lifting them is not a copy-paste. |
| 2 | **Meetily** `frontend/src-tauri/src/audio/capture/core_audio.rs` | Smallest readable `cidre` example | MIT. Pins `cidre` to a git revision. Do not take anything described as PRO. |
| 3 | **Minutes** `system_audio_tap.swift` | Best logic for refused taps and FaceTime calls (tap first, SCK as witness) | MIT. Swift, so it would be a sidecar or a port. |
| Reference | Omi, Quill, AudioCap, SystemAudioKit | Swift tap implementations | MIT, MIT, BSD-2-Clause, MIT |
| Do not use | **screenpipe** | Commercial source-available licence | Not MIT/Apache/BSD |

Also consider borrowing nothing: `cpal` ≥ 0.18 (Apache-2.0) does microphone and macOS loopback in one dependency [V].

### 4. Top 3 to copy, top 3 traps

**Copy**

1. **Proposals, not writes.** Anarlog's `propose_*` tools stay "pending until a human applies it"; char "waits for you to say yes"; Granola chat cites its sources. Every proposed ticket should carry the quote and timestamp it came from.
2. **Structured action items in a plain store, with a restricted flag.** Minutes' frontmatter (`assignee`, `task`, `due`, `status`) and its rule that restricted meetings never reach an agent by default.
3. **Two tracks and the calendar for names.** Microphone is "me", system audio is "them", attendee names come from the invite. No voiceprints.

**Traps**

1. **Silent capture failure.** A refused or broken tap returns zeros with no error; there is no API to ask; macOS upgrades can reset it. A notetaker that silently recorded nothing is the top user complaint in the category.
2. **Echo and Bluetooth.** Loudspeakers double every remote sentence; Apple's built-in fix lowers the call volume; Bluetooth headsets degrade when used for input and output. This is weeks of audio work that produces no visible feature.
3. **Invisible recording in a regulated workplace.** Bot-less means nobody else sees it. The AEPD asks for prior information, a visible indicator and per-session consent; a regulated employer's policy is likely stricter. Keep work meetings excluded in software, and send a consent message by default for the rest.

A fourth, smaller trap: rebuilding the notes interface. That is the commoditised part.

---

## Not verified, or worth a direct test

- Whether a Core Audio tap shows the purple indicator on macOS 26 and 27. Only third-party statements were found.
- Whether Wispr Flow exposes extracted tasks through MCP. The docs do not mention it; one tool call on a non-work meeting would settle it.
- Delay between a meeting ending and its transcript being readable through the Wispr Flow MCP.
- How well any cloud model handles Spanish and English mixed within one sentence. Only vendor statements were read; no audio was tested.
- Omi, Bee and Fireflies prices; Otter and tl;dv bot-less modes.
- Spanish case law and the AI Act and DORA references, which come from secondary sources or memory.
