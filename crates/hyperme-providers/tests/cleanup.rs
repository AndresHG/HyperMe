//! Slice 5: OpenAI-compatible cleanup against a local HTTP server, and the state machine's
//! deadline and fallback with the real client. Tests cite the acceptance criteria in
//! `specs/25-dictation-core/spec.md`.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hyperme_core::config;
use hyperme_core::dictation::{Engine, Event, Parts, Settings, Status, fixed};
use hyperme_core::traits::{
    CleanupProvider, DictationId, Error, ErrorKind, FocusInfo, History, Inserter, LiveSession,
    Microphone, Sound, Sounds, SpeechProvider, Transcript,
};
use hyperme_providers::cleanup::ChatCleanup;
use tokio::net::TcpListener;

mod chat_server;
use chat_server::*;

const KEY: &str = "not-a-real-key-for-tests";

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
    async fn copy(&mut self, text: &str) -> Result<(), Error> {
        self.0.push(format!("copy: {text}"));
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
    dictate_with(cleanup(endpoint), deadline).await
}

async fn dictate_with(cleanup: ChatCleanup, deadline: Duration) -> (Status, String, Duration) {
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
    let parts = Parts {
        mic: NoMic,
        providers: fixed(speech, cleanup),
        inserter: Paste(marks.clone()),
        focus: Notes,
        history: NoHistory,
        sounds: Silent,
    };
    let engine = Engine::new(parts, settings, status_tx);
    tokio::spawn(engine.run(events_rx));
    events.send(Event::KeyDown).await.unwrap();
    // Let the warm-up finish, as it does while the user speaks.
    tokio::time::sleep(Duration::from_millis(50)).await;
    events.send(Event::KeyUp).await.unwrap();
    let last = loop {
        if let done @ (Status::Done { .. } | Status::Failed { .. }) = status.recv().await.unwrap() {
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
    assert_eq!(
        status,
        Status::Done {
            raw: false,
            via: None
        }
    );
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
    assert_eq!(
        status,
        Status::Done {
            raw: true,
            via: None
        }
    );
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
    assert_eq!(
        status,
        Status::Done {
            raw: true,
            via: None
        }
    );
    assert_eq!(pasted, RAW);
    assert!(
        waited < Duration::from_millis(500),
        "a failure should not wait for the deadline: {waited:?}"
    );
}

/// No history: `begin` fails, so the state machine runs without one.
struct NoHistory;
impl History for NoHistory {
    fn begin(&mut self, _app: Option<&str>) -> Result<DictationId, Error> {
        Err(Error {
            kind: ErrorKind::Device,
            message: "no history in this test".to_owned(),
        })
    }
    fn append(&mut self, _id: DictationId, _pcm: &[i16]) -> Result<(), Error> {
        Ok(())
    }
    fn done(
        &mut self,
        _id: DictationId,
        _text: &str,
        _language: Option<&str>,
    ) -> Result<(), Error> {
        Ok(())
    }
    fn failed(&mut self, _id: DictationId, _message: &str) -> Result<(), Error> {
        Ok(())
    }
    fn discard(&mut self, _id: DictationId) -> Result<(), Error> {
        Ok(())
    }
    fn audio(&self, _id: DictationId) -> Result<Vec<i16>, Error> {
        Ok(Vec::new())
    }
    fn app(&self, _id: DictationId) -> Result<Option<String>, Error> {
        Ok(None)
    }
    fn replace_text(
        &mut self,
        _id: DictationId,
        _text: &str,
        _language: Option<&str>,
    ) -> Result<(), Error> {
        Ok(())
    }
}

struct Silent;
impl Sounds for Silent {
    fn play(&self, _sound: Sound) {}
}

/// Provider settings AC-9 (R-23): a local server, which has no key, is held to the same
/// deadline and falls back to the raw transcript when it is slow.
#[tokio::test]
async fn ac9_a_slow_local_server_falls_back_to_raw() {
    let mut slow = streamed(&["never in time"]);
    slow.delay = Duration::from_secs(3);
    let (base, requests) = serve(models(), slow).await;
    let mut config = config::Config::default().cleanup;
    config.endpoint = base;
    let keyless = ChatCleanup::new(&config, "").unwrap();
    let deadline = Duration::from_millis(300);
    let (status, pasted, waited) = dictate_with(keyless, deadline).await;
    assert_eq!(
        status,
        Status::Done {
            raw: true,
            via: None
        }
    );
    assert_eq!(pasted, RAW);
    assert!(
        waited.abs_diff(deadline) <= Duration::from_millis(20),
        "{waited:?}"
    );
    // No key, so no Authorization header at all.
    for request in requests.lock().unwrap().iter() {
        assert!(
            !request.to_lowercase().contains("authorization"),
            "{request}"
        );
    }
}
