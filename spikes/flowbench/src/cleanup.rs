//! Cleanup of the raw transcript by a language model: streamed, with a deadline.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tokio::time::{Instant, timeout};

use crate::http::ensure_ok;

/// The starting prompt from `docs/SPEED_TEST.md`.
pub const PROMPT: &str = "You clean up dictated text. Remove filler words and false starts. \
Fix punctuation, capitalisation and obvious mis-hearings. Keep the speaker's wording and \
language. Format a list when the speaker enumerates. Output only the cleaned text.";

/// A language model that rewrites a raw transcript into clean text.
pub trait CleanupProvider {
    /// Opens the connection ahead of time, on key-down. Fails fast on a bad key or URL.
    async fn warm(&self) -> Result<()>;

    /// Streams the cleaned text, calling `on_text` for each piece as it arrives.
    async fn clean(&self, raw: &str, on_text: &mut dyn FnMut(&str)) -> Result<()>;
}

/// The text the user gets, and when it arrived.
pub struct Cleaned {
    /// The cleaned text, or the raw transcript when `fallback` is set.
    pub text: String,
    /// When the first token of the cleaned text arrived. `None` on a fallback.
    pub first: Option<Instant>,
    /// When the cleaned text was complete, or when the deadline passed.
    pub done: Instant,
    /// The deadline passed, so the raw transcript is used.
    pub fallback: bool,
    /// On a fallback: what happened.
    pub note: Option<String>,
}

/// Runs the cleanup with a deadline. When the deadline passes, the raw transcript is returned
/// and the sample counts as a fallback.
///
/// A cleanup that fails, or answers with nothing, is an error and not a fallback: it ends
/// quickly, and measuring it would make a broken run look fast.
pub async fn clean_with_deadline<C: CleanupProvider>(
    llm: &C,
    raw: &str,
    deadline: Duration,
) -> Result<Cleaned> {
    let start = Instant::now();
    let mut text = String::new();
    let mut first = None;
    let outcome = {
        let mut on_text = |piece: &str| {
            first.get_or_insert_with(Instant::now);
            text.push_str(piece);
        };
        timeout(deadline, llm.clean(raw, &mut on_text)).await
    };
    let done = Instant::now();
    let Ok(finished) = outcome else {
        let progress = match first {
            Some(at) => format!("first token after {} ms", (at - start).as_millis()),
            None => "no token arrived".to_owned(),
        };
        return Ok(Cleaned {
            text: raw.to_owned(),
            first: None,
            done,
            fallback: true,
            note: Some(format!(
                "cleanup not complete within {} ms ({progress})",
                deadline.as_millis()
            )),
        });
    };
    finished.context("cleanup failed")?;
    let text = text.trim().to_owned();
    if text.is_empty() {
        bail!("cleanup returned no text");
    }
    Ok(Cleaned {
        text,
        first,
        done,
        fallback: false,
        note: None,
    })
}

/// Any host with an OpenAI-compatible `/chat/completions` endpoint: OpenAI, Groq, Cerebras.
pub struct ChatCleanup {
    pub http: reqwest::Client,
    /// For example `https://api.openai.com/v1`.
    pub base_url: String,
    pub key: String,
    /// Name of the variable the key came from, for error messages.
    pub key_env: String,
    pub model: String,
    /// `None` leaves the field out of the request.
    pub reasoning_effort: Option<String>,
}

impl CleanupProvider for ChatCleanup {
    async fn warm(&self) -> Result<()> {
        let resp = self
            .http
            .get(format!("{}/models", self.base_url))
            .bearer_auth(&self.key)
            .send()
            .await
            .with_context(|| format!("connecting to {}", self.base_url))?;
        let resp = ensure_ok(resp, &self.key_env).await?;
        // Read the body so the connection goes back to the pool, ready for the cleanup call.
        resp.bytes().await.context("reading warm-up response")?;
        Ok(())
    }

    async fn clean(&self, raw: &str, on_text: &mut dyn FnMut(&str)) -> Result<()> {
        let resp = self
            .http
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.key)
            .json(&request_body(
                &self.model,
                self.reasoning_effort.as_deref(),
                raw,
            ))
            .send()
            .await
            .context("sending cleanup request")?;
        let mut resp = ensure_ok(resp, &self.key_env).await?;
        let mut lines = SseLines::default();
        while let Some(bytes) = resp.chunk().await.context("reading cleanup stream")? {
            for data in lines.push(&bytes) {
                if data == "[DONE]" {
                    return Ok(());
                }
                if data.is_empty() {
                    continue;
                }
                if let Some(text) = delta_text(&data)? {
                    on_text(&text);
                }
            }
        }
        Ok(())
    }
}

