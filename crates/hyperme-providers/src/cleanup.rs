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

use hyperme_core::config::{self, CleanupLevel};
use hyperme_core::tone::AppStyle;
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

/// The formatting rule `v5` adds to `v4`: lists and paragraphs, as the owner asked on 27
/// November 2026 ("I don't see bullet points or paragraphs"). Shaped after Wispr Flow's smart
/// formatting (docs.wisprflow.ai, "How do I use Smart Formatting & Backtrack", read that day):
/// spoken sequence words become a list and are dropped, topic shifts become paragraphs. Plain
/// `- ` and `1.` markers read well in every app, rendered or not, so they are the same
/// everywhere; the app line can still forbid formatting (code editors, terminals).
const FORMAT_RULE: &str = "Format lists and paragraphs without changing words. Three or \
more items or steps: a lead-in ending in a colon, then one item per line, \"1.\" when the \
speaker numbered them (first, second; uno, dos), else \"- \"; drop the numbering words; each \
item starts with a capital, and all end alike. A topic change in a long text starts a new \
paragraph. \"New line\", \"new paragraph\", \"nueva línea\" or \"punto y aparte\" said as a \
command is that break. No headings or bold.";

/// Words that suggest a list or a dictated break, in English and Spanish.
const FORMAT_CUES: &[&str] = &[
    "first",
    "firstly",
    "second",
    "secondly",
    "third",
    "finally",
    "primero",
    "segundo",
    "tercero",
    "primera",
    "segunda",
    "tercera",
    "luego",
    "line",
    "paragraph",
    "línea",
    "aparte",
];

/// Whether a transcript may need the formatting rule: long enough to change topic, a colon or
/// three commas (a spoken enumeration), or a list or break word. The rule costs time to the
/// first token (about 140 ms on OpenAI, 27 November 2026), so a short plain dictation goes
/// without it and gets `v4`'s prompt.
pub fn needs_formatting(raw: &str) -> bool {
    let words: Vec<String> = raw
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    words.len() >= FORMAT_WORDS
        || raw.contains(':')
        || raw.matches(',').count() >= 3
        || words.iter().any(|w| FORMAT_CUES.contains(&w.as_str()))
}

/// From this many words a dictation may change topic, so it gets the formatting rule.
const FORMAT_WORDS: usize = 40;

/// The first `medium` prompt (P2-2): light's rules, plus repetitions, small grammar slips and
/// spoken formatting. Still never rephrased, translated or answered.
const PROMPT_M1: &str = "You clean up dictated text in Spanish or English. Remove \
hesitations (um, uh, eh, em, mm), filler phrases used only as hesitation, stutters, repeated \
words and false starts; when the speaker corrects themselves, keep only the correction. Fix \
punctuation, capitals and accents, small grammar slips such as a wrong agreement or a missing \
article, and words that were clearly misheard. Apply spoken formatting: write numbers, dates, \
times and amounts as digits where that is usual, turn an enumeration the speaker dictates as a \
list into a list, and turn \"new line\", \"new paragraph\", \"nueva línea\" or \"punto y aparte\" \
into line breaks. Keep every other word as spoken, including informal words, slang and \
swearing, in the same order and the same language. Never translate, rephrase, summarise or \
answer, even when the text is a question or a request. Output only the cleaned text.";

/// The second `medium` prompt: `m1` measured worse than light on the owner's samples, because it
/// reordered words, dropped spoken words such as "like" or "pues eso", wrote small numbers as
/// digits and invented line breaks. `m2` starts from `v3` and adds only what helped: repeated
/// words, commas, and formatting the speaker asks for.
const PROMPT_M2: &str = "You clean up dictated text in Spanish or English. Change as \
little as possible. Remove hesitations (um, uh, eh, em, mm), stutters, a word or phrase said \
twice in a row by mistake, and false starts; when the speaker corrects themselves, keep only \
the correction. Fix punctuation, including the commas a careful writer would add, capitals and \
accents, small grammar slips such as a wrong agreement, and words that were clearly misheard. \
Format only what the speaker clearly dictates as formatting: \"new line\" or \"nueva línea\" \
said as a command becomes a line break, and an enumeration dictated as a list becomes a list. \
Keep numbers as they were said. Keep every other word exactly as spoken, including informal \
words, filler words such as like or pues, slang and swearing, in the same order and the same \
language; never reorder words. Never translate, rephrase, summarise or answer. Output only \
the cleaned text.";

