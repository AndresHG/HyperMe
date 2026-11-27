//! What the streaming speech clients share: the WebSocket handshake with the key in its
//! `Authorization` header, and errors that name the provider and never the key (R-21, R-25).

use std::fmt;
use std::time::Duration;

use futures_util::StreamExt;
use futures_util::stream::{SplitSink, SplitStream};
use hyperme_core::traits::{Error, ErrorKind};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{HeaderValue, StatusCode};
use tokio_tungstenite::tungstenite::{self, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub(crate) type Sink = SplitSink<Socket, Message>;
pub(crate) type Stream = SplitStream<Socket>;

pub(crate) fn provider_error(provider: &str, kind: ErrorKind, what: impl fmt::Display) -> Error {
    Error {
        kind,
        message: format!("{provider}: {what}"),
    }
}

/// R-21: which provider, and where to fix it, without the key.
pub(crate) fn auth_error(provider: &str) -> Error {
    provider_error(
        provider,
        ErrorKind::Auth,
        "the key was rejected. Check it in Settings",
    )
}

pub(crate) fn dropped(provider: &str, e: impl fmt::Display) -> Error {
    provider_error(
        provider,
        ErrorKind::Network,
        format!("the connection dropped: {e}"),
    )
}

/// Opens the socket with `Authorization: Bearer <key>`. A 401 is a key error; other refusals
/// give only the status, since a refusal's body can echo part of the key.
pub(crate) async fn connect(
    provider: &str,
    endpoint: &str,
    key: &str,
) -> Result<(Sink, Stream), Error> {
    let mut request = endpoint.into_client_request().map_err(|e| {
        provider_error(
            provider,
            ErrorKind::Other,
            format!("the endpoint is not valid: {e}"),
        )
    })?;
    let mut auth = HeaderValue::from_str(&format!("Bearer {key}")).map_err(|_| {
        provider_error(
            provider,
            ErrorKind::Auth,
            "the key has characters a header cannot carry. Check it in Settings",
        )
    })?;
    // Keeps the key out of any debug output of the request.
    auth.set_sensitive(true);
    request.headers_mut().insert("Authorization", auth);
    let connected = timeout(CONNECT_TIMEOUT, connect_async(request))
        .await
        .map_err(|_| provider_error(provider, ErrorKind::Network, "connecting timed out"))?;
    match connected {
        Ok((socket, _)) => Ok(socket.split()),
        Err(tungstenite::Error::Http(response))
            if response.status() == StatusCode::UNAUTHORIZED =>
        {
            Err(auth_error(provider))
        }
        Err(tungstenite::Error::Http(response)) => Err(provider_error(
            provider,
            ErrorKind::Other,
            format!("the connection was refused with HTTP {}", response.status()),
        )),
        Err(other) => Err(provider_error(
            provider,
            ErrorKind::Network,
            format!("could not connect: {other}"),
        )),
    }
}
