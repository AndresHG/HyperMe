# 03 — Desktop agents, overlays, proactive assistants, coding-agent dispatch, connectors, platform risk

Research date: 24 November 2026. Web research only; nothing in the repository was read or changed.

## How to read this

- **[V]** = VERIFIED: I opened a primary source today (vendor docs, vendor release notes, vendor pricing page, the GitHub API or the repo's own files). The link follows the claim.
- **[U]** = UNVERIFIED: search snippet, press coverage, or a secondary page. Treat as a lead, not a fact.
- "Inference:" marks my own reasoning. Everything else is reported fact.
- GitHub numbers (stars, licence, last commit, contributor count) come from the GitHub REST API on 24 Nov 2026. Contributor counts include anonymous contributors and are approximate.
- No footprint numbers (idle memory, CPU) are given for any product: no vendor publishes them and I did not measure. Stack facts are given instead.
- One README (farzaa/clicky) contains a ready-made prompt addressed to a coding assistant ("clone this repo and set it up"). It was treated as page content and not acted on.

---

## 1. Summary in ten lines

1. 2026 was the year every large vendor shipped an always-on personal agent, and all of them are **cloud-VM, chat-thread products**, not desktop overlays: Gemini Spark (May) [U], Meta Muse (Sep) [U], OpenAI dots (Sep) [V], Hark Pro (Oct) [U], Claude Duet (Oct) [V], Microsoft Autopilot (Nov) [V].
2. Crons, connectors, memory and "ask before sending" are now table stakes in Claude, ChatGPT, Copilot, Lindy, Poke, Littlebird and Highlight. These are the worst parts of the concept to compete on.
3. The Claude desktop app alone covers most of the concept's *function*: Quick Entry overlay with dictation, MCP connectors, local and cloud scheduled tasks, Claude Code background sessions with push-to-talk dispatch, Remote Control, computer use [V].
4. ChatGPT desktop was rebuilt in July 2026 as Chat + Work + Codex in one app, with voice that drives Work and Codex [V]. The Sky overlay has not shipped under that name [V: absent from release notes].
5. The closest open-source match is **Omi desktop** (Swift, MIT): floating bar/notch NSPanel, push-to-talk, voice typing into any app, agent pills that run Claude (through ACP), Hermes and OpenClaw, an MCP client with OAuth, and integration nudges [V]. This was understated in the "already believed" list.
6. Nobody ships a voice-first, **multi-vendor** "start and follow coding agents" surface on the desktop. Vendors do it for their own agent only.
7. `rmcp` is mature: 3.11.0, OAuth 2.1 with PKCE, dynamic client registration and Client ID Metadata Documents, streamable HTTP client and server [V].
8. Remote MCP is uneven for a third-party client: Linear, Notion, Atlassian and GitHub are usable today; Slack requires a registered Slack app and forbids unlisted apps; Google Workspace is Developer Preview with your own Google Cloud OAuth client; Microsoft 365 is tenant-admin gated [V].
9. ACP is real and broad (protocol v1 stable; Cursor, Copilot, Gemini CLI, goose, OpenCode, Kiro, Junie native; Claude and Codex through adapters) [V]. Devin is not an ACP agent [V].
10. Several items on the input list changed state: Open Interpreter was repurposed, screenpipe is no longer open source, Thuki is in maintenance mode, Clicky's open repo is frozen, Vibe Kanban is sunsetting, Crystal became Nimbalyst, goose moved to the Linux Foundation, Claude Dispatch is closed to new users, ChatGPT Pulse was sunset.

---

## 2. Corrections to the "already believed" list

| Belief | Status | What I found |
| --- | --- | --- |
| OpenClaw (TypeScript, MIT), chat-gateway, heavy, cron and skills | Mostly right, extend | MIT, TypeScript, 395,155 stars, ~3,824 contributors, commits today. Now run by the OpenClaw Foundation, a 501(c)(3) funded by donors including Amazon, OpenAI and Red Hat. Has a **macOS menu-bar companion app** (requires macOS 26.2+) with voice wake and push-to-talk, native notifications and a widget panel. Scheduler is now called "Automations" (`openclaw cron` is an alias) and heartbeat is being folded into it. [V: [repo](https://github.com/openclaw/openclaw), [macOS docs](https://docs.openclaw.ai/platforms/macos), [automations docs](https://docs.openclaw.ai/automation/cron-jobs)] |
| Hermes Agent (Python, MIT), chat-gateway | Right, extend | MIT, Python, 265,870 stars, ~6,757 contributors. Built-in cron, self-improving skills, gateways for Telegram, Discord, Slack, WhatsApp, Signal. **Hermes Desktop** now exists; it is Electron (`apps/desktop/electron-builder.config.cjs`). [V: [repo](https://github.com/NousResearch/hermes-agent)] Public preview dated 2 June 2026 [U]. |
| ZeroClaw: small Rust runtime, Apache-2.0 | Right, extend | Rust, 33,606 stars, ~490 contributors, v0.9.0 (12 Nov 2026). Repo carries both `LICENSE-APACHE` and `LICENSE-MIT` (GitHub reports Apache-2.0). Single binary, 30+ channels, cron, approval gates, ACP support, and an `apps/tauri` directory. [V: [repo](https://github.com/zeroclaw-labs/zeroclaw)] |
| IronClaw: Rust, Apache-2.0, isolates credentials | Right, but slowing | Apache-2.0, 12,719 stars, ~120 contributors. README: "Secrets are never exposed to tools; injected at the host boundary with leak detection", WASM sandbox, routines, heartbeat. **Last commit on `main` is 10 Sep 2026**; latest release v1.4.1 on 29 Sep 2026; later pushes are on side branches. [V: [repo](https://github.com/nearai/ironclaw)] Inference: treat as a design reference, not a dependency. |
| goose: Rust core, Apache-2.0, `rmcp`, recipes, scheduler, Electron UI | Right, but moved and reframed | Repo is now **`aaif-goose/goose`** (Agentic AI Foundation, Linux Foundation); `block/goose` redirects. 56,942 stars, ~702 contributors, v1.60.2. README now leads with **oi-kit** (reusable Rust agent components, alpha) and `goose acp` as a backend for your own app. Desktop is Electron 45.4.1; `rmcp = 3.9.0`; scheduler lives in `crates/goose/src/scheduler`. [V: [repo](https://github.com/aaif-goose/goose)] |
| Omi (Swift, MIT): floating bar, push-to-talk, agent pills, nudges | Right and understated | See section 4 and 8. It also has voice typing into any app, an MCP client with OAuth, ACP-based agent dispatch and adapters for Hermes and OpenClaw. [V: [repo](https://github.com/BasedHardware/omi)] |
| Thuki: Tauri + Rust NSPanel above fullscreen apps | Right, but frozen and local-only | Apache-2.0 (LICENSE file), 584 stars, 6 contributors, last commit 11 Aug 2026. README: "Thuki is in maintenance mode". It runs **local models only** (bundled llama.cpp, optional Ollama); no cloud APIs, no MCP, no voice. [V: [repo](https://github.com/quiet-node/thuki)] |
| Clicky and Teachy: Swift companions beside the cursor | Clicky frozen; Teachy is tiny | `farzaa/clicky`: MIT, 8,074 stars, last commit 28 Oct 2026. README (27 Apr 2026): existing code stays open, new work is private as **HeyClicky** for Mac and Windows. [V: [repo](https://github.com/farzaa/clicky)] `VedSoni-dev/teachy`: MIT, Swift, **4 stars**. [V: [repo](https://github.com/VedSoni-dev/teachy)] A more used fork is `jasonkneen/openclicky` (MIT, 601 stars) [V: [repo](https://github.com/jasonkneen/openclicky)]. |
| Claude Code headless and Agent SDK; Devin API or MCP; ACP as common layer | Right, with three caveats | (1) Claude Code now has first-class **background sessions** (`claude --bg`, `claude agents`, `claude agents --json`), which is a better dispatch surface than `claude -p`. (2) Anthropic states third parties may not offer claude.ai login or subscription rate limits in their products without approval. (3) Devin is not listed as an ACP agent; use its MCP server or v3 API. [V: [agent view](https://code.claude.com/docs/en/agent-view), [Agent SDK overview](https://code.claude.com/docs/en/agent-sdk/overview), [ACP agents](https://agentclientprotocol.com/overview/agents)] |

Other items that changed:

- **Open Interpreter** is now "a coding agent for open models", a Rust fork of OpenAI's Codex CLI (Apache-2.0, 68,961 stars). It is no longer a general computer-control assistant. [V: [repo](https://github.com/openinterpreter/openinterpreter)]
- **screenpipe** is no longer open source: "Screenpipe Commercial License (source-available; personal, non-commercial use permitted, commercial use requires a license)". Positioning is now "finds the work worth automating". Do not copy code. [V: [repo](https://github.com/screenpipe/screenpipe)]
- **Leon**: MIT, 17,715 stars; 2.0 alpha with a desktop app is in progress and not released. [V: [repo](https://github.com/leon-ai/leon)]
- **Crystal** is now **Nimbalyst** (MIT, Electron, 2,109 stars). [V: [crystal](https://github.com/stravu/crystal), [nimbalyst](https://github.com/nimbalyst/nimbalyst)]
- **Vibe Kanban**: README says "Vibe Kanban is sunsetting" [V: [repo](https://github.com/BloopAI/vibe-kanban)]. Bloop shut down on 10 April 2026; project continues community-maintained [U].
- **Claude Code "routines"** were renamed "scheduled tasks"; **Dispatch** "isn't available to new users"; **Cowork** was merged into every conversation on 16 Sep 2026. [V: [routines doc](https://code.claude.com/docs/en/routines), [desktop doc](https://code.claude.com/docs/en/desktop), [release notes](https://support.claude.com/en/articles/12138966-release-notes)]
- **ChatGPT Pulse** was sunset on 17 June 2026 ("proactive updates move into scheduled tasks"). "Connectors" became "apps" (Dec 2025) and then "plugins" (July 2026). "Agent mode" was succeeded by **ChatGPT Work** (9 July 2026). [V: [release notes](https://help.openai.com/en/articles/6825453-chatgpt-release-notes)]
- **Sky**: OpenAI acquired its maker in October 2025 [U]. The 2026 ChatGPT release notes I read (Jan–Nov) never mention Sky [V].
- **Manus** is part of Meta (deal announced Dec 2025) and shipped a "My Computer" desktop app in March 2026 [U].
- **Highlight AI** has repositioned around meetings, follow-ups and daily briefs [V: [site](https://highlightai.com/)].

---

## 3. A. Product landscape — platform vendors

| Product | Form factor | Voice in | Scheduled tasks | Proactive | Connectors (MCP?) | Confirm before acting | Coding-agent dispatch | Memory | Stack / price |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **Claude desktop app** (Anthropic) | Chat window; **Quick Entry** overlay on Mac (double-tap Option or Option+Space) with screenshot and window attach [V: [quick entry](https://support.claude.com/en/articles/12626668-use-quick-entry-with-claude-desktop-on-mac)] | Caps Lock dictation in Quick Entry, off by default, macOS 14+ [V same]. Hold-to-talk dictation in the Claude Code CLI and VS Code [V: [voice](https://code.claude.com/docs/en/voice-dictation)] | Local (app open, machine awake; 1-minute minimum when Claude sets it) and cloud (1-hour minimum; schedule, API and GitHub triggers) [V: [desktop tasks](https://code.claude.com/docs/en/desktop-scheduled-tasks), [cloud tasks](https://code.claude.com/docs/en/routines)]. Cowork scheduled tasks run with no device online since 7 Jul 2026 [V: release notes] | OS notification when a session finishes; mobile push via Remote Control [V: [remote control](https://code.claude.com/docs/en/remote-control)]. **Claude Duet** (21 Oct 2026): separate app on iOS, Android and claude.ai, morning brief, checks accounts "every few hours", US waitlist, Pro/Max/Ultra [V: [Duet](https://claude.com/resources/articles/claude-duet)] | MCP. Duet page cites "over 6,000 connectors and plugins". Custom remote MCP connectors supported. Microsoft 365 connector gained write tools 7 Jul 2026 [V: Duet page, release notes] | Per-tool permission prompts; Duet asks before send, pay, sign, delete, and refuses trades and money movement [V: Duet] | Claude Code only: Code tab, parallel sessions, cloud sessions, Remote Control, agent view, Dispatch (closed to new users) [V: [desktop](https://code.claude.com/docs/en/desktop)] | Categorised memory entries since 10 Jul 2026; shared across chat and Cowork [V: release notes] | Linux desktop app in beta [V: [docs index](https://code.claude.com/docs/llms.txt)]. Pro $20/mo ($17 annual), Max $100, Ultra $200 [V: [pricing](https://claude.com/pricing)]. Computer use beta for Team/Enterprise 20 Nov 2026 [V: release notes] |
| **ChatGPT desktop app** (OpenAI) | New app 9 Jul 2026 for macOS and Windows combining Chat, Work and Codex; built-in browser [V: [release notes](https://help.openai.com/en/articles/6825453-chatgpt-release-notes)]. No overlay or pill documented in the notes [V]. Reviews call it Electron, 1.45 GB [U] | ChatGPT Voice (GPT-Live) in Work and Codex on desktop since 23 Jul 2026; **Live Dictation** 18 Nov 2026 on all plans [V] | In Work: once, scheduled, triggered, or monitor-for-change; webhooks for Gmail, Slack, GitHub (25 Aug); hourly minimum [V] | **dots** (29 Sep 2026): always-on agents with their own cloud computer; "proactive research" is read-only; Pro and Business Premium only [V: [dots](https://openai.com/dots/)] | Plugins (formerly apps/connectors); "over 4,000 apps"; plugins can start automations from MCP events [V: dots page, release notes] | Custom Rules: allow, require approval, block; confirmation before consequential browser actions [V] | Codex only: Codex app, Codex Remote GA 25 Jun 2026 (phone to Mac/Windows host), Codex Cloud 29 Sep 2026; dots can start Codex tasks [V] | Automatic memory; Computer History on macOS (interaction events, not screenshots), Pro/Business/Enterprise [V] | Prices not shown on the pricing page text I could read [V: [pricing](https://chatgpt.com/pricing)]; press says Pro starts at $100/mo and a $500 tier exists [U] |
| **Gemini on Mac** (Google) | Native Mac app, Option+Space, window or screen sharing, free, macOS 15+ [V: [blog](https://blog.google/innovation-and-ai/products/gemini-app/gemini-app-now-on-mac-os/)]. Launched 15 Apr 2026 [U] | Not mentioned on the launch page [V] | Not on the Mac launch page [V] | **Gemini Spark** (I/O, 19 May 2026): 24/7 cloud agent, US Ultra beta; Daily Brief [U] | Google apps first; third-party via MCP "rolling out soon" for Spark [U] | [U] | None on Mac; Antigravity is the separate coding surface [U] | Personal Intelligence [U] | Free app [V] |
| **Siri AI, macOS 27 "Golden Gate"** (Apple) | System-wide; dedicated Siri app; Spotlight integration [V: [Apple](https://support.apple.com/en-us/127257)] | Yes (Siri); not detailed on the page | None described [V] | Messages Suggestions only [V] | Acts across apps "by request"; personal context from Mail, Messages, Photos, Calendar; Excluded Apps control (27.2) [V]. Built on App Intents marked as tools; no third-party invocation API; no MCP confirmed [U] | [U] | None | Recalls past conversations (27.2) [V] | Bundled with macOS; Gemini-based [U]; M1 or later [U] |
| **Microsoft Copilot** | Copilot app Home combines Chat and Cowork; **Automations** run Cowork tasks on a schedule or event; **Autopilot** always-on cloud agent with its own mailbox rolls out to Frontier in Nov 2026 [V: [Oct 2026 post](https://techcommunity.microsoft.com/blog/microsoft-copilot-blog/what's-new-in-microsoft-copilot--october-2026/4568528)] | "Hey Copilot" wake word on Windows [U] | Yes (Automations) [V] | Autopilot [V] | Copilot connectors and Marketplace; Work IQ MCP for M365 data [V: [Learn](https://learn.microsoft.com/en-us/microsoft-agent-365/tooling-servers-overview)] | Approval guardrails for consequential actions [V] | GitHub Copilot is separate | Persistent context for Autopilot [V] | macOS status not stated in the post [V] |
| **Perplexity Personal Computer** | Mac app; press both Command keys to invoke; works across local files, native apps, connectors and web [V: [blog](https://www.perplexity.ai/hub/blog/personal-computer-is-here), 16 Apr 2026] | Yes, "speak to it directly" [V] | Not specified [V] | Mac mini 24/7 mode; start tasks from phone [V] | Connectors plus Comet browser [V/U] | "Keeps the user in the loop on sensitive actions" [V] | Not a focus | — | Launched for Max subscribers [V]; opened to Pro in a new Mac app 7 May 2026 [U] |

---

## 4. A. Product landscape — startups and closed products

| Product | Form factor | Voice in | Scheduled | Proactive | Connectors / MCP | Confirm UX | Coding dispatch | Price |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **Raycast** | Hotkey palette, AI Chat window, Notes; macOS, Windows, iOS | **Dictation** and **Dictate to AI** ("combines what you say with context from your focused app", v2.11, 4 Nov 2026) [V: [changelog](https://www.raycast.com/changelog)] | Not listed as a feature [V: [pricing](https://www.raycast.com/pricing)] | No | AI Extensions, MCP client, and since v2.13 (18 Nov 2026) a local **Raycast MCP server** exposing extensions to Claude, Codex, Cursor [V] | Permission approvals for AI tool calls [V: changelog v2.6] | None native; shows Claude and ChatGPT subscription usage [V] | Free; Pro $10/mo (AI, BYOK, MCP, Dictation); Plus $20; Max $50 [V] |
| **Highlight AI** | Desktop recorder; arm64 download | Not mentioned | Daily brief 9:00, recap 18:00, scheduled or triggered skills (Pro) | Drafts follow-ups "before you ask" | Gmail, Docs, Slack, GitHub, Notion, Linear, Granola, **Claude Code** | Stages work, waits for approval | Hands coding tasks to Claude Code with context | Free; Pro $35/mo; Max $200/mo [V: [site](https://highlightai.com/), [pricing](https://highlightai.com/pricing)] |
| **Cluely** | Floating movable window, "invisible to screen share"; desktop + iOS | Listens to meetings | No | In-meeting suggestions | None listed | n/a | No | Not on the home page [V: [site](https://cluely.com/)] |
| **Littlebird** | Mac and Windows app, web, iOS, Android; iMessage and WhatsApp | Not mentioned | **Routines** (5 on Basic, 100 on Plus), e.g. 7:00 morning brief | Pre-call briefs | Gmail, Calendar, Drive, Outlook, Slack, Notion, Linear; **Littlebird MCP server** exposes its memory | "Never sends email on its own"; saves Gmail drafts | No | Basic $0; Plus $17/mo billed annually [V: [site](https://littlebird.ai/)] |
| **Omi desktop** | Floating bar / notch NSPanel + main window; macOS (Swift), Windows (Node) | Push-to-talk, hands-free lock, **voice typing** into the focused app | Recurring tasks only (inference from `TaskRecurrenceDatePolicy.swift`); no agent cron found | Notification cards on the bar; integration nudges; `ProactiveTaskExecute.swift` | MCP client with OAuth (`LocalMcpStore+OAuth.swift`), Notion MCP connector, Gmail and Calendar readers | Receipt cards (`FloatingControlBarReceiptCard.swift`), shell confirmation dialog | **Agent pills**: Claude through `@zed-industries/claude-agent-acp`, plus Hermes, OpenClaw and pi adapters | App free tier; Pro $29.99/mo; pendant $129 [V: [repo](https://github.com/BasedHardware/omi/tree/main/desktop/macos), [site](https://www.omi.me/)] |
| **Poke** | iMessage, WhatsApp, Telegram; no desktop app | Voice messages | "Recipes"; background automations on Pro | Reaches out unprompted | Gmail, Calendar, Notion, Linear, GitHub, Sentry, Vercel and more; **Cursor and Devin** listed | Not stated | Yes, through Cursor and Devin integrations | Free; Pro $19/mo; Ultra $199/mo [V: [site](https://poke.com/)] |
| **Lindy** | Slack, iMessage, email | No | Scheduled routines | Daily briefs, nudges | "3,000+ integrations", MCP | External-impact actions wait for approval | No | Team from $29.99/user/mo [V: [pricing](https://www.lindy.ai/pricing)] |
| **Martin** | Phone calls, text, email, Slack, iOS app | Yes | Reminders | Yes | Gmail, Outlook, calendars | — | No | Roughly $21–$35/mo, sources disagree [U] |
| **Manus desktop** (Meta) | Desktop app, "My Computer" local execution; macOS, Windows | — | Scheduled Tasks | — | — | — | Runs coding tasks itself | [U] |
| **Meta Muse** | Standalone app and WhatsApp; cloud VM; US only | — | — | Yes | — | Returns for approval | No | Free; $20; $100 [U] |
| **Hark** | Web, mobile; Mac app since 29 Oct 2026 | — | — | "Spots unfinished business"; Suggested Action Buttons | Google, Outlook; browser agent "Handoff" | Asks before sending or buying unless pre-authorised | No | Free; $20; $100 [U] |

---

## 5. A. Product landscape — open source

GitHub API, 24 Nov 2026 [V for every row; link is the repo].

| Project | Licence | Stars | Contributors | Last commit (default branch) | Language / UI | What it is today |
| --- | --- | --- | --- | --- | --- | --- |
| [openclaw/openclaw](https://github.com/openclaw/openclaw) | MIT | 395,155 | ~3,824 | 24 Nov 2026 | TypeScript, Node gateway; macOS menu-bar app | Chat-gateway agent; Automations (cron); skills; companion apps add voice wake and push-to-talk |
| [NousResearch/hermes-agent](https://github.com/NousResearch/hermes-agent) | MIT | 265,870 | ~6,757 | 24 Nov 2026 | Python; Electron desktop | Self-improving skills, cron with delivery to any channel, subagents, six terminal backends |
| [aaif-goose/goose](https://github.com/aaif-goose/goose) | Apache-2.0 | 56,942 | ~702 | 24 Nov 2026 | Rust; Electron desktop | oi-kit components, recipes, scheduler, MCP via `rmcp` 3.9, `goose acp` backend |
| [zeroclaw-labs/zeroclaw](https://github.com/zeroclaw-labs/zeroclaw) | Apache-2.0 (+ MIT file) | 33,606 | ~490 | 24 Nov 2026 | Rust single binary; `apps/tauri` | Agent runtime, 30+ channels incl. voice, cron, SOP engine with approval gates, ACP |
| [nearai/ironclaw](https://github.com/nearai/ironclaw) | Apache-2.0 | 12,719 | ~120 | 10 Sep 2026 | Rust; web UI | WASM-sandboxed tools, host-boundary credential injection, routines, heartbeat |
| [BasedHardware/omi](https://github.com/BasedHardware/omi) | MIT | 14,020 | ~488 | 24 Nov 2026 | Swift (macOS), Flutter, Python backend, bundled Node agent runtime | Wearable + phone + desktop "second brain"; desktop floating bar with PTT and agent pills |
| [quiet-node/thuki](https://github.com/quiet-node/thuki) | Apache-2.0 | 584 | 6 | 11 Aug 2026 | Tauri 2, Rust, React | Local-model overlay above fullscreen apps; maintenance mode |
| [farzaa/clicky](https://github.com/farzaa/clicky) | MIT | 8,074 | n/a | 28 Oct 2026 | Swift | Cursor-buddy teacher; open repo frozen, product closed as HeyClicky |
| [VedSoni-dev/teachy](https://github.com/VedSoni-dev/teachy) | MIT | 4 | n/a | 24 Nov 2026 | Swift | Clicky clone; negligible adoption |
| [screenpipe/screenpipe](https://github.com/screenpipe/screenpipe) | **Commercial, source-available** | 22,530 | ~182 | 24 Nov 2026 | Rust, desktop app | Local screen/audio capture, workflow discovery, "pipes" (scheduled agents as markdown), MCP server |
| [openinterpreter/openinterpreter](https://github.com/openinterpreter/openinterpreter) | Apache-2.0 | 68,961 | ~617 | 24 Nov 2026 | Rust (Codex fork) | Terminal coding agent for low-cost models |
| [leon-ai/leon](https://github.com/leon-ai/leon) | MIT | 17,715 | ~27 | 24 Nov 2026 | TypeScript | Personal assistant; 2.0 alpha with desktop app not released |

Notable 2026 projects not on the input list:

- [slopus/happy](https://github.com/slopus/happy): MIT, 24,788 stars. Mobile, web and desktop client that orchestrates Claude, Codex and Grok, "reuses the subscriptions you already pay for", any ACP agent plugs in [V]. Voice control is described as experimental [U].
- [ParadevAI/Paradev](https://github.com/ParadevAI/Paradev): MIT, 16,555 stars, "open-source Codex App and Conductor alternative" [V].
- [winfunc/opcode](https://github.com/winfunc/opcode): **AGPL-3.0**, 22,464 stars, GUI for Claude Code [V]. Do not copy.
- [coder/agentapi](https://github.com/coder/agentapi): MIT, 1,493 stars, "HTTP API for Claude Code, Goose, Aider, Gemini, Amp, and Codex" [V].
- [ahkohd/tauri-nspanel](https://github.com/ahkohd/tauri-nspanel): Apache-2.0 per GitHub, 426 stars, pushed 25 Oct 2026; the plugin Thuki uses [V].
- [jasonkneen/openclicky](https://github.com/jasonkneen/openclicky): MIT, 601 stars, push-to-talk cursor companion with local agent work [V/U].

---

## 6. B. Closeness ranking

Target: one overlay + voice command + crons + connectors + coding-agent dispatch.

| Rank | Product | Has | Lacks, concretely |
| --- | --- | --- | --- |
| 1 | **Claude desktop app + Claude Code** | Quick Entry overlay with dictation; MCP connectors incl. custom remote servers; local and cloud scheduled tasks with API and GitHub triggers; background sessions, agent view with push-to-talk dispatch, Remote Control, cloud sessions; computer use; memory; Linux beta | The overlay is a launcher that opens a new chat, not a persistent pill with states. Dictation goes into Claude, not into any app. No single-utterance "command → preview card → Enter". Nudges are OS notifications; Duet is a separate, US-waitlist, mobile and web app. Meeting notes are bot-based (Claude Meetings joins Google Meet as a visible participant, Team/Enterprise) [V: release notes]. Claude Code only: no Devin or Codex. No BYOK in the consumer app. |
| 2 | **Omi desktop** (MIT) | Floating bar/notch that stays above fullscreen apps without stealing focus; push-to-talk; voice typing into any app; agent pills for Claude (ACP), Hermes, OpenClaw; MCP client with OAuth; nudges; meeting notes; calendar and Gmail context | No cron for agent jobs found (inference from the source tree). Product centre is capture-everything memory, with a hosted cloud backend by default. No Devin, Codex or Cursor pills. Swift-only on Mac (no Linux path). Bundles a Node runtime for agents, so the under-100 MB target is unlikely (inference, not measured). Preview-and-confirm for external writes exists only as receipt cards and a shell confirmation dialog. |
| 3 | **ChatGPT desktop app** | Chat + Work + Codex in one app; voice that drives Work and Codex and computer use; scheduled tasks with webhooks; plugins; Live Dictation; Codex Remote; dots on Pro | No overlay or pill documented. Dictation is into ChatGPT only. Codex only. dots are Pro-tier and cloud-side. Large Electron app [U]. No open or local-data option. |
| 4 | **Raycast Pro** ($10/mo) | Fastest hotkey palette; system dictation; Dictate to AI with focused-app context; AI Extensions and MCP client; BYOK; Notes | No scheduling, no proactive nudges, no coding-agent dispatch or follow, no meeting notes. Closed source. No Linux. |
| 5 | **OpenClaw or Hermes Agent** | Cron, skills, memory, MCP, coding agents as swappable harnesses; OpenClaw has a Mac menu-bar app with voice wake and push-to-talk; Hermes has a desktop app | No state pill or preview card on the desktop; no dictation into apps; chat-gateway first; heavy Node or Python runtime; broad attack surface for a personal machine (inference). |

Near misses: Highlight (meeting-first, scheduled skills, Claude Code hand-off, no voice command), Littlebird (routines, bot-less notes, connectors, no voice or coding), Poke (proactive, recipes, Cursor and Devin, but text-only and no desktop), goose (scheduler, recipes, MCP, ACP, no voice or overlay).

### Can two or three existing products deliver the combination today?

Yes, for function. A realistic stack for this developer:

1. The dictation app already in daily use → capability 1.
2. Claude desktop + Claude Code on a Pro or Max plan → capabilities 2, 3 and 5: Quick Entry (double-tap Option, Caps Lock to dictate), connectors with permission prompts, local and cloud scheduled tasks, `claude --bg` and agent view (hold push-to-talk to dispatch), Remote Control with phone push. Devin can be added to Claude as a connector through `https://mcp.devin.ai/mcp`, which exposes `devin_session_create` and `devin_session_interact` [V: [Devin MCP](https://docs.devin.ai/work-with-devin/devin-mcp)].
3. A bot-less notetaker with an MCP server → capability 4 (covered by another research track).

Remaining gap (inference):

- **One surface.** There is no always-visible pill that shows "recording / thinking / needs you / agent X running / meeting in 2 min". State is spread across three apps and the OS notification centre.
- **One key.** No product routes a single push-to-talk utterance to dictation, a previewed command, or an agent dispatch. Omi's "type …" wake word is the only shipped example of that routing.
- **Latency.** Quick Entry opens a full chat turn. A sub-second "ticket preview card" does not exist off the shelf.
- **Multi-vendor follow.** Claude Code, Devin and Codex statuses in one list, by voice.
- **Ownership.** Open source, bring-your-own-key, local SQLite, small footprint.

---

## 7. C. Platform risk

### What shipped or was announced in 2026

| Vendor | 2026 moves that overlap | Source |
| --- | --- | --- |
| Anthropic | Cowork with scheduled tasks; Dispatch (then closed to new users); computer use; Cowork on web and mobile with remote sessions (7 Jul); memory rewrite (10 Jul); Cowork merged into chat (16 Sep); Claude Duet proactive agent (21 Oct); Claude Meetings (12 Nov); desktop release channels (17 Nov); artifacts installable as apps (24 Nov); Claude Code agent view, background sessions, Remote Control with Trusted Devices, cloud scheduled tasks, channels, voice dictation | [V: release notes](https://support.claude.com/en/articles/12138966-release-notes), [Claude Code docs](https://code.claude.com/docs/llms.txt) |
| OpenAI | Codex apps (Feb macOS, Mar Windows); Codex Remote GA (25 Jun); Pulse sunset into scheduled tasks (17 Jun); new desktop app with Work and Codex (9 Jul); voice in Work and Codex (23 Jul); Computer History (13 Aug); webhook triggers (25 Aug); dots and Codex Cloud (29 Sep); Live Dictation (18 Nov) | [V: release notes](https://help.openai.com/en/articles/6825453-chatgpt-release-notes), [dots](https://openai.com/dots/) |
| Apple | macOS 27 with Siri AI, Siri app, on-screen awareness (Shift-Cmd-Space), actions across apps, memory in 27.2, "Describe a Shortcut" | [V: Apple](https://support.apple.com/en-us/127257) |
| Google | Gemini Mac app (Option+Space, screen sharing); Gemini Spark 24/7 agent; Workspace remote MCP servers in Developer Preview | [V: blog](https://blog.google/innovation-and-ai/products/gemini-app/gemini-app-now-on-mac-os/), [V: Workspace MCP](https://developers.google.com/workspace/guides/configure-mcp-servers), Spark [U] |
| Microsoft | Copilot app unifies Chat and Cowork; Automations; Autopilot always-on agent; Work IQ MCP | [V: Oct 2026 post](https://techcommunity.microsoft.com/blog/microsoft-copilot-blog/what's-new-in-microsoft-copilot--october-2026/4568528), [V: Learn](https://learn.microsoft.com/en-us/microsoft-agent-365/tooling-servers-overview) |

### Likely 12-month direction (inference)

- **Anthropic**: fold Duet into the main app and desktop; make Quick Entry a richer command surface; extend voice from dictation to conversation in Claude Code. Signals: Cowork was merged into chat within nine months of launch, and Duet "carries over your memory, skills, and connected apps".
- **OpenAI**: ship a Sky-style overlay inside the ChatGPT desktop app, with Computer History as context and voice as the input. Signals: the Sky team was acquired for exactly this, voice already drives Work, Codex and computer use.
- **Apple**: Siri AI gains more third-party actions as developers mark App Intents as tools. It will stay orchestrator-only, with no scheduling or coding agents.
- **Google**: Spark reaches the Mac app and Chrome; third-party MCP arrives.
- **Microsoft**: Autopilot from Frontier to general availability, enterprise-first.

### What they are structurally unlikely to do

| Property | Why it is safe ground | Caveat |
| --- | --- | --- |
| Multi-vendor coding-agent dispatch | Each vendor dispatches only its own agent (Claude → Claude Code, ChatGPT → Codex) | ACP makes cross-vendor cheap for third parties, so Happy, Paradev, Conductor and Nimbalyst already compete here |
| Bring-your-own-key and model choice | Consumer apps are tied to subscriptions | Raycast Pro offers BYOK |
| Open source and local data | The 2026 trend is the opposite: agents on vendor cloud VMs with their own browser | OpenClaw, Hermes, goose and Omi cover "open" already |
| Small native footprint | Vendor desktop apps are large; ChatGPT's is reported at 1.45 GB [U] | Not measured here |
| System-wide dictation into any app | Claude and ChatGPT dictate into their own input box | Raycast and dedicated dictation apps do this well |
| Linux | Only Anthropic ships a Linux desktop beta | — |

### Exposure by concept part (inference)

| Concept part | Exposure | Reason |
| --- | --- | --- |
| Crons and proactive nudges | Very high | Shipped by Anthropic, OpenAI, Google, Meta, Microsoft, Hark, Lindy, Poke, Littlebird, Highlight |
| Quick commands through connectors with confirm | High | Claude connectors, ChatGPT plugins, Raycast AI Extensions |
| Meeting notes to actions | High | Other track; Highlight, Littlebird, Omi, Claude Meetings |
| Push-to-talk dictation | High | Other track; also ChatGPT Live Dictation, Raycast, Claude Quick Entry |
| Coding-agent dispatch, single vendor | High | Claude agent view with push-to-talk; ChatGPT voice to Codex |
| Coding-agent dispatch, multi-vendor, by voice | Low to medium | Only small open-source projects |
| The unified pill itself | Medium | Omi has it; OpenAI likely to ship one |

---

## 8. D. Coding-agent orchestration

### First-party surfaces

| Agent | Start from outside the terminal | Follow | Notes |
| --- | --- | --- | --- |
| **Claude Code** | `claude --bg "<prompt>"` starts a background session with no terminal attached; `claude agents` is the dashboard (research preview); cloud sessions from web, phone or desktop; cloud scheduled tasks with **API trigger** (HTTP POST with bearer token) and GitHub triggers; Dispatch (closed to new users); Slack via Claude Tag; channels plugins for Telegram, Discord, iMessage | `claude agents --json`, `claude logs <id>`, `claude attach <id>`, `claude stop <id>`; Remote Control from claude.ai or the mobile app with push notifications; desktop OS notifications | Remote Control and voice dictation need a claude.ai subscription and do not work with API keys. Cloud scheduled tasks: 1-hour minimum interval, 100 scheduled runs per hour per account. [V: [agent view](https://code.claude.com/docs/en/agent-view), [remote control](https://code.claude.com/docs/en/remote-control), [routines](https://code.claude.com/docs/en/routines), [channels](https://code.claude.com/docs/en/channels)] |
| **Devin** | MCP server `https://mcp.devin.ai/mcp` (streamable HTTP, bearer API key; tools `devin_session_create`, `devin_session_interact`, `devin_session_events`, `devin_schedule_manage`); REST API v3 (`POST /v3/organizations/{org_id}/sessions`, `cog_` service-user keys; v1 and v2 deprecated) | `devin_session_interact`, `devin_session_events` | OAuth is not described for the MCP server; API key only. A "Devin Desktop" app exists and is itself an ACP client. [V: [Devin MCP](https://docs.devin.ai/work-with-devin/devin-mcp), [API](https://docs.devin.ai/api-reference/overview), [ACP clients](https://agentclientprotocol.com/overview/clients)] |
| **Codex** | Codex app (macOS, Windows), Codex Cloud, Codex Remote from the ChatGPT mobile app, dots | In-app, mobile approval | Voice in the desktop app works with Codex. CLI is Apache-2.0, 136,332 stars. [V: release notes, [repo](https://github.com/openai/codex)] |
| **Cursor** | Cloud Agents API v1 (public beta): `POST /v1/agents`, follow-up runs with `delivery: "steer"`, SSE streams | `GET /v1/agents/{id}`, run status, streams; webhooks "coming soon" in v1 | Native ACP agent. [V: [API](https://cursor.com/docs/cloud-agent/api/endpoints), [ACP agents](https://agentclientprotocol.com/overview/agents)] |

### Third-party orchestrators

| Tool | Licence | State | Agents | Voice |
| --- | --- | --- | --- | --- |
| [Conductor](https://www.conductor.build/) | Closed | v0.99.0, macOS only | Claude Code, Codex, Cursor in isolated workspaces | Not mentioned [V] |
| [Happy](https://github.com/slopus/happy) | MIT | Active, 24,788 stars | Claude, Codex, Grok, any ACP agent | Experimental [U] |
| [Nimbalyst](https://github.com/nimbalyst/nimbalyst) (was Crystal) | MIT | Active, Electron | Codex, Claude Code, OpenCode | iOS companion: "reply by text or voice" [V] |
| [Paradev](https://github.com/ParadevAI/Paradev) | MIT | Active, 16,555 stars | Multiple | Not checked |
| [Vibe Kanban](https://github.com/BloopAI/vibe-kanban) | Apache-2.0 | **Sunsetting** | 10+ agents | No |
| [Claude Squad](https://github.com/smtg-ai/claude-squad) | **AGPL-3.0** | 8,863 stars, last release Aug 2026 | Claude Code, Codex, OpenCode, Amp | No |
| [opcode](https://github.com/winfunc/opcode) | **AGPL-3.0** | 22,464 stars | Claude Code | No |
| [coder/agentapi](https://github.com/coder/agentapi) | MIT | 1,493 stars | HTTP wrapper for six CLIs | No |

### Is there a voice front end?

- **First party, single vendor**: Claude Code hold-to-talk dictation works in agent view's dispatch input and peek replies [V: [voice](https://code.claude.com/docs/en/voice-dictation)]. ChatGPT Voice works in Codex on desktop [V: release notes].
- **Desktop overlay, multi-agent**: Omi's floating bar launches agent pills for Claude, Hermes and OpenClaw [V: source].
- **Mobile**: Happy, Nimbalyst iOS, and small projects such as DashVox, m'kay and Omnara [U]. The ACP client list includes one entry labelled "VACP (voice control)" [V].
- **Messaging**: Poke lists Cursor and Devin integrations and accepts voice messages [V].
- Nothing found that does voice-first start-and-follow across Claude Code, Devin and Codex from a desktop overlay.

### ACP status, late 2026

- Repo [agentclientprotocol/agent-client-protocol](https://github.com/agentclientprotocol/agent-client-protocol): Apache-2.0, 4,730 stars, ~252 contributors, schema v1.35.0 (23 Nov 2026). "The current stable ACP protocol version is `1`"; a v2 schema directory exists. Official SDKs for Rust, Python, Java, Kotlin. [V]
- Agents: Gemini CLI, Cursor, goose, OpenCode, Kiro CLI, Junie, GitHub Copilot (public preview) listed directly; **Claude through Zed's SDK adapter; Codex through an adapter**; Devin and Amp not listed. [V: [agents](https://agentclientprotocol.com/overview/agents)]
- Clients: Zed, JetBrains, Neovim, Emacs, VS Code extensions, Obsidian plugins, dozens of desktop, mobile and chat-bridge apps, Devin Desktop, Happy. [V: [clients](https://agentclientprotocol.com/overview/clients)]
- goose pins `agent-client-protocol = 2.2.0` and uses ACP between its own desktop UI and backend [V: goose `Cargo.toml`].
- Inference: ACP is a safe common layer for *local* agents (Claude through the adapter, Codex, Cursor, goose). Cloud agents (Devin, Claude cloud sessions, Cursor cloud agents, Codex Cloud) still need their own API or MCP server. Design the dispatch trait with two implementations: ACP subprocess and remote API.

---

## 9. E. Connectors

| Service | Official remote MCP | Auth | Limits on a third-party client | Source |
| --- | --- | --- | --- | --- |
| **Linear** | `https://mcp.linear.app/mcp` (streamable HTTP); read-only variant at `/mcp/readonly`; SSE deprecated | OAuth 2.1 **with dynamic client registration**; or bearer OAuth token or Linear API key | No client allow-list stated. 25-day refresh window. Tools cover issues, projects, comments, documents | [V](https://linear.app/docs/mcp) |
| **GitHub** | `https://api.githubcopilot.com/mcp/`; per-toolset paths such as `/x/issues`, `/x/projects`; read-only suffix | "One-click OAuth" in supported hosts; or PAT bearer | Available to all GitHub users; some tools need Copilot. Under org OAuth-app restrictions, each host's OAuth App must be enabled. Docs do not say arbitrary clients can self-register, so **plan on a PAT** (inference) | [V](https://docs.github.com/en/copilot/how-tos/provide-context/use-mcp/set-up-the-github-mcp-server), [V](https://github.com/github/github-mcp-server/blob/main/docs/remote-server.md) |
| **Atlassian** | `https://mcp.atlassian.com/v2/mcp` (v2 "recently released") | OAuth 2.1; API token optional | "Any app with MCP support". Covers Jira, JSM, Confluence, Bitbucket, Loom. Rejects custom URL-scheme redirects, so use a loopback redirect [V: Raycast 2.0 changelog] | [V](https://support.atlassian.com/atlassian-rovo-mcp-server/docs/getting-started-with-the-atlassian-remote-mcp-server/) |
| **Google Workspace** | Per-product servers, e.g. `https://gmailmcp.googleapis.com/mcp/v1`, `https://calendarmcp.googleapis.com/mcp/v1` | OAuth 2.0 with a client ID and secret from **your own Google Cloud project** | **Developer Preview Program** only. No dynamic registration mentioned. Personal mode works for your own account without app verification | [V](https://developers.google.com/workspace/guides/configure-mcp-servers) |
| **Microsoft 365** | Work IQ MCP (single endpoint, 10 generic tools); older per-workload servers (Mail, Calendar, Teams) are preview and marked "backward compatibility" only | Entra ID | Tenant registry, admin approval and policy. Copilot licence required [U]. Personal Microsoft accounts not addressed | [V](https://learn.microsoft.com/en-us/microsoft-agent-365/tooling-servers-overview) |
| **Notion** | `https://mcp.notion.com/mcp`; SSE endpoint retires 23 Feb 2027 | OAuth; headless MCP tokens on Business and Enterprise only | Any streamable-HTTP client can connect; guests cannot | [V](https://developers.notion.com/guides/mcp/get-started-with-mcp) |
| **Slack** | `https://mcp.slack.com/mcp` (streamable HTTP only) | Confidential OAuth with your Slack app's client ID and secret; PKCE for desktop | **No dynamic client registration. Requires a registered Slack app; only Marketplace-published or internal apps; unlisted apps prohibited.** Admin approval. Web API rate limits apply | [V](https://docs.slack.dev/ai/slack-mcp-server/) |

Community alternative for Google: `taylorwilsdon/google_workspace_mcp` covers Gmail, Calendar, Drive and more [U]; check its licence before use.

Implications (inference):

- Phase 1 with Linear is the easiest possible start: dynamic registration, or just an API key in the keychain.
- GitHub Issues and Projects: PAT in the keychain.
- Mail and calendar are the hard part for either provider. For personal use, Google "personal mode" with your own Cloud project is workable but in preview. For Microsoft 365, a personal tool has no clean path without tenant admin consent.
- Slack is impractical for an open-source desktop client that users install themselves, because each user would need an internal Slack app.

### `rmcp` (official Rust SDK)

- Version **3.11.0**, published 17 Nov 2026. 1.0.0 on 3 Mar 2026, 2.0.0 on 29 Jun 2026, 3.0.0 on 28 Jul 2026. 74 releases, ~54.5 million downloads. Crate licence Apache-2.0; the repo is mid-transition from MIT to Apache-2.0. [V: [crates.io API](https://crates.io/crates/rmcp), [repo](https://github.com/modelcontextprotocol/rust-sdk)]
- Implements the **MCP 2026-07-28** specification and stays compatible with 2025-11-25 and earlier. New in 2026-07-28: server discovery and negotiation, transport-neutral subscriptions, long-running tasks, response caching, standard HTTP headers. [V: README]
- Streamable HTTP client (`StreamableHttpClientTransport`, reqwest feature) and server (Tower service); stdio and child-process transports. [V: README]
- OAuth (`auth` feature): OAuth 2.1 with PKCE S256, RFC 8707 resource binding, RFC 9728 and RFC 8414 discovery, **dynamic client registration (RFC 7591)**, **Client ID Metadata Documents**, scope upgrade on 403, automatic refresh, optional enterprise-managed authorization. [V: [OAUTH_SUPPORT.md](https://github.com/modelcontextprotocol/rust-sdk/blob/main/docs/OAUTH_SUPPORT.md)]
- Caveat: three major versions in five months. Pin the version and read the [3.x migration guide](https://github.com/modelcontextprotocol/rust-sdk/discussions/969). The OAuth doc's own example still shows `version = "0.1"`, so examples lag the code. goose is a production reference on 3.9.0.

---

## 10. F. Overlay UX patterns worth copying

| Pattern | Where to look | What to take |
| --- | --- | --- |
| **Stay above fullscreen apps without stealing focus (Swift)** | Omi [`FloatingControlBarWindow.swift`](https://github.com/BasedHardware/omi/blob/main/desktop/macos/Desktop/Sources/FloatingControlBar/FloatingControlBarWindow.swift) | `NSPanel` with `styleMask: [.borderless, .nonactivatingPanel]`, `isFloatingPanel = true`, `hidesOnDeactivate = false`, collection behaviour `[.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]`, `canBecomeKey = true`, `canBecomeMain = false`. Level is raised to `.popUpMenu` only while a menu is tracking. [V] |
| **Passive versus explicit presentation** | Omi [`FloatingControlBar/ARCHITECTURE.md`](https://github.com/BasedHardware/omi/blob/main/desktop/macos/Desktop/Sources/FloatingControlBar/ARCHITECTURE.md) | "Only the explicit request may acquire key-window or text-input focus." Notifications, snooze expiry and tool overlays order the panel forward without making it key, and re-hide when their queue drains. This rule is the fix for a real bug: the 20 Nov 2026 changelog says "The Floating Bar no longer steals keyboard focus". [V] |
| **Same thing in Tauri 2** | Thuki [`src-tauri/src/lib.rs`](https://github.com/quiet-node/thuki/blob/main/src-tauri/src/lib.rs) and [`tauri-nspanel`](https://github.com/ahkohd/tauri-nspanel) (`v2.1` branch) | Convert the Tauri window to an NSPanel "something a standard NSWindow cannot do regardless of window level". Thuki notes that a nonactivating panel under Accessory activation policy needs a tracking area that makes the panel key on cursor-enter, otherwise clicks and typing die after defocus. [V] |
| **Global double-tap hotkey** | Thuki [`activator.rs`](https://github.com/quiet-node/thuki/blob/main/src-tauri/src/activator.rs) | `CGEventTap` on its own run loop watching Control key transitions; 400 ms double-tap window, 600 ms cooldown; needs Accessibility permission, with self-diagnosis and prompting. [V] |
| **One microphone state machine** | Omi ARCHITECTURE.md, "Push-to-talk entry points" | Global shortcut, automation bridge and mic button all enter one `PushToTalkManager`. Hold = push-to-talk; double-tap or click = hands-free lock. [V] |
| **Dictate-or-ask routing in one key** | Omi ARCHITECTURE.md, "Voice typing" | A turn that opens with the word "type" is dictation into the focused app; otherwise it is a question. An on-device probe decides early; the decision latches one way; the pill tints red over 0.28 s to show dictation mode. [V] |
| **Agent pills** | Omi [`AgentPill.swift`](https://github.com/BasedHardware/omi/blob/main/desktop/macos/Desktop/Sources/FloatingControlBar/AgentPill.swift) | States queued, starting, running, done, stopped, failed, each with a tint; provider logo as identity. [V] |
| **Agent list grouped by need** | Claude Code [agent view](https://code.claude.com/docs/en/agent-view) (screenshot on the page) | Rows grouped "Needs input / Working / Completed", latest status line per row, one input at the bottom to dispatch. [V] |
| **Quick overlay with context attach** | Claude [Quick Entry](https://support.claude.com/en/articles/12626668-use-quick-entry-with-claude-desktop-on-mac) | Double-tap a modifier; drag to screenshot or click a window to attach; a separate dictation key that is off by default because it takes over Caps Lock. [V] |
| **Approval tiers** | [OpenAI dots](https://openai.com/dots/), [Claude Duet](https://claude.com/resources/articles/claude-duet), [Littlebird](https://littlebird.ai/) | Three tiers everywhere: free to read and draft; ask before send, pay, sign, delete; never (trades, password changes). Per-action Custom Rules (allow, require approval, block). Email as draft only. This matches the project's own rule. [V] |
| **Cursor-side companion** | [Clicky demo GIF](https://github.com/farzaa/clicky) | Overlay that follows the cursor and can point at UI. Clicky also keeps API keys in a small proxy so they never ship in the app. [V] |
| **Hide from screen share** | [Cluely](https://cluely.com/) | "Invisible to screen share". Inference: `NSWindow.sharingType = .none`. Useful during meetings. |

---

## 11. Decision questions

### 1. Does a product already exist that makes building this pointless for personal use?

No single product, but the answer is close to yes for function.

- **Claude Pro at $20/month ($17 annual), or Max at $100** [V: pricing], combined with the dictation app already in use, covers quick commands through connectors, scheduled jobs, and starting and following Claude Code by voice. The developer already pays for this.
- **Omi desktop** (MIT, free tier, Pro $29.99/month) is the only shipping product with the same *shape*: floating bar, push-to-talk, voice typing, agent pills, MCP, nudges. It should be installed and used for a week before any code is written. If its capture-everything model and cloud backend are acceptable, most of the concept is already there.
- What neither gives: one push-to-talk key that routes to dictation, a previewed command or an agent; multi-vendor agent following; local data with bring-your-own-key; a small Rust footprint.

Inference: building is justified as a personal tool only if the unified pill and the latency are the point. It is not justified for crons, connectors or proactive briefs alone.

### 2. Which single capability is least served, and is therefore the best wedge?

**Voice-first start-and-follow of coding agents across vendors, from a persistent pill.**

- Vendors do it for their own agent only (Claude agent view; ChatGPT voice to Codex).
- Orchestrators (Conductor, Happy, Nimbalyst, Paradev) are window or phone apps without a voice overlay.
- Omi's pills cover Claude, Hermes and OpenClaw, not Devin, Codex or Cursor.
- The plumbing is ready: `claude --bg` and `claude agents --json`, Devin's MCP server, Cursor's Cloud Agents API, ACP for local agents.

Second-best wedge: the one-key router (dictate, previewed command, dispatch). Worst wedges: crons and proactive nudges, where six large vendors shipped in six months. For those, call into what exists (for example, a Claude cloud scheduled task fired through its API trigger) rather than building a scheduler first.

### 3. What to borrow from each open-source project

| Project | Licence | Borrow | Code or ideas | Caveat |
| --- | --- | --- | --- | --- |
| Omi | MIT | NSPanel configuration, passive-focus policy, single push-to-talk state machine, "type" routing, agent pill states, notification queue | Ideas, and small Swift snippets translated to Rust/objc2 | Record source and commit in `NOTICE.md`. It is Swift; nothing drops into a Tauri app unchanged |
| Thuki | Apache-2.0 | Tauri 2 NSPanel setup, hover-to-key tracking area, CGEventTap double-tap activator, SQLite layout | Code is directly reusable (same stack) | Maintenance mode: fork-and-own, expect no upstream fixes. Keep the Apache NOTICE |
| tauri-nspanel | Apache-2.0 (per GitHub) | Dependency | Code | Git dependency on a `v2.1` branch, not a crates.io release; pin a commit |
| goose | Apache-2.0 | Recipe file format, scheduler design, `rmcp` 3.x usage, ACP client code | Ideas first; small code where useful | oi-kit is alpha. Do not adopt the runtime: it conflicts with "no agent framework" |
| IronClaw | Apache-2.0 | Credential injection at the host boundary, endpoint allow-listing, leak detection | Ideas | `main` idle since Sep 2026 |
| ZeroClaw | Apache-2.0 / MIT | Approval gates, risk profiles, tool receipts, cron config in TOML | Ideas | Large surface; do not embed |
| OpenClaw | MIT | Skills as files, automation payload model (main vs isolated session), delivery modes | Ideas | TypeScript; nothing to link |
| Hermes Agent | MIT | Skill self-improvement loop, FTS5 session search, cron delivery | Ideas | Python |
| Clicky / OpenClicky | MIT | Cursor-side overlay, key-proxy pattern | Ideas | Upstream frozen |
| Happy | MIT | Multi-agent session model, encrypted relay, push design | Ideas | — |
| coder/agentapi | MIT | Uniform HTTP control of CLI agents | Ideas or use as a sidecar | Go binary adds weight |
| screenpipe | Commercial | Nothing | Read only | Not open source any more |
| Claude Squad, opcode | AGPL-3.0 | Nothing | Read only | Copyleft; incompatible with MIT/Apache distribution |
| Open Interpreter, Leon | Apache-2.0, MIT | Nothing relevant now | — | Repurposed / unreleased |

### 4. Top three platform-risk threats

| # | Threat | Likelihood in 12 months (inference) | Exposed parts | Less exposed parts |
| --- | --- | --- | --- | --- |
| 1 | **Anthropic turns Quick Entry + Duet + agent view into one desktop surface.** All pieces exist today in separate places | High | Quick commands, connectors, crons, nudges, Claude Code dispatch | Devin and Codex following, BYOK, local data, dictation into other apps |
| 2 | **OpenAI ships the Sky-derived overlay in the ChatGPT desktop app**, voice-first, with Computer History and computer use | High | The overlay form factor itself, voice commands, dictation, scheduled tasks | Anything Claude- or Devin-specific; open source; footprint |
| 3 | **Access rules, not features.** (a) Anthropic does not allow third parties to offer claude.ai login or subscription limits without approval [V], so a distributed build must use API keys or drive the user's own installed CLI. (b) Slack's MCP server bars unlisted apps; Google's is preview-only with your own Cloud project; Microsoft's is tenant-gated [V]. (c) MCP and `rmcp` shipped three major versions in 2026 | Certain, already true | Connectors for mail, calendar and chat; any open-source distribution; maintenance cost | Personal use with your own keys; Linear and GitHub connectors |

Apple's Siri AI is a fourth, lower threat: it will absorb simple app actions and on-screen questions on the Mac, but it has no scheduling, no third-party agent orchestration and no coding agents [V: Apple page shows none].

---

## 12. Source list (primary sources opened today)

Anthropic
- Release notes: https://support.claude.com/en/articles/12138966-release-notes
- Quick Entry: https://support.claude.com/en/articles/12626668-use-quick-entry-with-claude-desktop-on-mac
- Dispatch: https://support.claude.com/en/articles/13947068-assign-tasks-from-anywhere-in-claude-cowork
- Claude Duet: https://claude.com/resources/articles/claude-duet
- Pricing: https://claude.com/pricing
- Claude Code docs index: https://code.claude.com/docs/llms.txt (pages: remote-control, routines, desktop, desktop-scheduled-tasks, scheduled-tasks, agent-view, voice-dictation, channels, headless, agent-sdk/overview)

OpenAI
- ChatGPT release notes: https://help.openai.com/en/articles/6825453-chatgpt-release-notes
- dots: https://openai.com/dots/
- Pricing: https://chatgpt.com/pricing

Apple, Google, Microsoft, Perplexity
- https://support.apple.com/en-us/127257
- https://blog.google/innovation-and-ai/products/gemini-app/gemini-app-now-on-mac-os/
- https://developers.google.com/workspace/guides/configure-mcp-servers
- https://techcommunity.microsoft.com/blog/microsoft-copilot-blog/what's-new-in-microsoft-copilot--october-2026/4568528
- https://learn.microsoft.com/en-us/microsoft-agent-365/tooling-servers-overview
- https://www.perplexity.ai/hub/blog/personal-computer-is-here

Products
- Raycast: https://www.raycast.com/pricing, https://www.raycast.com/changelog, https://www.raycast.com/changelog/macos/2-0
- https://highlightai.com/ and https://highlightai.com/pricing
- https://cluely.com/ · https://littlebird.ai/ · https://poke.com/ · https://www.lindy.ai/pricing · https://www.omi.me/ · https://www.conductor.build/

Coding agents and protocols
- https://docs.devin.ai/work-with-devin/devin-mcp · https://docs.devin.ai/api-reference/overview
- https://cursor.com/docs/cloud-agent/api/endpoints
- https://agentclientprotocol.com/overview/agents · https://agentclientprotocol.com/overview/clients

Connectors
- https://linear.app/docs/mcp
- https://docs.github.com/en/copilot/how-tos/provide-context/use-mcp/set-up-the-github-mcp-server
- https://github.com/github/github-mcp-server/blob/main/docs/remote-server.md
- https://support.atlassian.com/atlassian-rovo-mcp-server/docs/getting-started-with-the-atlassian-remote-mcp-server/
- https://developers.notion.com/guides/mcp/get-started-with-mcp
- https://docs.slack.dev/ai/slack-mcp-server/
- https://github.com/modelcontextprotocol/rust-sdk and `docs/OAUTH_SUPPORT.md`; https://crates.io/crates/rmcp

OpenClaw docs
- https://docs.openclaw.ai/platforms/macos · https://docs.openclaw.ai/automation/cron-jobs

GitHub repositories (API and files): all linked inline in sections 2, 5, 8 and 10.

Secondary sources used only for [U] claims: 9to5Mac, TechCrunch, VentureBeat, Digital Trends, MacRumors, The Decoder and similar, reached through search.

## 13. Gaps and limits of this research

- No footprint measurements. Any claim about memory is inference from the stack.
- ChatGPT pricing figures, the Sky acquisition details, Gemini Spark, Meta Muse, Hark, Manus and Martin rest on secondary sources.
- I did not install or run any product. "Lacks" entries for Omi are read from the source tree and file names, not from use.
- ChatGPT release notes were read from November back to January 2026; "record mode" did not appear in what I read, so its current state is unknown.
- GitHub's OAuth behaviour for an arbitrary self-registered MCP client was not confirmed either way.
- The official `plugin:github` MCP server in this session failed to connect; the `gh` CLI was used instead, so nothing was lost.
