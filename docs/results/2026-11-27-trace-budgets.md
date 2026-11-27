# Trace store budgets, 27 November 2026

Spec 182's two budgets, measured on the owner's Mac. Numbers only.

- AC-11 (R-19: traces may add at most 1 ms at p90 between key release and text ready): they add
  **50 to 57 µs**, about 5% of the limit.
- AC-10 (R-18: idle memory at most 2 MB above the build without traces, idle CPU near 0%): the
  app process goes from **29.0 to 30.0 MB**, and idle CPU is **0%**.

## Latency (AC-11)

### Setup

| Item | Value |
| --- | --- |
| Date | 2026-11-27, about 15:00 CET |
| Machine | MacBookPro18,3, Apple M1 Pro, 16 GB, macOS 26.6.2, on mains power |
| Commit | 5ec824f (branch `feat/182-budgets`) |
| Tool | `cargo test --release -p hyperme-core --test trace_overhead -- --ignored --nocapture` |
| Method | 2,000 dictations per engine through the real engine with instant fake providers, 100 at a time alternating between an engine with no subscriber and one with the trace layer, the timing sink and a trace collector. Time from key release to text pasted |
| Runs | 3 |

The providers answer at once, so these times are the engine's own work. With real providers, key release to
text is several hundred milliseconds, and the trace cost is the same few tens of microseconds. The
SQLite write runs on the trace store's own thread and is not in this path.

### Results

Microseconds from key release to text pasted.

| Run | Engine | p50 | p90 | p99 |
| --- | --- | --- | --- | --- |
| 1 | without traces | 45 | 98 | 350 |
| 1 | with traces | 63 | 153 | 740 |
| 2 | without traces | 49 | 109 | 505 |
| 2 | with traces | 67 | 159 | 642 |
| 3 | without traces | 49 | 102 | 576 |
| 3 | with traces | 67 | 159 | 686 |

p90 difference: 55, 50 and 57 µs. R-19 holds.

The same measure in a Linux cloud container gave 2 to 17 µs at p90. That machine is slower overall
(p90 about 230 µs without traces), so the trace share there is smaller.

## Idle memory and CPU (AC-10)

### Setup

| Item | Value |
| --- | --- |
| Date | 2026-11-27, 15:58 and 16:08 CET |
| Machine | MacBookPro18,3, Apple M1 Pro, 16 GB, macOS 26.6.2, on mains power |
| Before | release build of f5fc48a (master just before the trace work) |
| After | release build of f39bc75 (slices 1 to 4: spans, trace store, warnings file, developer exports compiled out) |
| Tool | `scripts/measure-idle.sh --seconds 600`, one run each, one after the other |
| Dictations | none (the owner chose idle only; traces grow only with dictations, a few hundred bytes each) |

No other HyperMe ran during either run (checked with `ps` and the macOS log).

### Results

| Process | Before | After |
| --- | --- | --- |
| HyperMe app | 29.0 MB | 30.0 MB |
| WebKit GPU | not counted | 8.2 MB |
| WebKit Networking | not counted | 6.3 MB |
| WebKit WebContent | not counted | 13.0 MB |
| Average idle CPU, all processes | 0.54% | 0% |

R-18 compares the app process, the one the trace store runs in: +1.0 MB, within 2 MB. Idle CPU is
0%.

In the before run the script lost track of the three WebKit helpers: they show no name and 0 MB, so
that run's total (29.0 MB) and its CPU figure leave them out. The trace store does not touch the web
views, so the comparison above uses the app process alone. The script also printed the worktree's
commit rather than the binary's. Both script bugs are tracked in issue 194.
