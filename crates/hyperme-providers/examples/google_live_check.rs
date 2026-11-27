//! The live check of specs/97-google-connectors (slice 1 and AC-14), run by hand on the owner's
//! Mac with the owner's own Google account:
//!
//! ```text
//! cargo run -p hyperme-providers --example google_live_check
//! ```
//!
//! It reads the user's OAuth client from the Keychain (service `ai.hyperme.connector`, account
//! `google-client`), signs in through the system browser with the read scopes only, lists each
//! Google MCP server's tools on `/mcp/v1` and `/mcp/v1self`, runs the preset's two pulls, and
//! disconnects. It prints names, argument names, shapes (keys), counts, yes/no and timings only:
//! never a token, an event, a mail, an address or any other content.

use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

use hyperme_core::config::Connectors;
use hyperme_core::connector_settings as cs;
use hyperme_core::connectors::MemoryRequestLog;
use hyperme_core::schedule::Zone;
use hyperme_core::secrets::{SecretError, SecretStore};
use hyperme_core::sync::{self, MemoryEvents, MemoryMail, ToolCaller};
use hyperme_providers::mcp::{AccountAuth, McpCaller, RemoteServer};
use hyperme_providers::oauth::{DEFAULT_LISTEN_TIMEOUT, SystemBrowser};
use hyperme_providers::setup;
use serde_json::Value;
use time::{UtcDateTime, UtcOffset};

const SERVICE: &str = "ai.hyperme.connector";
const ACCOUNT: &str = "google:livecheck";

/// The macOS Keychain through the `security` tool; values never reach the output.
#[derive(Debug)]
struct Keychain;

impl SecretStore for Keychain {
    fn get(&self, account: &str) -> Result<Option<String>, SecretError> {
        let out = Command::new("security")
            .args(["find-generic-password", "-s", SERVICE, "-a", account, "-w"])
            .output()
            .map_err(|e| SecretError(e.to_string()))?;
        if !out.status.success() {
            return Ok(None);
        }
        Ok(Some(
            String::from_utf8_lossy(&out.stdout).trim_end().to_owned(),
        ))
    }

    fn set(&self, account: &str, value: &str) -> Result<(), SecretError> {
        let status = Command::new("security")
            .args([
                "add-generic-password",
                "-U",
                "-s",
                SERVICE,
                "-a",
                account,
                "-w",
                value,
            ])
            .output()
            .map_err(|e| SecretError(e.to_string()))?
            .status;
        status
            .success()
            .then_some(())
            .ok_or_else(|| SecretError("could not save to the keychain".to_owned()))
    }

    fn delete(&self, account: &str) -> Result<(), SecretError> {
        let _ = Command::new("security")
            .args(["delete-generic-password", "-s", SERVICE, "-a", account])
            .output();
        Ok(())
    }
}

/// The Mac's zone for the pull window; Madrid in winter.
struct Local;

impl Zone for Local {
    fn offset_at(&self, _at: UtcDateTime) -> UtcOffset {
        // Madrid in November; the check only needs the right day.
        UtcOffset::from_hms(1, 0, 0).unwrap_or(UtcOffset::UTC)
    }
}

/// A JSON schema's outline: property names, types and enums, nested; schemas hold no user data.
fn schema_outline(schema: &Value) -> String {
    let kind = schema.get("type").and_then(Value::as_str).unwrap_or("");
    let enums = schema
        .get("enum")
        .and_then(Value::as_array)
        .map(|e| {
            format!(
                " enum{:?}",
                e.iter().filter_map(Value::as_str).collect::<Vec<_>>()
            )
        })
        .unwrap_or_default();
    if let Some(props) = schema.get("properties").and_then(Value::as_object) {
        let inner: Vec<String> = props
            .iter()
            .map(|(k, v)| format!("{k}: {}", schema_outline(v)))
            .collect();
        return format!("{{{}}}", inner.join(", "));
    }
    if let Some(items) = schema.get("items") {
        return format!("[{}]", schema_outline(items));
    }
    if let Some(r) = schema.get("$ref").and_then(Value::as_str) {
        return format!("ref {r}");
    }
    format!("{kind}{enums}")
}

/// The keys of a JSON value's shape, never its values.
fn shape(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let keys: Vec<String> = map.keys().cloned().collect();
            format!("{{{}}}", keys.join(", "))
        }
        Value::Array(list) => format!(
            "[{} items; first: {}]",
            list.len(),
            list.first().map_or("-".to_owned(), shape)
        ),
        Value::String(_) => "string".to_owned(),
        Value::Number(_) => "number".to_owned(),
        Value::Bool(_) => "bool".to_owned(),
        Value::Null => "null".to_owned(),
    }
}

