# Kickoff

The repository exists and the field research is done ([docs/RESEARCH.md](docs/RESEARCH.md)).
The only committed work is Phase 0, the speed test. What follows is decided once its
results are in.

## Before a session

- Start Claude Code from a terminal that has `OPENAI_API_KEY` in its environment, plus one
  variable for each extra provider to compare. Do not paste keys into the chat.
- Keep the repository private until the licence is chosen and the owner decides on a
  first release.

## Phase 0: speed test

Paste this as the first message of a fresh session:

> Read CLAUDE.md, docs/SPEED_TEST.md and docs/RESEARCH.md.
> Check each provider's current docs against docs/SPEED_TEST.md, then build or finish
> `spikes/flowbench` as specified. Show me the plan before writing code.
> When it builds, tell me how to record the fifty samples (25 Spanish, 25 English) and
> how to try it by voice. After I record them, run the pipelines, put the output beside
> Wispr Flow's on the same samples, write docs/results/speed-test-<date>.md and tell me
> whether the gate passed.

## After the result

Do not start another phase from this file. Read the latest file in `docs/results/` and
the table "What the result decides" in [docs/SPEED_TEST.md](docs/SPEED_TEST.md). The
next phase is the dictation core, as set out in [docs/ROADMAP.md](docs/ROADMAP.md) and
[specs/25-dictation-core](specs/25-dictation-core/spec.md).

## Habits that keep sessions useful

- One piece of work per session; start a new session for the next.
- Ask for the plan first, then the code.
- When a gate passes or a decision changes, have Claude Code update CLAUDE.md so the
  next session starts from the truth.
