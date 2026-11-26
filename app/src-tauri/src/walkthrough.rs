//! The first-launch walkthrough window (R-27, docs/UX.md "Onboarding"). It opens at launch when
//! a permission or a key is missing; each screen runs its own test before Next lights up
//! (rules in `onboarding.rs`).

use std::sync::Mutex;

use hyperme_core::config::Config;
use hyperme_core::dictation::Event;
use hyperme_core::secrets::SecretStore;
use hyperme_core::traits::Microphone;
use hyperme_platform::keychain::KeychainStore;
use hyperme_platform::microphone::CpalMicrophone;
use hyperme_platform::permissions;
use hyperme_providers::speechmatics::SAMPLE_RATE;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::mpsc;

use crate::onboarding::{self, AtLaunch};
use hyperme_core::cost;

/// The walkthrough window's label.
pub const WALKTHROUGH: &str = "walkthrough";

/// The microphone test's capture, while the Microphone screen is open.
#[derive(Default)]
struct MicTest(Mutex<Option<CpalMicrophone>>);

fn at_launch(app: &AppHandle) -> AtLaunch {
    let config = Config::load_or_create(&crate::runtime::data_dir(app).join("config.toml")).ok();
    let keys = KeychainStore::new();
    let saved =
        |provider: Option<&str>| provider.is_some_and(|p| matches!(keys.get(p), Ok(Some(_))));
    AtLaunch {
        accessibility: permissions::accessibility(),
        input_monitoring: input_monitoring(),
        speech_key_saved: saved(config.as_ref().map(|c| c.speech.provider.as_str())),
        cleanup_key_saved: saved(config.as_ref().map(|c| c.cleanup.provider.as_str())),
    }
}

/// Opens the walkthrough at launch when something it sets up is missing.
pub fn open_if_needed(app: &AppHandle) {
    app.manage(MicTest::default());
    // scripts/measure-idle.sh sets this so a baseline measures the idle app, not the walkthrough.
    if std::env::var_os("HYPERME_NO_WALKTHROUGH").is_some() {
        return;
    }
    // After "Relaunch HyperMe", come back on the same screen.
    let resume = std::fs::read_to_string(resume_file(app))
        .ok()
        .and_then(|name| onboarding::Step::from_name(&name));
    let _ = std::fs::remove_file(resume_file(app));
    if resume.is_some() || onboarding::should_open(&at_launch(app)) {
        open(app, resume.unwrap_or(onboarding::Step::Welcome));
    }
}

/// The app was opened again while running: the walkthrough while setup is unfinished,
/// otherwise Settings.
pub fn reopen(app: &AppHandle) {
    if onboarding::should_open(&at_launch(app)) {
        open(app, onboarding::Step::Welcome);
    } else {
        crate::settings::open(app, "setup");
    }
}

pub fn open(app: &AppHandle, step: onboarding::Step) {
    if let Some(window) = app.get_webview_window(WALKTHROUGH) {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let url = format!("onboarding.html#{}", step.name());
    let built = WebviewWindowBuilder::new(app, WALKTHROUGH, WebviewUrl::App(url.into()))
        .title("Welcome to HyperMe")
        .inner_size(520.0, 440.0)
        .resizable(false)
        .build();
    if let Ok(window) = built {
        let _ = window.set_focus();
    }
}

#[derive(Debug, Serialize)]
pub struct Permissions {
    accessibility: bool,
    input_monitoring: bool,
    /// The terminal macOS gives the permissions to while HyperMe runs as a bare binary; `None`
    /// for HyperMe.app.
    host_app: Option<String>,
    /// Running as HyperMe.app, so "Relaunch HyperMe" can reopen it.
    bundled: bool,
    speech_provider: String,
    cleanup_provider: String,
}

/// The permission checks, polled by the Permissions screen so it turns green by itself.
#[tauri::command]
pub fn onboarding_checks(app: AppHandle) -> Permissions {
    log_permissions();
    let config = Config::load_or_create(&crate::runtime::data_dir(&app).join("config.toml")).ok();
    Permissions {
        accessibility: permissions::accessibility(),
        input_monitoring: input_monitoring(),
        host_app: permissions::host_app(),
        bundled: onboarding::bundle_of(&std::env::current_exe().unwrap_or_default()).is_some(),
        speech_provider: config
            .as_ref()
            .map_or_else(String::new, |c| c.speech.provider.clone()),
        cleanup_provider: config
            .as_ref()
            .map_or_else(String::new, |c| c.cleanup.provider.clone()),
    }
}

/// Starts or stops the microphone test. While on, the level goes to the window as
/// `onboarding-level`, so the user sees the microphone work. Starting it is also what makes
/// macOS ask for the microphone permission.
#[tauri::command]
pub fn onboarding_mic(app: AppHandle, on: bool) -> Result<(), String> {
    let state = app.state::<MicTest>();
    let mut slot = state
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(mut mic) = slot.take() {
        mic.stop();
    }
    if !on {
        return Ok(());
    }
    let (tx, mut rx) = mpsc::channel(256);
    let mut mic = CpalMicrophone::new(tx, SAMPLE_RATE);
    mic.start().map_err(|e| e.message)?;
    *slot = Some(mic);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // Ends when the test microphone is dropped and its sender with it.
        while let Some(event) = rx.recv().await {
            if let Event::Audio(frame) = event {
                let level = crate::pill::level(&frame);
                let _ = app.emit_to(
                    WALKTHROUGH,
                    "onboarding-level",
                    (level, level >= onboarding::HEARD_LEVEL),
                );
            }
        }
    });
    Ok(())
}

