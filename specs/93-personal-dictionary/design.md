# Personal dictionary: design

Goes with [spec.md](spec.md). Draft, 26 November 2026.

## Where it sits in the pipeline

```
key up ─► Speechmatics final transcript (raw)
            │
            ▼
   Dictionary::apply(raw)            on the Mac, microseconds (R-1, R-2, R-3)
            │  substituted raw
            ├──────────────► short dictation (spec 25, R-13) ─► paste
            ▼
   Cleanup request (Groq)            system = prompt v4 + entries line + app line (R-4)
            │                         user   = substituted raw
            ├── deadline missed ───► substituted raw (spec 25, R-12) ─► paste
            ▼
          paste ─► FieldWatcher, up to 60 s (R-16, R-20) ─► candidates ─► suggestions
```

Substitution runs once, on the raw transcript, so every later path gets the fixed names,
including the two that never see the model. That is a change from flowbench, where the
substitution lived inside a wrapper around the cleanup provider (`vocab::Replacing`): a
short dictation or a missed deadline would paste the uncorrected name.

The dictionary lives in `hyperme-core` (`dictionary` module: entries, matcher, prompt line,
candidate finder), with no platform code. Storage is in `hyperme-platform` (SQLite, next to the
history). The field watcher is a trait in `hyperme-core::traits` with a macOS implementation in
`hyperme-platform`, so Linux and Windows can follow.

## Data model

```
Entry      { id, written, kind_hint, source, created_ms, uses, last_used_ms }
HeardForm  { entry_id, heard, source, created_ms, uses }
Suggestion { entry_written, heard, seen, first_seen_ms, last_app }
Dismissed  { entry_written, heard }
```

