//! Looking for a cleanup server on this Mac (specs/96-provider-settings, R-22). Called from
//! Settings when the user picks Local server, never on the dictation path.

use std::time::Duration;

use futures_util::future::join_all;

use crate::cleanup::list_models_within;

/// How long each port gets to answer. A server on this Mac answers in milliseconds; a closed
/// port is refused at once, so this only bounds one that accepts and then hangs.
const PORT_LIMIT: Duration = Duration::from_millis(500);

/// A server that answered on a port, with the models it offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub port: u16,
    /// The base address to put in `cleanup.endpoint`: `http://127.0.0.1:{port}/v1`.
    pub endpoint: String,
    pub models: Vec<String>,
}

/// Asks each port, all at once, for `GET /v1/models` with no key, and keeps those that answer,
/// in the order given.
pub async fn discover(ports: &[u16]) -> Vec<Found> {
    let asks = ports.iter().map(|&port| async move {
        let endpoint = format!("http://127.0.0.1:{port}/v1");
        list_models_within("Local server", &endpoint, "", PORT_LIMIT)
            .await
            .ok()
            .map(|models| Found {
                port,
                endpoint,
                models,
            })
    });
    join_all(asks).await.into_iter().flatten().collect()
}
