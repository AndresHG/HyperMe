# Approvals for connector tools (MCP form elicitation), first for Gmail

**Status:** approved by the owner, 27 November 2026, with A-1 to A-3 answered as proposed the same day.
**Issue:** [#166](https://github.com/AndresHG/hyperme/issues/166). **Size:** L. It needs more than
one pull request, it changes the MCP client every connector shares, and it adds a card that answers
a provider for the user.
**Design:** [design.md](design.md). **Slices:** [tasks.md](tasks.md).
**Builds on:** [specs/97-google-connectors](../97-google-connectors/spec.md) (R-16, R-22, R-23, AC-14).

## Problem

The live check of 27 November 2026 ([results](../../docs/results/2026-11-27-google-live-check.md))
found that Gmail's MCP server gives HyperMe no mail. `search_threads` lists every thread under
`omittedContent` with reason `DATA_PROTECTION_POLICY`, on `/mcp/v1` and `/mcp/v1self` alike.
Calendar reads work.

Google's servers ask the user to approve some calls through MCP form elicitation
([Google, "Handle user approvals with form elicitation"](https://developers.google.com/workspace/guides/mcp-elicitation)).
They never ask a client that does not declare form elicitation. For such a client, Google leaves
out what needs an approval, or refuses the call with `-32021 MissingRequiredClientCapability`.
HyperMe's client declares nothing, so the mail pull of spec 97 (R-26, R-27) returns 0 threads, and
the morning card's "mail to answer" stays empty.

## How the approval works (from the sources, 27 November 2026)

- **Declaring support.** The client sends
  `_meta["io.modelcontextprotocol/clientCapabilities"] = {"elicitation": {"form": {}}}` on every
  request. Google's servers are stateless (protocol 2026-07-28, `server/discover`), so the
  declaration goes on each request rather than once per session.
- **Asking.** A call that needs approval answers `resultType: "input_required"` with an
  `inputRequests` map and an opaque `requestState`. Google sends one request, `approval_choice`,
  as a form:
  - a Markdown `message` that describes what the call touches;
  - a schema with one string property `approval_choice`, whose `title` is the localised question
    and whose `enum` and `enumNames` are the choices.
  - A result may also carry only a `requestState` and no request, when the server sheds load (MCP
    2026-07-28, "multi round-trip requests").
- **The choices.** Each prompt carries only the choices that apply:
  - `ALLOW_ONCE` "Allow once" and `DENY_ONCE` "Don't allow" are always there.
  - Persistent choices appear only when the user's and the admin's policy allow them:
    `ALLOW_TOOL_ALWAYS` "Always allow this tool", `ALLOW_OWNER_ALWAYS` (reads, up to 3 owners),
    `ALLOW_RESOURCE_ALWAYS` and `ALLOW_AUDIENCE_ALWAYS`.
  - Trash and destructive actions offer only once or don't.
- **Answering.** The client sends the same `tools/call` again:
  - with a new id and the same name and arguments;
  - with `requestState` echoed unchanged;
  - with `inputResponses = {"approval_choice": {"action": "accept", "content": {"approval_choice": "<value>"}}}`.
  - `DENY_ONCE` ends the call as a complete result with `isError: true`. A machine-readable reason
    (`UserDeniedAction`) appears in `_meta["google.com/error"]` only "when enabled"; otherwise
    there is only a localised text. An admin block (`AdminPolicyBlock`) is reported the same way,
    without asking.
- **Persistence.** "Saved persistent choices pre-approve matching future calls from the same
  agent." Google keeps them on its side, and clients must not cache approval decisions. Workspace
  admins can allow such choices "for up to 90 days"
  ([admin help](https://knowledge.workspace.google.com/admin/apps/control-which-apps-access-google-workspace-data)).
  Whether that limit applies to a personal account is not documented.
- **Unattended calls** "can only run MCP actions that don't require user confirmation". The
  guide implies, but does not state, that an earlier "Always allow" lets a later unattended pull
  through. Slice 1 checks it on the owner's account.
- **`/mcp/v1` and `/mcp/v1self`.** `/mcp/v1` is Google's recommended endpoint, with elicitation,
  every tool and full result sets. `/mcp/v1self` is the fallback for hosts without elicitation.
  It also filters list results down to what needs no approval.
- **rmcp 3.12** already speaks this round trip (SEP-2322, "MRTR"):
  - `call_tool_once` returns `CallToolResponse::InputRequired`;
  - `CallToolRequestParams` carries `request_state` and `input_responses`;
  - in the stateless lifecycle, the capabilities from `ClientHandler::get_info` go into each
    request's `_meta`.
  - `Peer::call_tool`, which HyperMe uses today, treats `input_required` as an unexpected answer.

Google does not document how long a `requestState` stays valid, where a user sees or revokes
saved choices, or whether `decline` and `cancel` behave like `DENY_ONCE`.

## Goal and gate

- **Goal:** HyperMe's mail pull returns the owner's inbox threads after one approval during setup,
  which the owner can understand at a glance. Any later approval Google asks for reaches the
  owner as one card, never as a silent gap.
- **Gate:** for a week on the owner's account, the scheduler's mail pulls return threads with no
  prompt after the setup approval, and no morning card has an empty mail block without a visible
  reason (slice 4).

## Requirements

EARS form. "The client" is HyperMe's remote MCP client (`hyperme-providers::mcp`). "A call" is a
`tools/call` to a connector server. Nothing here is Google code: the client handles any server
that asks through form elicitation. Google is the first server, and the one tested by hand.

### Protocol

- R-1. When a server speaks the stateless lifecycle, the client shall declare form elicitation
  (`elicitation.form`) in every request's client capabilities. It shall not declare URL
  elicitation or sampling.
- R-1a. When a server speaks only the older `initialize` lifecycle, the client shall declare no
  elicitation. If such a server still sends `elicitation/create`, the client shall answer with an
  error, never with a decline.
- R-2. When a call answers `input_required` with input requests, the client shall:
  - hand each request to the call's approver;
  - send the call again with the same tool name and arguments, the `requestState` echoed byte for
    byte, and the approver's answers as `inputResponses`;
  - answer only with `action: "accept"` and one of the form's own values, never with `decline`
    or `cancel`.
- R-2a. When a call answers `input_required` with only a `requestState`, the client shall send it
  again with the state and no answers, after 1 s and then 2 s, without asking any approver.
- R-3. The client shall send at most 3 retries for one call. A fourth `input_required` ends the
  call as failed. The server then has seen at most 4 calls.
- R-4. The client shall check the caller's tool allowlist and the preset's forbidden tools before
  the first call, as today (spec 97, R-22). An approval shall never widen what a caller may call.
- R-5. The client shall keep no approval decision. Every call asks the server, and the server
  decides (Google's rule).
- R-5a. Any answer other than a final result or `input_required` (for example a task result)
  shall end the call as failed.

### Who approves

- R-6. Every call shall have an approver:
  - **interactive**, when the user started the call or is watching it: the setup sheet's check,
    Review from an alert, and later a question to Hyper;
  - **unattended**, for scheduler jobs.
- R-7. HyperMe shall never answer an approval without the user's choice. The unattended approver
  shall send no answer and no retry. It shall end the call with "approval needed", keeping the
  tool name and nothing else from the request.
- R-8. When an unattended pull ends with "approval needed", the app shall:
  - mark that account's server "Needs your OK";
  - raise at most one alert per account and server per day, with ↵ Review, which runs the same
    pull with the interactive approver;
  - run that server's unattended pulls only once a day (on the morning presence event) until the
    state clears, so a state that only the user can change costs no quota every 30 minutes.
- R-9. The interactive approver shall show the approval card and wait for the user. If the user
  presses Esc, or the card goes unanswered for 5 minutes, it shall send nothing. The call then
  ends as "approval needed". The alert shrinks into the dot's ring and stays in Today, as every
  unanswered alert does (UX.md, "Alerts").

### The approval card

- R-10. The card shall show:
  - which account and server ask ("Google · personal · Mail");
  - a title: the choice property's `title`, else the schema's `title`, else "Allow HyperMe to use
    <tool>?";
  - the `message`;
  - one key per choice, in the server's order, labelled from `enumNames` (or `oneOf` titles), or
    with the raw value when there is no label.
  - It shall show how long a choice lasts only when the server's request says so.
- R-11. The card shall render `message` as safe text:
  - paragraphs, bold, italic, inline code and lists are drawn;
  - raw HTML, images and scripts are shown as plain characters;
  - a link shows its text and its address, and nothing opens.
- R-12. The card shall accept only a form whose schema has exactly one required string property
  with an `enum` (with or without `enumNames`), or with `oneOf` of `const` and `title`. For any
  other form, or one that fails to parse, HyperMe shall show no card and send no answer. The call
  ends as "Needs an update", logged with the tool name.
- R-13. ↵ shall pick the default choice (A-1). Keys 1 to n pick the choices in order. Esc closes
  the card and sends nothing (R-9). In the pill, the card takes no focus (UX rule 2). In the setup
  sheet it is part of the sheet.

### Setup and states

- R-14. After sign-in, the setup sheet's check shall run each switched-on pull's list call once,
  with the interactive approver. If the server asks, the card appears inside the sheet with one
  line: "Background updates need an *Always allow* choice."
- R-15. When the user picks a choice whose value starts with `DENY`, the app shall mark the server
  "Not allowed" on the account page. That server's unattended pulls shall be skipped, and no alert
  raised, until the time set by A-3 (only Review or Ask again).
- R-16. When a server's error result names `AdminPolicyBlock` in `_meta["google.com/error"]`, the
  app shall mark the server "Blocked by your admin", skip its unattended pulls and raise no alert,
  until the next sign-in or a Review that succeeds. An error result with no recognised reason is
  an ordinary server error, as today. This is best effort: it depends on Google sending the reason.
- R-16a. The states shall take precedence in this order: Sign in again (the whole account), then
  Blocked by your admin, then Not allowed, then Needs your OK. A pull that succeeds clears the
  server's state, and so does a new sign-in for that account.
- R-16b. "Needs your OK", "Not allowed" and "Blocked by your admin" shall not count toward the
  job-failed alert of spec 92.
- R-17. When an answer lists `omittedContent`, the pull shall count the omitted items. The account
  page shall show the count ("Gmail held back 12 threads") with Review.
- R-18. The Gmail server in the Google preset shall use the endpoint chosen in A-2. The forbidden
  tools (send, reply, forward) stay forbidden whatever the endpoint offers.

### Privacy and safety

- R-19. An approval shall send the server only the chosen value and the echoed `requestState`.
  Nothing new leaves the Mac.
- R-20. The `message`, the title and the `requestState` shall stay in memory only while the card
  lives:
  - they are never written to disk or to logs, never sent to a language model, and never read as
    instructions (spec 97, R-22);
  - logs keep the account, server, tool and choice value only;
  - the app's log filter keeps rmcp below debug level, because rmcp logs whole requests and
    results at debug.
- R-21. The Privacy section shall say, in one line per connected account, that Google may ask
  before HyperMe reads some mail, and that Google keeps the choice, not HyperMe.

## Acceptance criteria

All run against the local fake MCP server unless marked "by hand". Tests cite these ids.

- AC-1. Against a stateless fake, every `tools/list` and `tools/call` carries `elicitation.form` in
  `_meta["io.modelcontextprotocol/clientCapabilities"]`, with no `url` or `sampling`. Against a
  fake that speaks only `initialize`, the `initialize` request declares no elicitation, and an
  `elicitation/create` it sends gets an error, not a decline (R-1, R-1a).
- AC-2. The fake answers a read with a Google-shaped `approval_choice` form of four choices. With
  the interactive approver choosing "Always allow this tool", the fake receives a second call:
  - with the same name and arguments, and the `requestState` unchanged;
  - with `inputResponses.approval_choice = {action: "accept", content: {approval_choice: "ALLOW_TOOL_ALWAYS"}}`.
  - The final result reaches the caller.
  - A later identical call is sent to the server as a normal call, and is asked again if the
    server asks (R-2, R-5).
- AC-3. With the unattended approver, the fake receives exactly one call. The pull ends with
  "approval needed" and the server is "Needs your OK". A second failing pull the same day raises
  no second alert. The next 30-minute pull of that server does not run (R-7, R-8).
- AC-4. Review from that alert runs the pull with the interactive approver. After approval the
  threads are stored, and the state and the alert clear (R-8, R-16a).
- AC-5. "Don't allow" sends `DENY_ONCE` and marks the server "Not allowed", whether or not the
  fake's error result carries `_meta["google.com/error"]`. Later pulls of that server are skipped
  with no alert until A-3's trigger. No job-failed alert is raised (R-15, R-16b).
- AC-6. Esc, and a fake clock past 5 minutes, send no second call. The call ends "approval needed"
  (R-9).
- AC-7. A fake that keeps answering `input_required` sees exactly 4 calls, then the call fails. A
  fake that answers once with only a `requestState` gets one retry with no answers and no approver
  call (R-2a, R-3).
- AC-8. A form with a free-text field, two fields, an array property or no `enum` shows no card,
  sends no second call, and ends "Needs an update" naming the tool (R-12).
- AC-9. An error result naming `AdminPolicyBlock` in `_meta["google.com/error"]` marks the server
  "Blocked by your admin", with no alert and no retry. The same error result without that `_meta`
  is an ordinary server error (R-16).
- AC-10. A message holding `<script>`, `<img src=x onerror=…>`, raw HTML and
  `[x](javascript:alert(1))` renders as text spans only. No link is clickable, and the
  `javascript:` address is shown, not followed (R-11).
- AC-11. A tool outside the caller's allowlist, or forbidden by the preset, never reaches the fake,
  whatever the approver would answer (R-4, R-18).
- AC-12. After a test run with approvals, under the app's log filter, a grep of the captured logs
  and the data folder finds neither the fake's message, nor its title, nor its `requestState`
  (R-20).
- AC-13. In the setup check, a fake that asks shows the card in the sheet:
  - after approval, the check reads "Mail · works, 3 threads";
  - after "Don't allow", it reads "Mail · not allowed: mail pulls are skipped", and setup still
    finishes (R-14, R-15).
- AC-14. An answer with 12 items under `omittedContent` makes the account page show "held back 12"
  with Review (R-17).
- AC-16. The card model built from Google's form lists the choices in the server's order, with
  their `enumNames` labels, or the raw value when labels are missing. ↵ picks the A-1 default, and
  key 2 picks the second choice (R-10, R-13).
- AC-17. The Privacy section shows one line per connected Google account about approvals (R-21).
- AC-15. By hand, on the owner's account, with `examples/google_live_check` pointed at Gmail's
  `/mcp/v1`, recorded as steps, counts, choices offered and timings only:
  - sign in;
  - run the mail list call with the terminal approver, and pick the broadest persistent choice
    offered;
  - at least 30 minutes later, run it again with the unattended approver: it returns more than 0
    threads with no `input_required`;
  - an unattended calendar list call on the preset's endpoint still returns events with no
    prompt, now that elicitation is declared.

## Non-goals

- URL elicitation (a server sending the user to a web page) and sampling requests. HyperMe
  declares neither.
- Showing, listing or revoking Google's saved choices inside HyperMe. Google keeps them, and no
  documented API lists them.
- Approvals for sending mail. HyperMe never offers a send tool (spec 97, R-23).
- Answering for the user in any way: no saved answers and no auto-allow setting.
- Approval cards for questions to Hyper and for drafts. Both use the same approver once the phase
  3 cards carry connector calls. Google says creating a draft usually needs no approval.

## Pre-mortem

It is three months later and this failed. Why?

| Why | Becomes |
| --- | --- |
| "Always allow" does not cover unattended pulls, so mail still never arrives in the morning | Slice 1 checks it on the owner's account before any UI is built (AC-15). If it fails, the spec goes back to the owner with an interactive morning pull ("↵ Get mail") |
| Gmail offers only "Always allow for these owners" for reads, so every new sender asks again | Slice 1 records which choices are offered. A-1's default picks the broadest persistent choice. R-8 keeps it to one alert a day |
| The owner presses ↵ on "Allow once" in setup, and every background pull asks again | R-14's line in the sheet, A-1's default, and R-8's one alert a day |
| HyperMe nags, or burns quota, after a deny | R-15 skips the server's pulls, R-8 runs a "Needs your OK" server once a day, and R-16b keeps both out of the job-failed alert |
| Declaring elicitation makes Calendar start asking | AC-15 checks a calendar pull after the change. Any ask goes through the same path |
| A mail subject in the approval message tells Hyper to do something | R-20: the message is shown, and never sent to a model or read as instructions |
| The message carries HTML that runs in the web view | R-11 and AC-10: text spans only, drawn with `textContent` |
| Google changes the form (new choices, `oneOf` instead of `enumNames`) | R-12 accepts both shapes and takes labels from the server. Only the `DENY` prefix and the `ALWAYS` marker are read from values |
| The `requestState` expires while the card waits | R-9's 5 minutes. A late answer that fails shows "Took too long: Review to ask again" |
| `/mcp/v1` exposes send tools | R-18: forbidden tools are refused before any request (AC-11) |
| An older server asks through `elicitation/create` and HyperMe declines on its own | R-1a and AC-1 |

## Decisions

| ID | Decision | Chosen | Rejected and why | Who |
| --- | --- | --- | --- | --- |
| A-1 | What ↵ picks on the card | For a read pull (setup check, Review), the broadest persistent choice offered (this tool, then these owners, then this resource). Otherwise, and for any other call, "Allow once". Never a deny | Always "Allow once": safest per call, but background pulls would keep asking. No default: breaks UX rule 8 (↵ does the main thing) | Owner, 27 November 2026 |
| A-2 | Endpoint for Gmail and Calendar | Gmail moves to `/mcp/v1`, Google's recommended endpoint with full result sets. Calendar stays on `/mcp/v1self` until it needs approval | Gmail stays on `/mcp/v1self`: Google filters its list results to what needs no approval, so it may still leave threads out. Both on `/mcp/v1`: Calendar works today, and moving it may add prompts for no gain | Owner, 27 November 2026 |
| A-3 | After "Don't allow", when HyperMe asks again | Only when the user presses Review or Ask again on the account page | Next day: a daily nag about something the user already refused. Never: hides a state the user can fix; the account page still shows it | Owner, 27 November 2026 |
| A-4 | Drive the approval rounds ourselves or with rmcp's helper | Our own loop on `call_tool_once`, with an approver passed per call | `RunningService::call_tool`: it answers through the session's single `ClientHandler`, and that session is shared by sync jobs and the user's calls, so it cannot tell an unattended call from an interactive one | Proposed here |
| A-5 | Where the Markdown is rendered | In Rust, into a small list of typed spans that the web view draws with `textContent` | A Markdown crate or a JS library: more code and more risk for five constructs, and harder to test | Proposed here |
