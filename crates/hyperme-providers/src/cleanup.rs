//! Cleanup of the raw transcript by a language model on any OpenAI-compatible chat completions
//! API, Groq first. Ported from `spikes/flowbench`.
//!
//! Checked on 26 November 2026 against Groq's docs (console.groq.com/docs/reasoning, the API
//! reference and the `openai/gpt-oss-20b` model page): base URL `.../openai/v1`, `GET /models`
//! and `POST /chat/completions` with `Authorization: Bearer`, `stream: true` as server-sent
//! events ending in `data: [DONE]`, text in `choices[0].delta.content`, and `reasoning_effort`
//! `low`, `medium` or `high` for gpt-oss. Not stated there and kept from flowbench's live runs:
//! the `system` role, streamed reasoning in `delta.reasoning`, and the 401 for a bad key.
//!
//! The deadline and the fallback to the raw transcript (R-12, R-20) are the state machine's,
//! in `hyperme_core::dictation`, so they hold for every provider.

use std::fmt;
use std::time::Duration;

use hyperme_core::config;
use hyperme_core::traits::{CleanupProvider, Error, ErrorKind};
use reqwest::StatusCode;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue};
use serde_json::{Value, json};

/// The third prompt from flowbench: change as little as possible.
const PROMPT_V3: &str = "You clean up dictated text in Spanish or English. Change as \
little as possible. Remove only hesitations (um, uh, eh, em, mm), stutters and false starts; \
when the speaker corrects themselves, keep only the correction. Fix punctuation, capitals and \
accents, and words that were clearly misheard. Keep every other word exactly as spoken, \
including informal words, slang and swearing, in the same order and the same language. \
Never translate, rephrase, summarise or answer. Output only the cleaned text.";

/// The one rule `v4` adds to `v3` (flowbench issue 34).
const V4_RULE: &str = "When a word or short phrase clearly sounds like a term in the speaker's \
dictionary, write the dictionary term. Otherwise change nothing.";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// The prompt for a version named in the config.
pub fn prompt_named(version: &str) -> Option<String> {
    match version {
        "v3" => Some(PROMPT_V3.to_owned()),
        "v4" => Some(format!("{PROMPT_V3} {V4_RULE}")),
        _ => None,
    }
}

/// Any host with an OpenAI-compatible `/chat/completions` endpoint.
pub struct ChatCleanup {
    http: reqwest::Client,
    /// The provider name from the config, used in messages (R-21).
    provider: String,
    /// Base URL, without a trailing slash.
    endpoint: String,
    model: String,
    /// `None` leaves `reasoning_effort` out of the request.
    reasoning_effort: Option<String>,
    prompt: String,
    /// The `Authorization` header, marked sensitive.
    auth: HeaderValue,
}

impl fmt::Debug for ChatCleanup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChatCleanup")
            .field("endpoint", &self.endpoint)
            .field("model", &self.model)
            .finish_non_exhaustive()
    }
}

impl ChatCleanup {
    /// `key` comes from the keychain. Fails on a prompt version this build does not have.
    pub fn new(config: &config::Cleanup, key: &str) -> Result<Self, Error> {
        let provider = config.provider.clone();
        let prompt = prompt_named(&config.prompt).ok_or_else(|| {
            provider_error(
                &provider,
                ErrorKind::Other,
                format!(
                    "cleanup.prompt {:?} is not a known prompt version (v3, v4)",
                    config.prompt
                ),
            )
        })?;
        let mut auth = HeaderValue::from_str(&format!("Bearer {key}")).map_err(|_| {
            provider_error(
                &provider,
                ErrorKind::Auth,
                "the key has characters a header cannot carry. Check it in Settings",
            )
        })?;
        auth.set_sensitive(true);
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .map_err(|e| {
                provider_error(
                    &provider,
                    ErrorKind::Other,
                    format!("could not set up HTTP: {e}"),
                )
            })?;
        Ok(ChatCleanup {
            http,
            provider,
            endpoint: config.endpoint.trim_end_matches('/').to_owned(),
            model: config.model.clone(),
            reasoning_effort: (config.reasoning_effort != "off")
                .then(|| config.reasoning_effort.clone()),
            prompt,
            auth,
        })
    }

