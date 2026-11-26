//! Tasks slice 1: task types, the day and due rules, the order of Today, roll-over and the stale
//! mark. Tests cite the requirements and acceptance criteria of
//! `specs/92-daily-tasks/spec.md`; AC-7 is proved here for the rules, and again on SQLite in
//! slice 2.

mod common;

use common::tasks::MemoryTaskStore;
use hyperme_core::tasks::{
    DayRules, Due, Finish, Group, NewTask, Source, Status, Task, TaskError, TaskId, TaskStore,
    WeekdaySet, cycle, first_open_blocker, is_ready, order_today, roll_over,
};
use time::{Date, Month, PlainDateTime, Time, Weekday};

/// Wednesday 25, Thursday 26 and Friday 27 November 2026, and the weekend after.
fn day(d: u8) -> Date {
    Date::from_calendar_date(2026, Month::November, d).unwrap()
}
const WED: u8 = 25;
const THU: u8 = 26;
const FRI: u8 = 27;

fn at(d: u8, h: u8, m: u8) -> PlainDateTime {
    day(d).with_hms(h, m, 0).unwrap()
}

fn time(h: u8, m: u8) -> Time {
    Time::from_hms(h, m, 0).unwrap()
}

/// The instant Thursday began, in Unix ms. Finish times at or after it count as "today".
const THU_BEGAN: i64 = 1_000_000;

/// An open local task that arrived on Wednesday, created in id order.
fn task(id: TaskId, title: &str) -> Task {
    Task::new(id, title, id * 10, day(WED))
}

fn titles(today: &hyperme_core::tasks::Today<'_>) -> Vec<String> {
    today.lines.iter().map(|l| l.task.title.clone()).collect()
}

// --- Day boundary and workdays -------------------------------------------------------------

/// "Day start" in the spec's words: 04:00 by default, so 01:00 belongs to the day before.
#[test]
fn a_time_before_day_start_belongs_to_the_day_before() {
    let rules = DayRules::default();
    assert_eq!(rules.day_start, time(4, 0));
    assert_eq!(rules.day_of(at(FRI, 1, 0)), day(THU));
    assert_eq!(rules.day_of(at(FRI, 3, 59)), day(THU));
    assert_eq!(rules.day_of(at(FRI, 4, 0)), day(FRI));
    assert_eq!(rules.day_of(at(THU, 23, 59)), day(THU));
    assert_eq!(rules.starts_at(day(FRI)), at(FRI, 4, 0));
}

/// The day start hour is a setting: at midnight, 01:00 is already the new day.
#[test]
fn the_day_start_is_configurable() {
    let rules = DayRules {
        day_start: time(0, 0),
        ..DayRules::default()
    };
    assert_eq!(rules.day_of(at(FRI, 1, 0)), day(FRI));
    assert_eq!(rules.day_of(at(FRI, 0, 0)), day(FRI));
    assert_eq!(rules.day_of(at(THU, 23, 59)), day(THU));
}

/// "Workday" in the spec's words: Monday to Friday by default, so after Friday comes Monday.
#[test]
fn the_next_workday_skips_the_weekend() {
    let rules = DayRules::default();
    assert_eq!(day(THU).weekday(), Weekday::Thursday);
    assert_eq!(rules.next_workday(day(WED)), day(THU));
    assert_eq!(rules.next_workday(day(FRI)), day(30));
    assert_eq!(rules.next_workday(day(28)), day(30));
    assert!(rules.is_workday(day(FRI)));
    assert!(!rules.is_workday(day(28)));
    assert!(!rules.is_workday(day(29)));
}

/// The workdays are a setting, for a Sunday-to-Thursday week.
#[test]
fn the_workdays_are_configurable() {
    let rules = DayRules {
        workdays: WeekdaySet::from_array([
            Weekday::Sunday,
            Weekday::Monday,
            Weekday::Tuesday,
            Weekday::Wednesday,
            Weekday::Thursday,
        ]),
        ..DayRules::default()
    };
    assert_eq!(rules.next_workday(day(THU)), day(29));
    assert!(!rules.is_workday(day(FRI)));
    // With no workdays at all, the next workday is simply the next day, never a hang.
    let none = DayRules {
        workdays: WeekdaySet::EMPTY,
        ..DayRules::default()
    };
    assert_eq!(none.next_workday(day(THU)), day(FRI));
}

// --- Roll-over and the stale mark ----------------------------------------------------------

