//! Daily tasks in the app (specs/92-daily-tasks, slice 7, part A): the scheduler starts at
//! launch with the task store, macOS presence and the job table; its runner rolls the day
//! over, builds the morning card and runs the connectors' sync jobs.
//!
//! What a job does is decided in [`DailyRunner`], tested with a fake clock, a fake UI and a
//! fake syncer on a temporary store. The wiring below it (presence, the MCP pulls, Home) is
//! the macOS app's.

use std::future::Future;
use std::sync::Arc;

use hyperme_core::brief::{self, DayCounts, Meeting, MorningCard, Now, Part, Sources, TodayView};
use hyperme_core::schedule::Zone;
use hyperme_core::scheduler::{Clock, Job, JobKind, JobStatus, JobStore, Runner};
use hyperme_core::sync;
use hyperme_core::tasks::{
    DayRules, Finish, Group, NewTask, Source, TaskId, TaskStore, TaskUpdate,
};
use hyperme_platform::task_store::SqliteTaskStore;
use serde::Serialize;
use time::{Date, Duration, PlainDateTime, UtcDateTime};

/// The sync jobs that fill each part of the morning card, by job name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Feeds {
    /// Jobs that pull calendar events.
    pub events: Vec<String>,
    /// Jobs that pull mail.
    pub mail: Vec<String>,
}

/// What the runner shows.
pub trait Ui: Send + Sync {
    /// Today changed: Home reloads it if open.
    fn refresh(&self);
    /// The morning card, built now. `update` is true when a sync filled in a card already
    /// shown today (R-18); the view replaces it only if it is still on screen.
    fn morning(&self, card: &MorningCard, update: bool);
    /// A nudge before a meeting or a due time (R-22).
    fn nudge(&self, card: &crate::pill::AlertView);
}

/// Runs one connector's pulls for every account that has it.
pub trait Syncer: Send + Sync {
    /// Runs sync job `id` (`<connector>-<server>`) with only the tools in `allowlist`.
    fn sync(
        &self,
        id: &str,
        allowlist: &[String],
        now: UtcDateTime,
    ) -> impl Future<Output = Result<(), String>> + Send;
    /// Which sync jobs fill which part, from the connectors connected now.
    fn feeds(&self) -> Feeds;
}

/// Does each job's work (R-13, R-15 to R-18, R-21).
pub struct DailyRunner<C, Y, U> {
    pub store: Arc<SqliteTaskStore>,
    pub clock: C,
    pub rules: DayRules,
    pub syncer: Y,
    pub ui: U,
}

fn ms(at: UtcDateTime) -> i64 {
    (at - UtcDateTime::UNIX_EPOCH).whole_milliseconds() as i64
}

/// Today's date by the day start, and when it began (Unix ms), at `at`.
fn today_of(zone: &dyn Zone, rules: &DayRules, at: UtcDateTime) -> (Date, i64) {
    let offset = zone.offset_at(at);
    let local = at.to_offset(offset);
    let today = rules.day_of(PlainDateTime::new(local.date(), local.time()));
    let began = rules.starts_at(today).assume_offset(offset).to_utc();
    (today, ms(began))
}

/// Today as Home shows it, from the store at `now` (R-10, R-21).
async fn load_today(
    store: &SqliteTaskStore,
    zone: &(dyn Zone + Sync),
    rules: &DayRules,
    now: UtcDateTime,
) -> Result<TodayView, String> {
    let (today, day_began_ms) = today_of(zone, rules, now);
    let snapshot = store
        .snapshot(today, day_began_ms)
        .await
        .map_err(|e| e.to_string())?;
    let at = Now {
        today,
        day_began_ms,
        rules: *rules,
        morning_shown_ms: store.morning_at(today).map_err(|e| e.to_string())?,
        today_seen_ms: store.seen_at(today).map_err(|e| e.to_string())?,
    };
    Ok(brief::today(&snapshot, &at))
}

impl<C: Clock + Sync, Y: Syncer, U: Ui> DailyRunner<C, Y, U> {
    fn today(&self, at: UtcDateTime) -> (Date, i64) {
        today_of(&self.clock, &self.rules, at)
    }

    /// The morning card from the store and the caches, never the network (R-18). A part
    /// whose sync has not yet gone through shows "Updating…"; `synced` is a sync job that just
    /// did, before the scheduler saved it.
    async fn morning_card(
        &self,
        now: UtcDateTime,
        shown_ms: i64,
        synced: Option<&str>,
    ) -> Result<MorningCard, String> {
        let (today, day_began_ms) = self.today(now);
        let snapshot = self
            .store
            .snapshot(today, day_began_ms)
            .await
            .map_err(|e| e.to_string())?;
        let jobs = self.store.jobs().await.map_err(|e| e.to_string())?;
        let feeds = self.syncer.feeds();
        let ready = |names: &[String]| {
            names.is_empty()
                || names.iter().any(|name| {
                    synced == Some(name.as_str())
                        || jobs
                            .iter()
                            .any(|j| &j.name == name && j.last_status == Some(JobStatus::Ok))
                })
        };
        let meetings = if ready(&feeds.events) {
            let zone = &self.clock;
            Part::Ready(
                sync::meetings_on(&*self.store, today, zone)?
                    .into_iter()
                    .map(|m| Meeting {
                        title: m.title,
                        start: m.start.to_offset(zone.offset_at(m.start)).time(),
                        end: m.end.to_offset(zone.offset_at(m.end)).time(),
                        account: Some(m.account),
                        link: m.url,
                    })
                    .collect(),
            )
        } else {
            Part::Pending
        };
        let mail_to_answer = if ready(&feeds.mail) {
            Part::Ready(sync::threads_to_answer(&*self.store, now)? as u32)
        } else {
            Part::Pending
        };
        let yesterday = today
            .checked_sub(Duration::days(1))
            .map(|day| self.store.day_log(day))
            .transpose()
            .map_err(|e| e.to_string())?
            .filter(|log| *log != Default::default())
            .map(|log| DayCounts {
                captured: log.captured,
                done: log.done,
                moved: log.moved,
                dropped: log.dropped,
            });
        let now_view = Now {
            today,
            day_began_ms,
            rules: self.rules,
            morning_shown_ms: Some(shown_ms),
            today_seen_ms: self.store.seen_at(today).map_err(|e| e.to_string())?,
        };
        let sources = Sources {
            meetings,
            mail_to_answer,
            proposals: 0,
            yesterday,
        };
        Ok(brief::morning(&snapshot, &now_view, &sources))
    }
}

