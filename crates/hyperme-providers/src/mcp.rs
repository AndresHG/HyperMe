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
//! - A server may answer a call with "input required": an approval the user gives through a
//!   form of choices (specs/166-gmail-approvals). Each call has an [`Approver`]; the client
//!   sends back only the user's choice, with the server's state unchanged, and never answers
//!   for the user (R-2, R-7). Form elicitation is declared only to stateless servers (R-1a).

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use hyperme_core::approval::{Approval, ApprovalAsk, Approver, Choice, Unattended};
use hyperme_core::connectors::RequestLog;
use hyperme_core::health::ServerState;
use hyperme_core::secrets::SecretStore;
use hyperme_core::sync::SyncError;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ClientConfig, ElicitRequestParams,
    ElicitResult, ElicitationCapability, EnumSchema, FormElicitationCapability, InputRequest,
    InputResponses, JsonObject, PrimitiveSchemaDefinition, ProtocolVersion, SingleSelectEnumSchema,
    Tool,
};
use rmcp::service::{
    ClientInitializeError, ClientLifecycleMode, ClientServiceExt, RequestContext, RunningService,
};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::{ClientHandler, ErrorData, Peer, RoleClient, ServiceError};
use time::Date;
use tokio::sync::Mutex;

use crate::oauth::{AccessToken, OAuthClient, OAuthError};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// At most this many retries of one call after "input required" (R-3).
const MAX_RETRIES: usize = 3;

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
    /// The server asked for an approval and nobody answered: unattended, Esc or a timeout
    /// (specs/166, R-7, R-9). Carries the tool's name only.
    ApprovalNeeded(String),
    /// The user refused the call (specs/166, R-15).
    Denied(String),
    /// The server's admin policy blocks the tool (specs/166, R-16).
    AdminBlocked(String),
    /// The server asked for input HyperMe can't show as a card (specs/166, R-12).
    UnsupportedInput(String),
    /// The server kept asking past the retry limit (specs/166, R-3).
    TooManyRounds(String),
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
            ConnectorError::ApprovalNeeded(tool) => write!(f, "{tool} needs your OK"),
            ConnectorError::Denied(tool) => write!(f, "{tool} was not allowed"),
            ConnectorError::AdminBlocked(tool) => write!(f, "{tool} is blocked by your admin"),
            ConnectorError::UnsupportedInput(tool) => {
                write!(f, "{tool} asked for input HyperMe can't show yet")
            }
            ConnectorError::TooManyRounds(tool) => {
                write!(f, "{tool} kept asking for input")
            }
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
    service: RunningService<RoleClient, HyperClient>,
    token: AccessToken,
}

/// A request the client sends.
enum Request<'a> {
    ListTools,
    CallTool(&'a CallToolRequestParams),
}

