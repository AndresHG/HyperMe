//! The card models for Today and the morning card (specs/92-daily-tasks, design.md, "The
//! cards"; R-14, R-17, R-18, R-21). Pure builders: a snapshot of tasks and what other parts
//! already know go in, a model for the web view comes out. No store, no network, no model call.
//!
//! The evening card is deferred (owner, 26 November 2026).

use std::collections::HashMap;

use time::{Date, Time};

use crate::tasks::{DayRules, Due, Group, Status, Task, TaskId, order_today};

/// How many tasks the morning card lists (R-17).
pub const MORNING_TOP: usize = 5;

/// A part of a card that needs a sync: "Updating…" until it returns (R-18).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part<T> {
    Pending,
    Ready(T),
}

/// One of today's meetings, in local time, from the calendar connector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meeting {
    pub title: String,
    pub start: Time,
    pub end: Time,
    pub account: Option<String>,
    pub link: Option<String>,
}

/// One day's counts from the day log (R-29).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DayCounts {
    pub captured: u32,
    pub done: u32,
    pub moved: u32,
    pub dropped: u32,
}

/// The moment a card is built for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Now {
    pub today: Date,
    /// When today began, Unix ms ([`DayRules::starts_at`] in the local zone).
    pub day_began_ms: i64,
    pub rules: DayRules,
    /// When today's morning card showed, Unix ms; items after it are "new" (R-21).
    pub morning_shown_ms: Option<i64>,
    /// When the user last opened Today, Unix ms; it clears "new".
    pub today_seen_ms: Option<i64>,
}

/// What other parts know: the calendar and mail syncs, proposals waiting for a yes, and
/// yesterday's line of the day log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sources {
    pub meetings: Part<Vec<Meeting>>,
    pub mail_to_answer: Part<u32>,
    pub proposals: u32,
    pub yesterday: Option<DayCounts>,
}

/// A task as a card shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskLine {
    pub id: TaskId,
    pub title: String,
    pub group: Group,
    pub due: Option<Due>,
    pub rolled: u32,
    /// Rolled over too often: ask keep, pick a day or drop (R-14).
    pub stale: bool,
    /// Arrived after the morning card and not yet seen (R-21).
    pub new: bool,
}

/// A task under "Waiting on", with its first open blocker's title (R-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitingLine {
    pub id: TaskId,
    pub title: String,
    pub on: String,
    pub new: bool,
}

/// The answers to the stale question (R-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaleChoice {
    Keep,
    PickDay,
    Drop,
}

impl StaleChoice {
    pub const ALL: [StaleChoice; 3] = [StaleChoice::Keep, StaleChoice::PickDay, StaleChoice::Drop];
}

/// Today, for Home and the quick sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodayView {
    pub date: Date,
    pub lines: Vec<TaskLine>,
    pub waiting: Vec<WaitingLine>,
    /// The stale lines again, for the question (R-14).
    pub stale: Vec<TaskLine>,
    /// Tasks done since day start.
    pub done_today: usize,
}

/// The morning card (R-17). Parts that do not exist are empty or `None`; the view hides them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MorningCard {
    pub date: Date,
    pub meetings: Part<Vec<Meeting>>,
    /// Up to five tasks for today, in order.
    pub top: Vec<TaskLine>,
    /// How many more tasks Today holds.
    pub more: usize,
    pub waiting_on: usize,
    pub proposals: u32,
    pub mail_to_answer: Part<u32>,
    pub yesterday: Option<DayCounts>,
    pub stale: Vec<TaskLine>,
}

/// Today from a snapshot (R-10, R-17, R-21).
pub fn today(snapshot: &[Task], now: &Now) -> TodayView {
    let order = order_today(snapshot, now.today, now.day_began_ms, &now.rules);
    let by_id: HashMap<TaskId, &Task> = snapshot.iter().map(|t| (t.id, t)).collect();
    let new_after = new_after(now);
    let is_new = |task: &Task| new_after.is_some_and(|after| arrived_at(task, &by_id) > after);

    let lines: Vec<TaskLine> = order
        .lines
        .iter()
        .map(|l| TaskLine {
            id: l.task.id,
            title: l.task.title.clone(),
            group: l.group,
            due: l.task.due,
            rolled: l.task.rolled,
            stale: l.stale,
            new: is_new(l.task),
        })
        .collect();
    let waiting = order
        .waiting
        .iter()
        .map(|w| WaitingLine {
            id: w.task.id,
            title: w.task.title.clone(),
            on: w.on.title.clone(),
            new: is_new(w.task),
        })
        .collect();
    let stale = lines.iter().filter(|l| l.stale).cloned().collect();
    let done_today = snapshot
        .iter()
        .filter(|t| t.status == Status::Done)
        .filter(|t| t.finished_at.is_some_and(|at| at >= now.day_began_ms))
        .count();
    TodayView {
        date: now.today,
        lines,
        waiting,
        stale,
        done_today,
    }
}

/// The morning card from a snapshot and what the syncs know so far (R-17, R-18).
pub fn morning(snapshot: &[Task], now: &Now, sources: &Sources) -> MorningCard {
    let view = today(snapshot, now);
    let more = view.lines.len().saturating_sub(MORNING_TOP);
    let mut top = view.lines;
    top.truncate(MORNING_TOP);
    let meetings = match &sources.meetings {
        Part::Pending => Part::Pending,
        Part::Ready(list) => {
            let mut list = list.clone();
            list.sort_by_key(|m| m.start);
            Part::Ready(list)
        }
    };
    MorningCard {
        date: now.today,
        meetings,
        top,
        more,
        waiting_on: view.waiting.len(),
        proposals: sources.proposals,
        mail_to_answer: sources.mail_to_answer.clone(),
        yesterday: sources.yesterday,
        stale: view.stale,
    }
}

/// Items after this instant are new: the later of the morning card and the last look at
/// Today. Nothing is new before the morning card.
fn new_after(now: &Now) -> Option<i64> {
    let shown = now.morning_shown_ms?;
    Some(now.today_seen_ms.map_or(shown, |seen| seen.max(shown)))
}

/// When a task arrived in Today: created, or unblocked by its last blocker finishing.
fn arrived_at(task: &Task, by_id: &HashMap<TaskId, &Task>) -> i64 {
    let unblocked = task
        .blocked_by
        .iter()
        .filter_map(|id| by_id.get(id))
        .filter_map(|b| b.finished_at)
        .max();
    unblocked.map_or(task.created_at, |at| at.max(task.created_at))
}
