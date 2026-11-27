//! Slice 7 of specs/97-google-connectors: the calendar pull as data (G-5), its events cache,
//! retention, what the daily rhythm receives (R-25) and mirrored tasks (R-27). The tool is a
//! fake that answers with synthetic events; no network. Tests cite
//! `specs/97-google-connectors/spec.md`.

#[path = "common/tasks.rs"]
mod tasks_store;

use std::path::Path;
use std::sync::Mutex;

use hyperme_core::config::{Config, Connectors, DEFAULT_CONFIG};
use hyperme_core::connector_settings as cs;
use hyperme_core::registry::Pull;
use hyperme_core::schedule::{Schedule, Zone};
use hyperme_core::scheduler::JobKind;
use hyperme_core::sync::{self, EventStore, MemoryEvents, ToolCaller};
use hyperme_core::tasks::{Source, Status, TaskStore};
use serde_json::{Value, json};
use tasks_store::MemoryTaskStore;
use time::{Date, Month, Time, UtcDateTime, UtcOffset};

/// Madrid in winter, UTC+1.
struct Plus1;

impl Zone for Plus1 {
    fn offset_at(&self, _at: UtcDateTime) -> UtcOffset {
        UtcOffset::from_hms(1, 0, 0).unwrap()
    }
}

fn utc(day: u8, h: u8, m: u8) -> UtcDateTime {
    UtcDateTime::new(
        Date::from_calendar_date(2026, Month::November, day).unwrap(),
        Time::from_hms(h, m, 0).unwrap(),
    )
}

fn date(day: u8) -> Date {
    Date::from_calendar_date(2026, Month::November, day).unwrap()
}

/// Answers `list_events` with the items it holds, and records the arguments.
#[derive(Default)]
struct FakeCalendar {
    items: Mutex<Vec<Value>>,
    calls: Mutex<Vec<(String, String, Value)>>,
}

impl FakeCalendar {
    fn set(&self, items: Vec<Value>) {
        *self.items.lock().unwrap() = items;
    }
}

impl ToolCaller for FakeCalendar {
    async fn call(&self, server: &str, tool: &str, args: Value) -> Result<Value, String> {
        self.calls
            .lock()
            .unwrap()
            .push((server.to_owned(), tool.to_owned(), args));
        Ok(json!({ "kind": "calendar#events", "items": self.items.lock().unwrap().clone() }))
    }
}

fn event(id: &str, title: &str, start: &str, end: &str) -> Value {
    json!({
        "id": id,
        "summary": title,
        "start": { "dateTime": start },
        "end": { "dateTime": end },
        "hangoutLink": format!("https://meet.example/{id}"),
        "htmlLink": format!("https://calendar.example/{id}"),
        "status": "confirmed",
    })
}

fn google_pull() -> Pull {
    let registry = cs::registry(None, &Connectors::default()).unwrap();
    registry.get("google").unwrap().pulls[0].clone()
}

/// R-16, AC-8: one sync job per connector server with a pull, every 30 minutes while the user
/// is present, allowed only the pull's read tool, for connectors with an account.
#[test]
fn sync_jobs_come_from_the_presets_with_accounts() {
    let empty = Connectors::default();
    let registry = cs::registry(None, &empty).unwrap();
    assert!(
        sync::sync_jobs(&registry, &empty).is_empty(),
        "no account, no job"
    );

    let connectors = Config::parse(
        &format!(
            "{DEFAULT_CONFIG}\n[[connectors.accounts]]\nconnector = \"google\"\nlabel = \"personal\"\n"
        ),
        Path::new("config.toml"),
    )
    .unwrap()
    .connectors;
    let registry = cs::registry(None, &connectors).unwrap();
    let jobs = sync::sync_jobs(&registry, &connectors);
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].name, "sync:google-calendar");
    assert_eq!(jobs[0].kind, JobKind::Sync("google-calendar".to_owned()));
    assert_eq!(
        jobs[0].schedule,
        Schedule::EveryWhilePresent { minutes: 30 }
    );
    assert_eq!(jobs[0].allowlist, ["list_events"]);
}