#[tokio::main]
async fn main() {
    let registry = cs::registry(None, &Connectors::default()).expect("built-in presets");
    let google = registry.get("google").expect("google preset").clone();
    let store = Arc::new(Keychain);
    let oauth = Arc::new(setup::oauth_client(&google, &*store).expect("client in the keychain"));
    println!("client in keychain: yes");

    let features = vec!["calendar".to_owned(), "gmail".to_owned()];
    // KEEP=1 keeps the sign-in for another run; a kept sign-in is reused instead of a new one.
    let keep = std::env::var("KEEP").is_ok_and(|v| v == "1");
    let kept = store.get(ACCOUNT).ok().flatten().is_some();
    let started = Instant::now();
    let signed = if kept {
        println!("reusing the kept sign-in");
        oauth.refresh(&*store, ACCOUNT).await.map_err(Into::into)
    } else {
        setup::sign_in_account(
            &oauth,
            &google,
            &SystemBrowser,
            &*store,
            "livecheck",
            &features,
            DEFAULT_LISTEN_TIMEOUT,
        )
        .await
    };
    let token = match signed {
        Ok(token) => token,
        Err(error) => {
            println!(
                "sign-in: failed after {} ms: {error}",
                started.elapsed().as_millis()
            );
            return;
        }
    };
    if kept {
        println!(
            "kept sign-in refreshed: yes, {} ms",
            started.elapsed().as_millis()
        );
    } else {
        println!(
            "sign-in over loopback: yes, {} ms (includes the user's clicks)",
            started.elapsed().as_millis()
        );
    }
    println!(
        "refresh token stored: {}",
        store.get(ACCOUNT).ok().flatten().is_some()
    );
    let t = Instant::now();
    match oauth.refresh(&*store, ACCOUNT).await {
        Ok(_) => println!("refresh works: yes, {} ms", t.elapsed().as_millis()),
        Err(e) => println!("refresh works: no: {e}"),
    }

    let auth = Arc::new(AccountAuth::new(oauth.clone(), store.clone(), ACCOUNT).with_token(token));
    let log = Arc::new(MemoryRequestLog::default());
    for server in &google.servers {
        let hyperme_core::registry::Transport::Http(url) = &server.transport else {
            continue;
        };
        let full = url.trim_end_matches("self").to_owned();
        for endpoint in [full.clone(), format!("{full}self")] {
            let remote = RemoteServer::new(
                &server.id,
                &endpoint,
                auth.clone(),
                log.clone(),
                Arc::new(|| UtcDateTime::now().date()),
            );
            let t = Instant::now();
            match remote.list_tools().await {
                Ok(tools) => {
                    println!(
                        "\n{endpoint}: tools/list ok, {} tools, {} ms",
                        tools.len(),
                        t.elapsed().as_millis()
                    );
                    for tool in tools {
                        let schema = &tool.input_schema;
                        let props: Vec<String> = schema
                            .get("properties")
                            .and_then(Value::as_object)
                            .map(|p| p.keys().cloned().collect())
                            .unwrap_or_default();
                        let required: Vec<String> = schema
                            .get("required")
                            .and_then(Value::as_array)
                            .map(|r| {
                                r.iter()
                                    .filter_map(|v| v.as_str().map(str::to_owned))
                                    .collect()
                            })
                            .unwrap_or_default();
                        let annotations = tool
                            .annotations
                            .as_ref()
                            .map(|a| {
                                format!(
                                    "read_only={:?} destructive={:?}",
                                    a.read_only_hint, a.destructive_hint
                                )
                            })
                            .unwrap_or_default();
                        if std::env::var("SCHEMAS").is_ok_and(|v| v == "1")
                            && ["list_events", "search_threads", "get_thread"]
                                .contains(&tool.name.as_ref())
                        {
                            println!(
                                "    input schema: {}",
                                schema_outline(&Value::Object((**schema).clone()))
                            );
                            if let Some(out) = &tool.output_schema {
                                println!(
                                    "    output schema: {}",
                                    schema_outline(&Value::Object((**out).clone()))
                                );
                            }
                            if let Some(defs) = tool
                                .output_schema
                                .as_ref()
                                .and_then(|o| o.get("$defs"))
                                .and_then(Value::as_object)
                            {
                                for name in [
                                    "Event",
                                    "EventDateTime",
                                    "Thread",
                                    "Message",
                                    "OmittedContentSummary",
                                ] {
                                    if let Some(def) = defs.get(name) {
                                        println!("    $defs.{name}: {}", schema_outline(def));
                                    }
                                }
                            }
                        }
                        println!(
                            "  - {} args: [{}] required: [{}] output_schema: {} {annotations}",
                            tool.name,
                            props.join(", "),
                            required.join(", "),
                            tool.output_schema.is_some()
                        );
                    }
                }
                Err(e) => println!(
                    "\n{endpoint}: tools/list failed after {} ms: {e}",
                    t.elapsed().as_millis()
                ),
            }
        }
    }

    // The preset's pulls, on /mcp/v1self, shapes and counts only.
    let mut servers = Vec::new();
    for server in &google.servers {
        if let hyperme_core::registry::Transport::Http(url) = &server.transport {
            servers.push(RemoteServer::new(
                &server.id,
                url,
                auth.clone(),
                log.clone(),
                Arc::new(|| UtcDateTime::now().date()),
            ));
        }
    }
    let allow: Vec<String> = google.pulls.iter().map(|p| p.tool.clone()).collect();
    let caller = McpCaller::new(servers, allow);
    if std::env::var("PROBE").is_ok_and(|v| v == "1") {
        for view in [
            "THREAD_VIEW_METADATA_ONLY",
            "THREAD_VIEW_MINIMAL",
            "THREAD_VIEW_SUMMARY",
        ] {
            let args = serde_json::json!({ "query": "in:inbox newer_than:7d", "view": view, "pageSize": 5 });
            match caller.call("gmail", "search_threads", args).await {
                Ok(a) => println!(
                    "probe search_threads view={view}: shape {} omitted {} estimate-present {}",
                    shape(&a),
                    a.get("omittedContent")
                        .map(|o| o.to_string())
                        .unwrap_or_default(),
                    a.get("resultCountEstimate").is_some()
                ),
                Err(e) => println!("probe view={view}: {e}"),
            }
        }
        let args = serde_json::json!({ "startTime": "2026-11-27T00:00:00+01:00", "endTime": "2026-12-05T00:00:00+01:00", "pageSize": 5 });
        match caller.call("calendar", "list_events", args).await {
            Ok(a) => println!("probe list_events: shape {}", shape(&a)),
            Err(e) => println!("probe list_events: {e}"),
        }
        return;
    }
    for pull in &google.pulls {
        let args = sync::expand_args(&pull.args, UtcDateTime::now(), &Local);
        let t = Instant::now();
        match caller.call(&pull.server, &pull.tool, args).await {
            Ok(answer) => println!(
                "\npull {}.{}: ok, {} ms, answer shape: {}",
                pull.server,
                pull.tool,
                t.elapsed().as_millis(),
                shape(&answer)
            ),
            Err(e) => println!("\npull {}.{}: failed: {e}", pull.server, pull.tool),
        }
    }
    let now = UtcDateTime::now();
    let events = MemoryEvents::default();
    let mail = MemoryMail::default();
    let tasks = NoTasks;
    for pull in &google.pulls {
        let t = Instant::now();
        let outcome = if pull.into == "events" {
            sync::pull_events(&caller, &events, &tasks, ACCOUNT, pull, now, &Local)
                .await
                .map(|r| format!("{} events mapped, {} skipped", r.events, r.skipped))
        } else {
            sync::pull_mail(&caller, &mail, &tasks, ACCOUNT, pull, now, &Local)
                .await
                .map(|r| {
                    format!(
                        "{} threads mapped, {} skipped, {} to answer",
                        r.threads, r.skipped, r.to_answer
                    )
                })
        };
        match outcome {
            Ok(summary) => println!(
                "mapping {}: {summary}, {} ms",
                pull.into,
                t.elapsed().as_millis()
            ),
            Err(e) => println!("mapping {}: failed: {e}", pull.into),
        }
    }

    if keep {
        println!("\nsign-in kept for the next run (KEEP=1)");
        return;
    }
    let t = Instant::now();
    match setup::disconnect(&oauth, &*store, &mail, ACCOUNT).await {
        Ok(()) => println!(
            "\ndisconnect (revoke + keychain): yes, {} ms",
            t.elapsed().as_millis()
        ),
        Err(e) => println!("\ndisconnect: {e}"),
    }
    println!(
        "refresh token left in keychain: {}",
        store.get(ACCOUNT).ok().flatten().is_some()
    );
}