/// The `system` role and no token cap keep the request valid on every compatible host.
fn request_body(model: &str, reasoning_effort: Option<&str>, raw: &str) -> Value {
    let mut body = json!({
        "model": model,
        "stream": true,
        "messages": [
            {"role": "system", "content": PROMPT},
            {"role": "user", "content": raw},
        ],
    });
    if let Some(effort) = reasoning_effort {
        body["reasoning_effort"] = json!(effort);
    }
    body
}

/// Splits a server-sent event stream into the payloads of its `data:` lines.
#[derive(Default)]
struct SseLines {
    buf: Vec<u8>,
}

impl SseLines {
    /// Adds received bytes and returns the payload of every line completed by them.
    fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(bytes);
        let mut payloads = Vec::new();
        while let Some(end) = self.buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=end).collect();
            let line = String::from_utf8_lossy(&line);
            if let Some(data) = line.trim_end().strip_prefix("data:") {
                payloads.push(data.trim_start().to_owned());
            }
        }
        payloads
    }
}

/// The text in one stream chunk, if it carries any.
fn delta_text(data: &str) -> Result<Option<String>> {
    let chunk: Value =
        serde_json::from_str(data).with_context(|| format!("unexpected stream line: {data}"))?;
    if let Some(error) = chunk.get("error") {
        bail!("provider error in the stream: {error}");
    }
    Ok(chunk["choices"][0]["delta"]["content"]
        .as_str()
        .filter(|text| !text.is_empty())
        .map(str::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeCleanup;
    use crate::testserver;

    #[test]
    fn request_matches_the_plan() {
        let body = request_body("some-model", Some("none"), "um hello");
        assert_eq!(body["model"], "some-model");
        assert_eq!(body["stream"], true);
        assert_eq!(body["reasoning_effort"], "none");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], PROMPT);
        assert_eq!(body["messages"][1]["content"], "um hello");
    }

    #[test]
    fn reasoning_effort_off_omits_the_field() {
        let body = request_body("some-model", None, "hello");
        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn sse_lines_survive_any_split() {
        let stream = "data: {\"a\":1}\n\ndata: {\"b\":\"ñ\"}\r\n\n: comment\ndata: [DONE]\n\n";
        for size in [1, 2, 3, 7, stream.len()] {
            let mut lines = SseLines::default();
            let mut payloads = Vec::new();
            for part in stream.as_bytes().chunks(size) {
                payloads.extend(lines.push(part));
            }
            assert_eq!(payloads, ["{\"a\":1}", "{\"b\":\"ñ\"}", "[DONE]"], "{size}");
        }
    }

    #[test]
    fn delta_text_reads_content_and_skips_the_rest() {
        let content = r#"{"object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hola"},"finish_reason":null}]}"#;
        assert_eq!(delta_text(content).unwrap().as_deref(), Some("Hola"));
        let role_only = r#"{"choices":[{"index":0,"delta":{"role":"assistant","content":""}}]}"#;
        assert_eq!(delta_text(role_only).unwrap(), None);
        let usage_only = r#"{"choices":[],"usage":{"total_tokens":12}}"#;
        assert_eq!(delta_text(usage_only).unwrap(), None);
    }

    #[test]
    fn an_error_in_the_stream_is_reported() {
        let err = delta_text(r#"{"error":{"message":"overloaded"}}"#).unwrap_err();
        assert!(err.to_string().contains("overloaded"));
    }

    #[tokio::test(start_paused = true)]
    async fn cleanup_within_the_deadline_is_used() {
        let start = Instant::now();
        let cleaned =
            clean_with_deadline(&FakeCleanup, "um hello there", Duration::from_millis(1500))
                .await
                .unwrap();
        assert!(!cleaned.fallback);
        assert_eq!(cleaned.text, "Hello there.");
        let first = cleaned.first.unwrap() - start;
        assert_eq!(first, FakeCleanup::first_token_delay("um hello there"));
        assert!(cleaned.done >= cleaned.first.unwrap());
    }

    #[tokio::test(start_paused = true)]
    async fn a_slow_cleanup_falls_back_to_the_raw_transcript_at_the_deadline() {
        let start = Instant::now();
        let raw = "this one misses the deadline";
        let cleaned = clean_with_deadline(&FakeCleanup, raw, Duration::from_millis(1500))
            .await
            .unwrap();
        assert!(cleaned.fallback);
        assert_eq!(cleaned.text, raw);
        assert_eq!(cleaned.done - start, Duration::from_millis(1500));
        assert_eq!(
            cleaned.note.unwrap(),
            "cleanup not complete within 1500 ms (no token arrived)"
        );
    }

    /// Answers at once with one token, then never finishes.
    struct StallsAfterFirstToken;

    impl CleanupProvider for StallsAfterFirstToken {
        async fn warm(&self) -> Result<()> {
            Ok(())
        }

        async fn clean(&self, _raw: &str, on_text: &mut dyn FnMut(&str)) -> Result<()> {
            tokio::time::sleep(Duration::from_millis(200)).await;
            on_text("Hola");
            tokio::time::sleep(Duration::from_secs(60)).await;
            Ok(())
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_fallback_has_no_first_token_time_even_if_a_token_arrived() {
        let raw = "eh hola equipo";
        let cleaned = clean_with_deadline(&StallsAfterFirstToken, raw, Duration::from_millis(900))
            .await
            .unwrap();
        // The text shown is the raw transcript, so no cleaned token was used.
        assert!(cleaned.fallback);
        assert_eq!(cleaned.text, raw);
        assert_eq!(cleaned.first, None);
        assert_eq!(
            cleaned.note.unwrap(),
            "cleanup not complete within 900 ms (first token after 200 ms)"
        );
    }

    /// Fails at once, the way a rejected request does.
    struct Rejects;

    impl CleanupProvider for Rejects {
        async fn warm(&self) -> Result<()> {
            Ok(())
        }

        async fn clean(&self, _raw: &str, _on_text: &mut dyn FnMut(&str)) -> Result<()> {
            bail!("HTTP 404: the model does not exist")
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_or_empty_cleanup_is_an_error_not_a_fast_fallback() {
        let deadline = Duration::from_millis(1500);
        let failed = clean_with_deadline(&Rejects, "hello", deadline).await;
        let message = format!("{:#}", failed.err().expect("an error"));
        assert_eq!(
            message,
            "cleanup failed: HTTP 404: the model does not exist"
        );

        struct SaysNothing;
        impl CleanupProvider for SaysNothing {
            async fn warm(&self) -> Result<()> {
                Ok(())
            }
            async fn clean(&self, _raw: &str, on_text: &mut dyn FnMut(&str)) -> Result<()> {
                on_text("  \n");
                Ok(())
            }
        }
        let empty = clean_with_deadline(&SaysNothing, "hello", deadline).await;
        assert!(
            empty
                .err()
                .expect("an error")
                .to_string()
                .contains("no text")
        );
    }

    fn chat(base_url: String) -> ChatCleanup {
        ChatCleanup {
            http: reqwest::Client::new(),
            base_url,
            key: "test-key".to_owned(),
            key_env: "TEST_KEY_ENV".to_owned(),
            model: "some-model".to_owned(),
            reasoning_effort: Some("none".to_owned()),
        }
    }

    #[tokio::test]
    async fn warm_and_clean_talk_to_a_chat_server() {
        let stream = concat!(
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hola\"}}]}\n\n",
            "data:\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\", equipo.\"}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n",
        );
        let (base_url, server) = testserver::serve(vec![
            testserver::ok("application/json", "{\"data\":[]}"),
            testserver::ok("text/event-stream", stream),
        ])
        .await;
        let llm = chat(base_url);

        llm.warm().await.unwrap();
        let mut pieces = Vec::new();
        llm.clean("eh hola equipo", &mut |piece| pieces.push(piece.to_owned()))
            .await
            .unwrap();
        assert_eq!(pieces, ["Hola", ", equipo."]);

        let requests = server.await.unwrap();
        assert!(requests[0].starts_with("GET /v1/models HTTP/1.1\r\n"));
        assert!(
            requests[0]
                .to_lowercase()
                .contains("authorization: bearer test-key\r\n")
        );
        let (head, body) = requests[1].split_once("\r\n\r\n").unwrap();
        assert!(head.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"));
        assert!(
            head.to_lowercase()
                .contains("authorization: bearer test-key\r\n")
        );
        let body: Value = serde_json::from_str(body).unwrap();
        assert_eq!(
            body,
            request_body("some-model", Some("none"), "eh hola equipo")
        );
    }

    #[tokio::test]
    async fn a_rejected_key_names_the_variable_and_hides_the_body() {
        let body = "{\"error\":{\"message\":\"Incorrect API key provided: sk-abc***xyz\"}}";
        let (base_url, _server) = testserver::serve(vec![testserver::response(
            "401 Unauthorized",
            "application/json",
            body,
        )])
        .await;
        let error = format!("{:#}", chat(base_url).warm().await.unwrap_err());
        assert!(error.contains("401 Unauthorized"), "{error}");
        assert!(error.contains("$TEST_KEY_ENV"), "{error}");
        assert!(!error.contains("sk-abc"), "{error}");
    }

    #[tokio::test]
    async fn another_error_shows_the_provider_message() {
        let body = "{\"error\":{\"message\":\"The model `nope` does not exist\"}}";
        let (base_url, _server) = testserver::serve(vec![testserver::response(
            "404 Not Found",
            "application/json",
            body,
        )])
        .await;
        let outcome =
            clean_with_deadline(&chat(base_url), "hello", Duration::from_millis(1500)).await;
        let error = format!("{:#}", outcome.err().expect("an error"));
        assert!(
            error.contains("404") && error.contains("does not exist"),
            "{error}"
        );
    }
}
