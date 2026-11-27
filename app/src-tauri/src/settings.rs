//! The Settings window (docs/UX.md, "Settings window"; spec R-24, R-25): a normal window opened
//! from the menu, created when opened and dropped when closed, so it costs no memory while
//! closed. Every change saves at once to `config.toml`; keys go straight to the keychain and are
//! never sent back to the page.

use std::path::{Path, PathBuf};
use std::time::Instant;

use hyperme_core::config::{self, Config, ConfigError, Hotkey, Retention, Role};
use hyperme_core::secrets::SecretStore;
use hyperme_core::traits::SpeechProvider;
use hyperme_platform::keychain::KeychainStore;
use hyperme_platform::permissions;
use hyperme_providers::any;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// The Settings window's label.
pub const SETTINGS: &str = "settings";

fn config_path(app: &AppHandle) -> PathBuf {
    crate::runtime::data_dir(app).join("config.toml")
}

/// The sheet's size when Home has room for it (V0.1 redesign).
const SHEET: (f64, f64) = (820.0, 580.0);
/// The smallest sheet; Home's own minimum is 720 × 480.
const SHEET_MIN: (f64, f64) = (640.0, 440.0);

/// Where the Settings sheet sits over Home, in logical points: centred across, 28 pt below
/// Home's top edge (under its title bar), and never larger than Home less a margin.
pub fn sheet_frame(home_pos: (f64, f64), home_size: (f64, f64)) -> (f64, f64, f64, f64) {
    let w = SHEET.0.min(home_size.0 - 40.0).max(SHEET_MIN.0);
    let h = SHEET.1.min(home_size.1 - 56.0).max(SHEET_MIN.1);
    let x = home_pos.0 + (home_size.0 - w) / 2.0;
    let y = home_pos.1 + 28.0;
    (x, y, w, h)
}

/// Opens Settings at `section` (`providers`, for example) as a sheet over the Home window, as in
/// Wispr Flow (V0.1 redesign, owner, 27 November 2026): Home opens first if it is closed, and the
/// sheet is a borderless child window that moves with Home and closes with it. The menu, the
/// walkthrough, Gmail approvals and Home itself all come here.
pub fn open(app: &AppHandle, section: &str) {
    if let Some(window) = app.get_webview_window(SETTINGS) {
        let _ = window.eval(format!("location.hash = {}", serde_json::json!(section)));
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    if app.get_webview_window(crate::home::HOME).is_none() {
        crate::home::open(app, "home");
    }
    let Some(home) = app.get_webview_window(crate::home::HOME) else {
        return;
    };
    let scale = home.scale_factor().unwrap_or(1.0);
    let (Ok(pos), Ok(size)) = (home.outer_position(), home.outer_size()) else {
        return;
    };
    let (pos, size) = (pos.to_logical::<f64>(scale), size.to_logical::<f64>(scale));
    let (x, y, w, h) = sheet_frame((pos.x, pos.y), (size.width, size.height));
    let url = format!("settings.html#{section}");
    let Ok(builder) = WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App(url.into()))
        .title("HyperMe Settings")
        .inner_size(w, h)
        .position(x, y)
        .decorations(false)
        .resizable(false)
        .shadow(true)
        .parent(&home)
    else {
        return;
    };
    if let Ok(window) = builder.build() {
        let _ = home.eval("document.body.classList.add('sheet-open')");
        // Its page process must end when it closes, or it stays in memory while idle.
        let handle = app.clone();
        window.on_window_event(move |event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                // An approval card left open answers nothing (specs/166, R-9).
                crate::approvals::window_closed();
                if let Some(home) = handle.get_webview_window(crate::home::HOME) {
                    let _ = home.eval("document.body.classList.remove('sheet-open')");
                }
                crate::home::free_closed_window(&handle);
            }
        });
        let _ = window.set_focus();
    }
}

/// Closes the Settings sheet: its × button, Esc, or a click on Home's dimmed page.
#[tauri::command]
pub fn settings_close(app: AppHandle) {
    if let Some(window) = app.get_webview_window(SETTINGS) {
        let _ = window.close();
    }
}

/// What the page shows. Never holds a key: only whether one is set.
#[derive(Debug, Serialize)]
pub struct View {
    config: Option<ConfigView>,
    /// Why the config could not be read, if it could not.
    config_error: Option<String>,
    config_path: String,
    keys: KeysView,
    /// Settings → Providers' table of roles (specs/96-provider-settings, R-6).
    roles: Vec<RoleView>,
    /// Every provider dictation may send to, fallbacks included, for Privacy (R-29).
    destinations: Vec<config::Destination>,
    setup: SetupView,
    version: &'static str,
}

