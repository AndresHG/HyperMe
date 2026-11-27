# Choose the speech and cleanup provider: design

How [spec.md](spec.md) fits the code that exists. Draft, 26 November 2026.

## Config: a catalogue and roles

Today `[speech]` and `[cleanup]` each carry a provider name, a model, an endpoint, a key page and
prices. The change splits "what a provider is" from "which one a role uses":

```toml
# What each provider is. Shipped in default-config.toml; a user may add entries.
[providers.speechmatics]
name = "Speechmatics"
protocol = "speechmatics-rt"
endpoint = "wss://eu.rt.speechmatics.com/v2"
key_url = "https://portal.speechmatics.com/signup"
roles = ["speech"]
free_credit_usd = 100

[[providers.speechmatics.models]]
id = "melia-1"
price_per_hour_usd = 0.40
prices_checked = "2026-11-26"
measured = { release_to_text_ms = 426, p90_ms = 587, source = "docs/results/2026-11-25-eu-speech.md" }

[providers.groq]
name = "Groq"
protocol = "openai-chat"
endpoint = "https://api.groq.com/openai/v1"
key_url = "https://console.groq.com/keys"
roles = ["cleanup"]
list_models = true

[[providers.groq.models]]
id = "openai/gpt-oss-20b"
price_input_per_million_usd = 0.075
price_output_per_million_usd = 0.30
prices_checked = "2026-11-26"
reasoning_effort = ["low", "medium", "high"]   # sent only if listed
measured = { release_to_text_ms = 426, p90_ms = 587, source = "docs/results/2026-11-25-eu-speech.md" }

[providers.local]
name = "Local server"
protocol = "openai-chat"
endpoint = "http://127.0.0.1:11434/v1"   # Ollama; LM Studio 1234, mlx_lm.server 8080
roles = ["cleanup"]
needs_key = false
list_models = true

# Which provider each role uses.
[speech]
provider = "speechmatics"
model = "melia-1"
# fallback = { provider = "openai", model = "..." }

[cleanup]
provider = "groq"
model = "openai/gpt-oss-20b"
reasoning_effort = "low"
prompt = "v4"
deadline_ms = 1500
# fallback = { provider = "openai", model = "..." }
```

Model names above are today's; the real ones for each new provider are checked against its docs
in the slice that adds it (`CLAUDE.md`, working rules).

**Where the catalogue lives (slice 1).** Built into the app as `default-providers.toml`, not
written into the user's `config.toml`. A copy in the user's file would freeze that release's
prices and providers forever; built in, each release brings its own. The user's file may still
add an entry or replace a built-in one by writing `[providers.<name>]`, which wins by name (R-5).
`needs_key` is spelled `key_required` (default true), because the config refuses any field whose
name ends in `_key` as a likely secret.

**One key, two protocols (slice 8).** OpenAI speech and OpenAI cleanup speak different
protocols, so they are two catalogue entries, `openai-speech` and `openai`. The speech entry says
`key_from = "openai"`: every key lookup goes through `Config::key_account`, so both read and
write the one OpenAI keychain entry (D-2).

**Migration (R-4).** An old `[speech]` or `[cleanup]` with `endpoint`, `key_url` or price fields
still parses: those fields override the catalogue entry of the same provider, and the app does
not rewrite the user's file. The catalogue itself comes from `default-config.toml` when the user's
file has none, the way `or_default!` works today in `config.rs`. Unknown keys stay an error.

**Prices and the estimate.** `cost.rs` already turns prices and talking time into a monthly
figure for onboarding. It takes a catalogue model instead of the `[speech]` and `[cleanup]`
sections, so the wizard and every Settings row compute the same number (AC-8).

## Code: protocols, not providers

```
hyperme-core
  traits.rs        SpeechProvider, LiveSession, CleanupProvider   (unchanged)
  config.rs        Catalogue, ProviderEntry, ModelEntry, Role { provider, model, fallback }
  providers.rs     new: Protocol enum, resolve(role, catalogue) -> Resolved { entry, model }
hyperme-providers
  speechmatics.rs  protocol "speechmatics-rt"                      (exists)
  cleanup.rs       protocol "openai-chat"                          (exists, gains model listing)
  openai_rt.rs     protocol "openai-realtime"  (slice 8, ported from spikes/flowbench)
  any.rs           new: AnySpeech, AnyCleanup, Fallback<S>
```