    fn error(&self, kind: ErrorKind, what: impl fmt::Display) -> Error {
        provider_error(&self.provider, kind, what)
    }

    fn network(&self, error: &reqwest::Error) -> Error {
        // reqwest's message names the URL, which holds no key.
        self.error(
            ErrorKind::Network,
            format!("could not reach the cleanup host: {error}"),
        )
    }

    /// Passes a successful response through; turns any other into an error that never shows
    /// the key (R-21, AC-16).
    async fn ensure_ok(&self, response: reqwest::Response) -> Result<reqwest::Response, Error> {
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            // The body of a 401 can echo part of the key, so it is not shown.
            return Err(self.error(
                ErrorKind::Auth,
                "the key was rejected. Check it in Settings",
            ));
        }
        let body = response.text().await.unwrap_or_default();
        let body: String = body.chars().take(300).collect();
        Err(self.error(ErrorKind::Other, format!("HTTP {status}: {}", body.trim())))
    }

    /// Adds the user's dictionary to the prompt (spec 93, R-4): `line` is
    /// `Dictionary::prompt_line()`. It goes after the fixed prompt and before the app line, so
    /// the prompt's start stays the same from one dictation to the next.
    pub fn with_dictionary(mut self, line: &str) -> Self {
        self.prompt.push_str(line);
        self
    }

    /// The request body. The fixed prompt comes first so a provider's prompt cache keeps the
    /// first token fast; the app name (R-11) is its last line.
    fn request_body(&self, raw: &str, app: Option<&str>) -> Value {
        let mut system = self.prompt.clone();
        if let Some(app) = app.filter(|a| !a.trim().is_empty()) {
            system.push_str(&format!("\n\nThe text will be pasted into {app}."));
        }
        let mut body = json!({
            "model": self.model,
            "stream": true,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": raw},
            ],
        });
        if let Some(effort) = &self.reasoning_effort {
            body["reasoning_effort"] = json!(effort);
        }
        body
    }
}

fn provider_error(provider: &str, kind: ErrorKind, what: impl fmt::Display) -> Error {
    Error {
        kind,
        message: format!("{provider}: {what}"),
    }
}

impl CleanupProvider for ChatCleanup {
    /// R-2: opens the connection on key-down, so the cleanup call finds it ready.
    async fn warm(&self) -> Result<(), Error> {
        let response = self
            .http
            .get(format!("{}/models", self.endpoint))
            .header(AUTHORIZATION, self.auth.clone())
            .send()
            .await
            .map_err(|e| self.network(&e))?;
        let response = self.ensure_ok(response).await?;
        // Read the body so the connection goes back to the pool, ready for the cleanup call.
        response.bytes().await.map_err(|e| self.network(&e))?;
        Ok(())
    }

    async fn clean(
        &self,
        raw: &str,
        app: Option<&str>,
        on_text: &mut (dyn FnMut(&str) + Send),
    ) -> Result<(), Error> {
        let response = self
            .http
            .post(format!("{}/chat/completions", self.endpoint))
            .header(AUTHORIZATION, self.auth.clone())
            .header(CONTENT_TYPE, "application/json")
            .body(self.request_body(raw, app).to_string())
            .send()
            .await
            .map_err(|e| self.network(&e))?;
        let mut response = self.ensure_ok(response).await?;
        let mut lines = SseLines::default();
        while let Some(bytes) = response.chunk().await.map_err(|e| self.network(&e))? {
            for data in lines.push(&bytes) {
                if data == "[DONE]" {
                    return Ok(());
                }
                if data.is_empty() {
                    continue;
                }
                if let Some(text) =
                    chunk_text(&data).map_err(|what| self.error(ErrorKind::Other, what))?
                {
                    on_text(&text);
                }
            }
        }
        Ok(())
    }
}

/// Splits a server-sent event stream into the payloads of its `data:` lines.
#[derive(Default)]
struct SseLines {
    buf: Vec<u8>,
}