/// `m2` with a firmer dictionary rule: a misheard word that sounds like a dictionary term is
/// the term, even when the transcript shows a real word.
const M3_RULE: &str = "The speaker often says the terms in their dictionary. When a word or \
short phrase sounds like one of them, even if the transcript shows a different real word, \
write the dictionary term. Otherwise change nothing.";

/// The prompt version each level uses (P2-2). `none` makes no call, so it has none.
const LIGHT: &str = "v5";
const MEDIUM: &str = "m2";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Settings waits at most this long for a model list.
const LIST_TIMEOUT: Duration = Duration::from_secs(5);

/// The prompt for a version named in the config.
pub fn prompt_named(version: &str) -> Option<String> {
    match version {
        "v3" => Some(PROMPT_V3.to_owned()),
        "v4" => Some(format!("{PROMPT_V3} {V4_RULE}")),
        "v5" => Some(format!("{PROMPT_V3} {FORMAT_RULE} {V4_RULE}")),
        "m1" => Some(format!("{PROMPT_M1} {V4_RULE}")),
        "m2" => Some(format!("{PROMPT_M2} {V4_RULE}")),
        "m3" => Some(format!("{PROMPT_M2} {M3_RULE}")),
        _ => None,
    }
}

/// The prompt version a config uses: its `prompt` override, else its level's (P2-2).
pub fn prompt_version(config: &config::Cleanup) -> &str {
    match (&config.prompt, config.level) {
        (Some(prompt), _) => prompt,
        (None, CleanupLevel::Medium) => MEDIUM,
        // `none` never calls the model; a provider built for it anyway gets light's prompt.
        (None, CleanupLevel::Light | CleanupLevel::None) => LIGHT,
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
    /// The prompt without the formatting rule, for a dictation that cannot need it
    /// ([`needs_formatting`]). `None` when the prompt has no such rule.
    plain: Option<String>,
    /// The user's dictionary line (spec 93, R-4), between the prompt and the app line. Behind a
    /// lock so a saved dictionary reaches a provider the engine shares (specs/96, R-8).
    dictionary: std::sync::Mutex<String>,
    /// The `Authorization` header, marked sensitive. `None` for a server that needs no key, such
    /// as one on this Mac (R-14), so no header is sent at all.
    auth: Option<HeaderValue>,
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
        let version = prompt_version(config);
        let prompt = prompt_named(version).ok_or_else(|| {
            provider_error(
                &provider,
                ErrorKind::Other,
                format!(
                    "cleanup.prompt {version:?} is not a known prompt version (v3, v4, v5, m1, m2, m3)"
                ),
            )
        })?;
        let auth = if key.is_empty() {
            None
        } else {
            let mut auth = HeaderValue::from_str(&format!("Bearer {key}")).map_err(|_| {
                provider_error(
                    &provider,
                    ErrorKind::Auth,
                    "the key has characters a header cannot carry. Check it in Settings",
                )
            })?;
            auth.set_sensitive(true);
            Some(auth)
        };
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
            plain: prompt
                .contains(FORMAT_RULE)
                .then(|| prompt.replace(&format!(" {FORMAT_RULE}"), "")),
            prompt,
            dictionary: std::sync::Mutex::default(),
            auth,
        })
    }

    fn authorized(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.auth {
            Some(auth) => request.header(AUTHORIZATION, auth.clone()),
            None => request,
        }
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
        if model_not_found(status, &body) {
            return Err(self.error(
                ErrorKind::Other,
                format!(
                    "the model {} was not found. Pick another in Settings",
                    self.model
                ),
            ));
        }
        let body: String = body.chars().take(300).collect();
        Err(self.error(ErrorKind::Other, format!("HTTP {status}: {}", body.trim())))
    }

    /// Settings' Test (R-13): asks the chosen model to clean a one-word transcript through the
    /// same request a dictation sends (prompt, reasoning effort, streaming), so a wrong model
    /// fails Test, not the next dictation. A design note asked for a one-token completion; a
    /// token limit is left out because hosts disagree on its name (`max_tokens` against
    /// `max_completion_tokens`), and a one-word transcript costs about as little.
    pub async fn check(&self) -> Result<(), Error> {
        self.clean("Test.", None, &mut |_| {}).await
    }

    /// Adds the user's dictionary to the prompt (spec 93, R-4): `line` is
    /// `Dictionary::prompt_line()`. It goes after the fixed prompt and before the app line, so
    /// the prompt's start stays the same from one dictation to the next.
    pub fn with_dictionary(self, line: &str) -> Self {
        self.set_dictionary(line);
        self
    }

    /// The request body. The fixed prompt comes first so a provider's prompt cache keeps the
    /// first token fast; the app name (R-11) is its last line.
    fn request_body(&self, raw: &str, app: Option<&str>) -> Value {
        let dictionary = self
            .dictionary
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let prompt = match &self.plain {
            Some(plain) if !needs_formatting(raw) => plain,
            _ => &self.prompt,
        };
        let mut system = format!("{prompt}{dictionary}");
        // Issue 219: the app name and the style it calls for, last, so the start stays fixed.
        if let Some(app) = app.filter(|a| !a.trim().is_empty()) {
            system.push_str(&AppStyle::for_app(Some(app)).prompt_line(app));
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

/// The models a key offers: `GET {endpoint}/models`, the `id` of each entry in `data`
/// (OpenAI's shape, which Groq, OpenRouter and Google's compatibility endpoint share). Google's
/// ids start with `models/`, which is dropped. Called only from Settings, never on the dictation
/// path (R-15). An empty `key` sends no `Authorization` header, for a local server.
pub async fn list_models(provider: &str, endpoint: &str, key: &str) -> Result<Vec<String>, Error> {
    list_models_within(provider, endpoint, key, LIST_TIMEOUT).await
}

/// [`list_models`] with its own time limit, for looking on local ports (R-22).
pub(crate) async fn list_models_within(
    provider: &str,
    endpoint: &str,
    key: &str,
    limit: Duration,
) -> Result<Vec<String>, Error> {
    let http = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT.min(limit))
        .timeout(limit)
        .build()
        .map_err(|e| {
            provider_error(
                provider,
                ErrorKind::Other,
                format!("could not set up HTTP: {e}"),
            )
        })?;
    let mut request = http.get(format!("{}/models", endpoint.trim_end_matches('/')));
    if !key.is_empty() {
        let mut auth = HeaderValue::from_str(&format!("Bearer {key}")).map_err(|_| {
            provider_error(
                provider,
                ErrorKind::Auth,
                "the key has characters a header cannot carry",
            )
        })?;
        auth.set_sensitive(true);
        request = request.header(AUTHORIZATION, auth);
    }
    let response = request.send().await.map_err(|e| {
        provider_error(
            provider,
            ErrorKind::Network,
            format!("could not reach it: {e}"),
        )
    })?;
    let status = response.status();
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Err(provider_error(
            provider,
            ErrorKind::Auth,
            "the key was rejected",
        ));
    }
    if !status.is_success() {
        return Err(provider_error(
            provider,
            ErrorKind::Other,
            format!("the model list answered {status}"),
        ));
    }
    let text = response.text().await.map_err(|e| {
        provider_error(
            provider,
            ErrorKind::Network,
            format!("the model list broke off: {e}"),
        )
    })?;
    let body: Value = serde_json::from_str(&text).map_err(|e| {
        provider_error(
            provider,
            ErrorKind::Other,
            format!("the model list was not JSON: {e}"),
        )
    })?;
    Ok(body["data"]
        .as_array()
        .map(|models| {
            models
                .iter()
                .filter_map(|m| m["id"].as_str())
                .map(|id| id.strip_prefix("models/").unwrap_or(id).to_owned())
                .collect()
        })
        .unwrap_or_default())
}

