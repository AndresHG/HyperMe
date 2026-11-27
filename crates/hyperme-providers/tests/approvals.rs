//! Slice 1 of specs/166-gmail-approvals: approvals asked through MCP form elicitation, against a
//! local fake MCP server and the fake OAuth server. Tests cite the acceptance criteria in
//! `specs/166-gmail-approvals/spec.md`. No test calls Google or needs a key; forms and messages
//! are synthetic, shaped like Google's guide.

mod fake_mcp;
mod fake_oauth;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use fake_mcp::{FakeMcp, READ_TOOL, WRITE_TOOL};
use fake_oauth::FakeOAuth;
use hyperme_core::approval::{Approval, ApprovalAsk, ApprovalFuture, Approver, Unattended};
use hyperme_core::connectors::MemoryRequestLog;
use hyperme_core::secrets::MemorySecretStore;
use hyperme_core::sync::{SyncError, ToolCaller};
use hyperme_providers::mcp::{AccountAuth, ConnectorError, McpCaller, RemoteServer};
use hyperme_providers::oauth::{Browser, DEFAULT_LISTEN_TIMEOUT, OAuthClient, OAuthConfig};
use serde_json::{Value, json};
use time::{Date, Month};

const ACCOUNT: &str = "google:personal";
const SERVER: &str = "gmail";
const STATE: &str = r#"{"elicitationId":"e-1","requestBinding":"b-1"}"#;

fn day() -> Date {
    Date::from_calendar_date(2026, Month::November, 27).unwrap()
}

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

enum Kind {
    Stateless,
    Legacy,
}

async fn world(kind: Kind) -> (FakeMcp, RemoteServer) {
    let oauth = FakeOAuth::start().await;
    let accept = {
        let oauth = oauth.clone();
        Arc::new(move |token: &str| oauth.issued_access_tokens().iter().any(|t| t == token))
    };
    let mcp = match kind {
        Kind::Stateless => FakeMcp::start_stateless(accept).await,
        Kind::Legacy => FakeMcp::start_legacy(accept).await,
    };
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
            ACCOUNT,
            &["mail.read"],
            DEFAULT_LISTEN_TIMEOUT,
        )
        .await
        .unwrap();
    let auth = Arc::new(AccountAuth::new(client, store, ACCOUNT).with_token(first));
    let server = RemoteServer::new(
        SERVER,
        mcp.url(),
        auth,
        Arc::new(MemoryRequestLog::default()),
        Arc::new(day),
    );
    (mcp, server)
}

fn allow(tools: &[&str]) -> Vec<String> {
    tools.iter().map(|t| (*t).to_owned()).collect()
}

/// Google's approval form, as its guide shows it, with `schema` as the requested schema.
fn asking_with(schema: Value) -> Value {
    json!({
        "resultType": "input_required",
        "inputRequests": {"approval_choice": {
            "method": "elicitation/create",
            "params": {
                "mode": "form",
                "message": "HyperMe wants to read **3 threads** from synthetic senders.",
                "requestedSchema": schema,
            }
        }},
        "requestState": STATE,
    })
}

fn google_schema() -> Value {
    json!({
        "type": "object",
        "required": ["approval_choice"],
        "properties": {"approval_choice": {
            "type": "string",
            "title": "Allow HyperMe to use list_events?",
            "enum": ["ALLOW_ONCE", "ALLOW_OWNER_ALWAYS", "ALLOW_TOOL_ALWAYS", "DENY_ONCE"],
            "enumNames": ["Allow once", "Always allow for these owners", "Always allow this tool", "Don't allow"],
        }},
    })
}

fn asking() -> Value {
    asking_with(google_schema())
}

fn done() -> Value {
    json!({"resultType": "complete", "content": [{"type": "text", "text": "3 synthetic threads"}]})
}

fn refused(reason: Option<&str>) -> Value {
    let mut result = json!({
        "resultType": "complete",
        "isError": true,
        "content": [{"type": "text", "text": "Action canceled."}],
    });
    if let Some(reason) = reason {
        result["_meta"] = json!({"google.com/error": {"details": [{"reason": reason}]}});
    }
    result
}

/// An approver that answers from a script and remembers what it was asked.
#[derive(Default)]
struct Scripted {
    answers: Mutex<VecDeque<Approval>>,
    asked: Mutex<Vec<ApprovalAsk>>,
}

