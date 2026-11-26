//! Slice 4: Speechmatics streaming against a local WebSocket server. Tests cite the acceptance
//! criteria in `specs/25-dictation-core/spec.md`.

mod common;

use std::sync::{Arc, Mutex};

use common::{Got, Reply, serve, started_then};
use hyperme_core::config;
use hyperme_core::dictation::{Engine, Event, Parts, Settings, Status};
use hyperme_core::traits::{
    CleanupProvider, DictationId, Error, ErrorKind, FocusInfo, History, Inserter, LiveSession,
    Microphone, Sound, Sounds, SpeechProvider, TimingLine, TimingLog,
};
use hyperme_providers::speechmatics::{MAX_CHUNK_SAMPLES, Speechmatics};
use serde_json::{Value, json};

const KEY: &str = "not-a-real-key-for-tests";

fn speech(endpoint: &str) -> Speechmatics {
    let mut config = config::Config::default().speech;
    config.endpoint = endpoint.to_owned();
    Speechmatics::new(&config, KEY.to_owned())
}

fn word(content: &str, language: &str) -> Value {
    json!({"type": "word", "alternatives": [{"content": content, "language": language}]})
}

fn mark(content: &str) -> Value {
    json!({"type": "punctuation", "attaches_to": "previous", "alternatives": [{"content": content}]})
}

/// A forced final transcript, as Speechmatics answers ForceEndOfUtterance.
fn forced(results: Vec<Value>) -> Vec<Reply> {
    vec![
        Reply::Json(json!({"message": "AddTranscript", "forced": true, "results": results})),
        Reply::Json(json!({"message": "EndOfUtterance", "forced": true})),
    ]
}

/// AC-3: audio reaches the server in chunks of 100 ms or less, including audio buffered while
/// the session opened, which the state machine sends in one piece.
#[tokio::test]
async fn audio_goes_out_in_chunks_of_at_most_100_ms() {
    let (base, server) = serve(
        None,
        started_then(|got| {
            if got.is("ForceEndOfUtterance") {
                forced(vec![word("hi", "en")])
            } else {
                vec![]
            }
        }),
    )
    .await;
    let mut session = speech(&base).open().await.unwrap();
    let buffered = vec![7i16; 7200]; // 300 ms at 24 kHz.
    session.send_audio(&buffered).await.unwrap();
    for _ in 0..3 {
        session.send_audio(&[1i16; 480]).await.unwrap(); // 20 ms frames.
    }
    session.finish().await.unwrap();

    let seen = server.await.unwrap();
    let frames = seen.binary_frames();
    assert_eq!(frames.len(), 6);
    assert!(
        frames.iter().all(|f| f.len() <= MAX_CHUNK_SAMPLES * 2),
        "a frame over 100 ms"
    );
    let total: usize = frames.iter().map(|f| f.len()).sum();
    assert_eq!(total, (7200 + 3 * 480) * 2);
    // Little-endian PCM16.
    assert_eq!(&frames[0][..2], &[7, 0]);
    assert!(
        seen.got
            .iter()
            .any(|g| *g == Got::Json(json!({"message": "EndOfStream", "last_seq_no": 6})))
    );
}

/// AC-6 (R-9): on finish the client sends the explicit finalize messages after the audio, and
/// returns only the final transcript.
#[tokio::test]
async fn finish_sends_finalize_then_returns_the_final_transcript() {
    let (base, server) = serve(
        None,
        started_then(|got| match got {
            Got::Binary(_) => vec![Reply::Json(json!({"message": "AudioAdded", "seq_no": 1}))],
            g if g.is("ForceEndOfUtterance") => forced(vec![
                word("Send", "en"),
                word("it", "en"),
                word("today", "en"),
                mark("."),
            ]),
            _ => vec![],
        }),
    )
    .await;
    let mut session = speech(&base).open().await.unwrap();
    session.send_audio(&[1, -2]).await.unwrap();
    let transcript = session.finish().await.unwrap();
    assert_eq!(transcript.text, "Send it today.");

    let seen = server.await.unwrap();
    assert_eq!(seen.auth, format!("Bearer {KEY}"));
    assert!(seen.got[0].is("StartRecognition"));
    assert_eq!(seen.got[1], Got::Binary(vec![1, 0, 0xFE, 0xFF]));
    assert!(seen.got[2].is("ForceEndOfUtterance"));
    assert_eq!(
        seen.got[3],
        Got::Json(json!({"message": "EndOfStream", "last_seq_no": 1}))
    );
}

