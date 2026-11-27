# Voice commands: how others do it, and what HyperMe should copy

Research of 27 November 2026, for the phase 3 command loop ([ROADMAP.md](../ROADMAP.md)) and the
voice capture of phase 4 ([spec 92](../../specs/92-daily-tasks/spec.md)). Research only: nothing
here changes an earlier decision. The one choice it needed from the owner, how strictly a command
must start with "Hyper", was decided on 27 November 2026 (section 5).

The owner wants spoken commands such as "Hyper, create a task …", "Hyper, remind me to …" and
"Hyper, draft an email to …". Already decided: one key for everything, with "Hyper, …" routing to
commands and an optional command key (PRD Q-3); a preview card for anything that leaves the app,
Enter to confirm (PRD 7.2); tasks captured by voice save at once with ⌘Z to undo (spec 92, T-3);
email is draft-only (`CLAUDE.md`).

## 1. Summary

- **Nobody ships open-ended intent detection on top of dictation.** Every product that does both
  draws a hard line first: a separate key (Wispr Flow, Raycast), an explicit mode (Talon,
  Superwhisper, Aqua's Realtime mode) or a spoken name (Wispr's older "Hey Flow", Siri). The one
  automatic case, Aqua's "send it", only looks at the last words of a dictation.
- **Almost nobody previews.** Text edits apply at once and rely on undo. Wispr Flow asks only
  before changing a setting. HyperMe's rule (local actions at once with undo, outside actions
  behind a preview) is stricter than the field and matches what users expect of email.
- **Failure handling is thin everywhere.** Wispr Flow does not retry a failed command and puts the
  result on the clipboard when there is no text box. Aqua falls back to its simpler mode. No one
  publishes a latency figure for commands.
- **Recommendation:** route on the name "Hyper" in the first words of the transcript, decided
  locally in microseconds, then one tool-calling request to the cleanup provider's fast model,
  with a short tool list picked by the route. Tasks save at once with a done card; emails become a
  Gmail draft only after Enter on a preview card; if anything fails, the words are never lost.
  It adds no process, no local model and no listening microphone, so the idle budget is
  untouched.

## 2. The field

Sources were read on 27 November 2026. "Not stated" means the page does not say; it was not
tested.

