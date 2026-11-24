> Status: the source of the decisions of 24 November 2026 recorded in `CLAUDE.md` and
> `docs/RESEARCH.md` section 2, which win where they differ. Copied from local agent state
> (`.omc/specs/`), which is lost with its worktree. Unchanged apart from this block and one
> project name removed from round 7 of the transcript.

# Deep Interview Spec: HyperMe research, focus and first gate

## Metadata
- Interview ID: 81af3a66-hyperme-research-2026-11-24
- Rounds: 9 (plus Round 0 topology)
- Final Ambiguity Score: 19%
- Type: greenfield
- Generated: 2026-11-24
- Threshold: 0.2
- Threshold Source: default
- Initial Context Summarized: yes
- Status: PASSED (pending approval)

## Clarity Breakdown
| Dimension | Score | Weight | Weighted |
|-----------|-------|--------|----------|
| Goal Clarity | 0.80 | 0.40 | 0.32 |
| Constraint Clarity | 0.82 | 0.30 | 0.25 |
| Success Criteria | 0.80 | 0.30 | 0.24 |
| **Total Clarity** | | | **0.81** |
| **Ambiguity** | | | **0.19** |

## Topology

| Component | Status | Description | Coverage / Deferral Note |
|-----------|--------|-------------|--------------------------|
| Dictation | active | Hold a key, formatted text in any app at Wispr Flow speed | The first target. Covered by every acceptance criterion below |
| Overlay shell | deferred | Always-on-top pill, hotkeys, tiny footprint | Owner, round 6: no interface needed to validate the numbers. Decided after the gate |
| Voice commands + connectors | deferred | Say an action, preview, confirm | Owner, rounds 5 and 7: a capability to add after the gate |
| Crons and nudges | deferred | Scheduled jobs, meeting-now, pending for tomorrow | Same. Research: the most saturated part; call into existing schedulers |
| Meetings | deferred | Record, transcribe, notes, action points to tasks | Same. Research: consume an existing notetaker over MCP; do not build capture |
| Coding-agent dispatch | deferred | Start and follow Claude Code / Devin tasks | Same. Research: several vendors by voice is the least served capability |
| Memory and learning | deferred | Knows the user better every day; knowledge base in the style of Obsidian | Owner, round 0: not for any starting version, a pillar of the pitch |
| Local models; Linux and Windows | deferred | | SPEC phase 4 |
| Harness and project setup | deferred | Playbook import | Owner: later, as tickets in GitHub Projects |

## Goal
Decide whether HyperMe is worth building by first demonstrating, with a command-line
tool and the owner's own voice, that a self-built pipeline matches Wispr Flow's dictation
performance. HyperMe's intended identity is one fast, light, open-source surface that
replaces four apps, built to become an open-source project other people install and use.
Everything beyond dictation is a capability decided after the gate.

## Constraints
- Rust command-line tool in `spikes/flowbench/`, no interface. Provider code behind a
  trait, model names in config.
- Cloud speech and language-model APIs. Keys from environment variables or the keychain;
  never printed, logged or committed. Samples stay out of git.
- Spanish and English, one language per utterance, detected automatically.
- Dependencies MIT, Apache-2.0 or BSD. Borrowed code recorded in `NOTICE.md`.
- Check API shapes against current provider docs before coding.
- The owner's time is limited and has no fixed weekly budget. Only the validation test
  is committed.
- Product privacy boundary for later stages: audio, transcript, dictionary, app name and
  text before the cursor may leave the machine under the owner's keys; window text or a
  screenshot only behind a visible switch, off by default; never password fields; no
  telemetry; no audio kept on disk.
- Work data from the owner's employer stays out of scope.
- Tracker for the project: GitHub Projects.

## Non-Goals
- A pill, overlay or paste into other apps at this stage.
- Meetings, commands, crons, coding agents, memory.
- Mixed-language sentences.
- Replacing Wispr Flow in daily use now.
- Harness, hooks, knowledge base, ticket creation.
- Local speech models, Linux, Windows.

## Acceptance Criteria
- [ ] 50 utterances recorded by the owner, 25 Spanish and 25 English, 5 to 15 seconds each.
- [ ] The tool reports, per utterance and per provider combination, time from key release
      to final transcript, to first cleaned token and to full cleaned text, with median
      and 90th percentile.
- [ ] At least one combination reaches a median of 800 ms or less and a 90th percentile
      of 1.2 s or less from key release to full cleaned text, on the owner's network.
- [ ] Language is detected with no manual switch; no utterance is translated or
      transcribed in the wrong language.
- [ ] The same utterances go through Wispr Flow; outputs sit side by side and the owner
      accepts HyperMe's formatting.
- [ ] A live microphone mode lets the owner dictate through the tool and read the result.
- [ ] Results are written to `docs/results/speed-test-YYYY-MM-DD.md` with machine and network.

Recommended, not adopted by the owner:
- [ ] One day each on today's Handy, OpenWhispr and Omi desktop, with notes on what is
      slow or disliked.
- [ ] Wispr Flow and OpenWhispr measured on the same Mac (latency and idle memory) as the
      baseline to beat.

