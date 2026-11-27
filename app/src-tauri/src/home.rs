//! The Home window (spec 98; docs/UX.md, "Home" and "Home sections"): a normal Mac window with a
//! sidebar and a page, created when opened and destroyed when closed, so it costs no memory while
//! closed (R-2). While it is open HyperMe shows a Dock icon (R-3).
//!
//! The section list, the Home page's block list and the target resolution are pure and tested on
//! every platform; the window and its commands are macOS only, like Settings.

use serde::Serialize;

/// The Home window's label. Only the macOS window code uses it so far.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub const HOME: &str = "home";

/// One section in the sidebar (R-5). Only sections whose feature exists are listed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section {
    pub id: &'static str,
    pub title: &'static str,
    /// The page that draws it; the Home page itself is part of `home.html`.
    pub page: &'static str,
    /// ⌘1 to ⌘5, by position.
    pub shortcut: &'static str,
}

/// The sidebar's sections in order (R-5, AC-4). Every view is a sidebar item, never a tab: Home
/// is the day, History the dictations (V0.1 redesign, owner, 27 November 2026); Calendar and
/// Commands join later.
/// The Dictionary appears only while the app holds a dictionary (R-21); Style and Meetings come
/// with their own changes.
pub fn sections(dictionary_present: bool) -> Vec<Section> {
    let mut list = vec![
        Section {
            id: "home",
            title: "Home",
            page: "home.html",
            shortcut: "⌘1",
        },
        Section {
            id: "history",
            title: "History",
            page: "home.html",
            shortcut: "⌘2",
        },
    ];
    if dictionary_present {
        list.push(Section {
            id: "dictionary",
            title: "Dictionary",
            page: "home-dictionary.html",
            shortcut: "⌘3",
        });
    }
    list
}

/// One block on the Home page, above Recent (R-9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Block {
    pub id: &'static str,
    pub title: &'static str,
}

/// The Home page's blocks, in order (R-9), above the day. None ships now. A block that shows is
/// listed here; the page asks for the list on load and on `home://refresh`.
pub fn blocks() -> Vec<Block> {
    Vec::new()
}

/// Where a target opens (R-6, AC-5): a section id opens that section; a Home-page block id
/// opens the Home page at that block; anything else opens the Home page.
pub fn resolve(
    target: &str,
    sections: &[Section],
    blocks: &[Block],
) -> (&'static str, Option<&'static str>) {
    if let Some(section) = sections.iter().find(|s| s.id == target) {
        return (section.id, None);
    }
    if let Some(block) = blocks.iter().find(|b| b.id == target) {
        return ("home", Some(block.id));
    }
    ("home", None)
}

/// The URL hash for a resolved target: `history`, or `home/today` for a block.
pub fn hash(resolved: (&str, Option<&str>)) -> String {
    match resolved {
        (section, Some(block)) => format!("{section}/{block}"),
        (section, None) => section.to_owned(),
    }
}

// ---- Row actions (slice 4): the decisions, pure and tested on every platform ----

use hyperme_core::dictation::Status;
use hyperme_core::traits::{Added, Correction, DictationId, Dictionary, Error, Inserter};

/// The app that was in front before Home took focus (R-15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontApp {
    pub pid: i32,
    pub name: String,
}

/// What Paste again does (R-15, AC-9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasteDecision {
    /// The recorded app is in front again: paste there.
    Paste,
    /// Copy instead, and say why.
    Copy(String),
}

/// Paste again pastes only into the app recorded before Home took focus, once it is in front;
/// never while Home itself is in front. `frontmost` is the app in front after asking the
/// recorded one to come forward (R-15, AC-9).
pub fn paste_decision(
    recorded: Option<&FrontApp>,
    own_pid: i32,
    frontmost: Option<i32>,
) -> PasteDecision {
    match recorded {
        Some(app) if app.pid != own_pid && frontmost == Some(app.pid) => PasteDecision::Paste,
        Some(app) if app.pid != own_pid => {
            PasteDecision::Copy(format!("Copied: {} is no longer open", app.name))
        }
        _ => PasteDecision::Copy("Copied: there was no app to paste into".to_owned()),
    }
}

/// Brings an app to the front and says which app is in front afterwards.
pub trait Focuser {
    fn bring_to_front(&self, pid: i32) -> impl std::future::Future<Output = Option<i32>> + Send;
}

/// What Paste again did, for the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PasteOutcome {
    pub pasted: bool,
    pub notice: Option<String>,
}

/// Paste again (R-15, AC-9): back to the recorded app and paste, or copy with a notice when it
/// is gone or does not come to the front.
pub async fn paste_again<I: Inserter>(
    inserter: &mut I,
    focus: &impl Focuser,
    recorded: Option<&FrontApp>,
    own_pid: i32,
    text: &str,
) -> Result<PasteOutcome, Error> {
    let frontmost = match recorded {
        Some(app) if app.pid != own_pid => focus.bring_to_front(app.pid).await,
        _ => None,
    };
    match paste_decision(recorded, own_pid, frontmost) {
        PasteDecision::Paste => {
            inserter.paste(text).await?;
            Ok(PasteOutcome {
                pasted: true,
                notice: None,
            })
        }
        PasteDecision::Copy(notice) => {
            inserter.copy(text).await?;
            Ok(PasteOutcome {
                pasted: false,
                notice: Some(notice),
            })
        }
    }
}

