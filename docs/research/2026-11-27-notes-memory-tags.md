# Notetaker prompts, memory capture and tags: how others do it, and options for HyperMe

Research of 27 November 2026, for the roadmap only. No code, not for the current version.
The owner's answers from the interview of the same day are in section 7.
Voice commands as such ("Hyper, create a task") are covered by
[the voice commands research](2026-11-27-voice-commands.md); this note covers notes, memory and
tagging.

## 1. Summary

- **Meeting prompt.** Everyone serious asks first and records only after a yes. Wispr Flow and
  Granola both detect a call and show a card; auto-start is off by default. HyperMe can do the
  same at near-zero idle cost by listening for the macOS "microphone in use" event instead of
  polling.
- **Transcripts with and without a meeting.** Granola and Wispr Flow treat both the same: one
  list of notes, a calendar event attached when one overlaps (Granola: within 15 minutes). An
  ad-hoc call just gets a generated title.
- **Linking facts and to-dos to a meeting.** Nobody does this well. The common pattern is
  backlinks (Reflect: a person note collects every meeting that mentions them). The cheapest good
  idea for HyperMe: right after a meeting, "Hyper, remember …" and "Hyper, remind me …" attach to
  that meeting by default, shown as a chip on the preview you can remove.
- **Tags.** Things nests tags; Todoist nests projects but not labels; Reflect says "entities are
  backlinks, categories are tags". For HyperMe the lightest fit is **one list of contexts, two
  levels deep** (Customers > Amazon, Family > Mother), stored as the memory folder's own files, so
  tags, memory and people are one system, not three.

## 2. The field

### Meeting detection and the "transcribe?" prompt