/// G-5: the arguments are filled in with today's start and the end of the 7th day after, in
/// local time; the answer's items become events with UTC times; all-day events keep their day.
#[tokio::test]
async fn the_pull_maps_the_answer_into_events() {
    let calendar = FakeCalendar::default();
    calendar.set(vec![
        event(
            "a1",
            "Planning",
            "2026-11-27T10:00:00+01:00",
            "2026-11-27T10:30:00+01:00",
        ),
        json!({
            "id": "a2", "summary": "Holiday", "status": "confirmed",
            "start": { "date": "2026-11-28" }, "end": { "date": "2026-11-29" },
            "htmlLink": "https://calendar.example/a2",
        }),
        json!({ "summary": "no id, skipped" }),
    ]);
    let store = MemoryEvents::default();
    let tasks = MemoryTaskStore::default();
    let now = utc(27, 8, 0);
    let report = sync::pull_events(
        &calendar,
        &store,
        &tasks,
        "google:personal",
        &google_pull(),
        now,
        &Plus1,
    )
    .await
    .unwrap();
    assert_eq!(report.events, 2);
    assert_eq!(report.skipped, 1);

    let (server, tool, args) = calendar.calls.lock().unwrap()[0].clone();
    assert_eq!(
        (server.as_str(), tool.as_str()),
        ("calendar", "list_events")
    );
    assert_eq!(args["timeMin"], "2026-11-27T00:00:00+01:00");
    assert_eq!(args["timeMax"], "2026-12-05T00:00:00+01:00");

    let stored = store.between(None, utc(26, 0, 0), utc(30, 0, 0)).unwrap();
    assert_eq!(stored.len(), 2);
    assert_eq!(stored[0].title, "Planning");
    assert_eq!(stored[0].start, utc(27, 9, 0));
    assert_eq!(stored[0].url.as_deref(), Some("https://meet.example/a1"));
    assert!(!stored[0].all_day);
    assert!(stored[1].all_day);
    assert_eq!(
        stored[1].start,
        utc(27, 23, 0),
        "all-day starts at local midnight"
    );
}

/// R-25: the daily rhythm gets today's meetings as {title, start, end, account, url}, from
/// the cache only, every account together, cancelled ones left out.
#[tokio::test]
async fn todays_meetings_come_from_the_cache() {
    let calendar = FakeCalendar::default();
    let store = MemoryEvents::default();
    let tasks = MemoryTaskStore::default();
    let now = utc(27, 8, 0);
    calendar.set(vec![
        event(
            "p1",
            "Dentist",
            "2026-11-27T17:00:00+01:00",
            "2026-11-27T18:00:00+01:00",
        ),
        event(
            "p2",
            "Tomorrow",
            "2026-11-28T09:00:00+01:00",
            "2026-11-28T10:00:00+01:00",
        ),
    ]);
    sync::pull_events(
        &calendar,
        &store,
        &tasks,
        "google:personal",
        &google_pull(),
        now,
        &Plus1,
    )
    .await
    .unwrap();
    let mut cancelled = event(
        "w2",
        "Cancelled",
        "2026-11-27T12:00:00+01:00",
        "2026-11-27T13:00:00+01:00",
    );
    cancelled["status"] = json!("cancelled");
    calendar.set(vec![
        event(
            "w1",
            "Standup",
            "2026-11-27T09:30:00+01:00",
            "2026-11-27T09:45:00+01:00",
        ),
        cancelled,
    ]);
    sync::pull_events(
        &calendar,
        &store,
        &tasks,
        "google:work",
        &google_pull(),
        now,
        &Plus1,
    )
    .await
    .unwrap();

    let meetings = sync::meetings_on(&store, date(27), &Plus1).unwrap();
    let shown: Vec<(&str, &str)> = meetings
        .iter()
        .map(|m| (m.title.as_str(), m.account.as_str()))
        .collect();
    assert_eq!(
        shown,
        [("Standup", "google:work"), ("Dentist", "google:personal")]
    );
    assert_eq!(meetings[0].start, utc(27, 8, 30));
    assert_eq!(meetings[0].end, utc(27, 8, 45));
    assert_eq!(meetings[0].url.as_deref(), Some("https://meet.example/w1"));
}

/// A second pull replaces the account's window: an event that is gone is gone, and the other
/// account's events stay.
#[tokio::test]
async fn a_new_pull_replaces_the_window() {
    let calendar = FakeCalendar::default();
    let store = MemoryEvents::default();
    let tasks = MemoryTaskStore::default();
    let now = utc(27, 8, 0);
    calendar.set(vec![event(
        "o1",
        "Other",
        "2026-11-27T11:00:00+01:00",
        "2026-11-27T12:00:00+01:00",
    )]);
    sync::pull_events(
        &calendar,
        &store,
        &tasks,
        "google:work",
        &google_pull(),
        now,
        &Plus1,
    )
    .await
    .unwrap();
    calendar.set(vec![
        event(
            "a",
            "A",
            "2026-11-27T10:00:00+01:00",
            "2026-11-27T11:00:00+01:00",
        ),
        event(
            "b",
            "B",
            "2026-11-27T12:00:00+01:00",
            "2026-11-27T13:00:00+01:00",
        ),
    ]);
    sync::pull_events(
        &calendar,
        &store,
        &tasks,
        "google:personal",
        &google_pull(),
        now,
        &Plus1,
    )
    .await
    .unwrap();
    calendar.set(vec![event(
        "b",
        "B moved",
        "2026-11-27T14:00:00+01:00",
        "2026-11-27T15:00:00+01:00",
    )]);
    sync::pull_events(
        &calendar,
        &store,
        &tasks,
        "google:personal",
        &google_pull(),
        now,
        &Plus1,
    )
    .await
    .unwrap();

    let titles: Vec<String> = store
        .between(None, utc(27, 0, 0), utc(28, 0, 0))
        .unwrap()
        .into_iter()
        .map(|e| e.title)
        .collect();
    assert_eq!(titles, ["Other", "B moved"]);
}

