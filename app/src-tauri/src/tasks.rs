//! Daily tasks in the app (specs/92-daily-tasks, slice 7, part A): the scheduler starts at
//! launch with the task store, macOS presence and the job table; its runner rolls the day
//! over, builds the morning card and runs the connectors' sync jobs.
//!
//! What a job does is decided in [`DailyRunner`], tested with a fake clock, a fake UI and a
//! fake syncer on a temporary store. The wiring below it (presence, the MCP pulls, Home) is
//! the macOS app's.

use std::future::Future;
use std::sync::Arc;

use hyperme_core::brief::{self, DayCounts, Meeting, MorningCard, Now, Part, Sources};
use hyperme_core::scheduler::{Clock, Job, JobKind, JobStatus, JobStore, Runner};
use hyperme_core::sync;
use hyperme_core::tasks::{DayRules, TaskStore};
use hyperme_platform::task_store::SqliteTaskStore;
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

impl<C: Clock + Sync, Y: Syncer, U: Ui> DailyRunner<C, Y, U> {
    /// Today's date by the day start, and when it began, at `at`.
    fn today(&self, at: UtcDateTime) -> (Date, i64) {
        let offset = self.clock.offset_at(at);
        let local = at.to_offset(offset);
        let today = self
            .rules
            .day_of(PlainDateTime::new(local.date(), local.time()));
        let began = self.rules.starts_at(today).assume_offset(offset).to_utc();
        (today, ms(began))
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
            today_seen_ms: None,
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
            // Nudges, the evening card and prompt jobs are later slices.
            JobKind::Evening | JobKind::Nudge | JobKind::Prompt => {}
        }
        Ok(())
    }
}

// --- The app's wiring ----------------------------------------------------------------------

/// Sent to the pill when the morning card is built (slice C draws it).
pub const MORNING: &str = "tasks://morning";

/// The scheduler's control channel and the store, kept while the app runs.
pub struct Tasks {
    commands: tokio::sync::mpsc::UnboundedSender<hyperme_core::scheduler::Command>,
    store: Arc<SqliteTaskStore>,
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
    });
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(signal) = signals.recv().await {
            // The "Job failed" alert waits for the cards (R-36).
            let hyperme_core::scheduler::Signal::JobFailed { job } = signal;
            eprintln!("HyperMe: job {job} failed twice today");
        }
    });
    tauri::async_runtime::spawn(async move {
        let jobs = match job_table(&app, &store).await {
            Ok(jobs) => jobs,
            Err(e) => {
                eprintln!("HyperMe: daily tasks are off: {e}");
                return;
            }
        };
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
    }
    fn morning(&self, _card: &MorningCard, update: bool) {
        use tauri::Emitter;
        let _ = self.0.emit(MORNING, update);
    }
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
    }

    impl Ui for Arc<FakeUi> {
        fn refresh(&self) {
            *self.refreshes.lock().unwrap() += 1;
        }
        fn morning(&self, card: &MorningCard, update: bool) {
            self.cards.lock().unwrap().push((card.clone(), update));
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
}
