//! The design-token lint (docs/DESIGN.md, "Checks"): pages in `app/ui` take sizes, fonts and
//! colours from `tokens.css` through `var(--…)`, never as raw values. A page joins `MIGRATED` when
//! it moves to the tokens, so the check only ever grows.

use std::path::{Path, PathBuf};

/// Files in `app/ui` held to the tokens. Each page is added as it moves onto `tokens.css`.
const MIGRATED: &[&str] = &[
    "components.css",
    "dictionary.css",
    "home.html",
    "pill.html",
    "settings.html",
];

/// Properties whose lengths must be tokens (`0`, `auto` and keywords pass).
const SIZED: &[&str] = &[
    "border-radius",
    "border-top-left-radius",
    "border-top-right-radius",
    "border-bottom-left-radius",
    "border-bottom-right-radius",
    "padding",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "padding-inline",
    "padding-block",
    "margin",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "margin-inline",
    "margin-block",
    "gap",
    "row-gap",
    "column-gap",
    "font-size",
    "font-family",
];

/// Properties that may mix lengths with a colour; only the colour must be a token.
const COLOURED: &[&str] = &[
    "color",
    "background",
    "background-color",
    "border",
    "border-top",
    "border-right",
    "border-bottom",
    "border-left",
    "border-color",
    "outline",
    "outline-color",
    "box-shadow",
    "fill",
    "stroke",
    "caret-color",
    "accent-color",
    "text-decoration-color",
];

const NAMED_COLOURS: &[&str] = &[
    "white",
    "black",
    "red",
    "green",
    "blue",
    "gray",
    "grey",
    "orange",
    "yellow",
    "purple",
    "violet",
    "canvas",
    "canvastext",
    "buttonface",
    "buttontext",
    "field",
    "fieldtext",
    "highlight",
    "graytext",
];

fn ui_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui")
}

/// The CSS of a file: a stylesheet as is; for HTML, its `<style>` blocks and `style="…"`
/// attributes.
fn css_of(name: &str, text: &str) -> String {
    if name.ends_with(".css") {
        return text.to_owned();
    }
    let mut css = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("<style") {
        let Some(open_end) = rest[start..].find('>') else {
            break;
        };
        let body = &rest[start + open_end + 1..];
        let end = body.find("</style>").unwrap_or(body.len());
        css.push_str(&body[..end]);
        css.push('\n');
        rest = &body[end..];
    }
    let mut rest = text;
    while let Some(at) = rest.find(" style=\"") {
        let body = &rest[at + 8..];
        let end = body.find('"').unwrap_or(body.len());
        css.push_str(&body[..end]);
        css.push_str(";\n");
        rest = &body[end..];
    }
    css
}

fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        match rest[start + 2..].find("*/") {
            Some(end) => rest = &rest[start + 2 + end + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

fn is_raw_colour(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if [
        "rgb(", "rgba(", "hsl(", "hsla(", "hwb(", "lab(", "lch(", "oklch(",
    ]
    .iter()
    .any(|f| lower.contains(f))
    {
        return true;
    }
    let hex = lower.split('#').skip(1).any(|after| {
        let digits = after.chars().take_while(char::is_ascii_hexdigit).count();
        matches!(digits, 3 | 4 | 6 | 8)
    });
    hex || lower
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .any(|word| NAMED_COLOURS.contains(&word))
}

/// A sized value may hold tokens, keywords and plain numbers (as in `calc(-1 * var(--space-2))`),
/// but no length or percentage of its own.
fn is_raw_size(value: &str) -> bool {
    let mut rest = value.to_owned();
    while let Some(at) = rest.find("var(") {
        let end = rest[at..].find(')').map_or(rest.len(), |e| at + e + 1);
        rest.replace_range(at..end, " ");
    }
    let bytes = rest.as_bytes();
    (0..bytes.len()).any(|i| {
        bytes[i].is_ascii_digit()
            && ["px", "em", "rem", "pt", "%", "vh", "vw", "ch"]
                .iter()
                .any(|u| rest[i + 1..].starts_with(u))
    })
}

/// `property: value` declarations that use a raw value where a token belongs.
fn violations(css: &str) -> Vec<String> {
    let css = strip_comments(css).replace(['{', '}'], ";");
    let mut found = Vec::new();
    for decl in css.split(';') {
        let Some((name, value)) = decl.split_once(':') else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim().trim_end_matches("!important").trim();
        if value.is_empty() {
            continue;
        }
        let raw = if name == "font" {
            // `font: inherit` passes; any other shorthand must name a token for size and family.
            value != "inherit" && !(value.contains("var(--text") && value.contains("var(--font"))
        } else if name == "font-family" {
            !value.contains("var(--font") && value != "inherit"
        } else if SIZED.contains(&name.as_str()) {
            is_raw_size(value)
        } else if COLOURED.contains(&name.as_str()) || name.starts_with("--") {
            is_raw_colour(value)
        } else {
            false
        };
        if raw {
            found.push(format!("{name}: {value}"));
        }
    }
    found
}

#[test]
fn migrated_pages_use_the_tokens() {
    let mut report = Vec::new();
    for name in MIGRATED {
        assert_ne!(*name, "tokens.css", "tokens.css is where raw values live");
        let text =
            std::fs::read_to_string(ui_dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        for v in violations(&css_of(name, &text)) {
            report.push(format!("{name}: {v}"));
        }
    }
    assert!(
        report.is_empty(),
        "raw values where a token belongs (docs/DESIGN.md):\n{}",
        report.join("\n")
    );
}

/// The lint itself: it must catch each kind of raw value and pass tokens and keywords.
#[test]
fn the_lint_catches_raw_values() {
    let bad = "a { padding: 7px 10px; border-radius: 8px; font-size: 13px; gap: 3px; \
               font-family: ui-rounded; color: #111; background: rgba(0,0,0,.1); \
               border: 1px solid white; box-shadow: 0 1px 2px black; --local: #fff; \
               font: 700 15px -apple-system; }";
    assert_eq!(violations(bad).len(), 11, "{:?}", violations(bad));
    let good = "/* padding: 3px */ a:hover { padding: var(--space-1) var(--space-2); margin: 0 auto; \
                border-radius: var(--radius-control); font: inherit; color: var(--color-label); \
                margin: calc(-1 * var(--space-2)) 0 0; \
                border: var(--border-width) solid var(--color-separator); gap: var(--space-2); \
                background: transparent; font-family: var(--font-system); \
                font: var(--weight-bold) var(--text-title3) var(--font-system); }";
    assert!(violations(good).is_empty(), "{:?}", violations(good));
    let html = "<style>a { gap: 5px }</style><b style=\"color: red\">x</b>";
    assert_eq!(violations(&css_of("x.html", html)).len(), 2);
}

/// The tokens file holds the whole scale, both appearances and the accessibility media queries,
/// and only the system font.
#[test]
fn the_tokens_hold_the_scale() {
    let tokens = std::fs::read_to_string(ui_dir().join("tokens.css")).unwrap();
    for token in [
        "--font-system: -apple-system, BlinkMacSystemFont",
        "--text-large-title: 26px",
        "--text-body: 13px",
        "--text-caption: 10px",
        "--space-1: 4px",
        "--space-8: 32px",
        "--radius-small: 5px",
        "--radius-control: 6px",
        "--radius-card: 10px",
        "--radius-sheet: 12px",
        "--radius-capsule: 999px",
        "--control-height: 24px",
        "@media (prefers-color-scheme: dark)",
        "@media (prefers-reduced-motion: reduce)",
        "@media (prefers-reduced-transparency: reduce)",
    ] {
        assert!(tokens.contains(token), "tokens.css lacks {token}");
    }
    assert!(!tokens.contains("ui-rounded") && !tokens.contains("@import"));
    // Spacing is the 4-point grid only.
    for line in tokens
        .lines()
        .filter(|l| l.trim_start().starts_with("--space-"))
    {
        let px: u32 = line
            .split(':')
            .nth(1)
            .unwrap()
            .trim()
            .trim_end_matches(';')
            .trim_end_matches("px")
            .parse()
            .unwrap();
        assert_eq!(px % 4, 0, "{line}");
    }
}
