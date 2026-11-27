//! Slice 2: the config file. Tests cite the spec's requirements and acceptance criteria
//! (`specs/25-dictation-core/spec.md`).

use std::path::{Path, PathBuf};

use hyperme_core::config::{Config, ConfigError, DEFAULT_CONFIG, Hotkey, Modifier, Retention};
use hyperme_core::secrets::{MemorySecretStore, SecretStore};

/// A fresh, empty folder under the target directory, so tests never touch the user's files.
fn temp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The default file with one line replaced, for error tests.
fn default_with(from: &str, to: &str) -> String {
    assert!(
        DEFAULT_CONFIG.contains(from),
        "default config has no `{from}`"
    );
    DEFAULT_CONFIG.replacen(from, to, 1)
}

fn parse(text: &str) -> Result<Config, ConfigError> {
    Config::parse(text, Path::new("/tmp/hyperme/config.toml"))
}

/// The 1-based line of the first line of `text` that starts with `start`.
fn line_of(text: &str, start: &str) -> usize {
    text.lines().position(|l| l.starts_with(start)).unwrap() + 1
}

/// R-26: the defaults are the owner's decisions, and they come from the default file.
#[test]
fn defaults_are_the_owner_decisions() {
    let config = Config::default();
    let d = &config.dictation;
    assert_eq!(d.hotkey, Hotkey::Fn);
    assert_eq!(d.min_hold_ms, 250);
    assert_eq!(d.max_length_s, 120);
    // #167 AC-8.
    assert_eq!(d.double_tap_ms, 400);
    assert_eq!(d.short_dictation_words, 4);
    assert!(d.sounds);
    assert_eq!(d.dictionary, None);

    assert_eq!(config.speech.provider, "speechmatics");
    assert_eq!(config.speech.model, "melia-1");
    assert_eq!(config.speech.endpoint, "wss://eu.rt.speechmatics.com/v2");

    let c = &config.cleanup;
    assert_eq!(c.provider, "groq");
    assert_eq!(c.model, "openai/gpt-oss-20b");
    assert_eq!(c.endpoint, "https://api.groq.com/openai/v1");
    assert_eq!(c.reasoning_effort, "low");
    assert_eq!(c.prompt, "v4");
    assert_eq!(c.deadline_ms, 1500);

    assert!(config.history.enabled);
    assert_eq!(config.history.retention_days, Retention::Days(30));
    assert_eq!(config.history.size_limit_mb, 1024);
    assert!(!config.timing.log);
}

/// R-26: provider and model names and endpoints live in the default file, not in code.
#[test]
fn no_provider_or_model_names_in_code() {
    let sources = [
        ("lib.rs", include_str!("../src/lib.rs")),
        ("config.rs", include_str!("../src/config.rs")),
        ("secrets.rs", include_str!("../src/secrets.rs")),
        ("traits.rs", include_str!("../src/traits.rs")),
        ("dictation.rs", include_str!("../src/dictation.rs")),
    ];
    for (file, source) in sources {
        for name in [
            "melia-1",
            "gpt-oss",
            "groq.com",
            "speechmatics.com",
            "wss://",
            "https://",
        ] {
            assert!(
                !source.contains(name),
                "{file} names `{name}`; it belongs in the config"
            );
        }
    }
}

/// Design, "Config": a default file is written when none exists, and read back after.
#[test]
fn writes_the_default_file_when_there_is_none() {
    let path = temp_dir("writes_default")
        .join("HyperMe")
        .join("config.toml");
    let config = Config::load_or_create(&path).unwrap();
    assert_eq!(config, Config::default());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), DEFAULT_CONFIG);

    std::fs::write(
        &path,
        default_with("min_hold_ms = 250", "min_hold_ms = 300"),
    )
    .unwrap();
    let edited = Config::load_or_create(&path).unwrap();
    assert_eq!(edited.dictation.min_hold_ms, 300);
}

/// Design, "Config": an unknown key is an error with the file and the line.
#[test]
fn unknown_key_names_the_file_and_line() {
    let text = default_with("sounds = true", "sounds = true\nsound_volume = 3");
    let err = parse(&text).unwrap_err().to_string();
    let line = line_of(&text, "sound_volume");
    assert!(err.contains("/tmp/hyperme/config.toml"), "{err}");
    assert!(err.contains(&format!("line {line}")), "{err}");
    assert!(err.contains("sound_volume"), "{err}");
}

