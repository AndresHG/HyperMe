# Pill shadow: native against none, and the corner check

Date: 28 November 2026. Machine: MacBook Pro (MacBookPro18,3, Apple M1 Pro, 16 GB), macOS 26.6.2.
Captures with `screencapture -l` (window with its shadow) through `scripts/check-corners.swift`,
debug builds started with `HYPERME_SHOW_PILL=1 HYPERME_OPEN_HOME=home HYPERME_PILL_DEMO=<state>`.

## What was compared

- **A, before:** the pill panel with `has_shadow(false)`: no shadow at all around the capsule.
- **B, native shadow:** `has_shadow(true)`, and `invalidateShadow` 40 ms and 220 ms after every
  resize, so macOS recomputes the shadow from the panel's pixels once the page has drawn the new
  size (Apple: the shadow is recomputed from the window's current shape when asked).
- A CSS shadow in a padded panel was not built: the transparent padding would take clicks meant
  for the app underneath (the pill sits just above the Dock), and the panel's size and position
  would change for every state. window-vibrancy was not needed: the pill is opaque.

## Result

| State | A: corner pixels (alpha) | B: corner pixels (alpha) | By eye |
| --- | --- | --- | --- |
| Done capsule, 182 × 32 | 0.00, 0.00, 0.00, 0.00 | 0.00, 0.00, 0.00, 0.00 | B: a soft shadow that follows the capsule, no square |
| Error capsule, 211 × 32 | 0.00 × 4 | 0.00 × 4 | Same |
| Bead, 22 × 32 panel | 0.00 × 4 | 0.00 × 4 | B: a jagged, stepped outline around the 14-point bead |
| Home window, 912 × 614 | 0.00 × 4 | 0.00 × 4 | Native corners and shadow (since #241) |

Every capture passes the corner check: nothing opaque or square at the corners of any window.

**Chosen:** the native shadow for the capsule, its states and the morning card, and none for the
bead (`pill::wants_shadow`). macOS builds the shadow from a low-resolution outline, which shows
as steps on a 14-point circle but not on the capsule.

## Memory (release build, `footprint` of the app and its WebKit processes)

| State | Run 1 | Run 2 |
| --- | --- | --- |
| Idle | 59.5 MB | 61.4 MB |
| Home open | 102.3 MB | 101.5 MB |
| Pill showing "done" | 65.5 MB | 66.4 MB |

The shadow is drawn by the window server, not in HyperMe's processes; the numbers match master's
of 27 November (idle 60.3 MB). The window server's own memory was not measured.

Not captured: the morning card (it needs the day's data); it takes the same path as the capsule.
