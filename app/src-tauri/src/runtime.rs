//! The dictation at run time on macOS: the config, the keys from the keychain, the real
//! providers and devices, the state machine on Tauri's async runtime, and its states shown in
//! the pill and the menu.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use hyperme_core::config::Config;
use hyperme_core::dictation::{Engine, Event, Parts, Providers, Settings, Status};
use hyperme_core::dictionary::Dictionary;
use hyperme_core::traits::{
    DictationId, Error, ErrorKind, History, Inserter, TimingLine, TimingLog,
};
use hyperme_platform::history::{HistorySettings, SqliteHistory};
use hyperme_platform::hotkey::macos::HotkeyListener;
use hyperme_platform::keychain::KeychainStore;
use hyperme_platform::microphone::CpalMicrophone;
use hyperme_platform::paste::macos::{CmdV, MacFocus, MacPasteboard};
use hyperme_platform::paste::{ClipboardInserter, RESTORE_AFTER};
use hyperme_platform::sounds::SystemSounds;
use hyperme_platform::timing::{FileTimingLog, Names};
use hyperme_providers::any::{self, AnyCleanup, AnySpeech};
use hyperme_providers::speechmatics::SAMPLE_RATE;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Position, Size};
use tauri_nspanel::ManagerExt;
use tokio::sync::{mpsc, watch};

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
    /// Replaces the providers the next dictation uses (specs/96-provider-settings, R-8).
    providers: Option<watch::Sender<Arc<Providers<AnySpeech, AnyCleanup>>>>,
    /// The timing log, whose provider and model names follow a switch.
    timing: Arc<Mutex<FileTimingLog>>,
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
        providers: None,
        timing: Arc::new(Mutex::new(timing_log(&Config::default()))),
    }));
    settle(app);
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

fn wire(app: &AppHandle, history: Arc<Mutex<HistoryInner>>) -> Result<(), String> {
    let dir = data_dir(app);
    let config =
        Config::load_or_create(&dir.join("config.toml")).map_err(|e| format!("Config: {e}"))?;
    let dictionary = load_dictionary(config.dictation.dictionary.as_deref());
    let providers = with_dictionary(
        any::resolve(&config, &KeychainStore::new()).map_err(|e| e.message)?,
        &dictionary,
    );
    let (switch, providers) = watch::channel(Arc::new(providers));
    let timing = lock(app).timing.clone();
    *timing.lock().unwrap_or_else(PoisonError::into_inner) = timing_log(&config);
    {
        let mut inner = history.lock().unwrap_or_else(PoisonError::into_inner);
        // A history that cannot open leaves dictation working, without history or retry.
        inner.store = SqliteHistory::open(
            &dir.join("history"),
            HistorySettings::from_config(&config.history),
        )
        .ok();
    }

    let (events, events_rx) = mpsc::channel(1024);
    // The microphone's frames pass through a task that feeds the level meter.
    let (mic_tx, mic_rx) = mpsc::channel(1024);
    let hotkey =
        HotkeyListener::start(&config.dictation.hotkey, events.clone()).map_err(|e| e.message)?;
    let parts = Parts {
        mic: CpalMicrophone::new(mic_tx, SAMPLE_RATE),
        providers,
        inserter: ClipboardInserter::new(Arc::new(MacPasteboard), CmdV, RESTORE_AFTER),
        focus: MacFocus,
        history: SharedHistory(history),
        sounds: SystemSounds,
        timing: SharedTiming(timing),
    };
    let (status_tx, status_rx) = mpsc::unbounded_channel();
    let settings = Settings {
        dictionary,
        ..Settings::from_config(&config)
    };
    let engine = Engine::new(parts, settings, status_tx);
    tauri::async_runtime::spawn(engine.run(events_rx));
    tauri::async_runtime::spawn(forward_audio(app.clone(), mic_rx, events.clone()));
    tauri::async_runtime::spawn(show_statuses(app.clone(), status_rx));
    let mut shared = lock(app);
    shared.events = Some(events);
    shared._hotkey = Some(hotkey);
    shared.providers = Some(switch);
    Ok(())
}

/// Builds the providers the config names now and hands them to the next dictation, without a
/// restart (R-8). A dictation in progress keeps its own (R-9). If they cannot be built, a key
/// missing for example, the providers in use stay and the reason is returned (R-12). Does
/// nothing before dictation has started; it reads the config when it does.
pub fn reload_providers(app: &AppHandle) -> Result<(), String> {
    let Some(switch) = lock(app).providers.clone() else {
        return Ok(());
    };
    let config = Config::load_or_create(&data_dir(app).join("config.toml"))
        .map_err(|e| format!("Config: {e}"))?;
    let dictionary = load_dictionary(config.dictation.dictionary.as_deref());
    let providers = with_dictionary(
        any::resolve(&config, &KeychainStore::new()).map_err(|e| e.message)?,
        &dictionary,
    );
    let timing = lock(app).timing.clone();
    *timing.lock().unwrap_or_else(PoisonError::into_inner) = timing_log(&config);
    switch.send_replace(Arc::new(providers));
    Ok(())
}

/// The timing log, labelled with the config's providers and models.
fn timing_log(config: &Config) -> FileTimingLog {
    FileTimingLog::new(
        timing_path(),
        Names {
            speech_provider: config.speech.provider.clone(),
            speech_model: config.speech.model.clone(),
            cleanup_provider: config.cleanup.provider.clone(),
            cleanup_model: config.cleanup.model.clone(),
        },
    )
}

/// The timing log behind a lock, so a provider switch can relabel it.
struct SharedTiming(Arc<Mutex<FileTimingLog>>);

impl TimingLog for SharedTiming {
    fn record(&mut self, line: &TimingLine) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .record(line);
    }
}

/// The cleanup provider with the user's dictionary in its prompt (spec 93, R-4).
fn with_dictionary(
    providers: Providers<AnySpeech, AnyCleanup>,
    dictionary: &Dictionary,
) -> Providers<AnySpeech, AnyCleanup> {
    Providers {
        cleanup: providers.cleanup.with_dictionary(&dictionary.prompt_line()),
        ..providers
    }
}

/// The dictionary file named in the config (spec 93). A missing or unreadable file leaves
/// dictation working with no dictionary.
fn load_dictionary(path: Option<&std::path::Path>) -> Dictionary {
    let Some(path) = path else {
        return Dictionary::default();
    };
    match std::fs::read_to_string(path) {
        Ok(text) => Dictionary::parse(&text),
        Err(e) => {
            eprintln!("HyperMe: dictionary {} not read: {e}", path.display());
            Dictionary::default()
        }
    }
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

/// The pill's current view, for the page once it has loaded: an event sent before its listeners
/// exist is lost, and the bead is sent at launch (26 November 2026, the bead showed the hidden
/// recording bars instead).
#[tauri::command]
pub fn pill_view(app: AppHandle) -> Option<View> {
    lock(&app).view.clone()
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
        let _ = events.try_send(Event::Retry);
    }
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
        "retry" => {
            if let Some(events) = lock(app).events.clone() {
                let _ = events.try_send(Event::Retry);
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
}
