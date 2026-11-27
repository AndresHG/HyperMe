//! Connector pulls (specs/97-google-connectors, slices 7 and 8): the sync jobs on the #92
//! scheduler (R-16), a preset's declarative pull run without a model (G-5), the events cache
//! with its retention (R-19), what the daily rhythm receives (R-25) and mirrored tasks (R-27).
//!
//! The tool call goes through [`ToolCaller`], which the app backs with the remote MCP client
//! and its read-only allowlist (R-22); the cache through [`EventStore`], which SQLite backs in
//! `hyperme-platform`. No network and no clock here: callers pass `now` and the zone.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Mutex;

use serde_json::{Map, Value};
use time::format_description::well_known::Rfc3339;
use time::{Date, Duration, OffsetDateTime, Time, UtcDateTime};

use crate::config::Connectors;
use crate::registry::{Pull, Registry};
use crate::schedule::{Schedule, Zone};
use crate::scheduler::{Job, JobKind};
use crate::tasks::{NewTask, Source, TaskStore};

/// How often a sync job runs while the user is present (R-16).
pub const SYNC_EVERY_MINUTES: u32 = 30;

/// The cache keeps events this long after they end (R-19)...
pub const KEEP_BEHIND: Duration = Duration::days(1);
/// ...and this far ahead of now (today and the next 7 days).
pub const KEEP_AHEAD: Duration = Duration::days(8);

/// The sync jobs (R-16): one per connector server with a pull, named
/// `sync:<connector>-<server>`, every 30 minutes while present, for connectors with at least
/// one account. Each may call only its pulls' tools, which the registry checked are read tools.
pub fn sync_jobs(registry: &Registry, connectors: &Connectors) -> Vec<Job> {
    let mut jobs: Vec<Job> = Vec::new();
    for connector in registry.connectors() {
        if !connectors
            .accounts
            .iter()
            .any(|a| a.connector == connector.id)
        {
            continue;
        }
        for pull in &connector.pulls {
            let id = format!("{}-{}", connector.id, pull.server);
            let name = format!("sync:{id}");
            match jobs.iter_mut().find(|job| job.name == name) {
                Some(job) if !job.allowlist.contains(&pull.tool) => {
                    job.allowlist.push(pull.tool.clone());
                }
                Some(_) => {}
                None => {
                    let mut job = Job::new(
                        &name,
                        JobKind::Sync(id),
                        Schedule::EveryWhilePresent {
                            minutes: SYNC_EVERY_MINUTES,
                        },
                    );
                    job.allowlist = vec![pull.tool.clone()];
                    jobs.push(job);
                }
            }
        }
    }
    jobs
}

/// Calls one tool on one of the account's servers and returns its answer as JSON.
pub trait ToolCaller {
    fn call(
        &self,
        server: &str,
        tool: &str,
        args: Value,
    ) -> impl Future<Output = Result<Value, String>> + Send;
}

/// One calendar event in the cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// The account, `google:personal`.
    pub account: String,
    pub id: String,
    pub calendar: Option<String>,
    pub title: String,
    pub start: UtcDateTime,
    pub end: UtcDateTime,
    pub all_day: bool,
    /// The meeting link, or the event's own page.
    pub url: Option<String>,
    pub status: Option<String>,
}

impl Event {
    fn cancelled(&self) -> bool {
        self.status.as_deref() == Some("cancelled")
    }
}

/// The events cache (design.md, "Storage").
pub trait EventStore: Send + Sync {
    /// Replaces the account's events that start in `[from, to)` with `events`.
    fn replace(
        &self,
        account: &str,
        from: UtcDateTime,
        to: UtcDateTime,
        events: &[Event],
    ) -> Result<(), String>;
    /// Events that start in `[from, to)`, of one account or all, by start time.
    fn between(
        &self,
        account: Option<&str>,
        from: UtcDateTime,
        to: UtcDateTime,
    ) -> Result<Vec<Event>, String>;
    /// Deletes events that ended more than a day ago or start more than 8 days from `now`;
    /// returns how many (R-19).
    fn retain(&self, now: UtcDateTime) -> Result<usize, String>;
}

/// Whether an event is outside the window the cache keeps around `now`.
pub fn outside_retention(event: &Event, now: UtcDateTime) -> bool {
    event.end < now - KEEP_BEHIND || event.start >= now + KEEP_AHEAD
}

/// An in-memory [`EventStore`] for tests.
#[derive(Debug, Default)]
pub struct MemoryEvents {
    events: Mutex<Vec<Event>>,
}