/// AC-6: when the forced end of utterance gets no answer, EndOfTranscript ends the dictation.
#[tokio::test]
async fn end_of_transcript_also_ends_the_dictation() {
    let (base, _server) = serve(
        None,
        started_then(|got| {
            if got.is("EndOfStream") {
                vec![
                    Reply::Json(json!({"message": "AddTranscript", "results": [word("Hola", "es"), mark(".")]})),
                    Reply::Json(json!({"message": "EndOfTranscript"})),
                ]
            } else {
                vec![]
            }
        }),
    )
    .await;
    let mut session = speech(&base).open().await.unwrap();
    session.send_audio(&[0; 480]).await.unwrap();
    assert_eq!(session.finish().await.unwrap().text, "Hola.");
}

/// AC-7 (R-10): no language is sent, and an English and a Spanish dictation each come back in
/// their own language. The fake server answers by the first sample: 1 is English, 2 Spanish.
#[tokio::test]
async fn english_and_spanish_come_back_in_their_own_language() {
    for (first_sample, text, language) in [
        (1i16, "See you tomorrow.", "en"),
        (2, "Hasta mañana.", "es"),
    ] {
        let lang = Arc::new(Mutex::new(None::<i16>));
        let seen_lang = lang.clone();
        let (base, server) = serve(
            None,
            started_then(move |got| match got {
                Got::Binary(bytes) => {
                    seen_lang
                        .lock()
                        .unwrap()
                        .get_or_insert(i16::from_le_bytes([bytes[0], bytes[1]]));
                    vec![]
                }
                g if g.is("ForceEndOfUtterance") => match *seen_lang.lock().unwrap() {
                    Some(1) => forced(vec![
                        word("See", "en"),
                        word("you", "en"),
                        word("tomorrow", "en"),
                        mark("."),
                    ]),
                    _ => forced(vec![word("Hasta", "es"), word("mañana", "es"), mark(".")]),
                },
                _ => vec![],
            }),
        )
        .await;
        let mut session = speech(&base).open().await.unwrap();
        session.send_audio(&[first_sample; 480]).await.unwrap();
        let transcript = session.finish().await.unwrap();
        assert_eq!(transcript.text, text);
        assert_eq!(transcript.language.as_deref(), Some(language));

        let seen = server.await.unwrap();
        let Got::Json(start) = &seen.got[0] else {
            panic!("StartRecognition first")
        };
        let config = &start["transcription_config"];
        assert_eq!(config["language"], "multi", "a specific language was sent");
        assert!(config.get("language_hints").is_none());
        drop(lang);
    }
}

fn assert_no_key(error: &Error) {
    assert!(
        !error.message.contains(KEY),
        "the key leaked: {}",
        error.message
    );
    assert!(
        !format!("{error:?}").contains(KEY),
        "the key leaked into Debug: {error:?}"
    );
}

/// AC-16 (speech, R-21): a rejected key, at the handshake or in the session, names the provider
/// and Settings, never the key.
#[tokio::test]
async fn rejected_key_names_the_provider_not_the_key() {
    let (base, _server) = serve(Some(401), |_| vec![]).await;
    let error = speech(&base).open().await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::Auth);
    assert!(
        error.message.contains("speechmatics") && error.message.contains("Settings"),
        "{}",
        error.message
    );
    assert_no_key(&error);

    let (base, server) = serve(None, |_| {
        vec![Reply::Json(
            json!({"message": "Error", "type": "not_authorised", "reason": "bad key"}),
        )]
    })
    .await;
    let error = speech(&base).open().await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::Auth);
    assert_no_key(&error);
    // A refused session closes its socket, so it does not count against the quota.
    assert_eq!(server.await.unwrap().got.last(), Some(&Got::Close));

    let (base, _server) = serve(None, |_| vec![Reply::Close(4001)]).await;
    let error = speech(&base).open().await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::Auth);
    assert_no_key(&error);
}

/// AC-16 (speech, R-19): a connection that drops mid-dictation is a network error, reported at
/// the next send or at finish.
#[tokio::test]
async fn dropped_connection_is_a_network_error() {
    let (base, _server) = serve(
        None,
        started_then(|got| {
            if matches!(got, Got::Binary(_)) {
                vec![Reply::Drop]
            } else {
                vec![]
            }
        }),
    )
    .await;
    let mut session = speech(&base).open().await.unwrap();
    session.send_audio(&[0; 480]).await.unwrap();
    // Give the reader time to see the connection go.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let error = match session.send_audio(&[0; 480]).await {
        Err(error) => error,
        Ok(()) => session.finish().await.unwrap_err(),
    };
    assert_eq!(error.kind, ErrorKind::Network, "{}", error.message);
    assert!(error.message.contains("speechmatics"), "{}", error.message);
    assert_no_key(&error);
}

