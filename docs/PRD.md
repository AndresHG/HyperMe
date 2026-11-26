# HyperMe product requirements (PRD)

**Status:** draft, 26 November 2026. Decisions marked *proposed* wait for the owner; the
interview of 26 November 2026 settles them one by one ([section 12](#12-open-questions)).

## How the documents fit together

This file is the source of truth for **what** HyperMe is, **why** and **for whom**. The other
documents trace back to it:

| Document | Says | Wins on |
| --- | --- | --- |
| `docs/PRD.md` (this file) | What, why, for whom: users, jobs, principles, features and priority, non-goals, success, privacy | Product scope and priority |
| [ROADMAP.md](ROADMAP.md) | When: phases, their order and their gates | Order and gates |
| `specs/<issue>-<slug>/` | How, per phase or feature: requirements (`R-n`), acceptance criteria (`AC-n`), design | Behaviour of one feature |
| [UX.md](UX.md) | How it looks and feels | Surfaces, copy, settings layout |
| [SPEC.md](SPEC.md) | Architecture and budgets | Components and technical choices |
| [PITCH.md](PITCH.md) | How it is described in public, and which claims are earned | Public wording |
| `CLAUDE.md` | Decisions already made, and working rules | Rules for sessions |

When two documents disagree, the owner's latest dated decision wins, and the older document is
corrected in the same pull request. A spec that adds a feature not listed in section 7 updates
this file first.

## 1. Product in one paragraph

HyperMe is a small, open-source desktop copilot that you drive with your voice. You hold one
key and speak. If you are writing, clean text lands at your cursor, faster than you could type
it. If you start with "Hyper, …", it answers a short question, prepares an action in your tools
and shows it before anything leaves the app, or turns your meeting into notes and tickets. It runs on your own keys, keeps your data on your Mac, and stays light enough to
forget it is running. Mac first; Linux and Windows later.

## 2. Identity: voice to done

Decided by the owner on 26 November 2026 (Q-1). Dictation apps turn voice into text. HyperMe turns voice into
an outcome: text, an answer, an action or a task. Text is the first outcome and
the one that has to be perfect, because it is used dozens of times a day.

What that means for decisions:

- **Wispr Flow is a bar, not a template.** HyperMe must match it on dictation speed and quality,
  because that is what people compare. It does not copy Wispr Flow's product shape, its Hub
  window or its feature list. Where Wispr stops at text (its Command Mode takes no action in other
  apps, [FIELD.md](FIELD.md)), HyperMe keeps going.
- **Quick things happen at the pill, lasting things in Home** ([UX.md](UX.md), decisions U-8 to U-10).
  The pill and its cards are for talking. A small quick sheet from the dot at the bottom centre
  shows Ask, Now, the top three items waiting for you, and the last dictation with Retry. The
  Home window, with a sidebar (Home, History, Dictionary, Style, Meetings; Settings, Help), is
  the full app for lasting work.
- **"Hyper" is someone you address.** Commands start with the name, the way you would address a
  colleague. The name is also the future wake word (`CLAUDE.md`).

What sets it apart, in order of how hard it is for others to copy
([RESEARCH.md](RESEARCH.md) section 1.2):

1. One key for everything you say: dictation, a question, a previewed action.
2. Speak, preview card, Enter, in under a second.
3. Meetings that end as proposed tickets with an owner and the exact quote.
4. Light and native, with cloud speed, open source and on your own keys.

## 3. Users

### Primary: the owner

The owner dictates daily in Spanish and English, uses Wispr Flow today, and works with coding
agents. Every phase gate is first a gate on the owner's own use ([ROADMAP.md](ROADMAP.md)).

### First outside users

Decided by the owner on 26 November 2026 (Q-4): people who work in technology companies, on a
Mac. Developers first, because they dictate all day into coding agents and chat, already use
GitHub and can get their own API keys. Product owners, product managers and people who work with
data should feel just as at home: their day is tickets, meetings, mail and quick questions, and
the trackers they use (GitHub Projects, then Linear) are the first connectors. In the one thread
with usage reports, commenters described thousands of dictated prompts, mostly into coding
agents ([RESEARCH.md](RESEARCH.md) section 2).

### Not for, at first

- People who need offline or fully local dictation (later).
- Windows and Linux users (later).
- Teams that need admin controls, shared billing or single sign-on.
- Languages other than English and Spanish as a promise: other languages the speech provider
  detects work, labelled "untested", and contributors can add samples to test them (owner, Q-10).

## 4. Problems

| Problem | Today | Cost to the user |
| --- | --- | --- |
| Typing is slow next to speaking | Dictation apps exist; the good ones are closed and run on the vendor's servers, by subscription | Money, privacy, lock-in |
| Small tasks break focus | "Create a ticket for this", "what's next today", "draft a reply": each needs a switch of app and a few minutes | Attention, many times a day |
| Voice assistants are slow to act | The Claude path opens a chat turn that takes seconds; dictation apps stop at text | You type it yourself instead |
| Meetings end as notes, not work | Granola and Wispr Flow stop at notes and create no tickets ([RESEARCH.md](RESEARCH.md) section 1.2) | Action points get lost or retyped |
| Desktop helpers are heavy | Electron apps with hundreds of MB idle | Battery, memory, trust |

## 5. Jobs to be done

| ID | When I… | I want to… | So that… | Phase |
| --- | --- | --- | --- | --- |
| J-1 | am writing in any app | speak and get clean text at my cursor | I write three times faster, in my own words | 1, 2 |
| J-2 | have a quick question mid-task | ask it out loud and see a short answer | I don't leave what I'm doing | 3 |
| J-3 | think "this needs a ticket", "remind me", "tell X" | say it and confirm a preview | the task is captured in my tools in seconds | 3 |
| J-4 | start or plan my day | hear or see what's next and what's pending | nothing slips | 4 |
| J-5 | face my inbox | get it triaged and replies drafted | mail takes minutes, and nothing is sent without me | 4 |
| J-6 | finish a meeting | get notes, action points and proposed tickets | the meeting becomes work, not a document | 5 |
| J-7 | want code written or a bug looked at | say it and have the task handed to Claude Code | I start the work without switching apps; the agent does the coding, not HyperMe | later |
| J-8 | use it every day | have it learn my words, names and style | it gets better the more I use it | later (dictionary from Phase 2) |

## 6. Principles

These settle trade-offs when a spec does not.

1. **Speed is the feature.** Dictation has a hard budget (section 10). Anything that adds
   latency to the dictation path needs a number that shows it fits.
2. **Dictation is never worse because of the rest.** Commands, cards, the Home window and connectors must not
   slow, complicate or break dictation. One key, but the dictation path stays the fast default.
3. **Preview before anything leaves the app.** Anything that creates, sends or posts outside the
   app shows a preview and waits for confirmation. Email is draft-only (`CLAUDE.md`).
4. **Your keys, your Mac.** Bring your own keys, data on disk only on your Mac, no telemetry, no
   account with us (section 9).
5. **Light to run.** Under 100 MB idle, near 0% CPU; features that are not open cost nothing.
6. **Never take focus.** The pill and its cards never steal the cursor from the app you are in; the quick sheet and the Home window take focus only when you open them
   ([UX.md](UX.md)).
7. **Use what exists.** Connectors through MCP, scheduling through a small built-in scheduler,
   coding through the agents' own interfaces. No integration code per service, no agent
   framework runtime (`CLAUDE.md`).
8. **Earn every claim.** Nothing goes in public text before the evidence exists
   ([PITCH.md](PITCH.md), claims ledger).
9. **Open by default.** Permissive licence, readable code, no closed edition.

## 7. Features and priority

Priority: **P0** is needed to pass that phase's gate; **P1** belongs in the phase but can slip
to the next release; **P2** is nice to have. The phase is from [ROADMAP.md](ROADMAP.md).

### 7.1 Dictation (J-1)

| Feature | Phase | Priority | Notes |
| --- | --- | --- | --- |
| Hold-to-talk on one hotkey, default hold Fn | 1 | P0 | Spec R-1, R-8 |
| Streaming speech, warm connections on key-down | 1 | P0 | Speechmatics `melia-1` EU today |
| Cleanup with a small fast model, raw-text fallback after a deadline | 1 | P0 | Groq `gpt-oss-20b`, prompt `v4`; deadline 1.5 s, configurable |
| Spanish and English, detected per dictation | 1 | P0 | Never translated. Other languages work, labelled "untested" (owner, Q-10) |
| Paste at the cursor, clipboard restored | 1 | P0 | Paste matrix in the Phase 1 design |
| Pill with state, start and stop sounds | 1 | P0 | [UX.md](UX.md) |
| Dictation history on the Mac, retry after a lost connection | 1 | P0 | Owner, 26 November 2026 |
| Keys in the keychain, onboarding that tests each step | 1 | P0 | |
| Recommended setup in onboarding: our pick of speech and cleanup models, the keys it needs, and an estimate of the monthly cost from the hours a week you dictate | 1 | P0 | Owner, Q-5. Version 1 asks only for the recommended keys (Speechmatics, Groq) |
| Choose a provider and model per role (speech, cleanup, answers) in Settings → Advanced, from a list, with any OpenAI-compatible hosted model | 2 | P1 | Owner, Q-5. The range of options follows OpenWhispr's, not its interface. OpenAI is the first extra provider |
| Personal dictionary, learned from corrections, in the Home window | 2 | P0 | Replace sound-alikes before cleanup: 7 of 7 names right ([results](results/2026-11-26-dictionary.md)) |
| Cleanup levels (none, light, medium) and tone per app, on the Style page of the Home window | 2 | P1 | |
| Hands-free: double-tap to lock recording | 2 | P1 | Owner, U-3 |
| Second speech provider as fallback | 2 | P1 | |
| Clean in pieces while speaking | 2 | P2 | Only if long dictations miss the budget |
| Snippets (a spoken shortcut expands to saved text) | 2 | P2 | *Proposed* |

### 7.2 Questions and commands (J-2, J-3)

| Feature | Phase | Priority | Notes |
| --- | --- | --- | --- |
| "Hyper, …" routes the same key to a question or an action; optional command key in Settings | 3 | P0 | Owner, 26 November 2026 (Q-3) |
| Answers in a card that grows from the pill, a few lines, with sources when it searched | 3 | P0 | Its own "answers" model, chosen in Settings; the default is a very cheap, very fast hosted model (the class of `gpt-6-luna` or Claude Haiku; the name lives in config). Web search as an MCP tool, used only when the question needs fresh facts (owner, Q-11) |
| Switch the model from the answer card and regenerate the last answer | 3 | P1 | Owner, Q-11 |
| Preview card for actions, Enter to confirm, Esc to drop | 3 | P0 | Under one second from release to card. Default for every tool; a tool can be switched to fire at once in Settings later; email stays draft-only (owner, Q-8) |
| One MCP client; GitHub as the first connector | 3 | P0 | Owner, 25 November 2026 |
| Context of the frontmost app and selected text | 3 | P1 | Within the privacy boundary (section 9) |
| Dot at the bottom centre with a quick sheet (Ask, Now, top 3 waiting, last dictation); cards for answers and previews | 3 | P0 | Owner, U-8 and U-10 |
| Linear connector, after GitHub Projects | 3+ | P1 | Owner, Q-4. Any MCP server the user adds works without code |

### 7.3 Daily tasks and email (J-4, J-5)

| Feature | Phase | Priority | Notes |
| --- | --- | --- | --- |
| Calendar connector, "meeting now" nudges | 4 | P0 | Google first, Microsoft 365 later (owner, Q-7) |
| Scheduled jobs: a prompt plus a tool allowlist each | 4 | P0 | Built in, small; no agent runtime |
| Daily plan: what's next, what's pending for tomorrow | 4 | P0 | |
| Email triage and reply drafts, draft-only | 4 | P0 | Gmail first. Never sends |
| One "Waiting for you" queue on the Home page of the Home window, its top three in the quick sheet | 4 | P1 | [UX.md](UX.md) |

### 7.4 Meetings (J-6)

Daily tasks and email (7.3) and meetings are the two most important capabilities after
commands (owner, Q-6). Meetings follow daily tasks and email, or run in parallel with them.

| Feature | Phase | Priority | Notes |
| --- | --- | --- | --- |
| Live transcript on device (Apple's recognizer), grey to white cleanup | 5 | P0 | Owner, 26 November 2026 |
| Two tracks: me (microphone) and them (system audio) | 5 | P0 | Visible indicator always on |
| Notes, action points and proposed tickets with owner and quote | 5 | P0 | Tickets go through the preview |
| Diarization after the meeting | 5+ | P2 | FluidAudio (Apache-2.0) |

### 7.5 Later

Memory and learning beyond the dictionary ("knows you better every day", plain files the user
owns); a free local mode with speech and cleanup models you download in the app, so no key is
needed (owner, Q-5); handing a task to Claude Code by voice ("Hyper, create this and send it to
Claude Code"), a trigger and nothing more, through MCP or the agent's own interface (owner, Q-6); Linux and Windows; a wake word. Jira is not on the roadmap; it may come later, and an MCP server the user
adds already works (owner, 26 November 2026).

## 8. Non-goals

- A chat app. HyperMe answers briefly and acts; long conversations belong in a chat client.
- Its own agent framework, or embedding one (OpenClaw, Hermes and similar).
- Coding. HyperMe is for daily work: dictation, meetings, moving and acting fast. It never runs
  or follows coding tasks itself; at most it hands one to a coding agent (owner, 26 November 2026).
- Sending email, or posting anywhere, without a preview and a confirmation.
- Integration code per service. If there is no MCP server, it waits or uses a generic one.
- Telemetry, accounts or a cloud of our own in version 1.
- Mixed-language sentences in one dictation.
- Window text or screenshots by default (only behind a visible switch, off by default).
- Mobile apps.
- A closed or paid edition.

## 9. Privacy, keys and trust

| Topic | Rule | Source |
| --- | --- | --- |
| What may leave the Mac | Audio, transcript, dictionary, app name and the text before the cursor, to the providers the user configured, under the user's keys | `CLAUDE.md` |
| Behind a switch, off by default | Window text, screenshots | `CLAUDE.md` |
| Never | Password fields; telemetry; any destination the user did not configure | `CLAUDE.md`, spec R-17, R-29 |
| On disk | Dictation history, audio and text, on by default, 30 days or unlimited, under a size limit (default 1 GB); audio of a failed dictation kept for retry even with history off | Owner, 26 November 2026; spec R-18, R-31 |
| Keys | OS keychain only; never in files, logs or the web view | Spec R-25 |
| Agent safety | Text read from mail, tickets or web pages is data, never instructions; each scheduled job has a tool allowlist | [SPEC.md](SPEC.md) section 4 |
| Meetings | A visible indicator whenever capture runs | [SPEC.md](SPEC.md) section 4 |
| Keys model | Bring your own keys. Onboarding guides you to our recommended models and keys, with a cost estimate; other hosted providers live in Settings → Advanced; local models that you download come in a later version | Owner, 26 November 2026 (Q-5) |

## 10. Success

### For the product (phase gates, from [ROADMAP.md](ROADMAP.md))

| Phase | Gate |
| --- | --- |
| 0 | Median ≤ 800 ms and p90 ≤ 1200 ms from key release to cleaned text; language right on 50 of 50; formatting accepted beside Wispr Flow |
| 1 | A full week without opening Wispr Flow; idle memory and CPU within budget |
| 2 | Fewer edits than Wispr Flow on the owner's samples; first public release |
| 3 | Commands used every day for two weeks |
| 4 | A week of daily planning and mail drafts relied on |
| 5 | Tickets from a real meeting accepted with few edits |

### Budgets

| Measure | Target |
| --- | --- |
| Idle memory | under 100 MB |
| Idle CPU | near 0% |
| Key-down to recording | under 50 ms |
| Key release to cleaned text | median ≤ 800 ms, p90 ≤ 1200 ms (gate); design target under 800 ms at p90 |
| Release to preview card (commands) | under 1 s |

### For the project

Success is an open-source project other people install and use (owner, 24 November 2026). With
no telemetry, adoption can only be seen from the outside. The measure (owner, 26 November 2026,
Q-9): release downloads, GitHub stars, and at least 10 people outside the owner reporting weekly use in issues or
discussions within three months of the first release.

## 11. Platform and technical scope

Fixed decisions, kept here so the PRD stands alone; details in [SPEC.md](SPEC.md) and `CLAUDE.md`:
macOS first, then Linux and Windows behind traits; Rust core in Tauri 2 with a thin web UI; cloud
speech and language models for dictation, with provider and model names in config; on-device
recognition for meetings on macOS; MCP for connectors; SQLite storage; dual MIT OR Apache-2.0
licence; dependencies MIT, Apache-2.0 or BSD only.

## 12. Open questions

Asked in the interview of 26 November 2026. Recommendations first.

| ID | Question | Recommendation | Status |
| --- | --- | --- | --- |
| Q-1 | Product identity | "Voice to done"; Wispr Flow as the bar for dictation only | Decided 26 November 2026: voice to done |
| Q-2 | Phase 0 gate | Declare passed on the Wispr comparison and approve the Phase 1 spec | Decided 26 November 2026: Phase 1 approved to start, with the spec's open questions answered (hotkey Fn, cleanup deadline 1.5 s, speed gate wired only) |
| Q-3 | One key or two | One key; "Hyper, …" routes to commands; optional command key | Decided 26 November 2026: one key, "Hyper" prefix |
| Q-4 | First outside user | Developers who dictate into coding agents and chat | Decided 26 November 2026: people in tech companies, developers first, product owners and data people too |
| Q-5 | Keys model for outside users | Bring your own keys only in version 1 | Decided 26 November 2026: own keys with a recommended setup and cost estimate; other providers in Advanced; a local mode in a later version |
| Q-6 | Email (phase 4) before meetings (phase 5) | Keep email first | Decided 26 November 2026: daily tasks and email first, meetings next or in parallel; coding agents move to later, as a hand-off only |
| Q-7 | Mail and calendar provider | Google first | Decided 26 November 2026: Google first, Microsoft 365 later |
| Q-8 | Ticket creation: wait for Enter or fire at once | Wait for Enter, per-tool relaxing later | Decided 26 November 2026: wait for Enter |
| Q-9 | Adoption measure | Downloads, stars, 10 outside weekly users in three months | Decided 26 November 2026: as recommended |
| Q-10 | Languages at public release | English and Spanish only | Decided 26 November 2026: Spanish and English tested; others work, labelled untested |
| Q-11 | What answers questions | A fast model, web search when needed | Decided 26 November 2026: its own model setting, cheap and fast by default, switchable from the card with regenerate |
| Q-12 | Work and personal profiles ([SPEC.md](SPEC.md) section 4) | Drop from version 1; one profile | Open |
| Q-13 | Adopt the trial and baseline checks as gates ([RESEARCH.md](RESEARCH.md) section 1.3) | Drop as gates; the Wispr comparison covers the baseline | Open |

## 13. Contradictions found in the existing documents

Found while consolidating, 26 November 2026. Each is fixed in the document named once the owner
confirms, unless marked "fixed here".

| # | Where | Contradiction | Resolution |
| --- | --- | --- | --- |
| C-1 | [PITCH.md](PITCH.md) (elevator pitch, positioning table, claims ledger), [RESEARCH.md](RESEARCH.md) section 2 | Say "keeps nothing on disk" and "no audio kept on disk"; the owner chose dictation history on disk on 26 November 2026 | Follow the newer decision; reword PITCH to "keeps your data on your Mac" |
| C-2 | Phase 1 spec R-18 | Ends with "It shall not write it to disk", against the history it describes | Delete that sentence |
| C-3 | [SPEC.md](SPEC.md) section 2 vs [ROADMAP.md](ROADMAP.md) phase 3 and [UX.md](UX.md) | Command mode on a second hotkey vs one key that routes | Q-3 decided: one key; fix SPEC section 2 |
| C-4 | `CLAUDE.md` introduction and examples | "Create a ticket for this in Linear"; GitHub is the first connector | Change the examples to GitHub |
| C-5 | `CLAUDE.md` "No local models in version 1" vs meetings on device | Meetings use Apple's on-device recognizer and an optional local cleanup model | Reword: no local models for dictation in version 1; meetings on device are the decided exception |
| C-6 | [RESEARCH.md](RESEARCH.md) section 5 "consume instead of building: meeting capture, scheduling" vs ROADMAP phases 4 and 5 | Research advised not to build capture or a scheduler; the roadmap builds both | The owner's later decisions win (meetings on device, 26 November). RESEARCH stays as history |
| C-7 | [ROADMAP.md](ROADMAP.md) "Where we stand" | Says formatting quality is "not measured"; the Wispr comparison measured 7.6 against 11.6 edits per 100 characters | Update the table |
| C-8 | ROADMAP phase 2 gate | "Zero-edit rate at or above Wispr Flow's": already met on paper (5 of 50 against 4 of 50), so it no longer separates good from not | Gate on edits per 100 characters and on internal names, and on the owner's week of use |
| C-9 | Phase 1 spec decisions | Cleanup prompt `v1`, deadline 1000 ms; the pipeline now uses dictionary substitution and prompt `v4`, deadline 1500 ms | Being fixed in the spec by the UX thread (owner, 26 November 2026) |
| C-10 | [SPEC.md](SPEC.md) section 4 | "Work and personal profiles keep separate connectors and storage"; no phase or spec has profiles, and work data is out of scope | Q-12 |
| C-11 | Budgets in `CLAUDE.md` vs ROADMAP phase 0 gate | "Under 800 ms" vs "median ≤ 800, p90 ≤ 1200" | Both kept: the gate is the median rule; under 800 ms at p90 is the design target (section 10) |
| C-12 | [PITCH.md](PITCH.md) and README | Positioning is framed entirely against Wispr Flow | Q-1 |
| C-13 | Open-decision lists in `CLAUDE.md`, ROADMAP and RESEARCH | Overlap and disagree on what is still open (licence and first connector are decided) | This section is the single list; the others link here |
| C-14 | [ROADMAP.md](ROADMAP.md) phase 5, `CLAUDE.md` introduction, [SPEC.md](SPEC.md) | Phase 5 includes starting and following Claude Code and Devin tasks, and its gate needs an agent task reported back; the owner moved coding agents to later, as a hand-off only (Q-6) | Phase 5 becomes meetings only; agent hand-off moves to Later |
