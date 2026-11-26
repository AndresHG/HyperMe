//! Tasks slice 6: the Today and morning card models. Tests cite
//! `specs/92-daily-tasks/spec.md`: AC-9 (data), AC-18 (build time), R-14, R-17, R-21.

use std::time::Instant;

use hyperme_core::brief::{
    DayCounts, Meeting, MorningCard, Now, Part, Sources, StaleChoice, morning, today,
};
use hyperme_core::tasks::{DayRules, Due, Group, Status, Task, TaskId};
use time::{Date, Month, Time};

fn day(d: u8) -> Date {
    Date::from_calendar_date(2026, Month::November, d).unwrap()
}
const WED: u8 = 25;
const THU: u8 = 26;

fn hm(h: u8, m: u8) -> Time {
    Time::from_hms(h, m, 0).unwrap()
}

/// Thursday began at this instant (Unix ms); the morning card showed an hour later.
const BEGAN: i64 = 10_000_000;
const MORNING_AT: i64 = BEGAN + 3_600_000;

fn now() -> Now {
    Now {
        today: day(THU),
        day_began_ms: BEGAN,
        rules: DayRules::default(),
        morning_shown_ms: Some(MORNING_AT),
        today_seen_ms: None,
    }
}

fn task(id: TaskId, title: &str) -> Task {
    Task::new(id, title, id, day(WED))
}

fn planned(id: TaskId, title: &str) -> Task {
    let mut t = task(id, title);
    t.day = Some(day(THU));
    t
}

fn ready_sources() -> Sources {
    Sources {
        meetings: Part::Ready(vec![]),
        mail_to_answer: Part::Ready(0),
        proposals: 0,
        yesterday: None,
    }
}

fn top_titles(card: &MorningCard) -> Vec<&str> {
    card.top.iter().map(|l| l.title.as_str()).collect()
}

/// R-17: up to five tasks for today in order (due today or overdue, then unblocked, then
/// planned for today, then rolled over), and how many more.
#[test]
fn the_morning_card_shows_five_tasks_in_order_and_counts_the_rest() {
    let mut due = task(1, "Due at four");
    due.due = Some(Due {
        date: day(THU),
        time: Some(hm(16, 0)),
    });
    let mut rolled = task(2, "Rolled over");
    rolled.day = Some(day(WED));
    rolled.rolled = 1;
    let mut blocker = task(3, "Blocker");
    blocker.status = Status::Done;
    blocker.finished_at = Some(BEGAN + 1);
    let mut unblocked = planned(4, "Unblocked");
    unblocked.blocked_by = vec![3];
    let snapshot = vec![
        due,
        rolled,
        blocker,
        unblocked,
        planned(5, "Planned A"),
        planned(6, "Planned B"),
        planned(7, "Planned C"),
    ];

    let card = morning(&snapshot, &now(), &ready_sources());
    assert_eq!(card.date, day(THU));
    assert_eq!(
        top_titles(&card),
        vec![
            "Due at four",
            "Unblocked",
            "Planned A",
            "Planned B",
            "Planned C"
        ]
    );
    assert_eq!(card.more, 1, "the rolled-over task is the one more");
    assert_eq!(card.top[0].due.unwrap().time, Some(hm(16, 0)));
    assert_eq!(
        card.top[1].group,
        Group::Unblocked {
            blocker_dropped: false
        }
    );
}

