//! When timed work runs: the three schedule shapes, the next run in a zone, daylight saving and
//! the catch-up decision (specs/92-daily-tasks, design.md, "The scheduler"; R-32, R-35).
//!
//! Pure functions on UTC instants. Local times go through a [`Zone`], which the platform backs
//! with the Mac's current zone and tests back with a fake; there is no time zone database.

use time::{Date, OffsetDateTime, PlainDateTime, SignedDuration, Time, UtcDateTime, UtcOffset};

use crate::tasks::WeekdaySet;

/// The local zone: its UTC offset at an instant (T-9).
pub trait Zone {
    fn offset_at(&self, at: UtcDateTime) -> UtcOffset;
}

/// The three shapes a job's schedule can take (R-32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schedule {
    /// At a local time on chosen weekdays: roll-over, the evening card.
    At { time: Time, weekdays: WeekdaySet },
    /// Every n minutes while the user is present: sync jobs. Zero never runs.
    EveryWhilePresent { minutes: u32 },
    /// On the first presence after `from` on a chosen weekday, or at `latest` for a user already
    /// present then: the morning card (R-15, R-16).
    OnPresenceAfter {
        from: Time,
        latest: Time,
        weekdays: WeekdaySet,
    },
}

impl Schedule {
    /// The local time and weekdays of the shape's timed run, if it has one.
    fn timed(&self) -> Option<(Time, WeekdaySet)> {
        match *self {
            Schedule::At { time, weekdays } => Some((time, weekdays)),
            Schedule::OnPresenceAfter {
                latest, weekdays, ..
            } => Some((latest, weekdays)),
            Schedule::EveryWhilePresent { .. } => None,
        }
    }
}

/// A week and a day: any chosen weekday comes within it, whatever the offset.
const SEARCH_DAYS: i64 = 8;

/// The first run strictly after `after`, or `None` if the schedule never runs.
pub fn next_run(schedule: &Schedule, after: UtcDateTime, zone: &dyn Zone) -> Option<UtcDateTime> {
    if let Schedule::EveryWhilePresent { minutes } = *schedule {
        return (minutes > 0).then(|| after + SignedDuration::minutes(i64::from(minutes)));
    }
    let (time, weekdays) = schedule.timed()?;
    let today = local(after, zone).date();
    (0..=SEARCH_DAYS)
        .map(|n| today + SignedDuration::days(n))
        .filter(|d| weekdays.contains(d.weekday()))
        .map(|d| resolve(d, time, zone))
        .find(|run| *run > after)
}

/// The latest scheduled time not later than `now`. "Every n minutes" has no fixed times, so
/// none.
pub fn last_run_due(schedule: &Schedule, now: UtcDateTime, zone: &dyn Zone) -> Option<UtcDateTime> {
    let (time, weekdays) = schedule.timed()?;
    let today = local(now, zone).date();
    (0..=SEARCH_DAYS)
        .map(|n| today - SignedDuration::days(n))
        .filter(|d| weekdays.contains(d.weekday()))
        .map(|d| resolve(d, time, zone))
        .find(|run| *run <= now)
}

/// Whether a presence event at `at` triggers the schedule: only "on presence after a time", on
/// a chosen weekday, from its time on. Showing it once per day is the scheduler's state.
pub fn on_presence(schedule: &Schedule, at: UtcDateTime, zone: &dyn Zone) -> bool {
    match *schedule {
        Schedule::OnPresenceAfter { from, weekdays, .. } => {
            let now = local(at, zone);
            weekdays.contains(now.weekday()) && now.time() >= from
        }
        _ => false,
    }
}

/// What to do about a run missed while the Mac slept or HyperMe was not running (R-35).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatchUp {
    /// Run once now for the run that was due at `scheduled`.
    Run {
        scheduled: UtcDateTime,
    },
    Skip,
}

/// The catch-up decision at `now`: run once if the last scheduled time is after `last_run` and
/// no more than `window` ago; otherwise skip. A zero window skips every missed run. After the
/// caller sets `last_run` to now, the same run is never caught up again.
pub fn catch_up(
    schedule: &Schedule,
    last_run: Option<UtcDateTime>,
    now: UtcDateTime,
    window: SignedDuration,
    zone: &dyn Zone,
) -> CatchUp {
    let Some(scheduled) = last_run_due(schedule, now, zone) else {
        return CatchUp::Skip;
    };
    let missed = last_run.is_none_or(|ran| ran < scheduled);
    if window > SignedDuration::ZERO && missed && now - scheduled <= window {
        CatchUp::Run { scheduled }
    } else {
        CatchUp::Skip
    }
}

fn local(at: UtcDateTime, zone: &dyn Zone) -> OffsetDateTime {
    at.to_offset(zone.offset_at(at))
}

/// The instant of local `time` on `day`. A time that does not exist (skipped when clocks spring
/// forward) runs at the first valid minute after it; a time that happens twice (when they fall
/// back) runs at the first. The same rule as cron's fixed-time jobs.
fn resolve(day: Date, time: Time, zone: &dyn Zone) -> UtcDateTime {
    let wall = PlainDateTime::new(day, time);
    let as_utc = wall.as_utc();
    // The offsets a day either side: at most one change in between, as in every real zone.
    let before = zone.offset_at(as_utc - SignedDuration::DAY);
    let after = zone.offset_at(as_utc + SignedDuration::DAY);
    let mut valid: Vec<UtcDateTime> = [before, after]
        .into_iter()
        .map(|offset| (offset, wall.assume_offset(offset).to_utc()))
        .filter(|(offset, instant)| zone.offset_at(*instant) == *offset)
        .map(|(_, instant)| instant)
        .collect();
    valid.sort();
    if let Some(first) = valid.first() {
        return *first;
    }
    // In the gap: the first minute after the earlier candidate whose offset has changed.
    let earliest = wall.assume_offset(before.max(after)).to_utc();
    let latest = wall.assume_offset(before.min(after)).to_utc();
    let start = zone.offset_at(earliest);
    let mut minute = earliest;
    while minute < latest && zone.offset_at(minute) == start {
        minute += SignedDuration::MINUTE;
    }
    minute
}
