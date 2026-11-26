//! The Home window (spec 98; docs/UX.md, "Home" and "Home sections"): a normal Mac window with a
//! sidebar and a page, created when opened and destroyed when closed, so it costs no memory while
//! closed (R-2). While it is open HyperMe shows a Dock icon (R-3).
//!
//! The section list, the Home page's block list and the target resolution are pure and tested on
//! every platform; the window and its commands are macOS only, like Settings.

use serde::Serialize;

/// The Home window's label.
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

/// The sidebar's sections in order (R-5, AC-4). The Dictionary appears only while the app holds
/// a dictionary (R-21); Style and Meetings come with their own changes.
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

/// The Home page's blocks, in order (R-9). None ship in this change. The slots are for:
/// - daily tasks (#92): `waiting` ("Waiting for you", every proposal that needs a yes) and
///   `today` (the day's calendar and what is running), each shown only while it has something;
/// - the personal dictionary (#93): its suggestion line.
///
/// A block that shows is listed here; the page asks for the list on load and on
/// `home://refresh`.
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

#[cfg(target_os = "macos")]
pub use window::*;

#[cfg(target_os = "macos")]
mod window {
    use std::path::PathBuf;
    use std::sync::{Mutex, PoisonError};

    use hyperme_core::traits::FocusInfo;
    use hyperme_platform::history::{Item, Stats};
    use hyperme_platform::paste::macos::MacFocus;
    use serde::{Deserialize, Serialize};
    use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

    use super::{Block, HOME, Section, blocks, hash, resolve, sections};

    /// Sent to the page when the attention dot changes (R-22).
    pub const ATTENTION: &str = "home://attention";
    /// Sent to the page to reload a block or a section while Home is open (R-22).
    pub const REFRESH: &str = "home://refresh";
    /// The page emits this with a block's id once the block has been on screen (R-22). Other
    /// modules (#92's Waiting for you, for example) listen with `app.listen_any(BLOCK_VIEWED,
    /// ...)`; Home itself does nothing with it.
    #[allow(dead_code)] // A hook for #92 and #93; nothing listens yet.
    pub const BLOCK_VIEWED: &str = "home://block-viewed";

    const DEFAULT_SIZE: (f64, f64) = (900.0, 600.0);
    const MIN_SIZE: (f64, f64) = (720.0, 480.0);
    /// At most this many items per page or search call.
    const MAX_LIMIT: usize = 200;

    /// Home's state while the app runs.
    #[derive(Default)]
    pub struct HomeState {
        /// The violet dot on Home in the sidebar (R-22).
        attention: bool,
        /// The app in front before Home took focus, for Paste again (R-15, slice 4).
        front_app: Option<String>,
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

    /// The app that was in front before Home took focus (R-15).
    #[allow(dead_code)] // Paste again uses it in the next change (slice 4).
    pub fn front_app(app: &AppHandle) -> Option<String> {
        state(app).front_app.clone()
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
        let resolved = hash(resolve(target, &sections(false), &blocks()));
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
                let handle = app.clone();
                let win = window.clone();
                window.on_window_event(move |event| match event {
                    WindowEvent::CloseRequested { .. } => save_geometry(&handle, &win),
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
        let others = [crate::settings::SETTINGS, crate::walkthrough::WALKTHROUGH];
        if !others.iter().any(|w| app.get_webview_window(w).is_some()) {
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
        }
    }

    fn record_front_app(app: &AppHandle) {
        let front = MacFocus.frontmost_app();
        state(app).front_app = front;
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
    pub fn home_sections() -> Vec<Section> {
        // The app holds no dictionary yet (R-21); #93 brings it.
        sections(false)
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
    }

    impl From<Item> for ItemView {
        fn from(item: Item) -> Self {
            ItemView {
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

    fn views(items: Vec<Item>) -> Vec<ItemView> {
        items.into_iter().map(ItemView::from).collect()
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
        crate::runtime::with_history(&app, |h| h.page(before, limit.min(MAX_LIMIT))).map(views)
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
            .map(views)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// AC-14 (R-24): an item sent to the page holds no audio path, size or key.
        #[test]
        fn item_view_has_no_audio_path_or_key() {
            let view = ItemView::from(Item {
                id: 3,
                started_ms: 1000,
                app: Some("Mail".into()),
                language: Some("es".into()),
                text: Some("Hola.".into()),
                state: "done".into(),
                audio_bytes: 4800,
                has_audio: true,
                message: None,
            });
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
            let text = json.to_string();
            assert!(!text.contains(".wav") && !text.contains("audio/") && !text.contains("key"));
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

    /// AC-4 (R-5, R-21): only sections whose feature exists, in order, with their keys; no
    /// Dictionary while the app holds no dictionary.
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
        assert_eq!(sections(true)[2].page, "home-dictionary.html");
        assert!(blocks().is_empty(), "no blocks ship in this change (R-9)");
    }

    /// AC-5 (R-6): a section id opens that section, a block id the Home page at that block, and
    /// anything else the Home page.
    #[test]
    fn targets_resolve() {
        let s = sections(false);
        let today = [Block {
            id: "today",
            title: "Today",
        }];
        assert_eq!(resolve("history", &s, &today), ("history", None));
        assert_eq!(resolve("home", &s, &today), ("home", None));
        assert_eq!(resolve("today", &s, &today), ("home", Some("today")));
        assert_eq!(resolve("today", &s, &blocks()), ("home", None));
        assert_eq!(resolve("dictionary", &s, &today), ("home", None));
        assert_eq!(resolve("", &s, &today), ("home", None));
        assert_eq!(hash(("home", Some("today"))), "home/today");
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
}
