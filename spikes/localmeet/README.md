# localmeet

A spike for the meeting note-taker ([#20](https://github.com/AndresHG/hyperme/issues/20)): how
well do local speech and language models work on the owner's Mac, and how much load do they
put on it? Results: [`docs/results/2026-11-26-local-meetings.md`](../../docs/results/2026-11-26-local-meetings.md).

It reads the flowbench samples and gold texts from `../flowbench/samples/` (git-ignored) and
writes transcripts to `out/` (git-ignored). Only the numbers it prints belong in `docs/results/`.
It calls no paid API and needs no key. Models download from Hugging Face on first use
(about 1.6 GB for Whisper turbo, 1.2 GB for Parakeet, 2.3 GB for Qwen3 4B).

## Set up

```bash
uv venv -p 3.12 && uv pip install mlx-whisper parakeet-mlx mlx-lm jiwer psutil soundfile scipy
```

```bash
xcrun swiftc -parse-as-library -O apple/main.swift -o apple/applestt
```

Apple's recognizer needs macOS 26. The first run installs its Spanish and English models.

## Run

Run from this folder. `gpu.sh` samples GPU use every 0.5 s into a file until it is stopped.

```bash
./gpu.sh out/gpu-apple.txt & P=$!; nice -n 10 .venv/bin/python apple_bench.py; kill $P
```

```bash
nice -n 10 .venv/bin/python stt_bench.py whisper-turbo
```

`stt_bench.py` also takes `whisper-small`, `parakeet` and `parakeet-stream`. Then the cleanup
and summary, on the raw text of one engine:

```bash
nice -n 10 .venv/bin/python llm_bench.py mlx-community/Qwen3-4B-Instruct-2507-4bit apple
```

Python is used here because the MLX model ports are Python packages. The product stays Rust
(`CLAUDE.md`); this folder is a measurement tool only.
