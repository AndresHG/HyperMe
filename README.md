# HyperMe

**The open-source voice copilot for your workday.**

One small pill for everything you say at work. Hold a key and your words land as clean text in
any app; say "Hyper, open a ticket for this" and it previews the action; your day and your mail,
your meetings turned into tasks. All on your own keys, light
enough to forget it is running. Rust core, Tauri 2 shell, Mac first.

It starts with dictation that aims to be as fast as Wispr Flow: hold a key, speak in English or
Spanish, and clean text lands at your cursor. HyperMe streams to the speech and language models
you choose, keeps your audio only on your Mac, for as long as you choose, and sends no telemetry. The rest arrives phase by phase.

> **Status: pre-alpha. There is nothing to install yet.** Phase 0, a command-line speed test, is
> under way. Speed passes; formatting quality does not yet. See [where we stand](#where-we-stand).

## Why

- **Fast.** Speech streams while you talk, the connection is warm before you start, and a small
  model cleans the text in a few hundred milliseconds. The budget is under 800 ms from key
  release to text.
- **Light and open.** A small native app you can read, under MIT OR Apache-2.0. Target under
  100 MB of memory and near 0% CPU when idle.
- **Yours.** Your keys, your providers. Audio, the transcript, your dictionary, the app name and
  the text before the cursor may go to the providers you configured, and nothing else does.
  Never password fields. Details in [SECURITY.md](SECURITY.md).
- **Previews before actions.** Anything that creates, sends or posts outside the app shows a
  preview and waits for you. Email is draft-only.

## How it compares with Wispr Flow

Wispr Flow is the bar. HyperMe competes on openness, ownership and weight, and has to match it
on speed and quality to be worth switching to.

| | Wispr Flow | HyperMe |
| --- | --- | --- |
| Source | Closed | Open source |
| Your audio goes to | Wispr's servers | Providers you choose, with your keys |
| Price | Subscription | Free; you pay providers per use |
| Key release to text | Under 700 ms, vendor budget ([source](https://wisprflow.ai/post/technical-challenges)) | 429 ms median, 595 ms p90, CLI on 50 samples ([results](docs/results/2026-11-25-full-benchmark.md)) |
| Formatting quality | 79.3% zero-edit, US English ([Voice Arena](https://voicearena.com/dictation-bench)) | Behind: 3 of 50 exact on a stricter measure ([results](docs/results/2026-11-25-full-benchmark.md)) |
| Languages | 21 | English and Spanish, detected per dictation |
| Beyond dictation | Notetaker, drafts, MCP server | Planned: commands with previews, a daily task list, meetings to tickets |

The speed numbers are not yet like for like: Wispr Flow has not been timed on the same clips.
More in [docs/PITCH.md](docs/PITCH.md).

## Where we stand

| Phase | Goal | State |
| --- | --- | --- |
| 0. Speed test | Prove Wispr Flow speed on real dictations | Speed and language pass (Speechmatics EU + Groq). Quality gap open |
| 1. Dictation core | Use it instead of Wispr Flow every day | Draft spec: [specs/25-dictation-core](specs/25-dictation-core/spec.md) |
| 2. Quality and first release | Match Wispr Flow on formatting; signed build | Planned |
| 3. Smart routing and actions | One key for dictation, questions and actions | Planned |
| 4. Daily tasks and email | Calendar, nudges, draft-only email | Planned |
| 5. Meetings | Meetings to notes and tickets | Planned |

Each phase starts only after the gate of the one before it passes. Full plan:
[docs/ROADMAP.md](docs/ROADMAP.md).

## Try the speed test

`spikes/flowbench` measures key release to cleaned text on your own recordings, with fakes for
tests and real providers for numbers. See [its README](spikes/flowbench/README.md).

## Try the app on your Mac (development build)

```sh
scripts/create-signing-identity.sh   # once: a local signing identity, so permissions survive rebuilds (asks for your login password)
scripts/bundle-macos.sh              # builds HyperMe.app into ~/Applications
open ~/Applications/HyperMe.app
```

The walkthrough asks for the microphone, Accessibility and Input Monitoring, and your two keys.
This is a local, unnotarised build for trying HyperMe; a signed release comes later. Running the
binary with `cargo run` works too, but then macOS gives the permissions to your terminal app.

## Contributing

Contributions are welcome. Changes are sized S, M or L, and every change starts from a failing
test; L-sized ones start from a spec. Read [CONTRIBUTING.md](CONTRIBUTING.md), and look for
issues labelled `good first issue`. Please follow the [code of conduct](CODE_OF_CONDUCT.md), and
report security problems privately ([SECURITY.md](SECURITY.md)).

## Documents

- [Pitch and positioning](docs/PITCH.md)
- [Roadmap](docs/ROADMAP.md) · [Spec](docs/SPEC.md) · [Phase 0: speed test](docs/SPEED_TEST.md)
- [Research](docs/RESEARCH.md) · [The field](docs/FIELD.md)
- [Going public: checklist](docs/LAUNCH.md) · [Changelog](CHANGELOG.md) · [Notice](NOTICE.md)

## Licence

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
Unless you say otherwise, any contribution you submit is dual-licensed the same way.
