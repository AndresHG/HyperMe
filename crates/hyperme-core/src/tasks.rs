//! HyperMe's own task list: the types, the day and due rules, the order of Today, roll-over,
//! the dependency rules and the [`TaskStore`] trait (specs/92-daily-tasks, design.md).
//!
//! Pure logic: no clock, no time zone, no store. Days and due dates are local civil dates,
//! because "Friday" means Friday wherever the Mac is; the platform turns instants into local
//! times and passes them in. Finish and creation times are Unix ms, as the store keeps them.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::future::Future;

use time::{Date, PlainDateTime, Time, Weekday};

use crate::traits::Error;

/// Identifies one task in the store.
pub type TaskId = i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Open,
    Done,
    Dropped,
}

/// Where a task came from (R-1). A mirrored item keeps a link back to its source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Added in HyperMe itself.
    HyperMe,
    /// Mirrors an item from a connected source: a GitHub issue, a calendar event, a mail.
    Mirror {
        /// The source: `github`, `calendar`, `gmail`.
        name: String,
        /// Which account, for one profile with several.
        account: Option<String>,
        /// The item's id at the source.
        reference: String,
        url: Option<String>,
    },
}

/// A hard deadline: a local date, and optionally a local time (T-7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Due {
    pub date: Date,
    pub time: Option<Time>,
}

/// One item on the list (R-1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: TaskId,
    pub title: String,
    pub notes: Option<String>,
    pub status: Status,
    /// The local day the user plans to do it.
    pub day: Option<Date>,
    pub due: Option<Due>,
    pub source: Source,
    /// Unix ms.
    pub created_at: i64,
    /// The day it showed up, for Today and the "new" mark.
    pub arrived_day: Date,
    /// How many day starts found it open with its day in the past (R-13).
    pub rolled: u32,
    /// Unix ms, when done or dropped.
    pub finished_at: Option<i64>,
    /// The tasks this one waits for, in the order they were linked.
    pub blocked_by: Vec<TaskId>,
}

impl Task {
    /// An open local task with no day, due, notes or blockers.
    pub fn new(id: TaskId, title: &str, created_at: i64, arrived_day: Date) -> Task {
        Task {
            id,
            title: title.to_owned(),
            notes: None,
            status: Status::Open,
            day: None,
            due: None,
            source: Source::HyperMe,
            created_at,
            arrived_day,
            rolled: 0,
            finished_at: None,
            blocked_by: Vec::new(),
        }
    }
}

/// A set of weekdays, such as the workdays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WeekdaySet(u8);

impl WeekdaySet {
    pub const EMPTY: WeekdaySet = WeekdaySet(0);

    pub const fn from_array<const N: usize>(days: [Weekday; N]) -> WeekdaySet {
        let mut bits = 0;
        let mut i = 0;
        while i < N {
            bits |= 1 << days[i].number_days_from_monday();
            i += 1;
        }
        WeekdaySet(bits)
    }

    pub const fn contains(self, day: Weekday) -> bool {
        self.0 & (1 << day.number_days_from_monday()) != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// The settings behind "day start", "workday" and the stale mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayRules {
    /// The hour the day turns over: before it, a time belongs to the day before.
    pub day_start: Time,
    pub workdays: WeekdaySet,
    /// Roll-overs after which Today marks a task and asks keep, pick a day or drop (R-14).
    pub stale_after: u32,
}

impl Default for DayRules {
    fn default() -> Self {
        DayRules {
            day_start: Time::from_hms(4, 0, 0).expect("valid time"),
            workdays: WeekdaySet::from_array([
                Weekday::Monday,
                Weekday::Tuesday,
                Weekday::Wednesday,
                Weekday::Thursday,
                Weekday::Friday,
            ]),
            stale_after: 3,
        }
    }
}

impl DayRules {
    /// The day a local time belongs to: 01:00 with a 04:00 day start is the day before.
    pub fn day_of(&self, local: PlainDateTime) -> Date {
        if local.time() < self.day_start {
            local
                .date()
                .previous_day()
                .expect("a day after the first date")
        } else {
            local.date()
        }
    }

    /// The local time `day` begins.
    pub fn starts_at(&self, day: Date) -> PlainDateTime {
        day.with_time(self.day_start)
    }

    pub fn is_workday(&self, day: Date) -> bool {
        self.workdays.contains(day.weekday())
    }

    /// The first workday after `after`. With no workdays set, the next day.
    pub fn next_workday(&self, after: Date) -> Date {
        let next = |d: Date| d.next_day().expect("a day before the last date");
        let mut day = next(after);
        if self.workdays.is_empty() {
            return day;
        }
        while !self.is_workday(day) {
            day = next(day);
        }
        day
    }

    /// Whether Today marks the task as rolled over too often (R-14).
    pub fn is_stale(&self, task: &Task) -> bool {
        task.status == Status::Open && task.rolled >= self.stale_after
    }
}

/// Finds tasks by id. Ids missing from the slice count as finished: the store deletes a
/// task's links with it.
struct Index<'a>(HashMap<TaskId, &'a Task>);

