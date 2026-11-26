# Empty shell idle memory (slice 1 budget check), 26 November 2026

The Tauri 2 menu-bar app from slice 1: the menu-bar icon and menu, and the pill as a hidden
non-activating panel with its web view loaded. No dictation code is wired yet. Release build,
run from the command line (not a signed bundle), measured after 30 and 50 seconds idle.

- Machine: MacBook Pro (MacBookPro18,3), Apple M1 Pro, macOS 26.6.2
- Tools: `footprint -p <pid>` (physical footprint, what Activity Monitor shows as Memory) and
  `ps -o rss`; CPU from `top -l 6 -s 2`
- Network: not involved

| Process | Footprint | RSS |
| --- | --- | --- |
| `hyperme` | 29 to 30 MB | 75 MB |
| WebKit WebContent | 11 MB | 25 MB |
| WebKit GPU | 10 to 11 MB | 18 MB |
| WebKit Networking | 6.4 MB | 11 MB |
| **All four** | **57 to 58 MB** | 129 MB |

RSS counts shared system frameworks in every process, so the footprint is the figure to compare
with the budgets. Idle CPU of all four processes was 0.0% in every sample over 12 seconds.

design.md asks to stop and decide if the empty app is over 60 MB. It is at 57 to 58 MB: under the
line, with little room. The largest single item in `hyperme` is an 8 MB image region (likely the
512 px window icon decoded by AppKit), worth trimming later. The full-app budget (AC-19) is
under 100 MB after 20 dictations.
