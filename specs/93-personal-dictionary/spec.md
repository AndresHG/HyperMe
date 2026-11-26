# Personal dictionary

**Status:** draft, 26 November 2026. Needs the owner's approval before production code. It is
Phase 2 work ([PRD](../../docs/PRD.md) 7.1 and 7.5, P0), so slices after D1 start only after
the Phase 1 gate passes ([tasks.md](tasks.md) says which slice could go earlier).
**Issue:** [#93](https://github.com/AndresHG/hyperme/issues/93), part of
[#26](https://github.com/AndresHG/hyperme/issues/26). **Size:** L.
**Design:** [design.md](design.md). **Slices:** [tasks.md](tasks.md). **Mockup:**
[dictionary.svg](../../docs/ux/dictionary.svg).

## Problem

No speech engine can know the user's own names: colleagues, internal products, clients'
jargon, the spelling of a library. On the owner's 50 samples, the live pipeline got 0 of 7
internal names right, and Wispr Flow with the owner's dictionary got 4 of 7
([results](../../docs/results/2026-11-26-dictionary.md)). Replacing sound-alikes in the raw
transcript before the cleanup, with a list learned from corrections, got 7 of 7, 8 of 50
samples exact and 6.5 edits per 100 characters against 7.6, at no cost in speed.

The pipeline code for that exists only in the `spikes/flowbench` spike. The app has a
`dictation.dictionary` config key that nothing reads, no way to add a word without editing a
text file, and no way to learn from the corrections the user already makes.

## Goal

- The user's own words come out right every time after one correction, with no measurable cost
  in speed.
- Adding a word takes one step from wherever the user notices the mistake: History, the
  Dictionary page, or simply fixing the text where it was pasted.
- Nothing is learned that the user would not have chosen, and nothing leaves the Mac that the
  privacy boundary does not already allow (`CLAUDE.md`).

## Terms

- **Entry:** one word or phrase as it should be written (`Zentrix`, `LangChain`, `Ana Ruiz`).
- **Heard form:** how the speech engine gets it wrong (`sun trix`, `zen trix`). An entry has
  zero or more.
- **Substitution:** replacing a heard form in the raw transcript with its entry, on the Mac,
  before the cleanup.
- **Suggestion:** an entry or heard form that HyperMe noticed and that waits for the user's yes.

## Requirements

EARS form. "The app" is HyperMe running on macOS.

### Using the dictionary in a dictation

- R-1. When the final transcript arrives, the app shall replace every heard form in it with its
  entry before anything else uses the transcript: the cleanup, the short-dictation path (spec
  25, R-13) and the raw-text fallback (spec 25, R-12, R-20).
- R-2. Substitution shall match whole words or phrases only, ignoring case and accents, the
  longest heard form first, in one pass, so a replaced entry is never matched again.
- R-3. Substitution shall also fix an entry heard with the wrong case or split into words
  (`langchain`, `lang chain` become `LangChain`), unless the entry is a common word (R-17).
- R-4. The app shall give the cleanup model the written form of the entries in the system
  prompt, after the fixed prompt and before the app name, in a stable order, so the prompt's
  start does not change from one dictation to the next.
- R-5. If the dictionary has more entries than the prompt cap (default 150, configurable), the
  app shall put in the prompt only the entries that look relevant to this transcript (design.md,
  "Prompt").
- R-6. Substitution and choosing the prompt entries together shall take under 1 ms for 1,000
  entries and a 120 s dictation on the owner's Mac.
- R-7. The app shall not send the dictionary to the speech provider in this version. A later
  slice measures it (tasks.md, D8).
- R-8. The app shall keep the dictionary in memory and reload it only when it changes. No
  dictation shall read it from disk.
- R-9. The timing log shall record how many substitutions a dictation made, never which words.

### Adding and editing words

- R-10. The Settings window shall have a **Dictionary** section, one click from the menu-bar
  menu ("Dictionary…"), that lists the entries with their heard forms, where they came from and
  how often they were used, with search, add, edit and delete (owner, 26 November 2026: "a
  section in the settings to add more words, similar to Wispr Flow"). When the Home window
  exists, the same page also appears in its sidebar; both show and edit the same dictionary.
- R-10a. A change made on the Dictionary page shall take effect from the next dictation, with
  no restart.
- R-11. When the user adds an entry, the app shall ask for the written form and, optionally, the
  heard forms, and shall use it from the next dictation.
- R-12. When the user selects words in a dictation in History and chooses **Fix**, the app shall
  add the selection as a heard form of the entry the user types, at once, and offer to paste the
  corrected text again.
- R-13. The Dictionary page shall import and export a plain text file (one entry per line,
  `Entry: heard, heard`) and a two-column CSV (`heard,entry`, or one column for an entry alone).
- R-14. If the app finds the `dictation.dictionary` file from the config on start and the
  dictionary is empty, it shall import that file once.
- R-15. The app shall refuse an entry or heard form longer than 60 characters or 6 words, a
  duplicate, or more than 1,000 entries, and say why.

### Learning from corrections

- R-16. While "Learn from my edits" is on, when the user changes the text HyperMe just pasted
  within 60 seconds, without leaving the field, the app shall compare the pasted words with the
  words now there and keep each short replacement (1 to 3 words for 1 to 3 words) that looks
  like a fix of a misheard word as a candidate (design.md, "Learning").
- R-17. A heard form that is a common word (in the built-in list, or seen three times or more in
  the user's own dictations without being corrected) shall never be added without the user's yes
  on that exact form, and the add dialog shall warn that it will be replaced everywhere.
- R-18. The first time a candidate is seen, the app shall show it as a suggestion. When the same
  candidate is seen a second time, the app shall add it and mark it as learned, with undo
  (owner, 26 November 2026).
- R-19. When the user dismisses a suggestion or deletes a learned heard form, the app shall
  never suggest that heard form for that entry again.
- R-20. The app shall never read a secure text field, never read a field while secure input is
  on, and stop watching when the field loses focus, the next dictation starts, or 60 seconds
  pass.
- R-21. What the app reads from the field shall stay on the Mac and shall not be stored, except
  the candidate pair itself.
- R-22. When suggestions wait, the Home page shall show one line that leads to them. The pill
  shall show nothing.

## Acceptance criteria

*(fake)* runs in CI with local fakes; *(live)* is measured on the owner's Mac and written to
`docs/results/`.

- AC-1 *(fake)*: Heard forms are replaced as whole words, ignoring case and accents, longest
  first, in one pass; text inside longer words and entries already written right are left alone.
  (R-2)
- AC-2 *(fake)*: The cleanup, the short-dictation path and the raw-text fallback all receive the
  substituted transcript. (R-1)
- AC-3 *(fake)*: `langchain` and `lang chain` become `LangChain` with no heard forms saved; a
  common-word entry is not changed this way. (R-3, R-17)
- AC-4 *(fake)*: The cleanup request carries the entries in the system prompt between the fixed
  prompt and the app line, alphabetical, and the same dictionary gives the same prompt start for
  two different apps. (R-4)
- AC-5 *(fake)*: With more entries than the cap, the prompt carries only the relevant entries,
  and never more than the cap. (R-5)
- AC-6 *(fake, benchmark)*: Substitution plus prompt choice over 1,000 entries and a 2,000-word
  transcript takes under 1 ms in release mode. (R-6)
- AC-7 *(live)*: On the owner's 50 samples with the owner's learned dictionary, the names are 7
  of 7 right and edits per 100 characters are no worse than the 6.5 of the dictionary results;
  key release to text stays within the Phase 1 budget (median under 800 ms) with 0, 150 and
  1,000 entries. (R-1, R-4, R-5)
- AC-8 *(fake)*: Changing the dictionary takes effect on the next dictation, with no restart and
  without reading the disk during a dictation. (R-8, R-10a, R-11)
- AC-9 *(fake)*: The timing log line has a substitution count and no dictionary words. (R-9)
- AC-10 *(fake)*: Import and export round-trip the text file and the CSV; over-long items,
  duplicates and item 1,001 are refused with a reason. (R-13, R-15)
- AC-11 *(fake)*: An empty dictionary imports the configured text file once; a second start
  does not import it again. (R-14)
- AC-12 *(fake)*: Fix from History adds the heard form at once, and the next dictation of the
  same audio gets the entry. (R-12)
- AC-13 *(fake)*: From fixture pairs of pasted and edited text, the candidate finder keeps
  misheard-word fixes (`sun trix` to `Zentrix`, `corbit` to `Quorbit`) and drops rewrites,
  added sentences, punctuation-only and deleted-word changes. (R-16)
- AC-14 *(fake)*: A candidate seen once is a suggestion; seen twice it is added as learned; a
  common heard form stays a suggestion; a dismissed one never comes back. (R-17, R-18, R-19)
- AC-15 *(fake)*: The watcher never reads a secure field or a field under secure input, and stops
  on focus change, on the next dictation and after 60 s. (R-20)
- AC-16 *(live)*: With learning on, idle CPU stays near 0% and idle memory under the 100 MB
  budget; a paste followed by 60 s of typing costs under 1% CPU on average. (R-20, budgets)
- AC-17 *(live)*: In TextEdit, Notes, Slack, Mail, Chrome and VS Code, record which apps let the
  watcher see an edit. Where it cannot, nothing breaks and Fix from History still works. (R-16)

## Non-goals

- Sending the dictionary to the speech provider (measured later, D8).
- Snippets (a spoken shortcut that expands to saved text): a separate P2 feature in PRD 7.1.
- Per-app or per-language dictionaries. One dictionary; replacements are not language-aware, as
  in Wispr Flow ([docs](https://docs.wisprflow.ai/articles/4052411709-teach-flow-your-words-with-the-dictionary)).
- Syncing between devices or a team dictionary.
- Learning from window text, screenshots or mail (memory, PRD 7.5, is separate).
- "Hyper, add … to the dictionary" by voice: it comes with commands in Phase 3, on top of R-11.

## Pre-mortem

"It is three months later and the dictionary failed. Why?"

| Cause | Answer in this spec |
| --- | --- |
| A learned heard form was a common word, and it rewrote normal sentences (the results already found two such Spanish words) | Common-word guard (R-17, AC-14); learning needs two sightings (R-18); dismissals stick (R-19) |
| The prompt grew with the dictionary and the first token got slower | Cap and relevance filter (R-5); stable prompt start for Groq's prefix cache (R-4); AC-7 measures 0, 150 and 1,000 entries |
| Watching edits cost CPU or memory at rest | Event-driven watcher, only for 60 s after a paste (R-20, AC-16) |
| The watcher saw nothing in the apps the owner lives in (Electron apps expose text poorly) | AC-17 records it per app; History Fix (R-12) works everywhere |
| Users felt watched | Off switch on the Dictionary page; field text never stored or sent (R-21); secure fields never read (R-20) |
| The owner's existing text file was lost on the move to the app | One-time import (R-14, AC-11), export any time (R-13) |
| The dictionary leaked into the public repository or a log | Stored only in the app's data folder; the log has counts only (R-9) |

## Decisions

| Decision | Chosen | Rejected, and why | Who |
| --- | --- | --- | --- |
| Where the dictionary acts | Substitution on the Mac before the cleanup, plus the entries in the prompt | Speechmatics `additional_vocab`: names right too, but the `enhanced` model it was tested with mishears more of everything else (9.3 against 7.6 edits per 100 characters) and needed the language up front ([results](../../docs/results/2026-11-26-dictionary.md)) | Data, 26 November 2026; owner to confirm |
| Where it is kept | SQLite in the app's data folder, with text and CSV import and export | A text file as the only store: no room for use counts, suggestions and dismissals without a second file | Proposed |
| How learning is approved | Suggestion first, added on the second sighting, common words always asked | Silent add at once (Wispr Flow's learned words carry a marker; whether it asks is not in its docs); always ask (more taps than users give) | Owner, 26 November 2026 |
| Learn from edits default | On, with a switch on the Dictionary page | Off by default: the field text never leaves the Mac, and without it the feature mostly does not happen | Proposed |

## Open questions for the owner

1. ~~How learning is approved~~ (R-18): decided 26 November 2026, twice, then on its own:
   a correction seen once is a suggestion, seen twice it is added, marked learned, with undo;
   common words always ask.