impl Scripted {
    fn new(answers: &[Approval]) -> Self {
        Scripted {
            answers: Mutex::new(answers.iter().cloned().collect()),
            asked: Mutex::default(),
        }
    }

    fn asked(&self) -> Vec<ApprovalAsk> {
        self.asked.lock().unwrap().clone()
    }
}

impl Approver for Scripted {
    fn answer(&self, ask: ApprovalAsk) -> ApprovalFuture<'_> {
        self.asked.lock().unwrap().push(ask);
        let answer = self
            .answers
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Approval::NoAnswer);
        Box::pin(std::future::ready(answer))
    }
}

fn choice(value: &str) -> Approval {
    Approval::Choice(value.to_owned())
}

/// AC-1: against a stateless server, every request declares form elicitation in its `_meta`
/// client capabilities, and nothing else that would let the server ask (R-1).
#[tokio::test]
async fn stateless_requests_declare_form_elicitation() {
    let (mcp, server) = world(Kind::Stateless).await;
    server.list_tools().await.unwrap();
    server
        .call_tool(READ_TOOL, None, &allow(&[READ_TOOL]))
        .await
        .unwrap();
    let requests: Vec<Value> = mcp
        .messages()
        .into_iter()
        .filter(|m| m["method"] == "tools/list" || m["method"] == "tools/call")
        .collect();
    assert_eq!(requests.len(), 2, "{requests:?}");
    for request in requests {
        let capabilities =
            &request["params"]["_meta"]["io.modelcontextprotocol/clientCapabilities"];
        assert!(capabilities["elicitation"]["form"].is_object(), "{request}");
        assert!(
            capabilities["elicitation"].get("url").is_none(),
            "{request}"
        );
        assert!(capabilities.get("sampling").is_none(), "{request}");
    }
}

/// AC-1: an older server that speaks only `initialize` is told nothing about elicitation, so
/// it never asks and HyperMe never declines on its own (R-1a). That is found once: the next
/// call does not try `server/discover` again. (The other half of R-1a, answering a
/// server-to-client `elicitation/create` with an error, is `HyperClient::create_elicitation`;
/// the fake cannot send server-to-client requests, so it has no test here.)
#[tokio::test]
async fn legacy_servers_are_not_told_about_elicitation() {
    let (mcp, server) = world(Kind::Legacy).await;
    server
        .call_tool(READ_TOOL, None, &allow(&[READ_TOOL]))
        .await
        .unwrap();
    let messages = mcp.messages();
    let initialize = messages
        .iter()
        .find(|m| m["method"] == "initialize")
        .expect("falls back to initialize");
    assert!(
        initialize["params"]["capabilities"]
            .get("elicitation")
            .is_none(),
        "{initialize}"
    );
    for call in mcp.tool_calls() {
        let capabilities = &call["_meta"]["io.modelcontextprotocol/clientCapabilities"];
        assert!(capabilities.get("elicitation").is_none(), "{call}");
    }
    let discovers = |mcp: &FakeMcp| {
        mcp.messages()
            .iter()
            .filter(|m| m["method"] == "server/discover")
            .count()
    };
    server.list_tools().await.unwrap();
    assert_eq!(discovers(&mcp), 1);
}

/// R-1a: a server that fails once (HTTP 503) is not taken for an older one: the next call
/// still uses the stateless lifecycle and declares form elicitation.
#[tokio::test]
async fn a_failed_discover_does_not_make_a_server_legacy() {
    let (mcp, server) = world(Kind::Stateless).await;
    mcp.fail_next(1);
    assert!(server.list_tools().await.is_err());
    server.list_tools().await.unwrap();
    let messages = mcp.messages();
    assert!(
        !messages.iter().any(|m| m["method"] == "initialize"),
        "{messages:?}"
    );
    let list = messages
        .iter()
        .find(|m| m["method"] == "tools/list")
        .unwrap();
    assert!(
        list["params"]["_meta"]["io.modelcontextprotocol/clientCapabilities"]["elicitation"]["form"]
            .is_object()
    );
}

