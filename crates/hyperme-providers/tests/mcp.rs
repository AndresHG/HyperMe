//! Slice 3 of specs/97-google-connectors: the remote MCP client with accounts, against a local
//! fake MCP server and the fake OAuth server. Tests cite the acceptance criteria in
//! `specs/97-google-connectors/spec.md`. No test calls Google or needs a key.

mod fake_mcp;
mod fake_oauth;

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use fake_mcp::{FakeMcp, READ_TOOL, WRITE_TOOL};
use fake_oauth::FakeOAuth;
use hyperme_core::connectors::{MemoryRequestLog, RequestLog};
use hyperme_core::secrets::MemorySecretStore;
use hyperme_providers::mcp::{AccountAuth, ConnectorError, RemoteServer};
use hyperme_providers::oauth::{Browser, DEFAULT_LISTEN_TIMEOUT, OAuthClient, OAuthConfig};
use time::{Date, Month};

const ACCOUNT: &str = "google:personal";
const SERVER: &str = "gmail";

fn day() -> Date {
    Date::from_calendar_date(2026, Month::November, 27).unwrap()
}

/// Follows the consent URL as a user who clicks "Allow".
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

/// Everything one test needs: both fakes, a signed-in account and a server client.
struct World {
    oauth: FakeOAuth,
    mcp: FakeMcp,
    /// Access tokens the MCP server refuses although the OAuth server issued them, as when a
    /// token expires early at the provider.
    refused: Arc<Mutex<HashSet<String>>>,
    log: Arc<MemoryRequestLog>,
    auth: Arc<AccountAuth>,
    server: RemoteServer,
}

impl World {
    /// Signs in with access tokens that live `ttl` seconds at the fake OAuth server.
    async fn new(ttl: i64) -> Self {
        let oauth = FakeOAuth::start().await;
        oauth.set_access_ttl(ttl);
        let refused = Arc::new(Mutex::new(HashSet::new()));
        let accept = {
            let (oauth, refused) = (oauth.clone(), refused.clone());
            Arc::new(move |token: &str| {
                oauth.issued_access_tokens().iter().any(|t| t == token)
                    && !refused.lock().unwrap().contains(token)
            })
        };
        let mcp = FakeMcp::start(accept).await;
        let client = Arc::new(OAuthClient::new(OAuthConfig {
            auth_url: oauth.auth_url(),
            token_url: oauth.token_url(),
            revoke_url: Some(oauth.revoke_url()),
            client_id: "fake-client-id".to_owned(),
            client_secret: None,
            extra_auth_params: Vec::new(),
        }));
        let store = Arc::new(MemorySecretStore::default());
        let first = client
            .sign_in(
                &AllowingBrowser,
                &*store,
                ACCOUNT,
                &["mail.read"],
                DEFAULT_LISTEN_TIMEOUT,
            )
            .await
            .unwrap();
        let auth = Arc::new(AccountAuth::new(client, store, ACCOUNT).with_token(first));
        let log = Arc::new(MemoryRequestLog::default());
        let server = RemoteServer::new(SERVER, mcp.url(), auth.clone(), log.clone(), Arc::new(day));
        World {
            oauth,
            mcp,
            refused,
            log,
            auth,
            server,
        }
    }

    fn refuse_current_tokens(&self) {
        let mut refused = self.refused.lock().unwrap();
        refused.extend(self.oauth.issued_access_tokens());
    }

    fn calls_counted(&self) -> u32 {
        self.log.usage(ACCOUNT, day(), SERVER).unwrap().calls
    }
}

fn allow(tools: &[&str]) -> Vec<String> {
    tools.iter().map(|t| (*t).to_owned()).collect()
}

fn text(result: &rmcp::model::CallToolResult) -> String {
    serde_json::to_value(&result.content).unwrap().to_string()
}

