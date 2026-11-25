# 04 · Red-team memo: should the voice-first desktop copilot be built at all?

Date: 24 November 2026. Scope: stress-test the earlier conclusion "build it, staged; replace the dictation app only if a one-day latency test passes; 2 to 3 months part-time".

**Bottom line.** Do not build the app as specified. The zero-build stack already covers four of the five pillars for no extra money, the open-source space the plan would enter is saturated and already contains an MIT project with nearly the same feature list, and the money saved by replacing the dictation app is at most about US$15 a month. What survives the red team is a much smaller thing: a thin voice-to-action command layer on top of tools already paid for, built as glue first and as an app only if two weeks of measured use demand it.

## How to read this memo

- **VERIFIED** = I opened the primary source today (URL given). Web pages were read through a fetch tool that summarises with a small model; for the Claude release notes and the Claude Code docs I also read the raw text. GitHub figures come from the GitHub REST API today, Hacker News figures from the HN Algolia API today.
- **UNVERIFIED** = search snippet, secondary source or memory.
- **INFERENCE** = my own reasoning from the facts.
- Not obtained: true star-over-time curves (the stargazer endpoint refused my token, so I sampled fork creation dates as a proxy), Reddit and X sentiment (searches returned vendor blogs, no threads), ChatGPT consumer prices (the pricing page rendered without figures).

---

## A. The zero-build and thin-glue alternatives

### A1. Best stack available today

