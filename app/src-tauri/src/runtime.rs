//! The dictation at run time on macOS: the config, the keys from the keychain, the real
//! providers and devices, the state machine on Tauri's async runtime, and its states shown in
//! the pill and the menu.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use hyperme_core::config::Config;
use hyperme_core::dictation::{Engine, Event, Parts, Settings, Status};
use hyperme_core::secrets::SecretStore;
use hyperme_core::traits::{DictationId, Error, ErrorKind, History, Inserter};
use hyperme_platform::history::{HistorySettings, SqliteHistory};
use hyperme_platform::hotkey::macos::HotkeyListener;
use hyperme_platform::keychain::KeychainStore;
use hyperme_platform::microphone::CpalMicrophone;
use hyperme_platform::paste::macos::{CmdV, MacFocus, MacPasteboard};
use hyperme_platform::paste::{ClipboardInserter, RESTORE_AFTER};
use hyperme_platform::sounds::SystemSounds;
use hyperme_platform::timing::{FileTimingLog, Names};
use hyperme_providers::cleanup::ChatCleanup;
use hyperme_providers::speechmatics::{SAMPLE_RATE, Speechmatics};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Position, Size};
use tauri_nspanel::ManagerExt;
use tokio::sync::mpsc;

use crate::pill::{self, Change, View};
use crate::{PILL, TrayMenu, geometry};

/// The meter refreshes at most this often (docs/UX.md: 30 fps).
const LEVEL_EVERY: Duration = Duration::from_millis(33);

/// What the menu and the pill's click handler need while the app runs.
struct Shared {
    events: Option<mpsc::Sender<Event>>,
    /// The view on screen, and a counter so a stale hide timer does nothing.
    view: Option<View>,
    generation: u64,
    history: Arc<Mutex<HistoryInner>>,
    menu: TrayMenu,
    /// The bead at rest instead of nothing (U-13, brought forward by the owner).
    show_while_idle: bool,
    /// Hotkey listening must live as long as the app.
    _hotkey: Option<HotkeyListener>,
    /// A dictation or retry is in progress: from its first status until Idle.
    busy: bool,
    /// The dictation Home asked to retry, while that retry runs (spec 98, R-16).
    home_retry: Option<DictationId>,
    /// History is on in the config (spec 98, R-19).
    history_enabled: bool,
}

type State = Mutex<Shared>;

fn lock(app: &AppHandle) -> std::sync::MutexGuard<'_, Shared> {
    app.state::<State>()
        .inner()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// HyperMe's data folder: `~/Library/Application Support/HyperMe` (design.md).
pub fn data_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("HyperMe")
}

/// The timing log (R-30): `~/Library/Logs/HyperMe/timing.tsv`, the app's log folder. Written
/// only while timing is on in Settings.
pub fn timing_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("Library/Logs/HyperMe/timing.tsv")
}

/// Where the pill's top-left corner goes for a given width.
pub fn pill_origin(app: &AppHandle, width: f64) -> (f64, f64) {
    match app.primary_monitor() {
        Ok(Some(monitor)) => {
            let scale = monitor.scale_factor();
            let area = monitor.work_area();
            let work = geometry::Rect {
                x: f64::from(area.position.x) / scale,
                y: f64::from(area.position.y) / scale,
                width: f64::from(area.size.width) / scale,
                height: f64::from(area.size.height) / scale,
            };
            geometry::bottom_centre(work, (width, geometry::PILL_SIZE.1))
        }
        _ => (0.0, 0.0),
    }
}

