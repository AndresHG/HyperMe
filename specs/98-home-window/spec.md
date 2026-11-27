# Home window and History

**Status:** approved by the owner, 27 November 2026. Amended the same day for the V0.1 redesign
(owner approved the mockups, 27 November 2026): History moves into the Home page as in Wispr Flow,
and the day gets its own Today section. R-5, R-6, R-8, R-9, R-10, AC-4, AC-5 and AC-7 changed.
**Issue:** [#98](https://github.com/AndresHG/hyperme/issues/98). **Size:** L.
**Plan:** chunk P2-5 in `docs/plans/2026-11-26-phases-1-2.md` (#94).
**UX:** [docs/UX.md](../../docs/UX.md), "Home", "Home sections" and "Where each thing lives"; mockup
[ux/home-window.svg](../../docs/ux/home-window.svg). **Slices:** [tasks.md](tasks.md).
**Shared with:** the personal dictionary (#93, owns the Dictionary section and the store) and
daily tasks (#92, owns the Waiting for you and Today blocks). The hand-offs below were agreed
with both on 26 November 2026.

## Problem

Phase 1 keeps every dictation on the Mac (audio and text, R-18), but the only ways back to it are
"Paste last dictation" and "Retry last dictation" in the menu. Nobody can find what they said an
hour ago, fix a word the speech model keeps getting wrong, or retry an older dictation. Phase 2
also needs a place for the Dictionary and Style pages, and phase 4 for Today. UX.md puts all of
that in one window, Home, in the style of Wispr Flow's main window.

The risk is memory. The app idles at 57 to 58 MB with the pill's web view
([results](../../docs/results/2026-11-26-shell-idle-memory.md)), against a budget of 100 MB. A
second web view that stays loaded would eat most of what is left, so Home must cost nothing
while it is closed.

## Requirements

EARS form. "The app" is HyperMe on macOS. "Home" is the Home window.

### The window

- R-1. When the user chooses **Open HyperMe** in the menu-bar menu, or opens the app while it
  runs (a double-click in Finder, `open -a HyperMe`) after the walkthrough is done, the app shall
  create Home and bring it to the front. If Home is already open, it shall bring it to the front.
- R-2. When the user closes Home, the app shall destroy its window and web view, not hide it.
- R-3. While Home is open, the app shall show a Dock icon and an app menu; when Home closes and no
  other window of the app is open, the Dock icon shall go away (this narrows Phase 1 R-22 to
  "while no window is open").
- R-4. Home shall be a normal, activating window with a sidebar and a page. It shall follow the
  system's light or dark mode, and remember its size and position between openings.
- R-5. The sidebar shall list only the sections whose feature exists, in this order: Home, Today,
  Dictionary, Style, Meetings, with Settings and Help at the bottom. ⌘1 to ⌘5 shall open the
  sections by position, and ⌘, shall open Settings.
- R-6. The app shall open Home at a target: a section id (`today`) or the id of a block on the
  Home page, which opens the Home page scrolled to that block. Other parts of the app call it with
  the target (the morning card's "Open Today" opens `today`). An unknown target, `history`
  included, opens the Home page. The `hyperme://home/…` URL form of the same targets comes with
  the URL scheme (non-goals).

### The Home page

- R-7. The Home page shall show a greeting and two numbers from the history: words dictated this
  week (Monday to now, local time), and time saved against typing at 40 words a minute, net of the
  time spent speaking.
- R-8. The Home page shall show the user's dictations as in Wispr Flow: a search field, then
  every dictation grouped by day (Today, Yesterday, then the weekday and date), newest first
  (R-10 to R-19 describe the list). Above the list, one **Up next** line shall show the next
  meeting and what is left in Today, and open the Today section.
- R-9. The Home page shall have an ordered list of blocks between Up next and the list, each with
  an id and a test of whether it shows. None ships now; the day lives in the Today section (#92).

### History

- R-10. The Home page's list shall hold every dictation in the history, newest first, grouped by
  day, loading 50 at a time as the user scrolls. ("History" below means this list.)
- R-11. Each row shall show the time, the app it was dictated into, and the text on one line,
  or for a failed dictation the reason it failed. The full text shows when the row is selected.
- R-12. When the user types in the search field, History shall show only the dictations whose
  text contains every word typed, ignoring case and accents ("cancion" finds "canción"), with the
  matches marked.
- R-13. For a selected row, History shall offer, each with a key: **Copy** (⌘C), **Paste again**
  (↵), **Retry** (⌘R), **Fix a word** (⌘F, only while a dictionary exists, R-17) and **Delete**
  (⌘⌫). ⌘K shall list the same actions with their keys.
- R-14. When the user chooses Copy, the app shall put the dictation's text on the clipboard.
- R-15. When the user chooses Paste again, the app shall return to the app that was in front
  before Home took focus and paste the text there, as a dictation is pasted (Phase 1 R-15, R-16).
  Home stays open behind it. If that app is gone, the app shall copy the text and say so.
- R-16. When the user chooses Retry on a dictation that has its audio, the app shall send the
  saved audio through speech and cleanup again, as Phase 1 R-31 does, replace the row's text with
  the new text, and copy it to the clipboard. The row shows that it is working, and an error in
  the row if it fails. A dictation with no audio left (history off, or deleted by the size limit)
  shall not offer Retry.
- R-17. When the user chooses Fix a word, History shall let the user select a word or phrase in
  the text, type what it should be, and send it to the dictionary as a correction. When the
  dictionary warns that the word is common and would be replaced everywhere, History shall show
  that warning and send it again only if the user confirms. When it succeeds, History shall
  update the row's text with the correction and offer Paste again.
- R-18. When the user chooses Delete, the app shall ask once in the row ("Delete this dictation
  and its audio? ↵ Delete · Esc Keep"), then delete the item's text and audio (Phase 1 R-28).
- R-19. While history is off, History shall say so in one line with a link to Settings → Privacy,
  and list only the failed dictations still waiting for a retry (Phase 1 R-31).

### Shared with other threads

- R-20. The core shall define a `Dictionary` trait with
  `fn add_correction(&mut self, c: Correction) -> Result<Added, Error>`, where
  `Correction { heard, wanted, dictation: Option<DictationId>, language: Option<String>,
  confirmed_common: bool }` and `Added` is `Added` or `NeedsConfirm(Refusal::CommonWord)`. Hard
  refusals (too long, duplicate, list full) come back as `Err` with a message to show. The
  dictionary thread (#93) owns the implementation, the storage and every other method.
- R-21. The app shall hold the dictionary as an optional part. While it has none, nothing in Home
  mentions the dictionary.
- R-22. The app shall expose to the other threads: a list of sections (id, title, page, position),
  a list of Home blocks (R-9), `home_attention(bool)` that shows or hides one violet dot on Home
  in the sidebar, a `home_block_viewed` event when a block has been on screen, and a push event
  that makes the page reload a block while Home is open.

### Memory and privacy

- R-23. Home shall read and write only the local history and the dictionary. It shall make no
  network call except the speech and cleanup calls of a retry, to the providers already
  configured (Phase 1 R-28, R-29).
- R-24. Home's page shall get dictation text only through the app's commands, never from a file
  URL, and shall keep none of it in the web view's storage.

## Acceptance criteria

*(fake)* criteria run in CI with local fakes; *(live)* ones are measured by hand on the owner's Mac
and recorded in `docs/results/`.

- AC-1 *(live)*: Open HyperMe brings up Home with the Home page; opening it again while open
  raises the same window, never a second one. (R-1)
- AC-2 *(live)*: Total footprint of all the app's processes, measured with `footprint`: 30 seconds
  after Home closes, under the 100 MB idle budget and Home's WebContent process gone; five more
  open and close cycles do not grow it by more than 5 MB; with Home open on 1,000 history items,
  under 130 MB on any page. (R-2, budget; owner, 26 November 2026, after the first measurement)
- AC-3 *(live)*: The Dock icon shows while Home is open and goes away after it closes; the pill
  still never takes focus while Home is open. (R-3, Phase 1 R-23)
- AC-4 *(fake)*: The section list is Home, Today and, once a dictionary exists, Dictionary, with
  ⌘1 to ⌘3; with no dictionary, there is no Dictionary entry and no Fix a word. (R-5, R-21)
- AC-5 *(fake)*: `today` resolves to the Today section, and `history` or any unknown target to the
  Home page. (R-6)
- AC-6 *(fake)*: On a fixed clock and fixed history, words this week and time saved match a hand
  count, including a week that starts on a Monday at midnight local time. (R-7)
- AC-7 *(fake)*: The Home page's list groups items by local day with the right labels, and page in
  fifties without repeating or skipping an item when a new dictation arrives between pages.
  (R-8, R-10)
- AC-8 *(fake)*: Search for "cancion reunion" on a fixture of Spanish and English items finds
  exactly the items holding both words in any case or accent, and returns in under 50 ms on 10,000
  items on the owner's Mac. (R-12)
- AC-9 *(fake)*: Copy puts the text on the fake clipboard; Paste again sends the text to the fake
  inserter after focus goes back to the recorded app, and copies with a notice when that app is
  gone. (R-14, R-15)
- AC-10 *(fake)*: Retry of a done dictation against working fakes replaces its text in the store
  and copies the new text; against a failing fake, the old text stays and the row shows the
  error; a dictation without audio offers no Retry. (R-16)
- AC-11 *(fake)*: Fix a word sends the exact selection and the typed word to a fake dictionary;
  `NeedsConfirm` shows the warning and a confirm sends again with `confirmed_common = true`; an
  `Err` shows its message and changes nothing. (R-17, R-20)
- AC-12 *(fake)*: Delete removes the row's text and its audio file. (R-18)
- AC-13 *(fake)*: With history off, History lists only failed items with audio and shows the line
  linking to Privacy. (R-19)
- AC-14 *(fake)*: The page's commands never return a provider key or a file path to audio, and a
  test over the page source finds no `localStorage`, `sessionStorage` or `indexedDB`. (R-23, R-24)
- AC-15 *(live)*: With Home open, a dictation into another app still pastes there, and the new
  item appears at the top of the Home page's list without reopening Home. (R-8, R-10, R-22)

## Non-goals

- The Dictionary and Style sections and their pages: #93 and P2-3. This change only gives them a
  place.
- The Waiting for you and Today blocks, and the tasks behind them: #92.
- The ⌘K command bar across Home (UX.md, "The command bar"). ⌘K here lists only the selected
  row's actions.
- The `hyperme://` URL scheme (`hyperme://home/today`, `hyperme://settings/<section>`): it needs
  a URL-scheme crate and bundle changes, and serves Settings too; its own change.
- A global shortcut to open Home (⌥⌘Space in UX.md is still a proposal): a later change, once
  shortcuts are configurable.
- Playing a dictation's audio, editing its text by hand, exporting history.
- Filtering by kind (dictations, questions, actions): questions and actions arrive in phase 3.
- The median latency number on the Home page: it needs the timing log, which is off by default.
- Linux and Windows. The window code is behind `cfg(target_os = "macos")` like Settings; the
  history queries and the stats are portable.

## Pre-mortem

"It is three months later and Home failed. Why?"

| Cause | Answer in this spec |
| --- | --- |
| Home quietly stayed in memory after closing and broke the idle budget | Destroy on close (R-2) and a live check over five cycles (AC-2) |
| History was slow to open or to search after a month of use | Paging (R-10), search on the Rust side with a 50 ms bound on 10,000 items (AC-8) |
| Paste again pasted into Home itself or the wrong app | Record the app in front before Home takes focus, return to it first, copy when it is gone (R-15, AC-9) |
| Retry from Home clashed with a dictation in progress | Retry goes through the same state machine as dictation, so a retry while dictating waits or is refused as Phase 1 does; the row says so |
| The dictionary and tasks threads each built their own window or sidebar | One section list and block list owned here, agreed with both (R-20 to R-22) |
| "Fix a word" taught the dictionary a common word and broke every later dictation | The dictionary's `NeedsConfirm` warning is shown and must be confirmed (R-17, AC-11) |
| Text leaked through the web view's storage or a file URL | Commands only, no storage, and a test over the page (R-24, AC-14) |

## Decisions

| Decision | Chosen | Rejected and why | Who |
| --- | --- | --- | --- |
| Life of the window | Create on open, destroy on close | Hide on close: faster to reopen but keeps a web view (11 MB or more) loaded all day | UX.md principle 6 |
| Search | Fold case and accents in Rust over the indexed texts, at most a few thousand items in 30 days | SQLite FTS5: needs a second table kept in sync and a custom tokenizer for accents, for no gain at this size | This spec |
| Paste again | Go back to the app that was in front, paste, leave Home open behind | Copy only: one more step for the user every time | This spec; owner to confirm |
| Retry of a done dictation | Replace the text, copy the new text | Keep both versions: more UI for a rare case | This spec |
| Correction type | The dictionary thread's `Correction` and `Added` (R-20) | A plain heard/wanted pair: it could not carry the common-word warning | Agreed with #93 |
| Memory target | The 100 MB budget applies at idle, after Home closes; Home open may reach 130 MB | Under 100 MB while open: WebKit costs about 23 MB for any window, even an empty one, and the open window measured 108 to 125 MB after fixes | Owner, 26 November 2026 |
| Home page numbers | Words this week and time saved | Median latency: needs the timing log, off by default | This spec |