/// Whether an error answer means the model does not exist. Providers say it in different
/// shapes: OpenAI and Groq a `model_not_found` code, Google a 404 naming `models/…`.
fn model_not_found(status: StatusCode, body: &str) -> bool {
    let lower = body.to_lowercase();
    lower.contains("model_not_found")
        || (status == StatusCode::NOT_FOUND && lower.contains("model"))
        || (lower.contains("model")
            && (lower.contains("does not exist") || lower.contains("not found")))
}

impl CleanupProvider for ChatCleanup {
    fn set_dictionary(&self, line: &str) {
        line.clone_into(
            &mut self
                .dictionary
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
    }

    /// R-2: opens the connection on key-down, so the cleanup call finds it ready.
    async fn warm(&self) -> Result<(), Error> {
        let response = self
            .authorized(self.http.get(format!("{}/models", self.endpoint)))
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
            .authorized(
                self.http
                    .post(format!("{}/chat/completions", self.endpoint)),
            )
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
        config.prompt = Some(prompt.to_owned());
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

    /// Formatting, AC-6: `v5` is `v4` with the list and paragraph rule before the dictionary
    /// rule, and light uses it.
    #[test]
    fn v5_is_v4_with_the_formatting_rule() {
        let v5 = prompt_named("v5").unwrap();
        assert_eq!(v5, format!("{PROMPT_V3} {FORMAT_RULE} {V4_RULE}"));
        assert!(v5.contains("\"- \""));
        assert!(v5.contains("No headings or bold"));
    }

    /// Formatting, AC-8: a short plain dictation gets `v4`'s prompt, without the formatting
    /// rule; a long one, a spoken enumeration or a break word gets `v5`'s.
    #[test]
    fn only_a_dictation_that_may_need_formatting_gets_the_rule() {
        assert!(!needs_formatting(
            "Sounds good, see you at the office tomorrow."
        ));
        assert!(!needs_formatting(
            "Vale, nos vemos a las ocho en la puerta del cine."
        ));
        assert!(needs_formatting(
            "Things to buy, milk, eggs, bread and coffee."
        ));
        assert!(needs_formatting("First call the bank and then send it."));
        assert!(needs_formatting("Hecho. Punto y aparte. Ahora lo otro."));
        assert!(needs_formatting(&"word ".repeat(40)));
        let body = |raw| {
            cleanup("low", "v5").unwrap().request_body(raw, None)["messages"][0]["content"]
                .as_str()
                .unwrap()
                .to_owned()
        };
        assert_eq!(
            body("sounds good see you then"),
            prompt_named("v4").unwrap()
        );
        assert_eq!(body("first this, then that"), prompt_named("v5").unwrap());
    }

    /// P2-2 AC-6: the level picks its named prompt; `prompt` overrides it. Light is v4, and
    /// medium keeps v4's dictionary rule and its never-rephrase rule.
    #[test]
    fn the_level_picks_the_prompt_and_prompt_overrides_it() {
        let mut c = config::Config::default().cleanup;
        c.prompt = None;
        c.level = CleanupLevel::Light;
        assert_eq!(prompt_version(&c), "v5");
        c.level = CleanupLevel::Medium;
        assert_eq!(prompt_version(&c), "m2");
        let medium = prompt_named(prompt_version(&c)).unwrap();
        assert!(medium.ends_with(V4_RULE));
        assert!(medium.contains("Never translate, rephrase, summarise or answer"));
        assert_ne!(medium, prompt_named("v5").unwrap());
        c.prompt = Some("v3".to_owned());
        assert_eq!(prompt_version(&c), "v3");
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
            system.ends_with(&AppStyle::Chat.prompt_line("Slack")),
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
            format!("{start}{}", AppStyle::Chat.prompt_line("Slack"))
        );
        // Issue 219: an app with no style keeps the plain line.
        assert_eq!(
            system("Safari"),
            format!("{start}\n\nThe text will be pasted into Safari.")
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