- `source` is `manual`, `history_fix`, `learned` or `imported`. The page shows learned items
  with a small mark, as Wispr Flow does for its auto-learned words
  ([docs](https://docs.wisprflow.ai/articles/8955301725-how-do-i-bulk-import-for-dictionary-and-snippets)).
- `uses` counts dictations where a heard form fired or the entry appeared in the final text.
  It orders the page and helps the relevance filter; it never leaves the Mac.

Storage: `dictionary.sqlite` in the app's data folder (`~/Library/Application Support/HyperMe/`).
Not in the history database, because history cleanup deletes rows and the dictionary must
outlive every retention setting. SQLite is already a dependency (`rusqlite`, bundled).

Limits follow Wispr Flow's and Speechmatics', so an import from either works and a later
`additional_vocab` slice needs no new rules: 60 characters per item, at most 6 words per heard
form (Speechmatics drops longer ones), 1,000 entries
([Wispr](https://docs.wisprflow.ai/articles/4052411709-teach-flow-your-words-with-the-dictionary),
[Speechmatics](https://docs.speechmatics.com/features/custom-dictionary)).

## The matcher

Port of `spikes/flowbench/src/vocab.rs` (`replace`, tested in issue 34), with three changes:

1. **Accent and case folding** when comparing (`á é í ó ú ü` fold to plain letters; `ñ` does
   not, since `año` and `ano` are different words). Speechmatics sometimes accents a name it
   does not know.
2. **Implicit forms** for each entry, unless the entry itself is a common word: the entry in any
   case (`zentrix`), and, for a CamelCase or spaced entry, its parts as separate words
   (`lang chain` for `LangChain`). They cost nothing to store and fix the most common miss with
   no heard forms at all.
3. **Built once, used many times.** The variants are folded, sorted longest first and grouped by
   first folded character when the dictionary changes. A dictation only scans. At 1,000 entries
   with three forms each this is a few thousand comparisons per word start, well under the 1 ms
   of R-6; if the benchmark (AC-6) says otherwise, swap in the `aho-corasick` crate (MIT or
   Unlicense) behind the same function.

The `Dictionary` holds the compiled matcher behind an `Arc`. Saving a change builds a new one
and swaps it; a dictation in flight keeps the one it started with (R-8, AC-8).

## Prompt

Today's prompt (`crates/hyperme-providers/src/cleanup.rs`) is `v4`: `v3` plus one rule, "When a
word or short phrase clearly sounds like a term in the speaker's dictionary, write the
dictionary term." The request builds the system message from the prompt and then the app line.

The entries line goes in between, written as flowbench writes it:

```
<prompt v4>

The speaker's dictionary: Ana Ruiz, LangChain, Quorbit, Zentrix.

The text will be pasted into Slack.
```

- **Order matters for speed.** Groq caches exact prefixes for `openai/gpt-oss-20b`,
  automatically, for two hours without use, at half price for cached tokens
  ([docs](https://console.groq.com/docs/prompt-caching)). Prompt plus a sorted entries line is
  the same from one dictation to the next; the app line, which changes with the app, comes
  after it. The cleanup warm-up on key-down (spec 25, R-2) can send the same prefix so the cache
  is hot before the transcript arrives. Groq's docs (as read through a summary, not checked
  word for word) give a minimum cacheable length of 128 to 1,024 tokens depending on the model,
  so a short prompt may not be cached at all; slice D7 measures it with
  `usage.prompt_tokens_details.cached_tokens`.
- **Size.** An entry costs about 3 tokens. 150 entries add about 450 tokens to a prompt of about
  150: small next to Groq's prefill speed, but not free. Up to the cap (150, configurable) the
  line carries every entry, alphabetical, so it stays cacheable.
- **Above the cap**, the line carries only entries that look relevant: entries whose heard form
  fired in this transcript, then entries with a character-trigram overlap with any 1 to 3 word
  window of the transcript above a threshold, most used first, up to 50. This changes the prefix
  per dictation, which is why it applies only above the cap. The results showed the prompt rule
  adds little on top of substitution (row d against b in the
  [results](../../docs/results/2026-11-26-dictionary.md)), so a small, relevant list loses
  nothing.
- The entries are user text inside the system prompt. Each entry is printed as plain text with
  commas and line breaks stripped, and the list is capped, so an entry cannot add instructions
  beyond a few words.

## Speech provider

Not in this version (R-7). The evidence:

- On 50 samples, `additional_vocab` with Speechmatics `enhanced` and a given language got the
  names, but 9.3 edits per 100 characters against 7.6 for `melia-1` without a dictionary.
- Our live pipeline uses `melia-1` with `language: "multi"` (spec 25, R-10). Flowbench's notes say
  `additional_vocab` needs a single-language model; Speechmatics' current page shows only an
  `enhanced` example and does not list which models take it, so whether `melia-1` with `multi`
  accepts it is unverified.
- Speechmatics caches each new dictionary for 24 hours and says the first session with a new one
  is slower ([docs](https://docs.speechmatics.com/features/custom-dictionary)). A dictionary that
  changes after every learned word would pay that start-up cost often, against our 50 ms
  key-down budget.

Slice D8 tries `melia-1` + `multi` + `additional_vocab` on the 50 samples and the session start
time. It is adopted only if it beats substitution alone.

## Learning

### What counts as a correction

The watcher sees the field's text after HyperMe pasted into it (macOS details below) and hands
the core two strings: what was pasted, and what the same stretch of the field holds when the
watch ends. The core:

1. Splits both into words and aligns them (word-level LCS; pasted dictations are short).
2. Keeps each replaced run of 1 to 3 words by 1 to 3 words. Drops inserted runs, deleted runs,
   changes in punctuation only, and any edit where more than 40% of the pasted words changed
   (a rewrite, not a fix).
3. Keeps a run as a candidate when it looks like a misheard word: the new text is not already
   in the pasted dictation, and either the folded letters are close (character-bigram
   similarity of at least 0.4, `sun trix` to `zentrix`) or the new text looks like a name (has
   a capital letter or a digit, or is not in the common-word list) and the old text is short.
   A case-only change (`zentrix` to `Zentrix`) is a spelling-only candidate.
4. Maps the old text back to the raw transcript, since the cleanup may have changed it: the
   raw words at the same position (via the alignment of raw to cleaned text) become the heard
   form, and the pasted words are kept as a second heard form if they differ.

Fixture tests (AC-13) hold the line between fixes and edits. Wispr Flow learns words "from apps,
edits, or meetings" and OpenWhispr has an "Auto-learn from corrections" setting that "watch[es]
for the moment you fix a transcription in the app you dictated into"
([Wispr](https://docs.wisprflow.ai/articles/8955301725-how-do-i-bulk-import-for-dictionary-and-snippets),
[OpenWhispr](https://docs.openwhispr.com/help/customise/custom-dictionary.md)). Neither documents
how it detects an edit, so the method above is ours.

### From candidate to entry

Decided by the owner on 26 November 2026 (twice, then on its own):

| Situation | What happens |
| --- | --- |
| A candidate seen for the first time | Suggestion on the Dictionary page; one line on the Home page |
| The same candidate seen a second time | Added, marked learned; the suggestion goes |
| The heard form is a common word | Stays a suggestion, however often it is seen; adding it shows the warning |
| The user dismisses a suggestion, or deletes a learned heard form | Remembered in `Dismissed`; never suggested again |
| A Fix from History | Added at once: the user chose it |

Common words are the built-in list (a few hundred of the most frequent Spanish and English
words, written by us, so no data licence is involved) plus any word seen in three or more of the
user's dictations that were not corrected there. The second part needs no list at all, comes from
the user's own speech, and covers the two common Spanish words that the results flagged.

### The watcher on macOS

`FieldWatcher` trait: `watch(pasted: &str) -> WatchHandle`, ending with the field text or
nothing. macOS implementation in `hyperme-platform`:

- Right after paste, take the focused element (`AXFocusedUIElement`, as the paste code does for
  the secure-input check). If its role is `AXSecureTextField`, secure input is on, or `AXValue`
  cannot be read, stop: nothing is learned from this paste (R-20).
- Register an `AXObserver` for `kAXValueChangedNotification` and
  `kAXFocusedUIElementChangedNotification`. No polling; the watcher costs nothing until the user
  types.
- On each value change, debounce 1 s, read `AXValue`, find the pasted stretch (by the text
  around it and the insertion point recorded at paste) and keep only that stretch. The rest of
  the field is never kept (R-21).
- End at focus change, at the next dictation's key-down, or 60 s after paste; hand the last
  stretch to the core; drop the observer.

Electron and web apps expose `AXValue` unevenly. AC-17 records which of the owner's apps work;
where none does, Fix from History is the way in.

## UX

Mockup: [dictionary.svg](../../docs/ux/dictionary.svg). Home is the light or dark system window
from [UX.md](../../docs/UX.md); the pill and cards stay dark and show nothing about the
dictionary (R-22).

**Where it lives.** The owner asked for the dictionary in Settings, as Wispr Flow has it (its
Dictionary sits in its main window's sidebar, beside Settings,
[docs](https://docs.wisprflow.ai/articles/5096240724-navigating-the-wispr-flow-app-desktop-ios-and-android)).
So the page ships first as a **Dictionary** section of the Settings window, which exists today,
with "Dictionary…" in the menu-bar menu. When Home ships, the same page is also Home's ⌘3
section; Settings keeps its entry. One page, two doors, so it is easy to find from either.

**Dictionary page** (Settings → Dictionary, later also ⌘3 in Home):

- Title "Dictionary", subtitle "Words HyperMe should always get right."
- A search field, **Add word** (keycap N) and a `⋯` menu with Import and Export.
- **Suggested** block on top, only while something waits: one line each,
  `next js → Next.js · fixed once in Slack`, with the keycaps `↵ Add` and `⌫ Dismiss`.
- The list: one line per entry. The written form, then "heard as sun trix, zen trix" in grey,
  a mark for learned ones, and the use count on the right. Hover or ⌘K: Edit, Delete. Sorted by
  most used; search matches the entry and its heard forms.
- At the bottom, a switch: **Learn from my edits** ("After a dictation, HyperMe notices when you
  fix a word. What it reads stays on your Mac.").
- Empty state: "No words yet. Add names, products and jargon, or fix a word from History."

**Add or edit**, a small sheet: **Word** (`Zentrix`), **Heard as** (optional, comma separated,
"How it comes out wrong. Leave empty if HyperMe only needs the spelling."). A common heard form
shows, inline: "'sun' is a common word. HyperMe will replace it everywhere." and the button
becomes **Add anyway**.

**History → Fix** (⌘D): select words in a dictation, press Fix, type the right form, ↵. It is
added at once, and the row offers **Paste again** with the corrected text. The Home window and
History thread owns the History page; the dictionary side is the `Dictionary` trait
(`add_correction`), agreed between the two threads.

**Home page:** one line while suggestions wait, "2 suggested words · Review", leading to the
Dictionary page. The single violet dot on Home in the sidebar keeps its meaning (UX.md, "Home
sections").

**Before Home exists** (Phase 1), the dictionary is the text file named by `dictation.dictionary`
in the config, as today. Slice D1 makes the app use it; nothing else changes on screen.

## Failure modes

| Failure | What the user sees |
| --- | --- |
| `dictionary.sqlite` cannot be opened | Dictation works with no dictionary; the Dictionary page says why and offers Export of what it could read |
| A bad line in an import | The import shows a count of skipped lines and the reason for each |
| A wrong substitution | The text is pasted with it; the user deletes the heard form from the page, and it is never learned again |
| The watcher cannot read the field | Nothing; no suggestion from that paste |

## Test plan

- Matcher, prompt line, relevance filter, candidate finder and learning policy: unit tests in
  `hyperme-core`, citing AC-1 to AC-5 and AC-13 to AC-14, with fixtures in
  `crates/hyperme-core/tests/fixtures/dictionary/` made of invented names only.
- The pipeline paths (AC-2, AC-8, AC-9): the dictation state machine tests with the fakes it
  already has.
- Store, import and export (AC-10, AC-11): temp-folder SQLite tests in `hyperme-platform`.
- Speed (AC-6): a `cargo bench`-free timing test in release mode, run by hand and in CI as a
  normal test with a generous bound.
- Live (AC-7, AC-16, AC-17): flowbench and the app on the owner's Mac; numbers only in
  `docs/results/`. The owner's dictionary and samples stay on the Mac, never in the repository.
