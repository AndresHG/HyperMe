# Idle cost of dictionary learning (spec 93, AC-16), 2026-11-27

AC-16 asks that, with "Learn from my edits" on, idle CPU stays near 0% and idle memory under the
100 MB budget. The field watcher has no thread and no observer between watches (slice D7), so
idle should not change. This compares the D7 build with master just before the dictionary work.

## Setup

| Item | Value |
| --- | --- |
| Machine | MacBookPro18,3, Apple M1 Pro, macOS 26.6.2 |
| Builds | release; `ddfcf15` (D3 + D6 + D7, learning on by default) and `1757ba3` (master before them) |
| Launch | bare binary, `HYPERME_NO_WALKTHROUGH=1`, no dictation, no window opened |
| Samples | `ps -o %cpu` every 10 s for 180 s, starting 60 s after launch; `footprint -p` at the end |
| Load | Other sessions were compiling on the same Mac during both runs, so the CPU numbers are noisy |

## Result

| Build | Footprint (hyperme) | Idle CPU, mean of 18 samples | Range |
| --- | --- | --- | --- |
| master `1757ba3` | 20 MB | 0.76% | 0.4 to 1.1% |
| D7 `ddfcf15` | 20 MB | 0.56% | 0.1 to 1.2% |

A 300 s run of `scripts/measure-idle.sh` on `ddfcf15` gave 20.0 MB and 0.49%. The script did not
find the WebKit helpers in either run (they start only when a window opens in this build).

- **Memory:** unchanged, well under 100 MB.
- **CPU:** no difference that learning could explain: the D7 build is not above master. Both are
  above the 0.04% measured on 2026-11-27 for the hands-free build on a quiet Mac, so the
  baseline itself is worth a second look on a quiet machine. Not caused by this slice.
- **Not measured here:** CPU during 60 s of typing after a paste. It needs a real dictation and
  typing in an app with Accessibility allowed for HyperMe: issue #214.