enum Reply {
    Tools(Vec<Tool>),
    Called(CallToolResponse),
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
    /// Set once a server has refused `server/discover`: it then gets `initialize` with no
    /// elicitation declared (R-1a).
    legacy: std::sync::atomic::AtomicBool,
    /// What each tool costs in quota units; others cost 1 (R-20).
    units: std::collections::BTreeMap<String, u32>,
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
            legacy: std::sync::atomic::AtomicBool::new(false),
            units: std::collections::BTreeMap::new(),
        }
    }

    /// Counts each tool's calls at its cost in units, from the preset (R-20).
    pub fn with_units(self, units: std::collections::BTreeMap<String, u32>) -> Self {
        RemoteServer { units, ..self }
    }

    /// Every tool the server offers.
    pub async fn list_tools(&self) -> Result<Vec<Tool>, ConnectorError> {
        match self.send(Request::ListTools).await? {
            Reply::Tools(tools) => Ok(tools),
            Reply::Called(_) => Err(ConnectorError::Transport("unexpected reply".to_owned())),
        }
    }

    /// Calls `name` if it is in `allowlist`, the tools this caller may use. Nobody is there to
    /// answer an approval, so a call that needs one ends in
    /// [`ConnectorError::ApprovalNeeded`].
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Option<JsonObject>,
        allowlist: &[String],
    ) -> Result<CallToolResult, ConnectorError> {
        self.call_tool_with(name, arguments, allowlist, &Unattended)
            .await
    }

    /// Calls `name` if it is in `allowlist`; when the server asks for an approval, `approver`
    /// answers it (specs/166, R-2 to R-5a).
    pub async fn call_tool_with(
        &self,
        name: &str,
        arguments: Option<JsonObject>,
        allowlist: &[String],
        approver: &dyn Approver,
    ) -> Result<CallToolResult, ConnectorError> {
        if !allowlist.iter().any(|allowed| allowed == name) {
            return Err(ConnectorError::NotAllowed(name.to_owned()));
        }
        let mut params = CallToolRequestParams::new(name.to_owned());
        if let Some(arguments) = arguments {
            params = params.with_arguments(arguments);
        }
        let mut deny_sent = false;
        let mut state_only = 0u32;
        for round in 0..=MAX_RETRIES {
            let response = match self.send(Request::CallTool(&params)).await? {
                Reply::Called(response) => response,
                Reply::Tools(_) => {
                    return Err(ConnectorError::Transport("unexpected reply".to_owned()));
                }
            };
            let asked = match response {
                CallToolResponse::Complete(result) => return finish(name, result, deny_sent),
                CallToolResponse::InputRequired(asked) => asked,
                _ => return Err(ConnectorError::Server("unexpected answer".to_owned())),
            };
            if round == MAX_RETRIES {
                return Err(ConnectorError::TooManyRounds(name.to_owned()));
            }
            let requests = asked.input_requests.unwrap_or_default();
            if requests.is_empty() {
                // The server sheds load: send the state back after a pause, asking nobody (R-2a).
                let Some(state) = asked.request_state else {
                    return Err(ConnectorError::Server(
                        "input required, nothing asked".to_owned(),
                    ));
                };
                tokio::time::sleep(Duration::from_secs(1 << state_only.min(1))).await;
                state_only += 1;
                params.input_responses = None;
                params.request_state = Some(state);
                continue;
            }
            // Every form is read before anyone is asked, so no choice is thrown away (R-12).
            let forms = requests
                .iter()
                .map(|(key, request)| Ok((key.clone(), Form::read(request, name)?)))
                .collect::<Result<Vec<_>, ConnectorError>>()?;
            let mut responses = InputResponses::new();
            for (key, form) in forms {
                let ask = ApprovalAsk {
                    account: self.auth.account.clone(),
                    server: self.id.clone(),
                    tool: name.to_owned(),
                    title: form.title,
                    message: form.message,
                    choices: form.choices.clone(),
                };
                let value = match approver.answer(ask).await {
                    Approval::NoAnswer => {
                        return Err(ConnectorError::ApprovalNeeded(name.to_owned()));
                    }
                    Approval::Choice(value) => value,
                };
                let Some(choice) = form.choices.iter().find(|c| c.value == value) else {
                    return Err(ConnectorError::UnsupportedInput(name.to_owned()));
                };
                deny_sent |= choice.is_deny();
                responses.insert(
                    key,
                    serde_json::json!({
                        "action": "accept",
                        "content": { form.property: value },
                    }),
                );
            }
            params.input_responses = Some(responses);
            params.request_state = asked.request_state;
        }
        Err(ConnectorError::TooManyRounds(name.to_owned()))
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
        let units = match request {
            Request::CallTool(params) => self.units.get(params.name.as_ref()).copied().unwrap_or(1),
            Request::ListTools => 1,
        };
        self.log
            .record(&self.auth.account, (self.today)(), &self.id, units)
            .map_err(ConnectorError::Count)?;
        let result = match request {
            Request::ListTools => peer.list_all_tools().await.map(Reply::Tools),
            Request::CallTool(params) => peer
                .call_tool_once((*params).clone())
                .await
                .map(Reply::Called),
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
        let service = self.open(&token).await?;
        let peer = service.peer().clone();
        *session = Some(Session {
            service,
            token: token.clone(),
        });
        Ok((peer, token))
    }
}

impl RemoteServer {
    /// Opens a session. Google's servers answer `initialize` with "Method not supported":
    /// they speak the stateless lifecycle (`server/discover`), which carries the form
    /// elicitation declaration on every request (R-1). A server that refuses
    /// `server/discover` gets `initialize` with nothing declared, so it never asks (R-1a);
    /// that is remembered, so it costs one extra request once.
    async fn open(
        &self,
        token: &AccessToken,
    ) -> Result<RunningService<RoleClient, HyperClient>, Failure> {
        use std::sync::atomic::Ordering;
        let failed = |error: ClientInitializeError| {
            if error.is_authorization_required() {
                Failure::Unauthorized(token.clone())
            } else {
                Failure::Other(initialize_error(error))
            }
        };
        if !self.legacy.load(Ordering::Relaxed) {
            let discover = ClientLifecycleMode::Discover {
                preferred_versions: vec![ProtocolVersion::LATEST],
            };
            let opened = tokio::time::timeout(
                CONNECT_TIMEOUT,
                (HyperClient { declare: true })
                    .serve_with_lifecycle(self.transport(token), discover),
            )
            .await
            .map_err(|_| {
                Failure::Other(ConnectorError::Transport(
                    "server/discover timed out".to_owned(),
                ))
            })?;
            match opened {
                Ok(service) => return Ok(service),
                // Only a server that does not know `server/discover` is older; a network
                // error, a 5xx or any other refusal is returned as it is (R-1a).
                Err(ClientInitializeError::JsonRpcError(data))
                    if data.code == rmcp::model::ErrorCode::METHOD_NOT_FOUND =>
                {
                    self.legacy.store(true, Ordering::Relaxed);
                }
                Err(error) => return Err(failed(error)),
            }
        }
        (HyperClient { declare: false })
            .serve_with_lifecycle(self.transport(token), ClientLifecycleMode::Initialize)
            .await
            .map_err(failed)
    }