/// R-19, AC-9: retention keeps events from 1 day back to 7 days ahead.
#[test]
fn retention_keeps_one_day_back_and_seven_ahead() {
    let store = MemoryEvents::default();
    let make = |id: &str, start: UtcDateTime| sync::Event {
        account: "google:personal".to_owned(),
        id: id.to_owned(),
        calendar: None,
        title: id.to_owned(),
        start,
        end: start + time::Duration::hours(1),
        all_day: false,
        url: None,
        status: None,
    };
    let now = utc(27, 12, 0);
    store
        .replace(
            "google:personal",
            utc(1, 0, 0),
            utc(30, 0, 0),
            &[
                make("old", utc(25, 9, 0)),
                make("yesterday", utc(26, 13, 0)),
                make("soon", utc(29, 9, 0)),
            ],
        )
        .unwrap();
    store
        .replace(
            "google:personal",
            utc(30, 0, 0),
            utc(30, 0, 0) + time::Duration::days(30),
            &[make("far", utc(28, 0, 0) + time::Duration::days(9))],
        )
        .unwrap();
    let removed = store.retain(now).unwrap();
    assert_eq!(removed, 2);
    let left: Vec<String> = store
        .between(None, utc(1, 0, 0), utc(30, 0, 0) + time::Duration::days(30))
        .unwrap()
        .into_iter()
        .map(|e| e.id)
        .collect();
    assert_eq!(left, ["yesterday", "soon"]);
}

/// R-27: a pull with `mirror` upserts each item as a mirrored task keyed by (source, account,
/// reference) with a link back; a second pull makes no duplicate; an item that is cancelled or
/// gone marks its task done.
#[tokio::test]
async fn mirrored_tasks_follow_their_source() {
    let mut pull = google_pull();
    pull.mirror = true;
    let calendar = FakeCalendar::default();
    let store = MemoryEvents::default();
    let tasks = MemoryTaskStore::default();
    let now = utc(27, 8, 0);
    calendar.set(vec![
        event(
            "m1",
            "Prep the review",
            "2026-11-27T15:00:00+01:00",
            "2026-11-27T16:00:00+01:00",
        ),
        event(
            "m2",
            "Board call",
            "2026-11-28T15:00:00+01:00",
            "2026-11-28T16:00:00+01:00",
        ),
    ]);
    for _ in 0..2 {
        let report =
            sync::pull_events(&calendar, &store, &tasks, "google:work", &pull, now, &Plus1)
                .await
                .unwrap();
        assert_eq!(report.mirrored, 2);
    }
    let all = tasks.snapshot(date(28), 0).await.unwrap();
    assert_eq!(all.len(), 2, "no duplicates: {all:?}");
    let review = all.iter().find(|t| t.title == "Prep the review").unwrap();
    assert_eq!(review.day, Some(date(27)));
    assert_eq!(
        review.source,
        Source::Mirror {
            name: "calendar".to_owned(),
            account: Some("google:work".to_owned()),
            reference: "m1".to_owned(),
            url: Some("https://meet.example/m1".to_owned()),
        }
    );

    let mut cancelled = event(
        "m1",
        "Prep the review",
        "2026-11-27T15:00:00+01:00",
        "2026-11-27T16:00:00+01:00",
    );
    cancelled["status"] = json!("cancelled");
    calendar.set(vec![cancelled]);
    sync::pull_events(
        &calendar,
        &store,
        &tasks,
        "google:work",
        &pull,
        now + time::Duration::minutes(30),
        &Plus1,
    )
    .await
    .unwrap();
    let all = tasks.snapshot(date(28), 0).await.unwrap();
    assert!(all.iter().all(|t| t.status == Status::Done), "{all:?}");
}

/// A tool answer that is not the expected shape is an error naming the problem, and the cache
/// is left as it was.
#[tokio::test]
async fn a_wrong_answer_leaves_the_cache_alone() {
    struct Broken;
    impl ToolCaller for Broken {
        async fn call(&self, _: &str, _: &str, _: Value) -> Result<Value, String> {
            Ok(json!({ "unexpected": true }))
        }
    }
    let store = MemoryEvents::default();
    let tasks = MemoryTaskStore::default();
    let error = sync::pull_events(
        &Broken,
        &store,
        &tasks,
        "google:work",
        &google_pull(),
        utc(27, 8, 0),
        &Plus1,
    )
    .await
    .unwrap_err();
    assert!(error.contains("items"), "{error}");
}