/// Mirrored tasks are not part of this check.
struct NoTasks;

impl hyperme_core::tasks::TaskStore for NoTasks {
    async fn add(
        &self,
        _: hyperme_core::tasks::NewTask,
    ) -> Result<i64, hyperme_core::tasks::TaskError> {
        Ok(0)
    }
    async fn update(
        &self,
        _: i64,
        _: hyperme_core::tasks::TaskUpdate,
    ) -> Result<(), hyperme_core::tasks::TaskError> {
        Ok(())
    }
    async fn finish(
        &self,
        _: i64,
        _: hyperme_core::tasks::Finish,
        _: i64,
    ) -> Result<(), hyperme_core::tasks::TaskError> {
        Ok(())
    }
    async fn remove(&self, _: i64) -> Result<(), hyperme_core::tasks::TaskError> {
        Ok(())
    }
    async fn upsert_mirror(
        &self,
        _: hyperme_core::tasks::NewTask,
        _: Option<i64>,
    ) -> Result<i64, hyperme_core::tasks::TaskError> {
        Ok(0)
    }
    async fn block(&self, _: i64, _: i64) -> Result<(), hyperme_core::tasks::TaskError> {
        Ok(())
    }
    async fn unblock(&self, _: i64, _: i64) -> Result<(), hyperme_core::tasks::TaskError> {
        Ok(())
    }
    async fn roll_over(&self, _: time::Date) -> Result<Vec<i64>, hyperme_core::tasks::TaskError> {
        Ok(Vec::new())
    }
    async fn snapshot(
        &self,
        _: time::Date,
        _: i64,
    ) -> Result<Vec<hyperme_core::tasks::Task>, hyperme_core::tasks::TaskError> {
        Ok(Vec::new())
    }
}