/// AC-2: the server asks; the approver sees Google's question and choices; the retry carries
/// the same name and arguments, the state unchanged and the chosen value; the result arrives.
/// A later identical call is asked again: HyperMe keeps no decision (R-2, R-5).
#[tokio::test]
async fn an_approval_is_sent_back_with_the_state_unchanged() {
    let (mcp, server) = world(Kind::Stateless).await;
    mcp.script(vec![asking(), done(), asking(), done()]);
    let approver = Scripted::new(&[choice("ALLOW_TOOL_ALWAYS"), choice("ALLOW_ONCE")]);
    let arguments = json!({"query": "in:inbox"}).as_object().cloned();

    let result = server
        .call_tool_with(
            READ_TOOL,
            arguments.clone(),
            &allow(&[READ_TOOL]),
            &approver,
        )
        .await
        .unwrap();
    assert!(
        serde_json::to_string(&result.content)
            .unwrap()
            .contains("3 synthetic threads")
    );

    let asked = approver.asked();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].account, ACCOUNT);
    assert_eq!(asked[0].server, SERVER);
    assert_eq!(asked[0].tool, READ_TOOL);
    assert_eq!(asked[0].title, "Allow HyperMe to use list_events?");
    assert!(asked[0].message.contains("**3 threads**"));
    let labels: Vec<&str> = asked[0].choices.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Allow once",
            "Always allow for these owners",
            "Always allow this tool",
            "Don't allow"
        ]
    );

    let calls = mcp.tool_calls();
    assert_eq!(calls.len(), 2);
    assert!(calls[0].get("requestState").is_none());
    assert!(calls[0].get("inputResponses").is_none());
    assert_eq!(calls[1]["name"], READ_TOOL);
    assert_eq!(calls[1]["arguments"], calls[0]["arguments"]);
    assert_eq!(calls[1]["requestState"], STATE);
    assert_eq!(
        calls[1]["inputResponses"],
        json!({"approval_choice": {"action": "accept", "content": {"approval_choice": "ALLOW_TOOL_ALWAYS"}}})
    );

    server
        .call_tool_with(READ_TOOL, arguments, &allow(&[READ_TOOL]), &approver)
        .await
        .unwrap();
    let calls = mcp.tool_calls();
    assert_eq!(calls.len(), 4);
    assert!(calls[2].get("inputResponses").is_none());
    assert_eq!(approver.asked().len(), 2);
}

/// AC-3 (client part): the unattended approver sends no answer and no retry; the call ends as
/// "approval needed" naming only the tool (R-7).
#[tokio::test]
async fn unattended_calls_never_answer() {
    let (mcp, server) = world(Kind::Stateless).await;
    mcp.script(vec![asking()]);
    let error = server
        .call_tool_with(READ_TOOL, None, &allow(&[READ_TOOL]), &Unattended)
        .await
        .unwrap_err();
    assert!(
        matches!(&error, ConnectorError::ApprovalNeeded(tool) if tool == READ_TOOL),
        "{error:?}"
    );
    assert_eq!(mcp.tool_calls().len(), 1);
    let shown = format!("{error} {error:?}");
    for secret in ["synthetic senders", "list_events?", "b-1", "e-1"] {
        assert!(!shown.contains(secret), "{secret} in {shown}");
    }
}

/// AC-5 (client part): "Don't allow" is sent as the form's own value, and the error that comes
/// back is a deny whether or not Google names the reason (R-15).
#[tokio::test]
async fn a_deny_is_recognised_with_or_without_the_reason() {
    for reason in [Some("UserDeniedAction"), None] {
        let (mcp, server) = world(Kind::Stateless).await;
        mcp.script(vec![asking(), refused(reason)]);
        let approver = Scripted::new(&[choice("DENY_ONCE")]);
        let error = server
            .call_tool_with(READ_TOOL, None, &allow(&[READ_TOOL]), &approver)
            .await
            .unwrap_err();
        assert!(
            matches!(&error, ConnectorError::Denied(tool) if tool == READ_TOOL),
            "{error:?}"
        );
        assert_eq!(
            mcp.tool_calls()[1]["inputResponses"]["approval_choice"]["content"]["approval_choice"],
            "DENY_ONCE"
        );
    }
}

/// AC-9 (client part): an admin block named in Google's error `_meta` is told apart; the same
/// error without the reason stays an ordinary error result (R-16).
#[tokio::test]
async fn an_admin_block_is_recognised_only_by_its_reason() {
    let (mcp, server) = world(Kind::Stateless).await;
    mcp.script(vec![refused(Some("AdminPolicyBlock")), refused(None)]);
    let error = server
        .call_tool_with(READ_TOOL, None, &allow(&[READ_TOOL]), &Unattended)
        .await
        .unwrap_err();
    assert!(
        matches!(&error, ConnectorError::AdminBlocked(tool) if tool == READ_TOOL),
        "{error:?}"
    );
    let plain = server
        .call_tool_with(READ_TOOL, None, &allow(&[READ_TOOL]), &Unattended)
        .await
        .unwrap();
    assert_eq!(plain.is_error, Some(true));
}