impl<C: Clock + Sync, Y: Syncer, U: Ui> Runner for DailyRunner<C, Y, U> {
    async fn run(&mut self, job: &Job) -> Result<(), String> {
        let now = self.clock.now();
        match &job.kind {
            JobKind::RollOver => {
                let (today, _) = self.today(now);
                self.store
                    .roll_over(today)
                    .await
                    .map_err(|e| e.to_string())?;
                self.ui.refresh();
            }
            JobKind::Morning => {
                let (today, _) = self.today(now);
                self.store
                    .mark_morning(today, ms(now))
                    .map_err(|e| e.to_string())?;
                let card = self.morning_card(now, ms(now), None).await?;
                self.ui.morning(&card, false);
                self.ui.refresh();
            }
            JobKind::Sync(id) => {
                self.syncer.sync(id, &job.allowlist, now).await?;
                let (today, _) = self.today(now);
                if let Some(shown) = self.store.morning_at(today).map_err(|e| e.to_string())? {
                    let card = self.morning_card(now, shown, Some(&job.name)).await?;
                    self.ui.morning(&card, true);
                }
                self.ui.refresh();
            }
            JobKind::Nudge => {
                let (today, day_began_ms) = self.today(now);
                let meetings = sync::meetings_on(&*self.store, today, &self.clock)?;
                let snapshot = self
                    .store
                    .snapshot(today, day_began_ms)
                    .await
                    .map_err(|e| e.to_string())?;
                if let Some(card) = nudge_card(&job.name, &meetings, &snapshot, now, &self.clock) {
                    self.ui.nudge(&card);
                }
            }
            // The evening card and prompt jobs are later slices.
            JobKind::Evening | JobKind::Prompt => {}
        }
        Ok(())
    }
}

// --- Nudges (slice 10, R-22, AC-10) -------------------------------------------------------

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

/// One of today's meetings as Home lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WaitingRow {
    pub id: TaskId,
    pub title: String,
    pub on: String,
    pub new: bool,
}

/// The Today block: meetings, then the tasks in order, then "Waiting on".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

fn hhmm(at: UtcDateTime, zone: &dyn Zone) -> String {
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

/// The scheduler's control channel and the store, kept while the app runs.
pub struct Tasks {
    commands: tokio::sync::mpsc::UnboundedSender<hyperme_core::scheduler::Command>,
    store: Arc<SqliteTaskStore>,
    rules: DayRules,
}

fn config(app: &tauri::AppHandle) -> Result<hyperme_core::config::Config, String> {
    hyperme_core::config::Config::load_or_create(&crate::runtime::data_dir(app).join("config.toml"))
        .map_err(|e| e.to_string())
}

fn registry(
    app: &tauri::AppHandle,
    config: &hyperme_core::config::Config,
) -> Result<hyperme_core::registry::Registry, String> {
    let dir = crate::runtime::data_dir(app).join("presets");
    hyperme_core::connector_settings::registry(
        dir.is_dir().then_some(dir.as_path()),
        &config.connectors,
    )
    .map_err(|e| e.to_string())
}

/// The job table from the store and the config: the built-in jobs and one sync job per
/// connected connector's server (R-31; specs/97-google-connectors, R-16).
async fn job_table(app: &tauri::AppHandle, store: &SqliteTaskStore) -> Result<Vec<Job>, String> {
    let config = config(app)?;
    let mut builtin = hyperme_core::scheduler::builtin_jobs(&config.tasks);
    builtin.extend(sync::sync_jobs(
        &registry(app, &config)?,
        &config.connectors,
    ));
    let stored = store.jobs().await.map_err(|e| e.to_string())?;
    Ok(hyperme_core::scheduler::seed(stored, builtin))
}

/// Starts the scheduler (R-30). Called once from setup, on the main thread, where the presence
/// observers must live. Without a store the app runs without daily tasks.
pub fn start(app: &tauri::AppHandle) {
    use hyperme_platform::presence::{MacClock, MacPresence};
    use tauri::Manager;

    let path = crate::runtime::data_dir(app).join("hyperme.sqlite3");
    let store = match SqliteTaskStore::open(&path) {
        Ok(store) => Arc::new(store),
        Err(e) => {
            eprintln!("HyperMe: daily tasks are off: {e}");
            return;
        }
    };
    let rules = config(app).map(|c| c.tasks.day_rules()).unwrap_or_default();
    let (presence, observers) = MacPresence::start();
    // The observers deliver on the main queue and stop when dropped: keep them for the life of
    // the app.
    std::mem::forget(observers);
    let (commands, commands_rx) = tokio::sync::mpsc::unbounded_channel();
    let (signals_tx, mut signals) = tokio::sync::mpsc::unbounded_channel();
    app.manage(Tasks {
        commands,
        store: store.clone(),
        rules,
    });
    // Home showed Today: what arrived until now is no longer "new" (R-21), and the dot goes.
    {
        use tauri::Listener;
        let (app, store) = (app.clone(), store.clone());
        app.clone()
            .listen_any(crate::home::BLOCK_VIEWED, move |event| {
                if event.payload() != "\"today\"" {
                    return;
                }
                let now = UtcDateTime::now();
                let (today, _) = today_of(&MacClock, &rules, now);
                let _ = store.mark_seen(today, ms(now));
                crate::home::set_attention(&app, false);
            });
    }
    let app = app.clone();
    {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(signal) = signals.recv().await {
                // The scheduler sends this once, at a job's second failure in a day (R-36).
                let hyperme_core::scheduler::Signal::JobFailed { job } = signal;
                eprintln!("HyperMe: job {job} failed twice today");
                let alert = match config(&app).and_then(|c| Ok((registry(&app, &c)?, c))) {
                    Ok((registry, config)) => failed_alert(&job, &registry, &config.connectors),
                    Err(_) => crate::pill::AlertView {
                        title: "Job failed".to_owned(),
                        detail: format!("{job} failed twice today"),
                        action: "Fix".to_owned(),
                        link: None,
                    },
                };
                crate::runtime::show_card(&app, crate::pill::Card::Alert(alert), false);
            }
        });
    }
    tauri::async_runtime::spawn(async move {
        let jobs = match job_table(&app, &store).await {
            Ok(jobs) => jobs,
            Err(e) => {
                eprintln!("HyperMe: daily tasks are off: {e}");
                return;
            }
        };
        plan_nudges(&app);
        let runner = DailyRunner {
            store: store.clone(),
            clock: MacClock,
            rules,
            syncer: McpSyncer {
                app: app.clone(),
                store: store.clone(),
            },
            ui: AppUi(app.clone()),
        };
        hyperme_core::scheduler::run(hyperme_core::scheduler::Parts {
            clock: MacClock,
            presence,
            runner,
            store: SharedStore(store),
            jobs,
            commands: commands_rx,
            signals: signals_tx,
        })
        .await;
    });
}

