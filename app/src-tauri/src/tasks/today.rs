//! The Today view in Home: meetings, task lines and what is waiting, and the actions on a row.

use super::*;

/// One of today's meetings as Home lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct MeetingRow {
    pub title: String,
    /// Local times, `09:30`.
    pub start: String,
    pub end: String,
    /// A link to join or open the event.
    pub link: Option<String>,
    /// Minutes until the next meeting starts; set on that one only.
    pub starts_in_min: Option<i64>,
    /// Under way now.
    pub now: bool,
    /// Over already.
    pub past: bool,
}

/// One task line in Today.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct LineRow {
    pub id: TaskId,
    pub title: String,
    /// Small marks after the title: `new`, `unblocked`, `blocker dropped`, `due 16:00`,
    /// `overdue` (R-11, R-21).
    pub marks: Vec<String>,
    /// Rolled over too often: ask keep, pick a day or drop (R-14).
    pub stale: bool,
    pub rolled: u32,
}

/// A task under "Waiting on", with the blocker's title (R-10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct WaitingRow {
    pub id: TaskId,
    pub title: String,
    pub on: String,
    pub new: bool,
}

/// The Today block: meetings, then the tasks in order, then "Waiting on".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct TodayPage {
    /// `YYYY-MM-DD`, today and tomorrow, for the day picker.
    pub today: String,
    pub tomorrow: String,
    pub meetings: Vec<MeetingRow>,
    pub lines: Vec<LineRow>,
    pub waiting: Vec<WaitingRow>,
    pub done_today: usize,
    /// Something arrived that the user has not seen (R-21): the attention dot.
    pub has_new: bool,
}

pub(super) fn hhmm(at: UtcDateTime, zone: &dyn Zone) -> String {
    let t = at.to_offset(zone.offset_at(at)).time();
    format!("{:02}:{:02}", t.hour(), t.minute())
}

/// Builds the Today block from the view and today's meetings at `now`.
pub fn today_page(
    view: &TodayView,
    meetings: &[sync::Meeting],
    now: UtcDateTime,
    zone: &dyn Zone,
) -> TodayPage {
    let next = meetings.iter().position(|m| m.start > now);
    let meetings = meetings
        .iter()
        .enumerate()
        .map(|(i, m)| MeetingRow {
            title: m.title.clone(),
            start: hhmm(m.start, zone),
            end: hhmm(m.end, zone),
            link: m.url.clone(),
            // Rounded up, so a meeting 30 s away is "in 1 min", never "in 0 min".
            starts_in_min: (Some(i) == next).then(|| ((m.start - now).whole_seconds() + 59) / 60),
            now: m.start <= now && now < m.end,
            past: m.end <= now,
        })
        .collect();
    let lines = view
        .lines
        .iter()
        .map(|l| {
            let mut marks = Vec::new();
            if l.new {
                marks.push("new".to_owned());
            }
            match l.group {
                Group::Unblocked {
                    blocker_dropped: true,
                } => marks.push("blocker dropped".to_owned()),
                Group::Unblocked { .. } => marks.push("unblocked".to_owned()),
                _ => {}
            }
            if let Some(due) = l.due {
                if due.date < view.date {
                    marks.push("overdue".to_owned());
                } else if let (true, Some(t)) = (due.date == view.date, due.time) {
                    marks.push(format!("due {:02}:{:02}", t.hour(), t.minute()));
                }
            }
            LineRow {
                id: l.id,
                title: l.title.clone(),
                marks,
                stale: l.stale,
                rolled: l.rolled,
            }
        })
        .collect::<Vec<_>>();
    let waiting = view
        .waiting
        .iter()
        .map(|w| WaitingRow {
            id: w.id,
            title: w.title.clone(),
            on: w.on.clone(),
            new: w.new,
        })
        .collect::<Vec<_>>();
    let has_new = view.lines.iter().any(|l| l.new) || view.waiting.iter().any(|w| w.new);
    TodayPage {
        today: view.date.to_string(),
        tomorrow: view.date.next_day().unwrap_or(view.date).to_string(),
        meetings,
        lines,
        waiting,
        done_today: view.done_today,
        has_new,
    }
}

/// A `YYYY-MM-DD` day, as the page's day picker sends it.
pub fn parse_day(text: &str) -> Option<Date> {
    let mut parts = text.splitn(3, '-').map(str::parse::<i32>);
    let (y, m, d) = (
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    );
    let month = time::Month::try_from(u8::try_from(m).ok()?).ok()?;
    Date::from_calendar_date(y, month, u8::try_from(d).ok()?).ok()
}

/// What a row in Today does (R-4, R-14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    Done,
    Drop,
    /// Plan it for tomorrow.
    Tomorrow,
    /// Plan it for this day.
    Move(Date),
    Rename(String),
    /// Keep a stale task in today, asking no more (R-14).
    Keep,
}

/// Adds a task for today from Home's one-line field (R-1, R-4).
pub async fn add_today(
    store: &SqliteTaskStore,
    title: &str,
    today: Date,
    now_ms: i64,
) -> Result<TaskId, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Type what to do".to_owned());
    }
    let id = store
        .add(NewTask {
            title: title.to_owned(),
            notes: None,
            day: Some(today),
            due: None,
            source: Source::HyperMe,
            blocked_by: Vec::new(),
            created_at: now_ms,
            arrived_day: today,
        })
        .await
        .map_err(|e| e.to_string())?;
    store
        .count(today, hyperme_platform::task_store::DayCounter::Captured, 1)
        .map_err(|e| e.to_string())?;
    Ok(id)
}

/// Applies a row action and counts it in the day log (R-4, R-29).
pub async fn act(
    store: &SqliteTaskStore,
    id: TaskId,
    action: Act,
    today: Date,
    now_ms: i64,
) -> Result<(), String> {
    use hyperme_platform::task_store::DayCounter;
    let e = |e: hyperme_core::tasks::TaskError| e.to_string();
    let day = |day: Date| TaskUpdate {
        title: None,
        notes: None,
        day: Some(Some(day)),
        due: None,
    };
    match action {
        Act::Done => {
            store.finish(id, Finish::Done, now_ms).await.map_err(e)?;
            store.count(today, DayCounter::Done, 1).map_err(e)?;
        }
        Act::Drop => {
            store.finish(id, Finish::Dropped, now_ms).await.map_err(e)?;
            store.count(today, DayCounter::Dropped, 1).map_err(e)?;
        }
        Act::Tomorrow | Act::Move(_) => {
            let to = match action {
                Act::Move(to) => to,
                _ => today.next_day().ok_or("no tomorrow")?,
            };
            if to < today {
                return Err("Pick today or a later day".to_owned());
            }
            store.update(id, day(to)).await.map_err(e)?;
            if to > today {
                store.count(today, DayCounter::Moved, 1).map_err(e)?;
            }
        }
        Act::Rename(title) => {
            let title = title.trim();
            if title.is_empty() {
                return Err("A task needs a title".to_owned());
            }
            store
                .update(
                    id,
                    TaskUpdate {
                        title: Some(title.to_owned()),
                        notes: None,
                        day: None,
                        due: None,
                    },
                )
                .await
                .map_err(e)?;
        }
        Act::Keep => store.keep(id, today).map_err(e)?,
    }
    Ok(())
}

// --- The app's wiring ----------------------------------------------------------------------