/// Starts the dictation, or says in the menu why it cannot.
pub fn start(app: &AppHandle, menu: TrayMenu) {
    let history = Arc::new(Mutex::new(HistoryInner::default()));
    app.manage::<State>(Mutex::new(Shared {
        events: None,
        view: None,
        generation: 0,
        history: history.clone(),
        menu,
        show_while_idle: Config::load_or_create(&data_dir(app).join("config.toml"))
            .map_or(true, |config| config.dictation.show_while_idle),
        _hotkey: None,
        busy: false,
        home_retry: None,
        history_enabled: Config::load_or_create(&data_dir(app).join("config.toml"))
            .map_or(true, |config| config.history.enabled),
    }));
    settle(app);
    // Home shows the history even before the keys are in, or when one cannot be read.
    let dir = data_dir(app);
    if let Ok(config) = Config::load_or_create(&dir.join("config.toml")) {
        open_history(&history, &dir, &config);
    }
    match wire(app, history) {
        Ok(()) => set_status(app, "Ready"),
        Err(why) => set_status(app, &why),
    }
}

/// Starts the dictation if it is not running yet, as the walkthrough does once the keys are in.
pub fn ensure_running(app: &AppHandle) -> Result<(), String> {
    let history = {
        let shared = lock(app);
        if shared.events.is_some() {
            return Ok(());
        }
        shared.history.clone()
    };
    wire(app, history)?;
    set_status(app, "Ready");
    Ok(())
}

/// Opens the history store if it is not open yet. A history that cannot open leaves dictation
/// working, without history or retry.
fn open_history(history: &Mutex<HistoryInner>, dir: &std::path::Path, config: &Config) {
    let mut inner = history.lock().unwrap_or_else(PoisonError::into_inner);
    if inner.store.is_none() {
        inner.store = SqliteHistory::open(
            &dir.join("history"),
            HistorySettings::from_config(&config.history),
        )
        .ok();
    }
}

fn wire(app: &AppHandle, history: Arc<Mutex<HistoryInner>>) -> Result<(), String> {
    let dir = data_dir(app);
    let config =
        Config::load_or_create(&dir.join("config.toml")).map_err(|e| format!("Config: {e}"))?;
    let keys = KeychainStore::new();
    let key = |provider: &str| match keys.get(provider) {
        Ok(Some(key)) => Ok(key),
        Ok(None) => Err(format!("No {provider} key yet")),
        Err(e) => Err(e.to_string()),
    };
    let speech = Speechmatics::new(&config.speech, key(&config.speech.provider)?);
    let cleanup = ChatCleanup::new(&config.cleanup, &key(&config.cleanup.provider)?)
        .map_err(|e| e.message)?;
    open_history(&history, &dir, &config);

    let (events, events_rx) = mpsc::channel(1024);
    // The microphone's frames pass through a task that feeds the level meter.
    let (mic_tx, mic_rx) = mpsc::channel(1024);
    let hotkey =
        HotkeyListener::start(&config.dictation.hotkey, events.clone()).map_err(|e| e.message)?;
    let parts = Parts {
        mic: CpalMicrophone::new(mic_tx, SAMPLE_RATE),
        speech,
        cleanup,
        inserter: ClipboardInserter::new(Arc::new(MacPasteboard), CmdV, RESTORE_AFTER),
        focus: MacFocus,
        history: SharedHistory(history),
        sounds: SystemSounds,
        timing: FileTimingLog::new(
            timing_path(),
            Names {
                speech_provider: config.speech.provider.clone(),
                speech_model: config.speech.model.clone(),
                cleanup_provider: config.cleanup.provider.clone(),
                cleanup_model: config.cleanup.model.clone(),
            },
        ),
    };
    let (status_tx, status_rx) = mpsc::unbounded_channel();
    let engine = Engine::new(parts, Settings::from_config(&config), status_tx);
    tauri::async_runtime::spawn(engine.run(events_rx));
    tauri::async_runtime::spawn(forward_audio(app.clone(), mic_rx, events.clone()));
    tauri::async_runtime::spawn(show_statuses(app.clone(), status_rx));
    let mut shared = lock(app);
    shared.events = Some(events);
    shared._hotkey = Some(hotkey);
    Ok(())
}

/// Passes each frame on to the state machine and sends the level to the pill (R-4).
async fn forward_audio(
    app: AppHandle,
    mut frames: mpsc::Receiver<Event>,
    events: mpsc::Sender<Event>,
) {
    let mut last = Instant::now() - LEVEL_EVERY;
    while let Some(event) = frames.recv().await {
        if let Event::Audio(frame) = &event
            && last.elapsed() >= LEVEL_EVERY
        {
            last = Instant::now();
            let _ = app.emit_to(PILL, "level", pill::level(frame));
        }
        if events.send(event).await.is_err() {
            return;
        }
    }
}