#[test]
fn unknown_section_names_the_file_and_line() {
    let text = format!("{DEFAULT_CONFIG}\n[ui]\ntheme = \"dark\"\n");
    let err = parse(&text).unwrap_err().to_string();
    assert!(
        err.contains(&format!("line {}", line_of(&text, "[ui]"))),
        "{err}"
    );
    assert!(err.contains("ui"), "{err}");
}

/// #167 AC-8: a file written before hands-free existed still loads, with the default window.
#[test]
fn an_older_file_without_double_tap_loads() {
    let text = default_with("double_tap_ms = 400\n", "");
    assert_eq!(parse(&text).unwrap().dictation.double_tap_ms, 400);
}

#[test]
fn missing_key_is_named() {
    let text = default_with("deadline_ms = 1500\n", "");
    let err = parse(&text).unwrap_err().to_string();
    assert!(err.contains("deadline_ms"), "{err}");
}

#[test]
fn wrong_type_names_the_line() {
    let text = default_with("min_hold_ms = 250", "min_hold_ms = \"long\"");
    let err = parse(&text).unwrap_err().to_string();
    assert!(
        err.contains(&format!("line {}", line_of(&text, "min_hold_ms"))),
        "{err}"
    );
}

/// Design, "Config": out-of-range values name the key.
#[test]
fn out_of_range_values_name_the_key() {
    let cases = [
        (
            "min_hold_ms = 250",
            "min_hold_ms = 10",
            "dictation.min_hold_ms",
        ),
        (
            "max_length_s = 120",
            "max_length_s = 0",
            "dictation.max_length_s",
        ),
        // #167 AC-8.
        (
            "double_tap_ms = 400",
            "double_tap_ms = 1001",
            "dictation.double_tap_ms",
        ),
        (
            "short_dictation_words = 4",
            "short_dictation_words = 51",
            "dictation.short_dictation_words",
        ),
        (
            "deadline_ms = 1500",
            "deadline_ms = 50",
            "cleanup.deadline_ms",
        ),
        (
            "retention_days = 30",
            "retention_days = 0",
            "history.retention_days",
        ),
        (
            "size_limit_mb = 1024",
            "size_limit_mb = 1",
            "history.size_limit_mb",
        ),
    ];
    for (from, to, key) in cases {
        let err = parse(&default_with(from, to)).unwrap_err();
        let msg = err.to_string();
        assert!(matches!(err, ConfigError::Invalid { .. }), "{to}: {msg}");
        assert!(msg.contains(key), "{to}: {msg}");
        assert!(msg.contains("/tmp/hyperme/config.toml"), "{to}: {msg}");
    }
}

#[test]
fn empty_names_and_wrong_schemes_are_rejected() {
    let cases = [
        (
            "provider = \"speechmatics\"",
            "provider = \"whisperx\"",
            "speech.provider",
        ),
        ("model = \"melia-1\"", "model = \"\"", "speech.model"),
        (
            "provider = \"groq\"",
            "provider = \" \"",
            "cleanup.provider",
        ),
        (
            "reasoning_effort = \"low\"",
            "reasoning_effort = \"\"",
            "cleanup.reasoning_effort",
        ),
        ("prompt = \"v4\"", "prompt = \"\"", "cleanup.prompt"),
    ];
    for (from, to, key) in cases {
        let msg = parse(&default_with(from, to)).unwrap_err().to_string();
        assert!(msg.contains(key), "{to}: {msg}");
    }
    // A file from before the provider catalogue still sets endpoints, and they are checked
    // against the provider's protocol.
    let before = include_str!("fixtures/config-before-96.toml");
    for (from, to, key) in [
        (
            "endpoint = \"wss://eu.rt.speechmatics.com/v2\"",
            "endpoint = \"https://eu.rt.speechmatics.com/v2\"",
            "speech.endpoint",
        ),
        (
            "endpoint = \"https://api.groq.com/openai/v1\"",
            "endpoint = \"api.groq.com\"",
            "cleanup.endpoint",
        ),
    ] {
        assert!(before.contains(from), "{from}");
        let msg = parse(&before.replacen(from, to, 1))
            .unwrap_err()
            .to_string();
        assert!(msg.contains(key), "{to}: {msg}");
    }
}

