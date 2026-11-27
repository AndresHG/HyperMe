//! The window rule (docs/DESIGN.md, "Windows"): every normal window keeps native chrome, so macOS
//! draws its rounded corners, shadow and traffic lights; only the pill may be transparent. This
//! replaces the old check that Settings had `.shadow(false)`: a transparent window with a painted
//! card got a square shadow from macOS twice (27 November 2026).

use std::path::Path;

/// A window builder found in the app's source: the file, the label and the builder chain up to
/// its `.build()`.
struct Builder {
    file: String,
    label: String,
    chain: String,
    panel: bool,
}

fn builders() -> Vec<Builder> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        for (needle, panel) in [
            ("WebviewWindowBuilder::new(", false),
            ("PanelBuilder::<", true),
        ] {
            for (at, _) in text.match_indices(needle) {
                let rest = &text[at..];
                let end = rest.find(".build()").expect("a builder ends in .build()");
                let chain = rest[..end].to_owned();
                // The label is the second argument of `new(app, LABEL, …)`.
                let args = &chain[chain.find("::new(").unwrap() + 6..];
                let label = args.split([',', ')']).nth(1).unwrap().trim().to_owned();
                found.push(Builder {
                    file: file.clone(),
                    label,
                    chain,
                    panel,
                });
            }
        }
    }
    found
}

#[test]
fn only_the_pill_is_transparent_and_every_window_has_native_chrome() {
    let all = builders();
    let labels: Vec<&str> = all.iter().map(|b| b.label.as_str()).collect();
    for expected in ["HOME", "WALKTHROUGH", "PILL"] {
        assert!(
            labels.contains(&expected),
            "no builder for {expected}: {labels:?}"
        );
    }
    for b in &all {
        let at = format!("{} ({})", b.label, b.file);
        if b.panel {
            assert_eq!(b.label, "PILL", "{at}: only the pill is a panel");
            continue;
        }
        for banned in [
            ".transparent(true)",
            ".decorations(false)",
            ".shadow(false)",
        ] {
            assert!(
                !b.chain.contains(banned),
                "{at}: {banned} fakes window chrome"
            );
        }
        assert!(
            b.chain.contains("TitleBarStyle::Overlay") && b.chain.contains(".hidden_title(true)"),
            "{at}: a normal window uses the overlay title bar with a hidden title"
        );
    }
}

/// Settings is a sheet inside Home's page, not a window of its own, so its rounded corners sit
/// inside Home's native frame (V0.1 redesign).
#[test]
fn settings_is_not_a_window() {
    assert!(!builders().iter().any(|b| b.label.contains("SETTINGS")));
    let home = include_str!("../../ui/home.html");
    assert!(home.contains("id=\"sheet\"") && home.contains("settings.html"));
    let settings = include_str!("../../ui/settings.html");
    assert!(!settings.contains("html { background: transparent"));
}
