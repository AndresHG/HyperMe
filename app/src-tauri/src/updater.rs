//! Runs "Check for updates…" with Tauri's updater plugin (docs/RELEASING.md). The plugin checks
//! the signed `latest.json` of the newest GitHub release, verifies the download against the
//! public key in `tauri.conf.json`, replaces HyperMe.app and restarts it. `update.rs` decides
//! what the menu line says and what a click does.

use std::sync::{Mutex, PoisonError};

use tauri::menu::MenuItem;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::update::{self, Action, State};

/// The menu line, where the update stands, and the update found by the last check.
pub struct Updates {
    item: MenuItem<tauri::Wry>,
    inner: Mutex<(State, Option<Update>)>,
}

/// Keeps the menu line so clicks can update it.
pub fn manage(app: &AppHandle, item: MenuItem<tauri::Wry>) {
    app.manage(Updates {
        item,
        inner: Mutex::new((State::Idle, None)),
    });
}

/// The menu line's id.
pub const MENU_ID: &str = "update";

/// A click on the menu line: checks, or installs the update the last check found.
pub fn clicked(app: &AppHandle) {
    let state = current(app);
    match update::on_click(&state) {
        Action::Check => check(app.clone()),
        Action::Install => install(app.clone()),
        Action::Nothing => {}
    }
}

fn current(app: &AppHandle) -> State {
    let updates = app.state::<Updates>();
    let inner = updates.inner.lock().unwrap_or_else(PoisonError::into_inner);
    inner.0.clone()
}

fn set(app: &AppHandle, state: State, found: Option<Option<Update>>) {
    let updates = app.state::<Updates>();
    let line = update::line(&state);
    {
        let mut inner = updates.inner.lock().unwrap_or_else(PoisonError::into_inner);
        inner.0 = state;
        if let Some(found) = found {
            inner.1 = found;
        }
    }
    let _ = updates.item.set_text(line.text);
    let _ = updates.item.set_enabled(line.enabled);
}

fn check(app: AppHandle) {
    set(&app, State::Checking, Some(None));
    tauri::async_runtime::spawn(async move {
        let result = match app.updater() {
            Ok(updater) => updater.check().await,
            Err(e) => Err(e),
        };
        match result {
            Ok(Some(found)) => {
                let version = found.version.clone();
                set(&app, State::Available(version), Some(Some(found)));
            }
            Ok(None) => set(&app, State::UpToDate, Some(None)),
            Err(e) => set(
                &app,
                State::Failed(update::short_reason(&e.to_string())),
                Some(None),
            ),
        }
    });
}

fn install(app: AppHandle) {
    let found = {
        let updates = app.state::<Updates>();
        let mut inner = updates.inner.lock().unwrap_or_else(PoisonError::into_inner);
        inner.1.take()
    };
    let Some(found) = found else {
        set(&app, State::Idle, None);
        return;
    };
    set(&app, State::Downloading(None), None);
    tauri::async_runtime::spawn(async move {
        let mut done: u64 = 0;
        let mut shown = None;
        let progress = app.clone();
        let finished = app.clone();
        let result = found
            .download_and_install(
                move |chunk, total| {
                    done = done.saturating_add(chunk as u64);
                    let percent = update::percent(done, total);
                    // Only when the shown number changes, so the menu is not redrawn per chunk.
                    if percent != shown {
                        shown = percent;
                        set(&progress, State::Downloading(percent), None);
                    }
                },
                move || set(&finished, State::Installing, None),
            )
            .await;
        match result {
            Ok(()) => app.restart(),
            Err(e) => set(
                &app,
                State::Failed(update::short_reason(&e.to_string())),
                None,
            ),
        }
    });
}
