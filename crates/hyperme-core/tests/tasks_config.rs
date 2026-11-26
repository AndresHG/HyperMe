//! Tasks slice 4: the `[tasks]` section of the config file. Tests cite
//! `specs/92-daily-tasks/spec.md`.

use std::path::Path;

use hyperme_core::config::{Config, ConfigError, DEFAULT_CONFIG};
use hyperme_core::schedule::Schedule;
use hyperme_core::tasks::WeekdaySet;
use time::{Time, Weekday};

fn parse(text: &str) -> Result<Config, ConfigError> {
    Config::parse(text, Path::new("/tmp/hyperme/config.toml"))
}

fn default_with(from: &str, to: &str) -> String {
    assert!(DEFAULT_CONFIG.contains(from), "no `{from}` in the defaults");
    DEFAULT_CONFIG.replacen(from, to, 1)
}

fn hm(h: u8, m: u8) -> Time {
    Time::from_hms(h, m, 0).unwrap()
}

/// "Day start", "Workday", R-14 and T-1: 04:00, Monday to Friday, three roll-overs, and the
/// morning card from 06:00 with 09:30 as the latest.
#[test]
fn the_task_defaults_are_the_spec_decisions() {
    let tasks = Config::default().tasks;
    let rules = tasks.day_rules();
    assert_eq!(rules.day_start, hm(4, 0));
    assert_eq!(
        rules.workdays,
        WeekdaySet::from_array([
            Weekday::Monday,
            Weekday::Tuesday,
            Weekday::Wednesday,
            Weekday::Thursday,
            Weekday::Friday,
        ])
    );
    assert_eq!(rules.stale_after, 3);
    assert_eq!(
        tasks.morning(),
        Schedule::OnPresenceAfter {
            from: hm(6, 0),
            latest: hm(9, 30),
            weekdays: rules.workdays,
        }
    );
}

/// The settings are the user's: a Sunday-to-Thursday week, a day that turns over at midnight.
#[test]
fn the_task_settings_can_be_changed() {
    let text = default_with("day_start = \"04:00\"", "day_start = \"00:00\"");
    let text = text.replacen(
        "workdays = [\"mon\", \"tue\", \"wed\", \"thu\", \"fri\"]",
        "workdays = [\"sun\", \"mon\", \"tue\", \"wed\", \"thu\"]",
        1,
    );
    let rules = parse(&text).unwrap().tasks.day_rules();
    assert_eq!(rules.day_start, hm(0, 0));
    assert!(rules.workdays.contains(Weekday::Sunday));
    assert!(!rules.workdays.contains(Weekday::Friday));
}

/// An older file without `[tasks]` still loads, with the defaults.
#[test]
fn a_file_without_the_tasks_section_takes_the_defaults() {
    let start = DEFAULT_CONFIG.find("[tasks]").expect("a [tasks] section");
    let end = DEFAULT_CONFIG[start + 1..]
        .find("\n[")
        .map_or(DEFAULT_CONFIG.len(), |i| start + 1 + i);
    let text = format!("{}{}", &DEFAULT_CONFIG[..start], &DEFAULT_CONFIG[end..]);
    assert!(!text.contains("[tasks]"));
    assert_eq!(parse(&text).unwrap().tasks, Config::default().tasks);
}

/// Values out of range name their key, like the rest of the file.
#[test]
fn bad_task_settings_are_named() {
    let cases = [
        ("day_start = \"04:00\"", "day_start = \"4 am\"", "day_start"),
        (
            "day_start = \"04:00\"",
            "day_start = \"25:00\"",
            "day_start",
        ),
        (
            "workdays = [\"mon\", \"tue\", \"wed\", \"thu\", \"fri\"]",
            "workdays = [\"mon\", \"funday\"]",
            "workdays",
        ),
        (
            "workdays = [\"mon\", \"tue\", \"wed\", \"thu\", \"fri\"]",
            "workdays = []",
            "tasks.workdays",
        ),
        ("stale_after = 3", "stale_after = 0", "tasks.stale_after"),
        ("stale_after = 3", "stale_after = 31", "tasks.stale_after"),
        (
            "morning_latest = \"09:30\"",
            "morning_latest = \"05:00\"",
            "tasks.morning_latest",
        ),
    ];
    for (from, to, key) in cases {
        let err = parse(&default_with(from, to)).unwrap_err().to_string();
        assert!(err.contains(key), "{to}: {err}");
    }
}
