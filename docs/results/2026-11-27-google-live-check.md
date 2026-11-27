# Google connector: live check (slice 1) and AC-14, 27 November 2026

Spec: [specs/97-google-connectors](../../specs/97-google-connectors/spec.md), slice 1 and AC-14.
Run on the owner's Mac (macOS, Apple Silicon), home network, with the owner's personal Google
account, using `cargo run -p hyperme-providers --example google_live_check`. Steps, yes or no,
counts and timings only: no event, mail, address, client id or other content is recorded.

## Client setup

| Step | Result |
| --- | --- |
| OAuth client made in personal mode (Desktop app, "Use this client for an AI-powered agent") | Yes. Personal mode makes its own project; it does not offer an existing one |
| User type shown by Google Auth Platform | "Personal". No Testing or In production status exists for it |
| APIs personal mode turned on by itself (link without `apis=`) | None of the Workspace APIs. Gmail API, Google Calendar API, Gmail MCP and Calendar MCP were turned on by hand |
| Client id and secret | Moved from the downloaded JSON into the Keychain by a script that prints nothing; file deleted |

## Sign-in and tokens

| Check | Result |
| --- | --- |
| Desktop client accepts the loopback redirect (`http://127.0.0.1:<port>/`, PKCE S256) | Yes |
| Refresh token returned (`access_type=offline`, `prompt=consent`) | Yes |
| `refresh_token_expires_in` in the token answer (sign of a 7-day "Testing" token) | No: the Testing check of R-10a did not trigger |
| Refresh | Yes, 91 to 176 ms over five runs |
| Revoke and Keychain removal (Disconnect) | Yes, 125 and 138 ms |
| Sign-in round trip, including the owner's clicks | 6.9 s (second sign-in) |

Whether the account stays signed in past 14 days is still to be seen: #119.

## MCP servers

| Check | Calendar | Gmail |
| --- | --- | --- |
| `initialize` (classic lifecycle) | No: JSON-RPC -32601 "Method not supported" | No: same |
| `server/discover` (stateless lifecycle, protocol 2026-07-28) | Yes | Yes |
| `tools/list` on `/mcp/v1` | 9 tools, 198 to 266 ms | 30 tools, 804 to 1691 ms |
| `tools/list` on `/mcp/v1self` | 8 tools (no `delete_event`) | 25 tools (no `send_message`, `reply`, `forward`, `apply_sensitive_*_label`) |
| Read call without MCP form elicitation | Works: `list_events` 331 to 504 ms | `search_threads` returns no threads: every result is listed under `omittedContent` with reason `DATA_PROTECTION_POLICY`, on both endpoints and with every `view` |
| Quota or cost per call visible in the answers or the console | No | No |

Google's guide says reads can need the user's approval under the account's data-protection
policy, asked through MCP form elicitation; a client that does not declare it gets the omitted
results above. HyperMe's client does not declare it yet, so the mail pull returns 0 threads.

### Tools seen (names and arguments only)

Calendar (`/mcp/v1`): `list_events` (calendarId, startTime, endTime, timeZone, pageSize,
pageToken, orderBy, fullText, eventType, eventTypeFilter), `get_event`, `list_calendars`,
`suggest_time`, `search_events`, `create_event`, `update_event`, `delete_event`,
`respond_to_event`. Read-only hints true for the five reads; `delete_event` destructive.

Gmail (`/mcp/v1`): `search_threads` (query, view, pageSize, pageToken, includeTrash),
`get_thread` (threadId, messageFormat), `get_message`, `list_labels`, `list_drafts`, `get_draft`,
`list_filters`, `create_draft`, `update_draft`, `send_message`, `reply`, `forward`, `label_*`,
`unlabel_*`, `update_message_labels`, `trash_*`, `untrash_*`, `mark_*_spam`, `unmark_*_spam`,
`apply_sensitive_*_label`, `create_label`, `update_label`, `create_filter`.

Answer shapes from the output schemas: `list_events` gives `{events: [Event], nextPageToken, ...}`
with `Event` holding id, summary, start and end (dateTime or date), conferenceUrl, htmlLink and
status. `search_threads` gives `{threads: [Thread], omittedContent, resultCountEstimate,
nextPageToken}` with `Thread` holding id and messages, and `Message` holding subject, sender,
snippet, internalDate, labelIds and, from `get_thread`, plaintextBody. Empty lists are left out
of the JSON.

## AC-14 (by hand, read only)

| Step | Result |
| --- | --- |
| Sign in | Yes |
| Pull today's calendar (today and the next 7 days) through the preset's mapping | Yes: 5 events mapped, 0 skipped, 267 ms |
| Pull inbox metadata through the preset's mapping | Ran without error; 0 threads, because of the omitted content above |
| Disconnect (revoke, Keychain item removed) | Yes, 138 ms; no refresh token left |
| Draft created | No (AC-12 waits for the owner) |

## Changes made from these findings

- The client tries the stateless lifecycle first and falls back to `initialize`.
- The Google preset points at `/mcp/v1self`, uses the real argument names (`startTime`,
  `endTime`, `threadId`, `view`) and answer paths (`events`, `threads`, `messages`), lists the
  real tools in its defaults, and forbids `reply` and `forward` with the sending tools.
  `tools_verified` is now true.
- A missing list in an answer counts as empty.
- Open: MCP form elicitation, needed for the mail pull and drafts on `/mcp/v1` (#166).
