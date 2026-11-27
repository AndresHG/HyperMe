//! Slice 7 of specs/97-google-connectors, end to end: the calendar pull through the remote MCP
//! client against the fake MCP server, with synthetic events. Tests cite
//! `specs/97-google-connectors/spec.md`. No test calls Google or needs a key.

mod fake_mcp;
mod fake_oauth;

use std::sync::Arc;

use fake_mcp::{FakeMcp, READ_TOOL, WRITE_TOOL};
use fake_oauth::FakeOAuth;
use hyperme_core::connectors::MemoryRequestLog;
use hyperme_core::registry::Pull;
use hyperme_core::schedule::Zone;
use hyperme_core::secrets::MemorySecretStore;
use hyperme_core::sync::{self, EventStore, MemoryEvents, ToolCaller};
use hyperme_core::tasks::{Finish, NewTask, Task, TaskError, TaskId, TaskStore, TaskUpdate};
use hyperme_providers::mcp::{AccountAuth, McpCaller, RemoteServer};
use hyperme_providers::oauth::{Browser, DEFAULT_LISTEN_TIMEOUT, OAuthClient, OAuthConfig};
use serde_json::json;
use time::{Date, Month, Time, UtcDateTime, UtcOffset};

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

struct Plus1;

impl Zone for Plus1 {
    fn offset_at(&self, _at: UtcDateTime) -> UtcOffset {
        UtcOffset::from_hms(1, 0, 0).unwrap()
    }
}

/// A task store that must not be touched: this pull does not mirror.
struct NoTasks;

impl TaskStore for NoTasks {
    async fn add(&self, _: NewTask) -> Result<TaskId, TaskError> {
        unreachable!()
    }
    async fn update(&self, _: TaskId, _: TaskUpdate) -> Result<(), TaskError> {
        unreachable!()
    }
    async fn finish(&self, _: TaskId, _: Finish, _: i64) -> Result<(), TaskError> {
        unreachable!()
    }
    async fn remove(&self, _: TaskId) -> Result<(), TaskError> {
        unreachable!()
    }
    async fn upsert_mirror(&self, _: NewTask, _: Option<i64>) -> Result<TaskId, TaskError> {
        unreachable!()
    }
    async fn block(&self, _: TaskId, _: TaskId) -> Result<(), TaskError> {
        unreachable!()
    }
    async fn unblock(&self, _: TaskId, _: TaskId) -> Result<(), TaskError> {
        unreachable!()
    }
    async fn roll_over(&self, _: Date) -> Result<Vec<TaskId>, TaskError> {
        unreachable!()
    }
    async fn snapshot(&self, _: Date, _: i64) -> Result<Vec<Task>, TaskError> {
        unreachable!()
    }
}

fn now() -> UtcDateTime {
    UtcDateTime::new(
        Date::from_calendar_date(2026, Month::November, 27).unwrap(),
        Time::from_hms(8, 0, 0).unwrap(),
    )
}

/// AC-14 (pull part, against the fake), R-16, R-22: the sync job's caller sends the pull's
/// arguments to the account's server, reads the JSON answer into the cache, and refuses any
/// tool outside the job's allowlist.
#[tokio::test]
async fn the_calendar_pull_runs_through_the_mcp_client() {
    let oauth = FakeOAuth::start().await;
    let accept = {
        let oauth = oauth.clone();
        Arc::new(move |token: &str| oauth.issued_access_tokens().iter().any(|t| t == token))
    };
    let mcp = FakeMcp::start(accept).await;
    mcp.set_answer(
        &json!({ "items": [{
            "id": "e1", "summary": "Synthetic standup", "status": "confirmed",
            "start": { "dateTime": "2026-11-27T09:30:00+01:00" },
            "end": { "dateTime": "2026-11-27T09:45:00+01:00" },
            "htmlLink": "https://calendar.example/e1",
        }]})
        .to_string(),
    );
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
            &["cal"],
            DEFAULT_LISTEN_TIMEOUT,
        )
        .await
        .unwrap();
    let auth = Arc::new(AccountAuth::new(client, store, "google:work").with_token(first));
    let remote = RemoteServer::new(
        "calendar",
        mcp.url(),
        auth,
        Arc::new(MemoryRequestLog::default()),
        Arc::new(|| now().date()),
    );
    let caller = McpCaller::new(vec![remote], vec![READ_TOOL.to_owned()]);
    let pull = Pull {
        server: "calendar".to_owned(),
        tool: READ_TOOL.to_owned(),
        args: [("timeMin".to_owned(), "{today_start}".to_owned())].into(),
        keep: Vec::new(),
        list: "items".to_owned(),
        fields: [
            ("id", "id"),
            ("title", "summary"),
            ("start", "start.dateTime|start.date"),
            ("end", "end.dateTime|end.date"),
            ("url", "hangoutLink|htmlLink"),
            ("status", "status"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect(),
        mirror: false,
        exclude_labels: Vec::new(),
        done_label: None,
        link: None,
        body_tool: None,
        body_arg: None,
        body_path: None,
        into: "events".to_owned(),
    };
    let events = MemoryEvents::default();
    let report = sync::pull_events(
        &caller,
        &events,
        &NoTasks,
        "google:work",
        &pull,
        now(),
        &Plus1,
    )
    .await
    .unwrap();
    assert_eq!(report.events, 1);
    assert_eq!(mcp.arguments()[0]["timeMin"], "2026-11-27T00:00:00+01:00");
    let meetings = sync::meetings_on(&events, now().date(), &Plus1).unwrap();
    assert_eq!(meetings[0].title, "Synthetic standup");
    assert_eq!(meetings[0].account, "google:work");
    assert_eq!(
        meetings[0].url.as_deref(),
        Some("https://calendar.example/e1")
    );
    assert!(
        events
            .between(None, now(), now() + time::Duration::days(1))
            .unwrap()
            .len()
            == 1
    );

    let refused = caller
        .call("calendar", WRITE_TOOL, json!({}))
        .await
        .unwrap_err();
    assert!(refused.contains(WRITE_TOOL), "{refused}");
    assert_eq!(mcp.calls(), [READ_TOOL]);

    mcp.set_answer("not json");
    let error = caller
        .call("calendar", READ_TOOL, json!({}))
        .await
        .unwrap_err();
    assert!(error.contains("not JSON"), "{error}");
}
