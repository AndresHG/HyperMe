//! Tasks slice 3: the three schedule shapes, the next run in a zone, daylight saving and the
//! catch-up decision. Tests cite `specs/92-daily-tasks/spec.md`: AC-15, and the catch-up half
//! of AC-14 (the wake-up count belongs to the scheduler, slice 4).

use hyperme_core::schedule::{
    CatchUp, Schedule, Zone, catch_up, last_run_due, next_run, on_presence,
};
use hyperme_core::tasks::WeekdaySet;
use time::{Date, Month, SignedDuration, Time, UtcDateTime, UtcOffset, Weekday};

/// A zone with a standard offset and one daylight saving period, given by its two UTC
/// instants. Enough for one year of tests; no time zone database.
struct FakeZone {
    standard: i8,
    summer: i8,
    starts: UtcDateTime,
    ends: UtcDateTime,
}

impl Zone for FakeZone {
    fn offset_at(&self, at: UtcDateTime) -> UtcOffset {
        let hours = if at >= self.starts && at < self.ends {
            self.summer
        } else {
            self.standard
        };
        UtcOffset::from_hms(hours, 0, 0).unwrap()
    }
}

fn date(month: Month, day: u8) -> Date {
    Date::from_calendar_date(2026, month, day).unwrap()
}

fn utc(month: Month, day: u8, h: u8, m: u8) -> UtcDateTime {
    UtcDateTime::new(date(month, day), Time::from_hms(h, m, 0).unwrap())
}

fn time(h: u8, m: u8) -> Time {
    Time::from_hms(h, m, 0).unwrap()
}

/// Europe/Madrid in 2026: +1, and +2 from 29 March 01:00 UTC to 25 October 01:00 UTC.
fn madrid() -> FakeZone {
    FakeZone {
        standard: 1,
        summer: 2,
        starts: utc(Month::March, 29, 1, 0),
        ends: utc(Month::October, 25, 1, 0),
    }
}

/// America/New_York in 2026: -5, and -4 from 8 March 07:00 UTC to 1 November 06:00 UTC.
fn new_york() -> FakeZone {
    FakeZone {
        standard: -5,
        summer: -4,
        starts: utc(Month::March, 8, 7, 0),
        ends: utc(Month::November, 1, 6, 0),
    }
}

const EVERY_DAY: WeekdaySet = WeekdaySet::from_array([
    Weekday::Monday,
    Weekday::Tuesday,
    Weekday::Wednesday,
    Weekday::Thursday,
    Weekday::Friday,
    Weekday::Saturday,
    Weekday::Sunday,
]);

const WORKDAYS: WeekdaySet = WeekdaySet::from_array([
    Weekday::Monday,
    Weekday::Tuesday,
    Weekday::Wednesday,
    Weekday::Thursday,
    Weekday::Friday,
]);

fn at(h: u8, m: u8, weekdays: WeekdaySet) -> Schedule {
    Schedule::At {
        time: time(h, m),
        weekdays,
    }
}

// --- Next run --------------------------------------------------------------------------------

/// R-32: "at a time on chosen weekdays", in local time. Later today if still ahead, else the
/// next chosen weekday. Thursday 26 November 2026 is the day of the tests.
#[test]
fn at_runs_at_the_local_time_on_chosen_weekdays() {
    let zone = madrid();
    let evening = at(18, 30, WORKDAYS);
    // 12:00 UTC is 13:00 in Madrid: 18:30 local today is 17:30 UTC.
    assert_eq!(
        next_run(&evening, utc(Month::November, 26, 12, 0), &zone),
        Some(utc(Month::November, 26, 17, 30))
    );
    // Exactly at the run, the next one is tomorrow: a run is never returned twice.
    assert_eq!(
        next_run(&evening, utc(Month::November, 26, 17, 30), &zone),
        Some(utc(Month::November, 27, 17, 30))
    );
    // After Friday's run comes Monday's.
    assert_eq!(
        next_run(&evening, utc(Month::November, 27, 18, 0), &zone),
        Some(utc(Month::November, 30, 17, 30))
    );
    // No weekday chosen: never.
    assert_eq!(
        next_run(
            &at(8, 0, WeekdaySet::EMPTY),
            utc(Month::November, 26, 0, 0),
            &zone
        ),
        None
    );
}