impl MemoryEvents {
    fn events(&self) -> std::sync::MutexGuard<'_, Vec<Event>> {
        self.events.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl EventStore for MemoryEvents {
    fn replace(
        &self,
        account: &str,
        from: UtcDateTime,
        to: UtcDateTime,
        events: &[Event],
    ) -> Result<(), String> {
        let mut all = self.events();
        all.retain(|e| e.account != account || e.start < from || e.start >= to);
        all.extend(events.iter().cloned());
        Ok(())
    }

    fn between(
        &self,
        account: Option<&str>,
        from: UtcDateTime,
        to: UtcDateTime,
    ) -> Result<Vec<Event>, String> {
        let mut found: Vec<Event> = self
            .events()
            .iter()
            .filter(|e| account.is_none_or(|a| e.account == a) && e.start >= from && e.start < to)
            .cloned()
            .collect();
        found.sort_by_key(|e| (e.start, e.id.clone()));
        Ok(found)
    }

    fn retain(&self, now: UtcDateTime) -> Result<usize, String> {
        let mut all = self.events();
        let before = all.len();
        all.retain(|e| !outside_retention(e, now));
        Ok(before - all.len())
    }
}

impl crate::connectors::AccountData for MemoryEvents {
    fn delete_account(&self, account: &str) -> Result<(), String> {
        self.events().retain(|e| e.account != account);
        Ok(())
    }
}

/// Local midnight of `day` in `zone`, as UTC.
fn local_midnight(day: Date, zone: &dyn Zone) -> OffsetDateTime {
    // The offset at noon of that day avoids a DST change at midnight picking the wrong side.
    let noon = UtcDateTime::new(day, Time::from_hms(12, 0, 0).unwrap_or(Time::MIDNIGHT));
    let offset = zone.offset_at(noon);
    day.with_time(Time::MIDNIGHT).assume_offset(offset)
}

fn local_day(at: UtcDateTime, zone: &dyn Zone) -> Date {
    at.to_offset(zone.offset_at(at)).date()
}

/// The window a calendar pull asks for: from this morning's local midnight to the end of the
/// 7th day after.
fn window(now: UtcDateTime, zone: &dyn Zone) -> (OffsetDateTime, OffsetDateTime) {
    let today = local_day(now, zone);
    let end_day = today.checked_add(Duration::days(8)).unwrap_or(today);
    (local_midnight(today, zone), local_midnight(end_day, zone))
}

/// The pull's arguments with `{today_start}`, `{today_plus_7}` and `{now}` filled in.
pub fn expand_args(args: &BTreeMap<String, String>, now: UtcDateTime, zone: &dyn Zone) -> Value {
    let (start, end) = window(now, zone);
    let text = |at: OffsetDateTime| at.format(&Rfc3339).unwrap_or_default();
    let now_text = text(now.to_offset(zone.offset_at(now)));
    let mut map = Map::new();
    for (key, value) in args {
        let filled = value
            .replace("{today_start}", &text(start))
            .replace("{today_plus_7}", &text(end))
            .replace("{now}", &now_text);
        map.insert(key.clone(), Value::String(filled));
    }
    Value::Object(map)
}

/// The value at a dotted path, trying each `|` alternative in turn.
fn lookup<'a>(item: &'a Value, paths: &str) -> Option<&'a Value> {
    paths.split('|').find_map(|path| {
        path.trim()
            .split('.')
            .try_fold(item, |value, key| value.get(key))
            .filter(|value| !value.is_null())
    })
}

fn text_at(item: &Value, fields: &BTreeMap<String, String>, field: &str) -> Option<String> {
    let value = lookup(item, fields.get(field)?)?;
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
    .filter(|s| !s.is_empty())
}

/// A time as RFC 3339 (a timed event) or a bare date (an all-day event, local midnight).
fn time_at(text: &str, zone: &dyn Zone) -> Option<(UtcDateTime, bool)> {
    if let Ok(at) = OffsetDateTime::parse(text, &Rfc3339) {
        return Some((at.to_utc(), false));
    }
    let mut parts = text.splitn(3, '-').map(str::parse::<i32>);
    let (Some(Ok(year)), Some(Ok(month)), Some(Ok(day))) =
        (parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    let month = time::Month::try_from(u8::try_from(month).ok()?).ok()?;
    let day = Date::from_calendar_date(year, month, u8::try_from(day).ok()?).ok()?;
    Some((local_midnight(day, zone).to_utc(), true))
}

/// The items of a tool's answer: the list at `pull.list`, or the answer itself.
fn items<'a>(answer: &'a Value, pull: &Pull) -> Result<&'a Vec<Value>, String> {
    let list = if pull.list.is_empty() {
        Some(answer)
    } else {
        lookup(answer, &pull.list)
    };
    list.and_then(Value::as_array).ok_or_else(|| {
        let at = if pull.list.is_empty() {
            "the answer".to_owned()
        } else {
            format!("\"{}\"", pull.list)
        };
        format!("{}: expected a list of items at {at}", pull.tool)
    })
}

