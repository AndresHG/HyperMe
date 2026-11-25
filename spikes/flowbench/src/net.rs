//! Network round trips, measured apart from any server's own processing time.
//!
//! A TCP handshake costs one round trip and no server work, so the fastest of a few connects is
//! a fair figure for the network alone. The warm-up request in `cleanup.rs` includes the
//! server's time, which is why the two can differ a lot.

use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::net::{TcpStream, lookup_host};
use tokio::time::{Instant, timeout};

/// Connects per host. The fastest is the round trip; the slowest shows jitter.
const CONNECTS: usize = 5;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Hosts for `--net`: every speech and cleanup host worth comparing from the owner's network.
/// Only host names; nothing is sent beyond a TCP handshake, and no key is used.
pub const DEFAULT_HOSTS: &[&str] = &[
    "api.openai.com",
    "api.groq.com",
    "api.cerebras.ai",
    "api.anthropic.com",
    "eu.rt.speechmatics.com",
    "global.rt.speechmatics.com",
    "api.eu.deepgram.com",
    "api.deepgram.com",
    "streaming.eu.assemblyai.com",
    "streaming.assemblyai.com",
    "api.elevenlabs.io",
];

pub struct Trip {
    pub host: String,
    pub addr: SocketAddr,
    pub dns_ms: u64,
    /// TCP connect times, fastest first.
    pub connect_ms: Vec<u64>,
}

impl Trip {
    pub fn fastest(&self) -> u64 {
        self.connect_ms[0]
    }

    pub fn slowest(&self) -> u64 {
        *self.connect_ms.last().expect("at least one connect")
    }
}

/// The host part of a URL such as `wss://eu.rt.speechmatics.com/v2?x=1`.
pub fn host_of(url: &str) -> Option<&str> {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?']).next()?;
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = host.split(':').next()?;
    (!host.is_empty()).then_some(host)
}

/// Resolves `host` once, then times `CONNECTS` TCP handshakes to port 443.
pub async fn round_trip(host: &str) -> Result<Trip> {
    round_trip_to(host, 443).await
}

async fn round_trip_to(host: &str, port: u16) -> Result<Trip> {
    let start = Instant::now();
    let addr = timeout(CONNECT_TIMEOUT, lookup_host((host, port)))
        .await
        .with_context(|| format!("resolving {host} timed out"))?
        .with_context(|| format!("resolving {host}"))?
        .next()
        .with_context(|| format!("{host} has no address"))?;
    let dns_ms = start.elapsed().as_millis() as u64;
    let mut connect_ms = Vec::with_capacity(CONNECTS);
    for _ in 0..CONNECTS {
        let start = Instant::now();
        let stream = timeout(CONNECT_TIMEOUT, TcpStream::connect(addr))
            .await
            .with_context(|| format!("connecting to {host} timed out"))?
            .with_context(|| format!("connecting to {host}"))?;
        connect_ms.push(start.elapsed().as_millis() as u64);
        drop(stream);
    }
    connect_ms.sort_unstable();
    Ok(Trip {
        host: host.to_owned(),
        addr,
        dns_ms,
        connect_ms,
    })
}

/// One line for the results file: `host 38 ms`, or why it could not be measured.
pub async fn summary(host: &str) -> String {
    match round_trip(host).await {
        Ok(trip) => format!("{host} {} ms", trip.fastest()),
        Err(err) => format!("{host} not measured ({err:#})"),
    }
}

/// `--net`: the round trip to each host, as a table for the terminal.
pub async fn report(hosts: &[String]) -> String {
    let mut out = format!(
        "TCP round trip from this machine, fastest and slowest of {CONNECTS} connects, in ms.\n\
         No key is used and nothing is sent beyond the handshake.\n\n\
         {:<30} {:>8} {:>8} {:>6}  address\n",
        "host", "fastest", "slowest", "dns"
    );
    for host in hosts {
        match round_trip(host).await {
            Ok(trip) => out.push_str(&format!(
                "{:<30} {:>8} {:>8} {:>6}  {}\n",
                trip.host,
                trip.fastest(),
                trip.slowest(),
                trip.dns_ms,
                trip.addr.ip()
            )),
            Err(err) => out.push_str(&format!("{host:<30} failed: {err:#}\n")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[test]
    fn host_is_taken_from_any_url() {
        assert_eq!(
            host_of("wss://eu.rt.speechmatics.com/v2"),
            Some("eu.rt.speechmatics.com")
        );
        assert_eq!(
            host_of("wss://api.openai.com/v1/realtime?intent=transcription"),
            Some("api.openai.com")
        );
        assert_eq!(
            host_of("https://api.groq.com/openai/v1"),
            Some("api.groq.com")
        );
        assert_eq!(host_of("http://127.0.0.1:8080/v1"), Some("127.0.0.1"));
        assert_eq!(host_of("api.groq.com"), Some("api.groq.com"));
        assert_eq!(host_of("https:///nothing"), None);
    }

    #[tokio::test]
    async fn connects_are_timed_and_sorted() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let _ = listener.accept().await;
            }
        });
        let trip = round_trip_to("127.0.0.1", port).await.unwrap();
        assert_eq!(trip.connect_ms.len(), CONNECTS);
        assert!(trip.connect_ms.is_sorted());
        assert!(trip.fastest() <= trip.slowest() && trip.slowest() < 1_000);
        assert_eq!(trip.addr.port(), port);
    }

    #[tokio::test]
    async fn a_closed_port_is_an_error_not_a_time() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        assert!(round_trip_to("127.0.0.1", port).await.is_err());
    }
}
