//! Slice 5 of specs/97-google-connectors: what Settings → Connectors shows and saves, as pure
//! logic the page only draws. Tests cite `specs/97-google-connectors/spec.md`.

use std::path::Path;

use hyperme_core::config::{Config, DEFAULT_CONFIG};
use hyperme_core::connector_settings::{
    self as cs, AccountEntry, AccountHealth, CustomForm, Link, Tone,
};
use hyperme_core::registry::ToolAccess;
use time::{Date, Duration, Month, Time, UtcDateTime};

fn now() -> UtcDateTime {
    UtcDateTime::new(
        Date::from_calendar_date(2026, Month::November, 27).unwrap(),
        Time::from_hms(10, 0, 0).unwrap(),
    )
}

fn config(extra: &str) -> Config {
    Config::parse(
        &format!("{DEFAULT_CONFIG}\n{extra}"),
        Path::new("config.toml"),
    )
    .unwrap()
}

const ACCOUNTS: &str = r#"
[[connectors.accounts]]
connector = "google"
label = "personal"

[[connectors.accounts]]
connector = "google"
label = "work"

[[connectors.accounts]]
connector = "google"
label = "old"
enabled = false

[[connectors.custom]]
id = "wiki"
name = "Team wiki"
url = "https://wiki.example.com/mcp"
auth = "none"
"#;

fn health(connector: &str, label: &str) -> AccountHealth {
    match (connector, label) {
        ("google", "personal") => AccountHealth::Works {
            synced: Some(now() - Duration::minutes(3)),
        },
        ("google", "work") => AccountHealth::SignInAgain,
        _ => AccountHealth::Works { synced: None },
    }
}

/// R-3: the list shows each connector with its accounts, their state and sync age, and a link
/// to each row.
#[test]
fn the_list_shows_each_account_with_its_state() {
    let config = config(ACCOUNTS);
    let registry = cs::registry(None, &config.connectors).unwrap();
    let rows = cs::list_rows(&registry, &config.connectors, &health, now());
    let shown: Vec<(&str, &str, Tone)> = rows
        .iter()
        .map(|r| (r.title.as_str(), r.state.as_str(), r.tone))
        .collect();
    assert_eq!(
        shown,
        [
            ("Google · personal", "Works · synced 3 min ago", Tone::Ok),
            ("Google · work", "Sign in again", Tone::Bad),
            ("Google · old", "Off", Tone::Off),
            ("Team wiki", "Works · not synced yet", Tone::Ok),
        ]
    );
    assert_eq!(rows[1].id, "google:work");
    assert_eq!(rows[1].link, "hyperme://settings/connectors/google:work");
    assert_eq!(rows[3].id, "wiki");
}

/// AC-13: each connector and each account has a Setup check row, and every row links to its
/// place in Connectors.
#[test]
fn setup_check_has_a_row_per_connector_and_account() {
    let config = config(ACCOUNTS);
    let registry = cs::registry(None, &config.connectors).unwrap();
    let rows = cs::setup_rows(&registry, &config.connectors, &health, now());
    let titles: Vec<&str> = rows.iter().map(|r| r.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Google",
            "Google · personal",
            "Google · work",
            "Google · old",
            "Team wiki"
        ]
    );
    assert!(
        rows.iter()
            .all(|r| r.link.starts_with("hyperme://settings/connectors/"))
    );
    let work = rows.iter().find(|r| r.title == "Google · work").unwrap();
    assert_eq!(work.tone, Tone::Bad);
    assert_eq!(work.link, "hyperme://settings/connectors/google:work");
    let google = rows.iter().find(|r| r.title == "Google").unwrap();
    assert_eq!(google.state, "1 of 3 accounts needs you");
    assert_eq!(google.tone, Tone::Bad);
}

/// AC-13: an expired account's alert carries its link, and the link opens Connectors at that
/// account's row; other `hyperme://settings/<section>` links open their section.
#[test]
fn links_open_the_right_place() {
    let alert = cs::link_for("google:work");
    assert_eq!(
        cs::parse_link(&alert),
        Some(Link {
            section: "connectors".to_owned(),
            item: Some("google:work".to_owned()),
        })
    );
    assert_eq!(
        Link::from_hash("connectors/google:work"),
        cs::parse_link(&alert)
    );
    assert_eq!(
        cs::parse_link(&alert).unwrap().hash(),
        "connectors/google:work"
    );
    assert_eq!(
        cs::parse_link("hyperme://settings/providers"),
        Some(Link {
            section: "providers".to_owned(),
            item: None,
        })
    );
    for bad in [
        "https://settings/connectors",
        "hyperme://other/connectors",
        "hyperme://settings/",
        "hyperme://settings/Connectors",
        "hyperme://settings/connectors/<script>",
    ] {
        assert_eq!(cs::parse_link(bad), None, "{bad}");
    }
}

