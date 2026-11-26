//! The remote MCP client with accounts (specs/97-google-connectors, slice 3): one `rmcp`
//! streamable HTTP session per account and server, with the account's OAuth access token as
//! its bearer token. Nothing here knows a service: the server is a URL from a preset.
//!
//! - A token that has expired on our side is refreshed before the request; a 401 from the
//!   server refreshes it and retries the request once (R-10).
//! - Refreshes for one account run one at a time, so calls that find the token expired
//!   together share one refresh (#121).
//! - A refused refresh token ends in [`ConnectorError::SignInAgain`], with no retry loop.
//! - Every request is counted per account, day and server (R-20).
//! - A call checks the caller's tool allowlist before anything is sent, so a sync job limited
//!   to read tools cannot call a write tool (R-22, AC-10).

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use hyperme_core::connectors::RequestLog;
use hyperme_core::secrets::SecretStore;
use rmcp::model::{CallToolRequestParams, CallToolResult, JsonObject, Tool};
use rmcp::service::{ClientInitializeError, RunningService};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::{Peer, RoleClient, ServiceError, ServiceExt};
use time::Date;
use tokio::sync::Mutex;

use crate::oauth::{AccessToken, OAuthClient, OAuthError};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Why a connector request failed. Never carries a token.
#[derive(Debug)]
pub enum ConnectorError {
    /// The tool is not in the caller's allowlist; nothing was sent.
    NotAllowed(String),
    /// The account's refresh token was refused: mark it "Sign in again" (R-10).
    SignInAgain,
    /// The server refused a freshly refreshed token too, for example for a missing scope.
    Unauthorized,
    /// Signing in or refreshing failed for another reason.
    Auth(OAuthError),
    /// The server answered the request with an MCP error.
    Server(String),
    /// The server could not be reached, or the session failed.
    Transport(String),
    /// The request counter could not be written.
    Count(String),
}

impl fmt::Display for ConnectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectorError::NotAllowed(tool) => {
                write!(f, "the tool {tool} is not allowed here")
            }
            ConnectorError::SignInAgain => write!(f, "the account needs a new sign-in"),
            ConnectorError::Unauthorized => {
                write!(f, "the server refused the account's access")
            }
            ConnectorError::Auth(error) => write!(f, "{error}"),
            ConnectorError::Server(reason) => write!(f, "the server answered: {reason}"),
            ConnectorError::Transport(reason) => {
                write!(f, "could not reach the server: {reason}")
            }
            ConnectorError::Count(reason) => write!(f, "request count: {reason}"),
        }
    }
}

impl std::error::Error for ConnectorError {}

impl From<OAuthError> for ConnectorError {
    fn from(error: OAuthError) -> Self {
        match error {
            OAuthError::SignInAgain => ConnectorError::SignInAgain,
            other => ConnectorError::Auth(other),
        }
    }
}

/// One signed-in account: its refresh token in the keychain and its access token in memory.
/// Share one per account (an `Arc`) between that account's servers, so they share the token
/// and its refresh lock.
pub struct AccountAuth {
    oauth: Arc<OAuthClient>,
    store: Arc<dyn SecretStore + Send + Sync>,
    account: String,
    /// The current access token. The lock is held across a refresh, which makes it the
    /// account's refresh lock.
    token: Mutex<Option<AccessToken>>,
}

impl fmt::Debug for AccountAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccountAuth")
            .field("account", &self.account)
            .finish_non_exhaustive()
    }
}

impl AccountAuth {
    /// `account` is the keychain account of the refresh token, such as `google:personal`.
    pub fn new(
        oauth: Arc<OAuthClient>,
        store: Arc<dyn SecretStore + Send + Sync>,
        account: &str,
    ) -> Self {
        AccountAuth {
            oauth,
            store,
            account: account.to_owned(),
            token: Mutex::new(None),
        }
    }

    /// Starts with the access token a sign-in just returned, saving a refresh.
    pub fn with_token(self, token: AccessToken) -> Self {
        AccountAuth {
            token: Mutex::new(Some(token)),
            ..self
        }
    }

    pub fn account(&self) -> &str {
        &self.account
    }

    /// A token good for now, refreshed first if it has expired.
    pub async fn token(&self) -> Result<AccessToken, OAuthError> {
        let mut current = self.token.lock().await;
        let token = self
            .oauth
            .fresh_token(&*self.store, &self.account, current.as_ref())
            .await?;
        *current = Some(token.clone());
        Ok(token)
    }

    /// A new token after the server refused `rejected`. When another call has already
    /// replaced it while this one waited for the lock, that token is used instead.
    pub async fn renew(&self, rejected: &AccessToken) -> Result<AccessToken, OAuthError> {
        let mut current = self.token.lock().await;
        if let Some(token) = current.as_ref()
            && token.secret() != rejected.secret()
            && token.is_fresh()
        {
            return Ok(token.clone());
        }
        let token = self.oauth.refresh(&*self.store, &self.account).await?;
        *current = Some(token.clone());
        Ok(token)
    }
}

/// An open MCP session and the token it was opened with.
struct Session {
    service: RunningService<RoleClient, ()>,
    token: AccessToken,
}

/// A request the client sends.
enum Request<'a> {
    ListTools,
    CallTool(&'a CallToolRequestParams),
}

enum Reply {
    Tools(Vec<Tool>),
    Called(CallToolResult),
}

