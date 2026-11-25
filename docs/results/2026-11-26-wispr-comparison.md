# Quality against Wispr Flow, 25 to 26 November 2026

The same 50 samples went through Wispr Flow and through flowbench, and both were scored against
the owner's gold texts.

**Result.** With the strict cleanup prompt `v3`, our live pipeline (Speechmatics `melia-1` EU,
then Groq `openai/gpt-oss-20b`) needs fewer edits than Wispr Flow: 7.6 against 11.6 character
edits per 100 characters. It also finishes sooner. On exact matches the two are level: 5 of 50
for us, 4 of 50 for Wispr. Wispr gets the owner's internal names right and we do not.

**Gate.** Phase 0 is not passed. Speed and language pass. On quality, exact matches are still
rare for both. Most of our remaining distance comes from the speech engine, not the cleanup.

## Setup

| Item | Value |
| --- | --- |
| Date | 2026-11-25, evening, to 2026-11-26, just after midnight, Madrid |
| Machine | MacBook Pro M1 Pro, 16 GB, macOS 26.6.2 |
| Network | Wired, USB LAN (en13). TCP round trip: `eu.rt.speechmatics.com` 18 ms, `api.groq.com` 2 ms. Cold HTTPS request to Groq 180 ms, warm round trip 151 ms |
| Samples | `es-01`..`es-25`, `en-01`..`en-25`, the set of [the full benchmark](2026-11-25-full-benchmark.md) |
| Cleanup | Groq `openai/gpt-oss-20b`, reasoning effort low, deadline 1500 ms |
| Tool | `spikes/flowbench` at `21b8d53` (branch `feat/vocab-and-strict-cleanup`) for the replays and the live `v3` run |
| Wispr Flow | Version 1.6.1619. Each clip was played into the BlackHole 2ch virtual device with sox. Dictation was started and stopped with Wispr's `wispr-flow://start-hands-free` and `stop-hands-free` links, and the results were read afterwards from Wispr's local history. Wispr used the owner's own dictionary and settings |
| Gold | One text per sample: what the owner would send if he typed it. Drafted from the raw Speechmatics and OpenAI transcripts (not from any cleaned output), then reviewed by the owner. Fifteen doubtful samples were flagged and checked by ear; `en-14` is kept, but its audio is poor |

**Scoring.** Both texts have their spacing collapsed. The score is the character edit distance to
the gold text: zero-edit means the output needs no change at all, punctuation and capitals
included. Edits per 100 characters is the mean distance per 100 characters of gold. Transcripts,
gold texts and Wispr's output stay on the owner's Mac.

## Quality

Zero-edit count, then mean edits per 100 characters.

| Variant | es | en | All | All without `en-14` |
| --- | --- | --- | --- | --- |
| `v1` live (run 12 of the full benchmark) | 1/25, 10.4 | 2/25, 14.1 | 3/50 (6%), 12.3 | 3/49, 12.2 |
| `v1` replay on run 12's raw transcripts | 2/25, 10.0 | 2/25, 14.0 | 4/50 (8%), 12.0 | 4/49, 12.0 |
| `v3` replay | 2/25, 8.7 | 3/25, 5.8 | 5/50 (10%), 7.2 | 5/49, 7.1 |
| `v3` replay, dictionary in the prompt | 2/25, 8.3 | 3/25, 6.1 | 5/50 (10%), 7.2 | 5/49, 7.0 |
| **`v3` live**, Speechmatics, no dictionary | 2/25, 8.6 | 3/25, 6.6 | **5/50 (10%), 7.6** | 5/49, 7.5 |
| **Wispr Flow**, formatted output | 2/25, 11.5 | 2/25, 11.7 | **4/50 (8%), 11.6** | 4/49, 11.6 |
| Wispr Flow, speech text only (`asrText`) | 4/25, 11.2 | 4/25, 7.6 | 8/50 (16%), 9.4 | 8/49, 9.3 |

Notes:

- Replays run the cleanup again on run 12's raw Speechmatics transcripts, so the speech part is
  the same. They were paced in batches of 8 to stay inside Groq's free-tier token limit, with no
  429s.
