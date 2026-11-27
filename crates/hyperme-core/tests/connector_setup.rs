//! Slice 6 of specs/97-google-connectors: the setup sheet's logic. Scope switches, labels,
//! several accounts, the "In production" check and the Privacy block. Tests cite
//! `specs/97-google-connectors/spec.md`.

use std::path::Path;

use hyperme_core::config::{Config, Connectors, DEFAULT_CONFIG};
use hyperme_core::connector_settings as cs;
use hyperme_core::connector_setup as setup;

fn connectors(extra: &str) -> Connectors {
    Config::parse(
        &format!("{DEFAULT_CONFIG}\n{extra}"),
        Path::new("config.toml"),
    )
    .unwrap()
    .connectors
}

fn on(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| (*id).to_owned()).collect()
}

/// R-14, AC-7: the sheet offers one switch per server and per optional scope, all on by default
/// (drafts too, per G-2). The consent asks only for the scopes switched on, and never for a
/// forbidden one.
#[test]
fn consent_asks_only_for_the_switched_on_scopes() {
    let registry = cs::registry(None, &Connectors::default()).unwrap();
    let google = registry.get("google").unwrap();
    let features = setup::features(google);
    let shown: Vec<(&str, &str, bool)> = features
        .iter()
        .map(|f| (f.id.as_str(), f.label.as_str(), f.on_by_default))
        .collect();
    assert_eq!(
        shown,
        [
            ("calendar", "Calendar: read your events", true),
            ("gmail", "Mail: read your inbox", true),
            ("gmail.drafts", "Mail: write drafts for you to send", true),
        ]
    );

    let calendar_only = setup::consent_scopes(google, &on(&["calendar"])).unwrap();
    assert_eq!(
        calendar_only,
        [
            "https://www.googleapis.com/auth/calendar.calendarlist.readonly",
            "https://www.googleapis.com/auth/calendar.events.readonly",
        ]
    );
    let mail = setup::consent_scopes(google, &on(&["gmail"])).unwrap();
    assert_eq!(mail, ["https://www.googleapis.com/auth/gmail.readonly"]);
    let all = setup::consent_scopes(google, &on(&["calendar", "gmail", "gmail.drafts"])).unwrap();
    assert!(all.contains(&"https://www.googleapis.com/auth/gmail.compose".to_owned()));
    for scopes in [&calendar_only, &mail, &all] {
        for banned in ["gmail.send", "gmail.modify", "https://mail.google.com/"] {
            assert!(scopes.iter().all(|s| !s.contains(banned)), "{banned}");
        }
    }

    assert!(
        setup::consent_scopes(google, &[]).is_err(),
        "nothing switched on"
    );
    assert!(
        setup::consent_scopes(google, &on(&["gmail.drafts"])).is_err(),
        "drafts need mail"
    );
    assert!(setup::consent_scopes(google, &on(&["photos"])).is_err());
}

/// R-14: a preset that would ask for a forbidden scope does not load.
#[test]
fn a_preset_cannot_ask_for_a_forbidden_scope() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("forbidden");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("bad.toml"),
        r#"
id = "bad"
name = "Bad"
kind = "oauth-mcp"
auth_url = "https://a.example/auth"
token_url = "https://a.example/token"
forbidden_scopes = ["send"]

[[servers]]
id = "mail"
url = "https://a.example/mcp"
scopes = ["read"]
optional_scopes = { more = ["send"] }
"#,
    )
    .unwrap();
    let error = cs::registry(Some(&dir), &Connectors::default())
        .unwrap_err()
        .to_string();
    assert!(error.contains("send"), "{error}");
}

/// R-11, AC-5: several accounts per connector, each with a label the user can say; "in my
/// work calendar" picks the work account, and with no label named the first account on.
#[test]
fn labels_pick_the_account() {
    let mut connectors = Connectors::default();
    setup::add_account(
        &mut connectors,
        "google",
        "personal",
        &on(&["calendar", "gmail"]),
    )
    .unwrap();
    setup::add_account(&mut connectors, "google", "Work", &on(&["calendar"])).unwrap();
    assert_eq!(
        connectors.accounts[1].label, "work",
        "labels are lowercased"
    );
    assert_eq!(connectors.accounts[1].features, ["calendar"]);

    let pick = |connectors: &Connectors, text: &str| {
        setup::account_for(text, "google", connectors).map(|a| a.label.clone())
    };
    assert_eq!(
        pick(&connectors, "what's in my work calendar today").as_deref(),
        Some("work")
    );
    assert_eq!(
        pick(&connectors, "Hyper, read my Personal inbox").as_deref(),
        Some("personal")
    );
    assert_eq!(
        pick(&connectors, "what's next").as_deref(),
        Some("personal")
    );
    assert_eq!(
        pick(&connectors, "networking event").as_deref(),
        Some("personal"),
        "whole words only"
    );

    connectors.accounts[0].enabled = false;
    assert_eq!(pick(&connectors, "what's next").as_deref(), Some("work"));
    assert_eq!(
        pick(&connectors, "my personal calendar"),
        None,
        "an account switched off is not used"
    );
}

