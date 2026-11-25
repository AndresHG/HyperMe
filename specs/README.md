# specs/: what an L-sized change must do, agreed before it is built

A spec lives next to the code so that the pull request shows it, a coding agent reads it without
any tool, and tests can cite its acceptance criteria. Local agent state (`.omc/`) is deleted with
its worktree, so a spec is never left there.

## When a spec lives here

Only for L-sized changes (see `CONTRIBUTING.md`): a new part, a new provider or connector, a
change to what leaves the machine, or work that needs more than one pull request. M-sized changes
keep their acceptance criteria in the issue; S-sized changes need a repro and the expected
behaviour.

## Layout

```
specs/<issue>-<slug>/
  spec.md     status, problem, requirements, acceptance criteria (AC-n), non-goals,
              pre-mortem, decisions
  design.md   optional: architecture fit, failure modes, budgets, test plan
  tasks.md    optional: pull-request-sized slices, each mapped to its AC ids
```

`research-focus/` predates this layout and is kept as the record of the first interview.

## spec.md

- **Status:** `draft`, then `approved` (before any production code), then
  `implemented (PR #n)`.
- **Requirements** in EARS form: "When <trigger>, HyperMe shall <response>." or "While <state>,
  HyperMe shall ...". One behaviour per line.
- **Acceptance criteria** `AC-1`, `AC-2`, ...: each one observable, with numbers where a budget
  applies ("key release to cleaned text under 800 ms at the median").
- **Non-goals:** what this change will not do.
- **Pre-mortem:** "It is three months later and this failed. Why?" Each answer becomes a
  requirement, a test or a non-goal.
- **Decisions:** what was chosen, what was rejected and why, and who decided.

## Rules

- Tests cite the `AC-n` they prove, so a spec's coverage is a grep.
- Do not edit an implemented spec to follow later changes. Write a new spec.
- Never reference `.omc/` or any other untracked path from a committed file.
- No personal data, transcripts or employer details in a spec. Quote a user only with their
  agreement, and say so.
