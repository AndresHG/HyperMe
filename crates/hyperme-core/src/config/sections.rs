//! The config's sections: dictation, speech, cleanup, history, timing and tasks, with their
//! defaults and the values that take more than a plain type (hotkeys, retention, clock times).

use super::*;

fn show_while_idle() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dictation {
    /// R-8.
    pub hotkey: Hotkey,
    /// R-5: a shorter hold pastes nothing.
    pub min_hold_ms: u64,
    /// R-7.
    pub max_length_s: u64,
    /// #167: a key-down this soon after a tap locks recording (hands-free). Optional, so an
    /// older file still loads.
    #[serde(default = "default_double_tap_ms")]
    pub double_tap_ms: u64,
    /// R-13: fewer words than this skip cleanup.
    pub short_dictation_words: u32,
    /// R-32.
    pub sounds: bool,
    /// The bead at the bottom centre while idle (U-13, brought forward by the owner). Optional,
    /// so a config written before it existed still loads, with the bead on.
    #[serde(default = "show_while_idle")]
    pub show_while_idle: bool,
    /// Minutes of talking a day, for the cost estimates (U-11). Onboarding saves the user's
    /// choice here. Optional: an older file takes the default file's value.
    #[serde(default = "default_talking_minutes")]
    pub talking_minutes_per_day: u32,
    /// The user's terms, one per line.
    pub dictionary: Option<PathBuf>,
    /// Spec 93, R-5: up to this many entries the cleanup prompt carries them all; above it, only
    /// the ones relevant to the dictation. Optional: an older file takes the default.
    #[serde(default = "default_prompt_cap")]
    pub dictionary_prompt_cap: u32,
    /// #221, AC-7: mute the output while recording. Optional: an older file takes the default.
    #[serde(default = "yes")]
    pub mute_while_dictating: bool,
    /// #221, AC-2: preferred microphones by name, first first. Empty is the system default.
    #[serde(default)]
    pub microphones: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Speech {
    /// A provider in the catalogue whose protocol can fill the speech role.
    pub provider: String,
    pub model: String,
    /// The provider's endpoint, from the catalogue. A file written before the catalogue sets it
    /// here, and then it overrides the catalogue's (R-4).
    #[serde(default)]
    pub endpoint: String,
    /// Onboarding's "Get a key" page, from the catalogue unless an older file sets it.
    #[serde(default)]
    pub(super) key_url: Option<String>,
    /// Older files only: overrides the catalogue model's price (R-4).
    #[serde(default)]
    pub(super) price_per_hour_usd: Option<f64>,
    /// From the catalogue unless an older file sets it.
    #[serde(default)]
    pub(super) free_credit_usd: Option<f64>,
    /// Opened when the main provider fails (R-25, R-26). Settings saves it only after its Test
    /// passes (R-24a).
    #[serde(default)]
    pub fallback: Option<FallbackChoice>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cleanup {
    /// A provider in the catalogue whose protocol can fill the cleanup role. Labels the keychain
    /// entry and error messages (R-21).
    pub provider: String,
    pub model: String,
    /// Base URL of an OpenAI-compatible chat completions API, from the catalogue. A file written
    /// before the catalogue sets it here, and then it overrides the catalogue's (R-4).
    #[serde(default)]
    pub endpoint: String,
    /// Sent as `reasoning_effort`; `off` leaves the field out.
    pub reasoning_effort: String,
    /// How much the model may change the transcript (P2-2). Optional: an older file takes the
    /// default file's level.
    #[serde(default = "default_cleanup_level")]
    pub level: CleanupLevel,
    /// A named prompt version that replaces the level's prompt, for experiments. A file written
    /// before levels has `prompt = "v4"`, the old default, with no level; that is read as no
    /// override, so it gets the default level.
    #[serde(default)]
    pub prompt: Option<String>,
    /// R-12: after this, the raw transcript is pasted.
    pub deadline_ms: u64,
    /// Onboarding's "Get a key" page, from the catalogue unless an older file sets it.
    #[serde(default)]
    pub(super) key_url: Option<String>,
    /// Older files only: override the catalogue model's prices (R-4).
    #[serde(default)]
    pub(super) price_input_per_million_usd: Option<f64>,
    #[serde(default)]
    pub(super) price_output_per_million_usd: Option<f64>,
    /// What an hour of talking sends to cleanup, for the estimate. Optional: an older file
    /// takes the default file's values, so no estimate is ever written in code.
    #[serde(default)]
    pub(super) estimate_dictations_per_hour: Option<f64>,
    #[serde(default)]
    pub(super) estimate_input_tokens: Option<f64>,
    #[serde(default)]
    pub(super) estimate_output_tokens: Option<f64>,
    /// Tried when the main provider fails before the deadline (R-24). Settings saves it only
    /// after its Test passes (R-24a).
    #[serde(default)]
    pub fallback: Option<FallbackChoice>,
}

/// One provider dictation may send to (R-29), for the Privacy section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Destination {
    pub role: Role,
    /// As people write it: "Speechmatics".
    pub name: String,
    pub model: String,
    /// The main provider's name, when this is its fallback.
    pub fallback_for: Option<String>,
    /// A server on this Mac: nothing leaves it.
    pub on_this_mac: bool,
}

/// A role's fallback: `fallback = { provider = "openai", model = "gpt-6-luna" }` (R-3).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FallbackChoice {
    pub provider: String,
    pub model: String,
}

