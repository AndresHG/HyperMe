# Local models for meeting notes, 26 November 2026

Can meeting notes (issue [#20](https://github.com/AndresHG/hyperme/issues/20), roadmap phase 5)
run on the owner's Mac for free, without loading it heavily? We tested local speech recognition
and local language models on the 50 flowbench samples.

**Result.** Yes for the live transcript. Apple's built-in on-device recognizer (SpeechAnalyzer,
macOS 26) is a little behind Wispr Flow's own speech engine (15% of words wrong against 13% in
Spanish, 15% against 11% in English), close to local Whisper turbo in Spanish. It costs almost
nothing in load: about 1 to 2.5% of one CPU core while transcribing, no measurable GPU use, and
no model to ship. The cleanup that turns gray text white is the
expensive part locally. A 4-billion-parameter local model (Qwen3 4B, 4-bit, MLX) improves the
text only a little, and it would keep the GPU busy about 15% of the time during a meeting.
Groq's `gpt-oss-20b` does that step better, with no local load, for well under a dollar a month
at 8 hours of meetings a week. NVIDIA Parakeet v3 is not usable for Spanish here: on
meeting-length audio it switches to English.

## Setup

| Item | Value |
| --- | --- |
| Date | 2026-11-26, after midnight, Madrid |
| Machine | MacBook Pro M1 Pro (10 CPU cores, 16 GB), macOS 26.6.2, on mains power |
| Other load | Another benchmark thread and several local servers were running. Load average about 3.8. GPU use while idle, read the same way as below, averaged 41% (window server and other apps) |
| Samples | `es-01`..`es-25`, `en-01`..`en-25` (7.6 minutes in all, about 9 s each). For a meeting-length test, the 25 clips of each language were joined into one file: 3.8 minutes of Spanish, 3.8 of English |
| Gold | `samples/gold.json`, the owner-reviewed texts of the [Wispr Flow comparison](2026-11-26-wispr-comparison.md) |
| Tools | `spikes/localmeet/` (this change). Python 3.12 with `mlx-whisper` 0.4.3, `parakeet-mlx` 0.7.0, `mlx-lm` 0.32.2, `jiwer` 4.0; a Swift command for Apple's recognizer. Every run at `nice 10` |

**Measures.**

- **WER**: word error rate against the gold text, lower case, punctuation removed. Lower is better.
- **RTF**: processing time divided by audio length. 0.05 means one hour of audio in 3 minutes.
- **CPU**: CPU seconds per second of audio, for the process and, for Apple, its
  `localspeechrecognition` service. 0.02 means 2% of one core. MLX work on the GPU is not
  included in this figure, which is why the GPU column is there.
- **GPU**: mean "Device Utilization %" from `ioreg` during the whole run, sampled every 0.5 s,
  model loading included. Compare it with the 41% idle reading.
- **Battery**: not measured. The Mac was on mains power, and `powermetrics` needs `sudo`.

## Speech: quality and load

Short clips are the 50 samples one by one. Long is the 3.8-minute file per language.

| Engine | WER es | WER en | WER long es | WER long en | RTF long | CPU | GPU | Memory |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Wispr Flow's speech engine (raw `asr` field of its history), for reference | 13.4% | 10.8% | | | | | | |
| **Apple SpeechAnalyzer** (`SpeechTranscriber`, on device) | 15.2% | 14.6% | 17.3% | 18.1% | 0.008 to 0.019 | 0.01 to 0.027 | none above idle (41%) | runs in a system service |
| Whisper large-v3-turbo (MLX) | 14.6% | 12.2% | 13.6% | 13.2% | 0.045 | 0.01 | 86% | 1.3 GB |
| Whisper small (MLX) | 47.3% | 19.8% | 23.7% | 13.9% | 0.027 | 0.02 to 0.04 | 82% | 1.3 GB |
| Parakeet TDT 0.6B v3 (MLX) | 14.1% | 19.2% | **97.7%** | 19.5% | 0.03 | 0.01 | 77% | 1.0 GB |
| Parakeet v3, streaming in 1 s chunks | | | **96.7%** | 32.0% | 0.68 | 0.17 | 92% | |

Notes:

- Apple's recognizer used the Neural Engine, not the GPU: GPU use stayed at the idle level for
  the whole run. Its Spanish and English models installed on demand through `AssetInventory`.
- Parakeet's long Spanish result is not a scoring error. On 3.8 minutes of Spanish, and again on
  30-second pieces of it, the output came back in English. Parakeet v3 detects the language
  itself and has no setting to fix it. One clip at a time it was fine (14.1%).
- Parakeet streaming through `parakeet-mlx` took 650 ms of work per second of audio (p95
  890 ms), with the GPU near full: too heavy for a live view.