/// AC-7, R-13: at day start an open task planned for yesterday stays in Today with a roll-over
/// count of 1 and its original day.
#[test]
fn an_open_task_from_yesterday_rolls_over_and_keeps_its_day() {
    let mut planned = task(1, "Call the dentist");
    planned.day = Some(day(WED));
    let mut done = task(2, "Already done");
    done.day = Some(day(WED));
    done.status = Status::Done;
    done.finished_at = Some(THU_BEGAN - 1);
    let mut dropped = task(3, "Dropped");
    dropped.day = Some(day(WED));
    dropped.status = Status::Dropped;
    let mut for_today = task(4, "Planned for today");
    for_today.day = Some(day(THU));
    let mut later = task(5, "Planned for Friday");
    later.day = Some(day(FRI));
    let no_day = task(6, "No day");
    let mut tasks = vec![planned, done, dropped, for_today, later, no_day];

    let rolled = roll_over(&mut tasks, day(THU));

    assert_eq!(rolled, vec![1]);
    assert_eq!(tasks[0].rolled, 1);
    assert_eq!(tasks[0].day, Some(day(WED)), "the original day is kept");
    assert!(tasks[1..].iter().all(|t| t.rolled == 0));

    let today = order_today(&tasks, day(THU), THU_BEGAN, &DayRules::default());
    assert_eq!(
        titles(&today),
        vec!["Planned for today", "Call the dentist"]
    );
    let line = &today.lines[1];
    assert_eq!(line.group, Group::RolledOver);
    assert!(!line.stale);
}

/// AC-7, R-14: after three roll-overs Today marks the task, so the app can ask keep, pick a day
/// or drop. Two are not enough.
#[test]
fn a_task_rolled_over_three_times_is_marked_stale() {
    let rules = DayRules::default();
    assert_eq!(rules.stale_after, 3);
    let mut planned = task(1, "Renew the passport");
    planned.day = Some(day(23));
    let mut tasks = vec![planned];

    roll_over(&mut tasks, day(24));
    roll_over(&mut tasks, day(WED));
    let today = order_today(&tasks, day(WED), THU_BEGAN, &rules);
    assert_eq!(tasks[0].rolled, 2);
    assert!(!today.lines[0].stale);
    assert!(!rules.is_stale(&tasks[0]));

    roll_over(&mut tasks, day(THU));
    let today = order_today(&tasks, day(THU), THU_BEGAN, &rules);
    assert_eq!(tasks[0].rolled, 3);
    assert_eq!(tasks[0].day, Some(day(23)));
    assert!(today.lines[0].stale);
    assert!(rules.is_stale(&tasks[0]));
}

/// R-14: the threshold is configurable, and a finished task is never stale.
#[test]
fn the_stale_threshold_is_configurable() {
    let rules = DayRules {
        stale_after: 5,
        ..DayRules::default()
    };
    let mut t = task(1, "Sort the photos");
    t.rolled = 4;
    assert!(!rules.is_stale(&t));
    t.rolled = 5;
    assert!(rules.is_stale(&t));
    t.status = Status::Done;
    assert!(!rules.is_stale(&t));
}

// --- Ready and blocked ---------------------------------------------------------------------

/// "Ready" in the spec's words, and R-10: open, and every blocker done or dropped. A blocked
/// task waits under "Waiting on" with its first open blocker, not among the tasks to do now.
#[test]
fn a_blocked_task_waits_on_its_first_open_blocker() {
    let mut call = task(1, "Call the insurance");
    call.day = Some(day(THU));
    let mut book = task(2, "Book the room");
    book.status = Status::Done;
    book.finished_at = Some(THU_BEGAN - 5);
    let mut send = task(3, "Send Ana the numbers");
    send.day = Some(day(THU));
    send.blocked_by = vec![2, 1];
    let tasks = vec![call, book, send];

    assert!(is_ready(&tasks[0], &tasks));
    assert!(!is_ready(&tasks[2], &tasks));
    assert!(!is_ready(&tasks[1], &tasks), "a finished task is not ready");
    assert_eq!(first_open_blocker(&tasks[2], &tasks).map(|t| t.id), Some(1));

    let today = order_today(&tasks, day(THU), THU_BEGAN, &DayRules::default());
    assert_eq!(titles(&today), vec!["Call the insurance"]);
    assert_eq!(today.waiting.len(), 1);
    assert_eq!(today.waiting[0].task.title, "Send Ana the numbers");
    assert_eq!(today.waiting[0].on.title, "Call the insurance");
}

