# Phase 4 daily tasks: design

**Status:** draft, 26 November 2026, with [spec.md](spec.md). How the task list, the two daily
cards and the scheduler fit the code that exists, what they cost to run, and how they are tested.

## Where the code goes

No new crate. Pure logic in `hyperme-core` (no I/O, testable with a paused clock), SQLite and
macOS code in `hyperme-platform`, as the dictation history already does
([history.rs](../../crates/hyperme-platform/src/history.rs)).

| Module | Crate | Holds | Platform calls |
| --- | --- | --- | --- |
| `tasks` | `hyperme-core` | `Task`, `Status`, day and due rules, the order of Today, roll-over, the dependency rules, and the `TaskStore` trait | None |
| `brief` | `hyperme-core` | Pure builders: `today(snapshot, now)`, `morning(snapshot, now)`, `evening(snapshot, now)` return card models. No store and no network inside | None |
| `schedule` | `hyperme-core` | The three schedule shapes, `next_run(schedule, after, zone)`, and the catch-up decision | None |
| `scheduler` | `hyperme-core` | One async task: the job table in memory, one timer, presence events in, job runs out. `Clock` and `Presence` traits so tests use a paused clock | None |
| `task_store` | `hyperme-platform` | `TaskStore` on SQLite: schema, migrations, queries, the cycle check | SQLite (bundled) |
| `presence` | `hyperme-platform` | `Presence` on macOS: wake, unlock, lock, clock and time zone changes, seconds since the last input | `cfg(target_os = "macos")`, NSWorkspace and distributed notifications, `CGEventSource` |
| cards, Today block | `app` | Render the card models in the pill panel and Home's web view | Tauri |

Linux and Windows later only add a `presence` module; nothing in the core changes (`CLAUDE.md`).

**New dependency:** `chrono` in `hyperme-core`, for local dates, weekdays and time zone offsets.
It is already in `Cargo.lock` through other crates, MIT OR Apache-2.0. No time zone database
crate (T-9): the core asks the platform for the current local offset at a given instant
(`chrono::Local` on the Mac), and tests pass a fake zone with a daylight saving rule. No cron
crate (T-8).

## The store

One SQLite file, the one the history index uses, opened once at launch. A connection costs
about a megabyte with SQLite's default page cache; the tables below add nothing measurable at
idle.

```sql
CREATE TABLE task (
  id            INTEGER PRIMARY KEY,
  title         TEXT    NOT NULL,
  notes         TEXT,
  status        TEXT    NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'done', 'dropped')),
  day           TEXT,                 -- 'YYYY-MM-DD', local: when you plan to do it
  due_date      TEXT,                 -- 'YYYY-MM-DD', local: the hard cut-off
  due_time      TEXT,                 -- 'HH:MM', local, optional
  source        TEXT    NOT NULL DEFAULT 'hyperme',   -- hyperme, github, calendar, gmail
  source_account TEXT,                -- which account, for one profile with several (Q-12)
  source_ref    TEXT,                 -- the item's id at the source
  source_url    TEXT,                 -- the link back
  created_at    INTEGER NOT NULL,     -- Unix ms
  arrived_day   TEXT    NOT NULL,     -- the day it showed up, for the "new" mark
  rolled        INTEGER NOT NULL DEFAULT 0,
  finished_at   INTEGER,              -- Unix ms, when done or dropped
  UNIQUE (source, source_account, source_ref)
);

-- "blocked is blocked by blocker". A graph without cycles (R-7).
CREATE TABLE task_block (
  blocked INTEGER NOT NULL REFERENCES task (id) ON DELETE CASCADE,
  blocker INTEGER NOT NULL REFERENCES task (id) ON DELETE CASCADE,
  PRIMARY KEY (blocked, blocker)
) WITHOUT ROWID;
CREATE INDEX task_block_blocker ON task_block (blocker);

CREATE INDEX task_open ON task (day, due_date) WHERE status = 'open';
CREATE INDEX task_finished ON task (finished_at) WHERE status != 'open';

-- One row per day (R-29).
CREATE TABLE day_log (
  day TEXT PRIMARY KEY,
  captured INTEGER NOT NULL DEFAULT 0, done INTEGER NOT NULL DEFAULT 0,
  moved INTEGER NOT NULL DEFAULT 0,    dropped INTEGER NOT NULL DEFAULT 0,
  morning_at INTEGER, evening_at INTEGER, closed_at INTEGER
) WITHOUT ROWID;

-- Jobs (R-31). Built-in jobs are seeded at launch from config; prompt jobs come from Settings.
CREATE TABLE job (
  name        TEXT PRIMARY KEY,
  kind        TEXT NOT NULL,          -- rollover, morning, evening, nudge, sync:<connector>, prompt
  schedule    TEXT NOT NULL,          -- JSON, one of the three shapes below
  enabled     INTEGER NOT NULL DEFAULT 1,
  catch_up_min INTEGER NOT NULL,      -- the catch-up window in minutes; 0 skips missed runs
  allowlist   TEXT,                   -- JSON array of tool names, prompt jobs only
  prompt      TEXT,                   -- prompt jobs only
  last_run    INTEGER, last_status TEXT, fails_today INTEGER NOT NULL DEFAULT 0
) WITHOUT ROWID;
```

