//! OAuth 2.0 sign-in for native apps ([RFC 8252](https://www.rfc-editor.org/rfc/rfc8252)), for
//! any provider a connector needs (specs/97-google-connectors, R-7 to R-12). Nothing here is
//! Google code: the endpoints, client and extra parameters come from the connector's preset.
//!
//! The flow: bind a listener on `127.0.0.1` on a port the OS picks, make a PKCE verifier from
//! 32 random bytes and a random `state` ([RFC 7636](https://www.rfc-editor.org/rfc/rfc7636),
//! S256), open the consent page in the system browser, take the code from the redirect, close
//! the listener, and exchange the code. The refresh token goes to the [`SecretStore`] (the
//! Keychain in the app); the access token stays in memory in an [`AccessToken`].
//!
//! No token, verifier or client secret appears in a `Debug` or `Display` output or in an
//! error, so none can reach a log (R-9).

use std::fmt;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hyperme_core::registry::OAuthEndpoints;
use hyperme_core::secrets::{SecretError, SecretStore};
use reqwest::header::{CONTENT_TYPE, HeaderValue};
use reqwest::{StatusCode, Url};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::Instant;

/// How long the listener waits for the browser to come back (R-8).
pub const DEFAULT_LISTEN_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// An access token this close to its expiry is refreshed before use.
const EXPIRY_MARGIN: Duration = Duration::from_secs(60);

/// The life of an access token whose response has no `expires_in`. A request that still gets
/// a 401 refreshes then (slice 3).
const DEFAULT_ACCESS_LIFE: Duration = Duration::from_secs(3600);

/// How long one connection to the listener may take to send its request line.
const REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(10);

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// One provider's OAuth endpoints and the user's own client (G-1).
#[derive(Clone)]
pub struct OAuthConfig {
    pub auth_url: String,
    pub token_url: String,
    /// `None` when the provider has no revoke endpoint; disconnect then only forgets the token.
    pub revoke_url: Option<String>,
    pub client_id: String,
    /// Desktop clients are public and the secret is optional; when there is one it is the
    /// user's and comes from the Keychain.
    pub client_secret: Option<String>,
    /// Extra consent parameters from the preset, such as Google's `access_type=offline`.
    pub extra_auth_params: Vec<(String, String)>,
}

impl OAuthConfig {
    /// The config for a connector's endpoints from the registry, with the user's own client.
    pub fn from_endpoints(
        endpoints: &OAuthEndpoints,
        client_id: &str,
        client_secret: Option<String>,
    ) -> Self {
        OAuthConfig {
            auth_url: endpoints.auth_url.clone(),
            token_url: endpoints.token_url.clone(),
            revoke_url: endpoints.revoke_url.clone(),
            client_id: client_id.to_owned(),
            client_secret,
            extra_auth_params: endpoints.extra_auth_params.clone(),
        }
    }
}

impl fmt::Debug for OAuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuthConfig")
            .field("auth_url", &self.auth_url)
            .field("token_url", &self.token_url)
            .field("revoke_url", &self.revoke_url)
            .field("client_id", &self.client_id)
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "<hidden>"),
            )
            .field("extra_auth_params", &self.extra_auth_params)
            .finish()
    }
}

/// Why a sign-in, refresh or revoke failed. Never carries a token or a secret.
#[derive(Debug)]
pub enum OAuthError {
    /// The browser did not come back before the timeout; the listener is closed.
    Timeout,
    /// The provider sent an error to the redirect instead of a code, such as `access_denied`.
    Denied(String),
    /// The refresh token was refused (`invalid_grant`): the account needs a new sign-in, and
    /// the caller should not retry until the user does (R-10).
    SignInAgain,
    /// No refresh token is stored for the account.
    NotSignedIn,
    /// The token response had no refresh token, so the account could not stay signed in.
    NoRefreshToken,
    /// The provider answered with an error, by HTTP status and OAuth `error` code.
    Provider {
        status: u16,
        error: String,
    },
    /// The provider could not be reached or its answer could not be read.
    Network(String),
    /// The loopback listener could not be opened.
    Listener(String),
    /// The browser could not be opened.
    Browser(String),
    /// The system could not give random bytes.
    Random(String),
    /// An endpoint in the preset is not a URL.
    Config(String),
    Keychain(SecretError),
}

