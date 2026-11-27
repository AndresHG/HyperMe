# Settings as a sheet inside Home: memory, and the pill shadow spike

Date: 27 November 2026. Machine: MacBook Pro (MacBookPro18,3, Apple M1 Pro, 16 GB), macOS 26.6.2.
Release builds (`cargo build --release -p hyperme-app`), started bare with
`HYPERME_NO_WALKTHROUGH=1`; nothing else using WebKit open. Physical footprint (`footprint -p`) of
the app and of every WebKit process started after it, read 30 to 40 s after launch. Master is
`c4e5b88`; the branch is `design/settings-sheet`. Master was given the same
`HYPERME_OPEN_SETTINGS` switch for this run only.

| State | Master (Settings is its own window) | Branch (Settings is a sheet in Home) |
| --- | --- | --- |
| Idle, nothing open | 60.3 MB (app 30, pill page 13, GPU 11, network 6.3) | 60.4 MB (same) |
| Home open | 107 to 111 MB (Home page 38 to 39) | 108 to 111 MB (Home page 39) |
| Home with Settings open | 133.6 to 134.5 MB (Home page 38 to 39, Settings page 23) | 136.4 to 137.5 MB (Home page with the sheet 64 to 65) |

Runs: idle 2 each, Home 3 to 4 each, Settings 3 each (runs where a WebKit helper was missed are
left out).

- The sheet costs about the same as the window it replaces (+2 to 3 MB): WebKit gave the old
  Settings window its own page process of 23 MB, and the sheet adds about 26 MB to Home's. It does
  not save memory, contrary to the plan's guess; its gain is native corners and one window fewer.
- Idle and Home alone are unchanged and inside the budgets (idle under 100 MB, Home open up to
  130 MB). Home with Settings open is over 130 MB in both, as before.

## Pill shadow spike

Not measured. The comparison (native `has_shadow` with `invalidateShadow` after each resize,
against the current pill, which has no shadow around the capsule) needs screen captures, and this
session had no Screen Recording permission (`screencapture -l` returned no image). The corner check
`scripts/check-corners.swift` is in place to run it: grant the terminal Screen Recording, start the
app with `HYPERME_SHOW_PILL=1 HYPERME_OPEN_HOME=home`, and run `swift scripts/check-corners.swift`.
Apple's documentation says `invalidateShadow` recomputes the shadow from the window's current
shape; the window-shadows crate notes no system shadow for transparent windows. The pill stays as
it is until the spike runs, in its own pull request.
