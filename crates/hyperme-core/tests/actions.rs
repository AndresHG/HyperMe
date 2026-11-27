//! Slice 9 of specs/97-google-connectors: actions that create something outside the app go
//! through a preview and wait for Enter (R-23, R-5). Tests cite
//! `specs/97-google-connectors/spec.md`.

use hyperme_core::actions::{self, Gate};
use hyperme_core::config::Connectors;
use hyperme_core::connector_settings::{self as cs, AccountEntry};
use serde_json::json;

fn account(features: &[&str]) -> AccountEntry {
    AccountEntry {
        connector: "google".to_owned(),
        label: "work".to_owned(),
        enabled: true,
        features: features.iter().map(|f| (*f).to_owned()).collect(),
    }
}

/// R-23, G-2: creating a draft asks before it runs, and needs drafts switched on for the
/// account; reads run; what is off never runs.
#[test]
fn drafts_ask_before_and_need_the_drafts_switch() {
    let registry = cs::registry(None, &Connectors::default()).unwrap();
    let google = registry.get("google").unwrap();
    let with_drafts = account(&["calendar", "gmail", "gmail.drafts"]);
    let without = account(&["calendar", "gmail"]);

    assert_eq!(
        actions::gate(google, &with_drafts, "create_draft"),
        Ok(Gate::Confirm)
    );
    assert_eq!(
        actions::gate(google, &with_drafts, "list_events"),
        Ok(Gate::Run)
    );
    let error = actions::gate(google, &without, "create_draft").unwrap_err();
    assert!(
        error.contains("Mail: write drafts for you to send"),
        "{error}"
    );
    assert!(error.contains("sign in again"), "{error}");
    assert!(actions::gate(google, &with_drafts, "trash_thread").is_err());
}

/// R-23, `CLAUDE.md`: email is draft-only; a tool that sends is never run, whatever its name
/// looks like.
#[test]
fn no_tool_sends_mail() {
    let registry = cs::registry(None, &Connectors::default()).unwrap();
    let google = registry.get("google").unwrap();
    let all = account(&["calendar", "gmail", "gmail.drafts"]);
    for tool in ["send_message", "send_draft", "draft_send", "reply_and_send"] {
        let error = actions::gate(google, &all, tool).unwrap_err();
        assert!(error.contains("never"), "{tool}: {error}");
    }
}

/// R-23: the preview shows the draft's fields in the preset's order and words, so the user
/// sees what will be created before pressing Enter.
#[test]
fn the_preview_shows_what_will_be_created() {
    let registry = cs::registry(None, &Connectors::default()).unwrap();
    let google = registry.get("google").unwrap();
    let args = json!({
        "subject": "Thursday",
        "to": "someone@example.test",
        "body": "Synthetic draft text.",
        "threadId": "t1",
    });
    let preview = actions::preview(google, &account(&["gmail.drafts"]), "create_draft", &args);
    assert_eq!(preview.title, "New draft in Google · work");
    let lines: Vec<(&str, &str)> = preview
        .lines
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    assert_eq!(
        lines,
        [
            ("To", "someone@example.test"),
            ("Subject", "Thursday"),
            ("Text", "Synthetic draft text."),
        ]
    );
    assert_eq!(preview.enter, "Save draft");
}

/// R-22: a user switch cannot turn a sending tool on, so no job can be allowed one either.
#[test]
fn a_switch_cannot_turn_sending_on() {
    let mut connectors = Connectors::default();
    cs::set_tool(
        &mut connectors,
        "google",
        "send_message",
        hyperme_core::registry::ToolAccess::Read,
    )
    .unwrap();
    let registry = cs::registry(None, &connectors).unwrap();
    let google = registry.get("google").unwrap();
    assert_eq!(
        google.access("send_message"),
        hyperme_core::registry::ToolAccess::Off
    );
    assert!(google.job_allowlist(&["send_message".to_owned()]).is_err());
}
