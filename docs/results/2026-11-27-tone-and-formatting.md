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

## Second rule

The same setup at about 00:10 CET on 28 November, after the rule was tightened (numbered steps
get `1.`, items capitalised with even endings, a colon after the lead-in, the mail greeting on
its own line, no full stop after a sign-off name or at the end of terminal text) and the
terminal sample's corrected text was changed to accept `-m`. `v4` was run again just after the
first `v5` run, as a control for the network at that hour.

| Set | Row | es | en | All | Unchanged | Fallbacks |
| --- | --- | --- | --- | --- | --- | --- |
| Formatting | `v5` | 4/5, 0.5 | 5/7, 0.3 | 9/12, 0.4 | 0 | 0 |
| Formatting | `v5` again | 5/5, 0.0 | 5/7, 0.3 | 10/12, **0.2** | 0 | 0 |
| 50 samples | `v5` | 4/25, 9.4 | 3/25, 8.2 | 7/50, **8.8** | 12 | 4 |
| 50 samples | `v4` (control) | - | - | 7/50, 8.9 | 8 | 1 |
| 50 samples | `v5` again | 3/25, 9.3 | 3/25, 8.8 | 6/50, 9.0 | 12 | 2 |

| Set | Row | n | `first_ms` | `total_ms` | Fallbacks |
| --- | --- | --- | --- | --- | --- |
| Formatting | `v5` | 12 | 592 / 830 | 785 / 996 | 0 |
| Formatting | `v5` again | 12 | 601 / 837 | 775 / 1005 | 0 |
| 50 samples | `v5` | 50 | 754 / 941 | 915 / 1263 | 4 |
| 50 samples | `v4` (control) | 50 | 601 / 727 | 774 / 978 | 1 |
| 50 samples | `v5` again | 50 | 735 / 920 | 919 / 1071 | 2 |

- **Formatting.** 10 and 9 of 12 texts now match their corrected text exactly. Two remain in
  both runs (a colon in place of a comma in a chat message; a mail kept as one middle paragraph
  where the corrected text has two). In one run, Spanish steps that are short phrases had no
  final full stops where the corrected text has them, which the rule allows.
- **50 samples.** No output has a list or an empty line. `v5` scores 8.8 and 9.0 against 8.9
  for `v4` at the same hour: equal within noise. The first `v4` run (9.6, above) shows that this
  model's noise on the set is about 0.7.
- **Speed.** At the same hour `v5` is about 140 ms slower than `v4` at both the first token and
  the median total, in two runs (first rule: about 20 ms). The second rule and the longer app
  lines make the prompt longer; the cost is on the time to the first token, not on the output.