Queries that matter:

- **Ready.** `NOT EXISTS (SELECT 1 FROM task_block b JOIN task t ON t.id = b.blocker WHERE
  b.blocked = task.id AND t.status = 'open')`. Uses the primary key of `task_block`.
- **Today.** Open tasks with `day <= today OR due_date <= today OR arrived_day = today`, plus
  tasks finished since day start, in one statement through the partial indexes. Ordering is done
  in Rust by `brief::today`, so it is tested without SQLite.
- **Cycle check (R-7).** Before inserting `blocked ← blocker`, a recursive CTE walks the
  blockers of `blocker`. If it reaches `blocked`, the insert is refused and the path is returned
  for the message.
- **Undo (R-3).** The done card holds the new task's id; ⌘Z deletes it and its links in one
  transaction. After 5 seconds the card goes and the id is forgotten.

Day and due are stored as local civil dates, not instants, because "Friday" means Friday
wherever the Mac is. Due times are local too; a nudge is computed from them with the zone at
that moment.

## Capture by voice

Capture is a set of built-in tools in the phase 3 command loop, beside the MCP tools. HyperMe's
own list is not an outside service, so it needs no MCP server and no preview (T-3).

| Tool | Arguments | Does |
| --- | --- | --- |
| `tasks_add` | `title`, `day?`, `due_date?`, `due_time?`, `blocked_by?: [ref]`, `chain?: bool` | Adds one task, or a chain when the user said "first …, then …" (R-8) |
| `tasks_update` | `ref`, any field, `add_blocker?`, `remove_blocker?` | Moves, renames, links |
| `tasks_finish` | `ref`, `as: done or dropped` | Marks done or dropped |
| `tasks_today` | none | Shows the morning card built now (R-20) |

A `ref` is a task id the model picked from context, or a description the app matches. To
resolve "tomorrow at 10" and "after the insurance call", the prompt for a task command carries:

- today's date, weekday, local time and the next workday, in one line;
- the titles and ids of open tasks, most recent first, capped at 50 (about 600 tokens).

The model returns ISO dates; the app validates them (a past day is refused unless said
explicitly) before saving. Spanish and English go through the same tools; the prompt has
examples in both. Matching a description to a task (R-9) is done in Rust as a fallback when the
model returns text instead of an id: normalised words, then the share of words in common, and
more than one match above the threshold asks the user.

No date-parsing crate. The model already resolves language; a crate would be English-only and
would be a second source of truth.

## The scheduler