/// R-11, AC-5 (rules): when the last open blocker is finished today the task is in Today marked
/// "unblocked", and the mark says when a blocker was dropped rather than done.
#[test]
fn a_task_unblocked_today_is_marked_and_says_when_a_blocker_was_dropped() {
    let mut call = task(1, "Call the insurance");
    call.status = Status::Done;
    call.finished_at = Some(THU_BEGAN + 10);
    let mut send = task(2, "Send Ana the numbers");
    send.day = Some(day(THU));
    send.blocked_by = vec![1];

    let mut room = task(3, "Book the room");
    room.status = Status::Dropped;
    room.finished_at = Some(THU_BEGAN + 20);
    let mut invite = task(4, "Send the invite");
    invite.day = Some(day(THU));
    invite.blocked_by = vec![3];

    // Unblocked yesterday: an ordinary planned task today.
    let mut old = task(5, "Old blocker");
    old.status = Status::Done;
    old.finished_at = Some(THU_BEGAN - 1);
    let mut after_old = task(6, "After the old blocker");
    after_old.day = Some(day(THU));
    after_old.blocked_by = vec![5];

    let tasks = vec![call, send, room, invite, old, after_old];
    let today = order_today(&tasks, day(THU), THU_BEGAN, &DayRules::default());

    let groups: Vec<(String, Group)> = today
        .lines
        .iter()
        .map(|l| (l.task.title.clone(), l.group))
        .collect();
    assert_eq!(
        groups,
        vec![
            (
                "Send Ana the numbers".into(),
                Group::Unblocked {
                    blocker_dropped: false
                }
            ),
            (
                "Send the invite".into(),
                Group::Unblocked {
                    blocker_dropped: true
                }
            ),
            ("After the old blocker".into(), Group::Planned),
        ]
    );
    assert!(today.waiting.is_empty());
}

// --- The order of Today --------------------------------------------------------------------

/// R-17 and design.md: due today or overdue, then unblocked today, then planned for today, then
/// rolled over, then arrived today without a day.
#[test]
fn today_is_ordered_due_unblocked_planned_rolled_arrived() {
    let mut arrived = Task::new(1, "Arrived today", 10, day(THU));
    arrived.due = Some(Due {
        date: day(FRI),
        time: None,
    });
    let mut rolled = task(2, "Rolled over");
    rolled.day = Some(day(WED));
    rolled.rolled = 1;
    let mut planned = task(3, "Planned");
    planned.day = Some(day(THU));
    let mut blocker = task(4, "Blocker");
    blocker.status = Status::Done;
    blocker.finished_at = Some(THU_BEGAN + 1);
    let mut unblocked = task(5, "Unblocked");
    unblocked.day = Some(day(THU));
    unblocked.blocked_by = vec![4];
    let mut due = task(6, "Due today");
    due.day = Some(day(FRI));
    due.due = Some(Due {
        date: day(THU),
        time: Some(time(16, 0)),
    });
    let tasks = vec![arrived, rolled, planned, blocker, unblocked, due];

    let today = order_today(&tasks, day(THU), THU_BEGAN, &DayRules::default());
    assert_eq!(
        titles(&today),
        vec![
            "Due today",
            "Unblocked",
            "Planned",
            "Rolled over",
            "Arrived today"
        ]
    );
    assert_eq!(today.lines[0].group, Group::Due);
    assert_eq!(today.lines[4].group, Group::Arrived);
}

/// R-17: due tasks by due time, overdue first; a due date without a time sorts after the timed
/// ones of that date; ties by creation time.
#[test]
fn due_tasks_are_ordered_by_due_time() {
    let due = |id, title, date, t: Option<Time>| {
        let mut x = task(id, title);
        x.due = Some(Due { date, time: t });
        x
    };
    let tasks = vec![
        due(1, "Today, no time", day(THU), None),
        due(2, "Today 16:00", day(THU), Some(time(16, 0))),
        due(3, "Today 09:00", day(THU), Some(time(9, 0))),
        due(4, "Overdue", day(WED), Some(time(18, 0))),
        due(5, "Today, no time, later", day(THU), None),
        due(6, "Tomorrow", day(FRI), Some(time(8, 0))),
    ];
    let today = order_today(&tasks, day(THU), THU_BEGAN, &DayRules::default());
    assert_eq!(
        titles(&today),
        vec![
            "Overdue",
            "Today 09:00",
            "Today 16:00",
            "Today, no time",
            "Today, no time, later"
        ]
    );
}

/// R-17: rolled-over tasks fewest roll-overs first, then by creation time.
#[test]
fn rolled_over_tasks_come_fewest_first() {
    let rolled = |id, title, n| {
        let mut x = task(id, title);
        x.day = Some(day(20));
        x.rolled = n;
        x
    };
    let tasks = vec![
        rolled(1, "Three", 3),
        rolled(2, "One, older", 1),
        rolled(3, "Two", 2),
        rolled(4, "One, newer", 1),
    ];
    let today = order_today(&tasks, day(THU), THU_BEGAN, &DayRules::default());
    assert_eq!(
        titles(&today),
        vec!["One, older", "One, newer", "Two", "Three"]
    );
    assert!(today.lines[3].stale);
}