| Product | How a command is told apart from dictation | What a command can do | Preview or confirmation | Latency | When it fails |
| --- | --- | --- | --- | --- | --- |
| **Wispr Flow, Command Mode** | Its own global shortcut, separate from dictation (Fn + Ctrl on a Mac). "No wake word is required". Off by default, under Settings, Experimental, paid plans only. A dictation that starts with "Hey Flow" or "ask perplexity" still runs as a command when Command Mode is on ([docs](https://docs.wisprflow.ai/articles/4816967992-how-to-use-command-mode)) | Rewrite, shorten, reformat or translate the selection; draft new text when nothing is selected; search Perplexity in the browser. The result replaces the selection or lands at the cursor | None: the edit applies directly. Esc cancels a command while it runs | Not stated | "Failed commands are not retried automatically." Busy servers show a message to wait and rerun. With no text box focused, the result goes to a notification and the clipboard. Without a paid plan the shortcut silently does nothing |
| **Wispr Flow, Realtime voice commands** | Realtime mode only; "other dictation modes handle voice commands differently" ([docs](https://docs.wisprflow.ai/articles/9701824265-chaining-multiple-actions-in-a-single-realtime-voice-command), updated 6 August 2026) | Several actions in one utterance, run in order ("make this more concise and also add a rule to never use the word 'utilize'"). Mentions Slack, Google Calendar and Gmail steps | "Flow shows a confirmation prompt before applying any settings change." Text edits apply without one | Not stated | Partial success: a step that fails (Gmail send and search were disabled) returns an error while the other steps finish. "Commands with many independent actions may not all be processed" |
| **Aqua Voice, Realtime** | A mode: double-press the Aqua key for hands-free, hold it for normal dictation. "Send it" is detected by position: "Aqua reads how you ended the dictation rather than scanning for a keyword", so "tell them I'll send it tomorrow" stays text ([realtime](https://aquavoice.com/realtime), [guide](https://aquavoice.com/guide)) | Spoken edits of what you just said ("change X to Y", "delete X", "make this a list", "start over", "undo"); "send it" presses the app's send key (⌘⇧D in Mail, ⌘↵ in Gmail, ↵ in chat apps) | None. Words show live in Aqua's panel; a "Paste only" setting never presses send. The guide says "review important text before you send it" | "A moment later"; no figure | Edits reach only recent speech, not text already in the document. Unsupported languages fall back to normal dictation for that one dictation |
| **Superwhisper** | No spoken commands. Modes (a model plus an optional AI prompt) switch by shortcut, from the menu bar, or by auto-activation rules per app or website ([modes](https://superwhisper.com/docs/modes), [switching](https://superwhisper.com/docs/modes/switching-modes)) | Rewrite the transcript per mode (Message, Mail, Note, Custom). Deep links (`superwhisper://record`) let Raycast, Alfred or Shortcuts drive it | None | Not stated | Not stated |
| **Willow** | Hotkey dictation. Its page documents spoken "Shortcuts" that insert saved text ([page](https://willowvoice.com/use-cases/hey)). Our competitor research of 26 November listed a "Hey Willow" desktop assistant from a secondary review; the vendor page read today shows no such feature, so it is **unverified** | Snippets; formatting | None | Not stated | Not stated |
| **Raycast** | A separate command, "Dictate to AI", with its own hotkey, beside plain dictation ([manual](https://manual.raycast.com/ai/dictation), [v2.11, 4 November 2026](https://raycast.com/changelog/macos/2-11)) | Sends the transcript, with context from the focused app, as a prompt to Quick AI or AI Chat. AI Extensions can call tools ("Talk to extensions, get things done", [AI](https://manual.raycast.com/ai.md)); that page was not read | Dictation pastes unless the experimental "Auto Submit" presses Enter. Confirmation for tool calls not checked | Not stated; "Keep microphone warm" cuts start-up delay | Not stated |
| **Apple Siri AI** (announced June 2026) | A name or a button: a separate assistant, not mixed with dictation. A "system orchestrator" reaches apps through App Intents ([newsroom](https://apple.com/ca/newsroom/2026/06/apple-introduces-siri-ai-a-profoundly-more-capable-and-personal-assistant)) | Drafts text, answers about what is on screen, acts in apps. Reminders apps expose `createReminder`, `updateReminder` and others through the App Intents reminders schema ([schema](https://developer.apple.com/documentation/appintents/app-schema-domain-reminders.md)) | Not stated in the release | Not stated | Beta in English only; excluded from the EU at first on iOS, iPadOS and watchOS |
| **Talon Voice** | Explicit modes: command, dictation and sleep, switched by saying "command mode", "dictation mode", "go to sleep", "welcome back". Commands are exact spoken forms written in `.talon` files ([wiki](https://talon.wiki/), [community docs](https://wolfmanstout.github.io/wolfmanstout_talon/core/modes/), the second generated by a tool) | Anything scripted: keys, apps, editor actions | None; commands are deterministic | Not stated; matching is local | A phrase that matches no command does nothing in command mode |
| **VoiceOS** | "Dictation Mode, selected-text editing, and Agent Mode" on Mac and Windows; how they are told apart is not stated ([vendor guide](https://www.voiceos.com/compare/wispr-flow), written by the vendor) | Actions in connected apps: "email, calendars, notes, issue trackers" | Not stated | Not stated | Not stated |

Not checked: OpenWhispr's agent name in dictation (listed in [UX.md](../UX.md) section on hotkeys,
not re-read), Raycast's AI Extensions confirmation, and any measured latency. No product above was
run on the owner's Mac.

## 3. What the field teaches

### 3.1 Telling a command from dictation

Three families, ordered from most to least predictable:

1. **A separate key or mode** (Wispr Command Mode, Raycast, Talon, Superwhisper, Aqua Realtime).
   No misroutes, but one more thing to learn. Wispr keeps it off by default and hidden under
   Experimental, which says how few people find a second key.
2. **A spoken name** (Wispr's "Hey Flow", Siri, Talon's exact forms). One key stays, and the name
   is a clear signal because nobody starts a sentence in an email with it.
3. **Intent from the words alone.** Only Aqua does this, for one phrase ("send it"), and only at
   the end of the dictation. Nobody does it for open commands, because a misroute breaks the one
   thing a dictation app must never break: "remind me to call you on Monday" dictated into Slack
   must land in Slack, not in the task list.

HyperMe already chose the second family with the first as an option (PRD Q-3). The field supports
that choice.

### 3.2 Confirmation

- Edits of the user's own text apply at once with undo everywhere. No one asks first.
- Wispr Flow confirms only a change to settings. Aqua presses send without asking, and tells users
  to review first, which is the risk HyperMe's draft-only rule avoids.
- So HyperMe's split is right: **local and undoable → do it and show a done card; outside the app →
  preview and Enter.** It matches spec 92 (T-3) and the Google drafts flow (spec 97, R-23).

### 3.3 Failure

- Wispr: no automatic retry; a busy-server message; the result goes to the clipboard and a
  notification when it has nowhere to land.
- Wispr Realtime: chained actions finish what they can and report the step that failed.
- Aqua: drop to the simpler mode for that one dictation.
- The lesson for HyperMe: the spoken words must never be lost, a write must never run twice, and
  the user must always have a one-key way back to plain text.

## 4. What HyperMe should do

Everything below fits the existing pieces: the dictation pipeline and its history, the
dictionary (spec 93), the cleanup providers (spec 96), the task store (`crates/hyperme-core/src/tasks.rs`),
and the action gate and preview (`crates/hyperme-core/src/actions.rs`,
`crates/hyperme-providers/src/actions.rs`).

### 4.1 The router: the name first, decided locally

After the final transcript arrives and the dictionary has run (spec 93, R-1):

1. **Command key held** → command route, whatever was said.
2. **The transcript starts with the name** → command route. "Starts" means the first three words,
   ignoring punctuation, case and accents, and allowing a greeting: "Hyper, …", "Hey Hyper …",
   "Oye Hyper …", "Hyper por favor …". The name is stripped before the model sees the rest.
3. **Otherwise** → dictation, exactly as today. The name in the middle of a sentence never routes.

The check is a string match: microseconds, no network, testable with plain unit tests.

Speech engines will hear the name wrongly, above all in Spanish ("Háiper", "Hiper", "Piper",
"Haiper"). Rather than a special list, the name is a **dictionary entry with heard forms** (spec
93), shipped by default and fixable by the user on the Dictionary page. The same mechanism that
fixes "lang chain" fixes "Hiper". The quality harness (`crates/hyperme-quality`) gets a small set
of recorded "Hyper, …" samples in both languages to measure how often the name is heard.

An escape hatch in the other direction: the model has a `just_dictation` tool. If "Hyper is the
name of our app" was dictated, the model picks it and the text is pasted as plain dictation. The
pill names the route first and ⌘Z turns any command back into plain text ([UX.md](../UX.md), "The
pill grows into cards").

### 4.2 One model call, a short tool list

The command route makes **one tool-calling request** to the cleanup provider's fast model (today
Groq `gpt-oss-20b`; the name lives in config), on the connection the dictation already warmed.
No agent runtime, no second request for the common cases.

To keep the request small and fast, a local keyword pre-filter picks the tools:

| Words after the name (English and Spanish) | Tools offered |
| --- | --- |
| remind, add, task, todo / recuérdame, apunta, tarea | `add_task`, `update_task`, `just_dictation` |
| email, mail, reply, draft / correo, email, responde, borrador | `draft_email`, `find_contact`, `just_dictation` |
| remember / recuerda que | `remember`, `just_dictation` |
| a selection exists and the words read as an edit | `edit_selection`, `just_dictation` |
| anything else | the full list, including MCP tools and `answer` |

The tool arguments are plain fields. Dates come back as the words the user said ("tomorrow at
10", "el lunes") plus the model's resolved ISO date, and Rust checks the date against its own
resolver in the Mac's time zone (spec 92, R-2). A disagreement shows on the card instead of
silently picking one.

**Start early.** The speech stream gives partial transcripts while the user is still talking. If
the partial already starts with the name, the app can open the model connection and load what the
likely tools need (today's tasks for `update_task`, the Google account for `draft_email`) before
the key is released. This is the cheapest way to stay under one second, and costs nothing if the
user was dictating.

### 4.3 Latency budget for "release to card under 1 s"

| Step | Budget | Basis |
| --- | --- | --- |
| Final transcript after release | about 300 ms | Dictation measured median 426 ms including cleanup ([results](../results/2026-11-25-eu-speech.md)) |
| Dictionary and router | under 1 ms | Spec 93, R-6; a string match |
| Tool-calling request | about 400 ms | **Not measured.** A short prompt with two or three tools on the same model as cleanup; measure it before the phase 3 spec |
| Card shown | under 50 ms | Local |

The tool-calling step is the unknown. The first slice of the phase 3 spec should time it on the
owner's samples with the `flowbench` approach before any card is built.

### 4.4 What each command shows

- **"Hyper, remind me to …" / "Hyper, create a task …"** → saved at once, done card with what was
  understood (title, day, due, blocked by), ⌘Z for 5 seconds (spec 92, R-3). No Enter, because
  nothing leaves the Mac.
- **"Hyper, draft an email to Ana about …"** → preview card laid out like Gmail (To, Subject,
  body). ↵ creates the Gmail draft, Esc leaves nothing, ⌘K offers edit, change account, open in
  Gmail. It never sends (spec 97, R-23; the gate in `actions.rs` already refuses send tools). If
  "Ana" matches two contacts, the card lists them with 1 and 2 as keys instead of guessing.
- **Change the card by voice.** Holding the key while a card is open and saying "make it shorter"
  or "to Ana's work address" regenerates the card. This borrows Wispr's chaining, but keeps the
  preview.
- **"Hyper, remember …" and anything that changes settings** → a small confirm, the one place
  Wispr also asks.

### 4.5 When it fails

- **The words are never lost.** Every command keeps its audio and transcript in dictation history,
  as dictations already do. A failed command shows a card with the reason and **"Paste as
  text"**, so the user can fall back to dictation with one key.
- **No automatic retry of a write.** A draft or a ticket is created once; a retry is the user's
  key press. Reads may retry once.
- **Offline or provider down:** a task command still saves, with the spoken words as its title and
  a "check the date" mark, since the store is local. An email command keeps a local draft in the
  card's history to send to Gmail later through the same preview.
- **The model picks no tool, or two that conflict:** fall back to `answer` for a question, or to
  plain dictation, never to a guess that writes.
- **Partial success** in a chained command lists each step with its result, as Wispr does.

### 4.6 Lightness

- No always-on listening and no wake-word model: the key starts the microphone, as today. A wake
  word stays in "Later" (PRD 7.6).
- No new process and no local model: the router is a string match, the tool loop is in-process
  (`CLAUDE.md`), MCP servers start on first use.
- Memory cost is the tool list and a card: well under a megabyte. The idle budget is untouched.

### 4.7 Later, not now

- **App Intents.** Exposing HyperMe's `add_task` through the App Intents reminders schema would let
  Siri and Shortcuts add to HyperMe's list. Worth a look once Siri AI is out of beta and in Spanish.
- **A command palette for voice.** Show the spoken forms beside each command in Home's ⌘K bar, so
  the bar teaches what Hyper understands ([UX.md](../UX.md), "The command bar").
- **Commands without the name when no text box is focused.** If the focus is not a text field, a
  dictation has nowhere to land; Wispr then uses the clipboard. HyperMe could treat it as a command
  instead. Left as an option in section 5.

## 5. The decision for the owner

**How strictly must commands start with "Hyper"?**

**Decided by the owner on 27 November 2026: option A, the name at the start only.**

| Option | Behaviour | For | Against |
| --- | --- | --- | --- |
| **A. Name at the start only** (recommended) | "Hyper, …" in the first words, or the command key, routes to a command. Everything else is dictation | Dictation is never misrouted (PRD principle 2); decided locally, in microseconds; matches what the field does | You must say the name every time |
| B. Name at the start, or no text box focused | As A, plus: speaking with no text field focused is treated as a command, name or not | "Remind me to …" works with no name when you are not in a text field | The router needs the focus state from the platform layer; a wrong focus reading misroutes |
| C. Intent from the words | The model decides on every dictation whether it was a command | The most natural: "remind me to …" anywhere becomes a task | Every dictation pays a model call; "remind me to call you" dictated into Slack becomes a task. No product in the field does this |

Option A follows the decided Q-3 with no change. B can be added later without rework, because it is
one more input to the same router.

## Sources

Read on 27 November 2026: [Wispr Command Mode](https://docs.wisprflow.ai/articles/4816967992-how-to-use-command-mode),
[Wispr chaining in Realtime](https://docs.wisprflow.ai/articles/9701824265-chaining-multiple-actions-in-a-single-realtime-voice-command),
[Wispr features](https://wisprflow.ai/features),
[Aqua Realtime](https://aquavoice.com/realtime), [Aqua guide](https://aquavoice.com/guide),
[Superwhisper modes](https://superwhisper.com/docs/modes),
[Superwhisper switching modes](https://superwhisper.com/docs/modes/switching-modes),
[Willow](https://willowvoice.com/use-cases/hey),
[Raycast dictation](https://manual.raycast.com/ai/dictation), [Raycast 2.11](https://raycast.com/changelog/macos/2-11),
[Raycast AI](https://manual.raycast.com/ai.md),
[Apple Siri AI](https://apple.com/ca/newsroom/2026/06/apple-introduces-siri-ai-a-profoundly-more-capable-and-personal-assistant),
[App Intents reminders schema](https://developer.apple.com/documentation/appintents/app-schema-domain-reminders.md),
[Talon modes](https://talon.wiki/), [Talon community modes](https://wolfmanstout.github.io/wolfmanstout_talon/core/modes/),
[VoiceOS guide](https://www.voiceos.com/compare/wispr-flow) (vendor).