/// One action on a selected row, with its key (R-13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Action {
    pub id: &'static str,
    pub label: &'static str,
    pub key: &'static str,
}

/// The actions a row offers (R-13): Copy and Paste again need text, Retry needs the audio
/// (R-16, AC-10), Fix a word needs a dictionary and text (R-17, R-21), Delete is always there.
pub fn actions(has_text: bool, has_audio: bool, dictionary: bool) -> Vec<Action> {
    let mut list = Vec::new();
    let mut add = |id, label, key| list.push(Action { id, label, key });
    if has_text {
        add("copy", "Copy", "⌘C");
        add("paste", "Paste again", "↵");
    }
    if has_audio {
        add("retry", "Retry", "⌘R");
    }
    if has_text && dictionary {
        add("fix", "Fix a word", "⌘F");
    }
    add("delete", "Delete", "⌘⌫");
    list
}

/// What Fix a word did (R-17, AC-11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum FixOutcome {
    /// Added: the row's text with the correction, to store and show.
    Fixed { text: String },
    /// The dictionary warns that the word is common; ask, then send again confirmed.
    NeedsConfirm { heard: String },
}

/// Sends a correction to the dictionary (R-17, R-20, AC-11). The selection `heard` must be in
/// the row's text. On success the row's text has every `heard` replaced, as the dictionary will
/// do from now on; a hard refusal is the dictionary's message and changes nothing.
pub fn fix_word(
    dictionary: &mut dyn Dictionary,
    id: DictationId,
    text: &str,
    language: Option<&str>,
    heard: &str,
    wanted: &str,
    confirmed_common: bool,
) -> Result<FixOutcome, String> {
    let heard = heard.trim();
    let wanted = wanted.trim();
    if heard.is_empty() || wanted.is_empty() {
        return Err("Select a word and type what it should be".to_owned());
    }
    if !text.contains(heard) {
        return Err(format!("“{heard}” is not in this dictation"));
    }
    let correction = Correction {
        heard: heard.to_owned(),
        wanted: wanted.to_owned(),
        dictation: Some(id),
        language: language.map(str::to_owned),
        confirmed_common,
    };
    match dictionary.add_correction(correction) {
        Ok(Added::Added) => Ok(FixOutcome::Fixed {
            text: text.replace(heard, wanted),
        }),
        Ok(Added::NeedsConfirm(_)) => Ok(FixOutcome::NeedsConfirm {
            heard: heard.to_owned(),
        }),
        Err(error) => Err(error.message),
    }
}

/// How a Home retry is going, from the engine's statuses (R-16, AC-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryUpdate {
    Working,
    /// The new text is in the store and on the clipboard.
    Done,
    /// The old text stays; the message says why.
    Failed(String),
    /// The retry ended without a result (back to idle).
    Ended,
}

