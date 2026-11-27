# Diagnostics and Report a problem: live checks (spec 200, slice 6)

- **Date:** 27 November 2026, evening (Europe/Madrid).
- **Machine:** MacBookPro18,3, Apple M1 Pro, 16 GB, macOS 26.6.2.
- **Build:** master `dd57fd9`, release (`cargo build --release`). It ran as a bare binary, not an `.app` bundle.
- **Network:** not used. Nothing in these checks makes a request.

## R-20 and AC-11: read and build times

These ran on a copy of the owner's history store, taken with `sqlite3 .backup`, through a throwaway example that calls the same functions as the app commands. Each step ran 20 times.

| Step | What it does | Median | Max |
| --- | --- | --- | --- |
| Open Diagnostics | `traces_since(7 days, 5000)`, rows, `diagnostics::compute` | 0.21 ms | 0.87 ms |
| Build the bundle | the above, plus 50 traces, 200 warning lines, config, crash files, `report::build` and `report::issue_url` | 3.68 ms | 6.72 ms |

- Traces in the 7-day window: 10.
- Bundle: 134,652 bytes, not trimmed. Nearly all of it is two macOS crash reports for HyperMe. Without them it is 3,513 bytes. There were 0 HyperMe crash files.
- Issue link: 4,380 characters, under the 6,000 cap (R-15a).

Both steps are far under the 1-second budget of R-20. The store is small, though: 10 traces against the 5,000-trace limit.

## AC-11: idle with Settings closed

`scripts/measure-idle.sh` ran for 600 s with 60 samples while the new build was idle and Settings was closed.

| Measure | Value |
| --- | --- |
| Script's reading (the `hyperme` process only) | 30.0 MB, 0% CPU |
| RSS halfway through, all processes | hyperme 56.9 MB, plus WebKit GPU 10.1, Networking 6.9 and WebContent 3.9 MB, about 77.8 MB in all |
| Footprint of the running build, about a minute after launch | 30 + 11 + 6.3 + 13 MB, about 60 MB |
| CPU in every process | 0.0% |

- The budget is under 100 MB idle and near 0% CPU. Footprint holds it.
- RSS counts shared pages, and a later sample of it reached about 107 MB. So RSS is not the measure to hold the budget to; #194 tracks how the script should count.
- The script still misses the WebKit helpers (#194). Their parent is launchd, not HyperMe, so a parent filter finds none. They were matched by start time instead.

## AC-12: a real issue from a bundle

Pending: the owner builds a report in the app, reads it, and opens an issue with it.
