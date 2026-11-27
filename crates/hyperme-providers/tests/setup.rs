//! Slice 6 of specs/97-google-connectors: the setup sheet's sign-in, client check, the "In
//! production" check, several accounts and Disconnect, against the fake OAuth and MCP servers.
//! Tests cite `specs/97-google-connectors/spec.md`. No test calls Google or needs a key.

mod fake_mcp;
mod fake_oauth;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fake_mcp::{FakeMcp, READ_TOOL};
use fake_oauth::FakeOAuth;
use hyperme_core::config::Connectors;
use hyperme_core::connector_settings as cs;
use hyperme_core::connector_setup as core_setup;
use hyperme_core::connectors::{MemoryRequestLog, RequestLog};
use hyperme_core::registry::{Connector, Transport};
use hyperme_core::secrets::{MemorySecretStore, SecretStore};
use hyperme_providers::mcp::{AccountAuth, RemoteServer};
use hyperme_providers::oauth::{Browser, DEFAULT_LISTEN_TIMEOUT, OAuthError};
use hyperme_providers::setup::{self, ClientCredentials, SetupError};
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

fn day() -> Date {
    Date::from_calendar_date(2026, Month::November, 27).unwrap()
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("setup")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const CAL: &str = "https://example.test/auth/calendar.readonly";
const MAIL: &str = "https://example.test/auth/mail.readonly";
const DRAFTS: &str = "https://example.test/auth/mail.compose";

/// A Google-shaped preset pointing at the fakes: two servers, drafts as an optional scope, a
/// forbidden scope and a production URL.
fn preset(oauth: &FakeOAuth, mcp: &FakeMcp, name: &str) -> Connector {
    let dir = temp_dir(name);
    std::fs::write(
        dir.join("fakegoogle.toml"),
        format!(
            r#"
id = "fakegoogle"
name = "Fake Google"
kind = "oauth-mcp"
auth_url = "{auth}"
token_url = "{token}"
revoke_url = "{revoke}"
forbidden_scopes = ["https://example.test/auth/mail.send"]

[setup]
client_url = "https://example.test/client"
production_url = "https://example.test/audience"

[[servers]]
id = "calendar"
label = "Calendar: read your events"
url = "{mcp}"
scopes = ["{CAL}"]

[[servers]]
id = "mail"
label = "Mail: read your inbox"
url = "{mcp}"
scopes = ["{MAIL}"]
optional_scopes = {{ drafts = ["{DRAFTS}"] }}
optional_labels = {{ drafts = "Mail: write drafts for you to send" }}

[tools]
read = ["{READ_TOOL}"]
"#,
            auth = oauth.auth_url(),
            token = oauth.token_url(),
            revoke = oauth.revoke_url(),
            mcp = mcp.url(),
        ),
    )
    .unwrap();
    cs::registry(Some(&dir), &Connectors::default())
        .unwrap()
        .get("fakegoogle")
        .unwrap()
        .clone()
}

async fn fakes() -> (FakeOAuth, FakeMcp) {
    let oauth = FakeOAuth::start().await;
    let accept = {
        let oauth = oauth.clone();
        Arc::new(move |token: &str| oauth.issued_access_tokens().iter().any(|t| t == token))
    };
    let mcp = FakeMcp::start(accept).await;
    (oauth, mcp)
}

fn with_client(store: &MemorySecretStore) {
    setup::save_client(
        store,
        "fakegoogle",
        &ClientCredentials {
            client_id: "fake-client-id".to_owned(),
            client_secret: Some("fake-client-secret".to_owned()),
        },
    )
    .unwrap();
}

fn on(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| (*id).to_owned()).collect()
}

/// G-1: the client ID and secret are one keychain item, read back whole; its debug output
/// hides the secret. The check tells a known client from an unknown one without a sign-in.
#[tokio::test]
async fn the_client_is_saved_and_checked() {
    let (oauth, mcp) = fakes().await;
    let connector = preset(&oauth, &mcp, "client");
    let store = MemorySecretStore::default();
    assert!(matches!(
        setup::oauth_client(&connector, &store),
        Err(SetupError::NoClient)
    ));
    with_client(&store);
    let saved = setup::load_client(&store, "fakegoogle").unwrap().unwrap();
    assert_eq!(saved.client_id, "fake-client-id");
    assert!(!format!("{saved:?}").contains("fake-client-secret"));

    oauth.require_client("fake-client-id");
    setup::oauth_client(&connector, &store)
        .unwrap()
        .check_client()
        .await
        .unwrap();
    setup::save_client(
        &store,
        "fakegoogle",
        &ClientCredentials {
            client_id: "someone-else".to_owned(),
            client_secret: None,
        },
    )
    .unwrap();
    let error = setup::oauth_client(&connector, &store)
        .unwrap()
        .check_client()
        .await
        .unwrap_err();
    assert!(matches!(error, OAuthError::ClientRejected), "{error:?}");
}

/// AC-7: the consent asks for exactly the scopes switched on.
#[tokio::test]
async fn the_consent_holds_only_the_switched_on_scopes() {
    let (oauth, mcp) = fakes().await;
    let connector = preset(&oauth, &mcp, "scopes");
    let store = MemorySecretStore::default();
    with_client(&store);
    let client = setup::oauth_client(&connector, &store).unwrap();
    for (label, features, expected) in [
        ("one", on(&["calendar"]), CAL.to_owned()),
        ("two", on(&["mail"]), MAIL.to_owned()),
        (
            "three",
            on(&["calendar", "mail", "mail.drafts"]),
            format!("{CAL} {MAIL} {DRAFTS}"),
        ),
    ] {
        setup::sign_in_account(
            &client,
            &connector,
            &AllowingBrowser,
            &store,
            label,
            &features,
            DEFAULT_LISTEN_TIMEOUT,
        )
        .await
        .unwrap();
        let consent = oauth.authorize_queries().pop().unwrap();
        assert_eq!(consent["scope"], expected, "{label}");
        assert!(!consent["scope"].contains("mail.send"));
    }
}