/// AC-15, R-32: changing the zone from Madrid to New York moves the next 18:30 job to 18:30
/// New York time.
#[test]
fn a_zone_change_moves_the_next_run_to_the_new_local_time() {
    let evening = at(18, 30, EVERY_DAY);
    let after = utc(Month::November, 26, 12, 0);
    assert_eq!(
        next_run(&evening, after, &madrid()),
        Some(utc(Month::November, 26, 17, 30))
    );
    assert_eq!(
        next_run(&evening, after, &new_york()),
        Some(utc(Month::November, 26, 23, 30))
    );
}

/// AC-15: a 02:30 job on the day clocks spring forward (02:00 becomes 03:00) runs once, at
/// 03:00, the first valid minute after it; the next day it is back at 02:30.
#[test]
fn a_time_skipped_by_spring_forward_runs_once_at_the_first_valid_minute() {
    let zone = madrid();
    let job = at(2, 30, EVERY_DAY);
    let first = next_run(&job, utc(Month::March, 28, 23, 0), &zone).unwrap();
    assert_eq!(first, utc(Month::March, 29, 1, 0), "03:00 local, +2");
    let second = next_run(&job, first, &zone).unwrap();
    assert_eq!(second, utc(Month::March, 30, 0, 30), "02:30 local, +2");

    // The same in New York, where the change is at 02:00 local on 8 March.
    let ny = new_york();
    assert_eq!(
        next_run(&job, utc(Month::March, 8, 5, 0), &ny),
        Some(utc(Month::March, 8, 7, 0)),
        "03:00 local, -4"
    );
}

/// AC-15: on the day clocks fall back (03:00 becomes 02:00), 02:30 happens twice; the job runs
/// once, at the first.
#[test]
fn a_time_repeated_by_fall_back_runs_once_at_the_first() {
    let zone = madrid();
    let job = at(2, 30, EVERY_DAY);
    let first = next_run(&job, utc(Month::October, 24, 22, 0), &zone).unwrap();
    assert_eq!(first, utc(Month::October, 25, 0, 30), "the first 02:30, +2");
    let second = next_run(&job, first, &zone).unwrap();
    assert_eq!(
        second,
        utc(Month::October, 26, 1, 30),
        "not the second 02:30 of the 25th, but the 26th's, +1"
    );
}

/// R-32: "every n minutes while present". The next run counts from the last; whether the user is
/// present is the scheduler's business. Zero minutes never runs.
#[test]
fn every_while_present_runs_every_n_minutes() {
    let zone = madrid();
    let sync = Schedule::EveryWhilePresent { minutes: 30 };
    assert_eq!(
        next_run(&sync, utc(Month::November, 26, 9, 10), &zone),
        Some(utc(Month::November, 26, 9, 40))
    );
    let never = Schedule::EveryWhilePresent { minutes: 0 };
    assert_eq!(
        next_run(&never, utc(Month::November, 26, 9, 10), &zone),
        None
    );
}

/// R-15, R-16, R-32: "on presence after a time". Presence counts from `from` on a chosen
/// weekday; the timed run is at `latest`, for a user already present then.
#[test]
fn on_presence_after_counts_presence_from_its_time_and_runs_at_the_latest() {
    let zone = madrid();
    let morning = Schedule::OnPresenceAfter {
        from: time(6, 0),
        latest: time(9, 30),
        weekdays: WORKDAYS,
    };
    // Friday 27 November: 05:00 local is too early, 07:40 counts, 14:00 still counts.
    assert!(!on_presence(
        &morning,
        utc(Month::November, 27, 4, 0),
        &zone
    ));
    assert!(on_presence(
        &morning,
        utc(Month::November, 27, 6, 40),
        &zone
    ));
    assert!(on_presence(
        &morning,
        utc(Month::November, 27, 13, 0),
        &zone
    ));
    // Saturday 28 November: never.
    assert!(!on_presence(
        &morning,
        utc(Month::November, 28, 8, 0),
        &zone
    ));
    // The timed run: 09:30 local on the next workday.
    assert_eq!(
        next_run(&morning, utc(Month::November, 27, 4, 0), &zone),
        Some(utc(Month::November, 27, 8, 30))
    );
    assert_eq!(
        next_run(&morning, utc(Month::November, 27, 9, 0), &zone),
        Some(utc(Month::November, 30, 8, 30))
    );
    // The other shapes have no presence trigger.
    assert!(!on_presence(
        &at(8, 0, EVERY_DAY),
        utc(Month::November, 27, 9, 0),
        &zone
    ));
}