/// AC-9 (data), R-18: the card builds from local data while calendar and mail have not
/// synced, with those parts pending; when the sync returns, they fill in, meetings in order.
#[test]
fn parts_that_need_a_sync_are_pending_until_it_returns() {
    let snapshot = vec![planned(1, "Call the dentist")];
    let pending = Sources {
        meetings: Part::Pending,
        mail_to_answer: Part::Pending,
        proposals: 2,
        yesterday: None,
    };
    let card = morning(&snapshot, &now(), &pending);
    assert_eq!(card.meetings, Part::Pending);
    assert_eq!(card.mail_to_answer, Part::Pending);
    assert_eq!(top_titles(&card), vec!["Call the dentist"]);
    assert_eq!(card.proposals, 2);

    let meeting = |title: &str, h| Meeting {
        title: title.into(),
        start: hm(h, 0),
        end: hm(h + 1, 0),
        account: None,
        link: None,
    };
    let synced = Sources {
        meetings: Part::Ready(vec![meeting("Planning", 15), meeting("Standup", 10)]),
        mail_to_answer: Part::Ready(3),
        ..pending
    };
    let card = morning(&snapshot, &now(), &synced);
    let Part::Ready(meetings) = &card.meetings else {
        panic!("meetings should be ready");
    };
    assert_eq!(meetings[0].title, "Standup", "the first meeting first");
    assert_eq!(meetings[0].start, hm(10, 0));
    assert_eq!(card.mail_to_answer, Part::Ready(3));
}

/// R-17: how many tasks wait on blockers, and "Yesterday: n done" from the day log.
#[test]
fn the_morning_card_counts_waiting_tasks_and_yesterday() {
    let blocker = planned(1, "Call the insurance");
    let mut waiting = planned(2, "Send Ana the numbers");
    waiting.blocked_by = vec![1];
    let sources = Sources {
        yesterday: Some(DayCounts {
            captured: 4,
            done: 3,
            moved: 1,
            dropped: 0,
        }),
        ..ready_sources()
    };
    let card = morning(&[blocker, waiting], &now(), &sources);
    assert_eq!(card.waiting_on, 1);
    assert_eq!(top_titles(&card), vec!["Call the insurance"]);
    assert_eq!(card.yesterday.unwrap().done, 3);

    assert_eq!(morning(&[], &now(), &ready_sources()).yesterday, None);
}

/// R-14: tasks rolled over three times are marked and listed with the keep, pick a day or drop
/// question.
#[test]
fn stale_tasks_come_with_the_keep_day_or_drop_question() {
    let mut stale = task(1, "Renew the passport");
    stale.day = Some(day(20));
    stale.rolled = 3;
    let mut fresh = task(2, "Rolled once");
    fresh.day = Some(day(WED));
    fresh.rolled = 1;
    let snapshot = vec![stale, fresh];

    let card = morning(&snapshot, &now(), &ready_sources());
    assert_eq!(card.stale.len(), 1);
    assert_eq!(card.stale[0].title, "Renew the passport");
    assert!(card.stale[0].stale);
    assert_eq!(
        StaleChoice::ALL,
        [StaleChoice::Keep, StaleChoice::PickDay, StaleChoice::Drop]
    );

    let view = today(&snapshot, &now());
    assert_eq!(view.stale.len(), 1);
    assert!(view.lines.iter().any(|l| l.stale && l.id == 1));
}

/// R-21: items that arrived after the morning card carry "new" until the user opens Today:
/// tasks captured since, and tasks unblocked since. Before the morning card nothing is new.
#[test]
fn items_after_the_morning_card_are_new_until_today_is_seen() {
    let mut before = planned(1, "Captured before the card");
    before.created_at = MORNING_AT - 1;
    let mut after = planned(2, "Captured at 15:00");
    after.created_at = MORNING_AT + 5_000;
    let mut blocker = task(3, "Blocker");
    blocker.status = Status::Done;
    blocker.finished_at = Some(MORNING_AT + 6_000);
    let mut unblocked = planned(4, "Unblocked after the card");
    unblocked.created_at = 0;
    unblocked.blocked_by = vec![3];
    let snapshot = vec![before, after, blocker, unblocked];

    let new_ids = |n: &Now| -> Vec<TaskId> {
        today(&snapshot, n)
            .lines
            .iter()
            .filter(|l| l.new)
            .map(|l| l.id)
            .collect()
    };
    let mut ids = new_ids(&now());
    ids.sort();
    assert_eq!(ids, vec![2, 4]);

    let seen = Now {
        today_seen_ms: Some(MORNING_AT + 10_000),
        ..now()
    };
    assert!(new_ids(&seen).is_empty(), "seen: no longer new");

    let no_card = Now {
        morning_shown_ms: None,
        ..now()
    };
    assert!(new_ids(&no_card).is_empty());
}

