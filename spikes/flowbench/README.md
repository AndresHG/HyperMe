# flowbench

The Phase 0 speed test for HyperMe: how long from key release to cleaned text?
The plan and the gate are in [`docs/SPEED_TEST.md`](../../docs/SPEED_TEST.md).

It runs each recorded dictation through two pipelines and prints every stage:

- **A, upload:** at key release, upload the WAV, then clean the transcript.
- **B, streaming:** open and warm the connections, stream the audio in 100 ms chunks in real
  time, commit at key release, then clean the transcript.

Run every command below from this folder, `spikes/flowbench/`.

## Try it with no key

```bash
cargo run -- --stt-provider fake
```

This uses built-in fake providers: canned text and simulated timings, no network, no samples.
It shows the stage log, the results table, the gate and the results file. Its numbers mean
nothing, and its file goes to `target/speed-test-fake.md`, never to `docs/results/`.

## 1. Record the fifty samples

Fifty dictations of 5 to 15 seconds: 25 in Spanish and 25 in English, one language each.

| | |
| --- | --- |
| Folder | `spikes/flowbench/samples/` (git-ignored) |
| Names | `es-01.wav` to `es-25.wav`, `en-01.wav` to `en-25.wav` |
| Format | WAV, 24 kHz, mono, 16-bit |

The tool records in that format. For each sample, press Enter to start and Enter to stop:

```bash
for i in {01..25}; do cargo run -q --release -- --record es-$i; done
```

```bash
for i in {01..25}; do cargo run -q --release -- --record en-$i; done
```

- The first time, macOS asks to let your terminal use the microphone. Allow it.
- Ctrl-C stops the loop. Run it again to continue: samples that exist are skipped.
- To redo a take, delete the file and record it again: `rm samples/es-07.wav`, then
  `cargo run -q --release -- --record es-07`.
- Speak as you really dictate: messages, prompts for coding agents, notes. Keep the fillers
  and false starts. Include a few lists ("three things: ...") and some names and technical
  terms, because those test the formatting.
- Say nothing private. The transcripts are written to `docs/results/`, in a repository that
  is meant to be public.

A recording made elsewhere can be converted:

```bash
afconvert -f WAVE -d LEI16@24000 -c 1 recording.m4a samples/es-01.wav
```

## 2. Set the key

The tool reads `OPENAI_API_KEY` from the environment and never prints it. To keep the key out
of files and shell history, store it in the macOS keychain once. Copy the whole key to the
clipboard, then run:

```bash
security add-generic-password -U -a "$USER" -s OPENAI_API_KEY -w "$(pbpaste)"
```

Copy something else afterwards to clear the clipboard. Shell history keeps only `$(pbpaste)`.
`-U` replaces an entry that is already there. Do not use the interactive prompt (`-w` with no
value): it cuts the input at 128 characters, and OpenAI project keys are longer, so the stored
key is truncated and every call returns 401.

For a cleanup key on another host, such as Groq, store it the same way under its own name
(`-s GROQ_API_KEY`) and pass `--llm-key-env GROQ_API_KEY`.

## 3. Run

Two samples first, to check that everything connects:

```bash
OPENAI_API_KEY=$(security find-generic-password -a "$USER" -s OPENAI_API_KEY -w) cargo run --release -- es-01 en-01
```

Then all fifty. Streaming plays each sample in real time, so this takes about ten minutes. By
the prices in the research file it costs roughly 20 US cents.

```bash
OPENAI_API_KEY=$(security find-generic-password -a "$USER" -s OPENAI_API_KEY -w) cargo run --release
```

The run prints the stage log, the raw and cleaned text of every sample, the results table and
the gate, and writes `docs/results/speed-test-YYYY-MM-DD.md` with machine and network. A
second run on the same day writes `-2.md`: a results file is never overwritten.

## 4. Try it by voice

```bash
OPENAI_API_KEY=$(security find-generic-password -a "$USER" -s OPENAI_API_KEY -w) cargo run --release -- --mic
```

Enter is key-down, Enter again is key release. Each dictation runs through pipeline B and
prints its timings and text. No audio is written to disk. With `--stt-provider fake` this
checks the microphone without a key.

