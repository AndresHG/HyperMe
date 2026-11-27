//! A fake remote MCP server on loopback for the connector tests (specs/97-google-connectors,
//! design.md, "Tests"): rmcp's own streamable HTTP server with two canned tools, behind a small
//! hyper server that answers 401 to any bearer token the test does not accept. No network, no
//! key, synthetic data only.

#![allow(dead_code)]

use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use rmcp::ErrorData;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, JsonObject,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
};
use rmcp::service::{MaybeSendFuture, RequestContext, RoleServer};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use tokio::net::TcpListener;

/// The tools the fake offers: one that reads, one that writes.
pub const READ_TOOL: &str = "list_events";
pub const WRITE_TOOL: &str = "create_draft";

/// Decides which bearer tokens the server accepts.
pub type Accept = Arc<dyn Fn(&str) -> bool + Send + Sync>;

#[derive(Default)]
struct Seen {
    /// Every `Authorization` header, in order.
    auth: Vec<String>,
    unauthorized: usize,
    calls: Vec<String>,
    /// What the read tool answers, as text; a fixed sentence when unset.
    answer: Option<String>,
    /// The arguments of each call, in order.
    arguments: Vec<serde_json::Value>,
}

#[derive(Clone)]
struct Tools {
    seen: Arc<Mutex<Seen>>,
}

fn tool(name: &'static str) -> Tool {
    let schema: JsonObject = serde_json::from_value(serde_json::json!({"type": "object"})).unwrap();
    Tool::new(name, "synthetic test tool", Arc::new(schema))
}

impl ServerHandler for Tools {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + MaybeSendFuture + '_ {
        std::future::ready(Ok(ListToolsResult::with_all_items(vec![
            tool(READ_TOOL),
            tool(WRITE_TOOL),
        ])))
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResponse, ErrorData>> + MaybeSendFuture + '_ {
        let name = request.name.to_string();
        let mut seen = self.seen.lock().unwrap();
        seen.calls.push(name.clone());
        seen.arguments.push(serde_json::Value::Object(
            request.arguments.clone().unwrap_or_default(),
        ));
        let text = match name.as_str() {
            READ_TOOL => seen
                .answer
                .clone()
                .unwrap_or_else(|| "3 synthetic events".to_owned()),
            WRITE_TOOL => "synthetic draft".to_owned(),
            _ => "unknown tool".to_owned(),
        };
        std::future::ready(Ok(CallToolResponse::Complete(CallToolResult::success(
            vec![ContentBlock::text(text)],
        ))))
    }
}

/// The running server.
#[derive(Clone)]
pub struct FakeMcp {
    url: String,
    seen: Arc<Mutex<Seen>>,
}

impl FakeMcp {
    /// Serves on a random loopback port, accepting the bearer tokens `accept` allows.
    pub async fn start(accept: Accept) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/mcp", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Seen::default()));
        let tools = Tools { seen: seen.clone() };
        let mcp: StreamableHttpService<Tools, LocalSessionManager> = StreamableHttpService::new(
            move || Ok(tools.clone()),
            Default::default(),
            StreamableHttpServerConfig::default().with_json_response(true),
        );
        let shared = seen.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let (mcp, seen, accept) = (mcp.clone(), shared.clone(), accept.clone());
                tokio::spawn(async move {
                    let service = service_fn(move |request: Request<Incoming>| {
                        let (mcp, seen, accept) = (mcp.clone(), seen.clone(), accept.clone());
                        async move { Ok::<_, Infallible>(answer(request, &mcp, &seen, &accept).await) }
                    });
                    let _ = http1::Builder::new()
                        .serve_connection(TokioIo::new(stream), service)
                        .await;
                });
            }
        });
        FakeMcp { url, seen }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn auth_headers(&self) -> Vec<String> {
        self.seen.lock().unwrap().auth.clone()
    }

    pub fn unauthorized(&self) -> usize {
        self.seen.lock().unwrap().unauthorized
    }

    /// Makes the read tool answer with this text, such as synthetic JSON.
    pub fn set_answer(&self, text: &str) {
        self.seen.lock().unwrap().answer = Some(text.to_owned());
    }

    /// The arguments of each tool call, in order.
    pub fn arguments(&self) -> Vec<serde_json::Value> {
        self.seen.lock().unwrap().arguments.clone()
    }

    /// The tools called, in order.
    pub fn calls(&self) -> Vec<String> {
        self.seen.lock().unwrap().calls.clone()
    }
}

async fn answer(
    request: Request<Incoming>,
    mcp: &StreamableHttpService<Tools, LocalSessionManager>,
    seen: &Mutex<Seen>,
    accept: &Accept,
) -> Response<http_body_util::combinators::BoxBody<Bytes, Infallible>> {
    let header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let allowed = header
        .strip_prefix("Bearer ")
        .is_some_and(|token| accept(token));
    {
        let mut seen = seen.lock().unwrap();
        seen.auth.push(header);
        if !allowed {
            seen.unauthorized += 1;
        }
    }
    if !allowed {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header("www-authenticate", "Bearer error=\"invalid_token\"")
            .body(Full::new(Bytes::new()).boxed())
            .unwrap();
    }
    mcp.handle(request).await
}