impl SseLines {
    /// Adds received bytes and returns the payload of every line they complete.
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

/// The cleaned text one stream chunk carries, if any. Reasoning (`delta.reasoning`) is skipped.
fn chunk_text(data: &str) -> Result<Option<String>, String> {
    let chunk: Value = serde_json::from_str(data)
        .map_err(|_| "the stream sent a line that is not JSON".to_owned())?;
    if let Some(error) = chunk.get("error") {
        let message = error["message"]
            .as_str()
            .map_or_else(|| error.to_string(), str::to_owned);
        return Err(format!("the stream reported an error: {message}"));
    }
    Ok(chunk["choices"][0]["delta"]["content"]
        .as_str()
        .filter(|t| !t.is_empty())
        .map(str::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cleanup(effort: &str, prompt: &str) -> Result<ChatCleanup, Error> {
        let mut config = config::Config::default().cleanup;
        config.provider = "groq".to_owned();
        config.model = "test-model".to_owned();
        config.endpoint = "http://127.0.0.1:1/v1/".to_owned();
        config.reasoning_effort = effort.to_owned();
        config.prompt = prompt.to_owned();
        ChatCleanup::new(&config, "not-a-real-key")
    }

    #[test]
    fn v4_is_v3_with_the_dictionary_rule() {
        let v4 = prompt_named("v4").unwrap();
        assert!(v4.starts_with(PROMPT_V3));
        assert!(v4.ends_with(V4_RULE));
        assert!(prompt_named("v9").is_none());
        let error = cleanup("low", "v9").unwrap_err();
        assert!(error.message.contains("v9"), "{}", error.message);
    }

    /// R-11: the request carries the prompt, the transcript and the app name, with the reasoning
    /// effort from the config; `off` leaves it out.
    #[test]
    fn request_body_has_prompt_transcript_app_and_effort() {
        let body = cleanup("low", "v4")
            .unwrap()
            .request_body("hola equipo", Some("Slack"));
        assert_eq!(body["model"], "test-model");
        assert_eq!(body["stream"], true);
        assert_eq!(body["reasoning_effort"], "low");
        let system = body["messages"][0]["content"].as_str().unwrap();
        assert!(system.starts_with(PROMPT_V3));
        assert!(
            system.ends_with("The text will be pasted into Slack."),
            "{system}"
        );
        assert_eq!(
            body["messages"][1],
            json!({"role": "user", "content": "hola equipo"})
        );

        let off = cleanup("off", "v4").unwrap().request_body("hi", None);
        assert!(off.get("reasoning_effort").is_none());
        assert_eq!(off["messages"][0]["content"], prompt_named("v4").unwrap());
    }

    /// Spec 93, AC-4: the dictionary line sits between the fixed prompt and the app line, so
    /// two apps share the same prompt start.
    #[test]
    fn the_dictionary_goes_before_the_app_line() {
        let line = "\n\nThe speaker's dictionary: LangChain, Zentrix.";
        let with = cleanup("low", "v4").unwrap().with_dictionary(line);
        let system = |app| {
            with.request_body("hola", Some(app))["messages"][0]["content"]
                .as_str()
                .unwrap()
                .to_owned()
        };
        let start = format!("{}{line}", prompt_named("v4").unwrap());
        assert_eq!(
            system("Slack"),
            format!("{start}\n\nThe text will be pasted into Slack.")
        );
        assert!(system("Mail").starts_with(&start));
        // An empty dictionary leaves the prompt as it was.
        let none = cleanup("low", "v4").unwrap().with_dictionary("");
        assert_eq!(
            none.request_body("hi", None)["messages"][0]["content"],
            prompt_named("v4").unwrap()
        );
    }

    #[test]
    fn stream_lines_are_split_across_chunks() {
        let mut lines = SseLines::default();
        assert!(lines.push(b"data: {\"a\"").is_empty());
        assert_eq!(
            lines.push(b":1}\n\ndata: [DONE]\n"),
            ["{\"a\":1}", "[DONE]"]
        );
    }

    #[test]
    fn chunks_give_text_and_skip_reasoning() {
        let text = r#"{"choices":[{"index":0,"delta":{"content":"Hola"}}]}"#;
        assert_eq!(chunk_text(text).unwrap().as_deref(), Some("Hola"));
        let thinking = r#"{"choices":[{"index":0,"delta":{"reasoning":"The user"}}]}"#;
        assert_eq!(chunk_text(thinking).unwrap(), None);
        let error = r#"{"error":{"message":"overloaded"}}"#;
        assert!(chunk_text(error).unwrap_err().contains("overloaded"));
    }
}