/// AC-16: an error the provider reports mid-session ends it with that reason.
#[tokio::test]
async fn provider_error_mid_session_is_reported() {
    let (base, server) = serve(
        None,
        started_then(|got| match got {
            Got::Binary(_) => {
                vec![Reply::Json(json!({"message": "Error", "type": "quota_exceeded", "reason": "too many sessions"}))]
            }
            _ => vec![],
        }),
    )
    .await;
    let mut session = speech(&base).open().await.unwrap();
    session.send_audio(&[0; 480]).await.unwrap();
    let error = session.finish().await.unwrap_err();
    assert!(
        error.message.contains("quota_exceeded") && error.message.contains("too many sessions"),
        "{}",
        error.message
    );
    assert_eq!(server.await.unwrap().got.last(), Some(&Got::Close));
}

#[test]
fn debug_output_hides_the_key() {
    assert!(!format!("{:?}", speech("ws://127.0.0.1:1/v2")).contains(KEY));
}

// The state machine with the real Speechmatics client and fakes for the rest.

struct Recorder(Arc<Mutex<Vec<String>>>);

impl Recorder {
    fn push(&self, what: String) {
        self.0.lock().unwrap().push(what);
    }
}

struct NoMic;
impl Microphone for NoMic {
    fn start(&mut self) -> Result<(), Error> {
        Ok(())
    }
    fn stop(&mut self) {}
}

struct EchoCleanup;
impl CleanupProvider for EchoCleanup {
    async fn warm(&self) -> Result<(), Error> {
        Ok(())
    }
    async fn clean(
        &self,
        raw: &str,
        _app: Option<&str>,
        on_text: &mut (dyn FnMut(&str) + Send),
    ) -> Result<(), Error> {
        on_text(&format!("[clean] {raw}"));
        Ok(())
    }
}

struct Paste(Recorder);
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

struct NoFocus;
impl FocusInfo for NoFocus {
    fn frontmost_app(&self) -> Option<String> {
        None
    }
    fn secure_input(&self) -> bool {
        false
    }
}

/// AC-6 end to end: with the real client, the paste comes after the server's final transcript.
#[tokio::test]
async fn engine_pastes_the_final_transcript_from_speechmatics() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let server_log = log.clone();
    let (base, _server) = serve(
        None,
        started_then(move |got| {
            if got.is("ForceEndOfUtterance") {
                server_log
                    .lock()
                    .unwrap()
                    .push("final transcript sent".to_owned());
                forced(vec![
                    word("Please", "en"),
                    word("send", "en"),
                    word("the", "en"),
                    word("report", "en"),
                    mark("."),
                ])
            } else {
                vec![]
            }
        }),
    )
    .await;
    let (events, events_rx) = tokio::sync::mpsc::channel(64);
    let (status_tx, mut status) = tokio::sync::mpsc::unbounded_channel();
    let mut settings = Settings::from_config(&config::Config::default());
    settings.min_hold = std::time::Duration::ZERO;
    let parts = Parts {
        mic: NoMic,
        speech: speech(&base),
        cleanup: EchoCleanup,
        inserter: Paste(Recorder(log.clone())),
        focus: NoFocus,
        history: NoHistory,
        sounds: Silent,
        timing: NoTiming,
    };
    let engine = Engine::new(parts, settings, status_tx);
    tokio::spawn(engine.run(events_rx));

    events.send(Event::KeyDown).await.unwrap();
    events.send(Event::Audio(vec![3; 480])).await.unwrap();
    events.send(Event::KeyUp).await.unwrap();
    loop {
        match status.recv().await.unwrap() {
            Status::Done { raw } => {
                assert!(!raw);
                break;
            }
            Status::Failed { message, .. } => panic!("{message}"),
            _ => {}
        }
    }
    assert_eq!(
        *log.lock().unwrap(),
        [
            "final transcript sent",
            "paste: [clean] Please send the report."
        ]
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
}

struct NoTiming;
impl TimingLog for NoTiming {
    fn record(&mut self, _line: &TimingLine) {}
}

struct Silent;
impl Sounds for Silent {
    fn play(&self, _sound: Sound) {}
}
