# Idle memory and CPU, hands-free build, 2026-11-27

- Machine: MacBookPro18,3, Apple M1 Pro, macOS 26.6.2
- Build: 442e2af, release, installed in ~/Applications/HyperMe.app, signed with the local identity
- Use before measuring: launched by the owner 41 s before the window opened; no dictation and no
  window opened during the window (footprint flat, no new WebContent process)
- Idle period: 15:12:38 to 15:22:44 local, 600 s, sampled every 30 s (21 samples)
- Tools: `footprint -p` (physical footprint) and `ps -o %cpu`, measured in place on the running
  app (not with `scripts/measure-idle.sh`, which needs a fresh launch)
- WebKit helpers matched by start time: GPU, Networking and WebContent started within 1 s of
  hyperme; no other WebContent process started during the window

| Process | Footprint, start | Footprint, end | Baseline 2026-11-26 |
| --- | --- | --- | --- |
| hyperme | 30 MB | 30 MB | 29.0 MB |
| com.apple.WebKit.GPU | 11 MB | 11 MB | 11.0 MB |
| com.apple.WebKit.Networking | 6.5 MB | 6.4 MB | 6.4 MB |
| com.apple.WebKit.WebContent | 13 MB | 13 MB | 13.0 MB |
| **All** | **60.5 MB** | **60.4 MB** | **59.4 MB** |

Average idle CPU of all processes: **0.04%** (one sample at 0.8% summed, the other 20 at 0%).
Under 100 MB and under 0.5%.

hyperme alone at the end: RSS 31.3 MB; `vmmap` physical footprint 30.3 MB, peak 33.6 MB.

## Largest categories in hyperme (`footprint -p`, dirty memory)

| Category | Dirty |
| --- | --- |
| MALLOC_SMALL | 12 MB |
| CG image | 8.0 MB |
| WebKit malloc | 2.3 MB |
| __DATA_DIRTY | 1.8 MB |
| MALLOC metadata | 1.1 MB |
| __DATA | 1.0 MB |
| untagged (VM_ALLOCATE) | 0.8 MB |
| page table | 0.7 MB |
| stack | 0.6 MB |

## Comparison with the earlier build

On 6a8c796, measured in place earlier the same day after dictation, the processes summed to
84 MB, plus a second WebContent process of 25 MB (109 MB), and hyperme alone was 40 MB. That
measurement ended after 2.5 minutes when the app was quit, so the second process was never
identified. 442e2af adds #185, which makes Settings and the walkthrough free their page process
when closed, as Home already did. This run saw no second WebContent process and no dictation, so
it does not show the after-dictation case that AC-19 asks for; that measurement is still to do.