/// Appends the raw permission facts to `~/Library/Logs/HyperMe/permissions.log` whenever they
/// change, so a grant that does not show up can be diagnosed. Facts only, no user data.
fn log_permissions() {
    use std::io::Write;
    static LAST: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
    let line = permissions::diagnostics();
    let mut last = LAST
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if *last == line {
        return;
    }
    last.clone_from(&line);
    let path = std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join("Library/Logs/HyperMe/permissions.log");
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let _ = writeln!(file, "{now} {line}");
    }
}

/// Input Monitoring as the walkthrough sees it: the preflight check, or a tap that can be
/// created, since the preflight can lag behind a fresh grant.
pub fn input_monitoring() -> bool {
    permissions::input_monitoring() || hyperme_platform::hotkey::macos::can_listen()
}

/// Asks macOS for a permission. The first click (`prompt`) shows macOS's own dialog, which adds
/// the app's row to System Settings and has its own "Open System Settings" button, so the page
/// is not opened as well. Later clicks open the System Settings page directly. The page polls
/// only the checks that never prompt, so the dialog never comes back by itself.
#[tauri::command]
pub fn onboarding_request(app: AppHandle, what: String, prompt: bool) -> Result<(), String> {
    if prompt {
        let allowed = match what.as_str() {
            "accessibility" => permissions::request_accessibility(),
            "input-monitoring" => permissions::request_input_monitoring(),
            _ => false,
        };
        if allowed || what == "accessibility" || what == "input-monitoring" {
            return Ok(());
        }
    }
    crate::settings::settings_open(app, what)
}

/// The file that tells a relaunched HyperMe which walkthrough screen to come back to.
fn resume_file(app: &AppHandle) -> std::path::PathBuf {
    crate::runtime::data_dir(app).join("walkthrough-resume")
}

/// "Relaunch HyperMe": some grants (Input Monitoring above all) only reach the app after a
/// restart. Notes the screen, starts a fresh copy of HyperMe.app a moment after this one quits,
/// and quits.
#[tauri::command]
pub fn onboarding_relaunch(app: AppHandle, step: String) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let bundle = onboarding::bundle_of(&exe).ok_or_else(|| {
        "HyperMe is running from a terminal: quit that terminal app (⌘Q), open it again and start HyperMe again".to_owned()
    })?;
    if let Some(step) = onboarding::Step::from_name(&step) {
        let path = resume_file(&app);
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(&path, step.name()).map_err(|e| e.to_string())?;
    }
    // A detached shell waits until this process has gone, then reopens the bundle.
    std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg("sleep 1; /usr/bin/open -n \"$0\"")
        .arg(&bundle)
        .spawn()
        .map_err(|e| e.to_string())?;
    app.exit(0);
    Ok(())
}

/// The Try it screen: starts the dictation with the keys just tested.
#[tauri::command]
pub fn onboarding_try(app: AppHandle) -> Result<(), String> {
    crate::runtime::ensure_running(&app)
}

/// One choice of the cost block (U-11).
#[derive(Debug, Serialize)]
pub struct CostChoice {
    label: &'static str,
    detail: &'static str,
    shown_as: String,
    speech_usd: f64,
    cleanup_usd: f64,
}

/// What the keys screen shows about the recommended setup: the provider names, the cost for
/// each choice and the free-credit line, all worked out from the config (docs/UX.md, "Models
/// and keys in onboarding").
#[derive(Debug, Serialize)]
pub struct KeysScreen {
    speech: String,
    cleanup: String,
    choices: Vec<CostChoice>,
    free_credit: Option<String>,
}

#[tauri::command]
pub fn onboarding_keys_screen(app: AppHandle) -> Result<KeysScreen, String> {
    let config = Config::load_or_create(&crate::runtime::data_dir(&app).join("config.toml"))
        .map_err(|e| e.to_string())?;
    let choices = cost::CHOICES
        .iter()
        .map(|choice| {
            let c = cost::monthly_cost(&config, choice.minutes_per_day);
            CostChoice {
                label: choice.label,
                detail: choice.detail,
                shown_as: cost::shown_as(c.total_usd()),
                speech_usd: c.speech_usd,
                cleanup_usd: c.cleanup_usd,
            }
        })
        .collect();
    Ok(KeysScreen {
        speech: cost::display_name(&config.speech.provider),
        cleanup: cost::display_name(&config.cleanup.provider),
        choices,
        free_credit: cost::free_credit_line(&config),
    })
}

/// "Get a key": opens the provider's key page from the config in the browser.
#[tauri::command]
pub fn onboarding_get_key(app: AppHandle, role: String) -> Result<(), String> {
    let config = Config::load_or_create(&crate::runtime::data_dir(&app).join("config.toml"))
        .map_err(|e| e.to_string())?;
    let url = match role.as_str() {
        "speech" => config.speech.key_url(),
        "cleanup" => config.cleanup.key_url(),
        other => return Err(format!("{other} is not a provider role")),
    };
    if !url.starts_with("https") {
        return Err("the key page in the config is not an https address".to_owned());
    }
    std::process::Command::new("open")
        .arg(url)
        .status()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// U-12: "Use other providers" opens Settings → Providers with Advanced showing; the wizard
/// waits and tests again whenever Settings saves a provider or a key.
#[tauri::command]
pub fn onboarding_other_providers(app: AppHandle) {
    crate::settings::open(&app, "providers-advanced");
}

/// Whether Next may light up on `step`, given what the page's tests found (rules in
/// `onboarding.rs`, so the page cannot skip a test).
#[tauri::command]
pub fn onboarding_can_continue(step: onboarding::Step, checks: onboarding::Checks) -> bool {
    onboarding::can_continue(step, &checks)
}

#[tauri::command]
pub fn onboarding_done(app: AppHandle) {
    let _ = onboarding_mic(app.clone(), false);
    if let Some(window) = app.get_webview_window(WALKTHROUGH) {
        let _ = window.close();
    }
}
