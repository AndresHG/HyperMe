# Daily tasks: build times and the morning card's data path (spec 92, AC-18, AC-9), 27 November 2026

- Machine: MacBook Pro (MacBookPro18,3), Apple M1 Pro, macOS 26.6.2
- Build: `master` at cc0eb36 (#156 to #158 merged), release profile
- Network: not involved

## AC-18, R-38: Today and the morning card from 10 000 tasks (target under 20 ms each)

`cargo test --release -p hyperme-core --test brief -- --nocapture`, 21 runs, from a snapshot in
memory:

| Build | Median | Max | Verdict |
| --- | --- | --- | --- |
| Today | 0.93 ms | 1.00 ms | Passes |
| Morning card | 1.02 ms | 1.10 ms | Passes |

## AC-9, R-18: the Morning job from a store on disk (target under 150 ms to the card on screen)

`cargo test --release -p hyperme-app morning_from_10_000 -- --nocapture`, 21 runs: the whole
Morning job (today's rows from a SQLite file of 10 000 tasks, the events and mail caches, the
day log, the card model) and the card's words, without the panel.

| Step | Median | Max |
| --- | --- | --- |
| Morning job to the card's words | 5.75 ms | 6.40 ms |

The part left out is the pill panel being resized and the card drawn in its web view. That needs
the installed app; it is to be added here with the idle and Home memory once the build from
master is installed.