| Product | How it notices | Asks or records | Without a calendar event | Source |
| --- | --- | --- | --- | --- |
| Wispr Flow Notetaker (Mac, beta) | Detects a call in a supported meeting app; "microphone use or joining alone isn't enough" | Asks ("Meeting detected" card). Auto-start is off by default; when on, a countdown (10 s default) you can stop. After you keep 3 meetings it offers auto-start | Detects the call and asks anyway | [settings](https://docs.wisprflow.ai/articles/9319084321-notetaker-settings-explained-beta), [recording](https://docs.wisprflow.ai/articles/9238501024-recording-a-meeting-with-notetaker-beta), [FAQ](https://wisprflow.ai/notetaker) |
| Granola | Pop-up 1 minute before calendar events with 2 or more attendees; "Call detected" when the mic is in use | Asks. "Never transcribes a meeting in the background unless you've opened a note for it" | Ad-hoc note; a call within 15 minutes of an event is linked to it. Auto-stop after 15 minutes of silence or when the call app releases the mic | [notifications](https://docs.granola.ai/help-center/taking-notes/notifications), [transcription](https://docs.granola.ai/help-center/taking-notes/transcription) |
| Fathom | Calendar plus call apps | Per category: External, Internal, Unscheduled, each On, Off or Optional | "Impromptu" is its own category | [categories](https://help.fathom.video/en/articles/294208), [auto-capture](https://help.fathom.video/en/articles/13729025) |
| Otter | Bot joins calendar meetings when auto-join is on | Records; the auto-joining bot is a known complaint | Manual record | [Carleton College on Otter's bot](https://www.carleton.edu/ai/blog/an-uninvited-guest-what-you-should-know-about-otter-ais-notetaker/) |

Wispr Flow's notes page lists "Past notes" and "Upcoming" and summaries with "Next steps"; no
link from action items into a task list was found (unverified beyond the pages read).

### Tags, contexts and links

| Product | Model | Hierarchy | Source |
| --- | --- | --- | --- |
| Things 3 | Areas hold projects; tags cut across. A tag on an area or project is inherited by its to-dos | Tags nest (drag one onto another) | [Things tags](https://culturedcode.com/things/support/articles/2803581/) |
| Todoist | Projects, sub-projects, sections; labels cut across | Projects nest; labels do not (glossary) | [glossary](https://www.todoist.com/help/articles/todoist-glossary-cA60laWMH) |
| Reflect | `[[backlinks]]` for entities, `#tags` for categories; a person note shows every daily note that mentions them | Flat tags; the graph is the structure | [Reflect academy](https://reflect.academy/using-backlinks-and-tags) |
| Notion | Relations between databases (a task relates to a customer page) | As deep as you build it | Common knowledge, not re-checked |

## 3. What the field teaches

1. **Ask, don't record.** Asking is the privacy default and keeps the cost down, which is what
   the owner wants. The prompt must be one key (Enter) and go away on its own.
2. **Calendar is a bonus, not a requirement.** The meeting is the unit; the event is an optional
   attachment found by time overlap.
3. **Entities beat categories.** "Amazon" and "Mother" are things with their own facts, people and
   history, not just labels. Reflect's rule fits: make them notes, and let tags be the link.
4. **Two levels are enough.** Things allows deep nesting; most people use one level of groups.
   Deeper trees cost settings UI and voice ambiguity for little gain.

## 4. Options for HyperMe

### 4.1 Meeting prompt

| Option | What happens | Cost |
| --- | --- | --- |
| **A. Ask on every detected call (recommended)** | Mic in use by a known call app (Zoom, Meet in a browser, Teams, Slack huddle, FaceTime), or a calendar event with 2+ attendees starting: the dot grows into a card "In a meeting. Transcribe? Enter / Esc". Gone after 20 s. Mute per app | Event-driven: one CoreAudio listener on "device running somewhere", no polling, no idle CPU |
| B. Ask, plus optional auto-start for calendar meetings | As A, and a setting to start on its own for calendar meetings, with a countdown | Same |
| C. Calendar only | Only events with attendees get the card | Misses ad-hoc calls |
| D. Manual only | A menu item and a voice command "Hyper, transcribe this meeting" | Zero, but easy to forget |

### 4.2 Meetings list

One **Meetings** page in the Home window holds every transcript. Each has an optional calendar
event attached by overlap (within 15 minutes, as Granola) and a title from the event or, if none,
generated after the meeting. You can attach or detach an event afterwards.

### 4.3 Linking facts and to-dos to a meeting

| Option | What happens |
| --- | --- |
| **A. Recent-meeting default (recommended)** | For a while after a meeting ends (say 30 minutes, or until the next meeting), any "remember" or "remind me" preview shows a chip "· from: Weekly with Amazon". Enter keeps it, one key removes it. The meeting's contexts (tags) carry over too |
| B. Ask every time | "Is this about the meeting with Amazon?" Accurate, but one more step |
| C. Only by context | No meeting link; you say "for Amazon" and it lands under Amazon. Meetings show up under Amazon if their attendees or title match |
| D. By name, later | "Hyper, add to my Amazon meeting from Tuesday …" resolved by search. Works with A or C, costs a model call |

### 4.4 Tags ("contexts")

| Option | What happens |
| --- | --- |
| **A. Contexts, two levels, as memory files (recommended)** | Groups (Customers, Family, Work) hold contexts (Amazon, Mother, Brother). Each context is a Markdown file in the memory folder, e.g. `customers/amazon.md`, holding its facts and aliases ("AWS", "mamá"). Tasks, notes, facts and meetings carry context ids. "Remind me to send the deck for Amazon" matches the alias locally, no model needed. Settings page lists and edits them; new ones are proposed, never made silently |
| B. Flat tags | `#amazon`, `#family`; simplest, but no grouping on Home and no "everything for Customers" view |
| C. Free nesting (Things) | Any depth. More settings UI, more voice ambiguity |
| D. No tags; memory entities only | People and projects in memory, linked by name. Fine for facts, weak for filtering the task list |

Storage for A: a `context` table in SQLite (id, parent id, name, aliases) indexed from the files,
and a join table per item kind. Thousands of rows are kilobytes; no idle cost.

### 4.5 Where things land

Ties to open question Q-15 (notes into memory, the task list, or both): with contexts, a quick
note goes to the memory file of its context when it is a fact, and to the task list with that
context when it has an action.

## 5. How knowledge tools store and link things (second round)

Asked by the owner after the interview: how do others store tags, facts and relations, and how
do they relate things automatically? Should HyperMe change course?

| Tool | Storage | Tags and hierarchy | Automatic relating | Source |
| --- | --- | --- | --- | --- |
| Obsidian | Markdown files; YAML properties at the top; links in properties must be quoted | Tags can nest with `/` (not re-checked on the Tags page); no nested properties | Backlinks; unlinked mentions on request; graph view | [properties](https://obsidian.md/help/Editing+and+formatting/Properties) |
| Craft | Its own documents (no open format stated) | One level: `#project/design` | Backlinks are made automatically on every @-mention; no AI linking | [linking](https://support.craft.do/en/organize-and-find/linking), [tags](https://support.craft.do/en/organize-and-find/tags) |
| Reflect | Its own notes | Flat tags; "for entities use backlinks, for categories use tags" | Backlinks build each person's activity feed | [academy](https://reflect.academy/using-backlinks-and-tags) |
| Tana | Nodes with ids; format not public | Supertags are flat; "extend" copies a template; "Part of" builds a tree but is experimental and slows as it grows | References, unlinked mentions with a Link button (the user confirms), optional AI fields | [nodes](https://outliner.tana.inc/learn/features/nodes-and-references), [supertags](https://outliner.tana.inc/learn/features/supertags), [fields](https://outliner.tana.inc/learn/features/fields) |
| Mem | Cloud; embeddings | "Collections", filled automatically by AI | Auto-organise into collections; a year-long review praises fuzzy recall but finds confident wrong answers hard to spot and export loses the "retrieval graph" | [collections](https://help.mem.ai/features/collections), [review](https://dev.to/pickuma/mem-review-ai-organized-notes-one-year-on-4h83) |
| Basic Memory (MCP server) | **Markdown files are the truth; SQLite (FTS5) is a rebuildable index** | Flat tags | Deterministic parser, no model: `- [category] fact #tag` lines and `- relation [[Target]]` lines; a link to a missing note resolves when it is created | [how it works](https://www.mintlify.com/basicmachines-co/basic-memory/concepts/how-it-works), [graph](https://www.mintlify.com/basicmachines-co/basic-memory/concepts/knowledge-graph) |
| Zep / Graphiti (agent memory) | Graph database (Neo4j and others) plus embeddings | Entities and typed edges | A model call per message extracts entities and facts; another resolves duplicates and marks contradicted facts invalid. Each fact has a "valid from / until" time | [Zep paper](https://ar5iv.labs.arxiv.org/html/2501.13956), [Atlan comparison (vendor-neutral page, figures self-reported)](https://atlan.com/know/zep-vs-mem0/) |
| Mem0 | Embeddings, optional graph (paid tier) | None | Model extracts facts and merges them; can bring back stale facts that are semantically close | [Atlan comparison](https://atlan.com/know/zep-vs-mem0/) |

**What this teaches.**

1. The local-first tools (Obsidian, Basic Memory) agree with what HyperMe already decided:
   Markdown files as the truth, SQLite as a rebuildable index. Nobody light uses a graph
   database; the graph is just the links between files.
2. Automatic linking that people trust is **deterministic and confirmed**: backlinks, alias
   matches, "unlinked mentions" with a Link button. Fully automatic AI organisation (Mem) is
   good for fuzzy recall but makes confident mistakes that are hard to see.
3. The agent-memory systems spend one or two model calls on every message and need a graph
   database and a vector store. Too heavy for HyperMe's budget and against "nothing learned
   silently". One idea is worth taking: a fact can be **superseded** ("Amazon's contact is now
   Laura"), so facts carry a date and an optional "until".
4. Deep hierarchies are rare and slow (Tana calls its tree experimental). Two levels is in line
   with Craft and Obsidian practice.

**Recommendation: keep the approach, sharpen the format.**

- One Markdown file per context, person or project, in the memory folder. Groups are folders
  (`customers/amazon.md`, `family/mother.md`); this is the two-level tag tree.
- Inside a file, facts are lines in a fixed, Obsidian-friendly form, borrowed in spirit from Basic
  Memory (check its licence before borrowing any code): `- fact text (2026-11-27)`, links as
  `[[Laura Pérez]]`, and aliases in the properties (`aliases: [AWS]`), which Obsidian also reads.
  Tasks and meetings in SQLite carry the same context ids.
- Linking is local and deterministic: alias and name matching in Rust, no model call. The model is
  used only where a model call already happens (the command call that parses "remember …" picks
  the context), and anything new (a new context, a superseded fact) goes to the approval list.
- Embeddings (for "what did we say about pricing?") are a later, optional index next to the full
  text one, only if full-text search proves too weak; no vector database.

## 6. Phasing proposal

| Piece | Phase |
| --- | --- |
| Contexts (two levels) and aliases, used by "remember" | 3, with memory |
| Contexts on tasks; filter the task list by context; morning card grouped by group | 4, with tasks (changes spec 92's non-goal "no tags") |
| Meeting detection prompt, Meetings page, event attached by overlap | 5 |
| A meeting inherits contexts from its attendees or title, so it shows beside the tasks and facts of that context | 5 |

## 7. Decided by the owner, 27 November 2026

| ID | Question | Decision |
| --- | --- | --- |
| N-1 | What HyperMe does when it notices a call | Ask every time: a card "In a meeting. Transcribe?", Enter starts, Esc or 20 s dismisses, mute per app. Nothing records without a yes |
| N-2 | Linking facts and to-dos to a meeting | No direct link. Tags only: "for Amazon" puts it under Amazon, and the meeting gets the Amazon tag from attendees or title. A direct meeting link stays in Later, only if tags prove not enough |
| N-3 | Shape of tags | Two levels (group, then context), each context a Markdown file in the memory folder with its facts and aliases. The second round (section 5) checked it against Obsidian, Tana, Craft, Mem, Basic Memory, Zep and Mem0 and keeps it, with the format sharpened as recommended there |
| N-4 | When | Spread: contexts with memory in phase 3, contexts on tasks in phase 4, the meeting prompt and Meetings page in phase 5 |
| N-5 | Storage and linking after the second round | Keep the Markdown files and SQLite index, with the format sharpened. The owner likes embeddings and an AI knowledge graph too: both go to Later, to evaluate again when there is bandwidth, after an interview of the owner to settle the product questions |

## Sources

All links above, read on 27 November 2026. Notion's model was not re-checked.