- Whisper turbo is the most accurate local engine, but it keeps the GPU busy while it runs. At
  an RTF of 0.045, a one-hour meeting costs about 3 minutes of full GPU work if it is run once
  at the end.

## Cleanup and summary: local language models

Each sample's raw transcript went to the model with flowbench's strict prompt `v3` plus the
dictionary rule of `v4`, one request per sample, greedy decoding. Edits per 100 characters
is the character edit distance to the gold text, as in the [Wispr Flow comparison](2026-11-26-wispr-comparison.md).

| Model (MLX, 4-bit) | Input | Edits/100 raw | Edits/100 cleaned | Zero-edit raw / cleaned | Time per sample p50 / p90 | Output tokens/s | GPU | Memory |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Qwen3 4B Instruct 2507 | Apple | 10.4 | 9.8 | 2 / 4 of 50 | 1.24 s / 1.57 s | 24 | 83% | 2.3 GB |
| Qwen3 4B Instruct 2507 | Whisper turbo | 10.0 | 9.5 | 5 / 7 of 50 | 1.22 s / 1.75 s | 24 | 95% | 1.9 GB |
| Qwen3 1.7B (thinking off) | Apple | 10.4 | **11.9 (worse)** | 2 / 2 of 50 | 0.54 s / 0.75 s | 54 | 80% | 2.0 GB |
| For comparison: Groq `gpt-oss-20b`, prompt `v3` ([source](2026-11-26-wispr-comparison.md)) | Speechmatics | | 7.2 | 5 of 50 cleaned | | | none (cloud) | |

- The 4B model removes some errors but much less than `gpt-oss-20b`. The 1.7B model makes the
  text worse.
- Load during a meeting: people speak about 3.5 tokens of text a second. At 24 tokens a second,
  the 4B model would keep the GPU busy about 15% of the meeting (roughly 9 minutes per hour),
  in short bursts.
- A meeting summary of the 3.8-minute Spanish transcript (about 920 tokens in, 360 out) took
  10 s with the 4B model and 3 s with the 1.7B model. We did not judge summary quality beyond a
  quick read; the 4B summary was faithful in Spanish.

## Cost of the cheap cloud alternative

At 8 hours of meetings a week (about 35 hours a month), people speak about 9,000 words an
hour, so about 420,000 tokens of transcript a month. Cleaning it in batches with a
250-token prompt sends about 1.2 million tokens and returns 0.42 million. At Groq's
`gpt-oss-20b` prices of USD 0.075 in and 0.30 out per million tokens, that is about USD 0.22 a
month; at the launch prices of 0.10 and 0.50, about 0.33. The prices come from third-party
listings ([morphllm.com](https://www.morphllm.com/groq-pricing),
[flexprice.io](https://flexprice.io/pricing-index/groq)) and Groq's launch post
([groq.com](https://groq.com/blog/day-zero-support-for-openai-open-models)), not checked on
Groq's own pricing page. The live transcript stays free on device.

## Licences

| Piece | Licence | Source |
| --- | --- | --- |
| Apple Speech framework | Part of macOS, no model to ship | Apple |
| `parakeet-tdt-0.6b-v3` weights | CC-BY-4.0 (attribution) | [model card](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3) |
| WhisperKit | MIT | [LICENSE](https://github.com/argmaxinc/WhisperKit/blob/main/LICENSE) |
| FluidAudio SDK (Parakeet, diarization and VAD on the Neural Engine) | Apache-2.0; its CoreML diarization models are CC-BY-4.0, derived from pyannote | [LICENSE](https://github.com/FluidInference/FluidAudio/blob/main/LICENSE), [model card](https://huggingface.co/FluidInference/speaker-diarization-coreml) |
| sherpa-onnx | Apache-2.0 | [LICENSE](https://github.com/k2-fsa/sherpa-onnx/blob/master/LICENSE) |
| pyannote `speaker-diarization-community-1` | CC-BY-4.0, gated download | [model card](https://huggingface.co/pyannote/speaker-diarization-community-1) |

Model weights downloaded at run time are not crate dependencies, so `deny.toml` does not cover
them. CC-BY-4.0 asks for attribution in the app.

## Not checked

- Battery drain and package power: needs `sudo powermetrics` or a test on battery.
- Apple's live mode (volatile results while speaking): this test fed whole files. Its latency
  to first words and its accuracy on far-field meeting audio (laptop microphone, other people
  through the speakers) are not measured. The samples are close-microphone dictation by one
  speaker.
- Speaker diarization was researched, not benchmarked.
- Summary and ticket quality on a real meeting.
- Ollama: not installed; the local models ran through `mlx-lm`. Any server with an
  OpenAI-compatible API would plug into the same cleanup code.