/// AC-1: the client reaches a server known only by its URL (data, as in a preset), sends the
/// account's bearer token, lists its tools and calls one.
#[tokio::test]
async fn lists_and_calls_tools_with_the_accounts_bearer_token() {
    let world = World::new(3600).await;
    let tools: Vec<String> = world
        .server
        .list_tools()
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.name.to_string())
        .collect();
    assert_eq!(tools, vec![READ_TOOL, WRITE_TOOL]);

    let result = world
        .server
        .call_tool(READ_TOOL, None, &allow(&[READ_TOOL]))
        .await
        .unwrap();
    assert!(text(&result).contains("3 synthetic events"), "{result:?}");
    assert_eq!(world.mcp.calls(), vec![READ_TOOL]);

    let bearer = format!("Bearer {}", world.oauth.issued_access_tokens()[0]);
    let headers = world.mcp.auth_headers();
    assert!(!headers.is_empty());
    assert!(headers.iter().all(|h| *h == bearer), "{headers:?}");
    assert_eq!(world.mcp.unauthorized(), 0);
}

/// AC-4: when the server refuses the access token (401) mid-session, the client refreshes it
/// without asking the user and retries the call once, which succeeds.
#[tokio::test]
async fn a_401_refreshes_the_token_and_retries_once() {
    let world = World::new(3600).await;
    world
        .server
        .call_tool(READ_TOOL, None, &allow(&[READ_TOOL]))
        .await
        .unwrap();
    world.refuse_current_tokens();

    let result = world
        .server
        .call_tool(READ_TOOL, None, &allow(&[READ_TOOL]))
        .await
        .unwrap();
    assert!(text(&result).contains("3 synthetic events"));
    assert_eq!(world.oauth.refresh_calls(), 1);
    assert!(world.mcp.unauthorized() >= 1);
    assert_eq!(world.mcp.calls(), vec![READ_TOOL, READ_TOOL]);
    let fresh = format!("Bearer {}", world.oauth.issued_access_tokens()[1]);
    assert_eq!(world.mcp.auth_headers().last(), Some(&fresh));
}

/// AC-4: an access token that has expired on our side is refreshed before the request, so the
/// server never has to refuse it.
#[tokio::test]
async fn an_expired_token_is_refreshed_before_the_call() {
    let world = World::new(0).await;
    world
        .server
        .call_tool(READ_TOOL, None, &allow(&[READ_TOOL]))
        .await
        .unwrap();
    assert_eq!(world.oauth.refresh_calls(), 1);
    assert_eq!(world.mcp.unauthorized(), 0);
}

/// AC-4: with the refresh token revoked, the call ends in the distinct "sign in again" error,
/// after one refresh attempt and no loop.
#[tokio::test]
async fn a_revoked_refresh_token_asks_to_sign_in_again() {
    let world = World::new(3600).await;
    world.refuse_current_tokens();
    world.oauth.revoke_all_refresh_tokens();

    let error = world
        .server
        .call_tool(READ_TOOL, None, &allow(&[READ_TOOL]))
        .await
        .unwrap_err();
    assert!(matches!(error, ConnectorError::SignInAgain), "{error:?}");
    assert_eq!(world.oauth.refresh_calls(), 1);
    assert!(world.mcp.calls().is_empty());
}

/// AC-4 (#121): two calls that find the token expired at the same time share one refresh.
#[tokio::test]
async fn concurrent_calls_share_one_refresh() {
    let world = World::new(0).await;
    world.oauth.set_access_ttl(3600);
    let other = RemoteServer::new(
        "calendar",
        world.mcp.url(),
        world.auth.clone(),
        world.log.clone(),
        Arc::new(day),
    );
    let allowed = allow(&[READ_TOOL]);
    let (a, b) = tokio::join!(
        world.server.call_tool(READ_TOOL, None, &allowed),
        other.call_tool(READ_TOOL, None, &allowed),
    );
    a.unwrap();
    b.unwrap();
    assert_eq!(world.oauth.refresh_calls(), 1);
}

/// AC-10: a caller limited to read tools cannot call a write tool. The call is refused before
/// anything is sent, and it is not counted.
#[tokio::test]
async fn a_write_tool_outside_the_allowlist_is_refused() {
    let world = World::new(3600).await;
    let error = world
        .server
        .call_tool(WRITE_TOOL, None, &allow(&[READ_TOOL]))
        .await
        .unwrap_err();
    assert!(
        matches!(&error, ConnectorError::NotAllowed(tool) if tool == WRITE_TOOL),
        "{error:?}"
    );
    assert!(world.mcp.calls().is_empty());
    assert!(world.mcp.auth_headers().is_empty());
    assert_eq!(world.calls_counted(), 0);
}

