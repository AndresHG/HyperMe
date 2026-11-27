# Paid Claude and ChatGPT accounts, and launching agent tasks

Research of 27 November 2026, at the owner's request. **The owner approved both recommendations on 27 November 2026**, and
placed agent tasks after the first-wave commands (tasks, reminders, email drafts) as not urgent;
they are now in the PRD, the roadmap and `CLAUDE.md`. Two separate questions, kept apart:

1. **Paid accounts (near term).** Many users already pay for Claude or ChatGPT. Can they connect
   that account to HyperMe, click and sign in like the Google connector, instead of pasting API
   keys, and use it for transcription, cleanup and quick questions?
2. **Agent tasks (future).** Can HyperMe start a Claude Code or Codex session by voice ("Hyper,
   launch a Claude task to research X"), fire and forget, and could it show their interface
   inside HyperMe?

This extends the [competitor and plan sign-in research](2026-11-26-competitors-and-plan-sign-in.md)
of 26 November (PR #108), which covered "Sign in with ChatGPT" and was approved into the PRD and
roadmap. That part is not redone here; only what is new or changes is added. Pages were read on
27 November 2026, some through a summary; anything secondary or unconfirmed says so.

## Verdict in short

- **No sign-in removes the speech key.** Neither Anthropic nor OpenAI's plan sign-in offers
  speech to text: Anthropic has no transcription API at all, and Sign in with ChatGPT excludes
  audio and the transcription API. Dictation keeps its speech provider key (Speechmatics today).
  A paid account can only power the text roles: cleanup, answers, voice edits and commands.
- **ChatGPT: yes, as already approved.** "Continue with ChatGPT" for answers in phase 3 stands.
  Nothing read since changes it.
- **Claude: still no sign-in.** Anthropic's terms forbid third-party apps from offering Claude.ai
  login or routing requests through a user's Free, Pro or Max plan, and its Usage Policy of
  12 November 2026 repeats it. The one opening is the phrase "unless previously approved", with
  no public programme behind it.
- **But a Claude Max, Ultra or Team plan now pays for an API key.** Since 7 October 2026 those
  plans include monthly API credits ($100 Max, $200 Ultra, up to $500 per Team) on a linked
  Console organisation. A key from that organisation is the allowed way for a Claude subscriber
  to run HyperMe's text roles on their plan's money. HyperMe should say so in onboarding.
- **Recommendation:** add Anthropic as a key provider for cleanup and answers in the spec 96
  catalogue (phase 2, small), with onboarding copy pointing Max, Ultra and Team users to their
  credits; keep "Continue with ChatGPT" in phase 3; keep HyperMe as an MCP server as the way a
  Claude app uses HyperMe's data. Do not run Claude Code or the Agent SDK on the user's plan as
  HyperMe's hidden model.
- **Agent tasks:** launching a task is easy and allowed with each tool's own command line, run by
  the user on their own login; embedding their interface is not worth it. Recommended placement:
  keep it in "Later", as one action behind the usual preview card once command mode exists.
  The owner placed it after the first-wave commands (tasks, reminders, email drafts): not urgent.

---

## Part 1. Using a paid Claude or ChatGPT account

### What each HyperMe role needs

| Role | Needs | Claude plan sign-in | ChatGPT plan sign-in | API key |
| --- | --- | --- | --- | --- |
| Dictation speech | Streaming speech to text, under the 800 ms budget | Not possible: no Anthropic transcription API | Not possible: audio and transcription excluded | Yes (Speechmatics; OpenAI and Groq keys also offer speech) |
| Dictation cleanup | A fast text model, inside the cleanup deadline | Not allowed | Allowed, but latency unmeasured and every dictation spends the user's weekly ChatGPT allowance | Yes (Groq today) |
| Quick answers (phase 3) | A good text model, about a second to first words | Not allowed | Allowed: the approved use | Yes |
| Voice edits, commands (phase 3) | Text model; commands need tool calling | Not allowed | Allowed (the Responses API has tool calling) | Yes |
| Briefings, mail triage (phase 4) | Text model, longer inputs | Not allowed | Allowed | Yes |

So the honest pitch is "bring your ChatGPT plan for answers", never "no keys at all".

### Anthropic: what is allowed (primary sources)

Read on Anthropic's pages on 27 November 2026:

- [Legal and compliance](https://code.claude.com/docs/en/legal-and-compliance) (undated):
  "Anthropic does not permit third-party developers to offer Claude.ai login into their own
  applications, or to route requests through Free, Pro, or Max plan credentials on behalf of their
  users." Products, including those built on the Agent SDK, "should use API key authentication
  through Claude Console or a supported cloud provider". The same page allows an end user to sign
  in to the **unmodified** Claude Code binary with their own plan, and says plan limits "assume
  ordinary, individual usage of Claude Code and the Agent SDK".
- [Agent SDK overview](https://code.claude.com/docs/en/agent-sdk/overview): "Unless previously
  approved, Anthropic does not allow third party developers to offer claude.ai login or rate
  limits for their products, including agents built on the Claude Agent SDK." No approval
  programme is named; the legal page says to contact sales.
- [Usage Policy](https://www.anthropic.com/legal/aup), effective 12 November 2026: users may not
  "resell, proxy, or otherwise provide access to Claude through unauthorized means, including
  services that route requests through consumer subscriptions".
- [Agent SDK with your Claude plan](https://support.claude.com/en/articles/15036540-use-the-claude-agent-sdk-with-your-claude-plan)
  (support article, updates of 15 June and 7 October 2026): signed in with a plan, the Agent SDK
  and `claude -p` draw from the plan's limits; a change to Agent SDK usage announced earlier in
  2026 was paused on 15 June.
- [Monthly API credits for Max, Ultra and Team](https://support.claude.com/en/articles/17154008-monthly-api-credits-for-max-and-team-plans)
  and [API credits for subscribers](https://platform.claude.com/docs/en/about-claude/api-credits-for-subscribers):
  Max $100 a month, Ultra $200, Team $20 per standard seat and $100 per premium seat, pooled and
  capped at $500. Linked once from claude.ai billing to one Console organisation, after seven days
  on the plan. They cover the Claude API, Managed Agents, the Agent SDK and the Playground, not
  interactive Claude Code or extra usage. No rollover; Free, Pro and Enterprise get none. (The
  platform page also says "these credits apply only to the Claude API", contradicting its own
  table; the support article lists the Agent SDK as covered.)

### Anthropic: what others are doing (secondary, unconfirmed)

- **OpenCode** (30 October 2026, version 2.2.3) added a "Claude Agent" provider that runs the
  user's installed Claude Code through the Agent SDK on the user's plan login. Its author wrote
  "We got permission from Anthropic"; Anthropic has not confirmed this publicly
  ([cellcog](https://cellcog.ai/blog/opencode-claude-subscription/)).
- **Mastra** has an open issue (2 November) to move from reusing Claude Code's OAuth tokens, which
  "appears to conflict" with the terms, to running the unmodified binary
  ([issue](https://github.com/mastra-ai/mastra/issues/29198)).
- **openaidy** merged a "Sign in with Claude" that reuses the reverse-engineered Claude Code OAuth
  client, and says itself that it is "not an Anthropic-documented integration path"
  ([PR](https://github.com/imzodev/openaidy/pull/567)). **rakazo** shipped an in-app Claude sign-in
  on 22 November ([PR](https://github.com/elie222/rakazo/pull/6475)).

These show demand, not permission. Reusing another app's OAuth client was already marked "not
allowed" in the earlier research, and the November Usage Policy makes the risk higher, not lower:
blocks come without notice and it is the user's account that is at risk.

### The options for a Claude subscriber, ranked

| Option | Allowed | Fits HyperMe | Verdict |
| --- | --- | --- | --- |
| API key from the Console organisation linked to a Max, Ultra or Team plan | Yes | One more provider in the spec 96 catalogue; the plan's monthly credits pay for it | **Do it.** Onboarding copy: "Have Claude Max, Ultra or Team? Your plan includes monthly API credits; make a key in the linked organisation" |
| HyperMe as an MCP server, used from the user's Claude app | Yes | Already approved (phase 3 or 4) | **Keep.** The Claude plan does the thinking in Claude's own app, on HyperMe's data |
| Run the user's unmodified Claude Code or Agent SDK, signed in with their plan, as HyperMe's model for cleanup or answers | Grey: the binary is unmodified, but HyperMe would be routing its own product's requests through the plan, which the terms and Usage Policy name | Poor: starts a separate process per request, seconds of overhead, far from the 800 ms budget; adds a heavy dependency | **Do not.** Revisit only if Anthropic confirms an approval path |
| HyperMe's own "Sign in with Claude" (Claude.ai OAuth) | No, unless Anthropic approves | Would be ideal | **Do not**, unless Anthropic publishes a programme or approves HyperMe in writing |
| Reuse Claude Code's OAuth client or tokens | No | — | **Never** |

A Pro user (no credits) still needs a paid API key; their cheapest path for cleanup stays Groq.

### Rate limits and cost

- **Anthropic API** ([rate limits](https://platform.claude.com/docs/en/api/rate-limits)): tiers are
  now named Start, Build and Scale, set from usage history, with monthly spend caps of $500,
  $1,000 and $200,000. Start allows 4,000 requests a minute on Haiku 5.5 and Sonnet 5.5. One
  person dictating will never come near it; the spend cap and the monthly credits are the real
  limits. Whether subscriber credits count toward moving up a tier is not stated.
- **Sign in with ChatGPT** (from the earlier research): usage counts against the plan's own
  limits, and the user can cap each app as a share of the weekly allowance. HyperMe cannot see
  that limit, so it must treat "limit reached" as a normal error with a clear message and, where
  the user set one, a fallback to a key provider (spec 96 R-24).
- **Cost per role:** cleanup through a ChatGPT plan is free to the user in money but spends their
  allowance on every dictation; answers are a far better use of it. Claude credits are money, so
  the existing spec 96 cost hint applies unchanged.

### How it fits the provider settings (spec 96)

Spec 96 already has the right shape: a catalogue in config, one client per **protocol**, one
credential per provider, and a role table with fallbacks. Two small additions cover everything
above:

1. **Anthropic as a catalogue entry.** Either through Anthropic's OpenAI-compatible endpoint (no
   new code, a config entry; Anthropic describes it as meant for testing, so check its current
   status first) or a native `anthropic-messages` protocol client, which also allows prompt
   caching. Roles: cleanup and answers. Recommendation: start with the native client only if the
   compatible endpoint is not suitable; measure cleanup latency before marking it recommended for
   any role.
2. **An `auth` field on a provider**, beside the key: `key` (today), `none` (local server, R-14)
   and `chatgpt` (sign-in). A `chatgpt` provider shows "Continue with ChatGPT" in place of the key
   field, reuses the OAuth loopback with PKCE already built for the Google connector, keeps its
   tokens in the keychain, and talks the Responses API (a second protocol, `openai-responses`).
   The role's Test (R-13) works the same way: one short request, "Works · N ms".

Everything else in spec 96 carries over: the role keeps its previous provider until Test passes,
fallbacks only count when tested, and the Privacy section names every provider set. The model
menu for a `chatgpt` provider lists what the account offers, so no model name enters the code.

### Recommendation for part 1

1. **Phase 2, size S to M:** add Anthropic (key) to the spec 96 catalogue for cleanup and answers,
   and add the onboarding and Settings copy for Max, Ultra and Team credits. Measure Anthropic
   cleanup latency on the owner's samples before calling it recommended for cleanup.
2. **Phase 3, as approved:** "Continue with ChatGPT" for answers, voice edits and commands, built
   as the `auth = chatgpt` provider above. Offer it for cleanup only behind "Show advanced", after
   a latency measurement, with a note that it spends the plan's allowance.
3. **No Claude sign-in.** Keep the MCP server as the Claude subscriber's route. If the owner
   wants, he can ask Anthropic sales whether an open-source desktop app can be "previously
   approved"; until then the answer is no.
4. Update PRD Q-17 when the phase 3 spec is written; it stays open.

---

## Part 2. Launching Claude Code or Codex tasks (future)

The PRD already places this in "Later": "handing a task to Claude Code by voice ... a trigger and
nothing more, through MCP or the agent's own interface" (Q-6, J-7). HyperMe never does the coding
itself. This part checks what the trigger can be.

### Ways to start a task, fire and forget

| Route | How | Runs where | Login | Fit |
| --- | --- | --- | --- | --- |
| Claude Code, local | `claude -p "<prompt>"` in a folder the user picked, detached; `--permission-mode`, `--max-budget-usd`, `--output-format json` ([headless](https://code.claude.com/docs/en/headless)) | User's Mac, which must stay on | User's own login on the unmodified binary (ordinary use) | **Good first adapter.** No service, no token for HyperMe to hold |
| Claude Code, cloud | `claude --cloud "<prompt>"` from a folder with a GitHub remote; returns a session id and URL with `--output-format json` ([on the web](https://code.claude.com/docs/en/claude-code-on-the-web)) | Anthropic's cloud; keeps going with the laptop closed | User's claude.ai login (Pro, Max, Team, some Enterprise) | **Good.** Best for "research X": HyperMe shows the session link |
| Claude Code scheduled task, API trigger | `POST /v1/claude_code/routines/<id>/fire` with a per-task token and `{"text": ...}`; returns the session URL ([scheduled tasks](https://code.claude.com/docs/en/routines)) | Anthropic's cloud | A token the user makes once in the web UI; research preview, beta header | Possible later: a "Research inbox" task the user sets up once. More setup than `--cloud` |
| Codex, local | `codex exec "<prompt>"` with `--json`, `-C <folder>`, `-s` sandbox (Codex CLI reference, now on learn.chatgpt.com) | User's Mac | `codex login` (ChatGPT or key) | **Good**, same shape as `claude -p` |
| Codex, cloud | `codex cloud exec --env <ENV_ID> "<prompt>"`; `codex cloud list --json` for status (experimental) | OpenAI's cloud | Same as the CLI | **Good**, needs an environment id the user picks once |
| Codex SDK, Claude Agent SDK | Libraries in TypeScript or Python | Local | User's login | No: adds a Node or Python runtime to a Rust app for what one command does |
| MCP | Claude Code's `claude mcp` configures servers it uses; no documented "Claude Code as a server" was found. `codex mcp-server` has been removed in favour of App Server | — | — | No standard way to start a task through MCP today |
| Remote Control | Drives an existing local Claude Code session from claude.ai or the phone | User's Mac | claude.ai login | Not a launcher; useful to the user, not to HyperMe |

All of these keep the user's own login inside the vendor's own tool, which is what the terms
allow. HyperMe only builds a command line and shows what came back.

### Embedding their interface

- **Claude Code** has no embeddable interface or deep link that opens a session with a prompt.
  Its interfaces are the terminal, IDE extensions, the desktop app and claude.ai/code.
- **Codex App Server** (`codex app-server`, JSON-RPC over stdio) is documented for embedding Codex
  in a product, but is experimental, may change without notice, and new enterprise integrations
  are asked to join a known-clients list. The `codex app` command opens the desktop app on a
  folder, with no prompt.
- **Agent Client Protocol** ([agentclientprotocol.com](https://agentclientprotocol.com)) is real
  and lists Codex (through an adapter) and Claude (through Zed's Agent SDK adapter). It is how Zed
  embeds agents. Using it would mean HyperMe draws a full chat and diff interface.

Embedding is not recommended. It turns HyperMe into a coding client, which the PRD rules out; it
would cost far more than the 100 MB idle budget (the Home window is capped at 130 MB); and every
route is experimental. The right interface is a card: "Started in Claude Code · Open", linking
to the session URL (cloud) or the terminal (local), and later a line in the Home window's
history.

### What "Hyper, launch a Claude task to research X" would be

1. Command mode (phase 3) recognises the intent and fills one built-in action, `launch_agent`, with
   the agent (Claude Code or Codex), where (cloud or this Mac, and which folder or environment),
   and the prompt.
2. The preview card shows exactly that, as every action does; Enter starts it.
3. HyperMe runs the command detached, reads the session id or URL from its JSON output, and
   shows "Started · Open". Nothing more: no following, no result back (owner, Q-6).
4. Settings holds the defaults: preferred agent, a default folder for local runs, a default Codex
   environment. HyperMe checks that `claude` or `codex` is installed and signed in, and says how
   to fix it if not.

Size M once command mode exists: one action, two or four command adapters behind one trait, no
new crate. Mac first; both CLIs also run on Linux and Windows.

### Recommendation for part 2

Keep it in **"Later"**. Once command mode and the first-wave commands (tasks, reminders, email
drafts) work, it is one more action, not a new system; the owner called it not urgent. Begin with `claude --cloud` and
`claude -p`, then Codex. No embedding; no SDK runtime; no MCP route until one exists.

---

## Not checked

- Latency of Anthropic models for cleanup on the owner's samples, and of a ChatGPT plan account
  through the Responses API.
- Whether Anthropic's OpenAI-compatible endpoint is still offered and suitable for production.
- Whether subscriber API credits count toward Anthropic's tier advancement.
- OpenCode's claimed approval from Anthropic, and whether Anthropic has any approval process for
  open-source desktop apps.
- Plan requirements for Codex cloud tasks, and the exact Codex deep-link syntax.
- The routines API's hourly limits (read through a summary).

## Sources

Primary: [Anthropic legal and compliance](https://code.claude.com/docs/en/legal-and-compliance),
[Agent SDK overview](https://code.claude.com/docs/en/agent-sdk/overview),
[Anthropic Usage Policy](https://www.anthropic.com/legal/aup),
[Agent SDK with your Claude plan](https://support.claude.com/en/articles/15036540-use-the-claude-agent-sdk-with-your-claude-plan),
[Monthly API credits for Max, Ultra and Team](https://support.claude.com/en/articles/17154008-monthly-api-credits-for-max-and-team-plans),
[API credits for subscribers](https://platform.claude.com/docs/en/about-claude/api-credits-for-subscribers),
[Anthropic rate limits](https://platform.claude.com/docs/en/api/rate-limits),
[Claude Code headless](https://code.claude.com/docs/en/headless),
[Claude Code on the web](https://code.claude.com/docs/en/claude-code-on-the-web),
[Claude Code scheduled tasks](https://code.claude.com/docs/en/routines),
[Claude Code Remote Control](https://code.claude.com/docs/en/remote-control),
[Codex CLI commands](https://learn.chatgpt.com/docs/developer-commands?surface=cli),
[Codex SDK](https://learn.chatgpt.com/docs/codex-sdk),
[Codex App Server](https://learn.chatgpt.com/docs/app-server),
[Agent Client Protocol agents](https://agentclientprotocol.com/get-started/agents.md).
OpenAI's Sign in with ChatGPT pages are listed in the
[earlier research](2026-11-26-competitors-and-plan-sign-in.md).

Secondary: [cellcog on OpenCode](https://cellcog.ai/blog/opencode-claude-subscription/),
[Mastra issue 29198](https://github.com/mastra-ai/mastra/issues/29198),
[openaidy PR 567](https://github.com/imzodev/openaidy/pull/567),
[rakazo PR 6475](https://github.com/elie222/rakazo/pull/6475),
[OneHuman on the credits](https://onehuman.io/news/anthropic-promised-api-credits-delivered-max-team-october-2026),
[AI Pricing Guru on the credits](https://www.aipricing.guru/news/claude-max-team-monthly-api-credits-2026/),
[Daniel Vaughan on Codex cloud delegation](https://codex.danielvaughan.com/2026/05/19/codex-cli-cloud-delegation-workflows-plan-locally-execute-remotely-apply-diffs/),
[iamtypist on Claude and audio](https://iamtypist.dev/blog/can-claude-transcribe-audio).