/// What one pull did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PullReport {
    pub events: usize,
    /// Items without the fields an event needs.
    pub skipped: usize,
    pub mirrored: usize,
}

/// Runs a calendar pull for one account (G-5): calls the tool with the window's arguments,
/// maps the items into events, replaces the account's window in the cache, applies retention,
/// and, when the pull mirrors, upserts tasks and closes those whose event was cancelled or is
/// gone (R-27). A wrong answer changes nothing.
pub async fn pull_events<C: ToolCaller, T: TaskStore>(
    caller: &C,
    store: &dyn EventStore,
    tasks: &T,
    account: &str,
    pull: &Pull,
    now: UtcDateTime,
    zone: &(dyn Zone + Sync),
) -> Result<PullReport, String> {
    let args = expand_args(&pull.args, now, zone);
    let answer = caller.call(&pull.server, &pull.tool, args).await?;
    let mut report = PullReport::default();
    let mut events = Vec::new();
    for item in items(&answer, pull)? {
        let field = |name: &str| text_at(item, &pull.fields, name);
        let parsed = (|| {
            let (start, all_day) = time_at(&field("start")?, zone)?;
            let (end, _) = time_at(&field("end")?, zone)?;
            Some(Event {
                account: account.to_owned(),
                id: field("id")?,
                calendar: field("calendar"),
                title: field("title").unwrap_or_else(|| "(no title)".to_owned()),
                start,
                end,
                all_day,
                url: field("url"),
                status: field("status"),
            })
        })();
        match parsed {
            Some(event) => events.push(event),
            None => report.skipped += 1,
        }
    }
    let (from, to) = window(now, zone);
    let (from, to) = (from.to_utc(), to.to_utc());
    let before = store.between(Some(account), from, to)?;
    store.replace(account, from, to, &events)?;
    store.retain(now)?;
    report.events = events.len();

    if pull.mirror {
        let closed_at = (now - UtcDateTime::UNIX_EPOCH).whole_milliseconds() as i64;
        let gone = before
            .iter()
            .filter(|old| !events.iter().any(|new| new.id == old.id));
        for (event, closed) in events
            .iter()
            .map(|e| (e, e.cancelled()))
            .chain(gone.map(|e| (e, true)))
        {
            let task = NewTask {
                title: event.title.clone(),
                notes: None,
                day: Some(local_day(event.start, zone)),
                due: None,
                source: Source::Mirror {
                    name: pull.server.clone(),
                    account: Some(account.to_owned()),
                    reference: event.id.clone(),
                    url: event.url.clone(),
                },
                blocked_by: Vec::new(),
                created_at: closed_at,
                arrived_day: local_day(now, zone),
            };
            tasks
                .upsert_mirror(task, closed.then_some(closed_at))
                .await
                .map_err(|e| e.to_string())?;
            if !closed {
                report.mirrored += 1;
            }
        }
    }
    Ok(report)
}

/// One of today's meetings, as the daily rhythm receives it (R-25).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meeting {
    pub title: String,
    pub start: UtcDateTime,
    pub end: UtcDateTime,
    pub account: String,
    pub url: Option<String>,
}

/// The meetings on local `day` from every account, by start, from the cache only (no network
/// on a card's path, R-16). Cancelled and all-day events are left out.
pub fn meetings_on(
    store: &dyn EventStore,
    day: Date,
    zone: &dyn Zone,
) -> Result<Vec<Meeting>, String> {
    let next = day.checked_add(Duration::days(1)).unwrap_or(day);
    let from = local_midnight(day, zone).to_utc();
    let to = local_midnight(next, zone).to_utc();
    Ok(store
        .between(None, from, to)?
        .into_iter()
        .filter(|e| !e.cancelled() && !e.all_day)
        .map(|e| Meeting {
            title: e.title,
            start: e.start,
            end: e.end,
            account: e.account,
            url: e.url,
        })
        .collect())
}
