# Design: approvals for connector tools

Companion to [spec.md](spec.md). Approved with the spec, 27 November 2026.

## Where it fits

```
 scheduler sync job                     setup sheet check / alert Review
   McpCaller(.., Unattended)              McpCaller(.., Interactive -> card)
        \                                   /
         v                                 v
 +--------------------------------------------------------------+
 | hyperme-providers::mcp::RemoteServer::call_tool              |
 |   allowlist + forbidden check                 (spec 97 R-22) |
 |   loop: up to 4 calls (3 retries)                     (R-3)  |
 |     call_tool_once(params)                                   |
 |       Complete(result)      -> classify isError, return      |
 |       InputRequired(state only)   -> wait 1 s / 2 s, retry   |
 |       InputRequired(requests)     -> parse form (R-12)       |
 |                                      approver.answer(ask)    |
 |                                      retry or stop    (R-7)  |
 |       anything else         -> failed                 (R-5a) |
 |   HyperClient handler: declares elicitation.form             |
 |     (stateless only; R-1, R-1a)                              |
 +--------------------------------------------------------------+
         |
         v
 gmailmcp.googleapis.com/mcp/v1   (A-2)
```

## Files that change

| File | Change |
| --- | --- |
| `crates/hyperme-core/src/approval.rs` (new) | `ApprovalAsk`, `Choice`, `Approval`, the `Approver` trait, `Unattended`, the Markdown-to-spans function, the A-1 default |
| `crates/hyperme-core/src/sync.rs` | `SyncError::NeedsUser(ServerState)`; the pull runner reports `omitted` counts; the scheduler's pull guard reads server states |
| `crates/hyperme-core/src/health.rs` | `ServerStatusStore` trait and its memory version |
| `crates/hyperme-platform/src/task_store.rs` | SQLite table and migration for `ServerStatusStore` |
| `crates/hyperme-providers/src/mcp.rs` | `HyperClient`, the round loop, the new `ConnectorError` variants, `McpCaller::with_approver` |
| `crates/hyperme-providers/tests/fake_mcp/mod.rs` | Elicitation answers and recording (test plan) |
| `crates/hyperme-providers/examples/google_live_check.rs` | Terminal approver, unattended run, `--url` override (AC-15) |
| `crates/hyperme-core/presets/google.toml` | Gmail endpoint per A-2 |
| `app/src-tauri/src/connectors.rs` | Setup check runs pulls interactively; account page states; Ask again |
| `app/src-tauri/src/tasks.rs`, `pill.rs` | "Needs your OK" alert, Review, the card in the pill |
| `app/ui/connectors.js`, `settings.html`, `pill.html` | The card, drawn from spans with `textContent` |
| `docs/UX.md`, `docs/ux/` | The alert row and a card mockup (slice 3) |

No new crate, and no new rmcp feature. The elicitation types (`ElicitRequestParams`,
`ElicitationSchema`, `ElicitationCapability`) are not behind a feature in rmcp 3.12.

## The client handler and the lifecycle (R-1, R-1a)

Today `RemoteServer` serves the unit handler `()` with `ClientLifecycleMode::Auto`. With Auto, the
same handler's capabilities would go into `initialize` after a fallback, so an older server could
ask through `elicitation/create`. The change:

- `HyperClient { declare: bool }` implements `ClientHandler`.
  - `get_info` returns `ClientCapabilities` with
    `ElicitationCapability::new().with_form(FormElicitationCapability::new())` when `declare` is
    true, and with no elicitation otherwise.
  - `create_elicitation` returns an `McpError` ("not supported"), never rmcp's default decline.
- `connect` first serves `HyperClient { declare: true }` with `ClientLifecycleMode::Discover`.
  - Only if discovery fails does it serve `HyperClient { declare: false }` with `Initialize`.
  - The lifecycle that worked is remembered for the life of the `RemoteServer`, so the second
    attempt costs one request once, not on every call.
- In the stateless lifecycle, rmcp copies the handler's capabilities into each request's
  `_meta["io.modelcontextprotocol/clientCapabilities"]` (rmcp `model/meta.rs`).

## The approver (in `hyperme-core`, so the pull runner can carry it)

```rust
pub struct Choice { pub value: String, pub label: String }

pub struct ApprovalAsk {       // memory only (R-20)
    pub account: String,       // "google:personal"
    pub server: String,        // "gmail"
    pub tool: String,
    pub title: String,         // R-10's rule
    pub message: Vec<Span>,    // safe Markdown (A-5)
    pub choices: Vec<Choice>,  // server order
    pub default: Option<usize>,// A-1
}

pub enum Approval { Choice(String), NoAnswer }

pub trait Approver: Send + Sync {
    fn answer(&self, ask: ApprovalAsk)
        -> std::pin::Pin<Box<dyn Future<Output = Approval> + Send + '_>>;
}
```