/// Shows each state in the pill and the menu.
async fn show_statuses(app: AppHandle, mut statuses: mpsc::UnboundedReceiver<Status>) {
    // Key release, for the walkthrough's "Pasted in 0.43 s".
    let mut released: Option<Instant> = None;
    while let Some(status) = statuses.recv().await {
        follow_for_home(&app, &status);
        match status {
            Status::Transcribing if released.is_none() => released = Some(Instant::now()),
            Status::Done { .. } => {
                if let Some(at) = released.take() {
                    let ms = at.elapsed().as_millis() as u64;
                    let _ = app.emit_to(crate::walkthrough::WALKTHROUGH, "dictation-pasted", ms);
                }
            }
            Status::Idle => released = None,
            _ => {}
        }
        if let Status::Failed { message, retry } = &status {
            set_status(&app, message);
            let shared = lock(&app);
            let _ = shared.menu.retry.set_enabled(*retry);
        }
        if matches!(status, Status::Done { .. } | Status::Copied { .. }) {
            set_status(&app, "Ready");
            let _ = lock(&app).menu.retry.set_enabled(false);
        }
        match pill::change_for(&status) {
            Change::Show(view) => show(&app, view),
            Change::Hide => settle(&app),
            Change::Keep => {}
        }
    }
}

fn set_status(app: &AppHandle, text: &str) {
    // The menu shows one line; long provider messages are cut.
    let line: String = text.chars().take(80).collect();
    // On standard error too, for running from a terminal. Statuses never hold dictated text.
    eprintln!("HyperMe: {line}");
    let _ = lock(app).menu.status.set_text(line);
}

fn show(app: &AppHandle, view: View) {
    let (generation, was_pill) = {
        let mut shared = lock(app);
        shared.generation += 1;
        let was_pill = shared.view.as_ref().is_some_and(|v| v.kind != "idle");
        shared.view = Some(view.clone());
        (shared.generation, was_pill)
    };
    let width = pill::width_for(&view);
    if view.kind == "idle" && was_pill {
        // The page shrinks the capsule into the bead (the 180 ms spring); the panel follows once
        // it is done, so the animation is not cut off.
        let _ = app.emit_to(PILL, "pill", &view);
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            if lock(&app).generation == generation {
                place(&app, width);
            }
        });
    } else {
        // Growing: the panel takes the pill's size first, then the page grows the capsule.
        place(app, width);
        let _ = app.emit_to(PILL, "pill", &view);
    }
    on_panel(app, |panel| panel.show());
    if let Some(ms) = view.hide_after_ms {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(ms)).await;
            if lock(&app).generation == generation {
                settle(&app);
            }
        });
    }
}

/// Sizes the panel for `width` and puts it at the bottom centre.
fn place(app: &AppHandle, width: f64) {
    if let Some(window) = app.get_webview_window(PILL) {
        let (x, y) = pill_origin(app, width);
        let _ = window.set_size(Size::Logical(LogicalSize::new(
            width,
            geometry::PILL_SIZE.1,
        )));
        let _ = window.set_position(Position::Logical(LogicalPosition::new(x, y)));
    }
}

/// After a state: the bead when "Show while idle" is on, otherwise hidden.
fn settle(app: &AppHandle) {
    let show_while_idle = lock(app).show_while_idle;
    match pill::rest(show_while_idle) {
        Change::Show(view) => show(app, view),
        _ => hide(app),
    }
}

fn hide(app: &AppHandle) {
    {
        let mut shared = lock(app);
        shared.generation += 1;
        shared.view = None;
    }
    // The page stops drawing the voice, so nothing runs while the pill is hidden.
    let _ = app.emit_to(PILL, "pill-hidden", ());
    on_panel(app, |panel| panel.hide());
}

