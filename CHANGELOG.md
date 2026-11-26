# Changelog

All notable changes to HyperMe are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/) once there is a release.

## [Unreleased]

### Fixed

- The pill at rest showed a dark square with a row of white dots instead of the bead: the bead's
  view was sent at launch before the page had loaded and was lost, so the page kept its warm-up
  recording bars. The page now draws nothing until a view arrives and asks for the current view
  (`pill_view`) once its listeners exist.

### Added

- Switching providers without a restart (issue 124, slice 2 of `specs/96-provider-settings`):
  `hyperme-providers::any` has one client enum per role (`AnySpeech`, `AnyCleanup`) and one
  `resolve` path that builds the providers a config names, used at launch, after a change in
  Settings and by Test. The engine reads its providers from a `watch` channel once on key-down,
  so a change reaches the next dictation and never one in progress. A provider that cannot be
  built yet (no key) leaves the one in use running. The timing log's provider names follow a
  switch.
- A provider catalogue (issue 123, slice 1 of `specs/96-provider-settings`): what each provider
  is (protocol, endpoint, key page, roles, models with dated prices and measured latency) lives
  in `default-providers.toml`, built into the app, and `[speech]` and `[cleanup]` name only a
  provider and a model. `config.toml` may add a provider or replace a built-in one with a
  `[providers.<name>]` section; an unknown protocol is an error naming the file, line and the
  protocols that exist. Files written before keep working unchanged, their endpoints and prices
  overriding the catalogue's. The cost estimate reads catalogue models.
- `crates/hyperme-quality`, the quality harness (issue 99): replays a set of saved dictations
  through the app's own state machine and providers and scores the pasted text against the
  owner's corrected texts (zero-edit, edits per 100 characters, internal names), with flowbench's
  definitions. `import` copies done dictations from the history into a set. Audio and texts stay
  in the set folder; the results file holds numbers only.
- Releases made on the owner's Mac (`scripts/release-macos.sh`, `docs/RELEASING.md`): Developer ID
  signing with the hardened runtime, notarisation, a DMG, and a signed update with its
  `latest.json`; `--unsigned` for a trial run without an Apple account. **Check for updates…** in
  the menu-bar menu checks and installs signed updates with Tauri's updater, only when clicked.
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
- Paste at the cursor through the clipboard, restored afterwards unless something else changed
  it; no recording while a password field has focus; the frontmost app's name for the cleanup.
- Provider keys in the macOS keychain, behind the `SecretStore` trait.
- The dictation history: each dictation's audio written to disk while it is captured, a SQLite
  index, retention and a size limit; Retry of a failed dictation from its saved audio; start and
  stop sounds behind a trait.
- `app/`: the Tauri 2 menu-bar app with no Dock icon, and the pill as a non-activating panel at the
  bottom centre. Idle memory of the empty shell measured in `docs/results/`.
- The app runs the dictation: config and keychain keys, the real providers and devices, the
  pill's states with a live level meter and Retry, and the menu with Paste and Copy last
  dictation; soft system sounds for start and stop.
- The Settings window: setup check, general, shortcuts, providers with keys saved straight to the
  keychain and a Test button, privacy and history, advanced values and the config file, with
  search. Changes save at once to `config.toml`, keeping its comments.
- The first-launch walkthrough: welcome, microphone with a live level, Accessibility and Input
  Monitoring that turn green by themselves, both keys tested, and a first dictation with its
  timing; it opens at launch while anything it sets up is missing. Its keys step guides to the
  recommended pair with "Get a key" buttons, a monthly cost estimate worked out from prices in
  `config.toml`, and a quiet way to other providers in Settings.
- A local HyperMe.app: `scripts/bundle-macos.sh` builds and installs it, signed with a local
  identity from `scripts/create-signing-identity.sh` so permissions survive rebuilds. The
  permission buttons now ask macOS to add the row, and a run from a terminal names that
  terminal as the app to switch on.
- Permissions in the walkthrough: macOS's dialog is shown once per button, Input Monitoring is
  detected by creating the hotkey tap, and "Relaunch HyperMe" restarts the app on the same
  screen when a grant needs it. Opening HyperMe while it runs shows the walkthrough or Settings.
- A redesigned walkthrough: system typography, one main button per screen, key fields with their
  status inline, a segmented cost choice with one clear number, light and dark.
- The timing log: one line of stage timings per dictation in `~/Library/Logs/HyperMe/timing.tsv`
  when timing is on, numbers only, in flowbench's columns; `examples/timing_summary.rs` gives
  AC-10's median and 90th percentile, and `scripts/measure-idle.sh` measures AC-19.
- `spikes/flowbench`, a command-line speed test for dictation: upload against streaming,
  several speech and cleanup providers, stage timings, a quality score against gold texts.
- Phase 0 results in `docs/results/`: Speechmatics (EU) with Groq cleanup reaches a median of
  429 ms and a 90th percentile of 595 ms from key release to cleaned text on 50 samples.
- Project documents: spec, roadmap, research, pitch, contributing rules, security policy, code
  of conduct, issue and pull-request templates, CI.
- Licence: MIT OR Apache-2.0.
- `spikes/localmeet` and its results: local speech and language models for meeting notes on
  the owner's Mac, with a proposed on-device design for phase 5 in `docs/ROADMAP.md`.
- The pill's new look (U-14 to U-16): while you talk, white level bars on the dark capsule ripple
  out from the centre with your voice; after you let go they settle into a line with a violet
  light running along it until the text lands. Done, raw text and nothing heard show the line in
  their colour. Reduce motion drops the ripple and the running light, and nothing is drawn while
  the pill is hidden.
- Fixed: in the walkthrough, a key typed and saved with Test now shows "Testing…" and then its
  result. Saving a key never told the walkthrough, so the field cleared and nothing happened. A
  key not typed yet no longer shows as "Not working".
- The pill rests as a small still glass bead at the bottom centre (U-13, brought forward by the
  owner), and grows into the pill when you dictate; "Show while idle" in Settings > General
  turns it off. The pill's window is transparent, so no white rectangle shows around it, and its
  label and voice sit in the exact centre.
