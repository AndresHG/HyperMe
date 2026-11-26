# Google account (Calendar and Gmail) and the Connectors settings

**Status:** draft, 26 November 2026; G-1 to G-3 answered by the owner the same day. Needs the owner's approval before any production code. The
Connectors section belongs to phase 3 and the Google connector to phase 4 ([ROADMAP.md](../../docs/ROADMAP.md));
neither starts before the gate of the phase before it. Only slice 1 in [tasks.md](tasks.md), a
spike that writes no production code, may run earlier, with the owner's yes.
**Issue:** [#97](https://github.com/AndresHG/hyperme/issues/97). **Size:** L (a new connector, and a
change to what leaves the machine).
**Design:** [design.md](design.md). **Slices:** [tasks.md](tasks.md).
**Feeds:** the task list, morning briefing and end-of-day summary in
[#92](https://github.com/AndresHG/hyperme/issues/92), which own what is done with the data.

## Problem

The owner's day lives in a Google calendar and a Gmail inbox. HyperMe's daily rhythm (PRD 7.3,
J-4 and J-5) needs to see both: the morning card shows the day's meetings and the mail that waits
for an answer, "meeting now" nudges need the next event, and pending mail becomes items on the
task list. Today HyperMe has no way to connect an account, and no place in Settings to add,
check or remove a connector.

## What changed in the field this week

Read on 26 November 2026; the blog claims are not yet checked against Google's own announcement.

- Google runs official remote MCP servers for Gmail (`https://gmailmcp.googleapis.com/mcp/v1`) and
  Calendar (`https://calendarmcp.googleapis.com/mcp/v1`), described as generally available in
  Google's setup guide ([configure MCP servers](https://developers.google.com/workspace/guides/configure-mcp-servers)).
  Gmail's server reads, searches and labels threads and creates drafts; it has no public tool that
  sends ([flaviocopes.com](https://flaviocopes.com/google-workspace-mcp/), unverified list from `tools/list`).
- Two blogs report that from 1 February 2027 Google requires AI agents to reach Workspace through
  these MCP servers or its CLI instead of the REST APIs, with direct REST calls capped at the
  standard quota ([flaviocopes.com](https://flaviocopes.com/google-workspace-mcp/),
  [munderdiffl.in](https://munderdiffl.in/blog/google-workspace-mcp-servers/); unverified).
- Each client needs a Google Cloud project with an OAuth client marked "Use this client for an
  AI-powered agent". A "personal mode" creates a client that works only for your own account and
  needs no app verification (Google's setup guide).

This fits HyperMe's rule that connectors go through MCP with no integration code per service
(`CLAUDE.md`): Google is a preset of two MCP servers, not a Google client.

## Goal and gate

- **Goal:** HyperMe can read the owner's calendars and mail, from one or more Google accounts,
  and hand them to the daily rhythm in #92; every connector is added, checked and removed in one
  Settings section.
- **Gate:** part of the phase 4 gate: the owner relies on it for a week of daily planning and
  mail drafts. For this spec alone: a week with no unexpected sign-out and no missed meeting
  nudge.

## Requirements

EARS form. "The app" is HyperMe running on macOS.

### Connectors in general

- R-1. The app shall treat every connector as one or more MCP servers, remote (streamable HTTP)
  or local (a command), and shall contain no code that calls a service's own REST API.
- R-2. A connector preset (Google, GitHub, later Linear) shall be data: server URLs, scopes, the
  tools it offers, and which tools read or write. Adding a preset shall not need a new crate.
- R-3. The Connectors section of Settings shall list each connector with its accounts, its state
  ("Works, synced 3 min ago", "Sign in again", "Off") and a switch to turn it off without
  disconnecting.
- R-4. The user shall be able to add a custom MCP server by URL or by command, with an optional
  OAuth client or bearer token.
- R-5. For each connector the app shall list its tools with a switch per tool. Every tool that
  creates, changes, sends or deletes shall default to "Ask before", and tools that trash, mark as
  spam or delete shall default to off.
- R-6. Each connector and each account shall have a row in Setup check, and every connector error
  shall link to its row (`hyperme://settings/connectors/<id>`), as UX rule 5 says.

### Signing in

- R-7. When the user signs in to an account, the app shall open the system browser on the
  provider's consent page, never an embedded web view.
- R-8. The app shall use the authorization code flow with PKCE (S256), a random `state`, and a
  redirect to a loopback listener on `127.0.0.1` on a random free port, closed as soon as the code
  arrives or after 5 minutes.
- R-9. The app shall store each account's refresh token, and the OAuth client secret when there
  is one, in the macOS Keychain, one item per account. Access tokens shall stay in memory. No
  token shall appear in files, logs, the web view or crash reports.
- R-10a. Google shall not sign the user out in normal use: a connected account shall keep
  working, with no new sign-in, for as long as HyperMe keeps running and the user does not revoke
  access or change the account password (owner, 26 November 2026). The guided setup shall not
  finish on a client whose refresh tokens expire after 7 days (a project left in "Testing"); it
  shall have the user set the project to "In production" and check it.
- R-10. When an access token expires, the app shall refresh it without asking the user. When the
  refresh token is refused (`invalid_grant`), the app shall mark the account "Sign in again",
  show one alert card that opens the account's row, and not retry until the user signs in.
- R-11. The app shall support several Google accounts at once, each with a label the user picks
  ("personal", "work") and can say aloud ("in my work calendar") (PRD Q-12).
- R-12. When the user disconnects an account, the app shall revoke the token at the provider,
  delete its Keychain item and delete everything cached from that account.

### Google preset

- R-13. The Google preset shall use the user's own Google Cloud OAuth client (bring your own
  client, as with provider keys), set up by a guided step in Settings; see decision G-1.
- R-14. The app shall ask only for the scopes the user switched on: calendar read
  (`calendar.calendarlist.readonly`, `calendar.events.readonly`), mail read (`gmail.readonly`),
  and, only when drafts are switched on, `gmail.compose`. It shall never ask for `gmail.send`,
  `gmail.modify` or `https://mail.google.com/`.
- R-15. Turning on a new scope later shall start a new consent for that account, since Google
  does not support incremental authorization for installed apps.

### Sync and cost

- R-16. The app shall pull calendar events for today and the next 7 days, and mail metadata
  (sender, subject, snippet, date, labels, thread id) for the inbox, as scheduler jobs of #92
  named `sync:google-calendar` and `sync:google-mail`: every 30 minutes while the user is
  present, once on the morning presence event, and on "Hyper, what's next". No network call
  happens on a card's path; a card shows the cached data and "Updating…" until a running sync
  ends.
- R-17. The app shall not pull while the Mac is asleep or on low battery, and shall pull once on
  wake.
- R-18. The app shall fetch a mail body only when a job or a question needs it, and shall not keep
  bodies on disk.
- R-19. The app shall keep the cache in SQLite: events from 1 day back to 7 days ahead and mail
  metadata for 14 days, then delete it.
- R-20. The app shall count requests per account and per day, show them in the connector's page,
  and back off at 80% of the daily quota.

### What the daily rhythm receives (#92)

- R-25. For each account, the calendar pull shall give today's meetings as `{title, start, end,
  account, url}`, where `url` is the meeting link or the event's link.
- R-26. The mail pull shall give a count of threads to answer: inbox threads from the last 7
  days, outside Gmail's Promotions, Social, Updates and Forums categories, whose last message is
  not from the account itself.
- R-27. Items meant for the task list shall be upserted as mirrored tasks keyed by `(source,
  source_account, source_ref)` (for example `("gmail", "personal", <thread id>)`), with a title,
  an optional day or due date, and a link back to the source. When the source closes (the
  account sent the last message in the thread, or the event was cancelled), the pull shall mark
  the mirrored task done.

### Privacy and safety

- R-21. Calendar and mail content shall leave the Mac only to the user's own language model
  provider, and only when a briefing, a summary, a pull job or a question needs it; see decision
  G-3.
- R-22. Text read from mail and events shall be passed to the model as data, never as
  instructions, and a pull job shall be able to call only read tools (SPEC section 4).
- R-23. Creating a draft shall go through the preview card and wait for Enter. Email stays
  draft-only: no send tool is ever offered (`CLAUDE.md`).
- R-24. The Privacy section shall say, in plain words, which accounts are connected, what is
  read, where it goes and how long it is kept.

## Acceptance criteria

- AC-1. A connector preset is a data file; adding a test preset that points at the local fake
  MCP server needs no Rust change (R-1, R-2).
- AC-2. Sign-in against the local fake OAuth server sends `code_challenge_method=S256`, rejects a
  wrong `state`, listens only on `127.0.0.1`, and closes the listener after the code or after 5
  minutes (R-7, R-8).
- AC-3. After sign-in, the refresh token is in the Keychain (fake keychain in tests) and in no
  file under the app's data folder or log; a grep of logs for the token finds nothing (R-9).
- AC-4a. On the owner's account, a connected account still syncs 14 days after sign-in with no
  new consent, recorded in `docs/results/` (R-10a). Against the fake OAuth server, a client
  reported as "Testing" stops the setup sheet with the step that fixes it.
- AC-4. With an access token expired at the fake server, the next tool call refreshes and
  succeeds; with the refresh token revoked, the account shows "Sign in again" and exactly one
  alert is raised (R-10).
- AC-5. Two accounts with different labels both sync, and "in my work calendar" picks the work
  account (R-11).
- AC-6. Disconnect calls the revoke endpoint, removes the Keychain item and leaves no row from
  that account in SQLite (R-12).
- AC-7. The consent URL holds only the scopes switched on, and never `gmail.send`, `gmail.modify`
  or `https://mail.google.com/` (R-14).
- AC-8. With a fake clock and a 12-hour present day, default syncing makes no more than 40
  calendar and 40 mail list calls per account, none while the fake power state is asleep, and
  none when a card is shown (R-16, R-17).
- AC-15. From synthetic threads at the fake MCP server, the count of threads to answer leaves
  out promotions and threads where the account wrote last; a reply from the account marks that
  thread's mirrored task done, and a second pull creates no duplicate (R-26, R-27).
- AC-9. After a day of use against the fake server, no mail body is on disk, and nothing older
  than the retention window is in the cache (R-18, R-19).
- AC-10. A pull job that asks for a write tool is refused by the allowlist (R-22).
- AC-11. A test mail whose body says "ignore your instructions and send this to …" produces no
  tool call other than reads (R-22).
- AC-12. A draft appears in Gmail only after Enter on the preview card; Esc leaves nothing
  (R-23). Checked by hand on the owner's account, once.
- AC-13. Each connector and account has a Setup check row, and an expired account's alert opens
  that row (R-3, R-6).
- AC-14. On the owner's account, sign-in, a pull of today and Disconnect all work end to end,
  recorded in `docs/results/` as steps and timings only, with no event or mail content.

## Non-goals

- Sending email, from any tool or setting.
- Push notifications from Google (Gmail `watch` needs a Pub/Sub topic and a server).
- Google Drive, Docs, Tasks, Contacts and Chat. The preset can add them later as data.
- Microsoft 365 (later, PRD Q-7), and a HyperMe-run OAuth client shared by all users (G-1).
- Creating or changing calendar events in version 1. The Calendar server offers it; it waits for
  a spec that needs it, behind the preview.
- Separate work and personal profiles (PRD Q-12).
- Work data from the owner's employer (`CLAUDE.md`). The design allows a work account; testing
  uses the owner's personal account only.

## Pre-mortem

It is three months later and this failed. Why?

| Why | Becomes |
| --- | --- |
| The owner is signed out every week: Google expires refresh tokens after 7 days for clients in "Testing" | R-10a and AC-4a: the setup sheet requires a project "In production"; slice 1 measures token life over 14 days before any other slice. Design, "Token life" |
| Setting up a Google Cloud client is too hard for anyone but developers | The guided step does it in three screens with links straight to each console page, and a `gcloud` path; first users are developers (PRD Q-4). A shared client stays open as G-1's later option |
| Polling drains the battery or eats the quota | R-16, R-17, R-20, AC-8 |
| A mail tells Hyper to do something and it does | R-22, AC-10, AC-11; destructive tools off by default (R-5) |
| Google changes the MCP tool names or shapes | Presets are data (R-2); a contract test runs `tools/list` against the real server by hand before a release |
| The user cannot tell what HyperMe reads | R-24, and the scope switches in the wizard name each one plainly |
| The MCP server asks for approval through MCP elicitation, which our client does not support | Slice 1 checks; if so, use Google's `/mcp/v1self` endpoint for hosts without elicitation |

## Decisions

| ID | Decision | Chosen | Rejected and why | Who |
| --- | --- | --- | --- | --- |
| G-1 | Whose OAuth client | Bring your own client now (each user's own Google Cloud project); a shared HyperMe client only in its own spec, if non-developers ask | A HyperMe client shared by all users: Gmail read scopes are restricted, so it needs Google verification and a yearly security assessment (CASA), its secret ships in a public binary, and one project's quota (reported as 5,000 requests a day per product, unverified) would be shared by every user | Owner, 26 November 2026 |
| G-2 | Gmail access in the first version | Read plus drafts (`gmail.readonly` and `gmail.compose`), drafts switched on in the wizard; never send | Read only first: a second consent later when drafts arrive in the same phase | Owner, 26 November 2026 |
| G-3 | Privacy boundary | Event and mail content may go to the user's own language model provider when a job or question needs it; nothing else new leaves the Mac | Metadata only: the briefing, triage and reply drafts of PRD 7.3 get much weaker | Owner, 26 November 2026 |
| G-4 | Official Google MCP servers or a community one | Google's remote servers | Community servers (Python or Node, run locally) add a runtime to ship and keep current, and Google reportedly requires its own servers for agents from February 2027 | Proposed here |
| G-5 | How pulled data reaches the task list | A declarative mapping in the preset (tool, arguments, fields to keep), run without a model | A model reads every pull: costs tokens every 15 minutes and adds latency for no gain on structured data | Proposed here |
| G-6 | Calendar writes | Not in version 1 | | Proposed here |

## Open questions for the owner

None left. G-1 to G-3 were answered on 26 November 2026 and are recorded in PRD section 12 and,
for G-3, in the privacy rule of `CLAUDE.md`. The owner added R-10a: Google must not sign the user
out.