/// The "Job failed" alert for a job (R-36, docs/UX.md "Alerts"): what could not be reached,
/// and Fix opening that account's page in Settings → Connectors, or the connector's when it
/// has several accounts.
pub fn failed_alert(
    job: &str,
    registry: &hyperme_core::registry::Registry,
    connectors: &hyperme_core::config::Connectors,
) -> crate::pill::AlertView {
    use hyperme_core::connector_settings::link_for;
    let found = job.strip_prefix("sync:").and_then(|id| {
        registry.connectors().iter().find_map(|c| {
            c.servers
                .iter()
                .find(|s| format!("{}-{}", c.id, s.id) == id)
                .map(|s| (c, s))
        })
    });
    let Some((connector, server)) = found else {
        return crate::pill::AlertView {
            title: "Job failed".to_owned(),
            detail: format!("{job} failed twice today"),
            action: "Fix".to_owned(),
            link: None,
        };
    };
    let what = server
        .label
        .as_deref()
        .and_then(|l| l.split(':').next())
        .unwrap_or(&server.id)
        .to_lowercase();
    let accounts: Vec<String> = connectors
        .accounts
        .iter()
        .filter(|a| a.connector == connector.id && a.enabled)
        .map(|a| a.id())
        .collect();
    let link = match accounts.as_slice() {
        [one] => link_for(one),
        _ => link_for(&connector.id),
    };
    crate::pill::AlertView {
        title: "Job failed".to_owned(),
        detail: format!("Couldn't reach your {} {what} twice today", connector.name),
        action: "Fix".to_owned(),
        link: Some(link),
    }
}

/// The connectors changed (an account added, removed, switched): rebuild the job table.
pub fn jobs_changed(app: &tauri::AppHandle) {
    use tauri::Manager;
    let Some(tasks) = app.try_state::<Tasks>() else {
        return;
    };
    let (commands, store, app) = (tasks.commands.clone(), tasks.store.clone(), app.clone());
    tauri::async_runtime::spawn(async move {
        match job_table(&app, &store).await {
            Ok(jobs) => {
                let _ = commands.send(hyperme_core::scheduler::Command::SetJobs { jobs });
            }
            Err(e) => eprintln!("HyperMe: jobs not rebuilt: {e}"),
        }
    });
}

// --- Commands from Home ----------------------------------------------------------------------

fn state(app: &tauri::AppHandle) -> Result<(Arc<SqliteTaskStore>, DayRules), String> {
    use tauri::Manager;
    let tasks = app
        .try_state::<Tasks>()
        .ok_or("Daily tasks are off: the task store could not open")?;
    Ok((tasks.store.clone(), tasks.rules))
}

async fn page_now(app: &tauri::AppHandle) -> Result<TodayPage, String> {
    use hyperme_platform::presence::MacClock;
    let (store, rules) = state(app)?;
    let now = UtcDateTime::now();
    let view = load_today(&store, &MacClock, &rules, now).await?;
    let meetings = sync::meetings_on(&*store, view.date, &MacClock)?;
    Ok(today_page(&view, &meetings, now, &MacClock))
}

/// The Today block for Home.
#[tauri::command]
pub async fn home_today(app: tauri::AppHandle) -> Result<TodayPage, String> {
    page_now(&app).await
}

/// "Add a task": a title, for today.
#[tauri::command]
pub async fn tasks_add(app: tauri::AppHandle, title: String) -> Result<(), String> {
    let (store, rules) = state(&app)?;
    let now = UtcDateTime::now();
    let (today, _) = today_of(&hyperme_platform::presence::MacClock, &rules, now);
    add_today(&store, &title, today, ms(now)).await?;
    plan_nudges(&app);
    Ok(())
}

/// A row action: `done`, `drop`, `tomorrow`, `keep`, `move` with `value` a `YYYY-MM-DD` day,
/// or `rename` with `value` the new title.
#[tauri::command]
pub async fn tasks_act(
    app: tauri::AppHandle,
    id: TaskId,
    action: String,
    value: Option<String>,
) -> Result<(), String> {
    let (store, rules) = state(&app)?;
    let now = UtcDateTime::now();
    let (today, _) = today_of(&hyperme_platform::presence::MacClock, &rules, now);
    let action = match (action.as_str(), value) {
        ("done", _) => Act::Done,
        ("drop", _) => Act::Drop,
        ("tomorrow", _) => Act::Tomorrow,
        ("keep", _) => Act::Keep,
        ("rename", Some(title)) => Act::Rename(title),
        ("move", Some(day)) => Act::Move(parse_day(&day).ok_or(format!("{day} is not a day"))?),
        (other, _) => return Err(format!("{other} is not a task action")),
    };
    act(&store, id, action, today, ms(now)).await?;
    plan_nudges(&app);
    Ok(())
}

/// Opens a meeting's link in the browser; only web links.
#[tauri::command]
pub fn tasks_open_meeting(url: String) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("Only web links open from Home".to_owned());
    }
    std::process::Command::new("open")
        .arg(url)
        .status()
        .map(drop)
        .map_err(|e| e.to_string())
}

/// The store as the scheduler's job table.
struct SharedStore(Arc<SqliteTaskStore>);

impl JobStore for SharedStore {
    async fn jobs(&self) -> Result<Vec<Job>, hyperme_core::tasks::TaskError> {
        self.0.jobs().await
    }
    async fn save_job(&self, job: &Job) -> Result<(), hyperme_core::tasks::TaskError> {
        self.0.save_job(job).await
    }
}

struct AppUi(tauri::AppHandle);