/// R-5: the account page lists the preset's named tools with their access; the user's switch
/// wins over the default and is saved in the config, and jobs see it.
#[test]
fn tool_switches_override_defaults_and_save() {
    let mut connectors = config(ACCOUNTS).connectors;
    let registry = cs::registry(None, &connectors).unwrap();
    let google = registry.get("google").unwrap();
    let rows = cs::tool_rows(google);
    let list_events = rows.iter().find(|r| r.name == "list_events").unwrap();
    assert_eq!(list_events.access, ToolAccess::Read);
    assert!(!list_events.changed);
    assert!(
        rows.iter()
            .any(|r| r.name == "create_draft" && r.access == ToolAccess::Ask)
    );
    assert!(
        rows.iter().all(|r| !r.name.contains('*')),
        "globs are not tools"
    );

    cs::set_tool(&mut connectors, "google", "list_events", ToolAccess::Off).unwrap();
    cs::set_tool(&mut connectors, "google", "list_events", ToolAccess::Ask).unwrap();
    let text = cs::write_connectors(DEFAULT_CONFIG, &connectors).unwrap();
    let reloaded = Config::parse(&text, Path::new("config.toml")).unwrap();
    assert_eq!(reloaded.connectors, connectors);
    let registry = cs::registry(None, &reloaded.connectors).unwrap();
    let google = registry.get("google").unwrap();
    assert_eq!(google.access("list_events"), ToolAccess::Ask);
    assert!(
        cs::tool_rows(google)
            .iter()
            .any(|r| r.name == "list_events" && r.changed)
    );
    assert!(google.job_allowlist(&["list_events".to_owned()]).is_err());

    assert!(cs::set_tool(&mut connectors, "nope", "x", ToolAccess::Off).is_err());
}

/// R-3: an account turns off without disconnecting, and back on.
#[test]
fn an_account_turns_off_and_on() {
    let mut connectors = config(ACCOUNTS).connectors;
    cs::set_enabled(&mut connectors, "google:personal", false).unwrap();
    assert_eq!(
        connectors.accounts[0],
        AccountEntry {
            connector: "google".to_owned(),
            label: "personal".to_owned(),
            enabled: false,
            features: Vec::new(),
        }
    );
    cs::set_enabled(&mut connectors, "google:personal", true).unwrap();
    assert!(connectors.accounts[0].enabled);
    assert!(cs::set_enabled(&mut connectors, "google:nobody", false).is_err());
}

/// R-4: the Add connector sheet lists the presets and a custom server. A custom server by URL
/// is saved to the config; one started by a command is refused for now, saying so.
#[test]
fn add_connector_offers_presets_and_saves_custom_servers() {
    let mut connectors = config("").connectors;
    let registry = cs::registry(None, &connectors).unwrap();
    let choices = cs::add_choices(&registry, &connectors);
    assert_eq!(choices[0].id, "google");
    assert!(
        choices[0].ready,
        "Google signs in with the setup sheet (slice 6)"
    );
    assert_eq!(choices.last().unwrap().id, "custom");

    let form = CustomForm {
        name: "Team wiki".to_owned(),
        url: "https://wiki.example.com/mcp".to_owned(),
        command: String::new(),
        auth: "bearer".to_owned(),
    };
    let id = cs::add_custom(&mut connectors, &registry, &form).unwrap();
    assert_eq!(id, "team-wiki");
    let text = cs::write_connectors(DEFAULT_CONFIG, &connectors).unwrap();
    let reloaded = Config::parse(&text, Path::new("config.toml")).unwrap();
    assert_eq!(
        reloaded.connectors.custom[0].url.as_deref(),
        Some("https://wiki.example.com/mcp")
    );
    let registry = cs::registry(None, &reloaded.connectors).unwrap();
    assert!(registry.get("team-wiki").is_some());

    let again = cs::add_custom(&mut connectors, &registry, &form).unwrap();
    assert_eq!(again, "team-wiki-2", "ids stay unique");

    let command = CustomForm {
        name: "Files".to_owned(),
        url: String::new(),
        command: "files-mcp".to_owned(),
        auth: "none".to_owned(),
    };
    let error = cs::add_custom(&mut connectors, &registry, &command).unwrap_err();
    assert!(error.contains("not supported yet"), "{error}");
    let plain = CustomForm {
        url: "http://example.com/mcp".to_owned(),
        ..form
    };
    assert!(cs::add_custom(&mut connectors, &registry, &plain).is_err());
}

/// Writing the connectors keeps the rest of the file as it was, comments included, and can be
/// done again.
#[test]
fn writing_connectors_keeps_the_rest_of_the_file() {
    let mut connectors = config(ACCOUNTS).connectors;
    let text = cs::write_connectors(DEFAULT_CONFIG, &connectors).unwrap();
    assert!(text.starts_with(DEFAULT_CONFIG.trim_end()));
    cs::set_tool(&mut connectors, "google", "get_thread", ToolAccess::Off).unwrap();
    let again = cs::write_connectors(&text, &connectors).unwrap();
    assert_eq!(again.matches("[[connectors.accounts]]").count(), 3);
    assert!(again.starts_with(DEFAULT_CONFIG.trim_end()));
    assert_eq!(
        Config::parse(&again, Path::new("config.toml"))
            .unwrap()
            .connectors,
        connectors
    );
}

/// The age shown next to "synced".
#[test]
fn sync_ages_read_plainly() {
    let ago = |d: Duration| cs::sync_age(now(), Some(now() - d));
    assert_eq!(ago(Duration::seconds(20)), "synced just now");
    assert_eq!(ago(Duration::minutes(3)), "synced 3 min ago");
    assert_eq!(ago(Duration::hours(2)), "synced 2 h ago");
    assert_eq!(ago(Duration::days(3)), "synced 3 days ago");
    assert_eq!(cs::sync_age(now(), None), "not synced yet");
}