/// R-10, design.md: Today holds the lines to do now, the tasks waiting on a blocker with its
/// title, and how many were finished since day start.
#[test]
fn today_has_lines_waiting_and_done() {
    let blocker = planned(1, "Call the insurance");
    let mut waiting = planned(2, "Send Ana the numbers");
    waiting.blocked_by = vec![1];
    let mut done = planned(3, "Book the room");
    done.status = Status::Done;
    done.finished_at = Some(BEGAN + 5);
    let mut done_yesterday = planned(4, "Old");
    done_yesterday.status = Status::Done;
    done_yesterday.finished_at = Some(BEGAN - 5);

    let view = today(&[blocker, waiting, done, done_yesterday], &now());
    assert_eq!(view.date, day(THU));
    assert_eq!(view.lines.len(), 1);
    assert_eq!(view.waiting.len(), 1);
    assert_eq!(view.waiting[0].title, "Send Ana the numbers");
    assert_eq!(view.waiting[0].on, "Call the insurance");
    assert_eq!(view.done_today, 1);
}

/// 10 000 synthetic tasks: a mix of planned, due, rolled over, blocked, done and later ones.
fn synthetic(n: i64) -> Vec<Task> {
    (1..=n)
        .map(|id| {
            let mut t = Task::new(id, &format!("Task number {id}"), id, day(WED));
            match id % 10 {
                0 => t.day = Some(day(THU)),
                1 => {
                    t.day = Some(day(WED));
                    t.rolled = (id % 5) as u32;
                }
                2 => {
                    t.due = Some(Due {
                        date: day(THU),
                        time: Some(hm((id % 24) as u8, 0)),
                    })
                }
                3 => {
                    t.day = Some(day(THU));
                    t.blocked_by = vec![id - 3, id - 1];
                }
                4 => {
                    t.status = Status::Done;
                    t.finished_at = Some(BEGAN + id);
                }
                5 => t.day = Some(Date::from_calendar_date(2026, Month::December, 3).unwrap()),
                _ => t.day = Some(day(THU)),
            }
            t
        })
        .collect()
}

/// AC-18, R-38: Today and the morning card each build in under 20 ms from 10 000 tasks.
/// Release builds only; run with
/// `cargo test --release -p hyperme-core --test brief -- --nocapture` on the Mac
/// and record the numbers in docs/results/.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn both_cards_build_from_10_000_tasks_in_under_20_ms() {
    let snapshot = synthetic(10_000);
    let sources = ready_sources();
    let n = now();
    let mut today_ms = Vec::new();
    let mut morning_ms = Vec::new();
    for _ in 0..21 {
        let t = Instant::now();
        let view = today(&snapshot, &n);
        today_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        assert!(!view.lines.is_empty());

        let t = Instant::now();
        let card = morning(&snapshot, &n, &sources);
        morning_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(card.top.len(), 5);
    }
    let stats = |v: &mut Vec<f64>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        (v[v.len() / 2], v[v.len() - 1])
    };
    let (today_median, today_max) = stats(&mut today_ms);
    let (morning_median, morning_max) = stats(&mut morning_ms);
    println!(
        "10000 tasks, 21 runs: today median {today_median:.3} ms max {today_max:.3} ms; \
         morning median {morning_median:.3} ms max {morning_max:.3} ms"
    );
    assert!(today_max < 20.0, "today took {today_max:.3} ms");
    assert!(morning_max < 20.0, "morning took {morning_max:.3} ms");
}
