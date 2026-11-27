# Gmail approvals: live check (spec 166, slice 1, AC-15), 27 November 2026

Spec: [specs/166-gmail-approvals](../../specs/166-gmail-approvals/spec.md), AC-15. Run on the
owner's Mac (macOS, Apple Silicon), home network, with the owner's personal Google account and
the same personal-mode OAuth client as the [Google live check](2026-11-27-google-live-check.md).
Commands: `KEEP=1 APPROVE=ask|unattended [SERVER=calendar] cargo run -p hyperme-providers
--example google_live_check`. Only steps, choice values, counts and timings are recorded: no
mail, event, subject, sender, address, message or title.

The owner chose each approval in the project chat, and the choice was typed into the run for
them.

## Steps

| UTC | Step | Result |
| --- | --- | --- |
| 15:09 | Sign in over loopback (PKCE) | Yes, 33.8 s including the owner's clicks |
| 15:16 | Mail pull's `search_threads` on Gmail `/mcp/v1`, client declaring form elicitation | `input_required`, one `approval_choice` form |
| 15:16 | Choices offered | `ALLOW_ONCE`, `ALLOW_TOOL_ALWAYS`, `DENY_ONCE` (no owner-level choice) |
| 15:16 | Owner picked `ALLOW_TOOL_ALWAYS`; retry with the state echoed | Complete: 20 threads, nothing under `omittedContent`; 2 requests, 3.9 s |
| 15:17 | Same call, unattended (no answer possible) | 20 threads, 1 request, 2.6 s |
| 15:17 | Calendar pull's `list_events` on `/mcp/v1self`, unattended | "needs your OK": Calendar asks too once elicitation is declared |
| 15:23 | Calendar choices offered | `ALLOW_ONCE`, `ALLOW_OWNER_ALWAYS`, `ALLOW_TOOL_ALWAYS`, `DENY_ONCE` |
| 15:24 | Owner chose "Always allow this tool": `ALLOW_TOOL_ALWAYS` | Complete; 2 requests, 3.3 s |
| 15:24 | Both pulls, unattended | Gmail 20 threads (1 request, 2.6 s); Calendar 6 events (1.0 s) |
| 15:48 | Both pulls, unattended, 32 min after the Gmail approval | Gmail 20 threads, 1 request, no prompt, 2.5 s; Calendar 6 events, 1.8 s |
| 15:55 | Both pulls, unattended, 31 min after the Calendar approval | Gmail 20 threads, 1 request, 2.8 s; Calendar 6 events, 1.5 s; no prompt |

Before this slice, the same Gmail call returned 0 threads, with every thread under
`omittedContent` (`DATA_PROTECTION_POLICY`).

## Findings

- **Persistence holds for unattended calls.** After "Always allow this tool", calls with no
  approver ran with no `input_required` at least 30 minutes later (the open question in the
  spec's pre-mortem, row 1). Slices 2 and 3 can go ahead.
- **Declaring elicitation makes Calendar ask too.** Calendar on `/mcp/v1self` answered without
  approval before; once the client declares form elicitation, `list_events` asks. The setup
  sheet's check (R-14) must therefore run and approve every switched-on pull, not only mail.
  Until slice 3 ships the card, a build of master after #186 has unattended calendar pulls that
  end in "needs your OK" (not a job failure, R-16b) until approved.
- **Calendar offers an owner-level choice; Gmail did not.** The order of choices differs between
  servers, so a card must never map a number to a fixed choice.
- **Google names the client "rmcp"** in its question ("Allow rmcp to use search_threads?"):
  HyperMe sends rmcp's default client name. Slice 2 sets the client info to HyperMe.
- The approvals are tied to the OAuth client ("the same agent"); whether they also cover the
  app's own sign-in with the same client is to be seen in slice 2's check.

## Not recorded

The sign-in made for this check is kept in the Keychain (`google:livecheck`) and has not been
revoked. Revoking a Google grant may end the other sign-ins of the same client, so that is left
to the owner.

## In-app check, after slice 3 (19:44 to 19:52 UTC)

HyperMe.app built from master (commit 8157484) with `scripts/bundle-macos.sh`, same Mac, same
network. The owner added the account through Settings → Connectors → Google.

| Step | Result |
| --- | --- |
| First two sign-ins, Label field left empty (its "personal" is placeholder text) | Browser sign-in went through, the token was stored without a label, no account was added: [#216](https://github.com/AndresHG/HyperMe/issues/216) |
| Sign-in with the label typed | Account "Google · personal" added (calendar, mail, drafts) |
| Setup check right after (both pulls, interactive approver) | No `input_required`: Google kept the afternoon's "Always allow this tool" for this client, so no card appeared. Mail 20 threads, Calendar 7 events, nothing held back, no server state |

The approval card itself was not seen live, because Google did not ask; it is covered by the
tests and the browser check of slice 3 (PR #205).