/// One row of the providers table: a role, what it uses, and what it could use.
#[derive(Debug, Serialize)]
struct RoleView {
    role: &'static str,
    label: &'static str,
    provider: String,
    model: String,
    /// Cleanup only.
    reasoning_effort: Option<String>,
    /// The providers that can fill the role, recommended first (R-7).
    providers: Vec<ProviderView>,
    /// The role's fallback, when one is set (R-24). `ready` is false when its key is missing,
    /// which turns its row red in the Setup check (R-24a).
    fallback: Option<FallbackView>,
}

#[derive(Debug, Serialize)]
struct FallbackView {
    provider: String,
    name: String,
    model: String,
    ready: bool,
}

/// A provider in a role's menu. Never holds a key: only whether one is set.
#[derive(Debug, Serialize)]
struct ProviderView {
    id: String,
    name: String,
    recommended: bool,
    key_required: bool,
    has_key: bool,
    has_key_page: bool,
    /// One line beside the key field while there is no key, from the catalogue.
    key_note: Option<String>,
    models: Vec<ModelView>,
}

#[derive(Debug, Serialize)]
struct ModelView {
    id: String,
    reasoning_effort: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ConfigView {
    hotkey: String,
    min_hold_ms: u64,
    max_length_s: u64,
    short_dictation_words: u32,
    sounds: bool,
    show_while_idle: bool,
    speech_provider: String,
    speech_model: String,
    speech_endpoint: String,
    cleanup_provider: String,
    cleanup_model: String,
    cleanup_endpoint: String,
    reasoning_effort: String,
    deadline_ms: u64,
    history: bool,
    retention_days: Option<u32>,
    size_limit_mb: u64,
    keep_traces: bool,
    timing_log: bool,
}

#[derive(Debug, Serialize)]
struct KeysView {
    speech: bool,
    cleanup: bool,
}

#[derive(Debug, Serialize)]
struct SetupView {
    accessibility: bool,
    input_monitoring: bool,
    /// The terminal that holds the permissions while HyperMe runs as a bare binary.
    host_app: Option<String>,
}

fn hotkey_text(hotkey: &Hotkey) -> String {
    match hotkey {
        Hotkey::Fn => "fn".to_owned(),
        Hotkey::RightOption => "right_option".to_owned(),
        Hotkey::Chord { modifiers, key } => {
            let mut parts: Vec<String> = modifiers
                .iter()
                .map(|m| format!("{m:?}").to_lowercase())
                .collect();
            parts.push(key.clone());
            parts.join("+")
        }
    }
}

fn config_view(config: &Config) -> ConfigView {
    ConfigView {
        hotkey: hotkey_text(&config.dictation.hotkey),
        min_hold_ms: config.dictation.min_hold_ms,
        max_length_s: config.dictation.max_length_s,
        short_dictation_words: config.dictation.short_dictation_words,
        sounds: config.dictation.sounds,
        show_while_idle: config.dictation.show_while_idle,
        speech_provider: config.speech.provider.clone(),
        speech_model: config.speech.model.clone(),
        speech_endpoint: config.speech.endpoint.clone(),
        cleanup_provider: config.cleanup.provider.clone(),
        cleanup_model: config.cleanup.model.clone(),
        cleanup_endpoint: config.cleanup.endpoint.clone(),
        reasoning_effort: config.cleanup.reasoning_effort.clone(),
        deadline_ms: config.cleanup.deadline_ms,
        history: config.history.enabled,
        retention_days: match config.history.retention_days {
            Retention::Days(days) => Some(days),
            Retention::Unlimited => None,
        },
        size_limit_mb: config.history.size_limit_mb,
        keep_traces: config.history.keep_traces,
        timing_log: config.timing.log,
    }
}

fn roles_view(config: &Config, keys: &dyn SecretStore) -> Vec<RoleView> {
    [
        (Role::Speech, "Dictation speech"),
        (Role::Cleanup, "Dictation cleanup"),
    ]
    .into_iter()
    .map(|(role, label)| {
        let (provider, model, reasoning_effort) = match role {
            Role::Speech => (&config.speech.provider, &config.speech.model, None),
            Role::Cleanup => (
                &config.cleanup.provider,
                &config.cleanup.model,
                Some(config.cleanup.reasoning_effort.clone()),
            ),
        };
        let recommended = Config::recommended(role).0;
        let fallback = match role {
            Role::Cleanup => config.cleanup.fallback.as_ref(),
            Role::Speech => config.speech.fallback.as_ref(),
        }
        .map(|f| FallbackView {
            provider: f.provider.clone(),
            name: config
                .provider(&f.provider)
                .map_or_else(|| f.provider.clone(), |p| p.name.clone()),
            model: f.model.clone(),
            ready: any::fallback_ready(config, role, keys).unwrap_or(false),
        });
        RoleView {
            fallback,
            role: role.section(),
            label,
            provider: provider.clone(),
            model: model.clone(),
            reasoning_effort,
            providers: config
                .providers_for(role)
                .map(|(id, entry)| ProviderView {
                    id: id.to_owned(),
                    name: entry.name.clone(),
                    recommended: id == recommended,
                    key_required: entry.key_required,
                    has_key: matches!(keys.get(config.key_account(id)), Ok(Some(_))),
                    has_key_page: entry.key_url.is_some(),
                    key_note: entry.key_note.clone(),
                    models: entry
                        .models
                        .iter()
                        .map(|m| ModelView {
                            id: m.id.clone(),
                            reasoning_effort: m.reasoning_effort.clone(),
                        })
                        .collect(),
                })
                .collect(),
        }
    })
    .collect()
}

fn load(path: &Path) -> Result<Config, String> {
    Config::load_or_create(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn settings_view(app: AppHandle) -> View {
    let path = config_path(&app);
    let loaded = load(&path);
    let keys = KeychainStore::new();
    let config = loaded.as_ref().ok();
    // A provider that shares another's key (D-2) reads that one.
    let has_key = |provider: Option<&str>| {
        provider.is_some_and(|p| {
            let account = config.map_or(p, |c| c.key_account(p));
            matches!(keys.get(account), Ok(Some(_)))
        })
    };
    View {
        keys: KeysView {
            speech: has_key(config.map(|c| c.speech.provider.as_str())),
            cleanup: has_key(config.map(|c| c.cleanup.provider.as_str())),
        },
        roles: config.map_or_else(Vec::new, |c| roles_view(c, &keys)),
        destinations: config.map_or_else(Vec::new, Config::destinations),
        config: config.map(config_view),
        config_error: loaded.err(),
        config_path: path.display().to_string(),
        setup: SetupView {
            accessibility: permissions::accessibility(),
            input_monitoring: crate::walkthrough::input_monitoring(),
            host_app: permissions::host_app(),
        },
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// A JSON value from the page as a TOML value: `true`, `2000` or a quoted string.
fn toml_value(value: &Value) -> Result<String, String> {
    match value {
        Value::Bool(b) => Ok(b.to_string()),
        Value::Number(n) if n.is_u64() => Ok(n.to_string()),
        Value::String(s) => Ok(toml_string(s)),
        other => Err(format!("{other} is not a setting value")),
    }
}

/// A TOML basic string, escaped.
fn toml_string(s: &str) -> String {
    let mut out = String::from('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Saves one setting. A provider or model change reaches the next dictation at once (specs/96-
/// provider-settings, R-8); other changes take effect after a restart, which the page offers.
#[tauri::command]
pub fn settings_set(
    app: AppHandle,
    section: String,
    key: String,
    value: Value,
) -> Result<(), String> {
    let value = toml_value(&value)?;
    rewrite_config(&app, |text| config::set_value(text, &section, &key, &value))?;
    if section == "speech" || section == "cleanup" {
        use_new_providers(&app);
    }
    if section == "history" {
        // Turning traces off, or a new retention, applies now (spec 182, R-9a).
        crate::runtime::refresh_traces(&app);
    }
    Ok(())
}

/// Rewrites `config.toml` with `change`, which validates the result. Writes beside the file,
/// then renames, so a crash never leaves half a config.
fn rewrite_config(
    app: &AppHandle,
    change: impl FnOnce(&str) -> Result<String, ConfigError>,
) -> Result<(), String> {
    let path = config_path(app);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let updated = change(&text).map_err(|e| e.to_string())?;
    let temp = path.with_extension("toml.new");
    std::fs::write(&temp, updated).map_err(|e| format!("{}: {e}", temp.display()))?;
    std::fs::rename(&temp, &path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Hands the providers now in the config to the next dictation (R-8), and tells a waiting
/// walkthrough. A provider that cannot be built yet (no key) leaves the one in use (R-12).
fn use_new_providers(app: &AppHandle) {
    providers_changed(app);
    if let Err(why) = crate::runtime::reload_providers(app) {
        tracing::warn!("providers unchanged: {why}");
    }
}

fn role_named(role: &str) -> Result<Role, String> {
    match role {
        "speech" => Ok(Role::Speech),
        "cleanup" => Ok(Role::Cleanup),
        other => Err(format!("{other} is not a provider role")),
    }
}

/// Picks a role's provider and model from the providers table (R-8). A provider with no key yet
/// is not saved: the page shows the key field, and the role keeps the provider it has until the
/// new one's key passes Test (R-12). Returns `saved` or `needs_key`.
#[tauri::command]
pub fn provider_choose(
    app: AppHandle,
    role: String,
    provider: String,
    model: String,
) -> Result<&'static str, String> {
    let role = role_named(&role)?;
    let config = load(&config_path(&app))?;
    if !any::key_ready(&config, &provider, &KeychainStore::new()) {
        return Ok("needs_key");
    }
    rewrite_config(&app, |text| {
        config::choose_provider(text, role, &provider, &model)
    })?;
    use_new_providers(&app);
    Ok("saved")
}

/// Sets a role's fallback (R-24a): tests the provider and model first, with the key it has,
/// and saves only when Test passes. Returns the time Test took in ms.
#[tauri::command]
pub async fn fallback_set(
    app: AppHandle,
    role: String,
    provider: String,
    model: String,
) -> Result<u64, String> {
    let role = role_named(&role)?;
    let path = config_path(&app);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let candidate =
        config::set_fallback(&text, role, Some((&provider, &model))).map_err(|e| e.to_string())?;
    let config = Config::parse(&candidate, &path).map_err(|e| e.to_string())?;
    let keys = KeychainStore::new();
    let start = Instant::now();
    match role {
        Role::Cleanup => any::cleanup_fallback(&config, &keys)
            .map_err(|e| e.message)?
            .check()
            .await
            .map_err(|e| e.message)?,
        Role::Speech => {
            // As Test does for speech: a session with the model opens, and no audio is sent.
            let session = any::speech_fallback(&config, &keys)
                .map_err(|e| e.message)?
                .open()
                .await
                .map_err(|e| e.message)?;
            drop(session);
        }
    }
    let ms = start.elapsed().as_millis() as u64;
    rewrite_config(&app, |text| {
        config::set_fallback(text, role, Some((&provider, &model)))
    })?;
    use_new_providers(&app);
    Ok(ms)
}

/// Removes a role's fallback.
#[tauri::command]
pub fn fallback_clear(app: AppHandle, role: String) -> Result<(), String> {
    let role = role_named(&role)?;
    rewrite_config(&app, |text| config::set_fallback(text, role, None))?;
    use_new_providers(&app);
    Ok(())
}

/// "Reset to recommended": every role back to the recommended pair; keys stay (R-10).
#[tauri::command]
pub fn providers_reset(app: AppHandle) -> Result<(), String> {
    rewrite_config(&app, config::reset_to_recommended)?;
    use_new_providers(&app);
    Ok(())
}

/// "Add a custom endpoint…": a new OpenAI-compatible cleanup provider in `config.toml`.
/// Returns its name in the catalogue. Nothing is sent to it until Test or a dictation (R-30).
#[tauri::command]
pub fn provider_add(app: AppHandle, name: String, endpoint: String) -> Result<String, String> {
    let mut id = String::new();
    rewrite_config(&app, |text| {
        let (updated, added) = config::add_provider(text, &name, &endpoint)?;
        id = added;
        Ok(updated)
    })?;
    Ok(id)
}

/// A server found on this Mac, for the page (R-22).
#[derive(Debug, Clone, Serialize)]
pub struct LocalServerView {
    port: u16,
    endpoint: String,
    models: Vec<String>,
}

/// When the user picks Local server: looks on the usual ports on this Mac, all at once, and
/// returns the servers that answer with their models (R-22). Nothing leaves the Mac.
#[tauri::command]
pub async fn local_discover(app: AppHandle) -> Result<Vec<LocalServerView>, String> {
    let config = load(&config_path(&app))?;
    let ports = config
        .provider("local")
        .map(|p| p.ports.clone())
        .unwrap_or_default();
    Ok(hyperme_providers::local::discover(&ports)
        .await
        .into_iter()
        .map(|found| LocalServerView {
            port: found.port,
            endpoint: found.endpoint,
            models: found.models,
        })
        .collect())
}

/// Uses the local server at `endpoint`, found or typed, with `model` for cleanup (R-21, R-22).
/// It needs no key, so the switch takes effect at once.
#[tauri::command]
pub fn local_use(app: AppHandle, endpoint: String, model: String) -> Result<(), String> {
    let endpoint = endpoint.trim().trim_end_matches('/').to_owned();
    rewrite_config(&app, |text| {
        let chosen = config::choose_provider(text, Role::Cleanup, "local", &model)?;
        config::set_value(&chosen, "cleanup", "endpoint", &toml_string(&endpoint))
    })?;
    model_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove("local");
    use_new_providers(&app);
    Ok(())
}

/// A model menu for the page: the models, and one line when the listing failed.
#[derive(Debug, Clone, Serialize)]
pub struct ModelMenuView {
    models: Vec<String>,
    notice: Option<String>,
}

/// Model menus fetched this session, by provider (R-15). Only full lists are kept; a new key
/// clears them.
fn model_cache() -> &'static std::sync::Mutex<std::collections::HashMap<String, ModelMenuView>> {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, ModelMenuView>>,
    > = std::sync::OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// The model menu of `provider`, fetched when the page opens the menu, never on the dictation
/// path (R-15 to R-17).
#[tauri::command]
pub async fn models_list(app: AppHandle, provider: String) -> Result<ModelMenuView, String> {
    let cached = model_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&provider)
        .cloned();
    if let Some(menu) = cached {
        return Ok(menu);
    }
    let config = load(&config_path(&app))?;
    let menu =
        hyperme_providers::models::model_menu(&config, &provider, &KeychainStore::new()).await;
    let view = ModelMenuView {
        models: menu.models,
        notice: menu.notice,
    };
    if view.notice.is_none() {
        model_cache()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(provider, view.clone());
    }
    Ok(view)
}

/// The hint line under a role (R-18 to R-20).
#[derive(Debug, Serialize)]
pub struct HintView {
    cost: String,
    speed: String,
    /// Under `docs/results/`, opened with `result_open`.
    source: Option<String>,
    mine: Option<String>,
}

/// The hint for `provider` and `model` in `role`: cost for the user's talking time, measured
/// speed with its source, and, with the timing log on, the user's own median on that provider.
#[tauri::command]
pub fn provider_hint(
    app: AppHandle,
    role: String,
    provider: String,
    model: String,
) -> Result<HintView, String> {
    let role = role_named(&role)?;
    let config = load(&config_path(&app))?;
    let own = if config.timing.log {
        let column = match role {
            Role::Speech => "speech_provider",
            Role::Cleanup => "cleanup_provider",
        };
        std::fs::read_to_string(crate::runtime::timing_path())
            .ok()
            .and_then(|text| hyperme_platform::timing::own_median(&text, column, &provider, 50))
    } else {
        None
    };
    let hint = hyperme_core::hints::hint(
        &config,
        role,
        &provider,
        &model,
        config.dictation.talking_minutes_per_day,
        own,
    );
    Ok(HintView {
        cost: hint.cost,
        speed: hint.speed,
        source: hint.source,
        mine: hint.mine,
    })
}

/// Opens a measurement the hint line cites, from the project's published results.
#[tauri::command]
pub fn result_open(source: String) -> Result<(), String> {
    let name = source
        .strip_prefix("docs/results/")
        .filter(|n| {
            !n.is_empty()
                && n.ends_with(".md")
                && n.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        })
        .ok_or_else(|| format!("{source} is not a result file"))?;
    std::process::Command::new("open")
        .arg(format!(
            "https://github.com/AndresHG/hyperme/blob/master/docs/results/{name}"
        ))
        .status()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// "Get a key" for any provider in the catalogue: opens its key page in the browser.
#[tauri::command]
pub fn provider_key_page(app: AppHandle, provider: String) -> Result<(), String> {
    let config = load(&config_path(&app))?;
    let url = config
        .provider(&provider)
        .and_then(|p| p.key_url.clone())
        .ok_or_else(|| format!("{provider} has no key page"))?;
    if !url.starts_with("https") {
        return Err("the key page in the catalogue is not an https address".to_owned());
    }
    std::process::Command::new("open")
        .arg(url)
        .status()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Tells a waiting walkthrough to test the providers again (U-12).
fn providers_changed<R: tauri::Runtime>(app: &AppHandle<R>) {
    use tauri::Emitter;
    let _ = app.emit_to(crate::walkthrough::WALKTHROUGH, "providers-changed", ());
}

fn provider_for(app: &AppHandle, role: &str) -> Result<(Config, String), String> {
    let config = load(&config_path(app))?;
    let provider = match role {
        "speech" => config.speech.provider.clone(),
        "cleanup" => config.cleanup.provider.clone(),
        other => return Err(format!("{other} is not a provider role")),
    };
    Ok((config, provider))
}

/// R-25: the key goes straight to the keychain; it is never returned, logged or stored
/// elsewhere. `provider` is the one being picked in the providers table, when it is not yet
/// the role's (R-12); otherwise the role's own provider gets the key.
#[tauri::command]
pub fn key_set(
    app: AppHandle,
    role: String,
    key: String,
    provider: Option<String>,
) -> Result<(), String> {
    let (config, current) = provider_for(&app, &role)?;
    let provider = provider.unwrap_or(current);
    if config.provider(&provider).is_none() {
        return Err(format!("{provider} is not in the provider catalogue"));
    }
    // One key per provider (D-2): OpenAI speech saves the OpenAI key.
    save_key(
        &app,
        &KeychainStore::new(),
        config.key_account(&provider),
        &key,
    )?;
    // Another key may offer other models.
    model_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&provider);
    // A new key reaches the next dictation without a restart.
    if let Err(why) = crate::runtime::reload_providers(&app) {
        tracing::warn!("providers unchanged: {why}");
    }
    Ok(())
}

/// Stores the key, then tells a waiting walkthrough to test it (U-12).
fn save_key<R: tauri::Runtime>(
    app: &AppHandle<R>,
    store: &dyn SecretStore,
    provider: &str,
    key: &str,
) -> Result<(), String> {
    store.set(provider, key).map_err(|e| e.to_string())?;
    providers_changed(app);
    Ok(())
}

/// Tests a key and model against their provider (R-13): opens a speech session with the model
/// (no audio is sent), or has the cleanup model clean one word. Returns the time it took in ms. With `provider` and `model`, tests the
/// pair being picked in the providers table without saving it (R-12).
#[tauri::command]
pub async fn key_test(
    app: AppHandle,
    role: String,
    provider: Option<String>,
    model: Option<String>,
) -> Result<u64, String> {
    // The same path the runtime builds its providers with, so Test and dictation agree.
    let (mut config, current) = provider_for(&app, &role)?;
    if let Some(provider) = provider.filter(|p| *p != current) {
        let text = std::fs::read_to_string(config_path(&app)).map_err(|e| e.to_string())?;
        let model = model.unwrap_or_default();
        let candidate = config::choose_provider(&text, role_named(&role)?, &provider, &model)
            .map_err(|e| e.to_string())?;
        config = Config::parse(&candidate, &config_path(&app)).map_err(|e| e.to_string())?;
    }
    let keys = KeychainStore::new();
    let start = Instant::now();
    if role == "speech" {
        let session = any::speech(&config, &keys)
            .map_err(|e| e.message)?
            .open()
            .await
            .map_err(|e| e.message)?;
        drop(session);
    } else {
        // R-13: a completion from the chosen model, so a wrong model fails here.
        any::cleanup(&config, &keys)
            .map_err(|e| e.message)?
            .check()
            .await
            .map_err(|e| e.message)?;
    }
    Ok(start.elapsed().as_millis() as u64)
}

/// Opens a System Settings page or the config file.
#[tauri::command]
pub fn settings_open(app: AppHandle, what: String) -> Result<(), String> {
    let target = match what.as_str() {
        "config" => config_path(&app).display().to_string(),
        // macOS's own dialog, which adds the row and has its own "Open System Settings"
        // button, so the page is not opened as well.
        "request-accessibility" => {
            permissions::request_accessibility();
            return Ok(());
        }
        "request-input-monitoring" => {
            permissions::request_input_monitoring();
            return Ok(());
        }
        permission => permissions::settings_url(permission)
            .ok_or_else(|| format!("{permission} is not something Settings opens"))?
            .to_owned(),
    };
    std::process::Command::new("open")
        .arg(target)
        .status()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Restarts HyperMe so saved settings take effect.
#[tauri::command]
pub fn app_restart(app: AppHandle) {
    app.restart();
}

#[cfg(test)]
mod tests {

    /// V0.1 redesign: the sheet sits centred over Home under its title bar, and shrinks with a
    /// small Home but never below its minimum.
    #[test]
    fn the_sheet_sits_over_home() {
        assert_eq!(
            super::sheet_frame((100.0, 50.0), (1000.0, 700.0)),
            (190.0, 78.0, 820.0, 580.0)
        );
        let (_, _, w, h) = super::sheet_frame((0.0, 0.0), (720.0, 480.0));
        assert_eq!((w, h), (680.0, 440.0));
    }

    /// #216 regression, V0.1 redesign: the account label is a choice (Personal, Work, Other), so
    /// no greyed placeholder can look like a value already filled in; and the Dictionary is not
    /// in Settings any more (it lives in Home's sidebar).
    #[test]
    fn the_connect_dialog_label_is_a_choice_and_no_dictionary_in_settings() {
        let page = include_str!("../../ui/settings.html");
        for value in ["personal", "work", "other"] {
            assert!(
                page.contains(&format!("name=\"oauth-label-choice\" value=\"{value}\"")),
                "missing the {value} choice"
            );
        }
        assert!(!page.contains("placeholder=\"personal\""));
        assert!(!page.contains("id=\"dictionary\""));
        assert!(!page.contains("dictionary.js"));
    }
    /// Bug, 26 November 2026: the walkthrough's Test button saves a typed key, clears the field
    /// and waits for `providers-changed` to test it, but saving a key never sent that event, so
    /// the key seemed to vanish untested. Saving a key must tell the walkthrough (U-12, AC-21).
    #[test]
    fn saving_a_key_tells_the_walkthrough() {
        use hyperme_core::secrets::{MemorySecretStore, SecretStore};
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tauri::Listener;

        let app = tauri::test::mock_app();
        let heard = Arc::new(AtomicUsize::new(0));
        let counter = heard.clone();
        app.listen_any("providers-changed", move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        let store = MemorySecretStore::default();
        super::save_key(app.handle(), &store, "groq", "test-key").unwrap();
        assert_eq!(store.get("groq").unwrap().as_deref(), Some("test-key"));
        assert_eq!(heard.load(Ordering::SeqCst), 1);
    }

    use super::*;
    use hyperme_core::config::Modifier;
    use serde_json::json;

    /// R-24: values from the page become TOML the config accepts.
    #[test]
    fn page_values_become_toml() {
        assert_eq!(toml_value(&json!(true)).unwrap(), "true");
        assert_eq!(toml_value(&json!(2000)).unwrap(), "2000");
        assert_eq!(
            toml_value(&json!("right_option")).unwrap(),
            "\"right_option\""
        );
        assert_eq!(toml_value(&json!("a\"b\\c")).unwrap(), r#""a\"b\\c""#);
        assert!(toml_value(&json!(-1)).is_err());
        assert!(toml_value(&json!(null)).is_err());
        // A changed value round-trips through the real config.
        let text = config::set_value(
            config::DEFAULT_CONFIG,
            "cleanup",
            "model",
            &toml_value(&json!("x\"y")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            Config::parse(&text, Path::new("c.toml"))
                .unwrap()
                .cleanup
                .model,
            "x\"y"
        );
    }

    #[test]
    fn hotkeys_show_as_the_config_writes_them() {
        assert_eq!(hotkey_text(&Hotkey::Fn), "fn");
        assert_eq!(hotkey_text(&Hotkey::RightOption), "right_option");
        let chord = Hotkey::Chord {
            modifiers: vec![Modifier::Ctrl, Modifier::Option],
            key: "space".into(),
        };
        assert_eq!(hotkey_text(&chord), "ctrl+option+space");
    }

    /// R-25: the view sent to the page says whether a key is set, never what it is.
    #[test]
    fn the_view_has_no_field_for_a_key() {
        let view = serde_json::to_value(KeysView {
            speech: true,
            cleanup: false,
        })
        .unwrap();
        assert_eq!(view, json!({"speech": true, "cleanup": false}));
    }
}
