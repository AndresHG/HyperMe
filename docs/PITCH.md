# HyperMe pitch and positioning

**Status: 25 November 2026.** The owner chose to lead with the full copilot, and to keep the
name HyperMe for the public launch. It builds on
[RESEARCH.md](RESEARCH.md) section 3, which it replaces as the place to edit the pitch.

Every claim here must be earned before it appears in the README or a launch post. The ledger at
the end says which ones are earned today.

## One line

> **HyperMe: the open-source voice copilot for your workday.**

The headline leads with the full copilot (owner, 25 November 2026). Because only dictation will
exist at first, the README says in its first paragraphs that it starts with dictation and that
the rest arrives phase by phase. Repository description:

> The open-source voice copilot for your workday. Starts with dictation as fast as Wispr Flow;
> light, on your own keys.

A dictation-only line stays available for a single-purpose launch post, where research found
such launches score far higher ([RESEARCH.md](RESEARCH.md) section 1.4):

> HyperMe is open-source voice dictation for your Mac: as fast as Wispr Flow, light enough to
> forget it is running, on your own keys.

## Elevator pitch (about 30 seconds)

> You talk three times faster than you type. Wispr Flow proved that a hotkey, your voice and a
> bit of cleanup beat the keyboard for most of what you write, from chat messages to prompts for
> coding agents. But it is closed, it runs on its vendor's servers, and you pay a subscription
> for it.
>
> HyperMe does the same in a small native app you can read: hold a key, speak in English or
> Spanish, and clean text lands at your cursor in under a second. It streams to the speech and
> language models you choose, with your own keys, and keeps nothing on disk. Then the same key
> grows into a copilot: "Hyper, open a ticket for this", short answers, your day and your mail,
> your meetings turned into tasks, your coding agents taking orders.

## The vision (two sentences)

> HyperMe is one small pill for everything you say at work. Hold a key and your words land as
> clean text in any app; your meetings become tasks, your agents take orders, and it knows you
> better every day, on your own keys and your own disk.

"Knows you better every day" stays out of the README until a memory design exists
([RESEARCH.md](RESEARCH.md) section 3).

## Who it is for first

Working assumption, not yet confirmed by the owner: people who dictate all day into coding
agents and chat apps, on a Mac, and want a tool that is open, light and under their own keys.
In the one thread with usage reports, commenters described thousands of dictated prompts, mostly
into coding agents ([RESEARCH.md](RESEARCH.md) section 2).

Not for, at first: people who need offline or local models (later phase), Windows or Linux users
(later phase), teams that need admin controls.

## Positioning against Wispr Flow

Wispr Flow is the bar: the product people compare dictation with. HyperMe does not try to beat it
on breadth. It competes on openness, ownership and weight, and must match it on speed and
quality to be worth switching to.

| | Wispr Flow | HyperMe |
| --- | --- | --- |
| Source | Closed | Open source, MIT OR Apache-2.0 |
| Where your audio goes | Wispr's servers and its own models ([wisprflow.ai/canto-1-results](https://wisprflow.ai/canto-1-results)) | The providers you choose, with your own keys. Nothing kept on disk, no telemetry |
| Price | Subscription (current price not checked for this draft) | Free; you pay your providers per use. Estimated $9 to $100 a month for heavy use, depending on models ([RESEARCH.md](RESEARCH.md) section 1.4) |
| Speed, key release to text | Under 700 ms, a vendor budget and p99 target, not a measurement ([Wispr, Sept 2025](https://wisprflow.ai/post/technical-challenges)) | 429 ms median, 595 ms p90 on 50 of the owner's samples, CLI only, wired ([results](results/2026-11-25-full-benchmark.md)). Not yet timed side by side with Wispr Flow |
| Formatting quality | 79.3% zero-edit rate, Voice Arena, US English only ([voicearena.com](https://voicearena.com/dictation-bench)) | **Behind.** 3 of 50 exact against the owner's gold texts, a stricter measure that does not compare directly ([results](results/2026-11-25-full-benchmark.md)) |
| Footprint | Electron plus a Swift helper (third-party teardown, unverified) | Rust core in Tauri 2; budget under 100 MB idle. No app yet |
| Languages | 21 languages (vendor claim, [RESEARCH.md](RESEARCH.md) section 4) | English and Spanish, detected per dictation |
| Beyond dictation | Notetaker, MCP server, "Ask Wispr" drafts ([RESEARCH.md](RESEARCH.md) section 1.1) | Planned: one key for dictation, questions and previewed actions; meetings to tickets; coding agents by voice ([ROADMAP.md](ROADMAP.md)) |

### Against the open-source field

| Project | What it is | How HyperMe differs |
| --- | --- | --- |
| Handy (MIT) | Light Tauri dictation, local models only | Cloud streaming and cleanup for Wispr-level speed and quality. HyperMe plans to build on Handy's crates |
| OpenWhispr (MIT) | Electron app with dictation, commands, meetings | Native and light; sub-second previewed actions |
| VoiceInk, FluidVoice (GPL-3.0) | Native Swift dictation | Permissive licence; cross-platform core |
| SpeakoFlow, OpenTypeless (MIT) | Small Tauri apps that stream to cloud speech | Measured against Wispr Flow, then a copilot on top |

Star counts and feature claims are from the research of 24 November 2026
([RESEARCH.md](RESEARCH.md) section 1, [FIELD.md](FIELD.md)) and should be checked again before
a launch post.

## Three things to say every time

1. **Fast.** Sub-second from key release to text, measured and published with the method.
2. **Light and open.** A small native app you can read, under your own keys, nothing kept.
3. **More than dictation, later.** The same key will do things, with a preview before anything
   leaves the app.

## Claims ledger

| Claim | Status on 25 November 2026 | What earns it |
| --- | --- | --- |
| "As fast as Wispr Flow" | Partly: our median is under Wispr's own budget, CLI only, one network | Wispr Flow timed on the same 50 clips ([#5](https://github.com/AndresHG/hyperme/issues/5)), then the Phase 1 app measured end to end, paste included |
| "As good as Wispr Flow" | **Not earned.** 6% exact match against gold | Wispr Flow scored on the same gold; the Phase 2 gate |
| "Light" | Not earned: no app yet | Idle memory and CPU of the Phase 1 app against Wispr Flow and OpenWhispr on the same Mac |
| "Open source, your keys, nothing kept" | Earned by design; enforced by the privacy rules in `SECURITY.md` | Keep it true; a break is a vulnerability |
| "One key for everything" | Not earned | Phase 3 |
| "Knows you better every day" | Not earned; no design | A memory design; keep out of public text until then |

## Name

- The program is **HyperMe**; the assistant is addressed as **"Hyper"** (`CLAUDE.md`).
- Public text always says HyperMe in full, because plain "Hyper" is used by many other apps
  (the Hyper terminal, the `hyper` Rust crate).
- Checked on 25 November 2026: the name `hyperme` is unclaimed on crates.io and npm. No
  repository on GitHub uses it as its name (code search by name). No trademark search was done.
- The owner chose to keep HyperMe for the public launch (25 November 2026).
- The old tagline "You, amplified." is dropped; the one-line pitch carries the meaning.
