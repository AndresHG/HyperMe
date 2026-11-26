//! The "Check for updates…" line in the menu (docs/RELEASING.md). Pure, so every state and what a
//! click does is tested without a window or a network; `updater.rs` runs the checks with Tauri's
//! updater plugin on macOS.
//!
//! HyperMe checks only when the user clicks, and installs only when they click again: nothing
//! leaves the Mac on its own, and nothing is replaced behind their back.

/// Where the update stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// Nothing checked yet, or the last check is over.
    Idle,
    Checking,
    UpToDate,
    /// A signed update to this version is ready to download.
    Available(String),
    /// Downloading, with the share done when the size is known.
    Downloading(Option<u8>),
    /// Downloaded and verified; HyperMe restarts into it.
    Installing,
    /// The check or the install failed; the reason is short and never holds a key.
    Failed(String),
}

/// One line of the menu: its text and whether a click does anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub text: String,
    pub enabled: bool,
}

/// What a click on the line asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Check,
    Install,
    Nothing,
}

/// The menu line for a state.
pub fn line(state: &State) -> Line {
    let (text, enabled) = match state {
        State::Idle => ("Check for updates…".to_owned(), true),
        State::Checking => ("Checking for updates…".to_owned(), false),
        State::UpToDate => (
            format!("HyperMe {} is up to date", env!("CARGO_PKG_VERSION")),
            true,
        ),
        State::Available(version) => (format!("Install HyperMe {version} and restart"), true),
        State::Downloading(Some(percent)) => (format!("Downloading update… {percent}%"), false),
        State::Downloading(None) => ("Downloading update…".to_owned(), false),
        State::Installing => ("Installing update…".to_owned(), false),
        State::Failed(why) => (format!("Update failed: {why}. Try again"), true),
    };
    Line { text, enabled }
}

/// What a click does in a state. A click on "up to date" or a failure checks again.
pub fn on_click(state: &State) -> Action {
    match state {
        State::Idle | State::UpToDate | State::Failed(_) => Action::Check,
        State::Available(_) => Action::Install,
        State::Checking | State::Downloading(_) | State::Installing => Action::Nothing,
    }
}

/// The share downloaded so far, from the bytes so far and the total when the server sent one.
pub fn percent(done: u64, total: Option<u64>) -> Option<u8> {
    let total = total.filter(|t| *t > 0)?;
    let share = u128::from(done.min(total)) * 100 / u128::from(total);
    Some(u8::try_from(share).unwrap_or(100))
}

/// A short reason for the menu, from the updater's error text: the first line, at most 60
/// characters, so a long error never widens the menu.
pub fn short_reason(error: &str) -> String {
    let first = error.lines().next().unwrap_or("").trim();
    if first.is_empty() {
        return "unknown error".to_owned();
    }
    let mut chars = first.chars();
    let short: String = chars.by_ref().take(60).collect();
    if chars.next().is_some() {
        format!("{short}…")
    } else {
        short
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AC-1: the menu offers a check, and a click on it checks.
    #[test]
    fn idle_offers_a_check() {
        let line = line(&State::Idle);
        assert_eq!(line.text, "Check for updates…");
        assert!(line.enabled);
        assert_eq!(on_click(&State::Idle), Action::Check);
    }

    /// AC-2: while checking, downloading or installing, the line is disabled and a click does
    /// nothing, so one click never starts two downloads.
    #[test]
    fn busy_states_ignore_clicks() {
        for state in [
            State::Checking,
            State::Downloading(Some(40)),
            State::Downloading(None),
            State::Installing,
        ] {
            assert!(!line(&state).enabled, "{state:?}");
            assert_eq!(on_click(&state), Action::Nothing, "{state:?}");
        }
        assert_eq!(
            line(&State::Downloading(Some(40))).text,
            "Downloading update… 40%"
        );
    }

    /// AC-3: an update is installed only after a second click that names the version.
    #[test]
    fn an_update_waits_for_a_click() {
        let state = State::Available("0.2.0".into());
        assert_eq!(line(&state).text, "Install HyperMe 0.2.0 and restart");
        assert_eq!(on_click(&state), Action::Install);
    }

    /// AC-4: "up to date" names the running version; a failure says why; both check again.
    #[test]
    fn up_to_date_and_failure_check_again() {
        let up = line(&State::UpToDate);
        assert!(up.text.contains(env!("CARGO_PKG_VERSION")));
        assert_eq!(on_click(&State::UpToDate), Action::Check);
        let failed = State::Failed("no network".into());
        assert_eq!(line(&failed).text, "Update failed: no network. Try again");
        assert_eq!(on_click(&failed), Action::Check);
    }

    #[test]
    fn percent_needs_a_total_and_stays_in_range() {
        assert_eq!(percent(50, Some(200)), Some(25));
        assert_eq!(percent(300, Some(200)), Some(100));
        assert_eq!(percent(10, None), None);
        assert_eq!(percent(10, Some(0)), None);
        assert_eq!(percent(u64::MAX, Some(u64::MAX)), Some(100));
    }

    #[test]
    fn reasons_are_short_and_one_line() {
        assert_eq!(short_reason("timed out\nmore detail"), "timed out");
        assert_eq!(short_reason("  "), "unknown error");
        let long = "x".repeat(80);
        let short = short_reason(&long);
        assert_eq!(short.chars().count(), 61);
        assert!(short.ends_with('…'));
    }

    /// AC-5: updates come only over HTTPS from the newest GitHub release, are verified against
    /// a minisign public key, and the signature must name the version announced, so an old
    /// signed build cannot be passed off as a new one.
    #[test]
    fn updater_config_is_locked_down() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json");
        let updater = &conf["plugins"]["updater"];
        assert_eq!(
            updater["endpoints"],
            serde_json::json!([
                "https://github.com/AndresHG/hyperme/releases/latest/download/latest.json"
            ])
        );
        assert_eq!(updater["requireSignedVersion"], true);
        assert!(updater.get("dangerousInsecureTransportProtocol").is_none());
        assert!(updater.get("allowDowngrades").is_none());
        let pubkey = updater["pubkey"].as_str().expect("pubkey");
        let decoded = decode_base64(pubkey).expect("pubkey is base64");
        let text = String::from_utf8(decoded).expect("pubkey is text");
        assert!(
            text.starts_with("untrusted comment: minisign public key"),
            "{text}"
        );
        // The version comes from Cargo.toml only, so the two can never disagree.
        assert!(conf.get("version").is_none());
    }

    /// Standard base64, enough for the test; no crate for it.
    fn decode_base64(text: &str) -> Option<Vec<u8>> {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut bits = 0u32;
        let mut count = 0;
        let mut out = Vec::new();
        for byte in text.trim().bytes().filter(|b| *b != b'=') {
            let value = ALPHABET.iter().position(|a| *a == byte)?;
            bits = (bits << 6) | u32::try_from(value).ok()?;
            count += 6;
            if count >= 8 {
                count -= 8;
                out.push(u8::try_from((bits >> count) & 0xff).ok()?);
            }
        }
        Some(out)
    }
}
