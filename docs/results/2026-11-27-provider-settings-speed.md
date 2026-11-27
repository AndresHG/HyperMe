# Provider settings speed check, 27 November 2026

AC-15 of [spec 96](../../specs/96-provider-settings/spec.md): the dictation path must be no
slower after provider settings (enum dispatch, the provider switch, and the speech and cleanup
fallback wrappers). The same 50 samples were replayed through the app's own state machine with
master before spec 96's slices 8 to 11 and master after them. The four runs alternated
before, after, after, before, so network drift hits both sides alike. Numbers only: the audio
and texts stay in the set folder on the owner's Mac.

**Result.** No slowdown can be measured. After the change, the median from key release to text
was 946 and 960 ms, against 939 and 1013 ms before. Paired by sample, the after runs were 23 ms
faster at the median (speech 11 ms faster). Two runs of the same build differ by up to 74 ms at
the median, so any effect of the change is far inside the network's noise. A 10 ms bound cannot
be resolved over Wi-Fi to cloud providers. The code adds one `match` per provider call, one
uncontended lock per cleanup, and no audio buffering unless a speech fallback is set.

## Setup

| Item | Value |
| --- | --- |
| Date | 2026-11-27, 22:16 to 22:51 CET |
| Machine | MacBookPro18,3, Apple M1 Pro, 16 GB, macOS 26.6.2 |
| Network | Wi-Fi (en0) |
| Samples | The 50 Phase 0 samples (25 es, 25 en, 457 s of audio), replayed in real time |
| Speech | Speechmatics `melia-1`, EU endpoint |
| Cleanup | OpenAI `gpt-6-luna`, reasoning effort `none`, deadline 1500 ms, `light` level |
| Before | master at 3247b0e (before #208) |
| After | master at 2703d7f (with #208, #210, #212 and #213), plus slice 12's replay change |
| Tool | `hyperme-quality run --config` with the same config file for both builds |

**Change of cleanup provider.** The recommended cleanup is Groq, but the owner's Groq free tier
hit its daily token limit (HTTP 429) during the first attempt that evening. Both builds were
therefore run with OpenAI cleanup. Only the difference between the builds matters for AC-15.
These totals do not compare with the Groq measurement of the same day (494 / 720 ms,
[cleanup levels](2026-11-27-cleanup-levels.md)), because OpenAI's first token comes about
0.5 s later.

The before build replays with the providers built directly. The after build builds them
through `any::speech_with_fallback` and `any::cleanup_with_fallback`, as the app does, so the
wrappers are in the timed path. No fallback was set.

## Results

Milliseconds from key release, median / 90th percentile, as the tool reports them.

| Run | Build | Done | `stt_ms` | `total_ms` | Cleanup fallbacks | Failed |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | before | 50 | 127 / 199 | 939 / 1249 | 2 | 0 |
| 2 | after | 50 | 127 / 201 | 960 / 1137 | 0 | 0 |
| 3 | after | 50 | 127 / 211 | 946 / 1173 | 0 | 0 |
| 4 | before | 45 | 162 / 280 | 1013 / 1432 | 0 | 5 |

Pooled by build, nearest-rank percentiles:

| Build | Dictations | `stt_ms` | `total_ms` |
| --- | --- | --- | --- |
| before | 95 | 139 / 225 | 969 / 1397 |
| after | 100 | 127 / 211 | 956 / 1172 |

Paired over the 45 samples done in all four runs (each sample's mean of the two after runs
minus its mean of the two before runs): the median difference is 11 ms less for `stt_ms` and
23 ms less for `total_ms`.

- **Run 4's failures** were Speechmatics refusing 5 Spanish sessions with "Concurrent Quota
  Exceeded". This is a provider limit, not a client error. Run 4's slower speech times may come
  from the same pressure on the account.
- **Run 1's two cleanup fallbacks** were the 1.5 s deadline: OpenAI was slower to answer for
  those two samples.

## Not measured

- **The timing log from daily use.** It needs a week of real dictations with the new build.
  Phase 1's gate collects it.
- **OpenAI streaming speech (slice 8) and a speech fallback in use.** These need their own run,
  with the speech provider set to `openai-speech`.
