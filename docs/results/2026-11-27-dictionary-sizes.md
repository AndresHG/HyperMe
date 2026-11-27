# Dictionary size and the cleanup (spec 93, D8, AC-7), 2026-11-27

Does a larger dictionary cost quality or speed? The owner's 50 samples were replayed through
the app's own pipeline (`hyperme-quality`, with slices D3, D6 and D7 merged) four times: with no
dictionary, the owner's learned list, and that list padded with invented names up to 150 and
1,000 entries. Numbers only; the texts, names and dictionaries stay on the owner's Mac.

## Setup

| Item | Value |
| --- | --- |
| Date | 2026-11-27, 20:45 to 20:58 CET |
| Machine | MacBookPro18,3, Apple M1 Pro, 16 GB, macOS 26.6.2 |
| Network | Home connection; speech not called |
| Speech | not called: the raw Speechmatics `melia-1` transcripts of Phase 0 run 12 were reused (`--reuse-raw raw-run12.json`) |
| Cleanup | Groq `openai/gpt-oss-20b`, reasoning low, level light, prompt `v4`, deadline 1500 ms, 2500 ms between samples (free tier) |
| Samples and gold | The 50 Phase 0 samples (25 es, 25 en, 457 s of audio) and the owner's corrected texts |
| Names scored | The 10 internal names of the owner's learned list (30 occurrences in the corrected texts) |
| Dictionaries | none; learned (10 entries); learned + 140 invented names (150, at the prompt cap); learned + 990 invented names (1,000, above the cap, so the prompt gets only the relevant entries) |
| Tool | `hyperme-quality` at `8157484` |

## Results

Edits per 100 characters of the corrected text, zero-edit samples, names right. `total_ms` is
milliseconds from key release to text, median / 90th percentile; with speech not called it is
the cleanup alone.

| Dictionary | Zero-edit | Edits per 100 chars (es / en / all) | Names right | Wrong names | Fallbacks | `total_ms` |
| --- | --- | --- | --- | --- | --- | --- |
| none | 5/50 | 8.5 / 5.8 / **7.2** | 9/30 | 0 | 0 | 350 / 501 |
| learned, 10 | 7/50 | 7.5 / 5.3 / **6.4** | **30/30** | 0 | 0 | 379 / 515 |
| 150 | 6/50 | 7.7 / 5.5 / **6.6** | **30/30** | 0 | 3 | 255 / 512 |
| 1,000 | 8/50 | 7.4 / 4.9 / **6.1** | **30/30** | 0 | 1 | 360 / 529 |

## What it shows

- **Names:** every dictionary gets all 30 names right, against 9 of 30 without one. Padding the
  list to 150 or 1,000 entries adds no wrong names.
- **Quality:** 6.1 to 6.6 edits per 100 characters with any dictionary, against 7.2 without, and
  no worse than the 6.5 of the earlier dictionary results
  ([2026-11-26](2026-11-26-dictionary.md)). AC-7's quality part passes.
- **Speed:** the cleanup's median stays between 350 and 380 ms, and its 90th percentile between
  501 and 529 ms, with 0, 10 and 1,000 entries. At 1,000 the relevance filter keeps the prompt
  short, so the size does not show. Adding the live speech step (`stt_ms` median 124 ms in the
  [2026-11-26 results](2026-11-26-dictionary.md)) keeps key release to text near 500 ms, well
  inside the 800 ms budget.
- **The 150 row** had 3 fallbacks to the raw text, and its median is low because those samples
  returned early. At the cap, every entry is in the prompt (about 450 more tokens), and Groq's
  free tier limits tokens per minute; the fallbacks look like that limit, not like the
  deadline. Not checked further: the tool does not log the reason for a fallback.

## Not measured

- **Groq cached tokens with the warm-up prefix:** `hyperme-quality` does not read the response's
  `usage` (`prompt_tokens_details.cached_tokens`), so whether the stable prefix is cached is still
  open. It needs the cleanup provider to report usage.
- **Speechmatics `melia-1` + `multi` + `additional_vocab`:** not run. The speech provider does not
  send `additional_vocab` yet, and spec 93 R-7 keeps the dictionary away from the speech provider
  in this version.
- **Live runs:** these are replays of saved transcripts. The speech step does not see the
  dictionary (R-7), so its timing does not depend on the dictionary's size.
