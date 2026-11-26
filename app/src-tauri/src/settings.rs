//! The Settings window (docs/UX.md, "Settings window"; spec R-24, R-25): a normal window opened
//! from the menu, created when opened and dropped when closed, so it costs no memory while
//! closed. Every change saves at once to `config.toml`; keys go straight to the keychain and are
//! never sent back to the page.

use std::path::{Path, PathBuf};
use std::time::Instant;

use hyperme_core::config::{self, Config, Hotkey, Retention};
use hyperme_core::secrets::SecretStore;
use hyperme_core::traits::{CleanupProvider, SpeechProvider};
use hyperme_platform::keychain::KeychainStore;
use hyperme_platform::permissions;
use hyperme_providers::cleanup::ChatCleanup;
use hyperme_providers::speechmatics::Speechmatics;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// The Settings window's label.
pub const SETTINGS: &str = "settings";

fn config_path(app: &AppHandle) -> PathBuf {
    crate::runtime::data_dir(app).join("config.toml")
}

/// Opens Settings, or brings it to the front, at `section` (`providers`, for example).
pub fn open(app: &AppHandle, section: &str) {
    let url = format!("settings.html#{section}");
    if let Some(window) = app.get_webview_window(SETTINGS) {
        let _ = window.eval(format!("location.hash = {}", serde_json::json!(section)));
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App(url.into()))
        .title("HyperMe Settings")
        .inner_size(760.0, 540.0)
        .min_inner_size(640.0, 440.0)
        .build();
    if let Ok(window) = built {
        let _ = window.set_focus();
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
    setup: SetupView,
    version: &'static str,
}

#[derive(Debug, Serialize)]
struct ConfigView {
    hotkey: String,
    min_hold_ms: u64,
    max_length_s: u64,
    short_dictation_words: u32,
    sounds: bool,
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
        timing_log: config.timing.log,
    }
}

fn load(path: &Path) -> Result<Config, String> {
    Config::load_or_create(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn settings_view(app: AppHandle) -> View {
    let path = config_path(&app);
    let loaded = load(&path);
    let keys = KeychainStore::new();
    let has_key =
        |provider: Option<&str>| provider.is_some_and(|p| matches!(keys.get(p), Ok(Some(_))));
    let config = loaded.as_ref().ok();
    View {
        keys: KeysView {
            speech: has_key(config.map(|c| c.speech.provider.as_str())),
            cleanup: has_key(config.map(|c| c.cleanup.provider.as_str())),
        },
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

/// Saves one setting. Changes take effect after a restart; the page offers one.
#[tauri::command]
pub fn settings_set(
    app: AppHandle,
    section: String,
    key: String,
    value: Value,
) -> Result<(), String> {
    let path = config_path(&app);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let updated = config::set_value(&text, &section, &key, &toml_value(&value)?)
        .map_err(|e| e.to_string())?;
    // Write beside the file, then rename, so a crash never leaves half a config.
    let temp = path.with_extension("toml.new");
    std::fs::write(&temp, updated).map_err(|e| format!("{}: {e}", temp.display()))?;
    std::fs::rename(&temp, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    if section == "speech" || section == "cleanup" {
        providers_changed(&app);
    }
    Ok(())
}

/// Tells a waiting walkthrough to test the providers again (U-12).
fn providers_changed(app: &AppHandle) {
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
/// elsewhere.
#[tauri::command]
pub fn key_set(app: AppHandle, role: String, key: String) -> Result<(), String> {
    let (_, provider) = provider_for(&app, &role)?;
    KeychainStore::new()
        .set(&provider, &key)
        .map_err(|e| e.to_string())
}

/// Tests a key against its provider: opens a speech session (no audio is sent) or warms the
/// cleanup connection. Returns the time it took in ms.
#[tauri::command]
pub async fn key_test(app: AppHandle, role: String) -> Result<u64, String> {
    let (config, provider) = provider_for(&app, &role)?;
    let key = KeychainStore::new()
        .get(&provider)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("No {provider} key yet"))?;
    let start = Instant::now();
    if role == "speech" {
        let session = Speechmatics::new(&config.speech, key)
            .open()
            .await
            .map_err(|e| e.message)?;
        drop(session);
    } else {
        ChatCleanup::new(&config.cleanup, &key)
            .map_err(|e| e.message)?
            .warm()
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
