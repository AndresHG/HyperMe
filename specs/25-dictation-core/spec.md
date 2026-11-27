# Phase 1: dictation core

**Status:** draft, 25 November 2026; all open questions answered by the owner on 26 November 2026.
Needs the owner's approval before any production code, and
starts only after the Phase 0 gate passes ([#24](https://github.com/AndresHG/hyperme/issues/24),
[#15](https://github.com/AndresHG/hyperme/issues/15)).
**Issue:** [#25](https://github.com/AndresHG/hyperme/issues/25). **Size:** L.
**Design:** [design.md](design.md). **Slices:** [tasks.md](tasks.md).

## Problem

Phase 0 showed, from a command line, that streaming speech plus a fast cleanup model can turn a
dictation into clean text in about 430 ms at the median after the key is released
([results](../../docs/results/2026-11-25-full-benchmark.md)). Nobody can use that yet: there is
no hotkey, no microphone outside the CLI, and no text at the cursor. Phase 1 builds the smallest
Mac app that the owner can use instead of Wispr Flow for daily dictation.

## Goal and gate

- **Goal:** the owner uses HyperMe instead of Wispr Flow for daily dictation.
- **Gate:** a full week without opening Wispr Flow, and idle memory and CPU measured against the
  budgets (`CLAUDE.md`). Then Phase 2 may start.

## Requirements

EARS form. "The app" is HyperMe running on macOS.

### Hotkey and recording

- R-1. When the user presses and holds the dictation hotkey, the app shall start capturing the
  microphone.
- R-2. When the hotkey goes down, the app shall open the speech session and warm the cleanup
  connection at the same time as capture starts, without waiting for either.
- R-3. While the hotkey is held, the app shall stream audio to the speech provider in chunks of
  at most 100 ms.
- R-4. While the hotkey is held, the pill shall show that the app is recording, with a live
  input level.
- R-5. If the hotkey is held for less than the minimum hold time (default 250 ms), the app shall
  cancel the dictation, paste nothing, and close the speech session.
- R-6. If the user presses Esc while recording or processing, the app shall cancel the dictation
  and paste nothing.
- R-7. If a dictation reaches the maximum length (default 120 s), the app shall stop recording
  and process what it has, as if the hotkey had been released.
- R-8. The hotkey shall be configurable. The default is hold Fn (Globe) (owner, 26 November
  2026); Right Option and chords are offered.

### Transcription and cleanup

- R-9. When the hotkey is released, the app shall send the provider's explicit end-of-audio or
  finalize message and wait for the final transcript. It shall not rely on server-side turn
  detection.
- R-10. The app shall not tell the speech provider which language to expect. It shall accept
  English and Spanish, one language per dictation, detected by the provider.
- R-11. When the final transcript arrives, the app shall send it to the cleanup model with the
  cleanup prompt, the name of the frontmost app, and reasoning turned off or set to its lowest
  level.
- R-12. If the cleaned text is not complete within the cleanup deadline (configurable, default 1500 ms after
  the final transcript), the app shall use the raw transcript instead.
- R-13. If the transcript has fewer words than the short-dictation threshold (default 4), the app
  shall skip cleanup and use the transcript with only its first letter capitalised and a final
  full stop added when it has none.
- R-14. If the transcript is empty, the app shall paste nothing and show "Nothing heard" in the
  pill.

### Text at the cursor

- R-15. When the text is ready, the app shall insert it at the cursor of the frontmost app by
  placing it on the clipboard and sending the paste shortcut.
- R-16. After pasting, the app shall restore the clipboard contents it replaced, unless the
  clipboard changed in the meantime.
- R-17. While macOS secure input is on (a password field has focus), the app shall not record and
  shall show "Not available in password fields" in the pill.
- R-18. The app shall keep a dictation history on the Mac: the audio and the text of each
  dictation. History is on by default, keeps 30 days (or unlimited, if the user chooses), and
  deletes the oldest items first when the total passes the size limit the user sets (default
  1 GB). The user can turn it off. The app shall offer "Paste last
  dictation" and "Copy last dictation" in the menu-bar menu.

### Failures

- R-19. If the speech session cannot open or drops, the app shall show the error in the pill,
  paste nothing, and keep the audio for a retry (R-31).
- R-20. If the cleanup call fails, the app shall paste the raw transcript and mark the pill with
  "Raw text".
- R-21. If a provider rejects the key, the app shall say which provider and point to Settings,
  without showing the key.

### App, settings and keys

- R-22. The app shall run as a menu-bar item with no Dock icon and no main window.
- R-23. The pill shall be a non-activating floating panel: it shall never take focus from the app
  the user is typing in, and it shall stay visible above full-screen apps.
- R-24. The settings window shall let the user choose the hotkey, the speech provider and model,
  the cleanup provider and model, the cleanup deadline, and enter one key per provider.
- R-25. The app shall store keys in the macOS keychain only. It shall never write a key to a
  file, a log, the clipboard or the web view's storage.
- R-26. Provider and model names shall come from the config file, never from code. The config
  file holds no secrets.
- R-27. On first launch, the app shall walk the user through the microphone and accessibility
  permissions and the keys, and test each before saying it is ready.

### Privacy and measurement

- R-28. The app shall keep audio only on the Mac and only as R-18 and R-31 allow. It shall send
  audio only to the configured speech provider. Deleting a history item, or turning history off,
  deletes its audio.
- R-29. The app shall send no telemetry. Network traffic shall go only to the providers the user
  configured.
- R-30. For each dictation, the app shall record stage timings (key-down, first audio, key
  release, final transcript, first cleaned token, text ready, paste sent) in memory, and write
  them to a local timing log only when the user turns timing on. The log holds numbers, never
  text.

### Recovery and sounds

- R-31. If the speech session cannot open or drops, or the cleanup fails, the app shall keep that
  dictation's audio on disk, even with history off, and offer **Retry** in the pill and the
  menu-bar menu. Retry sends the saved audio again and pastes the result, or copies it when the
  original app is no longer frontmost. With history off, that audio is deleted after a successful
  retry, when the user discards it, or after 24 hours.
- R-32. The app shall play a soft sound when recording starts and when it stops, on by default,
  with a switch in Settings (owner, 26 November 2026).

## Acceptance criteria

Each test cites the `AC-n` it proves. Criteria marked *(fake)* run in CI with local fakes and
test servers; *(live)* ones are measured by hand on the owner's Mac and recorded in
`docs/results/`.

- AC-1 *(fake)*: Pressing the hotkey starts capture and opens a speech session and a cleanup
  warm-up in parallel; the state machine reaches `Recording` before either connection completes.
  (R-1, R-2)
- AC-2 *(live)*: Key-down to the first captured audio sample (the first callback's arrival minus
  its buffer length) is under 50 ms at the median and under 80 ms at the 90th percentile over 50
  presses. The owner chose this measure on 27 November 2026: timing to a full 20 ms frame adds
  20 ms of audio that was already captured to the clock. The microphone stays off when idle
  (option B, play and pause, is kept). Measured by `examples/mic_first_sample.rs`.
- AC-3 *(fake)*: Audio reaches the fake speech server in chunks of 100 ms or less while the key
  is held. (R-3)
- AC-4 *(fake)*: A hold shorter than the minimum hold time, and Esc at any point before paste,
  both end in `Idle` with nothing pasted and the speech session closed. (R-5, R-6)
- AC-5 *(fake)*: At the maximum length, recording stops and the dictation is processed. (R-7)
- AC-6 *(fake)*: On release, the app sends the provider's finalize message and pastes only after
  the final transcript. (R-9)
- AC-7 *(fake)*: No language parameter is sent to the speech provider; an English and a Spanish
  fixture both come back in their own language. (R-10)
- AC-8 *(fake)*: When the fake cleanup server is slower than the deadline, the raw transcript is
  pasted at the deadline, give or take 20 ms. When it fails, the raw transcript is pasted and the
  pill says "Raw text". (R-12, R-20)
- AC-9 *(fake)*: Transcripts under the short-dictation threshold skip cleanup and are formatted
  as R-13 says; an empty transcript pastes nothing. (R-13, R-14)
- AC-10 *(live)*: Key release to text pasted, over 50 dictations of the owner's in the app,
  on a wired network: median at or under 800 ms, 90th percentile at or under 1200 ms. The same
  run on Wi-Fi is measured and recorded for reference, but is not part of the gate.
- AC-11 *(fake)*: The pipeline's own overhead, measured with zero-latency fakes from release to
  paste sent, is under 20 ms at the 90th percentile.
- AC-12 *(live)*: Paste works, and the clipboard is restored, in each app of the paste matrix in
  [design.md](design.md). (R-15, R-16)
- AC-13 *(fake)*: When the clipboard changes between paste and restore, the app does not
  overwrite it. (R-16)
- AC-14 *(live)*: With a password field focused, holding the hotkey records nothing and the pill
  shows the notice. (R-17)
- AC-15 *(fake)*: With history on, a dictation's audio and text are in the history store and
  "Copy last dictation" returns its text; items older than the retention period, and the oldest
  items past the size limit, are deleted. With history off, no audio or text file remains after a
  successful dictation. (R-18, R-28)
- AC-16 *(fake)*: A dropped speech connection, a 401 from each provider and a cleanup error each
  produce the message in R-19 to R-21, and the key never appears in the message or the log.
- AC-17 *(live)*: The pill never takes focus: typing continues in the frontmost app while it is
  visible, including over a full-screen app. (R-23)
- AC-18 *(fake)*: Keys are read from and written to the keychain through the `SecretStore` trait;
  a test with a fake store proves the config file and the timing log never contain a key.
  (R-25, R-26)
- AC-19 *(live)*: Idle memory of the app (all processes, including the web view) under 100 MB,
  and average idle CPU under 0.5% over 10 minutes, after 20 dictations.
- AC-20 *(fake)*: With timing on, each dictation writes one line of stage timings and no text;
  with timing off, nothing is written. (R-30)
- AC-21 *(live)*: A fresh macOS user account goes from first launch to a first pasted dictation
  through the walkthrough, without a terminal. (R-27)
- AC-22 *(fake)*: Tests make no network calls outside localhost and need no key (`CLAUDE.md`).
- AC-23 *(fake)*: When the fake speech server drops mid-dictation, the audio is saved, the pill
  offers Retry, and a retry against a working fake pastes the transcript. With history off, the
  audio is gone after the retry, and after 24 hours on the fake clock when not retried.
  (R-19, R-31)
- AC-24 *(fake)*: The start and stop sounds play when the setting is on and not when it is off.
  (R-32)

## Non-goals

- Personal dictionary, cleanup levels, tone per app beyond passing the app name, and cleaning
  while the user speaks: Phase 2.
- A second speech provider as runtime fallback: Phase 2. The trait must allow it.
- Signed and notarised builds, auto-update, and any public release: Phase 2.
- Commands, questions, actions, MCP: Phase 3.
- Hands-free (toggle) mode, a wake word, and typing the text out character by character.
- A history view and search: they come with the panel's Dictation tab (`docs/UX.md`).
- Local speech models, Linux and Windows. Platform code stays behind traits so they can follow.
- Languages other than English and Spanish, and mixed-language sentences.

## Pre-mortem

"It is three months later and Phase 1 failed. Why?"

| Cause | Answer in this spec |
| --- | --- |
| Paste broke in the apps the owner lives in (Handy has about 200 issues about paste alone, [RESEARCH.md](../../docs/RESEARCH.md) section 1.4) | Paste matrix in design.md and AC-12; clipboard restore with a change check (AC-13); "Paste last dictation" as a way out (R-18) |
| The owner kept opening Wispr Flow because the text was worse | Phase 0 gate includes quality accepted beside Wispr Flow; raw-text marking (R-20) makes failures visible; Phase 2 owns quality |
| The web view blew the memory budget | Slice 1 measures an empty Tauri app's idle memory before anything else is built; if it is over 60 MB, stop and decide (design.md) |
| The pill stole focus and text went to the wrong place | Non-activating panel (R-23, AC-17), borrowed from Handy and `tauri-nspanel` |
| Opening a speech session on every key-down cost money or hit rate limits on short taps | Minimum hold time (R-5); close the session at once on cancel; measure cost per day in slice 11 |
| A key leaked into a log or a crash report | Keychain only (R-25), no telemetry (R-29), a test that greps outputs (AC-18) |
| Provider APIs changed under us (they changed more than once in 2026) | Model names in config (R-26); providers behind traits; check provider docs at the start of each slice (`CLAUDE.md`) |
| macOS permissions confused the user and dictation silently did nothing | Walkthrough that tests each permission (R-27, AC-21); clear errors (R-19 to R-21) |
| The hotkey clashed with something the owner uses | Configurable hotkey (R-8); default chosen by the owner |

## Decisions

| Decision | Chosen | Rejected, and why | Who |
| --- | --- | --- | --- |
| Shell | Tauri 2, Rust core, thin web UI | Swift-only (no Linux or Windows later); Electron (weight) | Owner, `CLAUDE.md` |
| Speech | Speechmatics `melia-1`, EU endpoint, streaming | OpenAI (914 ms median), Deepgram (language wrong on 2 of 38) ([results](../../docs/results/2026-11-25-full-benchmark.md)) | Phase 0 data; owner to confirm |
| Cleanup | Dictionary substitution first (spec 93), then Groq `openai/gpt-oss-20b`, effort low, level `light` (prompt `v4`; `medium` uses `m2`), deadline 1500 ms, after which the raw transcript is pasted | OpenAI `gpt-6-luna` (slower first token), prompt `v1` (replaced by `v4`), prompt `v2` (slower, cuts words), deadline 1000 ms | Phase 0 data; deadline owner, 26 November 2026 |
| Insertion | Clipboard plus paste shortcut | Typing characters (slow, breaks with input methods) | What Handy ships ([FIELD.md](../../docs/FIELD.md)) |
| Base code | Port Speechmatics, cleanup and timing code from `spikes/flowbench` into the new crates; borrow Handy's recorder, hotkey, paste and pill code under MIT, recorded in `NOTICE.md` | Depending on flowbench (a spike, still being tuned) | Proposed |
| Storage | Config, the optional timing log, and the dictation history: audio files written as they are captured, plus a SQLite index, under the app's data folder | No history on disk (lost the dictation when a connection dropped) | Owner, 26 November 2026 |
| Default hotkey | Hold Fn (Globe) | Right Option (easier to capture, but not what Wispr Flow users know) | Owner, 26 November 2026 |

## Open questions for the owner

1. ~~Default hotkey~~: decided 26 November 2026, hold Fn (Globe), with Right Option and chords
   offered.
2. ~~Cleanup deadline~~: decided 26 November 2026, configurable, default 1500 ms; after it the
   raw transcript is pasted.
3. ~~Wi-Fi in the gate~~: decided 26 November 2026, wired only; Wi-Fi is measured for reference.
