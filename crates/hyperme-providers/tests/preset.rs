//! Slice 4 of specs/97-google-connectors, end to end: a preset file that points at the local
//! fake OAuth and MCP servers is enough to sign in and call a read tool, with no Rust change,
//! and a job's allowlist from that preset keeps write tools out. Tests cite
//! `specs/97-google-connectors/spec.md`. No test calls Google or needs a key.

mod fake_mcp;
mod fake_oauth;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fake_mcp::{FakeMcp, READ_TOOL, WRITE_TOOL};
use fake_oauth::FakeOAuth;
use hyperme_core::connectors::MemoryRequestLog;
use hyperme_core::registry::{Auth, Registry, Transport};
use hyperme_core::secrets::MemorySecretStore;
use hyperme_providers::mcp::{AccountAuth, ConnectorError, RemoteServer};
use hyperme_providers::oauth::{Browser, DEFAULT_LISTEN_TIMEOUT, OAuthClient, OAuthConfig};
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

fn temp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("preset")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// AC-1, AC-10: a test preset written as a file connects to the fake servers. Signing in uses
/// its endpoints and scopes, a job gets its read tools, and a write tool stays out.
#[tokio::test]
async fn a_preset_file_is_enough_to_connect_and_its_job_allowlist_keeps_writes_out() {
    let oauth = FakeOAuth::start().await;
    let accept = {
        let oauth = oauth.clone();
        Arc::new(move |token: &str| oauth.issued_access_tokens().iter().any(|t| t == token))
    };
    let mcp = FakeMcp::start(accept).await;
    let dir = temp_dir("fake");
    std::fs::write(
        dir.join("fake.toml"),
        format!(
            r#"
id = "fake"
name = "Fake"
kind = "oauth-mcp"
tools_verified = true
auth_url = "{auth}"
token_url = "{token}"
revoke_url = "{revoke}"
extra_auth_params = {{ access_type = "offline" }}

[[servers]]
id = "notes"
url = "{mcp}"
scopes = ["notes.read"]

[tools]
read = ["{READ_TOOL}"]
ask = ["{WRITE_TOOL}"]
"#,
            auth = oauth.auth_url(),
            token = oauth.token_url(),
            revoke = oauth.revoke_url(),
            mcp = mcp.url(),
        ),
    )
    .unwrap();

    let registry = Registry::load(Some(&dir), &[]).unwrap();
    let preset = registry.get("fake").unwrap();
    let Auth::OAuth(endpoints) = &preset.auth else {
        panic!("{:?}", preset.auth);
    };
    let server = preset.server("notes").unwrap();
    let Transport::Http(url) = &server.transport else {
        panic!("{:?}", server.transport);
    };
    let scopes: Vec<&str> = server.scopes.iter().map(String::as_str).collect();

    let client = Arc::new(OAuthClient::new(OAuthConfig::from_endpoints(
        endpoints,
        "fake-client-id",
        None,
    )));
    let store = Arc::new(MemorySecretStore::default());
    let account = "fake:personal";
    let first = client
        .sign_in(
            &AllowingBrowser,
            &*store,
            account,
            &scopes,
            DEFAULT_LISTEN_TIMEOUT,
        )
        .await
        .unwrap();
    let consent = &oauth.authorize_queries()[0];
    assert_eq!(consent["scope"], "notes.read");
    assert_eq!(consent["access_type"], "offline");

    let auth = Arc::new(AccountAuth::new(client, store, account).with_token(first));
    let remote = RemoteServer::new(
        &server.id,
        url,
        auth,
        Arc::new(MemoryRequestLog::default()),
        Arc::new(|| Date::from_calendar_date(2026, Month::November, 27).unwrap()),
    );

    assert!(preset.job_allowlist(&[WRITE_TOOL.to_owned()]).is_err());
    let allowlist = preset.job_allowlist(&[READ_TOOL.to_owned()]).unwrap();
    remote.call_tool(READ_TOOL, None, &allowlist).await.unwrap();
    let refused = remote.call_tool(WRITE_TOOL, None, &allowlist).await;
    assert!(
        matches!(&refused, Err(ConnectorError::NotAllowed(tool)) if tool == WRITE_TOOL),
        "{refused:?}"
    );
    assert_eq!(mcp.calls(), vec![READ_TOOL]);
}
