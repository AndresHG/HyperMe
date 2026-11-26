//! Slice 5: OpenAI-compatible cleanup against a local HTTP server, and the state machine's
//! deadline and fallback with the real client. Tests cite the acceptance criteria in
//! `specs/25-dictation-core/spec.md`.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hyperme_core::config;
use hyperme_core::dictation::{Engine, Event, Settings, Status};
use hyperme_core::traits::{
    CleanupProvider, Error, ErrorKind, FocusInfo, Inserter, LiveSession, Microphone,
    SpeechProvider, Transcript,
};
use hyperme_providers::cleanup::ChatCleanup;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const KEY: &str = "not-a-real-key-for-tests";

/// How the server answers one request.
#[derive(Clone)]
struct Answer {
    delay: Duration,
    response: String,
}

fn answer(status: &str, content_type: &str, body: &str) -> Answer {
    Answer {
        delay: Duration::ZERO,
        response: format!(
            "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        ),
    }
}

fn models() -> Answer {
    answer(
        "200 OK",
        "application/json",
        r#"{"object":"list","data":[]}"#,
    )
}

/// A streamed answer: reasoning first, as gpt-oss sends it, then the text in pieces.
fn streamed(pieces: &[&str]) -> Answer {
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
async fn serve(models: Answer, chat: Answer) -> (String, Arc<Mutex<Vec<String>>>) {
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
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => received.extend_from_slice(&chunk[..n]),
        }
    }
    String::from_utf8_lossy(&received).into_owned()
}

fn cleanup(endpoint: &str) -> ChatCleanup {
    let mut config = config::Config::default().cleanup;
    config.endpoint = endpoint.to_owned();
    ChatCleanup::new(&config, KEY).unwrap()
}

async fn clean(cleanup: &ChatCleanup, raw: &str, app: Option<&str>) -> Result<String, Error> {
    let mut text = String::new();
    cleanup
        .clean(raw, app, &mut |piece| text.push_str(piece))
        .await?;
    Ok(text)
}

fn assert_no_key(error: &Error) {
    assert!(
        !error.message.contains(KEY),
        "the key leaked: {}",
        error.message
    );
}

/// R-2, R-11: the warm-up opens the connection with the key; the cleanup streams the text
/// and sends the transcript, the model from the config, the reasoning effort and the app.
#[tokio::test]
async fn warms_up_then_streams_the_cleaned_text() {
    let (base, requests) = serve(models(), streamed(&["Hola ", "equipo, ", "mañana."])).await;
    let cleanup = cleanup(&base);
    cleanup.warm().await.unwrap();
    let text = clean(&cleanup, "eh hola equipo mañana", Some("Slack"))
        .await
        .unwrap();
    assert_eq!(text, "Hola equipo, mañana.");

    let requests = requests.lock().unwrap();
    assert!(requests[0].starts_with("GET /v1/models"), "{}", requests[0]);
    assert!(
        requests[0]
            .to_lowercase()
            .contains(&format!("authorization: bearer {KEY}").to_lowercase())
    );
    let chat = &requests[1];
    assert!(chat.starts_with("POST /v1/chat/completions"), "{chat}");
    let body: serde_json::Value =
        serde_json::from_str(chat.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(body["model"], "openai/gpt-oss-20b");
    assert_eq!(body["reasoning_effort"], "low");
    assert_eq!(body["messages"][1]["content"], "eh hola equipo mañana");
    assert!(
        body["messages"][0]["content"]
            .as_str()
            .unwrap()
            .ends_with("pasted into Slack.")
    );
}

/// AC-16 (cleanup, R-21): a 401 on the warm-up or the cleanup names the provider and Settings,
/// never the key, even when the server's body echoes it.
#[tokio::test]
async fn rejected_key_names_the_provider_not_the_key() {
    let echo = format!(r#"{{"error":{{"message":"Invalid API Key {KEY}"}}}}"#);
    let refused = answer("401 Unauthorized", "application/json", &echo);
    let (base, _) = serve(refused.clone(), refused).await;
    let cleanup = cleanup(&base);
    for error in [
        cleanup.warm().await.unwrap_err(),
        clean(&cleanup, "hi there", None).await.unwrap_err(),
    ] {
        assert_eq!(error.kind, ErrorKind::Auth);
        assert!(
            error.message.contains("groq") && error.message.contains("Settings"),
            "{}",
            error.message
        );
        assert_no_key(&error);
    }
    assert!(!format!("{cleanup:?}").contains(KEY));
}

/// AC-16 (cleanup): other HTTP errors, stream errors and an unreachable host are reported.
#[tokio::test]
async fn other_failures_are_reported() {
    let (base, _) = serve(
        models(),
        answer("500 Internal Server Error", "text/plain", "overloaded"),
    )
    .await;
    let error = clean(&cleanup(&base), "hi there", None).await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::Other);
    assert!(
        error.message.contains("500") && error.message.contains("overloaded"),
        "{}",
        error.message
    );

    let stream_error = answer(
        "200 OK",
        "text/event-stream",
        "data: {\"error\":{\"message\":\"model busy\"}}\n\n",
    );
    let (base, _) = serve(models(), stream_error).await;
    let error = clean(&cleanup(&base), "hi there", None).await.unwrap_err();
    assert!(error.message.contains("model busy"), "{}", error.message);

    // Nothing listens on this port.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let closed = format!("http://{}/v1", listener.local_addr().unwrap());
    drop(listener);
    let error = cleanup(&closed).warm().await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::Network, "{}", error.message);
    assert_no_key(&error);
}