/// "Today" in the spec's words: open tasks whose day or due is today or earlier, plus what came
/// in today without a day. Nothing planned later, nothing finished, nothing from before.
#[test]
fn today_holds_only_what_belongs_to_today() {
    let mut later = task(1, "Planned for Friday");
    later.day = Some(day(FRI));
    let mut captured_for_later = Task::new(2, "Captured today for Friday", 20, day(THU));
    captured_for_later.day = Some(day(FRI));
    let old_no_day = task(3, "Came in yesterday, no day");
    let mut finished = task(4, "Done today");
    finished.day = Some(day(THU));
    finished.status = Status::Done;
    finished.finished_at = Some(THU_BEGAN + 1);
    let mut due_later = task(5, "Due Friday");
    due_later.due = Some(Due {
        date: day(FRI),
        time: None,
    });
    let tasks = vec![later, captured_for_later, old_no_day, finished, due_later];

    let today = order_today(&tasks, day(THU), THU_BEGAN, &DayRules::default());
    assert!(today.lines.is_empty(), "got {:?}", titles(&today));
    assert!(today.waiting.is_empty());
}

// --- Dependencies --------------------------------------------------------------------------

/// AC-4 (rules), R-7: linking A blocked by B when B is already blocked by A is a loop naming
/// both; a three-task loop names all three; a task cannot block itself.
#[test]
fn a_link_that_would_make_a_loop_is_found() {
    let a = task(1, "A");
    let mut b = task(2, "B");
    b.blocked_by = vec![1];
    let mut c = task(3, "C");
    c.blocked_by = vec![2];
    let tasks = vec![a, b, c];

    assert_eq!(cycle(&tasks, 1, 2), Some(vec![2, 1]));
    assert_eq!(cycle(&tasks, 1, 3), Some(vec![3, 2, 1]));
    assert_eq!(cycle(&tasks, 1, 1), Some(vec![1]));
    assert_eq!(cycle(&tasks, 3, 1), None);
    assert_eq!(
        cycle(&tasks, 2, 1),
        None,
        "an existing link again is no loop"
    );
}

/// A mirrored item keeps its source, account, reference and link back (R-1).
#[test]
fn a_mirrored_task_keeps_its_source() {
    let mut t = task(1, "Review the PR");
    assert_eq!(t.source, Source::HyperMe);
    t.source = Source::Mirror {
        name: "github".into(),
        account: Some("work".into()),
        reference: "hyperme#105".into(),
        url: Some("https://github.com/AndresHG/HyperMe/issues/105".into()),
    };
    assert!(matches!(t.source, Source::Mirror { ref name, .. } if name == "github"));
}

// --- The TaskStore trait, through the in-memory fake ---------------------------------------

/// The trait carries the whole flow the rules need: add with a blocker, a loop refused with its
/// path (R-7), finish, roll-over and a snapshot that orders into Today (R-11, R-13).
#[tokio::test]
async fn the_store_trait_carries_the_daily_flow() {
    let store = MemoryTaskStore::default();
    let new = |title: &str, d: Option<Date>, blocked_by: Vec<TaskId>| NewTask {
        title: title.into(),
        notes: None,
        day: d,
        due: None,
        source: Source::HyperMe,
        blocked_by,
        created_at: 0,
        arrived_day: day(WED),
    };

    let call = store
        .add(new("Call the insurance", Some(day(WED)), vec![]))
        .await
        .unwrap();
    let send = store
        .add(new("Send Ana the numbers", Some(day(THU)), vec![call]))
        .await
        .unwrap();

    assert_eq!(
        store.block(call, send).await,
        Err(TaskError::Cycle(vec![send, call]))
    );
    assert_eq!(store.block(call, 99).await, Err(TaskError::NotFound(99)));

    assert_eq!(store.roll_over(day(THU)).await.unwrap(), vec![call]);

    let snapshot = store.snapshot(day(THU), THU_BEGAN).await.unwrap();
    let today = order_today(&snapshot, day(THU), THU_BEGAN, &DayRules::default());
    assert_eq!(titles(&today), vec!["Call the insurance"]);
    assert_eq!(today.waiting[0].task.id, send);

    store
        .finish(call, Finish::Done, THU_BEGAN + 60_000)
        .await
        .unwrap();
    let snapshot = store.snapshot(day(THU), THU_BEGAN).await.unwrap();
    let today = order_today(&snapshot, day(THU), THU_BEGAN, &DayRules::default());
    assert_eq!(titles(&today), vec!["Send Ana the numbers"]);
    assert_eq!(
        today.lines[0].group,
        Group::Unblocked {
            blocker_dropped: false
        }
    );

    store.unblock(send, call).await.unwrap();
    let snapshot = store.snapshot(day(THU), THU_BEGAN).await.unwrap();
    let sent = snapshot.iter().find(|t| t.id == send).unwrap();
    assert!(sent.blocked_by.is_empty());
}