**Enum dispatch (D-8).** The engine is generic over `S: SpeechProvider` and `C: CleanupProvider`,
and both traits return `impl Future`, so they cannot be trait objects without boxing. `any.rs`
adds:

```rust
pub enum AnySpeech { Speechmatics(Speechmatics), OpenAi(OpenAiRealtime) }
pub enum AnySession { Speechmatics(SpeechmaticsSession), OpenAi(OpenAiSession) }
pub enum AnyCleanup { Chat(ChatCleanup) }           // every cleanup host today
```

each implementing the trait with a `match`. Adding a protocol adds a variant; the engine and its
tests do not change. Cost on the dictation path: one branch per call.

**Changing provider without a restart (R-8, R-9).** The engine reads its providers from a
`tokio::sync::watch` channel of `Arc<(AnySpeech, AnyCleanup)>` once on key-down and keeps that
pair for the whole dictation. Settings sends a new pair after a successful save. No new crate: `watch` is in tokio.

**Fallback (R-24 to R-26).** A wrapper, so the engine stays as it is:

- `Fallback<AnyCleanup>`: `clean` calls the main provider; on an error before the first token it
  calls the fallback with the same `on_text`. The engine's deadline still wraps the whole call, so
  R-12 holds and the raw transcript is still the last resort.
- `Fallback<AnySpeech>`: `open` tries the main provider; on failure it opens the fallback. The
  session buffers every frame sent since key-down (at most `max_length_s`, already bounded by the
  history), so if it switches it replays the buffer first. A drop mid-dictation marks the session;
  `finish` then sends the buffered audio to the fallback as one fast stream.
- The wrapper reports which provider answered, for the pill (R-27) and the timing line, through a
  new optional field `provider` on `Transcript` and on the cleanup result.
- The three-in-a-row notice (R-28) is a counter in the app, not the engine.

**Test (R-13).** `key_test` builds the role's provider through the same `resolve` path the runtime
uses, so Test and dictation cannot disagree. Cleanup Test sends a one-token completion
(`max_tokens: 1`) to the chosen model instead of only `GET /models`, so a wrong model fails Test.
Speech Test opens a session with the chosen model and closes it.

**Model listing (R-15).** `ChatCleanup::list_models()` calls `GET /models` (it already does for
warm-up, and drops the body). The app filters out names the catalogue marks as non-chat (embedding,
whisper, tts, moderation) by simple patterns in the catalogue, not in code. Speech protocols list
from the catalogue only.

**Local server discovery (R-22).** On picking Local server, the app tries `GET /v1/models` on
127.0.0.1 ports 11434, 1234 and 8080 with a 300 ms timeout each, in parallel, and offers the ones
that answer. Ports live in the catalogue entry.

## Settings page

The Providers section keeps its two rows (UX.md) and gains "Show advanced":

```
Providers                                                        [Reset to recommended]
Every model service in one place. Keys stay in the macOS keychain.

Role                 Provider           Model                     Key / Test
Dictation speech     Speechmatics  ▾    melia-1             ▾     ● Works · 18 ms   [Test]
                     About $6 a month · 0.43 s measured (link) · Your median 0.47 s
  Fallback           OpenAI        ▾    (model)             ▾     ○ No key  [Paste key] [Get a key]
                     Only if Speechmatics fails · about 0.5 s slower
Dictation cleanup    Groq          ▾    openai/gpt-oss-20b  ▾     ● Works · 151 ms  [Test]
                     About $0.15 a month · 0.43 s measured with Speechmatics
  Fallback           None          ▾
Answers              (phase 3)
Meeting cleanup      (phase 5)
```

- Provider menu: recommended first, then the PRD's order (OpenAI first among the others), then
  the rest, then "Local server" for cleanup, then "Add a custom endpoint…", which writes a new
  catalogue entry with protocol `openai-chat`.