/// R-8: hold Fn by default; Right Option and chords are offered.
#[test]
fn hotkeys_fn_right_option_and_chords() {
    let parse_hotkey = |value: &str| {
        parse(&default_with(
            "hotkey = \"fn\"",
            &format!("hotkey = \"{value}\""),
        ))
        .map(|c| c.dictation.hotkey)
    };
    assert_eq!(parse_hotkey("right_option").unwrap(), Hotkey::RightOption);
    assert_eq!(
        parse_hotkey("ctrl+option+space").unwrap(),
        Hotkey::Chord {
            modifiers: vec![Modifier::Ctrl, Modifier::Option],
            key: "space".into()
        }
    );
    assert_eq!(
        parse_hotkey("Cmd+Shift+D").unwrap(),
        Hotkey::Chord {
            modifiers: vec![Modifier::Cmd, Modifier::Shift],
            key: "d".into()
        }
    );
    for bad in [
        "",
        "space",
        "ctrl+",
        "ctrl+shift",
        "ctrl+a+b",
        "hyper+a",
        "ctrl+ctrl+a",
    ] {
        let msg = parse_hotkey(bad).unwrap_err().to_string();
        assert!(msg.contains("dictation.hotkey"), "{bad:?}: {msg}");
    }
}

/// R-18: history keeps 30 days or unlimited.
#[test]
fn retention_can_be_unlimited() {
    let text = default_with("retention_days = 30", "retention_days = \"unlimited\"");
    assert_eq!(
        parse(&text).unwrap().history.retention_days,
        Retention::Unlimited
    );
    let bad = default_with("retention_days = 30", "retention_days = \"forever\"");
    let msg = parse(&bad).unwrap_err().to_string();
    assert!(msg.contains("retention_days"), "{msg}");
}

#[test]
fn dictionary_path_is_optional() {
    let text = default_with(
        "# dictionary = \"/Users/you/hyperme-dictionary.txt\"",
        "dictionary = \"/tmp/terms.txt\"",
    );
    assert_eq!(
        parse(&text).unwrap().dictation.dictionary,
        Some(PathBuf::from("/tmp/terms.txt"))
    );
}

/// R-26: the config file holds no secrets. A key-like field is rejected, with its line, and
/// the error never repeats the value.
#[test]
fn key_like_fields_are_rejected() {
    let secret = "fake-key-aaaa";
    for field in [
        "api_key",
        "key",
        "groq_key",
        "token",
        "access_token",
        "secret",
        "password",
    ] {
        let line = format!("{field} = \"{secret}\"");
        let text = default_with("deadline_ms = 1500", &format!("deadline_ms = 1500\n{line}"));
        let err = parse(&text).unwrap_err();
        let msg = err.to_string();
        assert!(matches!(err, ConfigError::Secret { .. }), "{field}: {msg}");
        assert!(msg.contains(&format!("cleanup.{field}")), "{field}: {msg}");
        assert!(
            msg.contains(&format!("line {}", line_of(&text, &line))),
            "{field}: {msg}"
        );
        assert!(msg.contains("keychain"), "{field}: {msg}");
        assert!(
            !msg.contains(secret),
            "{field}: the error repeats the secret: {msg}"
        );
    }
    // Unquoted, the line is not valid TOML; the error must still not echo it.
    let text = default_with(
        "deadline_ms = 1500",
        &format!("deadline_ms = 1500\napi_key = {secret}"),
    );
    let msg = parse(&text).unwrap_err().to_string();
    assert!(
        msg.contains("cleanup.api_key") || msg.contains("api_key"),
        "{msg}"
    );
    assert!(!msg.contains(secret), "the error repeats the secret: {msg}");
    // A dotted key in another section.
    let text = format!("{DEFAULT_CONFIG}\n[extra]\ngroq.api_key = \"{secret}\"\n");
    let msg = parse(&text).unwrap_err().to_string();
    assert!(msg.contains("api_key") && !msg.contains(secret), "{msg}");
    // `hotkey` is not a secret.
    assert!(parse(DEFAULT_CONFIG).is_ok());
}

/// AC-18 (config half): keys go through the `SecretStore` trait; with a key in a fake store,
/// the config file written and read never contains it.
#[test]
fn config_file_never_contains_a_key() {
    let secret = "fake-key-bbbb";
    let store = MemorySecretStore::default();
    store.set("groq", secret).unwrap();
    store.set("speechmatics", "fake-key-cccc").unwrap();

    let dir = temp_dir("never_contains_key");
    let path = dir.join("config.toml");
    let config = Config::load_or_create(&path).unwrap();
    assert_eq!(
        store.get(&config.cleanup.provider).unwrap().as_deref(),
        Some(secret)
    );

    let written = std::fs::read_to_string(&path).unwrap();
    assert!(!written.contains(secret));
    assert!(!written.contains("fake-key-cccc"));
    assert!(!format!("{config:?}").contains(secret));
}