impl<'a> Index<'a> {
    fn new(tasks: &'a [Task]) -> Self {
        Index(tasks.iter().map(|t| (t.id, t)).collect())
    }

    fn get(&self, id: TaskId) -> Option<&'a Task> {
        self.0.get(&id).copied()
    }

    fn first_open_blocker(&self, task: &Task) -> Option<&'a Task> {
        task.blocked_by
            .iter()
            .filter_map(|id| self.get(*id))
            .find(|b| b.status == Status::Open)
    }
}

/// "Ready": open, and every task it is blocked by is done or dropped.
pub fn is_ready(task: &Task, tasks: &[Task]) -> bool {
    task.status == Status::Open && first_open_blocker(task, tasks).is_none()
}

/// The first blocker, in link order, that is still open (R-10).
pub fn first_open_blocker<'a>(task: &Task, tasks: &'a [Task]) -> Option<&'a Task> {
    Index::new(tasks).first_open_blocker(task)
}

/// At day start: every open task whose day is before `today` stays in Today and counts one
/// more roll-over; its day is kept (R-13). Returns the ids rolled. The scheduler runs it once
/// per day start.
pub fn roll_over(tasks: &mut [Task], today: Date) -> Vec<TaskId> {
    tasks
        .iter_mut()
        .filter(|t| t.status == Status::Open && t.day.is_some_and(|d| d < today))
        .map(|t| {
            t.rolled += 1;
            t.id
        })
        .collect()
}

/// Whether linking "`blocked` is blocked by `blocker`" would close a loop (R-7). If so, returns
/// the loop from `blocker` to `blocked`, each task blocked by the next; the new link closes it.
pub fn cycle(tasks: &[Task], blocked: TaskId, blocker: TaskId) -> Option<Vec<TaskId>> {
    let index = Index::new(tasks);
    let mut path = vec![blocker];
    let mut seen = HashSet::new();
    walk(&index, blocked, &mut path, &mut seen).then_some(path)
}

/// Depth-first along `blocked_by` from the last task on `path`, looking for `target`.
fn walk(
    index: &Index<'_>,
    target: TaskId,
    path: &mut Vec<TaskId>,
    seen: &mut HashSet<TaskId>,
) -> bool {
    let current = *path.last().expect("path starts with the blocker");
    if current == target {
        return true;
    }
    if !seen.insert(current) {
        return false;
    }
    let Some(task) = index.get(current) else {
        return false;
    };
    for &next in &task.blocked_by {
        path.push(next);
        if walk(index, target, path, seen) {
            return true;
        }
        path.pop();
    }
    false
}

/// Why a task is in Today, in the order Today shows them (R-17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// Due today or overdue.
    Due,
    /// Its last open blocker was finished today (R-11). `blocker_dropped` when one of its
    /// blockers was dropped rather than done.
    Unblocked { blocker_dropped: bool },
    /// Planned for today.
    Planned,
    /// Planned for an earlier day and rolled over (R-13).
    RolledOver,
    /// Came in today without a day.
    Arrived,
}

impl Group {
    fn rank(self) -> u8 {
        match self {
            Group::Due => 0,
            Group::Unblocked { .. } => 1,
            Group::Planned => 2,
            Group::RolledOver => 3,
            Group::Arrived => 4,
        }
    }
}

/// A ready task in Today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line<'a> {
    pub task: &'a Task,
    pub group: Group,
    /// Rolled over too often (R-14).
    pub stale: bool,
}

/// A task in Today that is not ready, under "Waiting on" (R-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Waiting<'a> {
    pub task: &'a Task,
    /// Its first open blocker.
    pub on: &'a Task,
}

/// The open tasks of a day: ready ones in order, and the ones waiting on blockers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Today<'a> {
    pub lines: Vec<Line<'a>>,
    pub waiting: Vec<Waiting<'a>>,
}

/// Today for `today`, which began at `day_began_ms` (Unix ms, from [`DayRules::starts_at`]
/// in the local zone). `tasks` must hold every task Today needs and the blockers of those.
///
/// Today holds the open tasks whose day or due is `today` or earlier, plus those that came in
/// today without a day. Ready ones are ordered due (by due time, overdue first), unblocked
/// today, planned for today, rolled over (fewest roll-overs first), arrived today; ties by
/// creation time. The rest wait on their first open blocker, by creation time.
pub fn order_today<'a>(
    tasks: &'a [Task],
    today: Date,
    day_began_ms: i64,
    rules: &DayRules,
) -> Today<'a> {
    let index = Index::new(tasks);
    let mut lines = Vec::new();
    let mut waiting = Vec::new();

    for task in tasks.iter().filter(|t| belongs_to(t, today)) {
        if let Some(on) = index.first_open_blocker(task) {
            waiting.push(Waiting { task, on });
            continue;
        }
        lines.push(Line {
            task,
            group: group_of(task, today, day_began_ms, &index),
            stale: rules.is_stale(task),
        });
    }

    lines.sort_by(|a, b| {
        a.group
            .rank()
            .cmp(&b.group.rank())
            .then_with(|| within_group(a, b))
            .then_with(|| by_creation(a.task, b.task))
    });
    waiting.sort_by(|a, b| by_creation(a.task, b.task));
    Today { lines, waiting }
}

