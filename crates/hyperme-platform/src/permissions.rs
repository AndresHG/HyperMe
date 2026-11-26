//! The macOS permissions HyperMe needs, for Settings' setup check (docs/UX.md, "Setup check")
//! and the first-launch walkthrough: Accessibility to send the paste shortcut, and Input
//! Monitoring to see the hotkey. Both are read without prompting.

#[allow(unsafe_code)]
mod ffi {
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        /// True when this process may control the computer (Accessibility).
        pub safe fn AXIsProcessTrusted() -> u8;
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        /// True when this process may listen to keyboard events (Input Monitoring).
        pub safe fn CGPreflightListenEventAccess() -> u8;
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