pub fn retry_update(status: &Status) -> Option<RetryUpdate> {
    match status {
        Status::Transcribing | Status::Cleaning | Status::Pasting => Some(RetryUpdate::Working),
        Status::Copied { .. } | Status::Done { .. } => Some(RetryUpdate::Done),
        Status::Failed { message, .. } => Some(RetryUpdate::Failed(message.clone())),
        Status::NothingHeard => Some(RetryUpdate::Failed(
            "Nothing heard in the audio; the text is unchanged".to_owned(),
        )),
        Status::Cancelled => Some(RetryUpdate::Failed("Cancelled".to_owned())),
        Status::Idle => Some(RetryUpdate::Ended),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
pub use window::*;

#[cfg(target_os = "macos")]
mod window {
    use std::path::PathBuf;
    use std::sync::{Mutex, PoisonError};

    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use hyperme_core::traits::{DictationId, Dictionary, History, Inserter};
    use hyperme_platform::history::{Item, Stats};
    use hyperme_platform::paste::macos::{self as mac, CmdV, MacPasteboard};
    use hyperme_platform::paste::{ClipboardInserter, RESTORE_AFTER};
    use serde::{Deserialize, Serialize};
    use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

    use super::{
        Action, Block, FixOutcome, Focuser, FrontApp, HOME, PasteOutcome, Section, actions, blocks,
        fix_word, hash, paste_again, resolve, sections,
    };

    /// Sent to the page when the attention dot changes (R-22).
    pub const ATTENTION: &str = "home://attention";
    /// Sent to the page to reload a block or a section while Home is open (R-22).
    pub const REFRESH: &str = "home://refresh";
    /// The page emits this with a block's id once the block has been on screen (R-22). Other
    /// modules (#92's Waiting for you, for example) listen with `app.listen_any(BLOCK_VIEWED,
    /// ...)`; Home itself does nothing with it.
    pub const BLOCK_VIEWED: &str = "home://block-viewed";

    const DEFAULT_SIZE: (f64, f64) = (900.0, 600.0);
    const MIN_SIZE: (f64, f64) = (720.0, 480.0);
    /// At most this many items per page or search call.
    const MAX_LIMIT: usize = 200;
    /// How long Paste again waits for the recorded app to come to the front (R-15).
    const FRONT_WAIT: Duration = Duration::from_millis(500);
    /// Sent to the page when a dictation is added or changed outside Home (R-22, AC-15).
    pub const CHANGED: &str = "home://changed";
    /// Sent to the page as a retry from Home goes, keyed by the item's id (R-16).
    pub const RETRY: &str = "home://retry";

    /// Home's state while the app runs.
    #[derive(Default)]
    pub struct HomeState {
        /// The violet dot on Home in the sidebar (R-22).
        attention: bool,
        /// The app in front before Home took focus, for Paste again (R-15).
        front_app: Option<FrontApp>,
        /// The personal dictionary, when the app holds one (R-21): the dictionary file (#93,
        /// slice D5). While it is None nothing in Home mentions the dictionary.
        dictionary: Option<Box<dyn Dictionary + Send>>,
    }

    type State = Mutex<HomeState>;

    fn state(app: &AppHandle) -> std::sync::MutexGuard<'_, HomeState> {
        app.state::<State>()
            .inner()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Registers Home's state. Called once at setup.
    pub fn init(app: &AppHandle) {
        app.manage::<State>(Mutex::new(HomeState::default()));
    }

    /// Hands Home the personal dictionary (#93), which turns on Fix a word (R-17, R-21).
    pub fn set_dictionary(app: &AppHandle, dictionary: Box<dyn Dictionary + Send>) {
        state(app).dictionary = Some(dictionary);
    }

    fn has_dictionary(app: &AppHandle) -> bool {
        state(app).dictionary.is_some()
    }

    /// The window's size and position, kept between openings (R-4).
    #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
    pub struct Geometry {
        pub x: f64,
        pub y: f64,
        pub width: f64,
        pub height: f64,
    }

    impl Geometry {
        /// A saved geometry, if the file holds a sane one.
        pub fn parse(text: &str) -> Option<Geometry> {
            let g: Geometry = serde_json::from_str(text).ok()?;
            let finite = [g.x, g.y, g.width, g.height].iter().all(|v| v.is_finite());
            (finite && g.width >= MIN_SIZE.0 && g.height >= MIN_SIZE.1).then_some(g)
        }
    }

    fn geometry_path(app: &AppHandle) -> PathBuf {
        crate::runtime::data_dir(app).join("home-window.json")
    }

    fn save_geometry(app: &AppHandle, window: &tauri::WebviewWindow) {
        let (Ok(position), Ok(size), Ok(scale)) = (
            window.outer_position(),
            window.inner_size(),
            window.scale_factor(),
        ) else {
            return;
        };
        let position = position.to_logical::<f64>(scale);
        let size = size.to_logical::<f64>(scale);
        let geometry = Geometry {
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
        };
        if let Ok(text) = serde_json::to_string(&geometry) {
            let _ = std::fs::write(geometry_path(app), text);
        }
    }

    /// Opens Home at `target` (R-1, R-6), or brings it to the front there.
    pub fn open(app: &AppHandle, target: &str) {
        let resolved = hash(resolve(target, &sections(has_dictionary(app)), &blocks()));
        if let Some(window) = app.get_webview_window(HOME) {
            // Already in front: the app in front is Home itself, so keep the one recorded.
            if !window.is_focused().unwrap_or(false) {
                record_front_app(app);
            }
            let _ = window.eval(format!("location.hash = {}", serde_json::json!(resolved)));
            let _ = window.show();
            let _ = window.set_focus();
            return;
        }
        record_front_app(app);
        // R-3: a Dock icon and an app menu while Home is open.
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
        let saved = std::fs::read_to_string(geometry_path(app))
            .ok()
            .and_then(|text| Geometry::parse(&text));
        let url = format!("home.html#{resolved}");
        let mut builder = WebviewWindowBuilder::new(app, HOME, WebviewUrl::App(url.into()))
            .title("HyperMe")
            .min_inner_size(MIN_SIZE.0, MIN_SIZE.1);
        builder = match saved {
            Some(g) => builder.inner_size(g.width, g.height).position(g.x, g.y),
            None => builder.inner_size(DEFAULT_SIZE.0, DEFAULT_SIZE.1).center(),
        };
        match builder.build() {
            Ok(window) => {
                // The handler looks the window up by label: holding a clone of the window in its
                // own handler is a cycle that keeps the native window alive after it closes.
                let handle = app.clone();
                window.on_window_event(move |event| match event {
                    WindowEvent::CloseRequested { .. } => {
                        if let Some(win) = handle.get_webview_window(HOME) {
                            save_geometry(&handle, &win);
                        }
                    }
                    // The user went to another app: that is where Paste again goes back to.
                    WindowEvent::Focused(false) => record_front_app(&handle),
                    WindowEvent::Destroyed => closed(&handle),
                    _ => {}
                });
                let _ = window.set_focus();
            }
            Err(_) => closed(app),
        }
    }

    /// R-2, R-3: the window and its web view are gone; the Dock icon goes too unless another
    /// window of the app is still open.
    fn closed(app: &AppHandle) {
        // The Settings sheet belongs to Home and goes with it (V0.1 redesign).
        if let Some(sheet) = app.get_webview_window(crate::settings::SETTINGS) {
            let _ = sheet.close();
        }
        let others = [crate::settings::SETTINGS, crate::walkthrough::WALKTHROUGH];
        if !others.iter().any(|w| app.get_webview_window(w).is_some()) {
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
        }
        free_closed_window(app);
    }

    /// AppKit frees a closed window, and WebKit ends its page process, on the next main-loop
    /// turn. An idle menu-bar app may not get one until the user next does something, so the
    /// page's 25 MB or more stays (spec 98 AC-2; the same leak in Settings, 27 November 2026).
    /// Posts an event so that turn comes now. Twice, a moment apart: the first turn runs the
    /// close, the second the release. Every window calls this once it is destroyed.
    pub fn free_closed_window(app: &AppHandle) {
        let app = app.clone();
        std::thread::spawn(move || {
            for _ in 0..2 {
                std::thread::sleep(Duration::from_millis(300));
                let _ = app.run_on_main_thread(mac::wake_main_loop);
            }
        });
    }

    fn own_pid() -> i32 {
        std::process::id() as i32
    }

    /// Records the app in front, unless it is HyperMe itself (R-15).
    fn record_front_app(app: &AppHandle) {
        if let Some((pid, name)) = mac::frontmost()
            && pid != own_pid()
        {
            state(app).front_app = Some(FrontApp { pid, name });
        }
    }

    /// Shows or hides the violet dot on Home in the sidebar (R-22). For other modules.
    pub fn set_attention(app: &AppHandle, on: bool) {
        state(app).attention = on;
        let _ = app.emit_to(HOME, ATTENTION, on);
    }

    /// Makes the open page reload a block or a section (R-22). For other modules.
    pub fn refresh(app: &AppHandle, target: &str) {
        let _ = app.emit_to(HOME, REFRESH, target);
    }

    /// The sidebar's sections (R-5).
    #[tauri::command]
    pub fn home_sections(app: AppHandle) -> Vec<Section> {
        sections(has_dictionary(&app))
    }

    /// The Home page's blocks (R-9).
    #[tauri::command]
    pub fn home_blocks() -> Vec<Block> {
        blocks()
    }

    /// Sets the attention dot when `on` is given; returns it either way, so the page can ask on
    /// load.
    #[tauri::command]
    pub fn home_attention(app: AppHandle, on: Option<bool>) -> bool {
        if let Some(on) = on {
            set_attention(&app, on);
        }
        state(&app).attention
    }

    /// Settings and Help at the bottom of the sidebar open the Settings window at a section
    /// (docs/UX.md: ⌘, opens it at the section closest to where you are).
    #[tauri::command]
    pub fn home_settings(app: AppHandle, section: String) {
        crate::settings::open(&app, &section);
    }

    #[tauri::command]
    pub fn home_refresh(app: AppHandle, target: String) {
        refresh(&app, &target);
    }

    /// One dictation as the page sees it (R-24, AC-14): its text and facts, never a path to its
    /// audio or anything about keys.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
    pub struct ItemView {
        pub id: i64,
        pub started_ms: i64,
        pub app: Option<String>,
        pub language: Option<String>,
        pub text: Option<String>,
        pub state: String,
        pub message: Option<String>,
        /// Whether Retry can be offered (R-16).
        pub has_audio: bool,
        /// What the row offers when selected, with keys (R-13).
        pub actions: Vec<Action>,
    }

    fn view(item: Item, dictionary: bool) -> ItemView {
        let has_text = item.state == "done" && item.text.as_deref().is_some_and(|t| !t.is_empty());
        ItemView {
            actions: actions(has_text, item.has_audio, dictionary),
            id: item.id,
            started_ms: item.started_ms,
            app: item.app,
            language: item.language,
            text: item.text,
            state: item.state,
            message: item.message,
            has_audio: item.has_audio,
        }
    }

    /// The Home page's numbers (R-7).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
    pub struct StatsView {
        pub words_this_week: u64,
        pub seconds_saved: i64,
    }

    impl From<Stats> for StatsView {
        fn from(stats: Stats) -> Self {
            StatsView {
                words_this_week: stats.words_this_week,
                seconds_saved: stats.seconds_saved,
            }
        }
    }

    fn now_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64)
    }

    fn views(app: &AppHandle, items: Vec<Item>) -> Vec<ItemView> {
        let dictionary = has_dictionary(app);
        items.into_iter().map(|i| view(i, dictionary)).collect()
    }

    /// The page's local time zone comes from the page, which knows it (R-7).
    #[tauri::command]
    pub fn home_stats(app: AppHandle, utc_offset_minutes: i32) -> Result<StatsView, String> {
        crate::runtime::with_history(&app, |h| h.stats(now_ms(), utc_offset_minutes))
            .map(StatsView::from)
    }

    /// A page of History or Recent (R-8, R-10): items older than `before`, `[started_ms, id]`
    /// of the last item shown.
    #[tauri::command]
    pub fn home_page(
        app: AppHandle,
        before: Option<(i64, i64)>,
        limit: usize,
    ) -> Result<Vec<ItemView>, String> {
        crate::runtime::with_history(&app, |h| h.page(before, limit.min(MAX_LIMIT)))
            .map(|items| views(&app, items))
    }

    /// History's search (R-12).
    #[tauri::command]
    pub fn home_search(
        app: AppHandle,
        query: String,
        before: Option<(i64, i64)>,
        limit: usize,
    ) -> Result<Vec<ItemView>, String> {
        crate::runtime::with_history(&app, |h| h.search(&query, before, limit.min(MAX_LIMIT)))
            .map(|items| views(&app, items))
    }

    /// What the page needs to know once (R-19, R-21).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
    pub struct Info {
        /// History is on; while it is off, History lists only failed items with audio.
        pub history: bool,
        pub dictionary: bool,
    }

    #[tauri::command]
    pub fn home_info(app: AppHandle) -> Info {
        Info {
            history: crate::runtime::history_enabled(&app),
            dictionary: has_dictionary(&app),
        }
    }

    /// History while history is off: the failed dictations still waiting for a retry (R-19).
    #[tauri::command]
    pub fn home_failed(app: AppHandle) -> Result<Vec<ItemView>, String> {
        crate::runtime::with_history(&app, |h| h.failed_with_audio()).map(|i| views(&app, i))
    }

    /// One item again, after an action changed it.
    #[tauri::command]
    pub fn home_item(app: AppHandle, id: DictationId) -> Result<Option<ItemView>, String> {
        let dictionary = has_dictionary(&app);
        crate::runtime::with_history(&app, |h| h.item(id)).map(|i| i.map(|i| view(i, dictionary)))
    }

    /// The text of a finished dictation, from the store, not from the page.
    fn text_of(app: &AppHandle, id: DictationId) -> Result<(String, Option<String>), String> {
        let item = crate::runtime::with_history(app, |h| h.item(id))?
            .ok_or_else(|| "This dictation is gone".to_owned())?;
        match item.text {
            Some(text) if item.state == "done" && !text.is_empty() => Ok((text, item.language)),
            _ => Err("This dictation has no text".to_owned()),
        }
    }

    fn inserter() -> ClipboardInserter<MacPasteboard, CmdV> {
        ClipboardInserter::new(Arc::new(MacPasteboard), CmdV, RESTORE_AFTER)
    }

    /// Copy (R-14).
    #[tauri::command]
    pub async fn home_copy(app: AppHandle, id: DictationId) -> Result<(), String> {
        let (text, _) = text_of(&app, id)?;
        inserter().copy(&text).await.map_err(|e| e.message)
    }

    /// Brings an app forward and waits, at most [`FRONT_WAIT`], until it is in front.
    struct MacFocuser;

    impl Focuser for MacFocuser {
        async fn bring_to_front(&self, pid: i32) -> Option<i32> {
            let waited = tauri::async_runtime::spawn_blocking(move || {
                if !mac::activate(pid) {
                    return mac::frontmost().map(|(p, _)| p);
                }
                let start = Instant::now();
                loop {
                    let front = mac::frontmost().map(|(p, _)| p);
                    if front == Some(pid) || start.elapsed() >= FRONT_WAIT {
                        return front;
                    }
                    std::thread::sleep(Duration::from_millis(15));
                }
            });
            waited.await.ok().flatten()
        }
    }

    /// Paste again (R-15, AC-9): back to the app in front before Home, paste there; Home stays
    /// open behind it. Copies with a notice when that app is gone.
    #[tauri::command]
    pub async fn home_paste(app: AppHandle, id: DictationId) -> Result<PasteOutcome, String> {
        let (text, _) = text_of(&app, id)?;
        let recorded = state(&app).front_app.clone();
        paste_again(
            &mut inserter(),
            &MacFocuser,
            recorded.as_ref(),
            own_pid(),
            &text,
        )
        .await
        .map_err(|e| e.message)
    }

    /// Retry (R-16): the engine sends the saved audio again; the row follows `home://retry`.
    #[tauri::command]
    pub fn home_retry(app: AppHandle, id: DictationId) -> Result<(), String> {
        crate::runtime::retry_from_home(&app, id)
    }

    /// Delete (R-18, AC-12): the item's text and audio.
    #[tauri::command]
    pub fn home_delete(app: AppHandle, id: DictationId) -> Result<(), String> {
        crate::runtime::with_history_mut(&app, |h| h.delete_item(id))
    }

    /// Fix a word (R-17, AC-11): the selection and what it should be go to the dictionary; on
    /// success the row's text in the store has the correction.
    #[tauri::command]
    pub fn home_fix_word(
        app: AppHandle,
        id: DictationId,
        heard: String,
        wanted: String,
        confirmed_common: bool,
    ) -> Result<FixOutcome, String> {
        let (text, language) = text_of(&app, id)?;
        let outcome = {
            let mut home = state(&app);
            let dictionary = home
                .dictionary
                .as_deref_mut()
                .ok_or_else(|| "There is no dictionary yet".to_owned())?;
            fix_word(
                dictionary,
                id,
                &text,
                language.as_deref(),
                &heard,
                &wanted,
                confirmed_common,
            )?
        };
        if let FixOutcome::Fixed { text } = &outcome {
            crate::runtime::with_history_mut(&app, |h| {
                h.replace_text(id, text, language.as_deref())
            })?;
        }
        Ok(outcome)
    }

    /// A retry from Home moved on (R-16): the page updates that row.
    #[derive(Debug, Clone, Serialize)]
    pub struct RetryEvent {
        pub id: DictationId,
        /// `working`, `done`, `failed` or `ended`.
        pub state: &'static str,
        pub text: Option<String>,
        pub message: Option<String>,
    }

    pub fn emit_retry(app: &AppHandle, event: RetryEvent) {
        let _ = app.emit_to(HOME, RETRY, event);
    }

    /// Tells an open Home that the history changed outside it (AC-15).
    pub fn emit_changed(app: &AppHandle) {
        let _ = app.emit_to(HOME, CHANGED, ());
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// AC-14 (R-24): an item sent to the page holds no audio path, size or key.
        #[test]
        fn item_view_has_no_audio_path_or_key() {
            let view = view(
                Item {
                    id: 3,
                    started_ms: 1000,
                    app: Some("Mail".into()),
                    language: Some("es".into()),
                    text: Some("Hola.".into()),
                    state: "done".into(),
                    audio_bytes: 4800,
                    has_audio: true,
                    message: None,
                },
                false,
            );
            let json = serde_json::to_value(&view).unwrap();
            let mut keys: Vec<&str> = json
                .as_object()
                .unwrap()
                .keys()
                .map(|k| k.as_str())
                .collect();
            keys.sort_unstable();
            assert_eq!(
                keys,
                [
                    "actions",
                    "app",
                    "has_audio",
                    "id",
                    "language",
                    "message",
                    "started_ms",
                    "state",
                    "text"
                ]
            );
            // No value carries a path; the actions' `key` is a keyboard shortcut, not a secret.
            let text = json.to_string();
            assert!(!text.contains(".wav") && !text.contains("audio/") && !text.contains("/Users"));
            let action_keys: Vec<&str> = json["actions"][0]
                .as_object()
                .unwrap()
                .keys()
                .map(|k| k.as_str())
                .collect();
            assert_eq!(action_keys, ["id", "key", "label"]);
        }

        /// R-4: a saved size and position comes back; a broken or too small one is ignored.
        #[test]
        fn geometry_round_trips_and_rejects_nonsense() {
            let g = Geometry {
                x: 120.0,
                y: 80.0,
                width: 1000.0,
                height: 700.0,
            };
            assert_eq!(
                Geometry::parse(&serde_json::to_string(&g).unwrap()),
                Some(g)
            );
            assert_eq!(Geometry::parse("not json"), None);
            assert_eq!(
                Geometry::parse(r#"{"x":0,"y":0,"width":100,"height":100}"#),
                None
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AC-4 (R-5, R-21): Home, History and, once a dictionary exists, Dictionary, in order, with
    /// their keys; every view is a sidebar item (V0.1 redesign, owner, 27 November 2026).
    #[test]
    fn sections_list_only_what_exists() {
        let ids = |list: Vec<Section>| -> Vec<(&str, &str)> {
            list.iter().map(|s| (s.id, s.shortcut)).collect()
        };
        assert_eq!(ids(sections(false)), [("home", "⌘1"), ("history", "⌘2")]);
        assert_eq!(
            ids(sections(true)),
            [("home", "⌘1"), ("history", "⌘2"), ("dictionary", "⌘3")]
        );
        // R-9: no Home block ships; the day is the Home page itself.
        assert!(blocks().is_empty());
    }

    /// AC-5 (R-6): a section id opens that section, a block id the Home page at that block, and
    /// anything else the Home page.
    #[test]
    fn targets_resolve() {
        let s = sections(false);
        let brief = [Block {
            id: "brief",
            title: "Brief",
        }];
        assert_eq!(resolve("history", &s, &blocks()), ("history", None));
        assert_eq!(resolve("home", &s, &blocks()), ("home", None));
        // The morning card's "Open Today": the day is the Home page.
        assert_eq!(resolve("today", &s, &blocks()), ("home", None));
        assert_eq!(resolve("brief", &s, &brief), ("home", Some("brief")));
        assert_eq!(resolve("dictionary", &s, &brief), ("home", None));
        assert_eq!(resolve("", &s, &brief), ("home", None));
        assert_eq!(hash(("home", Some("brief"))), "home/brief");
        assert_eq!(hash(("history", None)), "history");
    }

    /// AC-14 (R-24): the page keeps no dictation text in the web view's storage.
    #[test]
    fn the_page_uses_no_web_storage() {
        let page = include_str!("../../ui/home.html");
        for banned in ["localStorage", "sessionStorage", "indexedDB"] {
            assert!(!page.contains(banned), "home.html mentions {banned}");
        }
        // The page emits the event other modules listen for (R-22).
        assert!(page.contains("'home://block-viewed'"));
    }

    /// AC-2: rows are made only by the list that keeps the rows near the viewport, never for a
    /// whole page of items at once.
    #[test]
    fn rows_are_made_only_by_the_list() {
        let page = include_str!("../../ui/home.html");
        // The definition, and the one call inside the list's render.
        assert_eq!(page.matches("rowNode(").count(), 2);
        let render = &page[page.find("    render() {").unwrap()..];
        assert!(render.find("rowNode(").unwrap() < render.find("\n    node(id)").unwrap());
        assert!(page.contains("const OVERSCAN"));
    }

    // ---- Slice 4 ----

    use hyperme_core::traits::{ErrorKind, Refusal};
    use std::sync::{Arc, Mutex};

    fn safari() -> FrontApp {
        FrontApp {
            pid: 42,
            name: "Safari".to_owned(),
        }
    }

    /// AC-9 (R-15): paste only into the recorded app once it is in front; copy with a notice
    /// when it is gone or stays behind; never paste while Home itself is in front.
    #[test]
    fn paste_goes_only_to_the_recorded_app() {
        let own = 7;
        assert_eq!(
            paste_decision(Some(&safari()), own, Some(42)),
            PasteDecision::Paste
        );
        let gone = PasteDecision::Copy("Copied: Safari is no longer open".to_owned());
        assert_eq!(paste_decision(Some(&safari()), own, None), gone);
        assert_eq!(paste_decision(Some(&safari()), own, Some(own)), gone);
        assert_eq!(paste_decision(Some(&safari()), own, Some(99)), gone);
        let home = FrontApp {
            pid: own,
            name: "HyperMe".to_owned(),
        };
        assert!(matches!(
            paste_decision(Some(&home), own, Some(own)),
            PasteDecision::Copy(_)
        ));
        assert!(matches!(
            paste_decision(None, own, Some(42)),
            PasteDecision::Copy(_)
        ));
    }

    #[derive(Clone, Default)]
    struct FakeInserter(Arc<Mutex<Vec<String>>>);

    impl Inserter for FakeInserter {
        async fn paste(&mut self, text: &str) -> Result<(), Error> {
            self.0.lock().unwrap().push(format!("paste: {text}"));
            Ok(())
        }
        async fn copy(&mut self, text: &str) -> Result<(), Error> {
            self.0.lock().unwrap().push(format!("copy: {text}"));
            Ok(())
        }
    }

    /// A focuser that records who it was asked to bring forward, and which app ends in front.
    struct FakeFocus {
        asked: Arc<Mutex<Vec<i32>>>,
        ends_in_front: Option<i32>,
    }

    impl Focuser for FakeFocus {
        async fn bring_to_front(&self, pid: i32) -> Option<i32> {
            self.asked.lock().unwrap().push(pid);
            self.ends_in_front
        }
    }

    /// AC-9: Paste again sends the text to the inserter after focus goes back to the recorded
    /// app, and copies with a notice when that app is gone.
    #[test]
    fn paste_again_returns_to_the_app_then_pastes() {
        let block = tauri::async_runtime::block_on;
        let log = FakeInserter::default();
        let asked = Arc::new(Mutex::new(Vec::new()));
        let back = FakeFocus {
            asked: asked.clone(),
            ends_in_front: Some(42),
        };
        let app = safari();
        let mut inserter = log.clone();
        let outcome = block(paste_again(&mut inserter, &back, Some(&app), 7, "Hola.")).unwrap();
        assert!(outcome.pasted);
        assert_eq!(*asked.lock().unwrap(), [42]);
        assert_eq!(*log.0.lock().unwrap(), ["paste: Hola."]);

        let log = FakeInserter::default();
        let gone = FakeFocus {
            asked: Arc::default(),
            ends_in_front: None,
        };
        let mut inserter = log.clone();
        let outcome = block(paste_again(&mut inserter, &gone, Some(&app), 7, "Hola.")).unwrap();
        assert_eq!(
            outcome,
            PasteOutcome {
                pasted: false,
                notice: Some("Copied: Safari is no longer open".to_owned())
            }
        );
        assert_eq!(*log.0.lock().unwrap(), ["copy: Hola."]);
    }

    /// AC-10, AC-11 (R-13, R-16, R-21): Retry only with audio; Fix a word only with a
    /// dictionary; a failed row offers Retry and Delete.
    #[test]
    fn rows_offer_only_what_they_can_do() {
        let ids = |list: Vec<Action>| -> Vec<&str> { list.iter().map(|a| a.id).collect() };
        assert_eq!(
            ids(actions(true, true, false)),
            ["copy", "paste", "retry", "delete"]
        );
        assert_eq!(
            ids(actions(true, false, false)),
            ["copy", "paste", "delete"]
        );
        assert_eq!(
            ids(actions(true, true, true)),
            ["copy", "paste", "retry", "fix", "delete"]
        );
        assert_eq!(ids(actions(false, true, true)), ["retry", "delete"]);
        let keys: Vec<&str> = actions(true, true, true).iter().map(|a| a.key).collect();
        assert_eq!(keys, ["⌘C", "↵", "⌘R", "⌘F", "⌘⌫"]);
    }

    /// A dictionary that records corrections and answers from a queue (`Added` when empty).
    #[derive(Default)]
    struct FakeDictionary {
        corrections: Vec<Correction>,
        answers: Vec<Result<Added, Error>>,
    }

    impl Dictionary for FakeDictionary {
        fn add_correction(&mut self, c: Correction) -> Result<Added, Error> {
            self.corrections.push(c);
            if self.answers.is_empty() {
                Ok(Added::Added)
            } else {
                self.answers.remove(0)
            }
        }
    }

    /// AC-11 (R-17, R-20): the exact selection and typed word go to the dictionary;
    /// NeedsConfirm asks, and a confirm sends again with `confirmed_common`; success replaces the
    /// word in the row's text.
    #[test]
    fn fix_a_word_confirms_common_words() {
        let mut dict = FakeDictionary {
            answers: vec![Ok(Added::NeedsConfirm(Refusal::CommonWord))],
            ..Default::default()
        };
        let text = "Ask cloud about the cloud deploy.";
        let first = fix_word(&mut dict, 5, text, Some("en"), "cloud", "Claude", false);
        assert_eq!(
            first,
            Ok(FixOutcome::NeedsConfirm {
                heard: "cloud".to_owned()
            })
        );
        let second = fix_word(&mut dict, 5, text, Some("en"), "cloud", "Claude", true);
        assert_eq!(
            second,
            Ok(FixOutcome::Fixed {
                text: "Ask Claude about the Claude deploy.".to_owned()
            })
        );
        assert_eq!(dict.corrections.len(), 2);
        assert_eq!(dict.corrections[0].heard, "cloud");
        assert_eq!(dict.corrections[0].wanted, "Claude");
        assert_eq!(dict.corrections[0].dictation, Some(5));
        assert_eq!(dict.corrections[0].language.as_deref(), Some("en"));
        assert!(!dict.corrections[0].confirmed_common);
        assert!(dict.corrections[1].confirmed_common);
    }

    /// AC-11: a hard refusal shows its message and changes nothing; a selection that is not in
    /// the text never reaches the dictionary.
    #[test]
    fn fix_a_word_shows_refusals() {
        let mut dict = FakeDictionary {
            answers: vec![Err(Error {
                kind: ErrorKind::Other,
                message: "The dictionary is full.".to_owned(),
            })],
            ..Default::default()
        };
        assert_eq!(
            fix_word(&mut dict, 1, "Hola mundo.", None, "mundo", "Mundo", false),
            Err("The dictionary is full.".to_owned())
        );
        assert!(fix_word(&mut dict, 1, "Hola mundo.", None, "adiós", "Adiós", false).is_err());
        assert!(fix_word(&mut dict, 1, "Hola mundo.", None, "mundo", "  ", false).is_err());
        assert_eq!(dict.corrections.len(), 1);
    }

    /// AC-10 (R-16): the row shows working, then the new text or the error.
    #[test]
    fn retry_statuses_become_row_updates() {
        assert_eq!(
            retry_update(&Status::Transcribing),
            Some(RetryUpdate::Working)
        );
        assert_eq!(
            retry_update(&Status::Copied { raw: false }),
            Some(RetryUpdate::Done)
        );
        assert_eq!(
            retry_update(&Status::Failed {
                message: "Speech: connection reset".into(),
                retry: false
            }),
            Some(RetryUpdate::Failed("Speech: connection reset".into()))
        );
        assert!(matches!(
            retry_update(&Status::NothingHeard),
            Some(RetryUpdate::Failed(_))
        ));
        assert_eq!(retry_update(&Status::Idle), Some(RetryUpdate::Ended));
        assert_eq!(retry_update(&Status::Recording), None);
    }
}
