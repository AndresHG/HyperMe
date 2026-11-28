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

mod commands;
mod nudges;
mod runner;
mod server;
#[cfg(test)]
mod tests;
mod today;

pub use commands::*;
pub use nudges::*;
pub use runner::*;
pub use server::*;
pub use today::*;

/// The scheduler's control channel and the store, kept while the app runs.
pub struct Tasks {
    commands: tokio::sync::mpsc::UnboundedSender<hyperme_core::scheduler::Command>,
    store: Arc<SqliteTaskStore>,
    rules: DayRules,
}

fn config(app: &tauri::AppHandle) -> Result<std::sync::Arc<hyperme_core::config::Config>, String> {
    crate::runtime::config(app).map_err(|e| e.to_string())
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
            tracing::warn!("daily tasks are off: {e}");
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
        use tauri_specta::Event as _;
        let (app, store) = (app.clone(), store.clone());
        crate::events::HomeBlockViewed::listen_any(&app.clone(), move |event| {
            if event.payload.0 != "today" {
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
                tracing::warn!("job {job} failed twice today");
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
                tracing::warn!("daily tasks are off: {e}");
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
            Err(e) => tracing::warn!("jobs not rebuilt: {e}"),
        }
    });
}

// --- Commands from Home ----------------------------------------------------------------------

/// The app's one connection to `hyperme.sqlite3`, opened at start-up; `None` when it could not
/// open. The connectors keep their request counts and cached data in the same file.
pub fn shared_store<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Option<Arc<SqliteTaskStore>> {
    use tauri::Manager;
    app.try_state::<Tasks>().map(|tasks| tasks.store.clone())
}

fn state(app: &tauri::AppHandle) -> Result<(Arc<SqliteTaskStore>, DayRules), String> {
    use tauri::Manager;
    let tasks = app
        .try_state::<Tasks>()
        .ok_or("Daily tasks are off: the task store could not open")?;
    Ok((tasks.store.clone(), tasks.rules))
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
