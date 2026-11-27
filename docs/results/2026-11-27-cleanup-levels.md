# Cleanup levels, 27 November 2026

Which cleanup level should be the default: `light` (the `v4` prompt, today's behaviour) or a
`medium` prompt that also removes repetitions, fixes small grammar slips and applies spoken
formatting? Plan chunk P2-2, measured with `hyperme-quality` on the owner's 50 samples and
corrected texts.

**Result.** `medium` does not beat `light`. Every medium variant scored worse than light on
these samples: 9.1 to 10.7 edits per 100 characters, against 8.6 to 8.8 for light. The default
stays **`light`**. Medium keeps its best variant, `m2`, for people who want more cleanup, and
waits for the Style page.

**Against the gate (restated after issue 181).** The harness scores exactly as flowbench did, so
its numbers compare with Wispr Flow's 11.6 edits per 100 characters on the same 50 samples and
gold. `light` scores 9.1 live today and 6.3 on Phase 0's run 12 transcripts; both beat 11.6. All
dictionary names are right on run 12's speech (30 of 30), but not on today's (26 of 30): four
new mishearings are missing from the learned list. Light also wins on run 12's transcripts
(section "Second transcript set").

## Setup

| Item | Value |
| --- | --- |
| Date | 2026-11-27, 12:45 to 13:40 CET |
| Machine | MacBookPro18,3, Apple M1 Pro, 16 GB, macOS 26.6.2 |
| Network | Wi-Fi (en0) |
| Samples and gold | The 50 Phase 0 samples (25 es, 25 en, 457 s of audio) and the owner's corrected texts, unchanged since 25 November |
| Speech | Speechmatics `melia-1`, the app's config |
| Cleanup | Groq `openai/gpt-oss-20b`, reasoning low, deadline 1500 ms, the app's config |
| Dictionary | 10 terms, the Phase 0 learned list, substituted and in the prompt (`--dictionary`) |
| Short dictations | under 4 words skip the cleanup, as in the app |
| Tool | `hyperme-quality`, with this PR's prompts |

One live run first, with the app's current config (`light`). It gives the baseline, the first
end-to-end number for the app's own pipeline (P2-1), and the raw transcripts. Every other row
reuses those transcripts (`--reuse-raw`), so only the cleanup changes between rows. The first
replays at 1000 ms between samples hit Groq's free-tier rate limit: about a third of the Spanish
samples fell back to the raw text. Those runs were thrown away, and the rows below use 2500 ms.

## Quality

Zero-edit count, then mean edits per 100 characters of the corrected text. Names: internal
names in the corrected texts that the output has too (30 occurrences in 27 samples); no row put
a name where the corrected text has none.

| Row | Prompt | es | en | All | Names right | Unchanged | Fallbacks |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Live baseline, `light` | `v4` | 4/25, 9.5 | 3/25, 8.7 | **7/50, 9.1** | 26/30 | 30 | 0 |
| Replay, `none` (no model call) | none | 3/25, 9.1 | 3/25, 8.1 | 6/50, 8.6 | 26/30 | 38 | 0 |
| Replay, `light` | `v4` | 3/25, 9.3 | 3/25, 8.2 | 6/50, 8.8 | 26/30 | 31 | 1 |
| Replay, `light` again | `v4` | 3/25, 8.8 | 3/25, 8.3 | 6/50, **8.6** | 26/30 | 29 | 0 |
| Replay, `medium` | `m1` | 2/25, 10.4 | 3/25, 8.4 | 5/50, 9.4 | 26/30 | 21 | 1 |
| Replay, `medium` | `m2` | 3/25, 9.1 | 3/25, 9.1 | 6/50, 9.1 | 26/30 | 32 | 0 |
| Replay, `medium` | `m3` | 3/25, 10.8 | 3/25, 10.5 | 6/50, 10.7 | 23/30 | 27 | 0 |

- **Noise.** The same light prompt on the same transcripts gave 8.8 and 8.6, so differences
  of about 0.2 are noise.
- **`m1`** reordered words, dropped words the owner keeps, such as "like" and "pues eso",
  wrote small numbers as digits, and added line breaks. Removing repeated words and adding
  commas did help.
- **`m2`** keeps only what helped in `m1`: words said twice, commas, and formatting the speaker
  dictates. It is the best medium, but still about 0.4 worse than light.
- **`m3`** is `m2` with a firmer dictionary rule. It lost 3 names in English, because it put
  dictionary terms in place of the names that were said.
- **`none`** scored the same as light. Speechmatics already punctuates, and these samples have
  almost no fillers (1 in the raw text). Light's work, removing fillers and keeping only
  self-corrections, matters more in everyday speech than in this set. It is kept as the default.
- **Names.** 26 of 30 in every row except `m3`. The 4 misses are new mishearings that are not
  in the learned list and that no prompt turned into the name. The dictionary only covers forms
  it has seen, so the "every name right" gate needs the list to grow from corrections (spec 93's
  Fix a word in History), not a stronger prompt.

## Speed

Milliseconds from key release, median / 90th percentile. The live row is end to end. Replay rows
time only the cleanup (`stt_ms` is 0).

| Row | n | `stt_ms` | `total_ms` | Fallbacks |
| --- | --- | --- | --- | --- |
| Live baseline, `light`, es | 25 | 116 / 198 | 519 / 858 | 0 |
| Live baseline, `light`, en | 25 | 130 / 229 | 462 / 720 | 0 |
| Live baseline, `light`, all | 50 | 121 / 198 | 494 / 720 | 0 |
| Replay `light` (second), all | 50 | 0 | 380 / 545 | 0 |
| Replay `m2`, all | 50 | 0 | 316 / 510 | 0 |

The winner is the current config, so the live baseline is also the end-to-end check: median
494 ms and 90th percentile 720 ms from key release. That is under the 800 ms budget, with no
sample past the 1.5 s deadline. The worst Spanish samples (858 ms at the 90th percentile) came
on Wi-Fi.

## Second transcript set: Phase 0's run 12

Issue 181 asked why the harness gave 9.1 on "the Phase 0 transcripts" when the dictionary
results reported 6.5 for `v4` with the learned list. The cause is the file, not the scorer:

- flowbench overwrites `samples/transcripts.json` on every live run. The last live run that
  night was option 1 (`enhanced` with `additional_vocab`, at 01:02), so the file holds option 1's
  transcripts, not run 12's: 4 of its 50 texts match run 12. Option 1 scored 9.3 then, and the
  harness gives it 9.1.
- Run 12's raw transcripts are still in flowbench's private replay files. Scored with the
  harness's definitions, flowbench's own cleaned outputs from that replay give 6.48, the 6.5
  reported. So the scorers agree.
- Replaying run 12's raw transcripts through the harness (app engine, `v4`, learned list) gives
  **6.3, with 30 of 30 names**, matching Phase 0's 6.5 and 7 of 7 samples.

The harness needs no fix: its live runs write a new `raw-YYYY-MM-DD.json` each time and never
overwrite one. Cleanup-only replays of run 12's raw transcripts, paced at 2500 ms, no fallbacks:

| Row | Prompt | es | en | All | Names right | Unchanged |
| --- | --- | --- | --- | --- | --- | --- |
| Raw transcripts, no dictionary, scored directly | none | 8.6 | 6.5 | 7.5 | not scored | - |
| Replay, `none` | none | 2/25, 7.7 | 3/25, 5.5 | 5/50, 6.6 | 30/30 | 34 |
| Replay, `light` | `v4` | 2/25, 7.6 | 4/25, 5.0 | 6/50, **6.3** | 30/30 | 30 |
| Replay, `medium` | `m1` | 2/25, 7.5 | 5/25, 6.7 | 7/50, 7.1 | 30/30 | 24 |
| Replay, `medium` | `m2` | 2/25, 7.7 | 4/25, 7.5 | 6/50, 7.6 | 28/30 | 28 |

On run 12's speech light beats every medium variant again, and beats `none` by 0.3. The default
stays `light`.

## Caveats

- **Speech varies from run to run.** The raw transcripts alone score 9.1 today (Wi-Fi) and 7.5
  in Phase 0's run 12 (wired), with the same model and settings. Most of the gap between today's
  9.1 and Phase 0's 6.5 is in the speech, not in the cleanup.
- **One set, one speaker, mostly clean speech.** A level that removes more may pay off on
  speech with more hesitation. The owner's own use decides, through the Style page.
- **The learned list is optimistic**, as in the dictionary results: it was learned from these
  same clips.
