# Competitors, and signing in with a paid Claude or ChatGPT plan

Research of 26 November 2026, at the owner's request. Two questions:

1. What do the products around HyperMe ship, at what price, and what do they leave open? Which
   features should HyperMe add beyond the ones already in [PRD.md](../PRD.md) section 7?
2. Can HyperMe let a user "sign in with" a paid Claude or ChatGPT plan and answer questions
   through that plan instead of an API key?

Most sources are third-party reviews, and many of them are written by competing dictation apps
(Voibe, Voicy, Spokenly, Morgen). Their prices and complaints are marked as such and are
unverified until checked on the vendor's own page. Primary sources are named where they were read.
Prices are US dollars per user per month unless stated.

## Verdict in short

- **ChatGPT: allowed, through OpenAI's official "Sign in with ChatGPT".** Since 29 September 2026
  an open-source app that runs locally can let a Plus or Pro user connect their plan, and HyperMe's
  answers then count against that plan's limits. It covers text only: no audio input and no
  transcription API, so dictation still needs a speech key. It is a preview and may change.
- **Claude: not allowed.** Anthropic's terms say third-party developers may not offer Claude.ai
  login or route requests through a user's Free, Pro or Max credentials, and it enforces this
  without notice. The allowed routes are an API key (or a cloud provider), or the reverse
  direction: HyperMe as an MCP server that the user's own Claude app calls.
