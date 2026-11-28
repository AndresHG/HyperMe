# Email drafts through the user's own Claude or ChatGPT

Research of 28 November 2026, at the owner's request. It follows the
[paid accounts research](2026-11-27-paid-accounts-and-agent-sessions.md) of the day before and
does not repeat it.

**The ask.** For the email feature (0.3), and later for other chosen tasks: instead of drafting a
reply with HyperMe's own model on an API key, hand the email to the user's Claude (or ChatGPT),
which already knows their projects, have it review the email in depth and draft the reply, and get
the draft back into HyperMe. In the AI models settings, Anthropic would offer both an API key and
"Connect to Claude", chosen per task, not for everything.

## Verdict in short

- **HyperMe calling Claude on the user's plan in the background is still not allowed.** The terms
  forbid a third-party app from routing its requests through a Free, Pro or Max plan, with or
  without the Agent SDK or Claude Code ([earlier research](2026-11-27-paid-accounts-and-agent-sessions.md),
  part 1). So "Connect to Claude" cannot be a hidden model behind a HyperMe card.
- **A hand-off the user sends is allowed, and it is the better product anyway.** HyperMe opens the
  user's Claude app with the request already written; the user presses Send; Claude reads the
  email and the user's own context and writes the draft; the draft comes back to HyperMe through
  HyperMe's own MCP server and lands in the usual preview card. The thinking happens in Claude, on
  the user's plan, in the user's own app, which is exactly the use Anthropic allows.
- **Recommendation:** add "Review in Claude" to 0.3 beside the draft-only replies, built on two
  pieces: a `claude://` link that pre-fills the request, and HyperMe's MCP server (already planned)
  with two tools, one to read the email and one to hand back a draft. ChatGPT follows the same
  shape once its desktop app can reach a local MCP server; until then, "Continue with ChatGPT"
  gives the model but not the user's ChatGPT context.

## What exists (checked 28 November 2026)

- **Opening Claude with a prompt.** Claude desktop on macOS, Windows and Linux answers
  `claude://` links ([Anthropic support: open Claude desktop with a link](https://support.claude.com/en/articles/14729294-open-claude-desktop-with-a-link),
  updated 30 June 2026). `claude://claude.ai/new?q=…` starts a new chat with the prompt filled in
  "so you can review and send it": it never sends by itself. The prompt is cut at about 14,000
  characters. `claude://claude.ai/project/{id}` opens a project, but no link starts a new chat
  inside a project. `claude://cowork/new?q=…&folder=…&file=…` starts a Cowork session with files
  attached (each folder confirmed by a dialog).
- **Claude reaching HyperMe.** Claude desktop runs local MCP servers the user adds. HyperMe as an
  MCP server is already in the PRD (phase 3 or 4, P1: "the allowed way for a Claude subscriber to
  use their plan with HyperMe's data"). How a local app offers itself to Claude desktop (a desktop
  extension or an entry in Claude's config) was not read this time: check before building.
- **The user's context.** A new chat has Claude's memory where the user turned it on; a project's
  files only come in if the chat starts inside that project, which a link cannot do yet. So "has
  all the context about my projects" holds for memory and for connectors the user added, not for
  project files, unless the user moves the chat into the project.
- **ChatGPT.** Sign in with ChatGPT gives HyperMe the model, but "the app does not see the user's
  chats or memories" (earlier research). A `chatgpt.com/?q=` prefill link is widely used but not
  documented by OpenAI (Trey Hunner and others; unverified), and whether the ChatGPT desktop app can
  call a local MCP server was not checked.

## How "Review in Claude" would work

1. A mail card in HyperMe (triage or a reply draft) shows **Review in Claude** next to the normal
   draft button, when the user has set Claude up for that task.
2. HyperMe opens `claude://claude.ai/new?q=` with a short request: "Review the email HyperMe
   shared as `mail:<id>` and draft a reply in my voice. When it is ready, give it back with
   HyperMe's `propose_draft`." The email body is not put in the link: Claude reads it through MCP,
   so long threads and attachments are not cut at 14,000 characters, and nothing sits in a URL.
3. The user reads the request in Claude and presses Send. Claude calls HyperMe's MCP tools:
   - `read_email(id)`: the message and its thread, only for ids HyperMe handed over in this way,
     so Claude cannot read the whole mailbox through HyperMe.
   - `propose_draft(id, text)`: stores the draft as a proposal in HyperMe. It does not touch Gmail.
4. HyperMe shows the proposal in its preview card ("Draft from Claude"). Enter saves it as a Gmail
   draft, as every reply does; email stays draft-only.

What this needs that is not already planned: the two MCP tools and the link. The mail connector,
the preview card and the draft-only rule are 0.3 work already; the MCP server moves from "0.2 or
0.3" to 0.3 at the latest.

## Settings

Not a "Connect to Claude" sign-in, since there is none to connect. In the AI models settings:

- **Anthropic: API key** stays as it is (cleanup and answers, and drafting when chosen).
- **Claude app** appears as a hand-off target, with one button that adds HyperMe's MCP server to
  Claude desktop and a check that it answers. Per task (first: reply drafts) the user picks
  "HyperMe's model" or "Hand off to Claude". Later tasks can join the list (a meeting summary, a
  research question) without new plumbing.

## What is not recommended

| Option | Why not |
| --- | --- |
| Run Claude Code or the Agent SDK on the user's plan in the background to draft | Routes HyperMe's requests through the plan: the terms and the November Usage Policy name it |
| Put the whole email in the `claude://` link | Cut at 14,000 characters, and mail content in a URL can end up in logs and history |
| Let Claude send or save to Gmail itself through HyperMe | Breaks the preview rule; HyperMe's tool only proposes |
| `claude://code/new` (Claude Code) for email | A coding tool; Cowork or a chat fits mail |

## Not checked

- How a local app registers its MCP server with Claude desktop (desktop extension or config file),
  and how Claude asks the user before calling it.
- Whether a future link can start a chat inside a project.
- The ChatGPT desktop app and local MCP servers, and an official ChatGPT prefill link.

## Sources

[Anthropic support: open Claude desktop with a link](https://support.claude.com/en/articles/14729294-open-claude-desktop-with-a-link),
[Anthropic legal and compliance](https://code.claude.com/docs/en/legal-and-compliance),
[Anthropic Usage Policy](https://www.anthropic.com/legal/aup), and the sources of the
[paid accounts research](2026-11-27-paid-accounts-and-agent-sessions.md). Secondary:
[Trey Hunner on ChatGPT and Claude URLs](https://treyhunner.com/2024/07/chatgpt-and-claude-from-your-browser-url-bar/).
