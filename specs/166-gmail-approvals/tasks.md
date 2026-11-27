# Slices: approvals for connector tools

Each slice is one pull request, test first, citing the AC ids it proves. Slice 1 settles the one
claim the design rests on (an "Always allow" lets later unattended pulls through) before any UI is
built.

| # | Slice | Size | Proves | Depends on |
| --- | --- | --- | --- | --- |
| 1 | **The protocol in the client, and the live check.** `HyperClient` and the Discover-then-Initialize connect; the approval loop on `call_tool_once` with the `Approver` trait in core (`Unattended` and scripted); form parsing; the `ConnectorError` variants; the fake server with elicitation. `google_live_check` gains `--url`, a terminal approver and `--unattended`. On the owner's account, against Gmail's `/mcp/v1`: approve with the broadest persistent choice, then make an unattended call 30 minutes later, and make a calendar call. Steps, counts, choices offered and timings go in `docs/results/` | M | AC-1, AC-2, AC-7, AC-8, AC-11, AC-15 | Spec approved; the owner's sign-in (asked first) |
| 2 | **States, alerts and the preset.** `SyncError::NeedsUser`; `ServerStatusStore` with its SQLite table and migration, precedence and clearing; "Needs your OK" alert once a day and the once-a-day pull; deny and admin-block handling; the `omittedContent` count; the Gmail endpoint per A-2; the Privacy line; rmcp held at `warn` in the log filter | M | AC-3, AC-5, AC-9, AC-12, AC-14, AC-17 | 1 |
| 3 | **The approval card.** Markdown to spans; the A-1 default; the card in the setup sheet's check and on Review from the alert; keys and the 5-minute wait; Ask again; the UX.md row and mockup | M | AC-4, AC-6, AC-10, AC-13, AC-16 | 2 |
| 4 | **Week check.** The gate: a week of scheduler mail pulls and morning cards on the owner's account, with the mail block filled or a visible reason, recorded in `docs/results/` | S | Gate | 3 |

If slice 1 finds that a persistent choice does not cover unattended calls, slices 2 and 3 stop and
the spec goes back to the owner (pre-mortem, row 1).
