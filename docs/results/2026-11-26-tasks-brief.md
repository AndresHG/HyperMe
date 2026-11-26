# Today and morning card build time, 26 November 2026

AC-18 and R-38 of [spec 92](../../specs/92-daily-tasks/spec.md): Today and the morning card
must each build in under 20 ms from a store of 10 000 tasks. This file covers the build time
only. The idle memory and wake-up half of AC-18 waits for the app (slice 12).

## Set-up

- Machine: MacBook Pro (MacBookPro18,3), Apple M1 Pro, 16 GB, macOS 26.6.2, on AC power.
- Toolchain: rustc 1.100.0, release profile.
- Network: none. The builders make no network call.
- Data: 10 000 synthetic tasks from `synthetic()` in `crates/hyperme-core/tests/brief.rs`.
  The mix is planned for today, rolled over, due today with a time, blocked by two tasks,
  done today, and planned for next week. All 10 000 go to the builders, not only today's
  rows, so this is a worst case for the snapshot slice 2 hands them.
- Command: `cargo test --release -p hyperme-core --test brief -- --nocapture`. Each run builds
  both cards 21 times.

## Results

| Run | Today median | Today max | Morning median | Morning max |
| --- | --- | --- | --- | --- |
| 1 | 0.816 ms | 1.337 ms | 0.963 ms | 1.360 ms |
| 2 | 0.667 ms | 0.800 ms | 0.746 ms | 0.817 ms |
| 3 | 0.669 ms | 0.933 ms | 0.748 ms | 0.837 ms |

The slowest build of all, 1.36 ms, is about 15 times faster than the 20 ms budget. The test
fails if any build takes 20 ms or more. It runs in release builds only and is ignored in debug
builds.
