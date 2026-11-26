//! HyperMe: a menu-bar app with no Dock icon and no main window (spec R-22), and the pill, a
//! non-activating panel at the bottom centre that never takes focus and shows above full-screen
//! apps (R-23). The dictation runs in `hyperme-core`'s state machine; this file wires the real
//! providers and devices to it and shows its states in the pill and the menu.

mod geometry;
mod icon;
mod pill;
#[cfg(target_os = "macos")]
mod runtime;
#[cfg(target_os = "macos")]
mod settings;

use tauri::AppHandle;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;

/// The pill window's label.
const PILL: &str = "pill";

fn main() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "macos")]
    let builder = builder
        .plugin(tauri_nspanel::init())
        .invoke_handler(tauri::generate_handler![
            runtime::pill_clicked,
            settings::settings_view,
            settings::settings_set,
            settings::key_set,
            settings::key_test,
            settings::settings_open,
            settings::app_restart,
        ]);
    builder
        .setup(|app| {
            // R-22: a menu-bar app, no Dock icon.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let menu = tray(app.handle())?;
            pill(app.handle())?;
            #[cfg(target_os = "macos")]
            runtime::start(app.handle(), menu);
            #[cfg(not(target_os = "macos"))]
            let _ = menu;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("HyperMe could not start");
}

/// The menu items whose text or state changes while the app runs.
pub(crate) struct TrayMenu {
    pub(crate) status: MenuItem<tauri::Wry>,
    pub(crate) retry: MenuItem<tauri::Wry>,
}

/// The menu-bar icon and its menu (docs/UX.md, "Menu-bar icon"): status, Paste last and Copy
/// last dictation (R-18), Retry (R-31), Settings, Quit.
fn tray(app: &AppHandle) -> tauri::Result<TrayMenu> {
    let status = MenuItem::with_id(app, "status", "Starting…", false, None::<&str>)?;
    let paste_last = MenuItem::with_id(
        app,
        "paste-last",
        "Paste last dictation",
        true,
        None::<&str>,
    )?;
    let copy_last = MenuItem::with_id(app, "copy-last", "Copy last dictation", true, None::<&str>)?;
    let retry = MenuItem::with_id(app, "retry", "Retry last dictation", false, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, Some("CmdOrCtrl+,"))?;
    let quit = MenuItem::with_id(app, "quit", "Quit HyperMe", true, Some("CmdOrCtrl+Q"))?;
    let separator = || PredefinedMenuItem::separator(app);
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &separator()?,
            &paste_last,
            &copy_last,
            &retry,
            &separator()?,
            &settings,
            &quit,
        ],
    )?;
    TrayIconBuilder::with_id("hyperme")
        .icon(Image::new_owned(icon::waveform(), icon::SIZE, icon::SIZE))
        .icon_as_template(true)
        .tooltip("HyperMe")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "quit" => app.exit(0),
            #[cfg(target_os = "macos")]
            id => runtime::menu_action(app, id),
            #[cfg(not(target_os = "macos"))]
            _ => {}
        })
        .build(app)?;
    Ok(TrayMenu { status, retry })
}

/// The pill: hidden while idle in Phase 1 (docs/UX.md, U-1).
#[cfg(target_os = "macos")]
fn pill(app: &AppHandle) -> tauri::Result<()> {
    use tauri::{LogicalPosition, LogicalSize, Position, Size, WebviewUrl};
    use tauri_nspanel::{CollectionBehavior, PanelBuilder, PanelLevel, StyleMask, tauri_panel};

    tauri_panel! {
        panel!(PillPanel {
            config: {
                // R-23: never key, never main, so typing stays in the app in front.
                can_become_key_window: false,
                can_become_main_window: false,
                is_floating_panel: true
            }
        })
    }

    let (width, height) = geometry::PILL_SIZE;
    let (x, y) = runtime::pill_origin(app, width);
    let panel = PanelBuilder::<_, PillPanel>::new(app, PILL)
        .url(WebviewUrl::App("pill.html".into()))
        .size(Size::Logical(LogicalSize::new(width, height)))
        .position(Position::Logical(LogicalPosition::new(x, y)))
        // Above full-screen apps (R-23).
        .level(PanelLevel::Status)
        // No title bar from Tauri; then the panel flag that keeps it from activating the app.
        .with_window(|window| window.decorations(false).skip_taskbar(true))
        .add_style_mask(StyleMask::empty().nonactivating_panel())
        .collection_behavior(
            CollectionBehavior::new()
                .can_join_all_spaces()
                .full_screen_auxiliary()
                .stationary()
                .ignores_cycle(),
        )
        .no_activate(true)
        .becomes_key_only_if_needed(true)
        .hides_on_deactivate(false)
        .transparent(true)
        .opaque(false)
        .has_shadow(false)
        .build()?;
    panel.hide();
    if std::env::var_os("HYPERME_SHOW_PILL").is_some() {
        // For checking the shape by eye during development.
        panel.show();
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn pill(_app: &AppHandle) -> tauri::Result<()> {
    // Linux and Windows follow macOS (CLAUDE.md); their pill is not built yet.
    Ok(())
}