## 5. Compare with Wispr Flow

The results file ends with the texts of every sample, each followed by an empty "Wispr Flow"
line. Dictate the same samples into Wispr Flow (playing one back with
`afplay samples/es-01.wav` while holding the Wispr Flow key works), paste each result there
and mark which reads better. The tool does not fill this in.

## 6. Find where the time goes (network)

```bash
cargo run --release -- --net
```

Prints the TCP round trip from this Mac to every speech and cleanup host worth comparing:
OpenAI, Groq, Cerebras, Anthropic, Speechmatics (EU and global), Deepgram (EU and default),
AssemblyAI (EU and default) and ElevenLabs. No key is used and nothing is sent beyond the TCP
handshake, so the numbers are the network alone. The "warm round trip" in a results file is a
`GET /models` request and includes the server's own time. Run it once on Wi-Fi and once wired.
`--net-host HOST` (repeatable) checks other hosts instead.

Every run now also writes the TCP round trip to its speech and cleanup hosts into the results
file, and the stage log notes the cleanup model's reasoning: when its first token of any kind
arrived, how many reasoning characters it streamed, and its reasoning tokens when the host
reports them. A gap between the first token of any kind and the first text token is time the
model spends thinking before it writes.

## 7. Compare cleanup setups without speaking (replay)

Every run with real providers saves the raw transcripts to `samples/transcripts.json`
(git-ignored). `--replay` sends them to a cleanup setup, with no speech provider and no
speaking, and reports time and quality. A results file that still has its "Texts" section works
too: `--transcripts path/to/speed-test-2026-11-25-3.md`.

```bash
GROQ_API_KEY=$(security find-generic-password -a "$USER" -s GROQ_API_KEY -w) cargo run --release -- \
  --replay --prompt v2 --llm-base-url https://api.groq.com/openai/v1 \
  --llm-key-env GROQ_API_KEY --llm-model openai/gpt-oss-20b --reasoning-effort low
```

- `--prompt v1` is the prompt of the 25 November runs. `--prompt v2` names the fillers in both
  languages, keeps only the final version after a self-correction, allows a word fix from
  context, forbids translating or answering, and has six made-up examples. `--prompt v3` is v1
  with strict limits: remove only hesitations and false starts, keep informal words and
  swearing, never translate or rephrase. `--prompt FILE` reads any other prompt.
- `--vocab FILE` is your dictionary, one term per line, optionally with how it sounds:
  `Zentrix: zen trix`. The cleanup prompt gets the written forms. Speechmatics gets it as
  `additional_vocab` only with a single-language model such as `enhanced`: on 25 November 2026
  `melia-1` with `language: multi` refused the session with "additional properties
  'additional_vocab' not allowed", so with `melia-1` or `oak-1` flowbench prints a warning and
  sends the dictionary to the cleanup only. Keep the file in `samples/`, which git ignores, if it holds internal
  names. Speechmatics warns that a dictionary can delay the session start by up to 15 s; the
  start is before key release, so `total_ms` does not include it.
- Times are from the moment the cleanup request is sent. Add the speech provider's `stt_ms` for
  the time from key release.
- The table also counts texts left unchanged and fillers left (um, uh, eh, o sea, you know and
  similar).
- Numbers go to `docs/results/replay-YYYY-MM-DD.md`; the texts to
  `samples/replay-YYYY-MM-DD-<model>.md`, which stays out of the repository.
- The first replay writes `samples/gold-draft.json` with its cleaned texts. Correct each text to
  exactly what you would send and save the file as `samples/gold.json`. From then on every replay
  also reports the zero-edit rate (outputs that need no edit, the measure Voice Arena uses) and
  edits per 100 characters.

Other hosts to try, each with its own key stored the same way. Check each host's current model
names and documentation first.

| Host | `--llm-base-url` | `--llm-model` (examples) | `--reasoning-effort` |
| --- | --- | --- | --- |
| Groq | `https://api.groq.com/openai/v1` | `openai/gpt-oss-20b`, `openai/gpt-oss-120b` | `low` |
| Cerebras | `https://api.cerebras.ai/v1` | `gpt-oss-120b` | `low` |
| OpenAI | `https://api.openai.com/v1` | `gpt-6-luna` | `none` |