impl Speech {
    /// The provider's "Get a key" page, or empty when it has none.
    pub fn key_url(&self) -> String {
        self.key_url.clone().unwrap_or_default()
    }
    /// Credit new accounts get, or 0.
    pub fn free_credit_usd(&self) -> f64 {
        self.free_credit_usd.unwrap_or_default()
    }
}

impl Cleanup {
    /// The provider's "Get a key" page, or empty when it has none.
    pub fn key_url(&self) -> String {
        self.key_url.clone().unwrap_or_default()
    }
    pub fn estimate_dictations_per_hour(&self) -> f64 {
        or_default!(self, cleanup, estimate_dictations_per_hour)
    }
    pub fn estimate_input_tokens(&self) -> f64 {
        or_default!(self, cleanup, estimate_input_tokens)
    }
    pub fn estimate_output_tokens(&self) -> f64 {
        or_default!(self, cleanup, estimate_output_tokens)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct History {
    /// R-18.
    pub enabled: bool,
    pub retention_days: Retention,
    pub size_limit_mb: u64,
    /// Spec 182, R-9a (TS-1): keep each dictation's timing trace, numbers only, even with
    /// history off. A file written before traces has no such key: on.
    #[serde(default = "keep_traces_default")]
    pub keep_traces: bool,
}

fn keep_traces_default() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timing {
    /// R-30: one line of stage timings per dictation, numbers only.
    pub log: bool,
}

/// The task list's day, workdays and stale mark, and the morning card's times
/// (specs/92-daily-tasks). Times are "HH:MM"; weekdays are `mon` to `sun`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tasks {
    pub day_start: String,
    pub workdays: Vec<String>,
    pub stale_after: u32,
    pub morning_from: String,
    pub morning_latest: String,
}

fn default_double_tap_ms() -> u64 {
    defaults().dictation.double_tap_ms
}

/// How much the cleanup model may change the transcript (P2-2). None of them rephrase,
/// translate or answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CleanupLevel {
    /// No model call: the transcript after dictionary substitution, as a short dictation.
    None,
    /// Fillers, punctuation, capitals and clear mishearings; every other word kept.
    Light,
    /// Light, plus repetitions, false starts, small grammar slips and spoken formatting.
    Medium,
}

fn default_cleanup_level() -> CleanupLevel {
    defaults().cleanup.level
}

fn default_talking_minutes() -> u32 {
    defaults().dictation.talking_minutes_per_day
}

fn default_prompt_cap() -> u32 {
    defaults().dictation.dictionary_prompt_cap
}

pub(super) fn default_tasks() -> Tasks {
    defaults().tasks.clone()
}

/// "HH:MM", 00:00 to 23:59.
pub(super) fn clock_time(text: &str) -> Option<Time> {
    let (h, m) = text.trim().split_once(':')?;
    if h.len() != 2 || m.len() != 2 {
        return None;
    }
    Time::from_hms(h.parse().ok()?, m.parse().ok()?, 0).ok()
}

pub(super) fn weekday(name: &str) -> Option<Weekday> {
    Some(match name.trim().to_lowercase().as_str() {
        "mon" => Weekday::Monday,
        "tue" => Weekday::Tuesday,
        "wed" => Weekday::Wednesday,
        "thu" => Weekday::Thursday,
        "fri" => Weekday::Friday,
        "sat" => Weekday::Saturday,
        "sun" => Weekday::Sunday,
        _ => return None,
    })
}

