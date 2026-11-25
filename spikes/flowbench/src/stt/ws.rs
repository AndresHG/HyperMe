//! What the streaming providers share: opening an authorised WebSocket, a reader task that ends
//! with the final transcript, and sending without streaming into a dead session.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::net::TcpStream;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

use super::Transcript;

pub type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub type Sink = SplitSink<Socket, Message>;
pub type Stream = SplitStream<Socket>;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// How long to wait for the final transcript after the end of speech before giving up.
const FINAL_TIMEOUT: Duration = Duration::from_secs(30);

/// Opens `url` with `authorization` as the `Authorization` header, such as `Bearer <key>`.
/// The header is marked sensitive, so it never shows in debug output.
pub async fn connect(url: &str, authorization: &str) -> Result<(Sink, Stream)> {
    let mut request = url
        .into_client_request()
        .context("streaming URL is not valid")?;
    let mut auth = HeaderValue::from_str(authorization)
        .context("the API key has characters a header cannot carry")?;
    auth.set_sensitive(true);
    request.headers_mut().insert("Authorization", auth);
    let (socket, _) = timeout(CONNECT_TIMEOUT, connect_async(request))
        .await
        .context("opening the streaming socket timed out")?
        .context("opening the streaming socket")?;
    Ok(socket.split())
}

/// What one server event means for the dictation.
#[derive(Debug, PartialEq)]
pub enum Step {
    Continue,
    Done(Transcript),
    Failed(String),
}

/// Reads text events, passing each to `on_event`, until it says the transcript is done.
pub async fn read_until_final(
    mut stream: Stream,
    trace: bool,
    mut on_event: impl FnMut(&str) -> Step + Send,
) -> Result<Transcript> {
    while let Some(message) = stream.next().await {
        match message.context("streaming socket")? {
            Message::Text(event) => {
                if trace {
                    eprintln!("    [stream] {}", event_kind(event.as_str()));
                }
                match on_event(event.as_str()) {
                    Step::Continue => {}
                    Step::Done(transcript) => return Ok(transcript),
                    Step::Failed(body) => bail!("the provider reported a failure: {body}"),
                }
            }
            Message::Close(frame) => {
                bail!("the socket closed before the final transcript: {frame:?}")
            }
            _ => {}
        }
    }
    bail!("the socket ended before the final transcript")
}

/// The `type` or `message` field of an event, for tracing.
pub fn event_kind(event: &str) -> String {
    serde_json::from_str::<Value>(event)
        .ok()
        .and_then(|v| {
            v["type"]
                .as_str()
                .or(v["message"].as_str())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "(not JSON)".to_owned())
}

/// One open streaming session: the sending half and the task reading the other.
pub struct LiveSocket {
    sink: Sink,
    /// Ends with the final transcript. `None` once its result has been taken: a task can be
    /// awaited only once.
    reader: Option<JoinHandle<Result<Transcript>>>,
}

impl LiveSocket {
    pub fn new(sink: Sink, reader: JoinHandle<Result<Transcript>>) -> LiveSocket {
        LiveSocket {
            sink,
            reader: Some(reader),
        }
    }

    fn reader_finished(&self) -> bool {
        self.reader.as_ref().is_some_and(JoinHandle::is_finished)
    }

    async fn reader_result(&mut self) -> Result<Transcript> {
        let Some(mut reader) = self.reader.take() else {
            bail!("the session has already ended");
        };
        match timeout(FINAL_TIMEOUT, &mut reader).await {
            Ok(joined) => joined.context("transcript reader stopped")?,
            Err(_) => {
                reader.abort();
                bail!(
                    "no final transcript within {} s of the end of speech",
                    FINAL_TIMEOUT.as_secs()
                )
            }
        }
    }

    /// Sends one message during the speech. If the reader has already ended, which before the
    /// end of speech only happens on a failure, reports why instead of streaming into a dead
    /// session.
    pub async fn send(&mut self, message: Message) -> Result<()> {
        if self.reader.is_none() || self.reader_finished() {
            let result = self.reader_result().await;
            self.close().await;
            return match result {
                Ok(_) => bail!("a final transcript arrived before the end of speech"),
                Err(err) => Err(err),
            };
        }
        self.sink.send(message).await.context("sending audio")
    }

    /// Key release: sends the end-of-speech messages and waits for the final transcript.
    pub async fn finish(mut self, messages: Vec<Message>, what: &str) -> Result<Transcript> {
        // A failure already taken by `send` closed the socket; there is nothing to send into.
        if self.reader.is_none() {
            bail!("the session has already ended");
        }
        for message in messages {
            if let Err(err) = self.sink.send(message).await {
                // If the provider said why it hung up, that message is the useful one.
                if !self.reader_finished() {
                    return Err(err).with_context(|| format!("sending {what}"));
                }
                break;
            }
        }
        let result = self.reader_result().await;
        if result.is_err() {
            self.close().await;
        }
        result
    }

    /// Sends a Close frame, so a failed session does not stay open and count against the
    /// provider's concurrent quota. The socket may already be gone, so a failure is ignored.
    async fn close(&mut self) {
        let _ = self.sink.send(Message::Close(None)).await;
    }
}

impl Drop for LiveSocket {
    fn drop(&mut self) {
        // An abandoned session must not keep its socket open in the background.
        if let Some(reader) = &self.reader {
            reader.abort();
        }
    }
}

#[cfg(test)]
pub mod testserver {
    //! A WebSocket server on loopback that answers each client message with canned events.

    use super::*;
    use tokio::task::JoinHandle;
    use tokio_tungstenite::accept_hdr_async;
    use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};

    /// A message the client sent: JSON text, or the length of a binary frame.
    #[derive(Debug, PartialEq)]
    pub enum Got {
        Json(Value),
        Binary(Vec<u8>),
        /// The client closed the socket with a Close frame.
        Close,
    }

    /// What the server saw: the request path and query, the authorization header, and every
    /// client message.
    pub struct Seen {
        pub path_and_query: String,
        pub auth: String,
        pub got: Vec<Got>,
    }

    /// Serves one connection. `greeting` is sent as soon as the socket opens; `reply` answers
    /// each client message.
    #[allow(clippy::result_large_err)]
    pub async fn serve(
        greeting: Vec<Value>,
        reply: fn(&Got) -> Vec<Value>,
    ) -> (String, JoinHandle<Seen>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("ws://{}", listener.local_addr().unwrap());
        let handle = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let (mut path_and_query, mut auth) = (String::new(), String::new());
            let mut socket = accept_hdr_async(tcp, |request: &Request, response: Response| {
                path_and_query = request
                    .uri()
                    .path_and_query()
                    .map(|pq| pq.as_str().to_owned())
                    .unwrap_or_default();
                auth = request.headers()["authorization"]
                    .to_str()
                    .unwrap()
                    .to_owned();
                Ok(response)
            })
            .await
            .unwrap();
            for event in greeting {
                socket.send(Message::text(event.to_string())).await.unwrap();
            }
            let mut got = Vec::new();
            while let Some(Ok(message)) = socket.next().await {
                let message = match message {
                    Message::Text(text) => Got::Json(serde_json::from_str(text.as_str()).unwrap()),
                    Message::Binary(bytes) => Got::Binary(bytes.to_vec()),
                    Message::Close(_) => {
                        got.push(Got::Close);
                        break;
                    }
                    _ => continue,
                };
                for answer in reply(&message) {
                    if socket
                        .send(Message::text(answer.to_string()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                got.push(message);
            }
            Seen {
                path_and_query,
                auth,
                got,
            }
        });
        (base, handle)
    }
}
