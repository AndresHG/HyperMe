# Choose the speech and cleanup provider

**Status:** draft, 26 November 2026. Needs the owner's approval before any production code.
**Issue:** [#96](https://github.com/AndresHG/hyperme/issues/96). **Size:** L.
**Design:** [design.md](design.md). **Slices:** [tasks.md](tasks.md).
**Traces to:** [PRD](../../docs/PRD.md) 7.1 ("Choose a provider and model per role", P1, phase 2;
"Second speech provider as fallback", P1, phase 2), Q-5; [UX.md](../../docs/UX.md), "Models and
keys in onboarding" and "The settings plan"; decisions U-11 and U-12.

## Problem

HyperMe runs on one speech provider (Speechmatics) and one cleanup host (Groq). Both names
already live in `config.toml` (R-26 of the Phase 1 spec), but the code can only build a
Speechmatics client, and the Settings page shows a free-text model field with no list, no hint
of speed or cost, and no way to pick another provider. Three things follow:

1. The owner cannot try another cleanup model (for example a larger Groq model, or OpenAI) to
   close the formatting gap with Wispr Flow, the real open risk ([ROADMAP.md](../../docs/ROADMAP.md))
   without editing the file by hand.
2. Outside users, developers first, often already hold an OpenAI, Anthropic or OpenRouter key
   and expect to use it, as they can in OpenWhispr.
3. When Speechmatics is down or the key runs out, dictation stops. The audio is safe in the
   history (R-18, R-31), but nothing tries another provider.

## Goal

Wispr Flow's feel with OpenWhispr's range of choice (PRD section 2): the recommended pair works
with two keys and no decisions; behind "Show advanced", one table of roles lets the user pick
any supported provider and model for each role, see what it costs and how fast it is, test it,
and name a fallback. One page, one table, never a tab per feature (UX.md, "How the top tools
organise it").

## What exists today (checked in the code on 26 November 2026)

| Part | Today | Where |
| --- | --- | --- |
| Speech trait | `SpeechProvider` / `LiveSession`, generic in the engine | `crates/hyperme-core/src/traits.rs` |
| Cleanup trait | `CleanupProvider` with `warm` and streaming `clean` | same |
| Speech client | Speechmatics real-time only; `SPEECH_PROVIDERS = ["speechmatics"]` | `crates/hyperme-providers/src/speechmatics.rs`, `config.rs` |
| Cleanup client | Any OpenAI-compatible `/chat/completions` (`ChatCleanup`) | `crates/hyperme-providers/src/cleanup.rs` |
| Keys | One keychain entry per provider name, shared by every role | `crates/hyperme-core/src/secrets.rs` |
| Test | Speech: opens a session, sends no audio. Cleanup: `GET /models`. Returns ms | `app/src-tauri/src/settings.rs` (`key_test`) |
| Fallback | Cleanup: raw transcript after the deadline or an error (R-12, R-20). Speech: none | `crates/hyperme-core/src/dictation.rs` |
| Price data | In config next to the model, for the onboarding estimate | `default-config.toml` |

## Requirements

EARS form. "The app" is HyperMe on macOS. "Role" is one job a model does: dictation speech and
dictation cleanup in this spec; answers (phase 3) and meeting cleanup (phase 5) reuse the same
mechanism later and are named only so the design leaves room for them.

### Catalogue and config

- R-1. The app shall describe every provider it offers in a catalogue in the config: its name,
  its protocol, endpoint, "Get a key" page, which roles it can fill, and per model the prices and
  any measured latency. Provider and model names shall stay out of code (Phase 1 R-26).
- R-2. The code shall know protocols, not providers: one client per protocol. Every cleanup
  provider in this spec shall use the OpenAI-compatible chat protocol, so adding one is a
  catalogue entry, not code.
- R-3. Each role in the config shall name one provider and one model from the catalogue, and may
  name a fallback provider and model.
- R-4. When the app reads a config written before this change, it shall keep working without
  edits, and shall add the catalogue from the defaults.
- R-5. A user may add a provider to the catalogue by hand (for example a self-hosted endpoint)
  if it uses a protocol the app has. An unknown protocol shall be an error naming the file, line
  and the protocols that exist.

### Choosing

- R-6. Settings → Providers shall show the recommended pair by default (UX.md). "Show advanced"
  shall open one table, one row per role, each with a provider menu, a model menu, a key state,
  a Test button and a hint line.
- R-7. The provider menu of a role shall list only providers whose protocol can fill that role,
  the recommended one first and marked "Recommended".
- R-8. When the user changes a role's provider or model, the app shall save it to `config.toml`
  at once and use it from the next dictation, without a restart.
- R-9. A dictation already in progress when the setting changes shall finish on the provider it
  started with.
- R-10. "Reset to recommended" shall put every role back to the recommended pair, keeping the
  keys.

### Keys and Test

- R-11. Each provider shall have one key, shared by every role that uses it (UX.md). The key
  field shall reuse the onboarding control: Get a key, paste, Test; the key goes straight to the
  keychain and is never shown again (Phase 1 R-25).
- R-12. When the user picks a provider that has no key yet, the row shall show the key field
  inline, and the role shall keep using its previous provider until the new one passes Test.
- R-13. Test shall check the key and the chosen model together: for speech, open a session with
  that model; for cleanup, ask the model for a one-token completion. It shall report "Works · N ms",
  or name what failed (key rejected, model not found, no network) without showing the key.
- R-14. A provider whose catalogue entry says it needs no key (a local server) shall show no key
  field, only Test.

### Models

- R-15. When a provider's key works and its protocol can list models, the model menu shall show
  the models that key offers, filtered to those the protocol can use for the role, with the
  catalogue's models first. The list shall be fetched when the menu opens, never on the
  dictation path, and cached for the session.
- R-16. The model menu shall always allow typing a model name the list does not have.
- R-17. When the listing fails, the menu shall fall back to the catalogue's models and say so in
  one line.

### Speed and cost hints

- R-18. Each row shall show one hint line: the expected monthly cost for the user's talking time
  (the onboarding choice of U-11), and the latency the project measured for that provider and
  model, with a link to the result. A model with no measurement shall say "Speed not measured".
- R-19. Hints shall come only from the catalogue (prices, with the date they were checked) and
  from `docs/results/` (latency). The app shall not call a pricing API.
- R-20. When the timing log is on (Phase 1 R-30), the row shall also show the user's own median
  from key release to text over their last 50 dictations on that provider.

### Local cleanup

- R-21. The catalogue shall include a "Local server" cleanup provider: any OpenAI-compatible
  server on this Mac (Ollama, LM Studio, `mlx_lm.server`), with no key.
- R-22. When the user picks Local server, the app shall look for a server on the usual local
  ports and offer what it finds; the user may type another address.
- R-23. A local cleanup model shall be held to the same deadline and raw-text fallback as any
  other (Phase 1 R-12, R-20). The hint line shall say that it runs on this Mac and that nothing
  leaves it for cleanup.

### Fallbacks

- R-24. If the cleanup call fails before the deadline and a fallback cleanup is set, the app
  shall send the transcript to the fallback within the time left; if that fails too, or time runs
  out, it shall paste the raw transcript as today.
- R-25. If the speech session cannot open (network error, server error, or key rejected) and a
  fallback speech provider is set, the app shall open the fallback at once and stream the audio
  captured so far into it, so the dictation is not lost and the user does not repeat it.
- R-26. If the speech session drops mid-dictation and a fallback is set, the app shall send the
  dictation's saved audio to the fallback when the key is released.
- R-27. When a fallback was used, the pill shall say so in its done state ("Pasted · via OpenAI"),
  and the timing line shall record the provider used.
- R-28. After three fallbacks in a row for the same role, the app shall show one notice that the
  main provider keeps failing, linking to its row; it shall not switch the setting by itself.

### Privacy

- R-29. Audio and text shall go only to the providers set for each role, including fallbacks
  (PRD section 9). The Privacy section shall list every provider currently set, fallbacks
  included.
- R-30. Choosing a provider shall never send anything to it before the user tests it or
  dictates.

## Acceptance criteria

- AC-1. A config file from before this change loads unchanged, dictation works, and the
  catalogue appears with the defaults. (R-4)
- AC-2. Adding a cleanup provider to the catalogue by hand (a test server speaking the
  OpenAI-compatible protocol) makes it appear in the cleanup menu and clean a dictation, with no
  code change. (R-1, R-2, R-5)
- AC-3. An unknown protocol in the catalogue stops the app with a message naming the file, the
  line and the known protocols. (R-5)
- AC-4. Switching the cleanup provider in Settings takes effect on the next dictation without a
  restart; a dictation in progress finishes on the old one. (R-8, R-9)
- AC-5. Picking a provider with no key shows the key field in the row, and the role keeps its old
  provider until Test passes. (R-12)
- AC-6. Test against a test server reports "Works · N ms" for a good key and model, "key
  rejected" for a 401, and "model not found" for an unknown model; no key appears in any message
  or log. (R-13)
- AC-7. With a working key, the model menu lists the test server's models, the catalogue's first;
  with the listing failing, it shows the catalogue's models and a one-line notice. Opening the
  menu never adds time to a dictation. (R-15, R-17)
- AC-8. Every row shows a monthly cost that matches the onboarding estimate for the same talking
  time, and either a measured latency with its source or "Speed not measured". (R-18, R-19)
- AC-9. With Local server chosen and a fake local server on a usual port, the app finds it, lists
  its models, cleans a dictation with no key, and falls back to raw text after the deadline when
  the server is slow. (R-21 to R-23)
- AC-10. With a fallback cleanup set and the main one answering 500, the dictation is cleaned by
  the fallback inside the deadline, and the pill says "via" the fallback. (R-24, R-27)
- AC-11. With a fallback speech provider set and the main one refusing the connection, a dictation
  is transcribed by the fallback with no audio lost (the fake server receives every captured
  frame), and the timing line names the fallback. (R-25, R-27)
- AC-12. With the main speech session dropping mid-dictation, the saved audio is sent to the
  fallback on release and the text is pasted. (R-26)
- AC-13. Three fallbacks in a row show one notice linking to the main provider's row; the setting
  does not change. (R-28)
- AC-14. Choosing providers sends nothing over the network until Test or a dictation (checked
  with a test server that counts requests), and the Privacy section lists every provider set,
  fallbacks included. (R-29, R-30)
- AC-15. The dictation path is no slower: on the owner's samples, the median and p90 from key
  release to text with the recommended pair stay within 10 ms of the last Phase 1 measurement.
  (Principle 1, PRD section 6)

## Non-goals

- Local speech models for dictation. They stay in "Later" (PRD 7.6; `CLAUDE.md`: no local
  models for dictation in version 1). Meetings use Apple's recognizer through their own spec.
- Downloading models inside the app.
- A tab or a separate engine per feature, as OpenWhispr does.
- Per-app provider choice (a different model for Slack). Tone per app is the Style page's job.
- Translation, or telling the speech provider which language to expect (Phase 1 R-10).
- Speech providers that cannot tell Spanish from English by themselves in one dictation. A
  provider joins the menu only after it passes language detection on the owner's samples
  (Deepgram `nova-3` multi missed 2 of 38, [results](../../docs/results/2026-11-25-eu-speech.md)).
- A pricing or usage dashboard. The hint line is an estimate.

## Pre-mortem

It is three months later and this failed. Why?

| Why it failed | What prevents it |
| --- | --- |
| People picked a slow provider, dictation felt worse, and they blamed HyperMe | Measured latency on every row, "Speed not measured" otherwise (R-18), and the user's own median (R-20) |
| A provider renamed its models and the menu offered dead names | Models come from the key's own list (R-15); Test checks the model, not only the key (R-13) |
| Some "OpenAI-compatible" host rejected a field (`reasoning_effort`, the `system` role) | The catalogue says per model which optional fields to send; each new provider is tested against its docs before it enters the catalogue (`CLAUDE.md`, working rules) |
| The fallback hid a broken main provider for weeks and the bill went to the wrong account | "via" in the pill (R-27) and the notice after three in a row (R-28) |
| Audio went to a provider the user forgot they had set as fallback | Privacy lists fallbacks (R-29); nothing is sent before a test or a dictation (R-30) |
| The settings page grew into OpenWhispr's nine sections | One table of roles behind "Show advanced" (R-6); this spec adds no section |
| Runtime switching made the engine slower or leaked memory | Static dispatch over an enum of protocols (design.md); AC-15 measures it |

## Decisions

Made, from earlier owner decisions:

| # | Decision | Source |
| --- | --- | --- |
| D-1 | One table of roles on one page behind "Show advanced", not tabs per feature | UX.md, owner 26 November 2026 (U-12 and the settings plan) |
| D-2 | One key per provider, shared by every role | UX.md |
| D-3 | Onboarding asks only for the recommended keys; other providers live in Settings | PRD Q-5 |
| D-4 | OpenAI is the first provider added after the recommended pair | PRD 7.1 |
| D-5 | No local speech for dictation in version 1; local cleanup through an OpenAI-compatible server is allowed (it is already the meeting cleanup choice) | `CLAUDE.md`, ROADMAP "Meetings on device" |
| D-6 | Cleanup fallback ends at the raw transcript, as today | Phase 1 R-12, R-20 |

Proposed in this spec (Claude), rejected options in brackets:

| # | Decision | Why |
| --- | --- | --- |
| D-7 | Protocols in code, providers in the catalogue [one Rust type per provider] | Every cleanup host speaks the same protocol; new cleanup providers then cost no code |
| D-8 | Enum dispatch over protocols [`dyn` trait objects; restart on change] | The traits return `impl Future`, which `dyn` cannot hold without boxing every call; an enum is free and keeps the engine generic |
| D-9 | Latency hints only from our own measurements [providers' published figures] | Published figures are not on the owner's network or voice; `CLAUDE.md` asks for sources |

For the owner (asked on decision cards in the project thread, 26 November 2026):

| # | Question | Recommendation | Status |
| --- | --- | --- | --- |
| P-1 | When to build it: the cleanup half beside Phase 1, or all of it in Phase 2 as the PRD says | Cleanup half now (slices 1 to 7): no new protocol, and it helps close the formatting gap. Speech and fallbacks in Phase 2 | Decided 26 November 2026 (owner): cleanup half (slices 1 to 7) now, beside Phase 1; speech and fallbacks in Phase 2 |
| P-2 | Which speech provider to add first | OpenAI, as the PRD says: 50 of 50 languages right, though about 0.5 s slower than Speechmatics ([results](../../docs/results/2026-11-25-full-benchmark.md)), so it is the fallback, not a daily choice | Open |
| P-3 | Speech fallback: automatic, or a Retry button | Automatic, with "via OpenAI" on the pill | Open |