The boxed future keeps the trait object-safe, so callers hold `Arc<dyn Approver>`.

- `Unattended` returns `NoAnswer` at once.
- `Interactive` (in the app) sends the ask to the web view with a card id, then waits on a oneshot
  for up to 5 minutes. A `tokio::time::timeout` runs only while the card is open.
- `McpCaller::with_approver(Arc<dyn Approver>)`:
  - the scheduler builds its `McpCaller` with `Unattended`;
  - the setup check and Review build theirs with `Interactive`.
  - `ToolCaller` keeps its signature, so the pull runner does not change.

**Parsing (R-12).** `ApprovalAsk::from_input_request` takes an `InputRequest::Elicitation` in form
mode. It needs a schema with exactly one required property, of type string, with `enum` (labels
from `enumNames`, which rmcp keeps as `LegacyEnumSchema`) or with `oneOf` of `{const, title}`.
Anything else, including a sampling or roots request, becomes `UnsupportedInput { tool }`. A
result that rmcp cannot parse at all arrives as a transport error and stays one (slice 1: mapping
every transport error after the first round to "needs an update" would hide network failures).

**The answer key.** The key of `inputResponses` is the input request's key. The property inside
`content` is the schema's one property. Neither is hard-coded, so a server with other names works
the same way.

**What is read from values.**
- A value starting with `DENY` is a deny (R-15).
- A value containing `ALWAYS` is persistent. That is used only for the A-1 default, ranked
  `TOOL`, `OWNER`, `RESOURCE`, then `AUDIENCE`.
- Other servers get "Allow once"-like behaviour: the first non-deny choice is the default.

The retry, for Google:

```json
{"name": "search_threads", "arguments": { "...": "unchanged" },
 "requestState": "<echoed>",
 "inputResponses": {"approval_choice": {"action": "accept",
                    "content": {"approval_choice": "ALLOW_TOOL_ALWAYS"}}}}
```

## Classifying the end of a call

| The client saw | `ConnectorError` | `SyncError` | Server state |
| --- | --- | --- | --- |
| Approver gave `NoAnswer` | `ApprovalNeeded { tool }` | `NeedsUser(NeedsOk)` | Needs your OK |
| It sent a `DENY*` value, and the result is an error (with or without a reason) | `Denied { tool }` | `NeedsUser(NotAllowed)` | Not allowed |
| An error result with `_meta["google.com/error"].details[].reason == "UserDeniedAction"` and no deny sent | `Denied { tool }` | `NeedsUser(NotAllowed)` | Not allowed |
| An error result with reason `AdminPolicyBlock` | `AdminBlocked { tool }` | `NeedsUser(Blocked)` | Blocked by your admin |
| An error result with no recognised reason | as today ("answered with an error") | `Failed` | none |
| A form HyperMe can't draw (R-12) | `UnsupportedInput { tool }` | `Failed` | none ("Needs an update" on the page) |
| A fourth `input_required` | `TooManyRounds { tool }` | `Failed` | none ("Needs an update") |
| A task result, or another unknown answer | `Server("unexpected answer")` | `Failed` | none |

Each variant carries a tool name and nothing from the form.

## Server states (R-8, R-15, R-16, R-16a)

`AccountStatusStore` stays as it is: one boolean per account for Sign in again. A new store, kept
separate because its key and its rules differ:

```rust
pub enum ServerState { NeedsOk, NotAllowed, Blocked }

pub trait ServerStatusStore: Send + Sync {
    fn get(&self, account: &str, server: &str) -> Result<Option<ServerState>, String>;
    /// Sets the state. Returns true when an alert is due: NeedsOk and no alert yet for this
    /// account and server on `day`.
    fn set(&self, account: &str, server: &str, state: ServerState, day: Date) -> Result<bool, String>;
    fn clear(&self, account: &str, server: &str) -> Result<(), String>;
    fn clear_account(&self, account: &str) -> Result<(), String>; // on sign-in and disconnect
}
```

```sql
create table if not exists connector_server_status (
  account text not null, server text not null,
  state text not null,            -- needs_ok | not_allowed | blocked
  since text not null,            -- UTC, for the account page
  last_alert_day text,            -- local date of the last "Needs your OK" alert
  primary key (account, server));
```

The migration is additive (`create table if not exists`). Disconnect deletes by account
(spec 97, R-12).

- **Precedence (R-16a).** If the account needs a sign-in, `guarded_sync` already skips it. Then:
  - Blocked and NotAllowed skip the server's unattended pulls;
  - NeedsOk runs them once a day: the first unattended pull of the local day (slice 2);
  - otherwise they run normally.
- **Clearing.**
  - Any pull that succeeds clears the server's state.
  - A sign-in clears the whole account.
  - Ask again on the account page clears NotAllowed and runs Review.
  - Higher states replace lower ones; a lower state never replaces a higher one, except through a
    pull that succeeds.
