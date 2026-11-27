//! Slice 9 of specs/97-google-connectors: a draft is created only after Enter on its preview;
//! Esc leaves nothing (R-23, AC-12 against the fake). Tests cite
//! `specs/97-google-connectors/spec.md`. No test calls Google or needs a key.

mod fake_mcp;
mod fake_oauth;

use std::sync::{Arc, Mutex};

use fake_mcp::{FakeMcp, READ_TOOL, WRITE_TOOL};
use fake_oauth::FakeOAuth;
use hyperme_core::actions::Preview;
use hyperme_core::config::Connectors;
use hyperme_core::connector_settings::{self as cs, AccountEntry};
use hyperme_core::connectors::MemoryRequestLog;
use hyperme_core::secrets::MemorySecretStore;
use hyperme_providers::actions::{ActionError, Confirmer, Decision, run_action};
use hyperme_providers::mcp::{AccountAuth, RemoteServer};
use hyperme_providers::oauth::{Browser, DEFAULT_LISTEN_TIMEOUT, OAuthClient, OAuthConfig};
use serde_json::json;
use time::{Date, Month};

#[derive(Debug)]
struct AllowingBrowser;

impl Browser for AllowingBrowser {
    fn open(&self, url: &str) -> Result<(), String> {
        let url = url.to_owned();
        tokio::spawn(async move {
            let _ = reqwest::get(url).await;
        });
        Ok(())
    }
}

/// The preview card: answers with `decision` and keeps what it was shown.
struct Card {
    decision: Decision,
    shown: Mutex<Vec<Preview>>,
}

impl Confirmer for Card {
    async fn confirm(&self, preview: &Preview) -> Decision {
        self.shown.lock().unwrap().push(preview.clone());
        self.decision
    }
}

fn card(decision: Decision) -> Card {
    Card {
        decision,
        shown: Mutex::new(Vec::new()),
    }
}

async fn server() -> (FakeMcp, RemoteServer) {
    let oauth = FakeOAuth::start().await;
    let accept = {
        let oauth = oauth.clone();
        Arc::new(move |token: &str| oauth.issued_access_tokens().iter().any(|t| t == token))
    };
    let mcp = FakeMcp::start(accept).await;
    let client = Arc::new(OAuthClient::new(OAuthConfig {
        auth_url: oauth.auth_url(),
        token_url: oauth.token_url(),
        revoke_url: None,
        client_id: "fake-client-id".to_owned(),
        client_secret: None,
        extra_auth_params: Vec::new(),
    }));
    let store = Arc::new(MemorySecretStore::default());
    let first = client
        .sign_in(
            &AllowingBrowser,
            &*store,
            "google:work",
            &["mail"],
            DEFAULT_LISTEN_TIMEOUT,
        )
        .await
        .unwrap();
    let auth = Arc::new(AccountAuth::new(client, store, "google:work").with_token(first));
    let remote = RemoteServer::new(
        "gmail",
        mcp.url(),
        auth,
        Arc::new(MemoryRequestLog::default()),
        Arc::new(|| Date::from_calendar_date(2026, Month::November, 27).unwrap()),
    );
    (mcp, remote)
}

fn account(features: &[&str]) -> AccountEntry {
    AccountEntry {
        connector: "google".to_owned(),
        label: "work".to_owned(),
        enabled: true,
        features: features.iter().map(|f| (*f).to_owned()).collect(),
    }
}

fn draft() -> serde_json::Value {
    json!({ "to": "someone@example.test", "subject": "Thursday", "body": "Synthetic draft." })
}

/// R-23, AC-12 (fake): Enter on the preview creates the draft; the card showed what it would
/// create.
#[tokio::test]
async fn enter_creates_the_draft() {
    let (mcp, remote) = server().await;
    let registry = cs::registry(None, &Connectors::default()).unwrap();
    let google = registry.get("google").unwrap();
    let enter = card(Decision::Enter);
    let result = run_action(
        &remote,
        google,
        &account(&["gmail", "gmail.drafts"]),
        WRITE_TOOL,
        draft(),
        &enter,
    )
    .await
    .unwrap();
    assert!(result.is_some());
    assert_eq!(mcp.calls(), [WRITE_TOOL]);
    let shown = enter.shown.lock().unwrap();
    assert_eq!(shown[0].enter, "Save draft");
    assert!(
        shown[0]
            .lines
            .contains(&("Subject".to_owned(), "Thursday".to_owned()))
    );
}

/// R-23, AC-12 (fake): Esc leaves nothing: no call reaches the server.
#[tokio::test]
async fn esc_leaves_nothing() {
    let (mcp, remote) = server().await;
    let registry = cs::registry(None, &Connectors::default()).unwrap();
    let google = registry.get("google").unwrap();
    let result = run_action(
        &remote,
        google,
        &account(&["gmail", "gmail.drafts"]),
        WRITE_TOOL,
        draft(),
        &card(Decision::Esc),
    )
    .await
    .unwrap();
    assert!(result.is_none());
    assert!(mcp.calls().is_empty());
    assert!(mcp.auth_headers().is_empty(), "not even a connection");
}

/// G-2, R-15: without the drafts switch the card is not even shown; reads run without a card;
/// a sending tool never runs.
#[tokio::test]
async fn refused_actions_show_no_card() {
    let (mcp, remote) = server().await;
    let registry = cs::registry(None, &Connectors::default()).unwrap();
    let google = registry.get("google").unwrap();
    let enter = card(Decision::Enter);
    let error = run_action(
        &remote,
        google,
        &account(&["gmail"]),
        WRITE_TOOL,
        draft(),
        &enter,
    )
    .await
    .unwrap_err();
    assert!(matches!(error, ActionError::Refused(_)), "{error:?}");
    let error = run_action(
        &remote,
        google,
        &account(&["gmail", "gmail.drafts"]),
        "send_message",
        draft(),
        &enter,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("draft-only"), "{error}");
    assert!(enter.shown.lock().unwrap().is_empty());

    run_action(
        &remote,
        google,
        &account(&["calendar"]),
        READ_TOOL,
        json!({}),
        &enter,
    )
    .await
    .unwrap();
    assert!(enter.shown.lock().unwrap().is_empty(), "reads need no card");
    assert_eq!(mcp.calls(), [READ_TOOL]);
}
