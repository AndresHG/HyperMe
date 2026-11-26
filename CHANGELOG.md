# Changelog

All notable changes to HyperMe are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/) once there is a release.

## [Unreleased]

### Added

- A Cargo workspace at the root and `crates/hyperme-core`, starting with the config file:
  `config.toml` with the owner's defaults, written on first launch, with errors that name the
  file and line or the key, and a refusal of any key-like field (keys live in the keychain).
- The dictation state machine in `hyperme-core`, with every provider and device behind a trait:
  hold to record, tap and Esc to cancel, maximum length, finalize then paste, short dictations
  without cleanup, raw text after the cleanup deadline or a cleanup error.
- `crates/hyperme-providers` with Speechmatics streaming, ported from flowbench: audio in chunks
  of at most 100 ms, explicit finalize, no expected language, and errors that name the provider
  and never the key.
- OpenAI-compatible cleanup in `hyperme-providers` (Groq first): warm-up on key-down, streamed
  text, prompt v4, the frontmost app's name, and the raw transcript at the deadline or on error.
- `crates/hyperme-platform` with microphone capture (`cpal`): mono, resampled to 24 kHz, 20 ms
  frames to the state machine. Start latency measured in `docs/results/`.
- The dictation hotkey: hold Fn (default), Right Option or a chord, and Esc to cancel, through a
  listen-only macOS event tap.
- `spikes/flowbench`, a command-line speed test for dictation: upload against streaming,
  several speech and cleanup providers, stage timings, a quality score against gold texts.
- Phase 0 results in `docs/results/`: Speechmatics (EU) with Groq cleanup reaches a median of
  429 ms and a 90th percentile of 595 ms from key release to cleaned text on 50 samples.
- Project documents: spec, roadmap, research, pitch, contributing rules, security policy, code
  of conduct, issue and pull-request templates, CI.
- Licence: MIT OR Apache-2.0.
- `spikes/localmeet` and its results: local speech and language models for meeting notes on
  the owner's Mac, with a proposed on-device design for phase 5 in `docs/ROADMAP.md`.