- **Failure count (R-16b).** `SyncError::NeedsUser` is not a failure for the job-failed alert of
  spec 92.

**`omittedContent` (R-17).** The pull runner counts the items in the answer's `omittedContent`; a
missing list counts as 0, as lists already do. The count goes into the pull's result and onto the
account page. The omitted items themselves are not stored.

## The card

There are two places for it, with one model (`ApprovalAsk`) and one renderer in `app/ui`.

**Setup sheet** (Settings, a normal window).
- After sign-in, the Done check runs each pull's list call once, with `Interactive`.
- If an ask arrives, the sheet shows the card above the check lines, with the line from R-14.

**Alert from the dot** (phase 4 alerts, UX.md "Alerts").
- A new alert kind: "Gmail wants your OK to read mail for personal" · ↵ Review · Esc.
- Review grows the alert into the approval card and runs the pull with `Interactive`.
- Unanswered, it shrinks into the dot's ring and stays in Today.

```
+--------------------------------------------------+
| Google · personal · Mail                         |
| Allow HyperMe to use search_threads?             |
|                                                  |
| HyperMe wants to read **12 threads** from 3      |
| senders outside your organisation.               |
|                                                  |
| ↵ Always allow this tool   2 Allow once          |
| 3 Don't allow                          Esc Later |
+--------------------------------------------------+
```

**Markdown (A-5).** The message becomes `Vec<Span>`, with the kinds `Text`, `Bold`, `Italic`,
`Code`, `Break`, `ListItem` and `Link { text, url }`.
- The web view creates elements and sets `textContent` only.
- `Link` shows "text (url)" as plain text.
- No `innerHTML` anywhere on the path (AC-10).

The UX.md "Alerts" table gains the "Needs your OK" row in slice 3, and the card gets a mockup in
`docs/ux/` with it.

## Privacy

- **What leaves the Mac per approval:** the echoed `requestState` and one enum value.
- **The message:** it describes mail Google already holds. HyperMe draws it and drops it.
- **The `requestState`:** opaque and never logged, since the MCP spec has servers bind it to the
  user and to a digest of the request.
- **Logs:** rmcp logs whole requests and results at debug level, so the app's tracing filter keeps
  `rmcp` at `warn` (R-20). AC-12 runs under that filter.
- **The privacy boundary** in `CLAUDE.md` does not change.

## Budgets

- **Memory:** one `ApprovalAsk` while a card is open, and nothing kept. No new crate.
- **Network:**
  - an approved call costs one extra request;
  - an unattended call that needs approval costs one request and stops;
  - a server in Needs your OK costs one request a day, not 24.
- **Idle:** no new timer; the 5-minute wait exists only while a card is open.

## Failure modes

| Failure | What the user sees | Recovery |
| --- | --- | --- |
| Google keeps asking every pull despite "Always allow" | "Needs your OK", once a day | Slice 1 finds this first (pre-mortem, row 1) |
| `requestState` expired when the answer arrives | "Took too long: Review to ask again" | Review starts a fresh call |
| The server sends a form HyperMe can't draw | "Needs an update" on the account page | A spec or preset update |
| An admin blocks the tool | "Blocked by your admin" | The admin, not HyperMe |
| The user denies | "Not allowed · Ask again" | Ask again on the account page |
| Offline in the middle of an approval | The card closes: "Couldn't reach Gmail" | The next pull |

## Test plan

No test calls Google. The fake MCP server in `tests/fake_mcp` already answers through rmcp's
server-side `CallToolResponse`. It gains:

- a switch to answer a tool with `CallToolResponse::InputRequired`, carrying a Google-shaped form
  and a canned `requestState`, or a `requestState` alone;
- recording of `_meta`, `requestState` and `inputResponses` for each call (AC-1, AC-2, AC-7);
- canned error results, each with and without `_meta["google.com/error"]`:
  - `UserDeniedAction` and `AdminPolicyBlock`;
  - endless `input_required`;
  - odd forms;
  - `omittedContent` (AC-5, AC-7 to AC-9, AC-14);
- an `initialize`-only mode that sends `elicitation/create` (AC-1).

The other checks:

- A scripted `Approver` returns fixed choices or `NoAnswer`, or waits on a fake clock (AC-6).
- The Markdown spans and the A-1 default are unit-tested in core (AC-10, AC-16).
- The server states use the memory store and the SQLite store (AC-3, AC-5, AC-9).
- The setup check, the alert and the Privacy line use the app's existing command tests with the
  fake (AC-4, AC-13, AC-17).
- AC-12 greps the test's temporary data folder and the logs captured under the app's filter.
- AC-15 runs by hand with `examples/google_live_check`. It gains `--url` to target `/mcp/v1`, a
  terminal approver that prints the title and the choices (never the message) and reads a number,
  and an `--unattended` flag.
