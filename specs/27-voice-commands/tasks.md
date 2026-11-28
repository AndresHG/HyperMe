# Voice commands: slices

Each slice is one pull request, starts from failing tests that cite its `AC-n`, and passes fmt,
clippy and tests before it merges. Open one issue per slice when the spec is approved, in the 0.2
milestone.

| # | Slice | Proves | Size | Depends on |
| --- | --- | --- | --- | --- |
| 1 | Measure first: time stage 2 (classifier on the cleanup model) and stage 3 (one tool call on a Luna or Haiku-class model) on the owner's command samples with the `flowbench` approach; numbers to `docs/results/`. If release to card cannot reach 1 s, stop and bring it to the owner | AC-15 (first numbers) | S | none |
| 2 | The router: name matching through the dictionary's heard forms (replacing spec 151's prefix list), the command key, the route as a pure function | AC-1, AC-2 | S | none |
| 3 | The tool registry and stage 1: built-in and connector tools with classes, the keyword table in config, schema validation of a tool call | AC-4 (keyword half), AC-5 | M | none |
| 4 | Stages 2 and 3: the classifier prompt on the cleanup provider, the tool-calling request on the Commands role, the date check against the task resolver, the fallback provider, the deadline | AC-4, AC-6, AC-9 | M | 3 |
| 5 | Settings: the Commands row in AI models (tool-calling models only, Luna or Haiku-class default), the optional command key, the list of tools with their switches | AC-12 | M | 4 |
| 6 | Task commands end to end: add (chains included) and change, the done card, ⌘Z and Paste as text, the best-guess card, choices for an ambiguous name, offline save | AC-7, AC-10, AC-11 | M | 2, 4 |
| 7 | GitHub tickets through the MCP client: preview, ↵ creates once, Esc creates nothing, no retry of the write | AC-8 | M | 4 |
| 8 | Questions to the Chat tab (#271): the answer pops up from the pill and continues in Chat | (spec of #271) | M | 4, #271 |
| 9 | History and traces: the command fields in history, feeding the Commands page (#231), and the stage timings in the trace | AC-13 | S | 6 |
| 10 | Quality and the live check: 40 command samples in `hyperme-quality`, 30 timed commands on the owner's Mac, idle memory and key-down before and after | AC-3, AC-14, AC-15, AC-16 | S | 6, 7 |

Slices 1, 2 and 3 can run in parallel. Voice edits (spec 151) build on slice 2's router when both
are in flight; until then spec 151 keeps its own prefix list. Linear tickets wait for the tracker
research (#270).