## Assumptions Exposed & Resolved
| Assumption | Challenge | Resolution |
|------------|-----------|------------|
| "Only for me" | Also called a competitor of OpenClaw and Wispr Flow | Success means an open-source project others use |
| Commands and crons come first (SPEC phase 1) | Which single part would make you install it? | Dictation and meetings-to-tasks lead; commands are a later capability |
| The gap is that nobody ships the whole | OpenWhispr and Omi desktop already do; all-in-one launches draw little attention | The difference is speed and lightness, with the full feature set, open source |
| Many gates are needed | Which results would stop the project? | One gate: demonstrate Wispr Flow performance. Other kill criteria not adopted |
| Validation needs a pill and a week of use | Smallest result that counts | Numbers from a CLI plus hands-on use in the CLI |
| Spanish and English may be mixed | How do you actually dictate? | One language per dictation |
| "Won't steal my information" | What may leave the machine? | Audio, text near the cursor, and window content behind an off-by-default switch |
| Time must be budgeted | Hours per week? | Declined; limited time, validation first, then decide |

## Technical Context
Greenfield: the repository holds only documents. Research in `docs/RESEARCH.md` and
`docs/research/2026-11-24/`. Relevant findings: stream while the key is held, pre-warm on
key-down, explicit finalize on release, small cleanup model with a deadline; provider
shortlist and prices in report 01; Handy (MIT) as the later base for the dictation core.

## Ontology (Key Entities)

| Entity | Type | Fields | Relationships |
|--------|------|--------|---------------|
| Owner | core domain | languages, dictionary, keys | Dictates utterances; judges formatting |
| HyperMe | core domain | parts, budgets | One surface for the capabilities |
| Dictation | core domain | utterance, transcript, cleaned text, latency | Produced by speech provider and cleanup model |
| Benchmark | core domain | samples, stages, median, p90 | Gates everything else |
| Speech provider | external system | model, transport, price | Transcribes audio |
| Cleanup model | external system | model, time to first token | Formats the transcript |
| Overlay | supporting | pill state | Hosts all capabilities later |
| Capability | supporting | command, cron, meeting, agent task | Added after the gate |
| Connector | external system | MCP server, auth | Used by capabilities |
| Task | supporting | title, owner, quote | Output of meetings and commands |
| Knowledge base | supporting | files, facts | Grows from dictation and meetings |
| Meeting | supporting | transcript, action items | Read from an existing notetaker |
| Coding-agent task | supporting | vendor, state | Started and followed later |
| Wispr Flow | external system | latency, formatting | The benchmark to match |

## Ontology Convergence

| Round | Entity Count | New | Changed | Stable | Stability Ratio |
|-------|-------------|-----|---------|--------|----------------|
| 1 | 11 | 11 | - | - | - |
| 2 | 12 | 1 | 0 | 11 | 92% |
| 3 | 12 | 0 | 0 | 12 | 100% |
| 4 | 12 | 0 | 0 | 12 | 100% |
| 5 | 14 | 2 | 0 | 12 | 86% |
| 6 to 9 | 14 | 0 | 0 | 14 | 100% |

## Interview Transcript
<details>
<summary>Full Q&A (9 rounds)</summary>

### Round 0
**Q:** Six top-level parts plus a possible memory part: is this the right shape?
**A:** Yes. Include memory and learning: how the agent learns from the user and the platform learns over time, a knowledge base in the style of Obsidian, "HyperMe knows the user better every day". Not the goal of any starting version, but a point for the elevator pitch.

### Round 1
**Q:** Twelve months from now, which one outcome makes it worth building?
**A:** Open-source project.
**Ambiguity:** 49% (Goal 0.55, Constraints 0.55, Criteria 0.42)

### Round 2
**Q:** If only one part could be excellent in the first public release, which?
**A:** Between dictation like Wispr Flow and meetings to tasks, probably plus self-learning.
**Ambiguity:** 48%

### Round 3
**Q:** At its core, HyperMe is...
**A:** One surface for four apps.
**Ambiguity:** 46%

### Round 4 (contrarian)
**Q:** An open-source single surface already exists. Why would someone pick HyperMe?
**A:** Speed and lightness, 100%, but also including the other functionalities, and open source.
**Ambiguity:** 40%

### Round 5
**Q:** Which results would you accept as a reason to stop?
**A:** The big thing to verify and demonstrate is that we can achieve Wispr Flow performance. That is the initial target. The rest (syncing, self-learning, calendar, notes) are capabilities. If we achieve it, then imagine the whole companion.
**Ambiguity:** 35%

### Round 6 (simplifier)
**Q:** The smallest result that counts as "we achieved it"?
**A:** Numbers from a CLI are good, and I would also like to test it myself. No interface or overlay is needed to validate.
**Ambiguity:** 29%

### Round 7
**Q:** How much time over the next three months?
**A:** There will be time constraints, not much per week, but we will make it work. First validate Wispr Flow performance, then see how to evolve the product. Another side project is mostly done.
**Ambiguity:** 26%

### Round 8
**Q:** How do you dictate day to day?
**A:** One language per dictation.
**Ambiguity:** 22%

### Round 9
**Q:** What may HyperMe send off your machine?
**A:** Audio plus text near the cursor, plus what is on screen behind a visible switch that is off by default.
**Ambiguity:** 19%
</details>