/// R-20: each request to the server is counted per account, day and server.
#[tokio::test]
async fn requests_are_counted_per_account_day_and_server() {
    let world = World::new(3600).await;
    world.server.list_tools().await.unwrap();
    for _ in 0..2 {
        world
            .server
            .call_tool(READ_TOOL, None, &allow(&[READ_TOOL]))
            .await
            .unwrap();
    }
    assert_eq!(world.calls_counted(), 3);
    assert_eq!(
        world.log.usage(ACCOUNT, day(), "calendar").unwrap().calls,
        0
    );
}

/// R-9: no token appears in the client's debug output or in its errors.
#[tokio::test]
async fn tokens_stay_out_of_debug_and_errors() {
    let world = World::new(3600).await;
    world.refuse_current_tokens();
    world.oauth.revoke_all_refresh_tokens();
    let error = world
        .server
        .call_tool(READ_TOOL, None, &allow(&[READ_TOOL]))
        .await
        .unwrap_err();
    let shown = format!("{:?} {:?} {error:?} {error}", world.server, world.auth);
    for token in world
        .oauth
        .issued_access_tokens()
        .iter()
        .chain(&world.oauth.issued_refresh_tokens())
    {
        assert!(!shown.contains(token.as_str()), "{token} leaked: {shown}");
    }
}

/// R-20: a tool's call is counted at its cost in units from the preset; others cost 1.
#[tokio::test]
async fn calls_are_counted_at_their_cost_in_units() {
    let world = World::new(3600).await;
    let costly = RemoteServer::new(
        "costly",
        world.mcp.url(),
        world.auth.clone(),
        world.log.clone(),
        Arc::new(day),
    )
    .with_units([(READ_TOOL.to_owned(), 10)].into());
    costly
        .call_tool(READ_TOOL, None, &allow(&[READ_TOOL]))
        .await
        .unwrap();
    costly.list_tools().await.unwrap();
    assert_eq!(
        world.log.usage(ACCOUNT, day(), "costly").unwrap(),
        hyperme_core::connectors::Usage {
            calls: 2,
            units: 11
        }
    );
}

/// AC-4 (alert part), R-10: a pull whose refresh token is refused reports "sign in again" to
/// the sync, which marks the account and raises exactly one alert; the next sync is not tried.
#[tokio::test]
async fn a_revoked_sign_in_reaches_the_sync_as_one_alert() {
    use hyperme_core::health::{self, MemoryAccountStatus, SyncOutcome};
    use hyperme_core::sync::{SyncError, ToolCaller};

    let world = World::new(3600).await;
    world.refuse_current_tokens();
    world.oauth.revoke_all_refresh_tokens();
    let server = RemoteServer::new(
        SERVER,
        world.mcp.url(),
        world.auth.clone(),
        world.log.clone(),
        Arc::new(day),
    );
    let caller = hyperme_providers::mcp::McpCaller::new(vec![server], allow(&[READ_TOOL]));
    let status = MemoryAccountStatus::default();
    let now = time::UtcDateTime::now();

    let first = health::guarded_sync(&status, ACCOUNT, "Google · personal", now, || async {
        caller.call(SERVER, READ_TOOL, serde_json::json!({})).await
    })
    .await;
    assert!(matches!(first, SyncOutcome::Alert(_)), "{first:?}");
    let refreshes = world.oauth.refresh_calls();
    let second = health::guarded_sync(&status, ACCOUNT, "Google · personal", now, || async {
        caller.call(SERVER, READ_TOOL, serde_json::json!({})).await
    })
    .await;
    assert!(matches!(second, SyncOutcome::SignInNeeded), "{second:?}");
    assert_eq!(world.oauth.refresh_calls(), refreshes, "no retry");
    assert!(matches!(
        caller.call(SERVER, READ_TOOL, serde_json::json!({})).await,
        Err(SyncError::SignInAgain)
    ));
}