// --- Catch-up --------------------------------------------------------------------------------

/// R-35: the last scheduled time not later than now.
#[test]
fn the_last_due_run_is_the_latest_scheduled_time_not_after_now() {
    let zone = madrid();
    let noon = at(12, 0, WORKDAYS);
    assert_eq!(
        last_run_due(&noon, utc(Month::November, 26, 12, 15), &zone),
        Some(utc(Month::November, 26, 11, 0))
    );
    assert_eq!(
        last_run_due(&noon, utc(Month::November, 26, 11, 0), &zone),
        Some(utc(Month::November, 26, 11, 0))
    );
    // Monday morning: the last was Friday's.
    assert_eq!(
        last_run_due(&noon, utc(Month::November, 30, 7, 0), &zone),
        Some(utc(Month::November, 27, 11, 0))
    );
    // "Every n minutes" has no fixed times to miss.
    assert_eq!(
        last_run_due(
            &Schedule::EveryWhilePresent { minutes: 30 },
            utc(Month::November, 26, 12, 0),
            &zone
        ),
        None
    );
}

/// AC-14 (catch-up half), R-35: a wake after a sleep across 12:00 runs the 12:00 job once when
/// inside its window and skips it when outside. Once run, it is never run again.
#[test]
fn a_missed_run_catches_up_once_inside_its_window_and_is_skipped_outside() {
    let zone = madrid();
    let noon = at(12, 0, EVERY_DAY);
    let ran_yesterday = Some(utc(Month::November, 25, 11, 0));
    // Asleep from 11:30 to 13:00 local; it wakes at 12:00 UTC, an hour after the run.
    let wake = utc(Month::November, 26, 12, 0);

    let inside = SignedDuration::hours(2);
    assert_eq!(
        catch_up(&noon, ran_yesterday, wake, inside, &zone),
        CatchUp::Run {
            scheduled: utc(Month::November, 26, 11, 0)
        }
    );
    let outside = SignedDuration::minutes(30);
    assert_eq!(
        catch_up(&noon, ran_yesterday, wake, outside, &zone),
        CatchUp::Skip
    );
    // A zero window never catches up.
    assert_eq!(
        catch_up(&noon, ran_yesterday, wake, SignedDuration::ZERO, &zone),
        CatchUp::Skip
    );

    // Run once at the wake: the next decision, even a minute later, skips.
    let ran_at_wake = Some(wake);
    assert_eq!(
        catch_up(
            &noon,
            ran_at_wake,
            wake + SignedDuration::MINUTE,
            inside,
            &zone
        ),
        CatchUp::Skip
    );
    // A job that has never run catches up its last time too.
    assert_eq!(
        catch_up(&noon, None, wake, inside, &zone),
        CatchUp::Run {
            scheduled: utc(Month::November, 26, 11, 0)
        }
    );
}

/// R-35: missing several days catches up only the last one, once.
#[test]
fn several_missed_runs_catch_up_as_one() {
    let zone = madrid();
    let rollover = at(4, 0, EVERY_DAY);
    let ran = Some(utc(Month::November, 23, 3, 0));
    let wake = utc(Month::November, 26, 8, 0);
    assert_eq!(
        catch_up(&rollover, ran, wake, SignedDuration::hours(24), &zone),
        CatchUp::Run {
            scheduled: utc(Month::November, 26, 3, 0)
        }
    );
}