- Model menu: the key's list, catalogue models first, with a free-text entry at the bottom.
- Fallback rows show only with "Show advanced", collapsed to "Fallback: None" until set.
- Saves as you go (UX.md rule 2). Errors from the pill link here with the row highlighted
  (`hyperme://settings/providers#speech`).
- The page reuses the walkthrough's key control (Get a key, paste, Test) so both look and behave
  the same (U-12).

Mockup: to be drawn as `docs/ux/providers-advanced.svg` in slice 3, in the style of
[settings.svg](../../docs/ux/settings.svg).

## First providers

| Role | Provider | Protocol | Slice | Why |
| --- | --- | --- | --- | --- |
| Cleanup | Groq | openai-chat | exists | Recommended |
| Cleanup | OpenAI | openai-chat | 3 | PRD: first extra provider. Measured slower to first token (about 0.8 s, [results](../../docs/results/2026-11-25-summary.md)) |
| Cleanup | OpenRouter | openai-chat | 3 | One key, many models |
| Cleanup | Google Gemini | openai-chat (Google's compatibility endpoint) | 3 | Check the endpoint and fields in Google's docs first |
| Cleanup | Anthropic | openai-chat (Anthropic's OpenAI SDK compatibility) | 3 | Check the endpoint and fields in Anthropic's docs first; a native client only if compatibility drops streaming or the system prompt |
| Cleanup | Local server | openai-chat | 7 | Ollama, LM Studio, `mlx_lm.server`; already measured for meetings ([results](../../docs/results/2026-11-26-local-meetings.md)) |
| Cleanup | Custom endpoint | openai-chat | 3 | Any other host |
| Speech | Speechmatics | speechmatics-rt | exists | Recommended |
| Speech | OpenAI | openai-realtime | 8 | PRD; language 50 of 50; flowbench has a working client to port |
| Speech | Soniox, AssemblyAI, ElevenLabs | one each | later | Each joins only after passing the owner's samples in flowbench (non-goal on language) |

## Failure modes

| Failure | Behaviour |
| --- | --- |
| Catalogue entry with an unknown protocol | Startup error naming file, line and known protocols (AC-3) |
| Role names a provider missing from the catalogue | Startup error naming the role and the providers that exist |
| Key missing for the chosen provider | Setup check row turns red; dictation uses nothing new until Test passes (R-12) |
| Model listing fails | Catalogue models, one-line notice (R-17) |
| Main and fallback both fail (speech) | Today's error in the pill (Phase 1 R-19); audio kept for Retry (R-31) |
| Main and fallback both fail (cleanup) | Raw transcript, "Raw text" on the pill (Phase 1 R-20) |

## Budgets

- Dictation path: one `match` per trait call and one `watch` read on key-down. AC-15 measures it
  on the owner's samples.
- Memory: the catalogue is a few KB; model lists are cached only while Settings is open, and
  Settings is dropped when closed (`settings.rs`).
- The speech fallback buffer reuses the PCM the history already writes; it is held in memory only
  while a fallback is set, and is bounded by `max_length_s` (120 s at 16 kHz mono is about 3.8 MB).

## Test plan

Every test uses local fakes (`CLAUDE.md`: no paid API, no key):

- Config: old files load (AC-1); hand-added provider (AC-2); unknown protocol (AC-3). Unit tests
  in `crates/hyperme-core/tests/config.rs`.
- Dispatch and switching: the engine with fake providers behind `AnySpeech`/`AnyCleanup`; a switch
  during a dictation (AC-4).
- Test and listing: the existing local OpenAI-compatible test server in
  `crates/hyperme-providers/tests/common`, extended with a 401, an unknown model and `/models`
  (AC-6, AC-7).
- Local server: the same test server on a usual port (AC-9).
- Fallbacks: two fake servers, one refusing or dropping, one counting frames (AC-10 to AC-13).
- Privacy: a request-counting server while the page changes settings (AC-14).
- Speed: flowbench replay and the timing log on the owner's samples, numbers to `docs/results/`
  (AC-15).
