//! A WebSocket server on loopback that answers each client message with scripted replies, so
//! tests run the real client code with no network and no key (AC-22).

#![allow(dead_code)]

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::task::JoinHandle;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

/// A message the client sent.
#[derive(Debug, Clone, PartialEq)]
pub enum Got {
    Json(Value),
    Binary(Vec<u8>),
    Close,
}

impl Got {
    pub fn is(&self, message: &str) -> bool {
        matches!(self, Got::Json(v) if v["message"] == message)
    }
}

/// What the server does in answer to one client message.
pub enum Reply {
    Json(Value),
    /// A Close frame with this code.
    Close(u16),
    /// Drops the TCP connection without a Close frame.
    Drop,
}

pub struct Seen {
    pub auth: String,
    pub got: Vec<Got>,
}

impl Seen {
    pub fn binary_frames(&self) -> Vec<&Vec<u8>> {
        self.got
            .iter()
            .filter_map(|g| {
                if let Got::Binary(b) = g {
                    Some(b)
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Answers `StartRecognition` with `RecognitionStarted`; other messages go to `reply`.
pub fn started_then(
    mut reply: impl FnMut(&Got) -> Vec<Reply> + Send + 'static,
) -> impl FnMut(&Got) -> Vec<Reply> + Send + 'static {
    move |got| {
        if got.is("StartRecognition") {
            vec![Reply::Json(
                serde_json::json!({"message": "RecognitionStarted", "id": "test"}),
            )]
        } else {
            reply(got)
        }
    }
}

/// Serves one connection. With `reject_with`, the handshake answers that HTTP status.
// The handshake callback's error type is tungstenite's, large by design.
#[allow(clippy::result_large_err)]
pub async fn serve(
    reject_with: Option<u16>,
    mut reply: impl FnMut(&Got) -> Vec<Reply> + Send + 'static,
) -> (String, JoinHandle<Seen>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("ws://{}/v2", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut auth = String::new();
        let accepted = accept_hdr_async(tcp, |request: &Request, response: Response| {
            auth = request
                .headers()
                .get("authorization")
                .map(|v| v.to_str().unwrap().to_owned())
                .unwrap_or_default();
            match reject_with {
                Some(code) => {
                    let mut refusal = ErrorResponse::new(Some("refused".to_owned()));
                    *refusal.status_mut() = StatusCode::from_u16(code).unwrap();
                    Err(refusal)
                }
                None => Ok(response),
            }
        })
        .await;
        let mut got = Vec::new();
        let Ok(mut socket) = accepted else {
            return Seen { auth, got };
        };
        'outer: while let Some(Ok(message)) = socket.next().await {
            let message = match message {
                Message::Text(text) => Got::Json(serde_json::from_str(text.as_str()).unwrap()),
                Message::Binary(bytes) => Got::Binary(bytes.to_vec()),
                Message::Close(_) => {
                    got.push(Got::Close);
                    break;
                }
                _ => continue,
            };
            let replies = reply(&message);
            got.push(message);
            for answer in replies {
                match answer {
                    Reply::Json(value) => {
                        if socket.send(Message::text(value.to_string())).await.is_err() {
                            break 'outer;
                        }
                    }
                    Reply::Close(code) => {
                        let frame = CloseFrame {
                            code: CloseCode::from(code),
                            reason: "test".into(),
                        };
                        let _ = socket.send(Message::Close(Some(frame))).await;
                        break 'outer;
                    }
                    Reply::Drop => break 'outer,
                }
            }
        }
        Seen { auth, got }
    });
    (base, handle)
}
