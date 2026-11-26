# Slices: Google account and the Connectors settings

Each slice is one pull request, test first, citing the AC ids it proves. Phases follow
[ROADMAP.md](../../docs/ROADMAP.md): Connectors work starts with phase 3, the Google connector with
phase 4. Slice 1 is research and can run earlier with the owner's yes.

| # | Slice | Size | Phase | Proves | Depends on |
| --- | --- | --- | --- | --- | --- |
| 1 | **Spike: Google MCP on the owner's account.** A throwaway CLI in `spikes/` that signs in with a personal-mode client over loopback and PKCE, calls `tools/list` and one read on each server, and records: does the agent-flag client accept a desktop loopback redirect, refresh-token life over 8 days, elicitation needed or not, real tool and argument names, quota units per call. Numbers and yes or no only in `docs/results/` | M | before 3 | Settles the design's unknowns | Owner's yes; G-1 |
| 2 | **OAuth module.** Loopback listener, PKCE, state, token exchange, refresh, revoke; tokens through the secrets trait; fake OAuth server for tests | M | 3 | AC-2, AC-3, AC-4 (refresh), AC-6 (revoke) | Spec approved |
| 3 | **Remote MCP client with accounts.** `rmcp` streamable HTTP with a bearer token per account, refresh on 401, request counting; fake MCP server | M | 3 | AC-1, AC-4, AC-10 | 2 |
| 4 | **Connector registry and presets as data.** TOML presets, custom servers, tool defaults (read, ask, off), allowlist enforcement for jobs | M | 3 | AC-1, AC-10 | 3 |
| 5 | **Settings → Connectors.** List page, Add connector sheet with Custom MCP server, account page with tool switches, Setup check rows, `hyperme://settings/connectors/<id>` | M | 3 | AC-13 | 4; shared with the GitHub connector |
| 6 | **Google preset and setup sheet.** The three steps, scope switches, labels for several accounts, Disconnect, Privacy block | M | 4 | AC-5, AC-6, AC-7, AC-14 (sign-in) | 5, 1 |
| 7 | **Calendar pull.** Mapping run on the #92 scheduler, `events` table, retention, sleep and battery rules; feeds Today, the morning card and meeting-now nudges | M | 4 | AC-8, AC-9, AC-14 (pull) | 6, #92 scheduler |
| 8 | **Mail pull.** Inbox metadata into `mail_meta`, bodies on demand only, injection test | M | 4 | AC-8, AC-9, AC-11 | 7 |
| 9 | **Drafts through the preview card.** `create_draft` behind Ask before, `gmail.compose` consent | S | 4 | AC-12 | 8, phase 3 preview card, G-2 |
| 10 | **Expired sign-in alert and quota back-off.** One alert card, account state, slow-down at 80% | S | 4 | AC-4 (alert), R-20 | 7 |

Docs that change with the slices: UX.md (Connectors mockups, with slice 5), PRD section 12 (G-1 to
G-3 once answered, with this spec), `CLAUDE.md` privacy boundary (with G-3).