impl Ui for AppUi {
    fn refresh(&self) {
        crate::home::refresh(&self.0, "home");
        plan_nudges(&self.0);
        // Something new in Today lights the dot until Home shows it (R-21).
        let app = self.0.clone();
        tauri::async_runtime::spawn(async move {
            if let Ok(page) = page_now(&app).await
                && page.has_new
            {
                crate::home::set_attention(&app, true);
            }
        });
    }
    fn morning(&self, card: &MorningCard, update: bool) {
        crate::runtime::show_card(
            &self.0,
            crate::pill::Card::Morning(crate::pill::morning_view(card)),
            update,
        );
    }
    fn nudge(&self, card: &crate::pill::AlertView) {
        crate::runtime::show_card(&self.0, crate::pill::Card::Alert(card.clone()), false);
    }
}

/// Computes the nudges still to come today and hands them to the scheduler. Called at start,
/// after every job (a sync brings new meetings) and after every change in Home.
fn plan_nudges(app: &tauri::AppHandle) {
    use hyperme_platform::presence::MacClock;
    use tauri::Manager;
    let Some(tasks) = app.try_state::<Tasks>() else {
        return;
    };
    let (commands, store, rules) = (tasks.commands.clone(), tasks.store.clone(), tasks.rules);
    tauri::async_runtime::spawn(async move {
        let now = UtcDateTime::now();
        let (today, day_began_ms) = today_of(&MacClock, &rules, now);
        let (Ok(meetings), Ok(snapshot)) = (
            sync::meetings_on(&*store, today, &MacClock),
            store.snapshot(today, day_began_ms).await,
        ) else {
            return;
        };
        let nudges = nudges_for(&meetings, &snapshot, now, &MacClock);
        let _ = commands.send(hyperme_core::scheduler::Command::SetNudges { nudges });
    });
}

/// The pulls through each account's remote MCP servers, with the job's allowlist (G-5, R-22).
struct McpSyncer {
    app: tauri::AppHandle,
    store: Arc<SqliteTaskStore>,
}

impl Syncer for McpSyncer {
    async fn sync(&self, id: &str, allowlist: &[String], now: UtcDateTime) -> Result<(), String> {
        use hyperme_core::health::AccountStatusStore;
        use hyperme_platform::keychain::KeychainStore;
        use hyperme_platform::presence::MacClock;
        use hyperme_providers::mcp::{AccountAuth, McpCaller, RemoteServer};

        let config = config(&self.app)?;
        let registry = registry(&self.app, &config)?;
        let Some((connector, server)) = registry.connectors().iter().find_map(|c| {
            c.servers
                .iter()
                .find(|s| format!("{}-{}", c.id, s.id) == id)
                .map(|s| (c, s))
        }) else {
            return Ok(()); // The connector went away; the next rebuild drops the job.
        };
        let hyperme_core::registry::Transport::Http(url) = &server.transport else {
            return Ok(());
        };
        let keychain: Arc<dyn hyperme_core::secrets::SecretStore + Send + Sync> = Arc::new(
            KeychainStore::with_service(crate::connectors::KEYCHAIN_SERVICE),
        );
        let oauth = Arc::new(
            hyperme_providers::setup::oauth_client(connector, &*keychain)
                .map_err(|e| e.to_string())?,
        );
        let mut failures = Vec::new();
        for entry in config.connectors.accounts.iter().filter(|a| {
            a.enabled
                && a.connector == connector.id
                && hyperme_core::connector_setup::features_on(connector, a).contains(&server.id)
        }) {
            let account = entry.id();
            if self.store.sign_in_needed(&account).unwrap_or(false) {
                continue; // Settings shows "Sign in again" (specs/97, R-10).
            }
            let auth = Arc::new(AccountAuth::new(oauth.clone(), keychain.clone(), &account));
            let remote = RemoteServer::new(
                &server.id,
                url,
                auth,
                self.store.clone(),
                Arc::new(|| {
                    let now = UtcDateTime::now();
                    now.to_offset(hyperme_core::schedule::Zone::offset_at(&MacClock, now))
                        .date()
                }),
            )
            .with_units(server.units.clone());
            let caller = McpCaller::new(vec![remote], allowlist.to_vec());
            for pull in connector.pulls.iter().filter(|p| p.server == server.id) {
                let result = match pull.into.as_str() {
                    "events" => sync::pull_events(
                        &caller,
                        &*self.store,
                        &*self.store,
                        &account,
                        pull,
                        now,
                        &MacClock,
                    )
                    .await
                    .map(drop),
                    "mail" => sync::pull_mail(
                        &caller,
                        &*self.store,
                        &*self.store,
                        &account,
                        pull,
                        now,
                        &MacClock,
                    )
                    .await
                    .map(drop),
                    _ => Ok(()),
                };
                match result {
                    Ok(()) => {}
                    Err(sync::SyncError::SignInAgain) => {
                        let _ = self.store.mark_sign_in_needed(&account, now);
                        failures.push(format!("{account} needs a new sign-in"));
                    }
                    // Waits for the user's OK, refusal or admin (specs/166): not a failure
                    // for the job-failed alert (R-16b); slice 2 shows it on the account page.
                    Err(sync::SyncError::NeedsUser(_)) => {}
                    Err(e) => failures.push(format!("{account}: {e}")),
                }
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("; "))
        }
    }

