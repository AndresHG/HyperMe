# Kickoff

## One-time setup

From this folder, in your own terminal:

```bash
git init
git add .
git commit -m "Project brief, spec and speed-test plan"
gh repo create hyperme --private --source=. --push
claude
```

Keep the repository private until the licence is chosen and the first release gate passes.
Put your OpenAI key in the environment of the terminal you start Claude Code from
(`OPENAI_API_KEY`). Do not paste it into the chat.

## Session 1: plan the work on GitHub, then run Phase 0

Paste this as the first message:

> Read CLAUDE.md, docs/SPEC.md, docs/SPEED_TEST.md and docs/FIELD.md.
> 1. Ask me the open decisions listed in CLAUDE.md and record my answers there.
> 2. On GitHub, create milestones "Phase 0" to "Phase 3" with their gates in the
>    description, and issues for every Phase 0 task and every Phase 1 milestone in the
>    spec. Use `gh`.
> 3. Then do Phase 0: check the OpenAI realtime transcription and file transcription
>    docs against docs/SPEED_TEST.md, and build `spikes/flowbench` as specified.
>    Show me the plan before writing code.
> 4. When it builds, tell me how to record the ten samples. After I record them, run
>    both pipelines, write docs/results/speed-test-<date>.md and tell me whether the
>    gate passed.

## Session 2: start Phase 1

Start a fresh session once the Phase 0 result is written:

> Read CLAUDE.md, docs/SPEC.md and the latest file in docs/results/.
> Start Phase 1, milestones 1 and 2 only: a Tauri 2 app with a floating, non-activating
> pill that stays above fullscreen apps (NSPanel, no Dock icon), and a global hold-to-talk
> hotkey that shows recording state in the pill. Study how Handy and Thuki do it
> (docs/FIELD.md) before choosing an approach. Show me the plan first. Measure idle
> memory and CPU when it runs and compare with the budgets in CLAUDE.md.

Then continue one or two milestones per session, in the order given in the spec. Reuse
the speech provider code from `spikes/flowbench` for milestone 3.

## Habits that keep sessions useful

- One milestone or two per session; start a new session for the next.
- Ask for the plan first, then the code.
- When a gate passes or a decision changes, have Claude Code update CLAUDE.md so the
  next session starts from the truth.