/// Runs `action` on the pill panel on the main thread. AppKit windows may only be ordered in or
/// out there: showing the pill from the status task, a tokio worker, crashed the app on the first
/// dictation (EXC_BREAKPOINT, "Must only be used from the main thread", 26 November 2026).
fn on_panel(app: &AppHandle, action: fn(&dyn tauri_nspanel::Panel)) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Ok(panel) = handle.get_webview_panel(PILL) {
            action(panel.as_ref());
        }
    });
}

/// A click on the pill: Retry when it offers Retry (R-31); on the bead, Settings for now;
/// otherwise the state goes away.
#[tauri::command]
pub fn pill_clicked(app: AppHandle) {
    let (retry, idle, events) = {
        let shared = lock(&app);
        (
            shared.view.as_ref().is_some_and(|v| v.retry),
            shared.view.as_ref().is_some_and(|v| v.kind == "idle"),
            shared.events.clone(),
        )
    };
    if idle {
        crate::settings::open(&app, "general");
        return;
    }
    settle(&app);
    if retry && let Some(events) = events {
        let _ = events.try_send(Event::Retry(None));
    }
}

/// Tells Home how a retry it asked for is going, and that the history changed after any other
/// dictation (spec 98, R-16, AC-15).
fn follow_for_home(app: &AppHandle, status: &Status) {
    use crate::home::{RetryEvent, RetryUpdate, retry_update};
    let retrying = {
        let mut shared = lock(app);
        shared.busy = *status != Status::Idle;
        shared.home_retry
    };
    let Some(id) = retrying else {
        if matches!(
            status,
            Status::Done { .. } | Status::Copied { .. } | Status::Failed { retry: true, .. }
        ) {
            crate::home::emit_changed(app);
        }
        return;
    };
    let event = |state, text, message| RetryEvent {
        id,
        state,
        text,
        message,
    };
    let (event, ended) = match retry_update(status) {
        Some(RetryUpdate::Working) => (event("working", None, None), false),
        Some(RetryUpdate::Done) => {
            let text = with_history(app, |h| h.item(id))
                .ok()
                .flatten()
                .and_then(|i| i.text);
            (event("done", text, None), true)
        }
        Some(RetryUpdate::Failed(message)) => (event("failed", None, Some(message)), true),
        Some(RetryUpdate::Ended) => (event("ended", None, None), true),
        None => return,
    };
    if ended {
        lock(app).home_retry = None;
    }
    crate::home::emit_retry(app, event);
}

/// Home's Retry (spec 98, R-16): refused while a dictation runs, as Phase 1 refuses a second
/// one, so the row says so instead of waiting on nothing.
pub fn retry_from_home(app: &AppHandle, id: DictationId) -> Result<(), String> {
    let mut shared = lock(app);
    if shared.busy || shared.home_retry.is_some() {
        return Err("A dictation is in progress; retry when it is done".to_owned());
    }
    let events = shared.events.clone().ok_or_else(|| {
        "Retry needs the speech and cleanup keys; add them in Settings".to_owned()
    })?;
    events
        .try_send(Event::Retry(Some(id)))
        .map_err(|_| "HyperMe is busy; try again".to_owned())?;
    shared.home_retry = Some(id);
    Ok(())
}

/// History is on in the config (spec 98, R-19).
pub fn history_enabled(app: &AppHandle) -> bool {
    lock(app).history_enabled
}

/// Runs `change` on the history store, for Home's actions.
pub fn with_history_mut<T>(
    app: &AppHandle,
    change: impl FnOnce(&mut SqliteHistory) -> Result<T, Error>,
) -> Result<T, String> {
    let history = lock(app).history.clone();
    let mut inner = history.lock().unwrap_or_else(PoisonError::into_inner);
    let store = inner
        .store
        .as_mut()
        .ok_or_else(|| "History is not available yet".to_owned())?;
    change(store).map_err(|e| e.message)
}