impl fmt::Display for OAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OAuthError::Timeout => write!(f, "sign-in timed out waiting for the browser"),
            OAuthError::Denied(error) => write!(f, "sign-in was not allowed: {error}"),
            OAuthError::SignInAgain => write!(f, "the account needs a new sign-in"),
            OAuthError::NotSignedIn => write!(f, "the account is not signed in"),
            OAuthError::NoRefreshToken => {
                write!(
                    f,
                    "the provider gave no refresh token, so the account cannot stay signed in"
                )
            }
            OAuthError::Provider { status, error } => {
                write!(f, "the provider answered {status}: {error}")
            }
            OAuthError::Network(reason) => write!(f, "could not reach the provider: {reason}"),
            OAuthError::Listener(reason) => {
                write!(f, "could not open the sign-in listener: {reason}")
            }
            OAuthError::Browser(reason) => write!(f, "could not open the browser: {reason}"),
            OAuthError::Random(reason) => write!(f, "no random bytes: {reason}"),
            OAuthError::Config(reason) => write!(f, "connector preset: {reason}"),
            OAuthError::Keychain(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for OAuthError {}

impl From<SecretError> for OAuthError {
    fn from(error: SecretError) -> Self {
        OAuthError::Keychain(error)
    }
}

/// Opens the consent page. The app opens the system browser, never an embedded web view
/// (R-7); tests drive the redirect themselves.
pub trait Browser {
    fn open(&self, url: &str) -> Result<(), String>;
}

/// The user's default browser.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemBrowser;

impl Browser for SystemBrowser {
    fn open(&self, url: &str) -> Result<(), String> {
        // The URL is one argument, never passed through a shell.
        #[cfg(target_os = "macos")]
        let mut command = std::process::Command::new("open");
        #[cfg(target_os = "windows")]
        let mut command = {
            let mut c = std::process::Command::new("rundll32");
            c.arg("url.dll,FileProtocolHandler");
            c
        };
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let mut command = std::process::Command::new("xdg-open");
        let mut child = command
            .arg(url)
            .spawn()
            .map_err(|error| error.to_string())?;
        // The opener exits at once; waiting on a thread reaps it, so no zombie is left.
        std::thread::spawn(move || child.wait());
        Ok(())
    }
}

/// An access token, kept in memory only (R-9). `Debug` shows when it expires, not the token.
#[derive(Clone)]
pub struct AccessToken {
    secret: String,
    /// When to refresh: a minute before expiry, or halfway for a token that lives under two.
    refresh_after: Instant,
}

impl AccessToken {
    /// The bearer token, for the `Authorization` header.
    pub fn secret(&self) -> &str {
        &self.secret
    }

    fn new(secret: String, life: Duration) -> Self {
        let margin = EXPIRY_MARGIN.min(life / 2);
        AccessToken {
            secret,
            refresh_after: Instant::now() + (life - margin),
        }
    }

    /// Whether the token can still be used without a refresh.
    pub fn is_fresh(&self) -> bool {
        Instant::now() < self.refresh_after
    }
}

impl fmt::Debug for AccessToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccessToken")
            .field("fresh", &self.is_fresh())
            .finish_non_exhaustive()
    }
}

/// A PKCE pair (RFC 7636): the verifier stays here, the S256 challenge goes in the consent URL.
pub struct Pkce {
    verifier: String,
    challenge: String,
}

impl Pkce {
    /// A verifier from 32 random bytes (43 base64url characters).
    pub fn new() -> Result<Self, OAuthError> {
        Ok(Self::from_verifier(random_token()?))
    }

    pub fn from_verifier(verifier: String) -> Self {
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Pkce {
            verifier,
            challenge,
        }
    }

    pub fn verifier(&self) -> &str {
        &self.verifier
    }

    pub fn challenge(&self) -> &str {
        &self.challenge
    }
}

impl fmt::Debug for Pkce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pkce")
            .field("challenge", &self.challenge)
            .finish_non_exhaustive()
    }
}

/// 32 random bytes as base64url: a PKCE verifier or a `state`.
pub fn random_token() -> Result<String, OAuthError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|error| OAuthError::Random(error.to_string()))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// The listener the browser is redirected to, on `127.0.0.1` only (R-8).
#[derive(Debug)]
pub struct Loopback {
    listener: TcpListener,
    addr: SocketAddr,
}

/// What one request to the listener brought.
type Outcome = Result<String, OAuthError>;