#[test]
fn secret_store_replaces_and_deletes() {
    let store = MemorySecretStore::default();
    assert_eq!(store.get("groq").unwrap(), None);
    store.set("groq", "one").unwrap();
    store.set("groq", "two").unwrap();
    assert_eq!(store.get("groq").unwrap().as_deref(), Some("two"));
    store.delete("groq").unwrap();
    assert_eq!(store.get("groq").unwrap(), None);
}

#[test]
fn unreadable_path_names_the_file() {
    let dir = temp_dir("unreadable");
    // A folder where the file should be: reading it fails.
    let path = dir.join("config.toml");
    std::fs::create_dir_all(&path).unwrap();
    let msg = Config::load_or_create(&path).unwrap_err().to_string();
    assert!(msg.contains(&path.display().to_string()), "{msg}");
}

/// Slice 9 (Settings): a change from the Settings window rewrites only that value in the file,
/// keeping its comments, and is validated like the file itself.
#[test]
fn set_value_rewrites_one_line_and_keeps_comments() {
    use hyperme_core::config::set_value;
    let text = set_value(DEFAULT_CONFIG, "cleanup", "deadline_ms", "2000").unwrap();
    assert_eq!(parse(&text).unwrap().cleanup.deadline_ms, 2000);
    assert_eq!(text.lines().count(), DEFAULT_CONFIG.lines().count());
    assert!(text.contains("# After this many ms without cleaned text"));

    // Strings are written quoted; the same key name in another section is not touched.
    let text = set_value(
        DEFAULT_CONFIG,
        "cleanup",
        "model",
        "\"llama-3.1-8b-instant\"",
    )
    .unwrap();
    let config = parse(&text).unwrap();
    assert_eq!(config.cleanup.model, "llama-3.1-8b-instant");
    assert_eq!(config.speech.model, "melia-1");

    let text = set_value(DEFAULT_CONFIG, "dictation", "hotkey", "\"right_option\"").unwrap();
    assert_eq!(parse(&text).unwrap().dictation.hotkey, Hotkey::RightOption);
    let text = set_value(DEFAULT_CONFIG, "history", "retention_days", "\"unlimited\"").unwrap();
    assert_eq!(
        parse(&text).unwrap().history.retention_days,
        Retention::Unlimited
    );
}

#[test]
fn set_value_adds_a_commented_out_key() {
    use hyperme_core::config::set_value;
    let text = set_value(
        DEFAULT_CONFIG,
        "dictation",
        "dictionary",
        "\"/tmp/terms.txt\"",
    )
    .unwrap();
    assert_eq!(
        parse(&text).unwrap().dictation.dictionary,
        Some(PathBuf::from("/tmp/terms.txt"))
    );
}

/// A value the file would reject is rejected before anything is written, with the key named;
/// so is a key-like field (R-26) and a key the file does not have.
#[test]
fn set_value_refuses_invalid_values_and_secrets() {
    use hyperme_core::config::set_value;
    let err = set_value(DEFAULT_CONFIG, "cleanup", "deadline_ms", "5")
        .unwrap_err()
        .to_string();
    assert!(err.contains("cleanup.deadline_ms"), "{err}");
    let err = set_value(DEFAULT_CONFIG, "cleanup", "api_key", "\"fake-key-gggg\"")
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("keychain") && !err.contains("fake-key-gggg"),
        "{err}"
    );
    let err = set_value(DEFAULT_CONFIG, "cleanup", "volume", "3")
        .unwrap_err()
        .to_string();
    assert!(err.contains("volume"), "{err}");
    let err = set_value(DEFAULT_CONFIG, "nowhere", "x", "1")
        .unwrap_err()
        .to_string();
    assert!(err.contains("nowhere"), "{err}");
}

/// The bead at rest (owner, 26 November 2026, U-13 brought forward): on by default, and a
/// config file written before the setting existed still loads, with the bead on.
#[test]
fn the_bead_shows_while_idle_by_default() {
    assert!(Config::default().dictation.show_while_idle);
    let older = default_with("show_while_idle = true\n", "");
    assert!(!older.contains("show_while_idle"));
    assert!(parse(&older).unwrap().dictation.show_while_idle);
    let off = default_with("show_while_idle = true", "show_while_idle = false");
    assert!(!parse(&off).unwrap().dictation.show_while_idle);
}