```
loop {
    let next = jobs.nearest_next_run();          // one value, computed, not polled
    select! {
        _ = clock.sleep_until(next)  => run_due_jobs(clock.now()),
        ev = presence.recv()         => match ev {
            Woke | ClockChanged | ZoneChanged => recompute_all(),   // R-34
            Unlocked | Launched              => run_presence_jobs(), // morning, catch-up
            Locked | Asleep                  => {}
        },
        cmd = control.recv()         => apply(cmd),                 // edit, pause, run now
    }
}
```

- **Three schedule shapes (R-32).** `At { time, weekdays }` for roll-over and the evening card;
  `EveryWhilePresent { minutes }` for sync jobs; `OnPresenceAfter { from, latest, weekdays }` for
  the morning card. About 200 lines with their tests, against a crate.
- **Why recompute on wake.** Timers in Tokio follow the monotonic clock, and on macOS the
  monotonic clock that Rust's `Instant` uses stops while the Mac sleeps. A timer set at 17:00
  for 18:30, across a sleep from 17:30 to 19:00, would fire at 20:00. Recomputing from the wall
  clock on every wake event fixes it (R-34, AC-14).
- **Catch-up (R-35).** On each run or recompute, for each job: the last scheduled time not later
  than now. If it is after `last_run` and within `catch_up_min`, run once; set `last_run` to
  now. Defaults: roll-over 24 hours (always catches up), evening card until 23:00 (R-25), nudges
  0 (a missed nudge is pointless), sync 0 (the next presence sync covers it).
- **Daylight saving (AC-15).** A local time that does not exist (02:30 on the spring day) runs at
  the first valid minute after it; a time that happens twice runs once, at the first. The same
  rule as `croner`'s fixed-time jobs, so behaviour matches what people expect from cron.
- **Failures (R-36).** A failed run sets `fails_today`; the next presence event retries with
  backoff (1, 5, 15 minutes). The second failure in a day raises "Job failed" once.
- **Pause (R-23).** "Pause Hyper for 1 hour" holds cards and nudges in a list; roll-over and sync
  still run. When the pause ends the held cards collapse into one: the latest morning or evening
  card, and the nudges still in the future.
- **App not running.** Nothing runs. At launch the catch-up rule applies, so the morning card
  still shows if HyperMe starts at 08:10. No launchd agent: it would need its own process and
  could not draw the card anyway.

## Presence on macOS

The trigger for the morning card is the owner sitting down, not a clock (T-1). The `Presence`
module turns a handful of system notifications into events:

| Event | Source |
| --- | --- |
| `Woke` | `NSWorkspaceDidWakeNotification` |
| `Asleep` | `NSWorkspaceWillSleepNotification` |
| `Unlocked`, `Locked` | Distributed notifications `com.apple.screenIsUnlocked` and `com.apple.screenIsLocked` |
| `ClockChanged`, `ZoneChanged` | `NSSystemClockDidChangeNotification`, `NSSystemTimeZoneDidChangeNotification` |
| `Launched` | HyperMe itself, once at start |
| `PowerChanged { low }` | `NSProcessInfoPowerStateDidChangeNotification` (Low Power Mode) and the battery level from IOKit's power sources, read on that notification and on wake (R-33a) |

"Present" is unlocked, awake, and input in the last 10 minutes
(`CGEventSourceSecondsSinceLastEventType`, read only when a job asks, so it costs nothing at
idle). The notification names above are to be checked against Apple's current documentation
before coding, as `CLAUDE.md` asks; the distributed screen-lock names are long-standing but not
formally documented, so the slice includes a probe example, as `hotkey_probe` did for the hotkey.

## The cards

The morning and evening cards are alert cards from the dot (UX.md, "Alerts"), drawn in the pill
panel that already exists, so they cost no new window and never take focus. The core builds a
small card model; the app sends it to the web view as one event.

```
MorningCard {
  date, meetings: Pending | Ready([Meeting]), top: [TaskLine; ≤5], waiting_on: n,
  proposals: n, mail: Pending | Ready(n), yesterday: Option<DayCounts>,
}
EveningCard {
  done: [TaskLine; ≤5], done_more: n, open: [TaskLine], unblocked_tomorrow: [TaskLine],
  proposals: n, stale: [TaskLine],   // rolled over 3 times or more (R-14)
  recap: Option<String>,             // only with the switch on (R-27)
}
```