impl Loopback {
    /// Binds `127.0.0.1` on a port the OS picks, so the port is never taken.
    pub async fn bind() -> Result<Self, OAuthError> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|error| OAuthError::Listener(error.to_string()))?;
        let addr = listener
            .local_addr()
            .map_err(|error| OAuthError::Listener(error.to_string()))?;
        Ok(Loopback { listener, addr })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// `http://127.0.0.1:<port>/`, the `redirect_uri` for the consent URL and the exchange.
    pub fn redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}/", self.addr.port())
    }

    /// Waits for the redirect and returns its code. Requests without a code or an error (a
    /// favicon, a preconnect) are answered 404 and ignored. A request with a `state` this
    /// sign-in did not make is answered 400 and ignored too, so another local process can
    /// neither inject a code nor end the sign-in; the real redirect or the timeout ends it.
    /// The listener is closed when this returns, whatever the outcome.
    pub async fn wait_for_code(self, state: &str, timeout: Duration) -> Outcome {
        let (sender, mut outcomes) = mpsc::channel::<Outcome>(4);
        let expected = state.to_owned();
        let listener = self.listener;
        let accept = tokio::spawn(async move {
            loop {
                let stream = match listener.accept().await {
                    Ok((stream, _)) => stream,
                    // A connection that failed before it was accepted, or no free file
                    // descriptor for a moment: keep listening.
                    Err(_) => {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        continue;
                    }
                };
                let (sender, expected) = (sender.clone(), expected.clone());
                // One task per connection, so a silent connection cannot hold up the redirect.
                tokio::spawn(async move {
                    if let Some(outcome) = answer_redirect(stream, &expected).await {
                        let _ = sender.send(outcome).await;
                    }
                });
            }
        });
        let outcome = match tokio::time::timeout(timeout, outcomes.recv()).await {
            Ok(Some(outcome)) => outcome,
            // The accept task holds a sender and never ends on its own.
            Ok(None) => Err(OAuthError::Listener("the listener stopped".to_owned())),
            Err(_) => Err(OAuthError::Timeout),
        };
        // Waiting for the cancelled task drops the listener before this returns.
        accept.abort();
        let _ = accept.await;
        outcome
    }
}

const PAGE_DONE: &str = "<!doctype html><meta charset=utf-8><title>HyperMe</title>\
<p>You are signed in. You can close this tab and go back to HyperMe.</p>";
const PAGE_FAILED: &str = "<!doctype html><meta charset=utf-8><title>HyperMe</title>\
<p>Sign-in did not finish. Go back to HyperMe and try again.</p>";

/// Reads one request, answers it, and returns what it meant for the sign-in, if anything.
async fn answer_redirect(mut stream: TcpStream, expected_state: &str) -> Option<Outcome> {
    let target = tokio::time::timeout(REQUEST_READ_TIMEOUT, read_target(&mut stream))
        .await
        .ok()
        .flatten()?;
    let url = Url::parse(&format!("http://127.0.0.1{target}")).ok()?;
    let (mut code, mut state, mut error) = (None, None, None);
    for (name, value) in url.query_pairs() {
        match name.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            "error" => error = Some(value.into_owned()),
            _ => {}
        }
    }
    let (status, page, outcome) = if code.is_none() && error.is_none() {
        ("404 Not Found", "", None)
    } else if state.as_deref() != Some(expected_state) {
        ("400 Bad Request", PAGE_FAILED, None)
    } else if let Some(error) = error {
        (
            "200 OK",
            PAGE_FAILED,
            Some(Err(OAuthError::Denied(oauth_code(&error)))),
        )
    } else {
        ("200 OK", PAGE_DONE, code.map(Ok))
    };
    let response = format!(
        "HTTP/1.1 {status}\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{page}",
        page.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
    outcome
}

/// The request target (`/?code=…&state=…`) from the request line.
async fn read_target(stream: &mut TcpStream) -> Option<String> {
    let mut received = Vec::new();
    let mut chunk = [0u8; 1024];
    while !received.windows(2).any(|w| w == b"\r\n") {
        if received.len() > 16 * 1024 {
            return None;
        }
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return None,
            Ok(n) => received.extend_from_slice(&chunk[..n]),
        }
    }
    let line = String::from_utf8_lossy(&received);
    let mut parts = line.lines().next()?.split(' ');
    match (parts.next(), parts.next()) {
        (Some("GET"), Some(target)) if target.starts_with('/') => Some(target.to_owned()),
        _ => None,
    }
}

/// An OAuth error code as the provider sent it, kept to the characters such codes use, so a
/// crafted redirect cannot put arbitrary text in a message.
fn oauth_code(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .take(64)
        .collect()
}

/// `application/x-www-form-urlencoded` for a token or revoke request body.
fn form(pairs: &[(&str, &str)]) -> String {
    let mut url = Url::parse("http://localhost/").expect("a constant URL");
    url.query_pairs_mut().extend_pairs(pairs);
    url.query().unwrap_or_default().to_owned()
}

/// One provider's client: sign-in, refresh and revoke.
pub struct OAuthClient {
    config: OAuthConfig,
    http: reqwest::Client,
}