| Pillar of the concept | Existing product | Evidence | Monthly price |
| --- | --- | --- | --- |
| Push-to-talk dictation in any app | Wispr Flow Pro (already paid) | VERIFIED: Pro is $15/mo monthly or $12/mo annual, unlimited dictation, Mac/Windows/iOS/Android ([pricing](https://wisprflow.ai/pricing)) | $12 to $15 |
| Bot-less meeting notes | Wispr Flow Notetaker, same subscription | VERIFIED: launched on Mac 5 Aug 2026, "No bot joins your call", included in existing plans; Windows 15 Sep, iOS 19 Nov ([what's new](https://wisprflow.ai/whats-new)) | included |
| Meeting notes into an agent | Wispr MCP server into Claude or ChatGPT | VERIFIED: read-only; exposes summaries, notes, transcripts, attendees, calendar events; no dictation history; disabled under a HIPAA BAA ([MCP doc](https://docs.wisprflow.ai/articles/4759919286-how-to-connect-wispr-flow-to-claude-chatgpt-and-other-ai-tools-mcp)) | included |
| Quick commands through connectors | Claude desktop: quick entry (double-tap Option from any app, optional Caps Lock dictation, screenshot and window attach) plus connectors | VERIFIED: quick entry on all plans, macOS ([help article](https://support.claude.com/en/articles/12626668-use-quick-entry-with-claude-desktop-on-mac)). UNVERIFIED: that connectors fire from the quick-entry box (the article does not say) | Claude Pro $20 (already paid) |
| Scheduled jobs and nudges | Cowork scheduled tasks; Claude Code Desktop scheduled tasks | VERIFIED: hourly/daily/weekdays/weekly, run in the cloud "even when your computer is asleep", use the same connectors, all paid plans ([Cowork](https://support.claude.com/en/articles/13854387-schedule-recurring-tasks-in-cowork)); local tasks accept cron expressions and can run every minute when Claude sets them ([Code Desktop](https://code.claude.com/docs/en/desktop-scheduled-tasks)) | included |
| Start and follow coding agents | Claude Code agent view, Dispatch, Remote Control, built-in voice dictation; Devin through its MCP server | VERIFIED: dictation "also works in agent view ... to dictate to a background session" ([voice docs](https://code.claude.com/docs/en/voice-dictation)); Devin MCP exposes `devin_session_create` and `devin_session_interact` ([Devin docs](https://docs.devin.ai/work-with-devin/devin-mcp)) | included (Devin billed separately) |
| Multi-vendor agent orchestration (optional) | Happy (MIT, free) | VERIFIED: "Orchestrate Claude, Codex and Grok in one app ... reuses the subscriptions you already pay for", 24,788 stars ([repo](https://github.com/slopus/happy), [site](https://happy.engineering/)). Voice support is not stated on the site today (UNVERIFIED) | $0 |
| Launcher with dictation, AI and MCP (optional) | Raycast Pro | VERIFIED: $10/mo; AI chat, AI Extensions, MCP, Dictation, BYOK from Pro up ([pricing](https://www.raycast.com/pricing)); system-wide dictation with cleanup, cloud STT ([manual](https://manual.raycast.com/ai/dictation)); "Dictate to AI" command shipped 4 Nov 2026 ([changelog](https://www.raycast.com/changelog/macos/2-11)) | $10 |

**Price.** $32 to $35 a month for Wispr Pro plus Claude Pro, which the developer already pays. Incremental cost of the zero-build stack: **$0** (or $10 with Raycast Pro). INFERENCE.

**Coverage.** Dictation, bot-less meeting notes, scheduled jobs, connectors and coding-agent dispatch are all present. Wispr's Command Mode covers spoken text edits and a Perplexity search only; it is still labelled experimental and takes no actions in other apps (VERIFIED, [doc](https://docs.wisprflow.ai/articles/4816967992-how-to-use-command-mode)).

**Concrete residual gap** (INFERENCE):

1. No single surface. Four apps, four hotkeys, four notification styles.
2. No sub-second "speak, see a preview card, press Enter" path to a ticket or calendar entry. The Claude path opens a chat and takes an agent round trip of several seconds.
3. No single glance at agents across vendors (Claude Code and Devin together).
4. No open source, no bring-your-own-key, no data ownership.
5. Nothing for work mail and meetings, which stay out of scope. A self-built tool does not fix this either.

### A2. Thin glue instead of a new app

| Glue | What it is | Effort | Closes gap |
| --- | --- | --- | --- |
| G1 | A Cowork scheduled task: "each hour, read new meetings from the Wispr MCP, draft Linear issues for my action items, wait for approval" | 0.5 day, no code | Meeting notes to action items |
| G2 | Hotkey (Raycast script command or Hammerspoon) that takes dictated text and runs `claude -p` with an MCP config and an allow-list, then shows the result as a notification | 1 to 2 days | Voice to ticket, voice to calendar |
| G3 | Menu-bar item (SwiftBar or similar) listing Claude Code sessions and Devin sessions with state | 1 day | Agent status at a glance |
| G4 | A small MCP server that wraps anything missing (for example a personal task inbox) | 1 to 3 days | Custom connectors |

VERIFIED basis for G2: `claude -p` runs non-interactively with `--allowedTools`, `--mcp-config`, JSON output and permission modes ([headless docs](https://code.claude.com/docs/en/headless)). Risk, also VERIFIED there: `--bare` "will become the default for `-p` in a future release" and bare mode does not use the subscription login, so this glue may later need an API key.

---

## B. Base rates

### B1. Closest open-source projects (GitHub API, 24 Nov 2026, all VERIFIED)

| Project | Created | First release | Releases since | Stars | Issues open / closed | Who writes it | Note |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [Handy](https://github.com/cjpais/Handy) (dictation, Rust + webview, MIT) | 13 Feb 2025 | 0.1.0 on 16 Jun 2025 | 69, latest v0.10.2 today | 40,050 | 59 / 816 | 193 contributors, top one has 64% of commits | Still 0.x after 21 months |
| [VoiceInk](https://github.com/Beingpax/VoiceInk) (dictation, Mac) | 20 Oct 2024 | v0.1 after 3 days | 135 | 7,207 | 145 / 568 | One person: 1,806 commits, next 18 | More than one release a week for 25 months |
| [OpenWhispr](https://github.com/OpenWhispr/openwhispr) (dictation + agent + meetings + notes, MIT) | 19 Jun 2025 | v1.0.0 after 5 weeks | 98 | 12,161 | 272 / 903 | Five people with 100+ commits each; 2,450 commits in 52 weeks | See B2 |
| [FluidVoice](https://github.com/altic-dev/FluidVoice) (dictation, GPL-3.0) | 21 Sep 2025 | day 1 | 66 | 13,244 | 97 / 638 | Two main authors | |
| [Anarlog, ex-Hyprnote](https://github.com/fastrepl/anarlog) (notetaker, MIT, YC) | 9 Dec 2024 | nightly 11 Mar 2025 | 344 | 9,984 | 8 / 955 | Two founders (4,721 and 3,714 commits) plus a Devin bot (1,129) | A funded team, not a side project |
| [Meetily](https://github.com/Zackriya-Solutions/meetily) (notetaker, MIT) | 26 Dec 2024 | v0.0.1 on 1 Feb 2025 | 13, latest v0.5.0 | 33,018 | 231 / 153 | A company team | More issues open than closed; still 0.x after 23 months |
| [screenpipe](https://github.com/screenpipe/screenpipe) | 19 Jun 2024 | 6 days | 507 | 22,530 | 12 / 2,345 | Founder: 9,739 commits | Full-time company effort |

INFERENCE: a usable dictation v1 takes days to weeks (FreeFlow reached the HN front page one day after its repository was created; OpenWhispr reached v1.0.0 in five weeks). The cost is not v1. It is the treadmill after v1: one release a week or more, for years, and none of these projects is described by its own users as matching the paid leader on formatting (see C).

### B2. The plan's feature list already exists under MIT

VERIFIED from the [OpenWhispr README](https://github.com/OpenWhispr/openwhispr) today: "voice-to-text dictation with AI agents, meeting transcription, and notes", macOS/Windows/Linux, a dedicated voice-assistant hotkey that "sends what you say straight to your AI assistant as a command", meeting detection with diarization and Google/Microsoft/Apple calendar integration, local or cloud models, cloud transcription "with your own API keys", a public API and an MCP server. UNVERIFIED: whether its agent can call third-party MCP connectors. It lacks, as far as the README shows, scheduled jobs and coding-agent dispatch.

### B3. Stalled, pivoted or closed (all VERIFIED via GitHub API or README today unless marked)

| Project | What happened | Time from start |
| --- | --- | --- |
| [Clicky](https://github.com/farzaa/clicky) (voice + screen "AI buddy" overlay for Mac) | 8,074 stars; a quarter of all forks arrived within two days of launch; README of 27 Apr 2026: new work "gonna keep it private"; 47 issues open, 0 closed | Closed-sourced after 3 weeks |
| [Crystal](https://github.com/stravu/crystal) (multi-session coding-agent manager) | README: "Deprecated: February 2026", replaced by another product | 8 months |
| [Amurex](https://github.com/thepersonalaicompany/amurex) ("AI meeting copilot ... invisible companion for work + life") | Last commit 27 May 2025 | 6 months |
| [Open Interpreter 01](https://github.com/openinterpreter/01) (voice interface for a computer agent) | Last push 1 Nov 2024 | 10 months |
| [whisper-writer](https://github.com/savbell/whisper-writer) (solo dictation app) | Last push 24 Aug 2024, 85 issues open | 16 months |
| [Mycroft](https://github.com/MycroftAI/mycroft-core) (open-source voice assistant) | Archived | 8 years |
| [Leon](https://github.com/leon-ai/leon) ("open-source personal assistant") | Started 2019; latest release tags are pre-release components from Feb 2026 | 7 years without a stable 2.0 |
| [Omnara](https://github.com/omnara-ai/omnara) | Now "open-source managed agent platform". UNVERIFIED (memory): it started as a mobile and voice command centre for coding agents | Pivoted |

### B4. Maintenance burden, measured in issue trackers (VERIFIED, GitHub search API today)

- Handy: 200 issues mention "paste", 110 mention "macOS 26", 53 mention hotkeys not working, 14 mention the Accessibility permission. Yesterday's example: [#2546](https://github.com/cjpais/Handy/issues/2546), "macOS 26 (Tahoe): Handy does not appear in Privacy & Security > Accessibility". Earlier: [#277](https://github.com/cjpais/Handy/issues/277) "Overlay is not shown over fullscreen apps on macOS", [#1281](https://github.com/cjpais/Handy/issues/1281) "System-wide freeze when Accessibility permissions are revoked while app is running".
- OpenWhispr: 218 issues mention "paste", 80 mention a macOS version name.
- System-audio capture: 59 issues in Meetily, 39 in Anarlog, 15 in two-week-old Yogurt mention "system audio".

---

## C. Demand signals

### C1. What gets attention on Hacker News (HN Algolia API, VERIFIED)

| Launch | Date | Points | Type |
| --- | --- | --- | --- |
| [Yogurt, open-source alternative to Granola](https://news.ycombinator.com/item?id=50454625) | 11 Nov 2026 | 308 | Single purpose |
| [Free alternative to Wispr Flow, Superwhisper, Monologue](https://news.ycombinator.com/item?id=47040375) | 16 Feb 2026 | 277 | Single purpose |
| [Handy](https://news.ycombinator.com/item?id=46628397) | 15 Jan 2026 | 247 | Single purpose |
| [BetterWispr, open-source dictation for Mac](https://news.ycombinator.com/item?id=50042333) | 11 Oct 2026 | 220 | Single purpose |
| [Rowboat, open-source alternative to Claude Desktop](https://news.ycombinator.com/item?id=48819808) | 7 Jul 2026 | 219 | Suite, YC team |
| [Muesli, "if Granola and Wisprflow had an open source ... baby"](https://news.ycombinator.com/item?id=48011116) | 4 May 2026 | 15 | Combination |
| [Wisp, open-source desktop AI overlay with MCP](https://news.ycombinator.com/item?id=48882687) | 12 Jul 2026 | 3 | The concept, nearly |
| [Omni, "Granola, Notion and Wispr Flow all in one"](https://news.ycombinator.com/item?id=48816190) | 7 Jul 2026 | 1 | All-in-one |
| Voice front ends to coding agents: [Ski](https://news.ycombinator.com/item?id=49113559) 15, [Luke](https://news.ycombinator.com/item?id=49994442) 5, [Dictare](https://news.ycombinator.com/item?id=47397427) 2, [AgentWire](https://news.ycombinator.com/item?id=46974968) 1 | 2026 | 1 to 15 | Voice to coding agents |

INFERENCE: "free alternative to <one paid app>" still draws 200 to 300 points late in 2026. All-in-one launches and voice-to-coding-agent launches draw almost nothing. HN points are noisy and the sample is small, but I found no counter-example.

### C2. Stars versus daily use

- BetterWispr: 220 HN points, [546 stars](https://github.com/opennookorg/betterwispr) six weeks later (VERIFIED). Attention did not convert.
- Clicky: 8,074 stars, zero closed issues (VERIFIED). Stars without a maintained product.
- Meetily: 33,018 stars but 384 issues in total; Handy has 875 issues on 40,050 stars. Fork dates show Meetily took a quarter of all its forks in 20 days of July 2026 and has slowed since; Handy's forks are spread evenly, half of them in the last five months (VERIFIED fork dates; INFERENCE that Handy has more real users per star).
- Signs of real daily use: commenters in the [BetterWispr thread](https://news.ycombinator.com/item?id=50042333) report thousands of dictated prompts, mostly into coding agents; the same thread says such apps "seem to be coming once a week now" (VERIFIED, paraphrased by the fetch tool). Happy's README badge claims 4.9 stars from 1,000+ App Store ratings (self-reported).
- In the [Ask HN on non-cloud alternatives](https://news.ycombinator.com/item?id=49809670) one user notes Handy "doesn't clean up grammar mistakes"; in the BetterWispr thread another calls Wispr Flow "magical", while others say its quality declined and its rewrites can flip meaning (VERIFIED, paraphrased).

### C3. Specific asks

- Voice to ticket: I found no native voice input in Linear and no thread asking for it; results were vendor pages (UNVERIFIED, search only).
- In the Rowboat thread, commenters asked for local models and said the labs will build the suite themselves; nobody asked for scheduled tasks (VERIFIED, paraphrased).

---

## D. Incumbent speed during 2026

| Date | Who | What shipped | Source |
| --- | --- | --- | --- |
| 25 Feb | Anthropic | Scheduled tasks in Cowork | VERIFIED, [release notes](https://support.claude.com/en/articles/12138966-release-notes) |
| 3 Mar | Anthropic | Voice dictation in Claude Code | UNVERIFIED date (TechCrunch snippet); feature VERIFIED in [docs](https://code.claude.com/docs/en/voice-dictation) |
| 17 and 23 Mar | Anthropic | Persistent agent thread from phone; computer use; Dispatch uses the computer while away | VERIFIED, release notes |
| 1 May | Wispr | Transforms and Scratchpad | VERIFIED, [what's new](https://wisprflow.ai/whats-new) |
| 7 Jul | Anthropic | Cowork in the cloud: "scheduled tasks run with no device online"; Microsoft 365 connector gains write tools | VERIFIED, release notes |
| 9 and 23 Jul | OpenAI | New desktop app (Chat + Work + Codex); Voice in Work and Codex, able to start and coordinate tasks | VERIFIED, [release notes](https://help.openai.com/en/articles/6825453-chatgpt-release-notes). Scheduled tasks in Work on 16 Jul: UNVERIFIED date (search snippet) |
| 5 Aug | Wispr | Bot-less Notetaker on Mac with MCP access | VERIFIED, what's new |
| 17 Aug | Wispr | $280M Series B at $2B "as it looks beyond dictation"; the notetaker "could later integrate with other tools to make updates or draft documents and emails" | VERIFIED, [TechCrunch](https://techcrunch.com/2026/08/17/wispr-raises-280m-at-2b-valuation-as-it-looks-beyond-dictation/) |
| 25 Aug | OpenAI | Webhook-triggered tasks (Gmail, Slack, GitHub) | VERIFIED, release notes |
| 26 Aug | Superwhisper | Free dictation tier | UNVERIFIED ([HN title](https://news.ycombinator.com/item?id=49453868) only) |
| Autumn | Apple | macOS 27: on-device dictation model for spelling, punctuation, capitalization; English, M3 or later, 12 GB | Feature VERIFIED, [Apple help](https://support.apple.com/guide/mac-help/mh40584/mac). Release month, and Siri AI beta absent from the EU: UNVERIFIED (press snippets) |
| 17 Sep | Wispr | Canto, its own speech model | VERIFIED, what's new |
| 29 Sep | OpenAI | Meetings plugin (beta, macOS, Pro and Business); "dots", always-on agents | VERIFIED, release notes |
| 21 Oct | Anthropic | Claude Duet: proactive agent, morning brief, checks accounts every few hours, asks before sending or deleting; US waitlist, iOS/Android/web | VERIFIED, [blog](https://claude.com/blog/claude-duet) |
| 29 Oct | Granola | System-wide dictation on Mac: "Unlimited words, for free" | VERIFIED, [TechCrunch](https://techcrunch.com/2026/10/29/granola-crunches-up-a-dictation-feature-to-compete-with-wispr/) |
| 30 Oct | Wispr | Custom skills in Markdown for meeting chats | VERIFIED, what's new |
| 4 Nov | Raycast | "Dictate to AI" | VERIFIED, [changelog](https://www.raycast.com/changelog/macos/2-11) |
| 12 Nov | Anthropic | Claude Meetings joins Google Meet (a bot; Team and Enterprise) | VERIFIED, release notes |
| 18 Nov | OpenAI | Live Dictation on all plans, inside ChatGPT | VERIFIED, release notes |

**Likely free or bundled within 12 months** (INFERENCE):

- Dictation with AI cleanup: already free from Granola, Superwhisper's free tier, Wispr's limited free tier and the OS (English only). Treat as a commodity now.
- Bot-less meeting notes with action items: already bundled (Wispr, Granola Basic at $0 with 30 days of history, VERIFIED [pricing](https://www.granola.ai/pricing)).
- Scheduled jobs, connectors, proactive briefs: already bundled in the $20 plans.
- Voice to a vendor's own coding agent: already shipped by both labs.

**Unlikely from incumbents within 12 months** (INFERENCE): one voice surface that dispatches to several vendors' coding agents; bring-your-own-key; local data ownership; Linux; an open-source client. One caution in the other direction: Wispr's Command Mode has been experimental since August 2024 (UNVERIFIED date, from a 2024 changelog snippet) and still takes no actions in other apps, so the dictation leader has been slow on exactly the command layer.

---

## E. Cost reality

Assumptions (mine): 30 hours of dictation audio a month, 43 hours of meetings a month, 1,200 agent commands a month.

**Prices, VERIFIED today.** Groq `whisper-large-v3-turbo` $0.04 per audio hour ([Groq](https://console.groq.com/docs/models)). Deepgram Nova-3 multilingual $0.0058/min streaming, $0.0052/min pre-recorded with diarization included, keyterm prompting +$0.0013/min ([Deepgram](https://deepgram.com/pricing)). ElevenLabs Scribe v2 $0.20/h batch, $0.22/h realtime ([ElevenLabs](https://elevenlabs.io/pricing/api)). OpenAI `gpt-4o-mini-transcribe` about $0.003/min ([OpenAI](https://developers.openai.com/api/docs/pricing)). Claude Haiku 5.5 $0.10 in / $0.50 out per million tokens, Sonnet 5.5 $2 / $10, Opus 5.5 $4 / $20, cache reads at 5% on the 5.5 Sonnet and Opus ([Anthropic](https://platform.claude.com/docs/en/about-claude/pricing)).

**Monthly estimates (INFERENCE from those prices):**

| Component | Lean | Balanced | Quality-first |
| --- | --- | --- | --- |
| Dictation speech-to-text, 30 h | $1.20 (Groq turbo) | $10.44 (Deepgram multilingual) | $12.78 (Deepgram + keyterms) |
| Dictation cleanup, about 3M tokens in, 0.5M out | $0.40 (Groq GPT-OSS 20B) | $0.55 (Haiku 5.5) | $11.00 (Sonnet 5.5) |
| Meetings speech-to-text, 43 h | $1.73 (Groq, no speakers) | $13.52 (Deepgram, speakers) | $20.28 (Deepgram streaming + speakers) |
| Meeting summaries | $0.11 (Haiku 5.5) | $2.20 (Sonnet 5.5) | $4.40 (Opus 5.5) |
| Agent commands, 12k tokens of tool definitions, three turns each | $5 (Haiku 5.5) | $5 (Haiku 5.5) | $53 (Sonnet 5.5, cached) |
| **Total** | **about $9** | **about $32** | **about $100** |

**Against subscriptions.** The only subscription the build would replace is Wispr Pro at $12 to $15. Claude stays because Claude Code is used daily. So the build saves at most $15 a month in the lean configuration and costs more than the subscription in the other two. API cost is not a reason to build and not a reason against. INFERENCE.

---

## F. Hidden difficulty

| Area | Why it is harder than it looks | Evidence |
| --- | --- | --- |
| Formatting quality | The leader trained its own speech model and had public outages and accuracy regressions while tuning cleanup in May and June 2026. A competitor's open-weights cleanup model is English only | VERIFIED: Wispr [what's new](https://wisprflow.ai/whats-new), entries of 4 Jun, 10 Jun, 17 Sep. UNVERIFIED: S1-mini details (search snippets) |
| Text insertion | The single largest issue class in both big open-source apps: clipboard races, modifier keys leaking into the paste, secure input, per-app quirks | VERIFIED: B4 |
| Overlay above fullscreen, non-activating | Needed a fix in Handy; relies on panel collection behaviours that must be tested on real fullscreen Spaces | VERIFIED: Handy #277, #278. UNVERIFIED: Apple forum and plugin notes (snippets) |
| Permissions after OS updates | A new Accessibility bug on macOS 26 was filed yesterday; a macOS 27.2 beta bug in September | VERIFIED: Handy #2546, [#2132](https://github.com/cjpais/Handy/issues/2132) |
| System-audio capture | No public API to check or request the permission; reports of silent buffers | UNVERIFIED (Apple forum snippets); issue counts in B4 VERIFIED |
| OAuth for remote MCP from a desktop app | Dynamic registration is unreliable across servers; the newer scheme has the client host an HTTPS metadata document, "for desktop applications, this typically means hosting on their backend infrastructure"; desktop apps "can't hold secrets" | VERIFIED: [MCP blog](https://blog.modelcontextprotocol.io/posts/client_registration/). So "no backend" is not quite true |
| Prompt injection | An agent that reads mail and tickets takes instructions from strangers. Researchers extracted a private key from OpenClaw with one email; 230+ malicious skills in one week. Anthropic ships Duet with action classifiers and hard limits | VERIFIED: [Kaspersky](https://www.kaspersky.com/blog/openclaw-vulnerabilities-exposed/55263/), [Duet blog](https://claude.com/blog/claude-duet) |
| Signing and distribution | Notarization needs a paid Apple developer account and a Developer ID certificate; ad-hoc signing "does not prevent" the Gatekeeper warning | VERIFIED: [Tauri docs](https://v2.tauri.app/distribute/sign/macos/). UNVERIFIED: the $99/year fee (memory) |
| Two languages in one dictation | Apple's cleanup is English only; automatic language detection "can occasionally pick the wrong language" in Raycast | VERIFIED: Apple help, Raycast manual |

---

## G. Genuine wedges

| Wedge | Will incumbents do it? | Real need or niche? |
| --- | --- | --- |
| Open source | No | Real, and already served: Handy, OpenWhispr, FluidVoice, VoiceInk, FreeFlow, Yogurt, Anarlog, Meetily |
| Bring your own key | Raycast does (VERIFIED). Labs do not | Real for a minority; served by OpenWhispr and FreeFlow (VERIFIED READMEs) |
| Local data ownership | No | The strongest pull in the open-source threads. **The plan does not deliver it**: it is cloud-first with no local models in version 1 |
| Multi-vendor coding-agent dispatch by voice | No lab will route to a rival. Claude Code dictation needs a claude.ai login and is unavailable with an API key (VERIFIED) | Niche by HN evidence (C1); orchestration without voice is served by Happy and Vibe Kanban |
| Linux | No | Small; dictation already served by Handy and OpenWhispr; the developer is on a Mac |
| One surface instead of four apps | Each incumbent is building its own single surface from a different side | A real personal preference; no market evidence (C1) |
| Preview-and-confirm for every outside action, draft-only email | Duet does this for sends, payments and deletes (VERIFIED) | Real, no longer unique |
| Region | Duet is US only; Siri AI is reported absent from the EU; ChatGPT dots exclude the EEA at launch (VERIFIED for Duet and dots, UNVERIFIED for Siri AI) | If the developer is in the EU, incumbent agent features arrive late. This is the best timing argument for building |

---

## Decision memo

### 1. The strongest case against building

1. **The gap is already small and closing monthly.** Section D lists more than fifteen dated launches in 2026. In the last four months alone the dictation leader added bot-less meeting notes, the notetaker leader added free unlimited dictation, and both labs added scheduled tasks, meetings, voice and proactive agents.
2. **The open-source niche is full.** At least eight maintained open-source apps, and one of them (OpenWhispr, MIT) already combines dictation, a voice-command hotkey, meetings, calendar, notes and MCP, with five active committers and 2,450 commits in a year.
3. **The treadmill is the cost, not version 1.** Handy: 69 releases and still 0.x after 21 months, 200 paste issues, a new macOS permission bug filed yesterday. A part-time developer with other side projects inherits that queue alone.
4. **The economics are negative.** At most $15 a month saved; 2 to 3 months of part-time work spent; API bills between $9 and $100 a month.
5. **Combination products do not pull.** All-in-one and voice-to-coding-agent launches scored 1 to 15 points where single-purpose clones scored 200 to 300.
6. **Keeping work data out removes half the use.** Work mail and meetings stay out whatever is built. Meeting notes and mail drafts lose most of their value.
7. **Security is a real liability.** Mail and ticket content is attacker-controlled input to an agent holding tokens.
8. **Abandonment is the base rate** for solo and small-team assistants and overlays (B3).

### 2. The strongest case for building

1. **Parts are cheap and good.** Speech-to-text from $0.04 an hour; a capable small model at $0.10 / $0.50 per million tokens; Handy is MIT and uses a Rust core with a webview (VERIFIED: its README names WebKitGTK as the embedded webview on Linux), so the hardest dictation plumbing can be borrowed rather than written.
2. **Version 1 is quick with coding agents.** Days to weeks in every comparable (B1).
3. **Nobody ships the exact combination.** Wispr has no outward actions, its MCP is read-only, and Command Mode has stayed experimental. Labs will not dispatch to each other's agents. No incumbent offers one overlay for dictation, commands and agent status.
4. **Vendor dependence is a real risk.** Wispr had weeks of outages and accuracy problems in mid-2026 and users report quality drift (VERIFIED changelog; user reports paraphrased from HN).
5. **Regional lag** if the developer is in the EU.
6. **Fit.** A personal dictionary, two languages, own prompts, own confirmation rules. For a single user, fit can outweigh polish.
7. **Non-financial return.** Learning Rust, macOS internals and agent design is a legitimate reason, if stated as the reason.

### 3. What would have to be true (testable)

1. Over two weeks on the configured zero-build stack, the developer logs at least five moments a day where the residual gap cost real time, and glue G1 to G3 does not remove them.
2. The latency test passes with margin: median key-release to cleaned text under 800 ms and 95th percentile under 1.5 s, on the developer's own network, over at least 50 real utterances in both languages.
3. In a blind comparison of those 50 utterances, the developer prefers the self-built output to the paid app's at least 40% of the time.
4. At least half of the intended daily uses involve personal or side-project data, not the employer's.
5. The developer can protect 6 to 8 hours a week for three months and names the side project that pauses.
6. The stated motive is learning or ownership, not saving money.
7. The scope is one wedge the incumbents will not ship, not a replacement for four apps.

### 4. Kill criteria

- The latency or blind-quality test fails: drop dictation replacement for good and keep paying.
- The two-week log shows fewer than three unresolved gap moments a day: build nothing.
- A wedge prototype is used fewer than five times a day over two weeks: stop.
- Any phase overruns its time budget by a factor of two: stop and write down why.
- After release, breakage fixes exceed two hours a week for a month: retire the component and return to the paid app.
- Wispr or Granola ships outward actions with connectors, or Anthropic ships system-wide dictation or brings Duet to the developer's region with a desktop surface: stop the overlapping part.
- Trying OpenWhispr or Handy for three days satisfies the need: contribute there instead.

### 5. Cheapest experiments, in order

| # | Experiment | Effort | What it proves |
| --- | --- | --- | --- |
| 1 | Configure the zero-build stack: Wispr MCP and Linear/calendar connectors in Claude, quick entry with dictation, one scheduled task. Start a tally of gap moments | 0.5 day, then two weeks passive | How much is covered for $0; whether the gap is real |
| 2 | Install OpenWhispr and Handy and use each for a day | 0.5 day | Whether open source is already good enough; what to borrow |
| 3 | Glue G1: meeting notes to drafted tickets as a scheduled task | 0.5 day | Whether the meeting pipeline is used at all, given the work-data limit |
| 4 | Glue G2: hotkey, dictated text, `claude -p` with a Linear MCP config, result as a notification | 1 to 2 days | Whether voice-to-ticket becomes a daily habit; real latency of the agent path |
| 5 | The planned latency and formatting bake-off, extended with the blind comparison | 1 day | Whether dictation replacement is feasible at all |
| 6 | Platform spike: non-activating panel above a fullscreen app, global key-down to recording under 50 ms, paste into five apps | 1 day | The three macOS risks that sink overlays |
| 7 | Agent-status menu-bar item across Claude Code and Devin | 1 day | Whether a cross-vendor view changes behaviour |

Total: about 6 days of effort and two weeks of calendar time before any commitment.

### 6. Three minimum lovable wedges, ranked by value over effort

1. **Voice-to-action hotkey.** Hold a key, say "create a ticket for this", see a preview card, press Enter. First version: three commands (ticket, calendar entry, note) on Claude Code headless with existing MCP servers, as a hotkey script before any app. Leaves dictation and meeting notes to the paid dictation app, schedules to Cowork, agents to their own apps.
2. **Meeting-to-tickets job.** An hourly scheduled task turns new meeting notes into drafted tickets awaiting approval. First version: one scheduled task, no interface, personal meetings only. Leaves recording, transcription and summaries to the notetaker and execution to Cowork.
3. **Agent radar.** A small always-on-top panel showing every coding-agent session across vendors with state, plus spoken "start" and "reply". First version: read-only status for Claude Code and Devin, then one spoken command. Leaves dictation to the paid app and all review and diff work to the agents' own apps.

Not on the list: an open-source dictation replacement. It has the highest effort, the lowest saving and the most competitors.

### 7. Verdict

**Do not build the full copilot now. Do not replace the dictation app. Spend six days on experiments 1 to 7, and build wedge 1 as an app only if its glue version is used daily for two weeks.**

Confidence: about 85% that replacing the dictation app is the wrong use of the hours; about 75% that the full five-pillar app is the wrong call in the next six months; about 55% that even wedge 1 deserves to become an app rather than stay a script.

Least sure about:

- How much the developer values the build itself. If learning and ownership are the goal, the financial argument does not apply and a narrow build is defensible.
- The real feel of the zero-build command path. I could not confirm that connectors fire from Claude's quick-entry box, nor that Raycast's "Dictate to AI" triggers tools. If both are clumsy, wedge 1 is worth more than I rate it.
- Demand beyond one user. I found no Reddit or X evidence either way, and HN points are a weak proxy.
- Region. If incumbent agent features keep arriving late in the developer's region, the window for a personal tool is longer than section D suggests.