/// Runs `read` on the history store, for Home's commands. An error when the history is not
/// open (no keys yet, or it could not open).
pub fn with_history<T>(
    app: &AppHandle,
    read: impl FnOnce(&SqliteHistory) -> Result<T, Error>,
) -> Result<T, String> {
    let history = lock(app).history.clone();
    let inner = history.lock().unwrap_or_else(PoisonError::into_inner);
    let store = inner
        .store
        .as_ref()
        .ok_or_else(|| "History is not available yet".to_owned())?;
    read(store).map_err(|e| e.message)
}

/// The menu's actions.
pub fn menu_action(app: &AppHandle, id: &str) {
    match id {
        "paste-last" | "copy-last" => {
            let text = {
                let shared = lock(app);
                let inner = shared
                    .history
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                inner.last_text()
            };
            let Some(text) = text else {
                set_status(app, "No dictation yet");
                return;
            };
            let paste = id == "paste-last";
            tauri::async_runtime::spawn(async move {
                // Let the menu close and focus return to the app the user was in.
                tokio::time::sleep(Duration::from_millis(150)).await;
                let mut inserter =
                    ClipboardInserter::new(Arc::new(MacPasteboard), CmdV, RESTORE_AFTER);
                let _ = if paste {
                    inserter.paste(&text).await
                } else {
                    inserter.copy(&text).await
                };
            });
        }
        "settings" => crate::settings::open(app, "setup"),
        "open-home" => crate::home::open(app, "home"),
        "retry" => {
            if let Some(events) = lock(app).events.clone() {
                let _ = events.try_send(Event::Retry(None));
            }
        }
        _ => {}
    }
}

/// The history store, and the last dictation's text in memory, so "Paste last dictation"
/// works with history off too (R-18: then it is never written to disk).
#[derive(Default)]
struct HistoryInner {
    store: Option<SqliteHistory>,
    last_text: Option<String>,
}

impl HistoryInner {
    fn last_text(&self) -> Option<String> {
        self.last_text
            .clone()
            .or_else(|| self.store.as_ref()?.last_text().ok().flatten())
    }

    fn store(&mut self) -> Result<&mut SqliteHistory, Error> {
        self.store.as_mut().ok_or_else(|| Error {
            kind: ErrorKind::Device,
            message: "history is not available".to_owned(),
        })
    }
}

/// The history as the state machine sees it, shared with the menu.
struct SharedHistory(Arc<Mutex<HistoryInner>>);

impl SharedHistory {
    fn inner(&self) -> std::sync::MutexGuard<'_, HistoryInner> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl History for SharedHistory {
    fn begin(&mut self, app: Option<&str>) -> Result<DictationId, Error> {
        self.inner().store()?.begin(app)
    }

    fn append(&mut self, id: DictationId, pcm: &[i16]) -> Result<(), Error> {
        self.inner().store()?.append(id, pcm)
    }

    fn done(&mut self, id: DictationId, text: &str, language: Option<&str>) -> Result<(), Error> {
        let mut inner = self.inner();
        inner.last_text = Some(text.to_owned());
        inner.store()?.done(id, text, language)
    }

    fn failed(&mut self, id: DictationId, message: &str) -> Result<(), Error> {
        self.inner().store()?.failed(id, message)
    }

    fn discard(&mut self, id: DictationId) -> Result<(), Error> {
        self.inner().store()?.discard(id)
    }

    fn audio(&self, id: DictationId) -> Result<Vec<i16>, Error> {
        let inner = self.inner();
        let store = inner.store.as_ref().ok_or_else(|| Error {
            kind: ErrorKind::Device,
            message: "history is not available".to_owned(),
        })?;
        store.audio(id)
    }

    fn app(&self, id: DictationId) -> Result<Option<String>, Error> {
        let inner = self.inner();
        let store = inner.store.as_ref().ok_or_else(|| Error {
            kind: ErrorKind::Device,
            message: "history is not available".to_owned(),
        })?;
        store.app(id)
    }

    fn replace_text(
        &mut self,
        id: DictationId,
        text: &str,
        language: Option<&str>,
    ) -> Result<(), Error> {
        self.inner().store()?.replace_text(id, text, language)
    }
}