fn belongs_to(task: &Task, today: Date) -> bool {
    task.status == Status::Open
        && (task.day.is_some_and(|d| d <= today)
            || task.due.is_some_and(|d| d.date <= today)
            || (task.day.is_none() && task.arrived_day == today))
}

fn group_of(task: &Task, today: Date, day_began_ms: i64, index: &Index<'_>) -> Group {
    if task.due.is_some_and(|d| d.date <= today) {
        return Group::Due;
    }
    let blockers: Vec<&Task> = task
        .blocked_by
        .iter()
        .filter_map(|id| index.get(*id))
        .collect();
    let unblocked_at = blockers.iter().filter_map(|b| b.finished_at).max();
    if unblocked_at.is_some_and(|at| at >= day_began_ms) {
        let blocker_dropped = blockers.iter().any(|b| b.status == Status::Dropped);
        return Group::Unblocked { blocker_dropped };
    }
    match task.day {
        Some(d) if d == today => Group::Planned,
        Some(_) => Group::RolledOver,
        None => Group::Arrived,
    }
}

fn within_group(a: &Line<'_>, b: &Line<'_>) -> Ordering {
    match a.group {
        // A due date without a time sorts after the timed ones of that date.
        Group::Due => {
            let key = |t: &Task| t.due.map(|d| (d.date, d.time.is_none(), d.time));
            key(a.task).cmp(&key(b.task))
        }
        Group::RolledOver => a.task.rolled.cmp(&b.task.rolled),
        _ => Ordering::Equal,
    }
}

fn by_creation(a: &Task, b: &Task) -> Ordering {
    a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id))
}

/// A task to add. The caller passes the times, so the store reads no clock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTask {
    pub title: String,
    pub notes: Option<String>,
    pub day: Option<Date>,
    pub due: Option<Due>,
    pub source: Source,
    pub blocked_by: Vec<TaskId>,
    /// Unix ms.
    pub created_at: i64,
    pub arrived_day: Date,
}

/// The fields to change; `None` leaves a field as it is, `Some(None)` clears it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskUpdate {
    pub title: Option<String>,
    pub notes: Option<Option<String>>,
    pub day: Option<Option<Date>>,
    pub due: Option<Option<Due>>,
}

/// How a task ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finish {
    Done,
    Dropped,
}

/// What a task store refuses or fails at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskError {
    NotFound(TaskId),
    /// The link would close a loop (R-7): the tasks from the blocker to the blocked task, each
    /// blocked by the next, as [`cycle`] returns them.
    Cycle(Vec<TaskId>),
    /// The store itself failed.
    Store(Error),
}

impl fmt::Display for TaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskError::NotFound(id) => write!(f, "no task {id}"),
            TaskError::Cycle(path) => {
                let ids: Vec<String> = path.iter().map(|id| id.to_string()).collect();
                write!(f, "the link would make a loop: {}", ids.join(" ← "))
            }
            TaskError::Store(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for TaskError {}

impl From<Error> for TaskError {
    fn from(e: Error) -> Self {
        TaskError::Store(e)
    }
}

/// The task list on disk (SQLite in `hyperme-platform`, slice 2). Async methods return
/// `impl Future + Send`, as in [`crate::traits`]; implementations may write `async fn`.
pub trait TaskStore {
    /// Adds a task, with its blockers. A blocker that does not exist is `NotFound`.
    fn add(&self, task: NewTask) -> impl Future<Output = Result<TaskId, TaskError>> + Send;

    fn update(
        &self,
        id: TaskId,
        change: TaskUpdate,
    ) -> impl Future<Output = Result<(), TaskError>> + Send;

    /// Marks a task done or dropped at `at_ms` (Unix ms).
    fn finish(
        &self,
        id: TaskId,
        how: Finish,
        at_ms: i64,
    ) -> impl Future<Output = Result<(), TaskError>> + Send;

    /// Links "`blocked` is blocked by `blocker`". A link that would close a loop is refused
    /// with [`TaskError::Cycle`] (R-7).
    fn block(
        &self,
        blocked: TaskId,
        blocker: TaskId,
    ) -> impl Future<Output = Result<(), TaskError>> + Send;

    fn unblock(
        &self,
        blocked: TaskId,
        blocker: TaskId,
    ) -> impl Future<Output = Result<(), TaskError>> + Send;

    /// Applies [`roll_over`] for the day start of `today` and returns the ids rolled.
    fn roll_over(&self, today: Date)
    -> impl Future<Output = Result<Vec<TaskId>, TaskError>> + Send;

    /// Every task [`order_today`] needs for `today`: the open tasks that belong to it, the
    /// tasks finished since `day_began_ms`, and the blockers of all of those.
    fn snapshot(
        &self,
        today: Date,
        day_began_ms: i64,
    ) -> impl Future<Output = Result<Vec<Task>, TaskError>> + Send;
}