impl Tasks {
    /// The day start, workdays and stale threshold. A value the file got wrong (validation
    /// refuses it, so only a hand-built config) falls back to the default.
    pub fn day_rules(&self) -> DayRules {
        let fallback = DayRules::default();
        let mut workdays = WeekdaySet::EMPTY;
        for name in &self.workdays {
            if let Some(day) = weekday(name) {
                workdays.insert(day);
            }
        }
        DayRules {
            day_start: clock_time(&self.day_start).unwrap_or(fallback.day_start),
            workdays: if workdays.is_empty() {
                fallback.workdays
            } else {
                workdays
            },
            stale_after: self.stale_after,
        }
    }

    /// The morning card's schedule (R-15, R-16).
    pub fn morning(&self) -> Schedule {
        let at = |text: &str, h, m| {
            clock_time(text).unwrap_or(Time::from_hms(h, m, 0).expect("a valid time"))
        };
        Schedule::OnPresenceAfter {
            from: at(&self.morning_from, 6, 0),
            latest: at(&self.morning_latest, 9, 30),
            weekdays: self.day_rules().workdays,
        }
    }
}

/// How long history keeps a dictation (R-18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RetentionValue")]
pub enum Retention {
    Days(u32),
    Unlimited,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RetentionValue {
    Days(u32),
    Word(String),
}

impl TryFrom<RetentionValue> for Retention {
    type Error = String;

    fn try_from(value: RetentionValue) -> Result<Self, String> {
        match value {
            RetentionValue::Days(days) => Ok(Retention::Days(days)),
            RetentionValue::Word(word) if word == "unlimited" => Ok(Retention::Unlimited),
            RetentionValue::Word(word) => Err(format!(
                "history.retention_days is {word:?}: write a number of days or \"unlimited\""
            )),
        }
    }
}

/// The dictation hotkey (R-8).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub enum Hotkey {
    /// Hold Fn (Globe), the default.
    Fn,
    RightOption,
    /// One or more modifiers and one other key, such as `ctrl+option+space`. The key is
    /// lowercase.
    Chord {
        modifiers: Vec<Modifier>,
        key: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    Cmd,
    Ctrl,
    Option,
    Shift,
    Fn,
}

impl TryFrom<String> for Hotkey {
    type Error = String;

    fn try_from(value: String) -> Result<Self, String> {
        let invalid = |why: &str| {
            format!(
                "dictation.hotkey is {value:?}: {why}. Write \"fn\", \"right_option\" or a chord \
                 such as \"ctrl+option+space\""
            )
        };
        let lower = value.trim().to_lowercase();
        match lower.as_str() {
            "fn" => return Ok(Hotkey::Fn),
            "right_option" => return Ok(Hotkey::RightOption),
            _ => {}
        }
        let parts: Vec<&str> = lower.split('+').map(str::trim).collect();
        let Some((key, modifiers)) = parts.split_last() else {
            return Err(invalid("it is empty"));
        };
        if modifiers.is_empty() {
            return Err(invalid("a chord needs at least one modifier"));
        }
        let mut seen = Vec::with_capacity(modifiers.len());
        for name in modifiers {
            let modifier = Modifier::named(name).ok_or_else(|| {
                invalid(&format!("{name:?} is not cmd, ctrl, option, shift or fn"))
            })?;
            if seen.contains(&modifier) {
                return Err(invalid(&format!("{name:?} appears twice")));
            }
            seen.push(modifier);
        }
        if key.is_empty() {
            return Err(invalid("the chord has no key after its modifiers"));
        }
        if Modifier::named(key).is_some() {
            return Err(invalid("the chord ends in a modifier, not a key"));
        }
        Ok(Hotkey::Chord {
            modifiers: seen,
            key: (*key).to_owned(),
        })
    }
}

impl Modifier {
    fn named(name: &str) -> Option<Self> {
        Some(match name {
            "cmd" => Modifier::Cmd,
            "ctrl" => Modifier::Ctrl,
            "option" => Modifier::Option,
            "shift" => Modifier::Shift,
            "fn" => Modifier::Fn,
            _ => return None,
        })
    }
}
