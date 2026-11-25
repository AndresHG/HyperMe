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

## Flags

| Flag | Default | Meaning |
| --- | --- | --- |
| `--stt-provider` | `openai` | `openai` or `fake` |
| `--stt-live-model` | `gpt-live-transcribe` | Speech model for streaming (B) |
| `--stt-file-model` | `gpt-transcribe` | Speech model for upload (A) |
| `--realtime-url` | `wss://api.openai.com/v1/realtime?intent=transcription` | WebSocket URL |
| `--languages` | `es,en` | Language hints, the same for every dictation. `off` sends none |
| `--stt-delay` | not sent | The live model's delay: `minimal`, `low`, `medium`, `high`, `xhigh` |
| `--llm-model` | `gpt-6-luna` | Cleanup model |
| `--llm-base-url` | `https://api.openai.com/v1` | Any OpenAI-compatible chat API |
| `--llm-key-env` | `OPENAI_API_KEY` | Variable that holds the cleanup key |
| `--llm-deadline-ms` | `1500` | After this, the raw transcript is used and counted as a fallback |
| `--reasoning-effort` | `none` | `off` leaves the field out of the request |
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

The tests need no key and no network. The OpenAI socket and HTTP code is tested against a
server on localhost.