- **Features to add** beyond the plan, in order: voice edits of selected text; HyperMe as an MCP
  server; "Sign in with ChatGPT" for answers; importing a Wispr Flow or Superwhisper dictionary;
  pausing media and choosing the microphone while dictating. Details in
  [section 3](#3-what-hyperme-should-add).

## 1. The field

### Dictation apps

| Product | What it ships | Price | Platforms | What it leaves open |
| --- | --- | --- | --- | --- |
| **Wispr Flow** | Cleanup, learned dictionary, snippets, Command Mode (select text, speak an edit; Pro, desktop, under Experimental), bot-less Notetaker, MCP server for its notes | Free 2,000 words a week on desktop; Pro $15, or $12 yearly; Growth from $18 yearly; Enterprise | Mac, Windows, iOS, Android | Cloud only, no offline mode. Electron, about 800 MB idle on Windows (independent review, via eesel). Early-2026 report that it uploaded window screenshots; the CTO apologised. Command Mode edits text but takes no action in other apps. Closed, no own keys |
| **Superwhisper** | Modes (prompt per use), local Whisper and Parakeet models, cloud models, own API keys on Pro, file transcription, Meetings on macOS | Free local dictation; Pro $8.49, $84.99 a year, or $249.99 once ([official plans page](https://superwhisper.com/docs/billing/plans.md)) | Mac, Windows, iPhone, Android | Setup is complex; audio kept on disk with no switch and keys stored as plain JSON (Spokenly, a competitor, unverified). No actions, tasks or mail |
| **Willow** | Cleanup, style per app, "AI Mode" turning notes into messages, "Hey Willow" desktop assistant, offline mode on Mac and iOS (opt-in) | Free 2,000 words a week; $15, or $144 a year; Team $10 yearly, 3 seats minimum (Voibe) | Mac, Windows, iPhone, Android | Cloud first. No actions beyond writing |
| **Aqua Voice** | Own "Avalon" speech model, custom instructions, dictionary (800 words on Pro), realtime mode and voice commands on Max, MCP and CLI | Free 1,000 words once; Pro $10 to $12; Max $19 to $24 (Voibe). Voicy gives lower prices; the vendor page was not read | Mac, Windows, iPhone keyboard | Cloud only. Transcripts stored unless Privacy Mode is on |
| **OpenWhispr** (MIT) | Local, own-key or hosted speech; cleanup; dictionary and snippets; voice agent that rewrites a selection; meetings with speaker labels linked to the calendar; file and URL import; REST API and MCP on paid plans | Free with unlimited local dictation; Pro $80 a year; Business $160 a year | Mac, Windows, Linux, iOS | Electron and heavy. Its hosted server is closed, so only the client is open. No tasks or mail |
| **Spokenly** | Local Parakeet and Whisper, own keys at no markup, an MCP server for coding agents | Free; Pro $12 yearly (its own blog) | Mac, Windows, Linux, iPhone, Android | Dictation only |

### Assistants and launchers

| Product | What it ships | Price | What it leaves open |
| --- | --- | --- | --- |
| **Raycast** | Launcher plus dictation with a pill, styles per app, App Context (reads the visible window, needs Screen Recording), dictation history with regenerate, "Dictate to AI", mic priority list, "keep microphone warm", pauses playing audio, import from Wispr Flow, Superwhisper, Aqua and others ([manual](https://manual.raycast.com/ai/dictation)). MCP client, own keys | Dictation is Pro only: Pro $10, or $8 yearly (Magica, secondary) | 20 minutes per dictation, 50-word vocabulary. 350 to 450 MB after the 2.0 rewrite (Magica). No tasks, nudges or meeting notes |
| **ChatGPT desktop** | "ChatGPT Voice" on GPT-Live (July 2026) starts and steers Chat, Work and Codex tasks; Appshots give the active window as context; scheduled tasks | Plus and up | Voice drives ChatGPT, not the app you are typing in. No overlay that pastes at the cursor |
| **Claude desktop** | Quick entry with dictation into Claude, connectors, scheduled tasks | Claude plans | Dictation only into Claude. Thin public detail on how it works (Tom's Guide via inkl) |
| **Granola** | Bot-less meeting notes from device audio, templates, chat, MCP; free voice typing into any app since autumn 2026; audio deleted after transcription ([pricing](https://www.granola.ai/pricing)) | Basic free (30 days of history); Business $14; Enterprise $35 | No push of action items to a tracker; voice typing has no commands |

### Planners

| Product | What it ships | Price | What it leaves open |
| --- | --- | --- | --- |
| **Motion** | Auto-schedules tasks around meetings and re-plans, own task manager, AI credits | About $19 yearly, $29 to $34 monthly (Morgen, a competitor; dev.to) | Moves tasks without asking: the most cited complaint. No Linux, no voice |
| **Sunsama** | Guided daily planning and shutdown ritual; pulls tasks from Asana, Trello, Todoist, Jira, Gmail | About $16 to $17 yearly, $20 to $22 monthly (same sources) | Manual planning, weak mobile, no voice capture |
| **Akiflow** | One inbox from Todoist, Gmail, Slack, Notion; command bar; time blocking | About $19 yearly (dev.to) | No voice, no AI scheduling |

### What this shows

- **Everyone now ships dictation.** Granola gives it away, Raycast bundles it, ChatGPT and Claude
  have it inside their apps. Dictation alone is not a moat; speed, lightness and what happens after
  the words are.
- **Common features HyperMe has not planned yet:** speaking an edit to selected text (Wispr Flow,
  Raycast, OpenWhispr), an MCP server so other agents can read your notes and dictations (Wispr
  Flow, Granola, Aqua, OpenWhispr, Spokenly), import from the app you are leaving (Raycast), and
  microphone handling (Raycast). Mobile keyboards are everywhere but stay a non-goal.
- **Nobody combines** dictation, actions in other apps behind a preview, a daily task list with a
  morning card, and meetings that turn into tickets, in one light open-source app on your own keys.
  That is still HyperMe's gap to fill ([RESEARCH.md](../RESEARCH.md)).
- **Weak spots to compete on:** memory (Wispr Flow about 800 MB, Raycast 350 to 450 MB, against
  HyperMe's 100 MB budget), privacy incidents (Wispr Flow screenshots), auto-scheduling without
  consent (Motion), closed servers behind "open source" (OpenWhispr), and Linux (only OpenWhispr
  and Spokenly).

## 2. Signing in with a paid plan

### ChatGPT: "Sign in with ChatGPT"

Announced at OpenAI DevDay on 29 September 2026, after a July beta. Read on OpenAI's own pages
on 26 November 2026: the [quickstart](https://developers.openai.com/siwc/quickstart), the
[open-source sign-in guide](https://developers.openai.com/siwc/token-sharing-open-source/sign-in),
the [preview limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations)
and the [help article](https://help.openai.com/en/articles/20001542-using-your-chatgpt-plan-in-other-apps-and-sites).
Parts of the developer pages were read through a summary; check them again before building.

- **Who may use it.** "ChatGPT plan usage is available to all open-source partners and selected
  private clients." Commercial apps fill in an interest form. HyperMe, open source and running on
  the user's Mac, fits the open-source route.
- **Plans.** The help article says Plus and Pro; the Learn page says Go, Plus, Pro and Business.
  Plus and Pro are the safe reading.
- **Whose usage.** "App usage counts toward your existing plan limits. Connecting an app does not
  add a new allowance." The user can cap each app as a share of the weekly allowance, and can
  disconnect in ChatGPT settings. The app does not see the user's chats or memories.
- **How.** OpenID Connect with PKCE from a desktop app, a loopback redirect on `127.0.0.1`, no
  client secret; the first sign-in registers a client per account. The app then calls the
  Responses API with the access token, `store: false` and `stream: true`, and refreshes the token
  itself. Tokens belong in the keychain, which HyperMe already requires.
- **Limits.** Text, images and files only: "Audio/video input, the Files upload API, and the
  transcription API are not supported by this flow." No hosted MCP, file search or image
  generation. The app picks from the models the account lists, so no model name is fixed in code.
- **Branding.** The button says "Continue with ChatGPT" or "Sign in with ChatGPT", next to other
  options and with comparable prominence.
- **Not coding only.** Partners include Notion and Vercel, and nothing read limits the use case.

Who uses it: OpenCode, OpenClaw, Pi and T3 are the listed open-source integrations; Warp, Kilo
Code, Amp, Devin, Notion and others are commercial partners. **Cline** offers "OpenAI Codex
(Subscription OAuth)" through the Codex CLI's own login client, which is not OpenAI's program
and is not on OpenAI's list (dbggr.dev and enjoy.dev; unverified). HyperMe should not copy that
pattern.

### Claude: no third-party sign-in

Read on Anthropic's [legal and compliance page](https://code.claude.com/docs/en/legal-and-compliance)
on 26 November 2026 (the page shows no date):

- Subscription sign-in is "designed to support ordinary use of Claude Code and other native
  Anthropic applications".
- "Anthropic does not permit third-party developers to offer Claude.ai login into their own
  applications", nor to route requests through subscription credentials for their users. Developers
  building products, including on the Agent SDK, "should use API key authentication through Claude
  Console or a supported cloud provider".
- Anthropic "may [enforce] without prior notice".

History, from secondary sources: server-side blocks of third-party clients from 9 January 2026;
the terms wording from about 18 to 20 February; **OpenCode removed its Claude login on 19 March
"per legal requests"** and moved to Sign in with ChatGPT; tools such as OpenClaw lost subscription
coverage in April. Account suspensions are reported, but only anecdotally.

What remains allowed:

- **An API key**, or Bedrock, Vertex or Foundry. Secondary sources report that since 7 October 2026
  Max and Team plans include monthly API credits for a linked Console organisation (unverified on
  Anthropic's pages). If true, a Max user's API key is partly paid for by the plan already.
- **The unmodified Claude Code binary**, signed in by the user with their own plan. Cline's
  "Claude Code" provider works this way. HyperMe already plans to hand coding tasks to Claude Code;
  using it as a hidden answer engine for a non-coding app is a grey area and not recommended.
- **The reverse direction:** HyperMe runs a local MCP server, and the user adds it to Claude
  desktop (or ChatGPT). Their own Claude app, on their own plan, can then read HyperMe's tasks,
  dictations and meeting notes. This is fully within the rules.

There is no official "Sign in with Claude" programme. The legal page says to contact sales about
other methods.

### What is allowed and what is risky

| Approach | Status | Risk |
| --- | --- | --- |
| Own API keys (today's plan) | Allowed everywhere | None |
| Sign in with ChatGPT, open-source route, for answers | Allowed, official preview | Preview may change; Plus and Pro only; usage counts against the user's ChatGPT limits; speech still needs a key |
| HyperMe as an MCP server inside Claude or ChatGPT | Allowed | None; it is the user's app calling HyperMe |
| Running the user's Claude Code binary for coding hand-off | Allowed | None for coding hand-off |
| Running Claude Code headless as HyperMe's answer engine | Grey | Against the spirit of "ordinary use of Claude Code"; could break or be blocked |
| Own Claude.ai OAuth login in HyperMe | **Prohibited** | Server-side blocks; the user's account may be suspended |
| Reusing another app's OAuth client (Codex CLI, Claude Code) | **Not allowed** | Blocks without notice; harms users and the project |

## 3. What HyperMe should add

Beyond the features already in the PRD, in priority order. Each is a proposal for the owner.

1. **Voice edit of selected text** ("Hyper, make this shorter", "translate this"). Phase 3, P0.
   The most common feature after plain dictation (Wispr Flow Command Mode, Raycast, OpenWhispr,
   Superwhisper modes). The PRD has selected text only as context (7.2, P1). It reuses the cleanup
   path and needs no tool call, so it can ship before MCP actions.
2. **HyperMe as an MCP server.** Phase 3 or 4, P1. Exposes tasks, dictation history, memory and
   later meeting notes to Claude desktop, Claude Code, ChatGPT and Cursor. Five competitors ship it,
   and it is the one fully allowed way for a Claude subscriber to use their plan with HyperMe's
   data. `rmcp` already does servers as well as clients.
3. **Sign in with ChatGPT for the answers model.** Phase 3, P1, as one more provider behind the
   answers trait. It removes the second key for Plus and Pro users. Measure latency first: answers
   have a budget, and cleanup should stay on Groq. Add an open question to the PRD, since it
   changes onboarding.
4. **Import from Wispr Flow and Superwhisper** (dictionary, snippets, styles where their export
   allows). Phase 2, P1, for the first public release. Raycast does this to win switchers, and
   HyperMe's launch audience is exactly those users.
5. **Microphone handling.** Phase 2, P1: pause or duck playing media while recording, a mic
   priority list, and keeping the input warm (Raycast). Cheap, and it shows in daily use.
6. **Evening wrap-up.** Phase 4, P2: a short shutdown card ("what got done, what rolls over"), like
   Sunsama's ritual, beside the morning card. Changes to the calendar always go through the
   preview; Motion's silent reshuffling is its main complaint.
7. **Transcribe a file or a link.** Later, P2 (Superwhisper, OpenWhispr). Useful for voice memos;
   low priority.

Not recommended: a mobile keyboard (non-goal; every competitor has one, so say so plainly in the
pitch), its own hosted cloud, and any unofficial plan login.

## Not checked

- Vendor pricing pages for Wispr Flow, Aqua, Willow, Raycast and OpenWhispr; prices above come
  from reviews and can be out of date.
- Anthropic's support articles on Agent SDK use with plans, and the October 2026 API credits.
- The latency and the model list a Plus account gets through Sign in with ChatGPT.
- OpenAI's UI guidelines page for the sign-in button.

## Sources

Primary: [OpenAI Sign in with ChatGPT quickstart](https://developers.openai.com/siwc/quickstart),
[open-source sign-in](https://developers.openai.com/siwc/token-sharing-open-source/sign-in),
[preview limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations),
[OpenAI help: using your plan in other apps](https://help.openai.com/en/articles/20001542-using-your-chatgpt-plan-in-other-apps-and-sites),
[Learn: Sign in with ChatGPT](https://learn.chatgpt.com/docs/sign-in-with-chatgpt),
[Anthropic legal and compliance](https://code.claude.com/docs/en/legal-and-compliance),
[Superwhisper plans](https://superwhisper.com/docs/billing/plans.md),
[Raycast dictation manual](https://manual.raycast.com/ai/dictation),
[Granola pricing](https://www.granola.ai/pricing).

Secondary: [The New Stack](https://thenewstack.io/sign-in-with-chatgpt/),
[XenoSpectrum](https://xenospectrum.com/en/chatgpt-sign-in-subscription-apps/),
[enjoy.dev](https://enjoy.dev/blog/chatgpt-subscription-other-apps/) (competitor),
[dbggr.dev](https://dbggr.dev/articles/claude-chatgpt-subscription-coding-tools/),
[WinBuzzer](https://winbuzzer.com/2026/02/19/anthropic-bans-claude-subscription-oauth-in-third-party-apps-xcxwbn/),
[GIGAZINE](https://gigazine.net/gsc_news/en/20260220-anthropic-third-party-block/),
[ZBuild](https://www.zbuild.io/resources/news/opencode-blocked-anthropic-2026),
[Autonomee](https://autonomee.ai/blog/claude-code-terms-of-service-explained/),
[eesel on Wispr Flow](https://www.eesel.ai/blog/wispr-flow-overview),
[Voicy on Superwhisper](https://usevoicy.com/blog/superwhisper-pricing) (competitor),
[Spokenly on Superwhisper and Wispr Flow](https://spokenly.app/blog/superwhisper-vs-wispr-flow) (competitor),
[Voibe on Willow](https://www.getvoibe.com/resources/willow-voice-review/),
[Voibe on Aqua](https://www.getvoibe.com/resources/aqua-voice-review/) and
[on OpenWhispr](https://www.getvoibe.com/resources/openwhispr-review/) (competitor),
[scriptbyai on OpenWhispr](https://www.scriptbyai.com/openwhispr-dictation-meeting-notes/),
[Magica on Raycast 2.0](https://magica.com/news/raycast-v2-byok-local-models-pro-only),
[9to5Mac on ChatGPT Voice](https://9to5mac.com/2026/07/23/openai-updating-chatgpt-desktop-app-with-gpt-voice-for-talking-through-work/),
[Morgen on Sunsama and Motion](https://www.morgen.so/blog-posts/sunsama-vs-motion) (competitor),
[pickuma on Motion, Akiflow and Sunsama](https://dev.to/pickuma/motion-vs-akiflow-vs-sunsama-ai-calendar-planners-compared-in-2026-525h).