impl fmt::Debug for OAuthClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuthClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// What the token endpoint gave.
struct TokenResponse {
    access: AccessToken,
    refresh: Option<String>,
}

impl OAuthClient {
    pub fn new(config: OAuthConfig) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .unwrap_or_default();
        OAuthClient { config, http }
    }

    /// The consent page URL: authorization code flow with PKCE S256, the scopes asked for,
    /// then the preset's extra parameters.
    pub fn consent_url(
        &self,
        redirect_uri: &str,
        scopes: &[&str],
        state: &str,
        challenge: &str,
    ) -> Result<String, OAuthError> {
        let mut url = Url::parse(&self.config.auth_url)
            .map_err(|error| OAuthError::Config(format!("auth_url: {error}")))?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.config.client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("scope", &scopes.join(" "))
            .append_pair("state", state)
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256")
            .extend_pairs(&self.config.extra_auth_params);
        Ok(url.into())
    }

    /// Signs `account` in: opens the consent page, waits up to `timeout` for the redirect,
    /// exchanges the code, and stores the refresh token under `account` in `store`. A new
    /// sign-in replaces the account's old token. `store` is the connectors' own keychain
    /// service (`ai.hyperme.connector` in the design), never the one that holds provider keys,
    /// so an account label cannot overwrite a key.
    pub async fn sign_in(
        &self,
        browser: &(dyn Browser + Sync),
        store: &(dyn SecretStore + Sync),
        account: &str,
        scopes: &[&str],
        timeout: Duration,
    ) -> Result<AccessToken, OAuthError> {
        let loopback = Loopback::bind().await?;
        let redirect_uri = loopback.redirect_uri();
        let pkce = Pkce::new()?;
        let state = random_token()?;
        let url = self.consent_url(&redirect_uri, scopes, &state, pkce.challenge())?;
        browser.open(&url).map_err(OAuthError::Browser)?;
        let code = loopback.wait_for_code(&state, timeout).await?;

        let mut params = vec![
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("code_verifier", pkce.verifier()),
            ("redirect_uri", redirect_uri.as_str()),
        ];
        let tokens = self.token_request(&mut params).await?;
        let refresh = tokens.refresh.ok_or(OAuthError::NoRefreshToken)?;
        store.set(account, &refresh)?;
        Ok(tokens.access)
    }

    /// A new access token from the account's refresh token. A refused refresh token gives
    /// [`OAuthError::SignInAgain`]; a rotated one replaces the stored one.
    pub async fn refresh(
        &self,
        store: &(dyn SecretStore + Sync),
        account: &str,
    ) -> Result<AccessToken, OAuthError> {
        let mut refresh = store.get(account)?.ok_or(OAuthError::NotSignedIn)?;
        let mut retried = false;
        loop {
            let mut params = vec![
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh.as_str()),
            ];
            match self.token_request(&mut params).await {
                Ok(tokens) => {
                    if let Some(rotated) = tokens.refresh.filter(|new| *new != refresh) {
                        store.set(account, &rotated)?;
                    }
                    return Ok(tokens.access);
                }
                Err(OAuthError::Provider { error, .. }) if error == "invalid_grant" => {
                    // A provider that rotates refresh tokens refuses the old one once a
                    // concurrent refresh has stored a new one: try that one, once.
                    match store.get(account)? {
                        Some(stored) if !retried && stored != refresh => {
                            refresh = stored;
                            retried = true;
                        }
                        _ => return Err(OAuthError::SignInAgain),
                    }
                }
                Err(other) => return Err(other),
            }
        }
    }

    /// `cached` if it is still fresh, otherwise a refreshed token.
    pub async fn fresh_token(
        &self,
        store: &(dyn SecretStore + Sync),
        account: &str,
        cached: Option<&AccessToken>,
    ) -> Result<AccessToken, OAuthError> {
        match cached {
            Some(token) if token.is_fresh() => Ok(token.clone()),
            _ => self.refresh(store, account).await,
        }
    }

    /// Disconnects `account` (R-12): revokes its refresh token at the provider, then removes
    /// it from `store`. The token is removed even when the provider cannot be reached, and the
    /// error is returned so the caller can say access may still be listed at the provider.
    pub async fn revoke(
        &self,
        store: &(dyn SecretStore + Sync),
        account: &str,
    ) -> Result<(), OAuthError> {
        let Some(refresh) = store.get(account)? else {
            return Ok(());
        };
        let revoked = match &self.config.revoke_url {
            Some(url) => self.revoke_request(url, &refresh).await,
            None => Ok(()),
        };
        store.delete(account)?;
        revoked
    }

    async fn revoke_request(&self, url: &str, token: &str) -> Result<(), OAuthError> {
        let mut params = vec![("token", token), ("token_type_hint", "refresh_token")];
        let (status, body) = self.post_form(url, &mut params).await?;
        // RFC 7009: a token that is already invalid counts as revoked; Google answers 400
        // `invalid_token` for it.
        if status.is_success() || error_code(&body) == "invalid_token" {
            Ok(())
        } else {
            Err(OAuthError::Provider {
                status: status.as_u16(),
                error: error_code(&body),
            })
        }
    }

    async fn token_request<'a>(
        &'a self,
        params: &mut Vec<(&'a str, &'a str)>,
    ) -> Result<TokenResponse, OAuthError> {
        let url = self.config.token_url.clone();
        let (status, body) = self.post_form(&url, params).await?;
        if !status.is_success() {
            return Err(OAuthError::Provider {
                status: status.as_u16(),
                error: error_code(&body),
            });
        }
        let json: Value = serde_json::from_str(&body)
            .map_err(|_| OAuthError::Network("the token response is not JSON".to_owned()))?;
        let secret = json["access_token"]
            .as_str()
            .filter(|token| !token.is_empty())
            .ok_or_else(|| {
                OAuthError::Network("the token response has no access_token".to_owned())
            })?
            .to_owned();
        let life = json["expires_in"]
            .as_u64()
            .map_or(DEFAULT_ACCESS_LIFE, Duration::from_secs);
        Ok(TokenResponse {
            access: AccessToken::new(secret, life),
            refresh: json["refresh_token"]
                .as_str()
                .filter(|token| !token.is_empty())
                .map(str::to_owned),
        })
    }

    /// Posts a form with the client's credentials (`client_secret_post`) and returns the
    /// status and body.
    async fn post_form<'a>(
        &'a self,
        url: &str,
        params: &mut Vec<(&'a str, &'a str)>,
    ) -> Result<(StatusCode, String), OAuthError> {
        params.push(("client_id", &self.config.client_id));
        if let Some(secret) = &self.config.client_secret {
            params.push(("client_secret", secret));
        }
        let response = self
            .http
            .post(url)
            .header(
                CONTENT_TYPE,
                HeaderValue::from_static("application/x-www-form-urlencoded"),
            )
            .body(form(params))
            .send()
            .await
            .map_err(|error| OAuthError::Network(error.to_string()))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| OAuthError::Network(error.to_string()))?;
        Ok((status, body))
    }
}

