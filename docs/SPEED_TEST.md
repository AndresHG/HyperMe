# Phase 0: speed test

Question to answer in one day: can our own pipeline put cleaned text on screen within
about 700 to 800 ms of key release, the way Wispr Flow does?

Build a small Rust command-line tool in `spikes/flowbench/`. No interface. It is a
throwaway measurement tool, but keep the provider code clean: Phase 1 reuses it.

## Before coding

The API details below were read from OpenAI's guide on 24 November 2026:
https://developers.openai.com/api/docs/guides/realtime-transcription
Open that page and the file transcription guide again and confirm URL, event names,
session shape and model names. They changed more than once this year.

The key comes from the `OPENAI_API_KEY` environment variable. Never print it.

## Samples

Ten dictations of 5 to 15 seconds, recorded by the owner, in the languages actually
used (Spanish and English, some mixed). Store as 24 kHz mono 16-bit WAV in
`spikes/flowbench/samples/` (git-ignored). Also support a live microphone mode
(`cpal`): Enter to start, Enter to stop.

## Pipeline A: upload after release (baseline)

1. At "key release", POST the WAV to `/v1/audio/transcriptions`, model `gpt-transcribe`.
2. Clean the text with a streamed chat completion.

## Pipeline B: streaming

1. Before the audio starts, open `wss://api.openai.com/v1/realtime?intent=transcription`
   with the bearer key. Do not put a model in the URL.
2. Send:
   ```json
   {"type": "session.update",
    "session": {"type": "transcription",
      "audio": {"input": {
        "format": {"type": "audio/pcm", "rate": 24000},
        "transcription": {"model": "gpt-live-transcribe"},
        "turn_detection": null}}}}
   ```
3. Send `{"type": "input_audio_buffer.append", "audio": "<base64 PCM16>"}` in chunks of
   about 100 ms, paced in real time (as a microphone would deliver them).
4. At "key release" send `{"type": "input_audio_buffer.commit"}` and start the clock.
5. Collect `conversation.item.input_audio_transcription.delta`; stop at
   `conversation.item.input_audio_transcription.completed`. Treat
   `...transcription.failed` and `error` as failures and print their bodies.
6. Clean the text: model `gpt-6-luna`, reasoning effort `none`, streamed.

Use the same warm HTTP client for the cleanup call in both pipelines, so the comparison
isolates streaming against upload.

## Cleanup prompt (starting point)

System: "You clean up dictated text. Remove filler words and false starts. Fix
punctuation, capitalisation and obvious mis-hearings. Keep the speaker's wording and
language. Format a list when the speaker enumerates. Output only the cleaned text."

## Measure

From the commit (B) or the start of the upload (A), per sample:

- `stt_ms`: final transcript received
- `first_ms`: first cleaned token received
- `total_ms`: full cleaned text received

Report median and 90th percentile over the ten samples, per pipeline, as a table, and
print the raw transcript and the cleaned text of each sample for a quality check.
Write the run to `docs/results/speed-test-YYYY-MM-DD.md` with machine and network.

## Flags

`--stt-live-model`, `--stt-file-model`, `--llm-model`, `--llm-base-url`,
`--llm-key-env`, `--reasoning-effort` (value `off` omits the field), `--realtime-url`,
`--runs`, `--mic`.

## Variants, if B misses the gate

- Cleanup on Groq or Cerebras: both expose OpenAI-compatible chat endpoints, so only
  the base URL, key and model change.
- Speech on Deepgram or ElevenLabs Scribe v2 Realtime: add a second provider behind
  the same trait.

## Quality check

The owner dictates the same ten samples into Wispr Flow. Put both outputs side by side
in the results file and mark which reads better.

## What the result decides

| Result | Decision |
| --- | --- |
| Median `total_ms` at or under 800, 90th percentile at or under 1200, formatting acceptable | Build dictation in Phase 2 |
| Speed passes, formatting is weak | Build dictation; budget time for prompt work and a test set |
| Median over 1200 on every variant | Keep Wispr Flow for dictation; build only the agent layer |

The thresholds are a proposal; the owner can change them.
