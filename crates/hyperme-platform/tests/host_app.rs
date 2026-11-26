//! Which app macOS gives HyperMe's permissions to. Run as a bare binary from a terminal, the
//! terminal is the "responsible" app, so the walkthrough must name it; run as HyperMe.app, the
//! permissions are HyperMe's own.

#![cfg(target_os = "macos")]

use hyperme_platform::permissions::{app_from_executable, host_app_in, own_bundle};

#[test]
fn app_names_come_from_the_bundle_path() {
    assert_eq!(
        app_from_executable("/Applications/iTerm.app/Contents/MacOS/iTerm2").as_deref(),
        Some("iTerm")
    );
    assert_eq!(
        app_from_executable("/System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal")
            .as_deref(),
        Some("Terminal")
    );
    assert_eq!(
        app_from_executable("/Applications/Ghostty.app/Contents/MacOS/ghostty").as_deref(),
        Some("Ghostty")
    );
    assert_eq!(
        app_from_executable("/Applications/Visual Studio Code.app/Contents/Frameworks/Code Helper.app/Contents/MacOS/Code Helper").as_deref(),
        Some("Visual Studio Code"),
        "a helper inside an app counts as the outer app"
    );
    assert_eq!(app_from_executable("/bin/zsh"), None);
    assert_eq!(app_from_executable("/usr/bin/login"), None);
}

/// The first ancestor that lives in an app bundle is the one macOS asks about.
#[test]
fn the_nearest_app_among_the_parents_is_the_host() {
    let parents = [
        "/Users/me/Github/hyperme/target/release/hyperme",
        "/Users/me/.cargo/bin/cargo",
        "-zsh",
        "/usr/bin/login",
        "/Users/me/Library/Application Support/iTerm2/iTermServer-3.7.9",
        "/Applications/iTerm.app/Contents/MacOS/iTerm2",
    ];
    assert_eq!(
        host_app_in(parents.iter().copied()).as_deref(),
        Some("iTerm")
    );
    assert_eq!(
        host_app_in(["/usr/bin/login", "/sbin/launchd"].into_iter()),
        None
    );
}

#[test]
fn a_bundled_hyperme_has_its_own_permissions() {
    assert!(own_bundle(
        "/Users/me/Applications/HyperMe.app/Contents/MacOS/hyperme"
    ));
    assert!(!own_bundle(
        "/Users/me/Github/hyperme/target/release/hyperme"
    ));
}
