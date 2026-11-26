//! HyperMe: a menu-bar app with no Dock icon and no main window (spec R-22), and the pill, a
//! non-activating panel at the bottom centre that never takes focus and shows above full-screen
//! apps (R-23). Slice 1 is the shell only; the dictation is wired in later slices.

mod geometry;
mod icon;

use tauri::AppHandle;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;

/// The pill window's label.
const PILL: &str = "pill";

fn main() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());
    builder
        .setup(|app| {
            // R-22: a menu-bar app, no Dock icon.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            tray(app.handle())?;
            pill(app.handle())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("HyperMe could not start");
}

/// The menu-bar icon and its menu (docs/UX.md, "Menu-bar icon"). Paste last and Copy last come
/// with slice 10, Settings with slice 9.
fn tray(app: &AppHandle) -> tauri::Result<()> {
    let status = MenuItem::with_id(app, "status", "Ready", false, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit HyperMe", true, Some("CmdOrCtrl+Q"))?;
    let menu = Menu::with_items(app, &[&status, &PredefinedMenuItem::separator(app)?, &quit])?;
    TrayIconBuilder::with_id("hyperme")
        .icon(Image::new_owned(icon::waveform(), icon::SIZE, icon::SIZE))
        .icon_as_template(true)
        .tooltip("HyperMe")
        .menu(&menu)
        .on_menu_event(|app, event| {
            if event.id() == "quit" {
                app.exit(0);
            }
        })
        .build(app)?;
    Ok(())
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

    let (x, y) = match app.primary_monitor()? {
        Some(monitor) => {
            let scale = monitor.scale_factor();
            let area = monitor.work_area();
            let work = geometry::Rect {
                x: f64::from(area.position.x) / scale,
                y: f64::from(area.position.y) / scale,
                width: f64::from(area.size.width) / scale,
                height: f64::from(area.size.height) / scale,
            };
            geometry::bottom_centre(work, geometry::PILL_SIZE)
        }
        None => (0.0, 0.0),
    };
    let (width, height) = geometry::PILL_SIZE;
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