/// R-11: a label is a short name, one per account; signing in again with the same label
/// replaces that account's switches.
#[test]
fn labels_are_checked() {
    let mut connectors = Connectors::default();
    for bad in ["", "two words", "😀", "a-very-long-label-that-nobody-says"] {
        assert!(
            setup::add_account(&mut connectors, "google", bad, &on(&["calendar"])).is_err(),
            "{bad:?}"
        );
    }
    setup::add_account(&mut connectors, "google", "home", &on(&["calendar"])).unwrap();
    setup::add_account(&mut connectors, "google", "home", &on(&["gmail"])).unwrap();
    assert_eq!(connectors.accounts.len(), 1);
    assert_eq!(connectors.accounts[0].features, ["gmail"]);
}

/// R-12: disconnecting removes the account from the config, and its tool switches stay with
/// the connector.
#[test]
fn removing_an_account_leaves_the_others() {
    let mut connectors = connectors(
        "[[connectors.accounts]]\nconnector = \"google\"\nlabel = \"personal\"\n\n[[connectors.accounts]]\nconnector = \"google\"\nlabel = \"work\"\n",
    );
    setup::remove_account(&mut connectors, "google:work").unwrap();
    assert_eq!(connectors.accounts.len(), 1);
    assert!(setup::remove_account(&mut connectors, "google:work").is_err());
    let text = cs::write_connectors(DEFAULT_CONFIG, &connectors).unwrap();
    assert!(!text.contains("label = \"work\""));
    assert!(text.contains("label = \"personal\""));
}

/// R-10a, AC-4a (fake part): a client whose refresh tokens expire within a week is a project
/// left in "Testing"; one with no expiry, or a long one, is not.
#[test]
fn a_week_long_refresh_token_means_testing() {
    assert!(setup::in_testing(Some(604_799)));
    assert!(setup::in_testing(Some(3_600)));
    assert!(!setup::in_testing(None));
    assert!(!setup::in_testing(Some(180 * 86_400)));
}

/// R-24: the Privacy block names each connected account, what is read, where it goes and
/// how long it is kept.
#[test]
fn privacy_names_each_account_and_what_is_read() {
    let connectors = connectors(
        "[[connectors.accounts]]\nconnector = \"google\"\nlabel = \"personal\"\nfeatures = [\"calendar\", \"gmail\"]\n\n[[connectors.accounts]]\nconnector = \"google\"\nlabel = \"work\"\nfeatures = [\"calendar\"]\n",
    );
    let registry = cs::registry(None, &connectors).unwrap();
    let block = setup::privacy(&registry, &connectors);
    assert_eq!(block.accounts.len(), 2);
    assert_eq!(block.accounts[0].title, "Google · personal");
    assert_eq!(
        block.accounts[0].reads,
        ["Calendar: read your events", "Mail: read your inbox"]
    );
    assert_eq!(block.accounts[1].reads, ["Calendar: read your events"]);
    assert!(block.where_it_goes.contains("language model provider"));
    assert!(block.kept.contains("14 days"));
    assert!(block.kept.contains("never"), "{}", block.kept);
}

/// #216: an empty Label (its "personal" is placeholder text) becomes "personal", and a label that
/// can't be used is refused, both before the browser opens, so no unlabelled sign-in is stored.
#[test]
fn an_empty_label_becomes_personal_before_sign_in() {
    use hyperme_core::connector_setup::account_label;
    assert_eq!(account_label("").unwrap(), "personal");
    assert_eq!(account_label("   ").unwrap(), "personal");
    assert_eq!(account_label(" Work ").unwrap(), "work");
    assert!(account_label("my work").is_err());
    assert!(account_label("1st").is_err());
}
