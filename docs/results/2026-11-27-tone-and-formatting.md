# Tone per app and formatting, 27 November 2026

Does the `v5` prompt (issue 219: `v4` plus a rule for lists and paragraphs, and a style line per
app) format text as asked without hurting the owner's samples or slowing the cleanup? Measured
with `hyperme-quality` on the invented formatting set and on the owner's 50 samples, cleanup
only, with OpenAI (Groq's free daily limit was spent).

**Result.** On the formatting set `v5` halves the edits against `v4` (4.3 and 5.7 against 11.0
edits per 100 characters): it makes the lists, paragraphs and mail greetings that `v4` never
makes. On the 50 samples it adds no list or paragraph (none is wanted there) and scores 9.1
twice against 9.6 for `v4`. It costs about 20 ms at the first token and 20 to 60 ms at the
median total. `gpt-6-luna` with reasoning `none` is the fastest setting at equal quality;
`low` is no better and falls back more, and `gpt-6.1-terra` scores best but is too slow for the
1500 ms deadline.

## Setup

| Item | Value |
| --- | --- |
| Date | 2026-11-27, 23:50 to 2026-11-28, 00:00 CET |
| Machine | MacBookPro18,3, Apple M1 Pro, 16 GB, macOS 26.6.2 |
| Network | Wi-Fi (en0) |
| Cleanup | OpenAI `gpt-6-luna`, reasoning `none`, deadline 1500 ms, no fallback provider (rows below say when the model or effort differs) |
| Prompts | `v5` (`--level light`, this branch's default) and `v4` (`--prompt v4`) |
| Formatting set | `crates/hyperme-quality/sets/formatting`: 12 invented texts (5 es, 7 en) with an app each, 300 ms between samples |
| 50 samples | The Phase 0 samples and the owner's corrected texts; raw transcripts reused from the newest complete live run of 27 November (`raw-2026-11-27-4.json`, 50 samples; the later `-5` file holds only 45), 500 ms between samples |
| Dictionary | 50 samples only: the Phase 0 learned list (`--dictionary`). No names file was found, so names are not scored |
| Tool | `hyperme-quality`, this branch |

Replay rows time only the cleanup: `first_ms` is release to the first cleaned token, `total_ms`
to the whole text. A fallback pastes the raw text at the deadline and has no first token.

## Quality

Zero-edit count, then mean edits per 100 characters of the corrected text.

| Set | Row | es | en | All | Unchanged | Fallbacks |
| --- | --- | --- | --- | --- | --- | --- |
| Formatting | `v5` | 2/5, 2.6 | 2/7, 8.0 | 4/12, 5.7 | 0 | 1 |
| Formatting | `v5` again | 2/5, 3.1 | 2/7, 5.2 | 4/12, **4.3** | 0 | 0 |
| Formatting | `v4` | 2/5, 10.1 | 2/7, 11.5 | 4/12, 11.0 | 1 | 1 |
| 50 samples | `v5` | 3/25, 9.3 | 3/25, 9.0 | 6/50, 9.1 | 11 | 4 |
| 50 samples | `v5` again | 3/25, 9.3 | 2/25, 8.9 | 5/50, 9.1 | 11 | 0 |
| 50 samples | `v4` | 3/25, 9.6 | 3/25, 9.6 | 6/50, 9.6 | 7 | 0 |
| 50 samples | `v5`, reasoning `low` | 2/25, 9.3 | 3/25, 8.9 | 5/50, 9.1 | 8 | 7 |
| 50 samples | `v5`, `gpt-6.1-terra` | 5/25, 8.3 | 4/25, 7.6 | 9/50, **7.9** | 16 | 8 |

- **No invented formatting.** No `v5` output of the 50 samples has a list or an empty line, and
  none of the corrected texts has one.
- **Noise.** The two `v5` runs on the 50 samples gave 9.1 both times; the formatting set moved
  from 5.7 to 4.3, mostly one English sample that fell back in the first run.
- **What `v5` still gets wrong on the formatting set**: steps said with an order ("first, second,
  third"; "uno, dos, tres") become `- ` items, not `1.`; items in a short list lose or keep a
  final full stop unevenly and stay lower case; a Spanish mail keeps the greeting inside the
  first sentence; the terminal sample (iTerm2) gets "dash m" written as `-m` and a full stop
  added, which `v4` does too.

## Speed

Milliseconds, median / 90th percentile.

| Set | Row | n | `first_ms` | `total_ms` | Fallbacks |
| --- | --- | --- | --- | --- | --- |
| Formatting | `v5` | 12 | 726 / 974 | 859 / 1178 | 0 |
| Formatting | `v4` | 12 | 538 / 593 | 720 / 847 | 1 |
| 50 samples | `v5` | 50 | 603 / 738 | 824 / 1116 | 4 |
| 50 samples | `v5` again | 50 | 602 / 714 | 787 / 970 | 0 |
| 50 samples | `v4` | 50 | 582 / 763 | 767 / 941 | 0 |
| 50 samples | `v5`, reasoning `low` | 50 | 572 / 829 | 759 / 1501 | 7 |
| 50 samples | `v5`, `gpt-6.1-terra` | 50 | 747 / 1015 | 1090 / 1502 | 8 |

- **`v5` against `v4`.** On the 50 samples the longer prompt adds about 20 ms at the first
  token and 20 to 60 ms at the median total. On the formatting set the gap is larger (about
  140 ms at the median total), because formatted output is longer: more lines to stream.
- **Reasoning `low`** is no faster at the median and has the worst tail: 7 samples hit the
  deadline. `none` stays.
- **`gpt-6.1-terra`** scores 1.2 better but is 300 ms slower at the median and misses the
  deadline on 8 of 50. Not usable as the default at 1500 ms.
- **Against Groq.** These OpenAI totals (770 to 820 ms median) are about twice Groq
  `gpt-oss-20b`'s (380 ms median for `light`, `docs/results/2026-11-27-cleanup-levels.md`), so the absolute numbers here
  are not the app's default path.

## Caveats

- **Twelve invented texts** are a small set; they show the rule works, not how often real
  dictations need it.
- **One run each** for `v4`, `low` and `terra`; the `v5` repeats show about 0.0 to 1.4 of noise
  per set.
- **Fallbacks count as raw text** in the quality rows, so runs with more fallbacks score closer
  to the raw transcripts.