/// AC-7: a server that keeps asking gets 3 retries (4 calls), then the call fails (R-3).
#[tokio::test]
async fn a_server_that_keeps_asking_gets_three_retries() {
    let (mcp, server) = world(Kind::Stateless).await;
    mcp.script(vec![asking(), asking(), asking(), asking(), asking()]);
    let approver = Scripted::new(&vec![choice("ALLOW_ONCE"); 5]);
    let error = server
        .call_tool_with(READ_TOOL, None, &allow(&[READ_TOOL]), &approver)
        .await
        .unwrap_err();
    assert!(
        matches!(&error, ConnectorError::TooManyRounds(tool) if tool == READ_TOOL),
        "{error:?}"
    );
    assert_eq!(mcp.tool_calls().len(), 4);
}

/// AC-7: an answer with only a state (the server shedding load) is sent again with the state
/// and no answers, without asking anyone (R-2a).
#[tokio::test]
async fn a_state_only_answer_is_retried_without_asking() {
    let (mcp, server) = world(Kind::Stateless).await;
    mcp.script(vec![
        json!({"resultType": "input_required", "requestState": STATE}),
        done(),
    ]);
    server
        .call_tool_with(READ_TOOL, None, &allow(&[READ_TOOL]), &Unattended)
        .await
        .unwrap();
    let calls = mcp.tool_calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[1]["requestState"], STATE);
    assert!(calls[1].get("inputResponses").is_none());
}

/// AC-8: forms HyperMe can't show as a card (free text, two fields, a multi-select, no
/// choices) get no answer and no retry, and end naming the tool (R-12).
#[tokio::test]
async fn forms_that_are_not_one_choice_get_no_answer() {
    let free_text = json!({"type": "object", "required": ["note"],
        "properties": {"note": {"type": "string"}}});
    let two_fields = json!({"type": "object", "required": ["approval_choice", "note"],
        "properties": {
            "approval_choice": {"type": "string", "enum": ["ALLOW_ONCE", "DENY_ONCE"]},
            "note": {"type": "string"}}});
    let multi = json!({"type": "object", "required": ["approval_choice"],
        "properties": {"approval_choice": {"type": "array",
            "items": {"type": "string", "enum": ["ALLOW_ONCE", "DENY_ONCE"]}}}});
    let optional = json!({"type": "object",
        "properties": {"approval_choice": {"type": "string", "enum": ["ALLOW_ONCE"]}}});
    for schema in [free_text, two_fields, multi, optional] {
        let (mcp, server) = world(Kind::Stateless).await;
        mcp.script(vec![asking_with(schema.clone())]);
        let approver = Scripted::new(&[choice("ALLOW_ONCE")]);
        let error = server
            .call_tool_with(READ_TOOL, None, &allow(&[READ_TOOL]), &approver)
            .await
            .unwrap_err();
        assert!(
            matches!(&error, ConnectorError::UnsupportedInput(tool) if tool == READ_TOOL),
            "{schema}: {error:?}"
        );
        assert!(approver.asked().is_empty(), "{schema}");
        assert_eq!(mcp.tool_calls().len(), 1, "{schema}");
    }
}

/// R-10, R-12: `oneOf` choices are read like `enum` ones, and a missing label falls back to the
/// value; the title falls back to the schema's, then to the tool's name.
#[tokio::test]
async fn choice_labels_and_titles_have_fallbacks() {
    let one_of = json!({"type": "object", "title": "Schema title", "required": ["pick"],
        "properties": {"pick": {"type": "string", "oneOf": [
            {"const": "ALLOW_ONCE", "title": "Allow once"},
            {"const": "DENY_ONCE", "title": "Don't allow"}]}}});
    let bare = json!({"type": "object", "required": ["pick"],
        "properties": {"pick": {"type": "string", "enum": ["ALLOW_ONCE", "DENY_ONCE"]}}});
    let cases = [
        (one_of, "Schema title", ["Allow once", "Don't allow"]),
        (
            bare,
            "Allow HyperMe to use list_events?",
            ["ALLOW_ONCE", "DENY_ONCE"],
        ),
    ];
    for (schema, title, labels) in cases {
        let (mcp, server) = world(Kind::Stateless).await;
        mcp.script(vec![asking_with(schema), done()]);
        let approver = Scripted::new(&[choice("ALLOW_ONCE")]);
        server
            .call_tool_with(READ_TOOL, None, &allow(&[READ_TOOL]), &approver)
            .await
            .unwrap();
        let ask = &approver.asked()[0];
        assert_eq!(ask.title, title);
        let got: Vec<&str> = ask.choices.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(got, labels);
        assert_eq!(
            mcp.tool_calls()[1]["inputResponses"],
            json!({"approval_choice": {"action": "accept", "content": {"pick": "ALLOW_ONCE"}}})
        );
    }
}

