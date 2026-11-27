# Choose the speech and cleanup provider: slices

Each slice is one pull request, starts from failing tests that cite its `AC-n`, and passes fmt,
clippy and tests before it merges. Open one issue per slice when the spec is approved. Part A starts
now, beside Phase 1; Part B waits for Phase 2 (owner, 26 November 2026, P-1 in [spec.md](spec.md)).

## Part A: the cleanup half (no new protocol)

| # | Slice | Proves | Size | Depends on |
| --- | --- | --- | --- | --- |
| 1 | Catalogue in the config: `[providers.*]` with models, prices and measured latency; roles name provider and model; old files keep working; `cost.rs` reads catalogue models | AC-1, AC-2 (config half), AC-3 | M | none |
| 2 | `AnySpeech` / `AnyCleanup` enum dispatch and one `resolve` path used by the runtime and by Test; switch providers without a restart through a `watch` channel | AC-4 | M | 1 |
| 3 | Providers page "Show advanced": the table of roles, provider and model menus, inline key control reused from the walkthrough, Reset to recommended; catalogue entries for OpenAI, OpenRouter, Gemini, Anthropic and a custom endpoint, each checked against its docs; mockup `docs/ux/providers-advanced.svg` | AC-2, AC-5 | M | 2 |
| 4 | Models from the key: `GET /models`, filtered and cached, catalogue first, free-text entry, fallback to the catalogue | AC-7 | S | 3 |
| 5 | Test checks key and model: one-token completion, clear messages for 401 and unknown model | AC-6 | S | 2 |
| 6 | Hint line: monthly cost from the onboarding talking time, measured latency with its source or "Speed not measured", the user's own median when the timing log is on | AC-8 | S | 3 |
| 7 | Local server for cleanup: no key, discovery on the usual ports, hint "runs on this Mac" | AC-9 | S | 3 |

## Part B: speech choice and fallbacks

| # | Slice | Proves | Size | Depends on |
| --- | --- | --- | --- | --- |
| 8 | OpenAI streaming speech (`openai-realtime`), ported from `spikes/flowbench` and checked against OpenAI's current docs; a local fake server; measured on the owner's samples, numbers to `docs/results/` | AC-11 (speech half) | M | 2 |
| 9 | Cleanup fallback: `Fallback<AnyCleanup>`, inside the deadline; "via" on the pill; provider on the timing line | AC-10, AC-10a | S | 2 |
| 10 | Speech fallback: `Fallback<AnySpeech>` with the frame buffer and replay; mid-dictation drop sends saved audio on release; three-in-a-row notice | AC-11, AC-12, AC-13 | M | 8 |
| 11 | Privacy section lists every provider set, fallbacks included; request-counting test | AC-14 | S | 3, 9 |
| 12 | Speed check: flowbench replay and timing log with the recommended pair after slices 2 and 10, written to `docs/results/` | AC-15 | S | 2, 10 |

## Progress

- Slices 1 to 7: merged.
- Slice 8: OpenAI streaming speech is the catalogue entry `openai-speech` (protocol
  `openai-realtime`, model `gpt-live-transcribe`), sharing the OpenAI key through `key_from`
  (D-2). `hyperme-quality run` now builds speech through `any::speech`, so a config naming
  `openai-speech` replays through it. Its measurement on the owner's samples waits for an OpenAI
  key in the keychain (none on 27 November 2026).
- Slice 9: `[cleanup] fallback = { provider, model }`, written by Settings only after its Test
  passes. `hyperme_core::fallback::Fallback` wraps any cleanup: an error before the first token
  goes to the fallback inside the engine's deadline. The pill says "Pasted · via <name>"; the
  trace root records `cleanup_via` and the timing line names the fallback. A fallback with no
  key is not built and shows red in the Setup check.
- Slice 10: `[speech] fallback`, the same Settings row. `Fallback<Arc<AnySpeech>>` opens the
  fallback when the main session cannot open (the engine's open buffer replays into it), and on
  a drop mid-dictation keeps every frame and sends it to the fallback on release. The frames
  are held only while a fallback is set. `Transcript.via` carries the name to the pill and the
  timing line. `FallbackStreak` in the app shows one alert card after three in a row, linking
  to `hyperme://settings/providers/<role>`.
- Slice 11: `Config::destinations()` lists every provider set, fallbacks included, and Settings
  → Privacy shows each with what it receives ("Stays on this Mac" for a local server). A test
  with counting servers shows choosing, saving and rebuilding providers opens no connection;
  Test is the first request.
- Slice 12: `hyperme-quality` builds providers with the app's own builders. Before and after
  runs, alternating, on the owner's 50 samples showed no measurable slowdown: median key release
  to text was 946 and 960 ms after, against 939 and 1013 ms before, with cleanup on OpenAI
  because Groq's free daily limit was spent
  ([results](../../docs/results/2026-11-27-provider-settings-speed.md)). Slices 1 to 12 are done.

## Later, one pull request each

- A further speech provider (Soniox, AssemblyAI, ElevenLabs), each only after it gets Spanish and
  English right on the owner's samples in flowbench.
- Answers role (phase 3) and meeting cleanup role (phase 5) as rows in the same table.