    fn feeds(&self) -> Feeds {
        let mut feeds = Feeds::default();
        let Ok(config) = config(&self.app) else {
            return feeds;
        };
        let Ok(registry) = registry(&self.app, &config) else {
            return feeds;
        };
        for job in sync::sync_jobs(&registry, &config.connectors) {
            let JobKind::Sync(id) = &job.kind else {
                continue;
            };
            for c in registry.connectors() {
                for pull in &c.pulls {
                    if format!("{}-{}", c.id, pull.server) != *id {
                        continue;
                    }
                    let list = match pull.into.as_str() {
                        "events" => &mut feeds.events,
                        "mail" => &mut feeds.mail,
                        _ => continue,
                    };
                    if !list.contains(&job.name) {
                        list.push(job.name.clone());
                    }
                }
            }
        }
        feeds
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use hyperme_core::scheduler::{Job, JobKind, JobStatus, JobStore, Runner};
    use hyperme_core::sync::{Event, EventStore};
    use hyperme_core::tasks::{DayRules, NewTask, Source, TaskStore};
    use hyperme_platform::task_store::DayCounter;
    use time::{Date, Month, SignedDuration, Time, UtcDateTime, UtcOffset};

    use super::*;

    /// Madrid in November, UTC+1, stopped at one moment.
    struct FixedClock(UtcDateTime);

    impl hyperme_core::schedule::Zone for FixedClock {
        fn offset_at(&self, _at: UtcDateTime) -> UtcOffset {
            UtcOffset::from_hms(1, 0, 0).unwrap()
        }
    }

    impl Clock for FixedClock {
        fn now(&self) -> UtcDateTime {
            self.0
        }
        async fn sleep_until(&self, _at: UtcDateTime) {
            std::future::pending::<()>().await
        }
    }

    #[derive(Default)]
    struct FakeUi {
        refreshes: Mutex<usize>,
        cards: Mutex<Vec<(MorningCard, bool)>>,
        nudges: Mutex<Vec<crate::pill::AlertView>>,
    }

    impl Ui for Arc<FakeUi> {
        fn refresh(&self) {
            *self.refreshes.lock().unwrap() += 1;
        }
        fn morning(&self, card: &MorningCard, update: bool) {
            self.cards.lock().unwrap().push((card.clone(), update));
        }
        fn nudge(&self, card: &crate::pill::AlertView) {
            self.nudges.lock().unwrap().push(card.clone());
        }
    }

    type Calls = Arc<Mutex<Vec<(String, Vec<String>)>>>;

    /// A syncer that writes one meeting into the cache, or fails.
    struct FakeSyncer {
        store: Arc<SqliteTaskStore>,
        feeds: Feeds,
        fail: bool,
        calls: Calls,
    }

    impl Syncer for FakeSyncer {
        async fn sync(
            &self,
            id: &str,
            allowlist: &[String],
            now: UtcDateTime,
        ) -> Result<(), String> {
            self.calls
                .lock()
                .unwrap()
                .push((id.to_owned(), allowlist.to_vec()));
            if self.fail {
                return Err("no network".into());
            }
            let start = local(FRI, 10, 0);
            let event = Event {
                account: "google:personal".into(),
                id: "e1".into(),
                calendar: None,
                title: "Standup".into(),
                start,
                end: start + SignedDuration::minutes(15),
                all_day: false,
                url: Some("https://meet.example/abc".into()),
                status: None,
            };
            self.store.replace(
                "google:personal",
                now - SignedDuration::days(1),
                now + SignedDuration::days(7),
                &[event],
            )
        }
        fn feeds(&self) -> Feeds {
            self.feeds.clone()
        }
    }

    const THU: u8 = 26;
    const FRI: u8 = 27;

    fn day(d: u8) -> Date {
        Date::from_calendar_date(2026, Month::November, d).unwrap()
    }

    fn local(d: u8, h: u8, m: u8) -> UtcDateTime {
        UtcDateTime::new(day(d), Time::from_hms(h, m, 0).unwrap()) - SignedDuration::hours(1)
    }

    fn store(name: &str) -> Arc<SqliteTaskStore> {
        let dir = std::env::temp_dir().join(format!("hyperme-app-tasks-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}.sqlite3"));
        let _ = std::fs::remove_file(&path);
        Arc::new(SqliteTaskStore::open(&path).unwrap())
    }

    fn calendar_feeds() -> Feeds {
        Feeds {
            events: vec!["sync:google-calendar".into()],
            mail: vec!["sync:google-gmail".into()],
        }
    }

    struct Setup {
        runner: DailyRunner<FixedClock, FakeSyncer, Arc<FakeUi>>,
        ui: Arc<FakeUi>,
        calls: Calls,
    }

    fn setup(store: Arc<SqliteTaskStore>, at: UtcDateTime, feeds: Feeds, fail: bool) -> Setup {
        let ui = Arc::new(FakeUi::default());
        let calls = Arc::default();
        Setup {
            runner: DailyRunner {
                store: store.clone(),
                clock: FixedClock(at),
                rules: DayRules::default(),
                syncer: FakeSyncer {
                    store,
                    feeds,
                    fail,
                    calls: Arc::clone(&calls),
                },
                ui: ui.clone(),
            },
            ui,
            calls,
        }
    }

    fn task(title: &str, planned: Date, arrived: Date) -> NewTask {
        NewTask {
            title: title.into(),
            notes: None,
            day: Some(planned),
            due: None,
            source: Source::HyperMe,
            blocked_by: Vec::new(),
            created_at: 0,
            arrived_day: arrived,
        }
    }

    fn job(name: &str, kind: JobKind) -> Job {
        Job::new(
            name,
            kind,
            hyperme_core::schedule::Schedule::EveryWhilePresent { minutes: 30 },
        )
    }

    fn sync_job() -> Job {
        let mut job = job(
            "sync:google-calendar",
            JobKind::Sync("google-calendar".into()),
        );
        job.allowlist = vec!["list_events".into()];
        job
    }

    fn block_on<F: Future>(f: F) -> F::Output {
        tauri::async_runtime::block_on(f)
    }

    /// AC-7 (app half), R-13: the roll-over job rolls yesterday's open task into today with a
    /// count of one, and Home reloads Today.
    #[test]
    fn roll_over_moves_yesterdays_tasks_and_refreshes_home() {
        let store = store("rollover");
        let id = block_on(store.add(task("Call the dentist", day(THU), day(THU)))).unwrap();
        let mut s = setup(store.clone(), local(FRI, 4, 0), Feeds::default(), false);
        block_on(s.runner.run(&job("rollover", JobKind::RollOver))).unwrap();

        let snapshot = block_on(store.snapshot(day(FRI), 0)).unwrap();
        let rolled = snapshot.iter().find(|t| t.id == id).unwrap();
        assert_eq!(rolled.rolled, 1);
        assert_eq!(rolled.day, Some(day(THU)), "original day kept");
        assert_eq!(*s.ui.refreshes.lock().unwrap(), 1);
    }

    /// AC-9 (app half), R-17, R-18: the morning card comes from the store before any sync:
    /// tasks for today, yesterday's count, and "Updating…" for calendar and mail. The day log
    /// records when it showed.
    #[test]
    fn the_morning_card_shows_before_the_syncs_with_updating_parts() {
        let store = store("morning_pending");
        block_on(store.add(task("Send the report", day(FRI), day(THU)))).unwrap();
        store.count(day(THU), DayCounter::Done, 3).unwrap();
        let at = local(FRI, 7, 40);
        let mut s = setup(store.clone(), at, calendar_feeds(), false);
        block_on(s.runner.run(&job("morning", JobKind::Morning))).unwrap();

        let cards = s.ui.cards.lock().unwrap();
        let (card, update) = &cards[0];
        assert!(!update);
        assert_eq!(card.date, day(FRI));
        assert_eq!(card.meetings, Part::Pending);
        assert_eq!(card.mail_to_answer, Part::Pending);
        assert_eq!(card.top[0].title, "Send the report");
        assert_eq!(card.yesterday.map(|y| y.done), Some(3));
        assert_eq!(store.morning_at(day(FRI)).unwrap(), Some(ms(at)));
        assert_eq!(*s.ui.refreshes.lock().unwrap(), 1);
    }

    /// R-17: with no calendar or mail connected, those parts are simply empty, never
    /// "Updating…"; a day with nothing in the log has no "Yesterday".
    #[test]
    fn without_connectors_the_card_has_no_updating_parts() {
        let store = store("morning_none");
        let mut s = setup(store, local(FRI, 7, 40), Feeds::default(), false);
        block_on(s.runner.run(&job("morning", JobKind::Morning))).unwrap();
        let cards = s.ui.cards.lock().unwrap();
        assert_eq!(cards[0].0.meetings, Part::Ready(Vec::new()));
        assert_eq!(cards[0].0.mail_to_answer, Part::Ready(0));
        assert_eq!(cards[0].0.yesterday, None);
    }

    /// AC-9, R-18: when the calendar sync returns after the card showed, the card is built
    /// again with today's meetings in local time, as an update; the sync ran with its own
    /// allowlist only.
    #[test]
    fn a_sync_after_the_card_fills_in_the_meetings() {
        let store = store("morning_fill");
        let at = local(FRI, 7, 40);
        let mut s = setup(store.clone(), at, calendar_feeds(), false);
        block_on(s.runner.run(&job("morning", JobKind::Morning))).unwrap();
        block_on(s.runner.run(&sync_job())).unwrap();

        assert_eq!(
            *s.calls.lock().unwrap(),
            [("google-calendar".to_owned(), vec!["list_events".to_owned()])]
        );
        let cards = s.ui.cards.lock().unwrap();
        let (card, update) = &cards[1];
        assert!(update);
        let Part::Ready(meetings) = &card.meetings else {
            panic!("meetings still pending");
        };
        assert_eq!(meetings[0].title, "Standup");
        assert_eq!(meetings[0].start, Time::from_hms(10, 0, 0).unwrap());
        assert_eq!(card.mail_to_answer, Part::Pending, "mail has not synced");
        assert_eq!(*s.ui.refreshes.lock().unwrap(), 2);
    }

    /// R-18: a sync the scheduler saved as OK, on an earlier day or before a restart, makes
    /// its part ready on the next card.
    #[test]
    fn a_sync_saved_as_ok_makes_its_part_ready() {
        let store = store("saved_ok");
        let mut saved = sync_job();
        saved.last_status = Some(JobStatus::Ok);
        block_on(store.save_job(&saved)).unwrap();
        let mut s = setup(store, local(FRI, 7, 40), calendar_feeds(), false);
        block_on(s.runner.run(&job("morning", JobKind::Morning))).unwrap();
        let cards = s.ui.cards.lock().unwrap();
        assert_eq!(cards[0].0.meetings, Part::Ready(Vec::new()));
        assert_eq!(cards[0].0.mail_to_answer, Part::Pending);
    }

    /// R-15, R-21: a sync before any morning card that day updates Home only; no card.
    #[test]
    fn a_sync_before_the_card_only_refreshes_home() {
        let store = store("sync_first");
        let mut s = setup(store, local(FRI, 5, 0), calendar_feeds(), false);
        block_on(s.runner.run(&sync_job())).unwrap();
        assert!(s.ui.cards.lock().unwrap().is_empty());
        assert_eq!(*s.ui.refreshes.lock().unwrap(), 1);
    }

    /// R-36: a failed sync is returned to the scheduler, which records it and retries; nothing
    /// is redrawn.
    #[test]
    fn a_failed_sync_is_returned_and_redraws_nothing() {
        let store = store("sync_fail");
        let mut s = setup(store, local(FRI, 9, 0), calendar_feeds(), true);
        let result = block_on(s.runner.run(&sync_job()));
        assert_eq!(result, Err("no network".to_owned()));
        assert_eq!(*s.ui.refreshes.lock().unwrap(), 0);
    }

    // --- Home's Today ---

    fn meeting(title: &str, start: UtcDateTime, minutes: i64) -> sync::Meeting {
        sync::Meeting {
            title: title.into(),
            start,
            end: start + SignedDuration::minutes(minutes),
            account: "google:personal".into(),
            url: Some("https://meet.example/x".into()),
        }
    }

    /// Owner, 27 November 2026: Home leads with today's meetings in local time; the next one
    /// says "in n min" (rounded up), one under way says now, and past ones are marked.
    #[test]
    fn the_today_block_lists_meetings_with_the_next_one_in_minutes() {
        let now = local(FRI, 9, 48);
        let view = block_on(load_today(
            &store("page_meetings"),
            &FixedClock(now),
            &DayRules::default(),
            now,
        ))
        .unwrap();
        let meetings = [
            meeting("Standup", local(FRI, 9, 0), 15),
            meeting("Review", local(FRI, 9, 40), 30),
            meeting("Planning", local(FRI, 10, 0), 60),
            meeting("1:1", local(FRI, 15, 0), 30),
        ];
        let page = today_page(&view, &meetings, now, &FixedClock(now));
        let rows: Vec<(&str, &str, Option<i64>, bool, bool)> = page
            .meetings
            .iter()
            .map(|m| {
                (
                    m.title.as_str(),
                    m.start.as_str(),
                    m.starts_in_min,
                    m.now,
                    m.past,
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                ("Standup", "09:00", None, false, true),
                ("Review", "09:40", None, true, false),
                ("Planning", "10:00", Some(12), false, false),
                ("1:1", "15:00", None, false, false),
            ]
        );
        assert_eq!(page.meetings[2].end, "11:00");
        assert_eq!(page.today, "2026-11-27");
        assert_eq!(page.tomorrow, "2026-11-28");
    }

    /// AC-5, AC-10, R-10, R-11, R-21: lines carry "new" after the morning card until Today is
    /// seen, "unblocked" or "blocker dropped", and due marks; a blocked task sits under
    /// "Waiting on" with its blocker's title.
    #[test]
    fn the_today_block_marks_new_unblocked_and_waiting() {
        let store = store("page_marks");
        let ins = block_on(store.add(task("Call the insurance", day(FRI), day(FRI)))).unwrap();
        let mut send = task("Send Ana the numbers", day(FRI), day(FRI));
        send.blocked_by = vec![ins];
        let send = block_on(store.add(send)).unwrap();
        let morning = local(FRI, 7, 40);
        store.mark_morning(day(FRI), ms(morning)).unwrap();
        let mut later = task("Book the room", day(FRI), day(FRI));
        later.created_at = ms(local(FRI, 15, 0));
        block_on(store.add(later)).unwrap();

        let now = local(FRI, 15, 30);
        let clock = FixedClock(now);
        let view = block_on(load_today(&store, &clock, &DayRules::default(), now)).unwrap();
        let page = today_page(&view, &[], now, &clock);
        assert_eq!(page.waiting.len(), 1);
        assert_eq!(page.waiting[0].on, "Call the insurance");
        let book = page
            .lines
            .iter()
            .find(|l| l.title == "Book the room")
            .unwrap();
        assert_eq!(book.marks, ["new"]);
        assert!(page.has_new);

        block_on(act(&store, ins, Act::Drop, day(FRI), ms(now))).unwrap();
        store.mark_seen(day(FRI), ms(now)).unwrap();
        let view = block_on(load_today(&store, &clock, &DayRules::default(), now)).unwrap();
        let page = today_page(&view, &[], now, &clock);
        let unblocked = page.lines.iter().find(|l| l.id == send).unwrap();
        assert_eq!(unblocked.marks, ["blocker dropped"]);
        assert!(!page.has_new, "seen clears new");
        assert!(page.waiting.is_empty());
    }

    /// R-4, R-29: Home's field adds a task for today, counted as captured; done, drop and
    /// tomorrow count in the day log; rename changes the title; an empty title or a past day
    /// is refused.
    #[test]
    fn row_actions_change_the_task_and_count_in_the_day_log() {
        let store = store("acts");
        let now_ms = ms(local(FRI, 11, 0));
        assert!(block_on(add_today(&store, "  ", day(FRI), now_ms)).is_err());
        let a = block_on(add_today(&store, " Call the dentist ", day(FRI), now_ms)).unwrap();
        let b = block_on(add_today(&store, "Pay the bill", day(FRI), now_ms)).unwrap();
        let c = block_on(add_today(&store, "Write the notes", day(FRI), now_ms)).unwrap();
        let d = block_on(add_today(&store, "Old thing", day(FRI), now_ms)).unwrap();

        block_on(act(&store, a, Act::Done, day(FRI), now_ms)).unwrap();
        block_on(act(&store, b, Act::Drop, day(FRI), now_ms)).unwrap();
        block_on(act(&store, c, Act::Tomorrow, day(FRI), now_ms)).unwrap();
        block_on(act(
            &store,
            d,
            Act::Rename("Old thing, renamed".into()),
            day(FRI),
            now_ms,
        ))
        .unwrap();
        assert!(block_on(act(&store, d, Act::Rename(" ".into()), day(FRI), now_ms)).is_err());
        assert!(block_on(act(&store, d, Act::Move(day(THU)), day(FRI), now_ms)).is_err());

        let log = store.day_log(day(FRI)).unwrap();
        assert_eq!(
            (log.captured, log.done, log.dropped, log.moved),
            (4, 1, 1, 1)
        );
        let tasks = block_on(store.snapshot(day(FRI), 0)).unwrap();
        let title = |id| tasks.iter().find(|t| t.id == id).map(|t| t.title.clone());
        assert_eq!(title(a).as_deref(), Some("Call the dentist"));
        assert_eq!(title(d).as_deref(), Some("Old thing, renamed"));
        assert!(title(c).is_none(), "moved to tomorrow, out of today");
    }

    /// AC-7 (UI), R-14: keep on a stale task keeps it in today and stops asking.
    #[test]
    fn keep_stops_asking_about_a_stale_task() {
        let store = store("keep_stale");
        let id = block_on(store.add(task("Renew the passport", day(25), day(25)))).unwrap();
        for _ in 0..3 {
            block_on(store.roll_over(day(FRI))).unwrap();
        }
        let now = local(FRI, 9, 0);
        let clock = FixedClock(now);
        let stale = |store: &SqliteTaskStore| {
            let view = block_on(load_today(store, &clock, &DayRules::default(), now)).unwrap();
            today_page(&view, &[], now, &clock)
                .lines
                .iter()
                .find(|l| l.id == id)
                .unwrap()
                .stale
        };
        assert!(stale(&store));
        block_on(act(&store, id, Act::Keep, day(FRI), ms(now))).unwrap();
        assert!(!stale(&store));
    }

    #[test]
    fn days_from_the_picker_parse() {
        assert_eq!(
            parse_day("2026-11-30"),
            Some(Date::from_calendar_date(2026, Month::November, 30).unwrap())
        );
        assert_eq!(parse_day("2026-02-30"), None);
        assert_eq!(parse_day("monday"), None);
    }

    /// AC-9, R-18 (the part before the screen): the Morning job, from a store of 10 000 tasks
    /// on disk to the card's words, in under 150 ms. Release builds only; run with
    /// `cargo test --release -p hyperme-app morning_from_10_000 -- --nocapture` on the Mac and
    /// record the numbers in docs/results/.
    #[test]
    #[cfg_attr(debug_assertions, ignore)]
    fn morning_from_10_000_tasks_on_disk_is_well_under_150_ms() {
        let store = store("morning_10k");
        for i in 0..10_000u32 {
            // A spread like a long-used list: most planned on other days, some for today.
            let planned = match i % 10 {
                0 => day(FRI),
                1 => day(THU),
                _ => Date::from_calendar_date(2026, Month::December, 1 + (i % 28) as u8).unwrap(),
            };
            block_on(store.add(task(&format!("Task {i}"), planned, day(THU)))).unwrap();
        }
        let mut runs = Vec::new();
        for _ in 0..21 {
            let mut s = setup(store.clone(), local(FRI, 7, 40), calendar_feeds(), false);
            let started = std::time::Instant::now();
            block_on(s.runner.run(&job("morning", JobKind::Morning))).unwrap();
            let card = s.ui.cards.lock().unwrap()[0].0.clone();
            let view = crate::pill::morning_view(&card);
            runs.push(started.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(view.tasks.len(), 5);
        }
        runs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let (median, max) = (runs[runs.len() / 2], runs[runs.len() - 1]);
        println!(
            "Morning job, 10000 tasks on disk, 21 runs: median {median:.2} ms, max {max:.2} ms"
        );
        assert!(max < 150.0, "the morning job took {max:.2} ms");
    }

    // --- Job failed ---

    fn google_with(
        accounts: &[&str],
    ) -> (
        hyperme_core::registry::Registry,
        hyperme_core::config::Connectors,
    ) {
        let mut text = hyperme_core::config::DEFAULT_CONFIG.to_owned();
        for label in accounts {
            text.push_str(&format!(
                "\n[[connectors.accounts]]\nconnector = \"google\"\nlabel = \"{label}\"\n"
            ));
        }
        let connectors =
            hyperme_core::config::Config::parse(&text, std::path::Path::new("config.toml"))
                .unwrap()
                .connectors;
        let registry = hyperme_core::connector_settings::registry(None, &connectors).unwrap();
        (registry, connectors)
    }

    /// AC-16, R-36: a sync job that failed twice says what it could not reach, and Fix opens
    /// the account's page; with two accounts, the connector's.
    #[test]
    fn a_failed_sync_names_what_it_could_not_reach_and_links_the_fix() {
        let (registry, connectors) = google_with(&["personal"]);
        let alert = failed_alert("sync:google-calendar", &registry, &connectors);
        assert_eq!(alert.title, "Job failed");
        assert_eq!(
            alert.detail,
            "Couldn't reach your Google calendar twice today"
        );
        assert_eq!(alert.action, "Fix");
        assert_eq!(
            alert.link.as_deref(),
            Some("hyperme://settings/connectors/google:personal")
        );

        let (registry, connectors) = google_with(&["personal", "work"]);
        let alert = failed_alert("sync:google-gmail", &registry, &connectors);
        assert_eq!(alert.detail, "Couldn't reach your Google mail twice today");
        assert_eq!(
            alert.link.as_deref(),
            Some("hyperme://settings/connectors/google")
        );
    }

    /// R-36: a job that is not a sync (roll-over, the morning card) still alerts, with Fix
    /// opening Settings.
    #[test]
    fn another_failed_job_alerts_without_a_link() {
        let (registry, connectors) = google_with(&[]);
        let alert = failed_alert("rollover", &registry, &connectors);
        assert_eq!(alert.detail, "rollover failed twice today");
        assert_eq!(alert.link, None);
    }

    // --- Nudges ---

    fn due_task(title: &str, h: u8, m: u8) -> NewTask {
        NewTask {
            due: Some(hyperme_core::tasks::Due {
                date: day(FRI),
                time: Some(Time::from_hms(h, m, 0).unwrap()),
            }),
            ..task(title, day(FRI), day(FRI))
        }
    }

    /// AC-10, R-22: a meeting at 10:30 is nudged at 10:28 and a task due at 16:00 at 15:50;
    /// moments already past, tasks with no due time and finished tasks get none.
    #[test]
    fn nudges_come_two_minutes_before_meetings_and_ten_before_due_times() {
        let store = store("nudges_plan");
        let due = block_on(store.add(due_task("Send the numbers", 16, 0))).unwrap();
        block_on(store.add(task("No time", day(FRI), day(FRI)))).unwrap();
        let done = block_on(store.add(due_task("Already done", 17, 0))).unwrap();
        block_on(store.finish(done, Finish::Done, 0)).unwrap();
        let now = local(FRI, 9, 0);
        let snapshot = block_on(store.snapshot(day(FRI), 0)).unwrap();
        let meetings = [
            meeting("Standup", local(FRI, 8, 30), 15),
            meeting("Weekly planning", local(FRI, 10, 30), 30),
        ];
        let nudges = nudges_for(&meetings, &snapshot, now, &FixedClock(now));
        let got: Vec<(String, UtcDateTime, UtcDateTime)> =
            nudges.into_iter().map(|n| (n.id, n.at, n.until)).collect();
        assert_eq!(
            got,
            [
                (
                    format!("meeting:{}", ms(local(FRI, 10, 30))),
                    local(FRI, 10, 28),
                    local(FRI, 10, 30)
                ),
                (format!("task:{due}"), local(FRI, 15, 50), local(FRI, 16, 0)),
            ]
        );
    }

    /// AC-10, R-22: the nudge card says what and when, with Join for a meeting's web link and
    /// Open Today for a task; a task finished meanwhile shows nothing.
    #[test]
    fn a_nudge_card_says_what_and_when() {
        let store = store("nudge_card");
        let id = block_on(store.add(due_task("Send the numbers", 16, 0))).unwrap();
        let now = local(FRI, 10, 28);
        let snapshot = block_on(store.snapshot(day(FRI), 0)).unwrap();
        let meetings = [meeting("Weekly planning", local(FRI, 10, 30), 30)];
        let zone = FixedClock(now);
        let card = nudge_card(
            &format!("meeting:{}", ms(local(FRI, 10, 30))),
            &meetings,
            &snapshot,
            now,
            &zone,
        )
        .unwrap();
        assert_eq!(card.title, "Weekly planning in 2 min");
        assert_eq!(card.detail, "10:30–11:00");
        assert_eq!(
            (card.action.as_str(), card.link.as_deref()),
            ("Join", Some("https://meet.example/x"))
        );

        let at = local(FRI, 15, 50);
        let card = nudge_card(&format!("task:{id}"), &[], &snapshot, at, &FixedClock(at)).unwrap();
        assert_eq!(card.title, "Send the numbers");
        assert_eq!(card.detail, "Due at 16:00 · in 10 min");
        assert_eq!(card.link.as_deref(), Some(TODAY_LINK));

        block_on(store.finish(id, Finish::Done, 0)).unwrap();
        let snapshot = block_on(store.snapshot(day(FRI), 0)).unwrap();
        assert_eq!(
            nudge_card(&format!("task:{id}"), &[], &snapshot, at, &FixedClock(at)),
            None
        );
    }

    /// AC-10, R-22: the Nudge job shows the card through the UI, with no sound or refresh.
    #[test]
    fn the_nudge_job_shows_its_card() {
        let store = store("nudge_job");
        let id = block_on(store.add(due_task("Send the numbers", 16, 0))).unwrap();
        let mut s = setup(store, local(FRI, 15, 50), Feeds::default(), false);
        block_on(s.runner.run(&job(&format!("task:{id}"), JobKind::Nudge))).unwrap();
        assert_eq!(s.ui.nudges.lock().unwrap()[0].title, "Send the numbers");
        assert_eq!(*s.ui.refreshes.lock().unwrap(), 0);
        assert!(s.ui.cards.lock().unwrap().is_empty());
    }
}
