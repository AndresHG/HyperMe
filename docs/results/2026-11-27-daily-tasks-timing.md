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

## In the installed app (build 6bcc4b5, master with #156 to #160)

Installed with `scripts/bundle-macos.sh` and started in the background (`open -g`) at 11:40:36
local time, with the scheduler running, no connector account connected and no dictation.
Footprint (`footprint -p`) of `hyperme` and the three WebKit processes that started with it,
20 to 60 s after launch; CPU from `top -l 7 -s 2`.

| Process | Footprint | CPU |
| --- | --- | --- |
| `hyperme` | 33 MB | 0.20% |
| WebKit GPU | 15 MB | 0.00% |
| WebKit Networking | 6.4 MB | 0.00% |
| Pill WebContent (after drawing the morning card) | 18 MB | 0.15% |
| **All** | **72.4 MB** | **0.35%** |

Against the idle budget of 100 MB: passes. The CPU figure is the average of seven samples taken
in the first minute; the first sample of `top` counts from launch, so it overstates idle.

The morning card ran at launch: the day log's `morning_at` and the roll-over's catch-up carry
the same millisecond (11:40:47.173 and .172), so the card's job ran at once when the scheduler
started. The scheduler started 11 s after the process, because it starts after the dictation is
wired in setup; that delay is outside the presence-to-card path but is a delay before the card
at launch. Drawing time in the panel was not measured.

Not measured: Home open on Today (spec 98, AC-2). Opening Home brings the window to the front,
which would have taken focus from the owner's work. The app was no longer running by about
11:43 (no crash report written); idle CPU over 10 minutes was not measured either.

## Home open on Today (spec 98, AC-2), installed build 6bcc4b5

Home opened at 12:35:52 local time with `open ~/Applications/HyperMe.app` (the menu-bar icon
was hidden among others), on the Home page with the new Today block. Footprint of `hyperme` and
its four WebKit processes, sampled every 9 s; the owner's own task list and day.

| Time | `hyperme` | WebKit GPU | WebKit Networking | Pill WebContent | Home WebContent | **All** |
| --- | --- | --- | --- | --- | --- | --- |
| 12:36:04 | 38 MB | 14 MB | 6.4 MB | 13 MB | 23 MB | **94.4 MB** |
| 12:36:13 | 40 MB | 20 MB | 6.4 MB | 13 MB | 32 MB | **111.4 MB** |
| 12:36:22 | 40 MB | 23 MB | 6.4 MB | 13 MB | 41 MB | **123.4 MB** |
| 12:36:32 | 40 MB | 22 MB | 6.4 MB | 13 MB | 33 MB | **114.4 MB** |

Against AC-2 (Home open, any page, under 130 MB): passes, with 6.6 MB to spare at the peak.

Idle with Home never opened, same build, three samples from 12:32:21 to 12:32:42: `hyperme`
30 MB, GPU 11 MB, Networking 6.5 MB, pill 13 MB, **60.5 MB** in all, steady.
