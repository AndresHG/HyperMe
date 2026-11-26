# Dictionary tests, 26 November 2026

How to get the owner's internal names right. Seven of the 50 samples contain one of three
internal names, private project or product names that no speech engine can know. Two ways of
using a dictionary were tested against the owner's gold texts:

- **B:** replace sound-alikes after transcription, before the cleanup (`--vocab`, issue 34).
- **Option 1:** give the dictionary to Speechmatics as `additional_vocab`, which needs a
  single-language model.

**Result.** B with a list learned from the user's corrections is the best variant so far: 7 of
7 names right, 8 of 50 samples exact and 6.5 edits per 100 characters, with no cost in speed.
Option 1 also gets the names, but its speech is less accurate overall and it needs the language
before the dictation starts. It is not worth it.

## Setup

| Item | Value |
| --- | --- |
| Date | 2026-11-26, just after midnight, Madrid |
| Machine | MacBook Pro M1 Pro, 16 GB, macOS 26.6.2 |
| Network | Wired, USB LAN (en13), as in [the Wispr comparison](2026-11-26-wispr-comparison.md) |
| Samples and gold | The 50 samples and gold texts of [the Wispr comparison](2026-11-26-wispr-comparison.md) |
| Cleanup | Groq `openai/gpt-oss-20b`, reasoning effort low, deadline 1500 ms |
| Tool | `spikes/flowbench` at `4abe9ca` (branch `feat/vocab-replace`) |
| Replays | Run 12's raw Speechmatics `melia-1` transcripts through the cleanup again, paced for Groq's free tier. No 429s |

Two dictionaries, kept on the owner's Mac:

- **Blind:** sound-alikes written only from how the names sound in Spanish and English, without
  looking at the transcripts.
- **Learned:** the blind list plus the mishearings seen in these same 50 clips. It stands for a
  list built from the user's corrections.

## Quality

Zero-edit count, then mean edits per 100 characters of gold. A wrong substitution is a
dictionary term in the output where the gold text has none.

| Variant | es | en | All | All without `en-14` | Names right | Wrong substitutions |
| --- | --- | --- | --- | --- | --- | --- |
| Live `v3`, no dictionary (baseline) | 2/25, 8.6 | 3/25, 6.6 | 5/50, 7.6 | 5/49, 7.5 | 0/7 | 0 |
| Wispr Flow, with the owner's dictionary | 2/25, 11.5 | 2/25, 11.7 | 4/50, 11.6 | 4/49, 11.6 | 4/7 | 0 |
| (a) `v3` + substitution, blind | 2/25, 8.5 | 3/25, 6.8 | 5/50, 7.7 | 5/49, 7.5 | 1/7 | 0 |
| (b) `v3` + substitution, learned | 2/25, 7.8 | 6/25, 5.1 | **8/50, 6.5** | 8/49, 6.3 | **7/7** | 0 |
| (c) `v4` prompt only, blind | 2/25, 8.3 | 4/25, 6.0 | 6/50, 7.1 | 6/49, 7.0 | 1/7 | 0 |
| (d) `v4` + substitution, learned | 2/25, 7.3 | 5/25, 5.7 | 7/50, 6.5 | 7/49, 6.3 | 7/7 | 0 |
| Option 1, live: `enhanced` + `additional_vocab` (learned), `v3` | 3/25, 10.8 | 4/25, 7.7 | 7/50, 9.3 | 7/49, 9.0 | 7/7 | 0 |

Rows (a) to (d) are replays; the baseline, Wispr and option 1 are live. Substitution takes
microseconds, so it does not change the speed of a live run.

## Option 1: speed

Speechmatics `enhanced`, `--languages es` for the Spanish samples and `--languages en` for the
English ones, learned dictionary as `additional_vocab`, `--vocab-replace off`. Milliseconds from
key release, median / 90th percentile.

| Pipeline | Language | n | `stt_ms` | `first_ms` | `total_ms` | Language right | Fallbacks | Failed |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| B stream | es | 25 | 104 / 138 | 404 / 603 | 435 / 629 | 25/25 | 0 | 0 |
| B stream | en | 25 | 134 / 175 | 419 / 591 | 467 / 643 | 25/25 | 1 | 0 |
| B stream | all (combined from the per-sample lines) | 50 | 124 / 162 | 416 / 603 | 445 / 629 | 50/50 | 1 | 0 |

- Gate (speed and language): es pass (median 435, 90th percentile 629), en pass (median 467,
  90th percentile 643). Combined, it would pass too: 1 fallback in 50 is within one in ten.
- The fallback: the cleanup of one English sample missed its 1500 ms deadline, and the raw text
  was used.
- Speechmatics accepted `additional_vocab` with `enhanced` and one language: no error, no
  warning.

## Caveats

- **The learned list is optimistic.** It was learned from the same 50 clips it was tested on.
  A list learned from real corrections would start empty and grow.
- **Two common Spanish words in the learned list would cause wrong substitutions in real
  use.** On these clips they appear only where the gold has the internal name, so the count of
  wrong substitutions is 0 here. A product should learn a substitution only from a user
  correction, and warn when the heard form is a common word.
- **Option 1 was given the language.** Each sample was run with its known language; the product
  would not know it before the dictation.
- **No session-start figure.** flowbench does not report the time from opening the session to
  Speechmatics' `RecognitionStarted`. Speechmatics warns that a dictionary can delay the start
  by up to 15 s. No session here was slow to start or failed.

## Conclusion

- Use B: substitution after transcription, with a list learned from the user's corrections.
  The blind list helps little (1 of 7), because the engines mishear the names in ways nobody
  would guess in advance.
- Option 1 is not worth it: it gets the names, but the `enhanced` model mishears more of
  everything else (9.3 against 7.6 edits per 100 characters), and it needs the language up
  front.
- `v4`'s dictionary rule adds little on top of substitution, (d) against (b).