/// The OAuth `error` code from an error body, or "no error code" when it has none. The rest of
/// the body is dropped, so nothing the provider echoes back can reach a message.
fn error_code(body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|json| json["error"].as_str().map(oauth_code))
        .filter(|code| !code.is_empty())
        .unwrap_or_else(|| "no error code".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AC-2: the S256 challenge matches RFC 7636, appendix B.
    #[test]
    fn s256_challenge_matches_the_rfc_example() {
        let pkce = Pkce::from_verifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".to_owned());
        assert_eq!(
            pkce.challenge(),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    /// AC-2: the verifier and the state come from 32 random bytes and differ every time.
    #[test]
    fn verifiers_and_states_are_random_and_43_characters() {
        let (a, b) = (random_token().unwrap(), random_token().unwrap());
        assert_eq!(a.len(), 43);
        assert_ne!(a, b);
        let pkce = Pkce::new().unwrap();
        assert_eq!(pkce.verifier().len(), 43);
        assert!(!format!("{pkce:?}").contains(pkce.verifier()));
    }

    /// R-10: a token that lives a minute is used for half of it rather than refreshed on
    /// every call; one that has expired is not used.
    #[test]
    fn short_lived_tokens_are_used_before_refreshing() {
        assert!(AccessToken::new("fake".to_owned(), Duration::from_secs(60)).is_fresh());
        assert!(AccessToken::new("fake".to_owned(), Duration::from_secs(3600)).is_fresh());
        assert!(!AccessToken::new("fake".to_owned(), Duration::ZERO).is_fresh());
    }

    /// R-9: an error body is reduced to its OAuth code.
    #[test]
    fn error_messages_keep_only_the_oauth_code() {
        assert_eq!(
            error_code(r#"{"error":"invalid_grant","error_description":"token fake-refresh-1"}"#),
            "invalid_grant"
        );
        assert_eq!(error_code("<html>fake-refresh-1</html>"), "no error code");
        assert_eq!(oauth_code("access_denied<script>"), "access_deniedscript");
    }
}
