# Phase 0: speed test

Question to answer in one to two days: can our own pipeline put cleaned text on screen
within about 700 to 800 ms of key release, the way Wispr Flow does?

This is the owner's gate for the whole project (24 November 2026): show Wispr Flow
performance first, with numbers and by trying it by voice. No overlay is needed. A pass
shows that it can be done, not that the product should be built: see
[RESEARCH.md](RESEARCH.md), section 1.3.

Build a small Rust command-line tool in `spikes/flowbench/`. No interface. It is a
throwaway measurement tool, but keep the provider code clean: the next phase reuses it.

## Before coding

The API details below were read from OpenAI's guide on 24 November 2026:
https://developers.openai.com/api/docs/guides/realtime-transcription
Open that page and the file transcription guide again and confirm URL, event names,
session shape and model names. They changed more than once this year. Do the same for
every other provider you add. Prices and latency claims for the candidates are in
[research/2026-11-24/01-dictation.md](research/2026-11-24/01-dictation.md), section C.
Every latency figure there is a vendor claim.

Keys come from environment variables: `OPENAI_API_KEY`, and one per extra provider.
Never print them.

## Samples

Fifty dictations of 5 to 15 seconds, recorded by the owner: 25 in Spanish and 25 in
English, one language per dictation. The owner does not mix languages inside a sentence,
so mixed sentences are not tested. Store as 24 kHz mono 16-bit WAV in
`spikes/flowbench/samples/` (git-ignored), named so the language is known (`es-01.wav`,
`en-01.wav`).

Also support a live microphone mode (`cpal`): Enter to start, Enter to stop. The owner
uses it to try the pipeline by voice. That is part of the gate.

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

Rules for every streaming provider:

- Open and warm every connection before the audio starts, including the cleanup client.
- Key release is the end of speech. Send the provider's explicit commit or finalize
  message; do not wait for server-side turn detection.
- Give cleanup a deadline (`--llm-deadline-ms`, default 1500). On timeout, output the raw
  transcript and count the sample as a fallback.

## Cleanup prompt (starting point)

System: "You clean up dictated text. Remove filler words and false starts. Fix
punctuation, capitalisation and obvious mis-hearings. Keep the speaker's wording and
language. Format a list when the speaker enumerates. Output only the cleaned text."

## Measure

From the commit (B) or the start of the upload (A), per sample:

- `stt_ms`: final transcript received
- `first_ms`: first cleaned token received
- `total_ms`: full cleaned text received
- `lang_ok`: the text is in the language spoken, with no manual switch and nothing
  translated

Log every stage for every sample: connection open, last audio sent, final transcript,
first token, last token.

Report median and 90th percentile over the fifty samples, per pipeline and per language,
as a table, and print the raw transcript and the cleaned text of each sample for a quality
check. Write the run to `docs/results/speed-test-YYYY-MM-DD.md` with machine and network.

## Providers to compare

No provider is known to win, and none publishes time to first token, so compare. Start
with OpenAI, whose key is at hand. Add others behind the same trait as keys become
available; each needs its own account.

| Step | Candidate | Note |
| --- | --- | --- |
| Speech | OpenAI `gpt-live-transcribe` | Specified above. The most expensive option, about $0.017 a minute |
| Speech | ElevenLabs Scribe v2 Realtime | Claims a median of about 150 ms to a committed transcript; $0.22 an hour |
| Speech | Soniox `stt-rt-v6` | Cheapest streaming option at $0.12 an hour; language hints and a manual finalize message |
| Speech | AssemblyAI STT-3.7 Realtime | $0.30 an hour, billed while the socket is open |
| Speech | Deepgram Nova-3 multilingual | $0.0058 a minute |
| Cleanup | `gpt-6-luna`, reasoning off | Specified above |
| Cleanup | Groq `gpt-oss-20b`, Cerebras `gpt-oss-120b` | About 1,000 and 3,000 tokens a second; OpenAI-compatible endpoints, so only base URL, key and model change |
| Cleanup | Claude Haiku 5.5, Gemini 2.5 Flash-Lite | Similar price to the above |

## Flags

`--stt-provider`, `--stt-live-model`, `--stt-file-model`, `--llm-model`, `--llm-base-url`,
`--llm-key-env`, `--llm-deadline-ms`, `--reasoning-effort` (value `off` omits the field),
`--realtime-url`, `--runs`, `--mic`.

## Quality check

The owner dictates the same fifty samples into Wispr Flow. Put both outputs side by side
in the results file and mark which reads better.

## What the result decides

| Result | Decision |
| --- | --- |
| Median `total_ms` at or under 800, 90th percentile at or under 1200, language correct, formatting acceptable | Build dictation next |
| Speed passes, formatting is weak | Build dictation; budget time for prompt work and a test set |
| Median over 1200 on every variant | Keep Wispr Flow for dictation; decide whether any other part is worth building |

The thresholds are a proposal; the owner can change them.
