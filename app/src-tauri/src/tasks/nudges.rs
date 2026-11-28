//! Nudges: the cards shown shortly before a meeting starts or a task is due.

use super::*;

/// A meeting is nudged this long before it starts (R-22, default 2 minutes).
pub const MEETING_LEAD: Duration = Duration::minutes(2);
/// A task with a due time is nudged this long before it (R-22, default 10 minutes).
pub const DUE_LEAD: Duration = Duration::minutes(10);
/// Where "Open Today" on a card goes.
pub const TODAY_LINK: &str = "hyperme://home/today";

/// When a task's due date and time fall, in the zone at `now`.
fn due_at(
    task: &hyperme_core::tasks::Task,
    zone: &dyn Zone,
    now: UtcDateTime,
) -> Option<UtcDateTime> {
    let due = task.due?;
    let time = due.time?;
    Some(
        PlainDateTime::new(due.date, time)
            .assume_offset(zone.offset_at(now))
            .to_utc(),
    )
}

/// The nudges still to come today: each meeting two minutes ahead and each open task with a
/// due time ten minutes ahead. Only moments after `now`, so one already shown is not shown
/// again when the list is computed anew.
pub fn nudges_for(
    meetings: &[sync::Meeting],
    tasks: &[hyperme_core::tasks::Task],
    now: UtcDateTime,
    zone: &dyn Zone,
) -> Vec<hyperme_core::scheduler::Nudge> {
    use hyperme_core::scheduler::Nudge;
    let mut nudges: Vec<Nudge> = meetings
        .iter()
        .map(|m| Nudge {
            id: format!("meeting:{}", ms(m.start)),
            at: m.start - MEETING_LEAD,
            until: m.start,
        })
        .chain(
            tasks
                .iter()
                .filter(|t| t.status == hyperme_core::tasks::Status::Open)
                .filter_map(|t| {
                    let due = due_at(t, zone, now)?;
                    Some(Nudge {
                        id: format!("task:{}", t.id),
                        at: due - DUE_LEAD,
                        until: due,
                    })
                }),
        )
        .filter(|n| n.at > now)
        .collect();
    nudges.sort_by_key(|n| n.at);
    nudges.dedup_by(|a, b| a.id == b.id);
    nudges
}

/// The card for nudge `id` at `now`: "Weekly planning in 2 min" with Join, or a task's title
/// with its due time and Open Today. `None` when the meeting or task is gone.
pub fn nudge_card(
    id: &str,
    meetings: &[sync::Meeting],
    tasks: &[hyperme_core::tasks::Task],
    now: UtcDateTime,
    zone: &dyn Zone,
) -> Option<crate::pill::AlertView> {
    let minutes = |at: UtcDateTime| ((at - now).whole_seconds() + 59).max(60) / 60;
    if let Some(start) = id.strip_prefix("meeting:") {
        let start: i64 = start.parse().ok()?;
        let m = meetings.iter().find(|m| ms(m.start) == start)?;
        let join = m.url.as_deref().filter(|u| u.starts_with("https://"));
        return Some(crate::pill::AlertView {
            title: format!("{} in {} min", m.title, minutes(m.start)),
            detail: format!("{}–{}", hhmm(m.start, zone), hhmm(m.end, zone)),
            action: if join.is_some() { "Join" } else { "Open Today" }.to_owned(),
            link: Some(join.unwrap_or(TODAY_LINK).to_owned()),
        });
    }
    let task_id: TaskId = id.strip_prefix("task:")?.parse().ok()?;
    let task = tasks
        .iter()
        .find(|t| t.id == task_id && t.status == hyperme_core::tasks::Status::Open)?;
    let due = due_at(task, zone, now)?;
    Some(crate::pill::AlertView {
        title: task.title.clone(),
        detail: format!("Due at {} · in {} min", hhmm(due, zone), minutes(due)),
        action: "Open Today".to_owned(),
        link: Some(TODAY_LINK.to_owned()),
    })
}

// --- Home's Today (slice 8) -----------------------------------------------------------------
