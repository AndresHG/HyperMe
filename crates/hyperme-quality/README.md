# hyperme-quality

The quality harness (issue 99): replays saved dictations through the app's own state machine
(`hyperme_core::dictation::Engine`) and the providers in your config, then scores what would have
been pasted against your corrected texts. It is the measure for the phase 2 gate and for every
prompt change. The scores are flowbench's, so the numbers compare with Phase 0's
([dictionary results](../../docs/results/2026-11-26-dictionary.md)).

Run the commands from the repository root.

## A set

A set is a folder of dictations and your corrected texts. It holds private speech, so it lives
outside the repository or in a git-ignored folder: the tool refuses any other place.

| File | What |
| --- | --- |
| `es-01.wav`, `en-h12.wav` ... | WAV, 24 kHz, mono, 16-bit. The prefix (`es-`, `en-`) is the language |
| `gold.json` | `{"es-01": "the text as it should be pasted", ...}` |
| `names.txt` | Optional: internal names, one per line, in the dictionary format (`Zentrix: zen trix`) |
| `gold-draft.json` | Written by the tool: texts to correct and move into `gold.json` |
| `quality-texts-*.md` | Written by the tool: raw, pasted and gold text of each sample |

`spikes/flowbench/samples/` already has this layout: the 50 Phase 0 samples with their gold texts.

## Add dictations from the app's history

```bash
cargo run --release -p hyperme-quality -- import --set ~/hyperme-quality --days 7
```

Copies each done dictation of the last 7 days (all of them without `--days`) as `es-h<id>.wav` or
`en-h<id>.wav`, and adds the pasted text to `gold-draft.json`. Correct each text into what you
wanted and move it into `gold.json`. The history is read, never changed.

## Run

```bash
cargo run --release -p hyperme-quality -- run --set spikes/flowbench/samples --network "wired"
```

Each sample is played in real time, in 20 ms frames, as the microphone would send it, so 50
samples take about ten minutes. The keys are the app's, from the keychain; `SPEECHMATICS_API_KEY`
and `GROQ_API_KEY` (the provider name in capitals, then `_API_KEY`) take precedence when set.

| Option | |
| --- | --- |
| `--only es-01,en-02` | Only these samples |
| `--level medium` | Another cleanup level than the config's: `none`, `light` or `medium`, each with its own prompt |
| `--prompt v3` | A named prompt version instead of the level's (`v3`, `v4`, `m1` ...), for experiments |
| `--deadline-ms 1500` | Another cleanup deadline |
| `--names FILE` | A names list other than the set's `names.txt` |
| `--dictionary FILE` | A dictionary in the text format (`Zentrix: zen trix`), applied as in the app: substituted in the raw transcript and named in the cleanup prompt. Without it, no dictionary: the app's own dictionary store is not read |
| `--reuse-raw FILE` | Cleanup only: reuse the raw transcripts of an earlier run (below) |
| `--pace-ms N` | With `--reuse-raw`: wait this long between samples, for a provider's free-tier rate limit |
| `--config FILE` | Another config than the app's |
| `--network TEXT` | Written into the results file, such as `wired` or `Wi-Fi` |
| `--out FILE` | Where the results file goes; `--no-results` writes none |

It prints a line per sample and a table per language, writes the texts and any gold draft into
the set folder, and writes `docs/results/quality-YYYY-MM-DD.md` with numbers only.

Every live run also writes the raw transcripts, `{"es-01": "raw text", ...}`, to
`raw-YYYY-MM-DD.json` in the set folder. They are private like the audio and never go into the
repository.

## Cleanup-only replays

```bash
cargo run --release -p hyperme-quality -- run --set spikes/flowbench/samples \
  --reuse-raw spikes/flowbench/samples/raw-2026-11-27.json --prompt v4 --pace-ms 1000
```

A live run takes about ten minutes, costs speech minutes, and the transcripts change a little from
run to run, which hides small prompt differences. `--reuse-raw` sends each saved transcript
through the same engine at once, with no audio: the dictionary, the short-dictation rule, the
prompt, the cleanup deadline and the raw fallback are the app's, and only the cleanup call is
live. `stt_ms` is 0 and `total_ms` is the cleanup alone. The results file says the transcripts
were reused and from which file. Confirm a winner with one live run.

## The columns

- **Zero-edit:** pasted text equal to the corrected text apart from spacing.
- **Edits per 100 chars:** character edits to reach the corrected text, per 100 characters of it.
- **Names right:** names in the corrected texts that the pasted text has too. **Wrong names:**
  names in the pasted text where the corrected text has none.
- **Fillers left:** um, uh, eh, o sea, you know and similar in the pasted text (in the raw text).
- **Unchanged:** cleaned text identical to the raw transcript.
- **Fallbacks:** the raw transcript was pasted after the cleanup deadline or an error.
- **`stt_ms`, `total_ms`:** from key release, median / 90th percentile, as in the timing log.