// The state machine with the real cleanup client and fakes for the rest.

#[derive(Clone, Default)]
struct Marks(Arc<Mutex<Vec<(String, Instant)>>>);

impl Marks {
    fn push(&self, what: String) {
        self.0.lock().unwrap().push((what, Instant::now()));
    }
    fn at(&self, prefix: &str) -> Option<(String, Instant)> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .find(|(w, _)| w.starts_with(prefix))
            .cloned()
    }
}

struct FixedSpeech {
    transcript: &'static str,
    marks: Marks,
}
struct FixedSession {
    transcript: &'static str,
    marks: Marks,
}
impl SpeechProvider for FixedSpeech {
    type Session = FixedSession;
    async fn open(&self) -> Result<FixedSession, Error> {
        Ok(FixedSession {
            transcript: self.transcript,
            marks: self.marks.clone(),
        })
    }
}
impl LiveSession for FixedSession {
    async fn send_audio(&mut self, _pcm: &[i16]) -> Result<(), Error> {
        Ok(())
    }
    async fn finish(self) -> Result<Transcript, Error> {
        self.marks.push("final transcript".to_owned());
        Ok(Transcript {
            text: self.transcript.to_owned(),
            language: Some("en".to_owned()),
        })
    }
}

struct NoMic;
impl Microphone for NoMic {
    fn start(&mut self) -> Result<(), Error> {
        Ok(())
    }
    fn stop(&mut self) {}
}

struct Paste(Marks);
impl Inserter for Paste {
    async fn paste(&mut self, text: &str) -> Result<(), Error> {
        self.0.push(format!("paste: {text}"));
        Ok(())
    }
}

struct Notes;
impl FocusInfo for Notes {
    fn frontmost_app(&self) -> Option<String> {
        Some("Notes".to_owned())
    }
    fn secure_input(&self) -> bool {
        false
    }
}

const RAW: &str = "so the deploy failed because of the cash invalidation";

/// Runs one dictation and returns the final status, what was pasted, and the time from the
/// final transcript to the paste.
async fn dictate(endpoint: &str, deadline: Duration) -> (Status, String, Duration) {
    let marks = Marks::default();
    let (events, events_rx) = tokio::sync::mpsc::channel(16);
    let (status_tx, mut status) = tokio::sync::mpsc::unbounded_channel();
    let mut settings = Settings::from_config(&config::Config::default());
    settings.min_hold = Duration::ZERO;
    settings.cleanup_deadline = deadline;
    let speech = FixedSpeech {
        transcript: RAW,
        marks: marks.clone(),
    };
    let engine = Engine::new(
        NoMic,
        speech,
        cleanup(endpoint),
        Paste(marks.clone()),
        Notes,
        settings,
        status_tx,
    );
    tokio::spawn(engine.run(events_rx));
    events.send(Event::KeyDown).await.unwrap();
    // Let the warm-up finish, as it does while the user speaks.
    tokio::time::sleep(Duration::from_millis(50)).await;
    events.send(Event::KeyUp).await.unwrap();
    let last = loop {
        if let done @ (Status::Done { .. } | Status::Failed(_)) = status.recv().await.unwrap() {
            break done;
        }
    };
    let (_, final_at) = marks.at("final transcript").unwrap();
    let (pasted, pasted_at) = marks.at("paste: ").unwrap();
    (
        last,
        pasted.trim_start_matches("paste: ").to_owned(),
        pasted_at - final_at,
    )
}

#[tokio::test]
async fn cleaned_text_is_pasted() {
    let (base, _) = serve(
        models(),
        streamed(&[
            "So the deploy failed ",
            "because of the cache invalidation.",
        ]),
    )
    .await;
    let (status, pasted, _) = dictate(&base, Duration::from_millis(1500)).await;
    assert_eq!(status, Status::Done { raw: false });
    assert_eq!(
        pasted,
        "So the deploy failed because of the cache invalidation."
    );
}

/// AC-8 (R-12): when the cleanup server is slower than the deadline, the raw transcript is
/// pasted at the deadline, give or take 20 ms, and marked raw ("Raw text" in the pill).
#[tokio::test]
async fn slow_cleanup_pastes_raw_at_the_deadline() {
    let mut slow = streamed(&["never in time"]);
    slow.delay = Duration::from_secs(3);
    let (base, _) = serve(models(), slow).await;
    let deadline = Duration::from_millis(300);
    let (status, pasted, waited) = dictate(&base, deadline).await;
    assert_eq!(status, Status::Done { raw: true });
    assert_eq!(pasted, RAW);
    let off = waited.abs_diff(deadline);
    assert!(
        off <= Duration::from_millis(20),
        "pasted {waited:?} after the transcript; deadline {deadline:?}"
    );
}

/// AC-8 (R-20): when the cleanup fails, the raw transcript is pasted and marked raw.
#[tokio::test]
async fn failed_cleanup_pastes_raw() {
    let (base, _) = serve(
        models(),
        answer("503 Service Unavailable", "text/plain", "down"),
    )
    .await;
    let (status, pasted, waited) = dictate(&base, Duration::from_millis(1500)).await;
    assert_eq!(status, Status::Done { raw: true });
    assert_eq!(pasted, RAW);
    assert!(
        waited < Duration::from_millis(500),
        "a failure should not wait for the deadline: {waited:?}"
    );
}
