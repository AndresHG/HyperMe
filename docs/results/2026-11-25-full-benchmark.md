# Full benchmark on 50 samples, 25 November 2026

Speechmatics `melia-1` (EU) with Groq cleanup passed the **speed and language** part of the gate
on the full set of 25 Spanish and 25 English samples. Median 429 ms and 90th percentile 595 ms from
key release to cleaned text, against 800 and 1200 ms. Language was right on 50 of 50. OpenAI
speech, with the same cleanup on the same samples and network, failed on both pipelines.

The **quality** part of the gate is not measured. There is no `samples/gold.json`, so there is
no zero-edit rate, and the owner has not compared the texts with Wispr Flow. The Phase 0 gate is
therefore not passed yet, and `CLAUDE.md` still says Phase 0.

## Setup

| Item | Value |
| --- | --- |
| Date | 2026-11-25, evening, Madrid |
| Machine | MacBook Pro M1 Pro, 16 GB, macOS 26.6.2 |
| Network | Wired, USB LAN (en13). TCP round trip: `eu.rt.speechmatics.com` 18 ms, `api.openai.com` 3 ms, `api.groq.com` 2 to 3 ms. Warm HTTPS round trip to Groq 157 to 158 ms |
| Samples | `es-01`..`es-25`, `en-01`..`en-25`: all 24 kHz, mono, 16-bit, 4.6 to 18.3 s, except `en-07` at 33.1 s (over the 20 s guideline; used in every earlier run, kept) |
| Cleanup | Groq `openai/gpt-oss-20b`, reasoning effort low, prompt `v1`, deadline 1500 ms |
| Tool | `spikes/flowbench` at master `6f4a7d6` |

The per-run files (runs 12 and 13) hold the transcripts and stay on the owner's Mac.

## Run 12: Speechmatics streaming

Milliseconds from key release, median / 90th percentile.

| Pipeline | Language | n | `stt_ms` | `first_ms` | `total_ms` | Language right | Fallbacks | Failed |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| B stream | es | 25 | 108 / 149 | 459 / 566 | 484 / 601 | 25/25 | 0 | 0 |
| B stream | en | 25 | 111 / 145 | 388 / 564 | 414 / 595 | 25/25 | 0 | 0 |
| B stream | all | 50 | 110 / 145 | 395 / 564 | **429 / 595** | 50/50 | 0 | 0 |

Gate (speed and language): **pass**. No Groq 429s.

## Run 13: OpenAI baseline

`gpt-transcribe` for upload (A), `gpt-live-transcribe` with `--stt-delay minimal` for streaming (B).

| Pipeline | Language | n | `stt_ms` | `first_ms` | `total_ms` | Language right | Fallbacks | Failed |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| A upload | es | 25 | 550 / 773 | 838 / 1072 | 871 / 1136 | 25/25 | 0 | 0 |
| A upload | en | 25 | 571 / 1299 | 779 / 1742 | 805 / 1777 | 25/25 | 0 | 0 |
| A upload | all | 50 | 561 / 927 | 820 / 1208 | 858 / 1275 | 50/50 | 0 | 0 |
| B stream | es | 25 | 662 / 745 | 911 / 1117 | 938 / 1140 | 25/25 | 0 | 0 |
| B stream | en | 25 | 664 / 721 | 823 / 997 | 862 / 1019 | 25/25 | 0 | 0 |
| B stream | all | 50 | 663 / 729 | 873 / 1027 | 914 / 1072 | 50/50 | 0 | 0 |

Gate: **fail** on both. A: median 858 > 800 and 90th percentile 1275 > 1200. B: median 914 > 800.
The gap is in speech: about 110 ms for Speechmatics against 560 to 660 ms for OpenAI, while the
TCP round trip to OpenAI (3 ms) is shorter than to Speechmatics (18 ms).

## Every approach tried today

Pipeline B unless noted, `total_ms` median / 90th percentile.

| Approach | Samples | Network | `total_ms` | Language right | Gate (speed, language) |
| --- | --- | --- | --- | --- | --- |
| OpenAI speech + OpenAI `gpt-6-luna` cleanup (run 1) | 38 | Wi-Fi | 2007 / 2362 | 38/38 | fail |
| OpenAI speech + Groq cleanup (run 2) | 38 | Wi-Fi | 1137 / 1330 | 38/38 | fail |
| OpenAI speech, delay minimal + Groq (run 3) | 38 | Wi-Fi | 1110 / 1290 | 38/38 | fail |
| OpenAI speech, delay minimal + Groq, B (run 13) | 50 | wired | 914 / 1072 | 50/50 | fail |
| OpenAI upload + Groq, A (run 13) | 50 | wired | 858 / 1275 | 50/50 | fail |
| Deepgram `nova-3` multi, EU + Groq (run 11) | 38 | wired | 449 / 617 | 36/38 | fail: language |
| Speechmatics `melia-1`, EU + Groq (run 10) | 38 | wired | 426 / 587 | 38/38 | pass |
| **Speechmatics `melia-1`, EU + Groq (run 12)** | **50** | wired | **429 / 595** | **50/50** | **pass** |

Cleanup prompt, replayed on the 38 transcripts of run 3 ([replay-2026-11-25.md](replay-2026-11-25.md)):
`v1` full text 374 / 519 ms, `v2` 392 / 795 ms. `v2` reasons about 4 times longer, failed once and
sometimes cuts real words, so `v1` stays.

## Distance from Wispr Flow

| Measure | HyperMe best (run 12) | Wispr Flow | Source |
| --- | --- | --- | --- |
| End of speech to text | 429 ms median, 595 ms p90, 50 samples | Under 700 ms, a vendor budget and p99 target, not a measurement | [wisprflow.ai/canto-1-results](https://wisprflow.ai/canto-1-results), [Baseten case study](https://www.baseten.co/resources/customers/wispr-flow/) |
| Same, measured by a third party | n/a | 1399 ms, reportedly measured by Aqua on an M3 Max | Unverified: search snippet, page returned 404 (research of 24 November) |
| Zero-edit rate | **Not measured**: no `gold.json` yet | 79.3%, Voice Arena Dictation Bench, US English only | [voicearena.com/dictation-bench](https://voicearena.com/dictation-bench) |

Our numbers and Wispr's are not like for like. Ours start at key release and end at cleaned
text on one machine and one network, without pasting. Wispr's is a stated target. On speed, the
measured median and 90th percentile are under Wispr's own budget. On quality, there is no number.

## Next

1. The owner corrects the draft gold texts into `samples/gold.json`. `--replay --gold` then gives
   a zero-edit rate and edits per 100 characters for each prompt.
2. The owner dictates a handful of the same sentences in Wispr Flow and compares the texts.
3. Only then can the Phase 0 gate be called passed and `CLAUDE.md` updated.
