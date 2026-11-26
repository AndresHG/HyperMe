# Idle memory and CPU (AC-19), 2026-11-26

- Machine: MacBookPro18,3, Apple M1 Pro, macOS 26.6.2
- Build: d38c062, release
- Dictations before measuring: none
- Idle period: 600 s, CPU sampled every 10 s (60 samples)
- Tools: `footprint -p` (physical footprint) and `ps -o %cpu`

| Process | Footprint |
| --- | --- |
| hyperme | 29.0 MB |
| com.apple.WebKit.GPU | 11.0 MB |
| com.apple.WebKit.Networking | 6.4 MB |
| com.apple.WebKit.WebContent | 13.0 MB |
| **All** | **59.4 MB** |

Average idle CPU of all processes: **0%**. AC-19: under 100 MB and under 0.5%.

## What this baseline is, and is not

- Measured on the app after slice 11, with no keys in the keychain: the menu-bar icon, the hidden
  pill with its web view, and no dictation engine running (it starts once the keys are set). The
  walkthrough was kept closed for the measurement (`HYPERME_NO_WALKTHROUGH=1`).
- AC-19 itself asks for the numbers after 20 dictations, with keys and the engine running. Run
  `scripts/measure-idle.sh --after-dictations --out docs/results/<date>-idle-usage.md` for that.
- The empty shell measured 57 to 58 MB in slice 1 (`2026-11-26-shell-idle-memory.md`); this is
  1 to 2 MB more, within run-to-run noise of the WebContent process.