## 8. Try an EU streaming speech provider

Speechmatics and Deepgram stream to EU hosts and take an explicit end-of-speech message, so
the final transcript does not wait for silence detection. Both only stream: pipeline A is not
run with them. Each gives free credit on sign-up; a 38-sample run is about 5 minutes of audio.

| Provider | Key variable | Default host | Default model | End of speech |
| --- | --- | --- | --- | --- |
| Speechmatics | `SPEECHMATICS_API_KEY` | `wss://eu.rt.speechmatics.com/v2` | `melia-1` (`language: multi`, finds the language itself) | `ForceEndOfUtterance` |
| Deepgram | `DEEPGRAM_API_KEY` | `wss://api.eu.deepgram.com/v1/listen` | `nova-3` (`language=multi`) | `Finalize`, then `CloseStream`; ends once all audio is processed |

Store each key in the keychain as in section 2 (`-s SPEECHMATICS_API_KEY`,
`-s DEEPGRAM_API_KEY`). Two samples first, with the Groq cleanup of run 3:

```bash
SPEECHMATICS_API_KEY=$(security find-generic-password -a "$USER" -s SPEECHMATICS_API_KEY -w) \
GROQ_API_KEY=$(security find-generic-password -a "$USER" -s GROQ_API_KEY -w) \
cargo run --release -- --stt-provider speechmatics \
  --llm-base-url https://api.groq.com/openai/v1 --llm-key-env GROQ_API_KEY \
  --llm-model openai/gpt-oss-20b --reasoning-effort low es-01 en-01
```

Then drop `es-01 en-01` for all samples, and repeat with `--stt-provider deepgram` and
`DEEPGRAM_API_KEY`. Notes:

- Speechmatics `--stt-delay 0.7` sets `max_delay` to its lowest value. If the service rejects
  the language hints, `--languages off` leaves them out. `enhanced` and `standard` need exactly
  one language in `--languages`.
- `--realtime-url wss://global.rt.speechmatics.com/v2` or `wss://api.deepgram.com/v1/listen`
  compares against the non-EU hosts.