/// AC-11: a tool outside the caller's allowlist never reaches the server, whatever the
/// approver would answer (R-4).
#[tokio::test]
async fn approvals_never_widen_the_allowlist() {
    let (mcp, server) = world(Kind::Stateless).await;
    let approver = Scripted::new(&[choice("ALLOW_TOOL_ALWAYS")]);
    let error = server
        .call_tool_with(WRITE_TOOL, None, &allow(&[READ_TOOL]), &approver)
        .await
        .unwrap_err();
    assert!(matches!(error, ConnectorError::NotAllowed(_)), "{error:?}");
    assert!(mcp.tool_calls().is_empty());
    assert!(approver.asked().is_empty());
}

/// R-2: an approver that answers with a value the form does not offer sends nothing.
#[tokio::test]
async fn a_value_the_form_does_not_offer_is_not_sent() {
    let (mcp, server) = world(Kind::Stateless).await;
    mcp.script(vec![asking()]);
    let approver = Scripted::new(&[choice("ALLOW_EVERYTHING")]);
    let error = server
        .call_tool_with(READ_TOOL, None, &allow(&[READ_TOOL]), &approver)
        .await
        .unwrap_err();
    assert!(
        matches!(error, ConnectorError::UnsupportedInput(_)),
        "{error:?}"
    );
    assert_eq!(mcp.tool_calls().len(), 1);
}

/// R-16b: a sync job's caller reports a call waiting for the user as `NeedsUser`, not as a
/// failure, so it never counts toward the job-failed alert.
#[tokio::test]
async fn a_pull_waiting_for_the_user_is_not_a_failure() {
    let (mcp, server) = world(Kind::Stateless).await;
    mcp.script(vec![asking()]);
    let caller = McpCaller::new(vec![server], allow(&[READ_TOOL]));
    let error = caller.call(SERVER, READ_TOOL, json!({})).await.unwrap_err();
    assert!(matches!(error, SyncError::NeedsUser(_)), "{error:?}");
}

/// Slice 2 (live check finding): servers see the client as HyperMe, the name they show when
/// they ask ("Allow HyperMe to use ..."), not rmcp's default.
#[tokio::test]
async fn servers_see_the_client_as_hyperme() {
    let (mcp, server) = world(Kind::Stateless).await;
    server.list_tools().await.unwrap();
    let list = mcp
        .messages()
        .into_iter()
        .find(|m| m["method"] == "tools/list")
        .unwrap();
    assert_eq!(
        list["params"]["_meta"]["io.modelcontextprotocol/clientInfo"]["name"], "HyperMe",
        "{list}"
    );
}

/// AC-5, AC-9 (sync part): a refusal and an admin block reach the sync job as their own
/// states, so the app marks the server and raises no job-failed alert (R-15, R-16, R-16b).
#[tokio::test]
async fn refusals_and_blocks_reach_the_sync_as_states() {
    use hyperme_core::health::ServerState;
    let cases = [
        (
            vec![refused(Some("UserDeniedAction"))],
            ServerState::NotAllowed,
        ),
        (
            vec![refused(Some("AdminPolicyBlock"))],
            ServerState::Blocked,
        ),
        (vec![asking()], ServerState::NeedsOk),
    ];
    for (script, state) in cases {
        let (mcp, server) = world(Kind::Stateless).await;
        mcp.script(script);
        let caller = McpCaller::new(vec![server], allow(&[READ_TOOL]));
        let error = caller.call(SERVER, READ_TOOL, json!({})).await.unwrap_err();
        assert_eq!(error, SyncError::NeedsUser(state));
    }
}
