# Phase 1 design: dictation core

Companion to [spec.md](spec.md). Draft, 25 November 2026. External APIs named here (Tauri 2,
`tauri-nspanel`, Handy's crates, Speechmatics, Groq, macOS keychain and pasteboard) must be
checked against their current docs at the start of the slice that uses them (`CLAUDE.md`).

## Layout

A Cargo workspace at the repository root. `spikes/flowbench` stays outside it and is not changed.

```
Cargo.toml                 workspace: members below, shared lints and versions
crates/
  hyperme-core/            no platform calls, no network: the dictation state machine,
                           the traits, config, timing. Pure and fast to test.
  hyperme-providers/       network code: Speechmatics streaming, OpenAI-compatible cleanup
                           (Groq), plus local test servers. Ported from flowbench.
  hyperme-platform/        the platform traits' implementations, one module per OS behind
                           cfg(target_os): hotkey, microphone, paste, frontmost app,
                           secure-input check, keychain.
app/
  src-tauri/               Tauri 2 shell: menu-bar item, pill panel, settings window, wiring.
  ui/                      static HTML, CSS and a little JS for the pill and settings. No
                           framework, no build step, unless one is agreed later.
```

CI gets each crate in its matrix (`.github/workflows/ci.yml`).

## Traits in `hyperme-core`

Names are a starting point; the slice that adds each trait settles them with its tests.

| Trait | Does | Real implementation | Fake for tests |
| --- | --- | --- | --- |
| `SpeechProvider`, `LiveSession` | Open a streaming session, send PCM, finalize, return the transcript and language | Speechmatics (providers) | Scripted transcript, configurable delays and failures |
| `CleanupProvider` | Warm the connection; stream the cleaned text | OpenAI-compatible chat completions, Groq first (providers) | Scripted pieces, delays, errors |
| `Microphone` | Start and stop capture; deliver PCM frames and a level | `cpal` (platform) | Frames from a WAV fixture, synthetic only |
| `Hotkey` | Report down, up and Esc for the configured key | macOS event tap or Handy's key crate (platform) | Scripted events |
| `Inserter` | Paste text at the cursor; restore the clipboard if unchanged | Pasteboard plus Cmd+V (platform) | Records what would be pasted |
| `FocusInfo` | Frontmost app name; whether secure input is on | NSWorkspace, `IsSecureEventInputEnabled` (platform) | Fixed values |
| `SecretStore` | Get and set a key by provider | macOS keychain (platform) | In-memory map |
| `Clock` | Monotonic time, so timing and deadlines are testable | `std::time::Instant` | Manual clock |

`flowbench`'s `SpeechProvider`, `LiveSession` and `CleanupProvider` are the starting shape; they
already carry the warm-up, finalize and deadline behaviour the spec needs.

## The state machine

```
Idle --key down--> Recording --key up (held >= min)--> Transcribing --final--> Cleaning
  ^                  |  |                                 |                       |
  |      key up < min|  |Esc / max length (max = release) |error                  |text or deadline
  |                  v  v                                 v                       v
  +------------- Cancelled                              Failed                 Pasting --> Idle
```

- Key-down starts the microphone, the speech session and the cleanup warm-up in parallel. Audio
  captured before the session is open is buffered in memory and sent when it opens.
- `Cleaning` is skipped for short transcripts (R-13).
- `Failed` and `Cancelled` return to `Idle` after the pill shows the message.
- Only one dictation at a time. A key-down while `Transcribing`, `Cleaning` or `Pasting` is
  ignored and the pill says "Still working".

The state machine runs on one async task and owns all state. Platform callbacks send it events
over a channel; it never blocks a platform thread.

## Audio

- Capture at the device rate and resample to what the provider wants (flowbench sends 24 kHz
  mono PCM16; check Speechmatics' accepted rates in the slice).
- Frames of 20 ms, sent in batches of up to 100 ms (R-3).
- Audio is written to the history folder as it is captured, compressed (format settled in the
  slice), so a crash or a dropped connection never loses it (R-18, R-31). With history off it is
  deleted once the text is pasted.

## Pill and settings

- The pill is a `tauri-nspanel` panel: non-activating, joins all spaces, above full-screen apps.
  States: hidden, recording (level meter), working, done (brief), raw text, error, notice.
- Settings is an ordinary window opened from the menu-bar menu.
- The web view gets events from Rust and sends commands back. It never sees a key; key entry
  goes straight to Rust, which writes the keychain.

## History (R-18, R-31)

`~/Library/Application Support/HyperMe/history/`: one audio file per dictation and a SQLite
index (time, app, language, text, state: done, failed or retried). A cleaner runs at launch and
after each dictation: it deletes items past the retention period, then the oldest items until the
total is under the size limit. Failed items are deleted last. The hold-Fn default needs an event
tap that watches the Fn flag; OpenWhispr ships a small bundled listener for the same reason
([docs](https://docs.openwhispr.com/help/dictation/hotkeys.md)).

## Config

`~/Library/Application Support/HyperMe/config.toml`: hotkey, providers and models, endpoints,
deadlines, thresholds, timing on or off. No secrets (R-26). Unknown keys are an error with the
file and line.

## Paste matrix (AC-12)

Each checked by hand on the owner's Mac before the gate: TextEdit, Notes, Safari text field,
Chrome text field, Slack desktop, VS Code editor, a terminal (Terminal and Ghostty), Claude
desktop, Claude Code in a terminal, Mail compose, Google Docs in a browser, a Spanish keyboard
layout.

## Memory budget check (slice 1)

Build an empty Tauri 2 menu-bar app with the pill panel and measure idle memory of all its
processes. If it is over 60 MB before any feature, stop and bring the number to the owner:
options are a lighter pill (native panel without a web view) or accepting a higher budget.

## Timing log (R-30)

One line per dictation, in the app's log directory, when timing is on: the stage timestamps as
milliseconds from key-down, provider and model names, language, word count, fallback flag. No
text. The format matches flowbench's columns (`stt_ms`, `first_ms`, `total_ms`) so results
compare with Phase 0.