/// How one attempt failed: an authorization failure is retried once with a new token.
enum Failure {
    Unauthorized(AccessToken),
    Other(ConnectorError),
}

impl From<ConnectorError> for Failure {
    fn from(error: ConnectorError) -> Self {
        Failure::Other(error)
    }
}

/// The local day for the request counter; the app passes the local clock.
pub type Today = Arc<dyn Fn() -> Date + Send + Sync>;

/// One remote MCP server reached as one account.
pub struct RemoteServer {
    /// The server's id in the preset, such as `gmail`; the counter's `server`.
    id: String,
    url: String,
    auth: Arc<AccountAuth>,
    log: Arc<dyn RequestLog>,
    today: Today,
    http: reqwest::Client,
    session: Mutex<Option<Session>>,
}

impl fmt::Debug for RemoteServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RemoteServer")
            .field("id", &self.id)
            .field("url", &self.url)
            .field("account", &self.auth.account)
            .finish_non_exhaustive()
    }
}

impl RemoteServer {
    pub fn new(
        id: &str,
        url: &str,
        auth: Arc<AccountAuth>,
        log: Arc<dyn RequestLog>,
        today: Today,
    ) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .unwrap_or_default();
        RemoteServer {
            id: id.to_owned(),
            url: url.to_owned(),
            auth,
            log,
            today,
            http,
            session: Mutex::new(None),
        }
    }

    /// Every tool the server offers.
    pub async fn list_tools(&self) -> Result<Vec<Tool>, ConnectorError> {
        match self.send(Request::ListTools).await? {
            Reply::Tools(tools) => Ok(tools),
            Reply::Called(_) => Err(ConnectorError::Transport("unexpected reply".to_owned())),
        }
    }

    /// Calls `name` if it is in `allowlist`, the tools this caller may use.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Option<JsonObject>,
        allowlist: &[String],
    ) -> Result<CallToolResult, ConnectorError> {
        if !allowlist.iter().any(|allowed| allowed == name) {
            return Err(ConnectorError::NotAllowed(name.to_owned()));
        }
        let mut params = CallToolRequestParams::new(name.to_owned());
        if let Some(arguments) = arguments {
            params = params.with_arguments(arguments);
        }
        match self.send(Request::CallTool(&params)).await? {
            Reply::Called(result) => Ok(result),
            Reply::Tools(_) => Err(ConnectorError::Transport("unexpected reply".to_owned())),
        }
    }

    /// Sends `request`; on an authorization failure, renews the token and tries once more.
    async fn send(&self, request: Request<'_>) -> Result<Reply, ConnectorError> {
        match self.attempt(&request, None).await {
            Ok(reply) => Ok(reply),
            Err(Failure::Other(error)) => Err(error),
            Err(Failure::Unauthorized(rejected)) => {
                match self.attempt(&request, Some(&rejected)).await {
                    Ok(reply) => Ok(reply),
                    Err(Failure::Other(error)) => Err(error),
                    Err(Failure::Unauthorized(_)) => Err(ConnectorError::Unauthorized),
                }
            }
        }
    }

    async fn attempt(
        &self,
        request: &Request<'_>,
        rejected: Option<&AccessToken>,
    ) -> Result<Reply, Failure> {
        let (peer, token) = self.connect(rejected).await?;
        self.log
            .record(&self.auth.account, (self.today)(), &self.id, 1)
            .map_err(ConnectorError::Count)?;
        let result = match request {
            Request::ListTools => peer.list_all_tools().await.map(Reply::Tools),
            Request::CallTool(params) => peer.call_tool((*params).clone()).await.map(Reply::Called),
        };
        result.map_err(|error| {
            if error.is_authorization_required() {
                Failure::Unauthorized(token)
            } else {
                Failure::Other(service_error(error))
            }
        })
    }

    /// The open session, or a new one when there is none, it closed, or the account's token
    /// changed since it was opened.
    async fn connect(
        &self,
        rejected: Option<&AccessToken>,
    ) -> Result<(Peer<RoleClient>, AccessToken), Failure> {
        let token = match rejected {
            Some(rejected) => self.auth.renew(rejected).await,
            None => self.auth.token().await,
        }
        .map_err(ConnectorError::from)?;

        let mut session = self.session.lock().await;
        if let Some(open) = session.as_ref()
            && open.token.secret() == token.secret()
            && !open.service.is_closed()
        {
            return Ok((open.service.peer().clone(), token));
        }
        if let Some(old) = session.take() {
            // Ends the old session; its DELETE may fail with the old token, which is fine.
            let _ = old.service.cancel().await;
        }
        let config = StreamableHttpClientTransportConfig::with_uri(self.url.as_str())
            .auth_header(token.secret());
        let transport = StreamableHttpClientTransport::with_client(self.http.clone(), config);
        let service = ().serve(transport).await.map_err(|error| {
            if error.is_authorization_required() {
                Failure::Unauthorized(token.clone())
            } else {
                Failure::Other(initialize_error(error))
            }
        })?;
        let peer = service.peer().clone();
        *session = Some(Session {
            service,
            token: token.clone(),
        });
        Ok((peer, token))
    }
}

fn service_error(error: ServiceError) -> ConnectorError {
    match error {
        ServiceError::McpError(data) => ConnectorError::Server(data.message.to_string()),
        other => ConnectorError::Transport(other.to_string()),
    }
}

fn initialize_error(error: ClientInitializeError) -> ConnectorError {
    ConnectorError::Transport(error.to_string())
}