/// R-10a, AC-4a (fake part): a client in "Testing" stops the setup with the step that fixes
/// it, and the week-long sign-in is revoked and not kept.
#[tokio::test]
async fn a_client_in_testing_stops_the_setup_with_the_fix() {
    let (oauth, mcp) = fakes().await;
    oauth.set_testing(true);
    let connector = preset(&oauth, &mcp, "testing");
    let store = MemorySecretStore::default();
    with_client(&store);
    let client = setup::oauth_client(&connector, &store).unwrap();
    let error = setup::sign_in_account(
        &client,
        &connector,
        &AllowingBrowser,
        &store,
        "personal",
        &on(&["calendar"]),
        DEFAULT_LISTEN_TIMEOUT,
    )
    .await
    .unwrap_err();
    let SetupError::ClientInTesting { fix_url } = &error else {
        panic!("{error:?}");
    };
    assert_eq!(fix_url.as_deref(), Some("https://example.test/audience"));
    assert!(error.to_string().contains("In production"));
    assert_eq!(store.get("fakegoogle:personal").unwrap(), None);
    assert_eq!(oauth.revoked(), oauth.issued_refresh_tokens());

    oauth.set_testing(false);
    setup::sign_in_account(
        &client,
        &connector,
        &AllowingBrowser,
        &store,
        "personal",
        &on(&["calendar"]),
        DEFAULT_LISTEN_TIMEOUT,
    )
    .await
    .unwrap();
    assert!(store.get("fakegoogle:personal").unwrap().is_some());
}

/// AC-5, R-11: two accounts with different labels both connect and both call their servers;
/// "in my work calendar" picks the work account.
#[tokio::test]
async fn two_labelled_accounts_both_work() {
    let (oauth, mcp) = fakes().await;
    let connector = preset(&oauth, &mcp, "two");
    let store = Arc::new(MemorySecretStore::default());
    with_client(&store);
    let client = Arc::new(setup::oauth_client(&connector, &*store).unwrap());
    let mut connectors = Connectors::default();
    let log = Arc::new(MemoryRequestLog::default());
    for label in ["personal", "work"] {
        let first = setup::sign_in_account(
            &client,
            &connector,
            &AllowingBrowser,
            &*store,
            label,
            &on(&["calendar"]),
            DEFAULT_LISTEN_TIMEOUT,
        )
        .await
        .unwrap();
        let account =
            core_setup::add_account(&mut connectors, &connector.id, label, &on(&["calendar"]))
                .unwrap();
        let auth =
            Arc::new(AccountAuth::new(client.clone(), store.clone(), &account).with_token(first));
        let Transport::Http(url) = &connector.server("calendar").unwrap().transport else {
            panic!();
        };
        let server = RemoteServer::new("calendar", url, auth, log.clone(), Arc::new(day));
        server
            .call_tool(READ_TOOL, None, &[READ_TOOL.to_owned()])
            .await
            .unwrap();
    }
    for account in ["fakegoogle:personal", "fakegoogle:work"] {
        assert!(store.get(account).unwrap().is_some(), "{account}");
        assert_eq!(
            log.usage(account, day(), "calendar").unwrap().calls,
            1,
            "{account}"
        );
    }
    let picked =
        core_setup::account_for("what is in my work calendar", "fakegoogle", &connectors).unwrap();
    assert_eq!(picked.id(), "fakegoogle:work");
}

/// AC-6, R-12: Disconnect calls the revoke endpoint with that account's token, removes its
/// keychain item and deletes its rows; the other account keeps everything.
#[tokio::test]
async fn disconnect_revokes_and_deletes_only_that_account() {
    let (oauth, mcp) = fakes().await;
    let connector = preset(&oauth, &mcp, "disconnect");
    let store = MemorySecretStore::default();
    with_client(&store);
    let client = setup::oauth_client(&connector, &store).unwrap();
    for label in ["personal", "work"] {
        setup::sign_in_account(
            &client,
            &connector,
            &AllowingBrowser,
            &store,
            label,
            &on(&["calendar"]),
            DEFAULT_LISTEN_TIMEOUT,
        )
        .await
        .unwrap();
    }
    let work_token = store.get("fakegoogle:work").unwrap().unwrap();
    let data = MemoryRequestLog::default();
    data.record("fakegoogle:work", day(), "calendar", 1)
        .unwrap();
    data.record("fakegoogle:personal", day(), "calendar", 1)
        .unwrap();

    setup::disconnect(&client, &store, &data, "fakegoogle:work")
        .await
        .unwrap();
    assert_eq!(oauth.revoked(), vec![work_token]);
    assert_eq!(store.get("fakegoogle:work").unwrap(), None);
    assert!(store.get("fakegoogle:personal").unwrap().is_some());
    assert_eq!(
        data.usage("fakegoogle:work", day(), "calendar")
            .unwrap()
            .calls,
        0
    );
    assert_eq!(
        data.usage("fakegoogle:personal", day(), "calendar")
            .unwrap()
            .calls,
        1
    );
}