    fn transport(&self, token: &AccessToken) -> StreamableHttpClientTransport<reqwest::Client> {
        let config = StreamableHttpClientTransportConfig::with_uri(self.url.as_str())
            .auth_header(token.secret());
        StreamableHttpClientTransport::with_client(self.http.clone(), config)
    }
}

/// HyperMe's side of a session. It declares form elicitation only when told to (stateless
/// servers, R-1, R-1a), and answers a server-to-client `elicitation/create` with an error,
/// never a decline: approvals come only through "input required" answers, where the call's
/// approver decides (R-7).
#[derive(Debug, Clone, Copy)]
struct HyperClient {
    declare: bool,
}

impl ClientHandler for HyperClient {
    fn get_info(&self) -> ClientConfig {
        let mut info = ClientConfig::default();
        // Servers show this name when they ask the user ("Allow HyperMe to use ..."); rmcp's
        // default is its own name (live check, specs/166).
        info.client_info = rmcp::model::Implementation::new("HyperMe", env!("CARGO_PKG_VERSION"));
        if self.declare {
            info.capabilities.elicitation =
                Some(ElicitationCapability::new().with_form(FormElicitationCapability::new()));
        }
        info
    }

    fn create_elicitation(
        &self,
        _request: ElicitRequestParams,
        _context: RequestContext<RoleClient>,
    ) -> impl Future<Output = Result<ElicitResult, ErrorData>> + Send + '_ {
        std::future::ready(Err(ErrorData::method_not_found::<
            rmcp::model::ElicitationCreateRequestMethod,
        >()))
    }
}

/// One approval form, read from an input request (R-10, R-12).
struct Form {
    /// The schema's one property, which the answer's `content` is keyed by.
    property: String,
    title: String,
    message: String,
    choices: Vec<Choice>,
}

impl Form {
    /// Accepts a form whose schema has exactly one property, required, a string with `enum`
    /// (labels from `enumNames` when there are any) or with `oneOf` of `const` and `title`.
    /// Anything else is [`ConnectorError::UnsupportedInput`].
    fn read(request: &InputRequest, tool: &str) -> Result<Form, ConnectorError> {
        let unsupported = || ConnectorError::UnsupportedInput(tool.to_owned());
        let InputRequest::Elicitation(request) = request else {
            return Err(unsupported());
        };
        let ElicitRequestParams::FormElicitationParams {
            message,
            requested_schema,
            ..
        } = &request.params
        else {
            return Err(unsupported());
        };
        let mut properties = requested_schema.properties.iter();
        let (Some((property, definition)), None) = (properties.next(), properties.next()) else {
            return Err(unsupported());
        };
        let required = requested_schema
            .required
            .as_ref()
            .is_some_and(|required| required.iter().any(|r| r == property));
        if !required {
            return Err(unsupported());
        }
        let PrimitiveSchemaDefinition::Enum(schema) = definition else {
            return Err(unsupported());
        };
        let (title, choices): (Option<&str>, Vec<Choice>) = match schema {
            EnumSchema::Legacy(legacy) => {
                let names = legacy.enum_names.as_deref().unwrap_or_default();
                let choices = legacy
                    .enum_
                    .iter()
                    .enumerate()
                    .map(|(i, value)| Choice {
                        value: value.clone(),
                        label: names.get(i).unwrap_or(value).clone(),
                    })
                    .collect();
                (legacy.title.as_deref(), choices)
            }
            EnumSchema::Single(SingleSelectEnumSchema::Untitled(single)) => (
                single.title.as_deref(),
                single
                    .enum_
                    .iter()
                    .map(|value| Choice {
                        value: value.clone(),
                        label: value.clone(),
                    })
                    .collect(),
            ),
            EnumSchema::Single(SingleSelectEnumSchema::Titled(single)) => (
                single.title.as_deref(),
                single
                    .one_of
                    .iter()
                    .map(|option| Choice {
                        value: option.const_.clone(),
                        label: option.title.clone(),
                    })
                    .collect(),
            ),
            _ => return Err(unsupported()),
        };
        if choices.is_empty() {
            return Err(unsupported());
        }
        let title = title
            .or(requested_schema.title.as_deref())
            .map(str::to_owned)
            .unwrap_or_else(|| format!("Allow HyperMe to use {tool}?"));
        Ok(Form {
            property: property.clone(),
            title,
            message: message.clone(),
            choices,
        })
    }
}