Order of Today's tasks (R-17): due today or overdue (by due time), then unblocked today, then
planned for today, then rolled over (fewest roll-overs first), then arrived today without a
day; ties by creation time. Blocked tasks go under "Waiting on".

The evening card's one-key choices act on the selected row (↑ ↓): T tomorrow, P pick a day (a
small day strip: the next five workdays and "Someday"), D drop. ↵ "Close the day" applies them
all (R-28). Someday means no day: the task leaves Today and stays in the full list.

Mockup: [docs/ux/daily-cards.svg](../../docs/ux/daily-cards.svg).

## Budgets

| Measure | Budget | Why it fits |
| --- | --- | --- |
| Presence event to morning card on screen | under 150 ms (R-18) | One SQLite read of today's rows (well under 5 ms at 10 000 tasks with the partial index), a pure build, one event to an existing web view |
| Build Today, morning or evening | under 20 ms at 10 000 tasks (R-38) | Checked by a plain timing test in `tests/` (no benchmark crate), run on the owner's Mac, numbers to `docs/results/` |
| Idle CPU | one timer wake-up per scheduled job, nothing else (R-33) | No polling; presence is push |
| Idle memory | under 5 MB more (R-39) | No new process or window; tables are on disk |
| Network | none on either card's path (R-18, R-27) | Sync jobs run before, while present, every 30 minutes by default |

## Failure modes

| Failure | Behaviour |
| --- | --- |
| Calendar or mail sync fails | Card shows tasks; the part says "Calendar not reached · ↵ Fix" on the second failure (R-36) |
| The model returns a date in the past or no date for "tomorrow" | Refused before saving; the done card shows "Couldn't read the date · edit" |
| The model invents a task id | Not found, so the Rust matcher runs on the description (R-9) |
| The store is locked by a long write | Reads use WAL mode, so cards never wait for a write |
| Clock jumps forward an hour (manual change) | `ClockChanged` recomputes; catch-up runs each missed job once at most |
| Two Macs | Not supported: one list per Mac (non-goal) |

## Test plan

- `hyperme-core` unit tests, with a paused Tokio clock and a fake zone: the order of Today,
  roll-over, catch-up, next run across daylight saving and time zone changes, the scheduler's
  wake-up count (AC-7, AC-14, AC-15, AC-16).
- `hyperme-platform` tests on a temporary SQLite file: the schema, the cycle check, ready, undo,
  mirror upserts (AC-4, AC-5, AC-6, AC-12).
- Card builders: golden tests from synthetic stores, English and Spanish titles (AC-9 data,
  AC-11, AC-13).
- Command tools: a fake model that returns the tool calls, for each sentence in AC-1 to AC-3,
  in English and Spanish. No paid API in any test (`CLAUDE.md`).
- Measured on the owner's Mac, written to `docs/results/`: AC-9 latency, AC-18 build time and
  idle cost.

## Interfaces with other work

Agreed or proposed with the threads building these parts on 26 November 2026; each part's own
spec wins where it differs.

| Part | What this spec needs from it |
| --- | --- |
| Home window | Blocks above Recent, so Today and Waiting for you are two more entries; a `hyperme://home/today` link for the morning card's ↵; an event when Today has been seen, to clear "new"; re-render on a push from the core; `home_attention(bool)` for the violet dot |
| Google calendar and mail ([spec 97](../97-google-connectors/spec.md), PR #101) | Sync as `sync:google-calendar` and `sync:google-mail` jobs in this scheduler, with read tools only, skipped on low power and run once on wake; today's meetings as title, start, end, account and link; items to act on upserted as mirrored tasks keyed by source, account and id; a count of mail to answer |
| Phase 3 command loop | Built-in tools beside MCP tools, and a done card with ⌘Z |
| Phase 3 dot and cards | Alert cards from the dot for the morning and evening cards and nudges |
