//! The macOS permissions HyperMe needs, for Settings' setup check (docs/UX.md, "Setup check")
//! and the first-launch walkthrough: Accessibility to send the paste shortcut, and Input
//! Monitoring to see the hotkey. The checks never prompt; the `request_*` calls do, and make
//! macOS add HyperMe's row (or, for a bare binary, its terminal's row) to System Settings.

#[allow(unsafe_code)]
mod ffi {
    use core_foundation::dictionary::CFDictionaryRef;

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        /// True when this process may control the computer (Accessibility).
        pub safe fn AXIsProcessTrusted() -> u8;
        /// As above; with the prompt option set, shows macOS's prompt and adds the row.
        pub fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> u8;
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        /// True when this process may listen to keyboard events (Input Monitoring).
        pub safe fn CGPreflightListenEventAccess() -> u8;
        /// Asks macOS to show the Input Monitoring prompt; true if already allowed.
        pub safe fn CGRequestListenEventAccess() -> u8;
    }
}

/// Whether HyperMe may send the paste shortcut.
pub fn accessibility() -> bool {
    ffi::AXIsProcessTrusted() != 0
}

/// Whether HyperMe may listen for the hotkey.
pub fn input_monitoring() -> bool {
    ffi::CGPreflightListenEventAccess() != 0
}

/// Shows macOS's own Accessibility prompt, which also adds the app's row to System Settings
/// (switched off). Returns whether access is already allowed.
pub fn request_accessibility() -> bool {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;

    // The value of `kAXTrustedCheckOptionPrompt`.
    let key = CFString::from_static_string("AXTrustedCheckOptionPrompt");
    let options = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::true_value())]);
    // SAFETY: a valid, retained CFDictionary that outlives the call.
    #[allow(unsafe_code)]
    let trusted = unsafe { ffi::AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) };
    trusted != 0
}

/// Shows macOS's own Input Monitoring prompt (once; later calls only report the state).
pub fn request_input_monitoring() -> bool {
    ffi::CGRequestListenEventAccess() != 0
}

/// The System Settings page for a permission, to open with `open`.
pub fn settings_url(permission: &str) -> Option<&'static str> {
    match permission {
        "accessibility" => {
            Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        }
        "input-monitoring" => {
            Some("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent")
        }
        "microphone" => {
            Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
        }
        _ => None,
    }
}

/// The app a bundle path belongs to: "/Applications/iTerm.app/Contents/MacOS/iTerm2" is
/// "iTerm". A helper app inside another counts as the outer one.
pub fn app_from_executable(path: &str) -> Option<String> {
    let end = path.find(".app/")?;
    let start = path[..end].rfind('/').map_or(0, |i| i + 1);
    Some(path[start..end].to_owned())
}

/// The first of `ancestors` (from HyperMe up) that lives in an app bundle.
pub fn host_app_in<'a>(ancestors: impl Iterator<Item = &'a str>) -> Option<String> {
    ancestors.filter_map(app_from_executable).next()
}

/// Whether an executable runs from inside its own .app bundle.
pub fn own_bundle(executable: &str) -> bool {
    executable.contains(".app/Contents/MacOS/")
}

/// When HyperMe runs as a bare binary (from `cargo run` in a terminal), macOS gives its
/// permissions to the terminal it was started from: this returns that app's name, read by
/// walking up the parent processes. `None` when HyperMe runs as HyperMe.app, whose permissions
/// are its own, or when no app is found.
pub fn host_app() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    if own_bundle(&exe.to_string_lossy()) {
        return None;
    }
    let mut paths = Vec::new();
    let mut pid = std::process::id();
    // Process trees are shallow; the bound only guards against a loop.
    for _ in 0..32 {
        let out = std::process::Command::new("ps")
            .args(["-o", "ppid=,comm=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        let line = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        let (ppid, comm) = line.split_once(' ')?;
        paths.push(comm.trim().to_owned());
        pid = ppid.trim().parse().ok()?;
        if pid <= 1 {
            break;
        }
    }
    host_app_in(paths.iter().map(String::as_str))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_run_without_prompting() {
        // The values depend on the machine; the calls must simply return.
        let _ = accessibility();
        let _ = input_monitoring();
    }

    #[test]
    fn settings_pages_are_known() {
        assert!(
            settings_url("accessibility")
                .unwrap()
                .contains("Privacy_Accessibility")
        );
        assert!(
            settings_url("input-monitoring")
                .unwrap()
                .contains("Privacy_ListenEvent")
        );
        assert!(settings_url("camera").is_none());
    }
}