/// How a call that reached a final result ends: a refusal the user made, or one the server's
/// error `_meta` names, becomes its own error; any other result goes back to the caller
/// (R-15, R-16).
fn finish(
    tool: &str,
    result: CallToolResult,
    deny_sent: bool,
) -> Result<CallToolResult, ConnectorError> {
    if result.is_error != Some(true) {
        return Ok(result);
    }
    if deny_sent {
        return Err(ConnectorError::Denied(tool.to_owned()));
    }
    let reasons = result
        .meta
        .as_ref()
        .and_then(|meta| meta.0.get("google.com/error"))
        .and_then(|error| error.get("details"))
        .and_then(|details| details.as_array())
        .map(|details| {
            details
                .iter()
                .filter_map(|detail| detail.get("reason").and_then(|r| r.as_str()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if reasons.contains(&"AdminPolicyBlock") {
        return Err(ConnectorError::AdminBlocked(tool.to_owned()));
    }
    if reasons.contains(&"UserDeniedAction") {
        return Err(ConnectorError::Denied(tool.to_owned()));
    }
    Ok(result)
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

/// The pulls' tool caller (G-5): one account's servers by id, each call checked against the
/// sync job's allowlist (R-22, AC-10). The answer is the tool's structured content, or its
/// first text block read as JSON. Approvals go to its approver: [`Unattended`] unless told
/// otherwise, for scheduler jobs (specs/166, R-6).
pub struct McpCaller {
    servers: Vec<RemoteServer>,
    allowlist: Vec<String>,
    approver: Arc<dyn Approver>,
}

impl fmt::Debug for McpCaller {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpCaller")
            .field("servers", &self.servers)
            .field("allowlist", &self.allowlist)
            .finish_non_exhaustive()
    }
}

impl McpCaller {
    pub fn new(servers: Vec<RemoteServer>, allowlist: Vec<String>) -> Self {
        McpCaller {
            servers,
            allowlist,
            approver: Arc::new(Unattended),
        }
    }

    /// Lets `approver` answer approvals, such as the card when the user is watching.
    pub fn with_approver(self, approver: Arc<dyn Approver>) -> Self {
        McpCaller { approver, ..self }
    }
}

impl hyperme_core::sync::ToolCaller for McpCaller {
    async fn call(
        &self,
        server: &str,
        tool: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, hyperme_core::sync::SyncError> {
        let remote = self
            .servers
            .iter()
            .find(|s| s.id == server)
            .ok_or_else(|| format!("no server {server} for this account"))?;
        let arguments = match args {
            serde_json::Value::Object(map) => Some(map),
            serde_json::Value::Null => None,
            other => return Err(format!("{tool}: arguments must be an object, not {other}").into()),
        };
        let result = remote
            .call_tool_with(tool, arguments, &self.allowlist, &*self.approver)
            .await
            .map_err(|e| match e {
                ConnectorError::SignInAgain => SyncError::SignInAgain,
                ConnectorError::ApprovalNeeded(_) => SyncError::NeedsUser(ServerState::NeedsOk),
                ConnectorError::Denied(_) => SyncError::NeedsUser(ServerState::NotAllowed),
                ConnectorError::AdminBlocked(_) => SyncError::NeedsUser(ServerState::Blocked),
                other => other.to_string().into(),
            })?;
        if result.is_error == Some(true) {
            return Err(format!("{tool} answered with an error").into());
        }
        if let Some(structured) = result.structured_content {
            return Ok(structured);
        }
        let text = result
            .content
            .iter()
            .find_map(|block| block.as_text().map(|t| t.text.clone()))
            .ok_or_else(|| format!("{tool} answered with no text"))?;
        serde_json::from_str(&text)
            .map_err(|_| format!("{tool} answered text that is not JSON").into())
    }
}
