# Trace overhead on key release to text, 27 November 2026

Spec 182, AC-11 (R-19: traces may add at most 1 ms at the 90th percentile). Traces add about
**50 to 57 µs at p90** on the owner's Mac, about 5% of the limit. Numbers only.

## Setup

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

## Results

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

## Not measured here

AC-10 (idle memory and CPU before and after, `scripts/measure-idle.sh --after-dictations`) needs
HyperMe quit on the Mac and is recorded separately.
