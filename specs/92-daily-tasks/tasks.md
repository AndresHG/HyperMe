# Phase 4 daily tasks: slices

Each slice is one pull request, starts from failing tests that cite its `AC-n`, and passes fmt,
clippy and tests before it merges. Open one issue per slice, labelled `phase-4` and its area,
once the spec is approved.

Two groups. **Core slices** (1 to 6) are pure Rust with tests and fakes: no UI, no network, no
change to the dictation path. They can run beside phases 1 and 2 if the owner pulls them forward
(decision T-5). **App slices** (7 to 12) need phase 3's dot, cards, Home and command loop, so
they wait for the phase 3 gate.

| # | Slice | Proves | Area | Size | Depends on |
| --- | --- | --- | --- | --- | --- |
| 1 | `hyperme-core::tasks`: types, day and due rules, the order of Today, roll-over rule, `TaskStore` trait. Adds `chrono` to the core | AC-7 (rules) | `area: core` | M | none |
| 2 | `hyperme-platform::task_store`: schema and migration, add, update, finish, blocked-by with the cycle check, ready, undo, mirror upsert, day log | AC-4, AC-5 (data), AC-6 (data), AC-7, AC-12 (data) | `area: core` | M | 1 |
| 3 | `hyperme-core::schedule`: the three schedule shapes, next run in a given zone, daylight saving, catch-up decision. **Good first issue** once 1 merges: pure functions, no Mac needed | AC-15, AC-14 (catch-up half) | `area: core` | S | 1 |
| 4 | `hyperme-core::scheduler`: the loop with `Clock` and `Presence` traits, failures and backoff, pause, seeded built-in jobs from config | AC-14, AC-16 | `area: core` | M | 3 |
| 5 | `hyperme-platform::presence` on macOS: wake, sleep, lock, unlock, clock and zone changes, seconds since input; a `presence_probe` example to check the notifications on the owner's Mac | R-15, R-34 (platform half) | `area: app` | M | 4 |
| 6 | `hyperme-core::brief`: Today, morning and evening card models from a snapshot; timing test at 10 000 synthetic tasks | AC-9 (data), AC-11, AC-12, AC-13, AC-18 (build time) | `area: core` | M | 1, 2 |
| 7 | App: morning and evening cards from the dot, with their keys; the "Updating…" parts | AC-8, AC-9, AC-11, AC-12 | `area: ui` | M | 4, 5, 6, phase 3 dot and cards |
| 8 | App: Today block on the Home page and a Today row in the quick sheet, with "new", "unblocked" and "Waiting on" | AC-5, AC-10 (UI) | `area: ui` | M | 6, phase 3 Home |
| 9 | Command tools `tasks_add`, `tasks_update`, `tasks_finish`, `tasks_today` in the phase 3 loop; done card with ⌘Z; chains; blocker matching; English and Spanish prompt examples | AC-1, AC-2, AC-3 | `area: core` | M | 2, phase 3 command loop |
| 10 | Nudges: a due time and a meeting a few minutes ahead, as alert cards | AC-10 | `area: ui` | S | 4, 7 |
| 11 | Sync mirrors: GitHub issues assigned to you become tasks; a closed issue resolves what it blocks. Calendar and mail follow with their connectors | AC-6 | `area: core` | M | 2, 4, phase 3 MCP client |
| 12 | Automations: prompt jobs with a tool allowlist, the Settings page, proposals into "Waiting for you"; then the measurements for AC-9 and AC-18 on the owner's Mac into `docs/results/` | AC-17, AC-9, AC-18 | `area: app` | M | 4, 9, 11 |

## Order and parallel work

```
1 ──┬── 2 ──┬── 6 ──┬── 7 ── 10
    │       │       └── 8
    │       ├── 9 (phase 3 loop)
    │       └── 11 (phase 3 MCP) ── 12
    └── 3 ── 4 ──┬── 5
                 └── (7, 10, 11, 12)
```

After slice 1, slices 2 and 3 can run at the same time, then 4 and 6, then 5. With two people or
two threads, the core group is about four pull requests deep.

## After the first version

Candidates, each its own issue, only if the owner asks after a week of use: recurring tasks
("every Monday"), the day as a Markdown note in the memory folder (R-29, second half), a week
view in Home, a soft cap on how many tasks are planned for one day.
