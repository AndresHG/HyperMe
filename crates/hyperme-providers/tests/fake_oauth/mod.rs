//! A fake OAuth 2.0 server on loopback for the sign-in tests (specs/97-google-connectors,
//! design.md, "Tests"): authorize, token (code and refresh) and revoke, with switches to expire
//! access tokens and revoke refresh tokens. It checks PKCE S256 itself, so a client that sends
//! the wrong verifier gets `invalid_grant`. No network, no key.

#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use reqwest::Url;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

struct Grant {
    challenge: String,
    redirect_uri: String,
}

#[derive(Default)]
struct State {
    grants: HashMap<String, Grant>,
    refresh_tokens: HashSet<String>,
    issued_refresh: Vec<String>,
    issued_access: Vec<String>,
    access_ttl: i64,
    next: u32,
    authorize_queries: Vec<HashMap<String, String>>,
    token_requests: Vec<HashMap<String, String>>,
    revoked: Vec<String>,
}

/// The running server. Dropping it leaves the task running until the test ends.
#[derive(Clone)]
pub struct FakeOAuth {
    base: String,
    state: Arc<Mutex<State>>,
}

impl FakeOAuth {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(State {
            access_ttl: 3600,
            ..State::default()
        }));
        let shared = state.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let shared = shared.clone();
                tokio::spawn(async move { serve(stream, shared).await });
            }
        });
        FakeOAuth { base, state }
    }

    pub fn auth_url(&self) -> String {
        format!("{}/authorize", self.base)
    }

    pub fn token_url(&self) -> String {
        format!("{}/token", self.base)
    }

    pub fn revoke_url(&self) -> String {
        format!("{}/revoke", self.base)
    }

    /// Seconds each new access token lives; 0 makes it expire at once.
    pub fn set_access_ttl(&self, seconds: i64) {
        self.state.lock().unwrap().access_ttl = seconds;
    }

    /// Refuses every refresh token issued so far, as when the user removes access at Google.
    pub fn revoke_all_refresh_tokens(&self) {
        self.state.lock().unwrap().refresh_tokens.clear();
    }

    pub fn issued_refresh_tokens(&self) -> Vec<String> {
        self.state.lock().unwrap().issued_refresh.clone()
    }

    pub fn issued_access_tokens(&self) -> Vec<String> {
        self.state.lock().unwrap().issued_access.clone()
    }

    pub fn authorize_queries(&self) -> Vec<HashMap<String, String>> {
        self.state.lock().unwrap().authorize_queries.clone()
    }

    pub fn token_requests(&self) -> Vec<HashMap<String, String>> {
        self.state.lock().unwrap().token_requests.clone()
    }

    pub fn refresh_calls(&self) -> usize {
        self.token_requests()
            .iter()
            .filter(|r| r.get("grant_type").map(String::as_str) == Some("refresh_token"))
            .count()
    }

    pub fn revoked(&self) -> Vec<String> {
        self.state.lock().unwrap().revoked.clone()
    }
}

fn pairs(query: &str) -> HashMap<String, String> {
    let url = Url::parse(&format!("http://x/?{query}")).unwrap();
    url.query_pairs().into_owned().collect()
}

fn reply(status: &str, extra_headers: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\n{extra_headers}content-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn invalid_grant() -> String {
    reply("400 Bad Request", "", r#"{"error":"invalid_grant"}"#)
}

async fn serve(mut stream: TcpStream, state: Arc<Mutex<State>>) {
    let (head, body) = read_request(&mut stream).await;
    let first = head.lines().next().unwrap_or_default().to_owned();
    let mut parts = first.split(' ');
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or_default();
    let (path, query) = target.split_once('?').unwrap_or((target, ""));

    let response = {
        let mut s = state.lock().unwrap();
        match (method, path) {
            ("GET", "/authorize") => {
                let q = pairs(query);
                s.authorize_queries.push(q.clone());
                if q.get("code_challenge_method").map(String::as_str) != Some("S256")
                    || q.get("response_type").map(String::as_str) != Some("code")
                {
                    reply("400 Bad Request", "", r#"{"error":"invalid_request"}"#)
                } else {
                    s.next += 1;
                    let code = format!("fake-code-{}", s.next);
                    let redirect_uri = q["redirect_uri"].clone();
                    s.grants.insert(
                        code.clone(),
                        Grant {
                            challenge: q["code_challenge"].clone(),
                            redirect_uri: redirect_uri.clone(),
                        },
                    );
                    let mut location = Url::parse(&redirect_uri).unwrap();
                    location
                        .query_pairs_mut()
                        .append_pair("code", &code)
                        .append_pair("state", &q["state"]);
                    reply("302 Found", &format!("location: {location}\r\n"), "")
                }
            }
            ("POST", "/token") => {
                let form = pairs(&body);
                s.token_requests.push(form.clone());
                match form.get("grant_type").map(String::as_str) {
                    Some("authorization_code") => {
                        let grant = s.grants.remove(form.get("code").map_or("", |c| c));
                        let ok = grant.is_some_and(|g| {
                            let verifier = form.get("code_verifier").map_or("", |v| v);
                            let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier));
                            challenge == g.challenge
                                && form.get("redirect_uri") == Some(&g.redirect_uri)
                        });
                        if ok {
                            s.next += 1;
                            let n = s.next;
                            let access = format!("fake-access-{n}");
                            let refresh = format!("fake-refresh-{n}");
                            s.refresh_tokens.insert(refresh.clone());
                            s.issued_refresh.push(refresh.clone());
                            s.issued_access.push(access.clone());
                            let body = serde_json::json!({
                                "access_token": access,
                                "expires_in": s.access_ttl,
                                "refresh_token": refresh,
                                "token_type": "Bearer",
                                "scope": "read",
                            });
                            reply("200 OK", "", &body.to_string())
                        } else {
                            invalid_grant()
                        }
                    }
                    Some("refresh_token") => {
                        let token = form.get("refresh_token").map_or("", |t| t);
                        if s.refresh_tokens.contains(token) {
                            s.next += 1;
                            let access = format!("fake-access-{}", s.next);
                            s.issued_access.push(access.clone());
                            let body = serde_json::json!({
                                "access_token": access,
                                "expires_in": s.access_ttl,
                                "token_type": "Bearer",
                            });
                            reply("200 OK", "", &body.to_string())
                        } else {
                            invalid_grant()
                        }
                    }
                    _ => reply(
                        "400 Bad Request",
                        "",
                        r#"{"error":"unsupported_grant_type"}"#,
                    ),
                }
            }
            ("POST", "/revoke") => {
                let form = pairs(&body);
                let token = form.get("token").cloned().unwrap_or_default();
                s.refresh_tokens.remove(&token);
                s.revoked.push(token);
                reply("200 OK", "", "{}")
            }
            _ => reply("404 Not Found", "", "{}"),
        }
    };
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

/// Reads the head and, going by `content-length`, the whole body.
async fn read_request(stream: &mut TcpStream) -> (String, String) {
    let mut received = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        if let Some(head_end) = received.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&received[..head_end]).to_string();
            let body_len = head
                .to_lowercase()
                .lines()
                .find_map(|line| line.strip_prefix("content-length:").map(str::to_owned))
                .map_or(0, |value| value.trim().parse::<usize>().unwrap());
            if received.len() >= head_end + 4 + body_len {
                let body =
                    String::from_utf8_lossy(&received[head_end + 4..head_end + 4 + body_len])
                        .to_string();
                return (head, body);
            }
        }
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return (String::from_utf8_lossy(&received).to_string(), String::new()),
            Ok(n) => received.extend_from_slice(&chunk[..n]),
        }
    }
}