- Sources, read on 25 November 2026:
  [Speechmatics real-time API](https://docs.speechmatics.com/api-ref/realtime-transcription-websocket),
  [Speechmatics turn detection](https://docs.speechmatics.com/speech-to-text/realtime/turn-detection),
  [Deepgram regional endpoints](https://developers.deepgram.com/reference/regional-endpoints),
  [Deepgram Finalize](https://developers.deepgram.com/docs/finalize),
  [Deepgram CloseStream](https://developers.deepgram.com/docs/close-stream),
  [Speechmatics custom dictionary](https://docs.speechmatics.com/speech-to-text/features/custom-dictionary).
  Not stated there: whether `additional_vocab` works with `language: multi` and `melia-1`. It
  did not on 25 November 2026: the session was refused (see `--vocab` above).
  Not verified there: the exact form of Speechmatics `language_hints`.

## Flags

| Flag | Default | Meaning |
| --- | --- | --- |
| `--stt-provider` | `openai` | `openai`, `speechmatics`, `deepgram` or `fake` |
| `--stt-live-model` | per provider | Speech model for streaming (B): `gpt-live-transcribe`, `melia-1`, `nova-3` |
| `--stt-file-model` | `gpt-transcribe` | Speech model for upload (A) |
| `--realtime-url` | per provider | WebSocket URL; EU hosts for Speechmatics and Deepgram |
| `--languages` | `es,en` | Language hints, the same for every dictation. `off` sends none |
| `--stt-delay` | not sent | OpenAI: `minimal` to `xhigh`. Speechmatics: `max_delay` in seconds, 0.7 to 4 |
| `--llm-model` | `gpt-6-luna` | Cleanup model |
| `--llm-base-url` | `https://api.openai.com/v1` | Any OpenAI-compatible chat API |
| `--llm-key-env` | `OPENAI_API_KEY` | Variable that holds the cleanup key |
| `--llm-deadline-ms` | `1500` | After this, the raw transcript is used and counted as a fallback |
| `--reasoning-effort` | `none` | `off` leaves the field out of the request |
| `--prompt` | `v1` | Cleanup prompt: `v1`, `v2`, `v3` or a file |
| `--vocab FILE` | none | Dictionary for the cleanup, and for Speechmatics single-language models |
| `--deepgram-finalize-only` | | Deepgram: end on the Finalize reply, not after CloseStream. Faster, but lost final words |
| `--net` | | Print TCP round trips to the provider hosts, then stop |
| `--net-host HOST` | built-in list | Hosts for `--net`; repeatable |
| `--replay` | | Saved transcripts through the cleanup only |
| `--transcripts PATH` | `samples/transcripts.json` | Transcripts for `--replay`: JSON or a results file |
| `--gold PATH` | `samples/gold.json` | Corrected texts for zero-edit scoring |
| `--runs` | `1` | Times to run the whole sample set |
| `--mic` | | Live microphone instead of samples |
| `--record NAME` | | Record one sample to `samples/NAME.wav` |
| `[SAMPLES]...` | all | Sample names to run, such as `es-01 en-01` |

To try the cleanup on another host, change `--llm-base-url`, `--llm-key-env` and
`--llm-model`. Check that host's current documentation first, and use `--reasoning-effort off`
if it does not accept the field.

`FLOWBENCH_TRACE=1` prints the type of every realtime event, for debugging.

## What is measured

Times are in milliseconds from key release: the start of the upload in A, the commit in B.

| Value | Meaning |
| --- | --- |
| `stt_ms` | Final transcript received |
| `first_ms` | First cleaned token received |
| `total_ms` | Full cleaned text received, or the raw transcript shown after a fallback |
| `lang_ok` | Raw and cleaned text are in the language the file name says |

The gate is over all samples: median `total_ms` at or under 800, 90th percentile at or under
1200, language correct on every sample, no failed sample, and the raw transcript used instead
of cleaned text on at most one sample in ten. Formatting quality is judged by reading the
texts.

- **Fallback:** the cleanup missed its deadline, so the raw transcript is used. It counts in
  `total_ms` at the moment the raw transcript would be shown.
- **Failed:** the speech provider or the cleanup returned an error, or the transcript was
  empty. A failed sample is left out of the times and fails the gate, so a broken run can
  never look fast.
- With a cleanup host other than `api.openai.com`, the first upload of pipeline A opens a new
  connection and is slower than the rest. Pipeline B is not affected.

`lang_ok` is a word-count check in the tool, because the streaming model reports no detected
language. It catches a wrong language or a translation, not a single wrong word.

## Layout

| File | Holds |
| --- | --- |
| `src/stt/mod.rs` | The speech-provider trait: file upload and live session |
| `src/stt/openai.rs` | OpenAI upload and realtime WebSocket |
| `src/stt/speechmatics.rs` | Speechmatics real-time WebSocket |
| `src/stt/deepgram.rs` | Deepgram streaming WebSocket |
| `src/stt/ws.rs` | What the streaming providers share: socket, reader, end of speech |
| `src/net.rs` | TCP round trips, `--net` |
| `src/replay.rs` | `--replay`: transcripts in, timings and quality signals out |
| `src/vocab.rs` | The user's dictionary, `--vocab` |
| `src/quality.rs` | Fillers, unchanged texts, zero-edit and edit distance |
| `src/cleanup.rs` | The cleanup trait, the OpenAI-compatible chat client, the deadline |
| `src/pipeline.rs` | Pipelines A and B and the stage timestamps |
| `src/stats.rs` | Median, 90th percentile, the gate |
| `src/report.rs` | Stage log, table, results file, machine and network |
| `src/audio.rs` | WAV, chunk schedule, resampler |
| `src/mic.rs` | Microphone: live mode and `--record` |
| `src/fake.rs` | Offline fake providers |

## Tests

```bash
cargo test
```

The tests need no key and no network. Every provider's socket and HTTP code is tested against
a server on localhost.
