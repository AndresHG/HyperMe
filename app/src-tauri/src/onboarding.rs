//! The first-launch walkthrough (spec R-27, docs/UX.md "Onboarding"): five short screens in one
//! window, each tested before Next lights up. This file holds the rules, pure and tested; the
//! window and its checks are in `walkthrough.rs` and `onboarding.html`.

use serde::Deserialize;

/// The screens, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Step {
    Welcome,
    Microphone,
    /// Accessibility to paste, and Input Monitoring to see the hotkey.
    Permissions,
    Keys,
    Try,
}

#[cfg(test)]
const STEPS: [Step; 5] = [
    Step::Welcome,
    Step::Microphone,
    Step::Permissions,
    Step::Keys,
    Step::Try,
];

/// What has been tested so far. Each flag is set only by a real test, never by the user saying
/// so (R-27: "test each before saying it is ready").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct Checks {
    /// A live level above silence came from the microphone.
    pub microphone: bool,
    pub accessibility: bool,
    pub input_monitoring: bool,
    /// The speech key passed Test.
    pub speech_key: bool,
    /// The cleanup key passed Test.
    pub cleanup_key: bool,
    /// A dictation was pasted in the Try it box.
    pub tried: bool,
}

/// Whether Next (or Done, on the last screen) may light up.
pub fn can_continue(step: Step, checks: &Checks) -> bool {
    match step {
        Step::Welcome => true,
        Step::Microphone => checks.microphone,
        Step::Permissions => checks.accessibility && checks.input_monitoring,
        Step::Keys => checks.speech_key && checks.cleanup_key,
        Step::Try => checks.tried,
    }
}

/// The screen after `step`, if any.
#[cfg(test)]
fn next(step: Step) -> Option<Step> {
    let index = STEPS.iter().position(|s| *s == step)?;
    STEPS.get(index + 1).copied()
}

/// What HyperMe knows at launch without asking anything: the permissions and whether each key is
/// stored. The walkthrough opens when any is missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtLaunch {
    pub accessibility: bool,
    pub input_monitoring: bool,
    pub speech_key_saved: bool,
    pub cleanup_key_saved: bool,
}

pub fn should_open(at_launch: &AtLaunch) -> bool {
    !(at_launch.accessibility
        && at_launch.input_monitoring
        && at_launch.speech_key_saved
        && at_launch.cleanup_key_saved)
}

impl Step {
    /// The screen's name in the page and in the resume file.
    pub fn name(self) -> &'static str {
        match self {
            Step::Welcome => "welcome",
            Step::Microphone => "microphone",
            Step::Permissions => "permissions",
            Step::Keys => "keys",
            Step::Try => "try",
        }
    }

    pub fn from_name(name: &str) -> Option<Step> {
        [
            Step::Welcome,
            Step::Microphone,
            Step::Permissions,
            Step::Keys,
            Step::Try,
        ]
        .into_iter()
        .find(|s| s.name() == name.trim())
    }
}

/// The app bundle to reopen for "Relaunch HyperMe": `/Users/me/Applications/HyperMe.app` for
/// `/Users/me/Applications/HyperMe.app/Contents/MacOS/hyperme`. `None` for a bare binary, which
/// macOS ties to its terminal: then the terminal itself has to be quit and reopened.
pub fn bundle_of(executable: &std::path::Path) -> Option<std::path::PathBuf> {
    let text = executable.to_str()?;
    let end = text.find(".app/Contents/MacOS/")?;
    Some(std::path::PathBuf::from(&text[..end + ".app".len()]))
}

/// A level from the meter counts as the microphone working above this (0 to 1, on the meter's
/// dB scale: about -38 dBFS, quiet speech).
pub const HEARD_LEVEL: f32 = 0.25;

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Checks {
        Checks {
            microphone: true,
            accessibility: true,
            input_monitoring: true,
            speech_key: true,
            cleanup_key: true,
            tried: true,
        }
    }

    /// R-27: no screen after Welcome lets the user go on until its test has passed.
    #[test]
    fn each_screen_waits_for_its_test() {
        let none = Checks::default();
        assert!(can_continue(Step::Welcome, &none));
        for step in [Step::Microphone, Step::Permissions, Step::Keys, Step::Try] {
            assert!(
                !can_continue(step, &none),
                "{step:?} continued with nothing tested"
            );
            assert!(
                can_continue(step, &all()),
                "{step:?} blocked with everything tested"
            );
        }
    }

    /// Half a screen's tests are not enough.
    #[test]
    fn every_check_on_a_screen_is_needed() {
        let mut checks = all();
        checks.input_monitoring = false;
        assert!(!can_continue(Step::Permissions, &checks));
        let mut checks = all();
        checks.cleanup_key = false;
        assert!(!can_continue(Step::Keys, &checks));
        let mut checks = all();
        checks.accessibility = false;
        assert!(!can_continue(Step::Permissions, &checks));
    }

    /// After "Relaunch HyperMe", the walkthrough comes back on the screen it was on.
    #[test]
    fn steps_round_trip_through_their_names() {
        for step in STEPS {
            assert_eq!(Step::from_name(step.name()), Some(step));
        }
        assert_eq!(Step::from_name(" permissions\n"), Some(Step::Permissions));
        assert_eq!(Step::from_name("done"), None);
    }

    #[test]
    fn relaunch_reopens_the_bundle_not_the_binary() {
        use std::path::Path;
        assert_eq!(
            bundle_of(Path::new(
                "/Users/me/Applications/HyperMe.app/Contents/MacOS/hyperme"
            )),
            Some(Path::new("/Users/me/Applications/HyperMe.app").to_path_buf())
        );
        assert_eq!(
            bundle_of(Path::new("/Users/me/Github/hyperme/target/release/hyperme")),
            None
        );
    }

    #[test]
    fn five_screens_in_order() {
        assert_eq!(STEPS.len(), 5);
        assert_eq!(next(Step::Welcome), Some(Step::Microphone));
        assert_eq!(next(Step::Keys), Some(Step::Try));
        assert_eq!(next(Step::Try), None);
    }

    /// The walkthrough opens on a fresh install and whenever something it sets up is missing,
    /// and stays away once everything is in place.
    #[test]
    fn opens_only_when_something_is_missing() {
        let ready = AtLaunch {
            accessibility: true,
            input_monitoring: true,
            speech_key_saved: true,
            cleanup_key_saved: true,
        };
        assert!(!should_open(&ready));
        assert!(should_open(&AtLaunch {
            speech_key_saved: false,
            ..ready
        }));
        assert!(should_open(&AtLaunch {
            accessibility: false,
            ..ready
        }));
        assert!(should_open(&AtLaunch {
            input_monitoring: false,
            cleanup_key_saved: false,
            ..ready
        }));
    }
}