- Wispr's speech text alone scores better than its formatted output. Its formatting step costs
  edits on these samples.
- Context only: [Voice Arena](https://voicearena.com/dictation-bench) reports 79.3% for Wispr
  Flow. That benchmark scores differently, so its figure does not compare with the zero-edit
  rates here.

## Live `v3` run: speed

Milliseconds from key release, median / 90th percentile.

| Pipeline | Language | n | `stt_ms` | `first_ms` | `total_ms` | Language right | Fallbacks | Failed |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| B stream | es | 25 | 111 / 148 | 414 / 539 | 447 / 588 | 25/25 | 0 | 0 |
| B stream | en | 25 | 119 / 144 | 412 / 545 | 442 / 570 | 25/25 | 0 | 0 |
| B stream | all | 50 | 115 / 144 | 413 / 539 | **444 / 570** | 50/50 | 0 | 0 |

Gate (speed and language): **pass**. No Groq 429s.

## Wispr Flow: latency and features

| Measure | es | en | All |
| --- | --- | --- | --- |
| `e2eLatency`, median / 90th percentile, ms | 548 / 684 | 520 / 673 | 529 / 680 |

- `e2eLatency` is Wispr's own figure from its local history. Its start point may not be key
  release, so it compares only loosely with our `total_ms`.
- Dictionary replacements were applied on 10 of 50 clips. The fallback speech engine was used
  on 0.

## Internal names

Seven samples contain one of three internal names, all private project or product names that no
speech engine can know.

| Variant | Right |
| --- | --- |
| `v3` replay | 0 of 7 |
| `v3` replay, dictionary in the prompt | 1 of 7 |
| `v3` live | 0 of 7 |
| Wispr Flow, with the owner's dictionary | 4 of 7 |

The dictionary in the prompt barely helps. `v3` is told to change as little as possible, and the
cleanup cannot tell which misheard word stands for which name. The names have to be handled on
the speech side.

## Main causes of the remaining edits

**Our live `v3` run** (45 samples not exact):

- Misheard by the speech engine: about 21 samples, the largest group.
- Internal names: 7.
- Punctuation or capitals only: 6.
- Words lost at speech recognition, such as a final word or a repeated phrase: 4.
- Fillers left, because `v3` is strict: 2.
- Light rewording by the cleanup: 3.
- A spoken false start in the other language was dropped instead of written: 1.

**Wispr Flow** (46 samples not exact):

- It rewords more than `v3`, and on 2 samples it changes the meaning.
- It drops deliberate repetitions.
- It applies the dictionary where it does not belong (a common word replaced by a product name).
- It keeps some self-corrections and a false start in the other language.
- It also mishears some words.

## Gold bias

The gold texts were drafted from the Speechmatics and OpenAI transcripts and then reviewed by the
owner. Where those engines agreed on a wording, the gold followed it, so the gold may favour
their wording over Wispr's. On two samples (`es-23`, `es-09`) Wispr's reading is plausible and
could be the right one. They are to be checked by ear.

## `--vocab` with `melia-1`

The live run with `--vocab` failed on all 50 samples. Speechmatics refused `additional_vocab`
with `melia-1` and `language: multi`: "additional properties 'additional_vocab' not allowed". It
sent no warning before refusing.

The refused sessions then counted against the concurrent quota, so every later session failed
with "Concurrent Quota Exceeded" until it cleared a few minutes later. Since no session started,
there is no session-start time with a dictionary.

flowbench now sends `additional_vocab` only to single-language models, and closes the socket
after a refusal or a failure (`91313ed`, issue 31).

## Next steps

1. Speech-side vocabulary. Either a single-language Speechmatics model with `additional_vocab`,
   which needs the language before the session starts, or a lexicon step that maps known
   mishearings to the dictionary terms before or inside the cleanup.
2. The punctuation-only misses (6 samples): look at whether a small rule or prompt change closes
   them without rewording.
3. Recheck `es-23` and `es-09` in the gold texts by ear, then score again.
