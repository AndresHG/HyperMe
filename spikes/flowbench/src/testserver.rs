//! A tiny HTTP server on loopback, so tests can run the real HTTP code without the network.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

/// Answers one request per connection with the next of `responses`, each a complete HTTP
/// response that closes the connection. Returns the base URL, and a handle that yields the raw
/// requests once every response has been served.
pub async fn serve(responses: Vec<String>) -> (String, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        let mut requests = Vec::new();
        for response in responses {
            let (mut stream, _) = listener.accept().await.unwrap();
            requests.push(read_request(&mut stream).await);
            stream.write_all(response.as_bytes()).await.unwrap();
            stream.shutdown().await.unwrap();
        }
        requests
    });
    (base_url, handle)
}

/// A `200 OK` response with a body, closing the connection.
pub fn ok(content_type: &str, body: &str) -> String {
    response("200 OK", content_type, body)
}

pub fn response(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// Reads the head and, going by `content-length`, the whole body.
async fn read_request(stream: &mut TcpStream) -> String {
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
        match stream.read(&mut chunk).await.unwrap() {
            0 => break,
            n => received.extend_from_slice(&chunk[..n]),
        }
    }
    String::from_utf8_lossy(&received).into_owned()
}
