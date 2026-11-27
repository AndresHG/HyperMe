//! A local OpenAI-compatible server for tests: `/v1/models` and `/v1/chat/completions` on a
//! loopback port, with canned answers. No test calls a paid API (CLAUDE.md).
#![allow(dead_code)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// How the server answers one request.
#[derive(Clone)]
pub struct Answer {
    pub delay: Duration,
    pub response: String,
}

pub fn answer(status: &str, content_type: &str, body: &str) -> Answer {
    Answer {
        delay: Duration::ZERO,
        response: format!(
            "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        ),
    }
}

pub fn models() -> Answer {
    answer(
        "200 OK",
        "application/json",
        r#"{"object":"list","data":[]}"#,
    )
}

/// A streamed answer: reasoning first, as gpt-oss sends it, then the text in pieces.
pub fn streamed(pieces: &[&str]) -> Answer {
    let mut body = String::from(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"reasoning\":\"Clean it.\"}}]}\n\n",
    );
    for piece in pieces {
        let chunk = serde_json::json!({"choices": [{"index": 0, "delta": {"content": piece}}]});
        body.push_str(&format!("data: {chunk}\n\n"));
    }
    body.push_str("data: [DONE]\n\n");
    answer("200 OK", "text/event-stream", &body)
}

/// Serves every connection on loopback: `GET /v1/models` gets `models`, anything else `chat`.
/// Returns the base URL and the raw requests seen.
pub async fn serve(models: Answer, chat: Answer) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let (models, chat, seen) = (models.clone(), chat.clone(), seen.clone());
            tokio::spawn(async move {
                let request = read_request(&mut stream).await;
                let reply = if request.starts_with("GET /v1/models") {
                    models
                } else {
                    chat
                };
                seen.lock().unwrap().push(request);
                tokio::time::sleep(reply.delay).await;
                // The client may have given up at its deadline.
                let _ = stream.write_all(reply.response.as_bytes()).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    (base, requests)
}

/// Reads the head and, going by `content-length`, the whole body.
pub async fn read_request(stream: &mut TcpStream) -> String {
    let mut received = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        if let Some(head_end) = received.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&received[..head_end]).to_lowercase();
            let body_len = head
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .map_or(0, |value| value.trim().parse::<usize>().unwrap());
            if received.len() >= head_end + 4 + body_len {
                break;
            }
        }
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => received.extend_from_slice(&chunk[..n]),
        }
    }
    String::from_utf8_lossy(&received).into_owned()
}
