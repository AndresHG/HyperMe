# Design: Google account and the Connectors settings

Companion to [spec.md](spec.md). Draft, 26 November 2026.

## Where it fits

```
Settings → Connectors (web view, no logic)
        |  commands / events
        v
+--------------------------------------------------------------------------+
| Rust core                                                                |
|  connectors::registry   presets (data) + user's custom servers           |
|  connectors::oauth      loopback + PKCE, refresh, revoke (any provider)  |
|  secrets (trait)        macOS Keychain; fake in tests                    |
|  mcp (rmcp)             streamable HTTP + stdio, bearer token per account|
|  sync                   pull jobs on the #92 scheduler, preset mappings  |
|  store (SQLite)         events, mail_meta, request counts                |
+--------------------------------------------------------------------------+
        |                                   |
        v                                   v
 accounts.google.com (OAuth)      gmailmcp / calendarmcp.googleapis.com
```

Nothing here is Google code. `oauth` speaks plain OAuth 2.0 for native apps
([RFC 8252](https://www.rfc-editor.org/rfc/rfc8252)), so GitHub, Linear or any MCP server that
needs OAuth uses the same module. The Google preset is a TOML file.

## The Google preset (data)

```toml
id = "google"
name = "Google"
kind = "oauth-mcp"
auth_url = "https://accounts.google.com/o/oauth2/v2/auth"
token_url = "https://oauth2.googleapis.com/token"
revoke_url = "https://oauth2.googleapis.com/revoke"
extra_auth_params = { access_type = "offline", prompt = "consent" }

[[servers]]
id = "calendar"
url = "https://calendarmcp.googleapis.com/mcp/v1"
scopes = ["https://www.googleapis.com/auth/calendar.calendarlist.readonly",
          "https://www.googleapis.com/auth/calendar.events.readonly"]

[[servers]]
id = "gmail"
url = "https://gmailmcp.googleapis.com/mcp/v1"
scopes = ["https://www.googleapis.com/auth/gmail.readonly"]
optional_scopes = { drafts = ["https://www.googleapis.com/auth/gmail.compose"] }

[tools]          # defaults; the user changes them per tool in Settings
read  = ["list_events", "search_threads", "get_thread", "get_message", "list_labels", "list_drafts", "get_draft"]
ask   = ["create_draft"]
off   = ["trash_*", "mark_*_spam", "label_*", "unlabel_*", "create_label", "update_message_labels"]

[[pull]]         # G-5: a declarative mapping, run without a model
server = "calendar"
tool = "list_events"
args = { timeMin = "{today_start}", timeMax = "{today_plus_7}" }
keep = ["id", "summary", "start", "end", "attendees[].email", "hangoutLink", "htmlLink", "status"]
into = "events"            # then exposed to #92 as {title, start, end, account, url} (R-25)
```

Tool names and argument names above come from a third-party `tools/list` dump and are to be
checked in slice 1; the file changes, not the code.

## Signing in

1. The user presses **Sign in with Google** for an account label.
2. The core binds `127.0.0.1:0`, makes a PKCE verifier (32 random bytes) and a `state`, and opens
   the consent URL in the default browser with the scopes switched on (R-14).
3. Google redirects to `http://127.0.0.1:<port>/?code=…&state=…`. The listener checks `state`,
   answers with a small page ("You can close this tab and go back to HyperMe"), and closes.
4. The core exchanges the code with the verifier, stores the refresh token in the Keychain as
   service `ai.hyperme.connector`, account `google:<label>`, and keeps the access token in memory.
5. It calls `tools/list` on each server to check the account works, and shows the result in the
   wizard: "Works: 4 events today, 12 unread".

Desktop clients are public: Google says they "cannot keep secrets confidential" and treats the
secret as optional ([native apps](https://developers.google.com/identity/protocols/oauth2/native-app)).
The app still keeps it in the Keychain, since it is the user's.

## Token life

From Google's [OAuth overview](https://developers.google.com/identity/protocols/oauth2#expiration):
a client in "Testing" with sensitive scopes gets refresh tokens that expire in 7 days; a refresh
token stops working after 6 months unused, when the password changes and it holds Gmail scopes,
or past 100 live tokens per account per client (the oldest goes silently).

Consequences:

- Re-signing in replaces the token; the app never holds more than one per account, so the 100
  limit cannot bite.
- The scheduler pulls every day, so 6 months unused cannot happen while HyperMe runs.
- No sign-out in normal use is a requirement (R-10a, owner). The only cause in our control is a
  project left in "Testing", whose tokens expire after 7 days. The setup sheet therefore has the
  user set their project to "In production" and checks it before it finishes. For a project used
  only by its owner this needs no verification; Google shows an "unverified app" screen once, at
  consent, which the sheet explains beforehand.
- Still to check, over 14 days on the owner's account: that an "In production" personal client
  keeps its token, and whether Google's personal mode is already "In production" by itself. This
  does not block the other slices (owner, 26 November 2026); it is tracked in
  [#119](https://github.com/AndresHG/hyperme/issues/119).

## Why bring your own client (G-1)

| | Bring your own client | One HyperMe client |
| --- | --- | --- |
| Google verification | None in personal mode (Google's setup guide) | Required; Gmail read scopes are restricted, so a yearly CASA assessment too ([Google](https://support.google.com/cloud/answer/13463073)) |
| Quota | Each user has their own project | Shared by every user; standard tier reported as 5,000 requests a day per product (unverified) |
| Secret | The user's, in their Keychain | Ships in a public binary |
| Setup | About 5 minutes, three console pages | One click |
| Matches "your keys, your Mac" (PRD principle 4) | Yes | Partly: a project we run sits between the user and Google |

For developers, the first users (PRD Q-4), five minutes is acceptable, and the app already asks
for two provider keys. A shared client can come later if non-developers ask, as a separate spec.

## Sync and cost

Defaults (R-16, agreed with #92): every 30 minutes while the user is present, once on the
morning presence event, and on demand, per account. Each connector server with a pull has one
job, named `sync:<connector>-<server>`: `sync:google-calendar` and `sync:google-gmail`.

Quota (R-20): each server in the preset has `daily_units` and a cost per tool (`units`); the
request counter adds those units per account and day. From 80% of the day's quota a pull runs
at most every 2 hours, and at the quota it stops until the next day.

| Call | Per day (12 present hours) | Quota units (reported) |
| --- | --- | --- |
| Calendar `list_events` | 24 + 1 + a few on demand ≈ 30 | about 30 |
| Gmail `search_threads` for the inbox | ≈ 30 | 10 each, about 300 |
| Gmail `get_thread`, only new or changed threads | depends on mail; say 40 | about 40 |

About 370 units a day per account against a reported 5,000, so several accounts fit with room.
Measured in slice 1; R-20 backs off at 80% whatever the real number is.

Cost on the Mac: a pull is two or three HTTPS requests on the existing Tokio runtime and a few
SQLite writes. No timer runs while the Mac sleeps (R-17); the scheduler of #92 owns the wakeups.
No model call is made for a pull (G-5); models run only for the briefing, summary and questions,
which #92 budgets.

Push (Gmail `watch`) is a non-goal: it needs a Pub/Sub topic and a public endpoint.

## Storage

```sql
create table events   (account text, id text, calendar text, title text, starts_at text,
                       ends_at text, attendees text, meet_url text, status text,
                       primary key (account, id));
create table mail_meta (account text, id text, thread_id text, sender text, subject text,
                       snippet text, received_at text, labels text, primary key (account, id));
create table connector_requests (account text, day text, server text, calls int, units int,
                       primary key (account, day, server));
```

No bodies (R-18). Retention runs after each pull (R-19). Disconnect deletes by `account` (R-12).
The data folder stays on the Mac; FileVault covers it at rest.

## Settings → Connectors

Follows [UX.md](../../docs/UX.md): one section in the Setup group, saves as you go, a test next to
every secret, each page fits the window, errors link to their row.

**List page.**

```
Connectors                                              [ + Add connector ]

  G  Google · personal        Works · synced 3 min ago             ›
  G  Google · work            Sign in again                         ›
  ⌥  GitHub                   Works · 14 tools                      ›
```

**Add connector** opens a sheet: Google, GitHub (phase 3), Linear (later), and "Custom MCP
server" (URL or command, then OAuth, token or none). No "coming soon" rows: a preset shows up once
it works.

**Google setup** is three steps in the sheet, the first only once per Mac:

1. *Your Google client.* "HyperMe uses your own Google Cloud client, so nothing passes through
   us." A button opens Google's personal-mode setup; fields for client ID and secret; a "Use
   gcloud instead" link copies the commands. Test checks the client against Google.
2. *Sign in.* A label field ("personal") and **Sign in with Google**. The sheet waits with
   "Finish in your browser" and a Cancel.
3. *What Hyper can see.* Switches with plain words: "Calendar: read your events" (on), "Mail:
   read your inbox" (on), "Mail: write drafts for you to send" (per G-2). Done shows the check
   result.

**Account page.** State and last sync; requests today against quota; the switches from step 3;
the tool list with Read, Ask before or Off for each; Sign in again; Disconnect (with a one-line
confirm: "Removes access and deletes what HyperMe cached from this account").

**Setup check** gains one row per account. **Privacy** gains a "Connected accounts" block (R-24).

Mockups follow in the UX pull request after the owner approves this spec; the layout reuses the
Providers rows in [ux/settings.svg](../../docs/ux/settings.svg).

## Failure modes

| Failure | What the user sees | Recovery |
| --- | --- | --- |
| Refresh token refused | Account row "Sign in again"; one alert card | Sign in again (R-10) |
| Browser never returns | Sheet times out after 5 minutes with "Try again" | Listener closed (R-8) |
| Port taken | Never: the OS picks a free port | |
| Quota near the limit | Account page shows it; pulls slow down | R-20 |
| MCP server down or slow | Last good data shown with its age ("as of 9:10") | Next pull |
| Tool renamed by Google | Account row "Needs update"; pulls for that tool stop | Preset update in a release |
| Offline | Nothing new; the briefing says "calendar as of 8:45" | Pull on reconnect |

## Test plan

No test calls Google (`CLAUDE.md`). Fakes, all local:

- **Fake OAuth server** (a small HTTP server on a random port, a test-only dependency): authorize, token, refresh, revoke, with switches to expire or
  revoke tokens. Proves AC-2, AC-4, AC-6, AC-7.
- **Fake MCP server** speaking streamable HTTP with canned `tools/list` and tool results built
  from synthetic data. Proves AC-1, AC-5, AC-8 to AC-11.
- **Fake keychain** behind the secrets trait (the Keychain itself is `security-framework`, already in `hyperme-platform`). Proves AC-3.
- **Fake clock and power state** for the scheduler. Proves AC-8.
- AC-12 and AC-14 by hand on the owner's personal account, recorded as steps and timings only.

## Dependencies

Expected: `oauth2` (MIT OR Apache-2.0) for PKCE and token exchange, or about 150 lines of our own
on `reqwest` if it pulls too much; `rmcp` with its streamable HTTP client feature (already
decided); `open` (MIT) to launch the browser, unless Tauri's opener plugin is already in the app.
Each is justified in its commit, and `deny.toml` checks the licence.
