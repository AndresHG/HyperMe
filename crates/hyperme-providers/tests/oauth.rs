//! Slice 2 of specs/97-google-connectors: OAuth 2.0 for native apps against a local fake OAuth
//! server. Tests cite the acceptance criteria in `specs/97-google-connectors/spec.md`. No test
//! calls Google or needs a key.

mod fake_oauth;

use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fake_oauth::FakeOAuth;
use hyperme_core::secrets::{MemorySecretStore, SecretStore};
use hyperme_providers::oauth::{
    Browser, DEFAULT_LISTEN_TIMEOUT, Loopback, OAuthClient, OAuthConfig, OAuthError,
};
use reqwest::Url;

const ACCOUNT: &str = "google:personal";
const SCOPES: &[&str] = &["calendar.read", "mail.read"];

fn client(fake: &FakeOAuth) -> OAuthClient {
    OAuthClient::new(config(fake))
}

fn config(fake: &FakeOAuth) -> OAuthConfig {
    OAuthConfig {
        auth_url: fake.auth_url(),
        token_url: fake.token_url(),
        revoke_url: Some(fake.revoke_url()),
        client_id: "fake-client-id".to_owned(),
        client_secret: Some("fake-client-secret".to_owned()),
        extra_auth_params: vec![
            ("access_type".to_owned(), "offline".to_owned()),
            ("prompt".to_owned(), "consent".to_owned()),
        ],
    }
}

/// What the browser was asked to open, shared with the test.
type Opened = Arc<Mutex<Vec<String>>>;

/// Opens the consent URL like a browser whose user clicks "Allow": the fake server redirects to
/// the loopback listener, and the request follows the redirect.
#[derive(Debug, Default)]
struct AllowingBrowser {
    opened: Opened,
}

impl Browser for AllowingBrowser {
    fn open(&self, url: &str) -> Result<(), String> {
        self.opened.lock().unwrap().push(url.to_owned());
        let url = url.to_owned();
        tokio::spawn(async move {
            let _ = reqwest::get(url).await;
        });
        Ok(())
    }
}

/// First sends the listener a code with a state the app never made, as another local process
/// could, then lets the user click "Allow".
#[derive(Debug, Default)]
struct ForgingBrowser {
    opened: Opened,
    forged_status: Arc<Mutex<Option<u16>>>,
}

impl Browser for ForgingBrowser {
    fn open(&self, url: &str) -> Result<(), String> {
        self.opened.lock().unwrap().push(url.to_owned());
        let redirect = query(url, "redirect_uri");
        let (url, forged_status) = (url.to_owned(), self.forged_status.clone());
        tokio::spawn(async move {
            let forged = reqwest::get(format!("{redirect}?code=fake-code-forged&state=forged"))
                .await
                .unwrap();
            *forged_status.lock().unwrap() = Some(forged.status().as_u16());
            let _ = reqwest::get(url).await;
        });
        Ok(())
    }
}

/// The user clicks "Cancel": the provider redirects with `error=access_denied` and the state.
#[derive(Debug, Default)]
struct DenyingBrowser;

impl Browser for DenyingBrowser {
    fn open(&self, url: &str) -> Result<(), String> {
        let (redirect, state) = (query(url, "redirect_uri"), query(url, "state"));
        tokio::spawn(async move {
            let _ = reqwest::get(format!("{redirect}?error=access_denied&state={state}")).await;
        });
        Ok(())
    }
}

/// The user never comes back from the browser.
#[derive(Debug, Default)]
struct SilentBrowser {
    opened: Opened,
}

impl Browser for SilentBrowser {
    fn open(&self, url: &str) -> Result<(), String> {
        self.opened.lock().unwrap().push(url.to_owned());
        Ok(())
    }
}

fn query(url: &str, name: &str) -> String {
    Url::parse(url)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.into_owned())
        .unwrap_or_else(|| panic!("{name} missing from {url}"))
}

fn port_of(redirect_uri: &str) -> u16 {
    Url::parse(redirect_uri).unwrap().port().unwrap()
}

async fn is_listening(port: u16) -> bool {
    tokio::net::TcpStream::connect((Ipv4Addr::LOCALHOST, port))
        .await
        .is_ok()
}

async fn signed_in(fake: &FakeOAuth, store: &MemorySecretStore) -> OAuthClient {
    let client = client(fake);
    client
        .sign_in(
            &AllowingBrowser::default(),
            store,
            ACCOUNT,
            SCOPES,
            DEFAULT_LISTEN_TIMEOUT,
        )
        .await
        .unwrap();
    client
}

/// AC-2: the consent URL carries an S256 challenge, a state, the scopes and the extra params;
/// the code exchange sends the verifier, and the fake server accepts it only if it hashes to
/// the challenge. The listener is on 127.0.0.1 and is closed once the code arrives.
#[tokio::test]
async fn sign_in_uses_pkce_s256_and_closes_the_listener_after_the_code() {
    let fake = FakeOAuth::start().await;
    let store = MemorySecretStore::default();
    let browser = AllowingBrowser::default();
    let token = client(&fake)
        .sign_in(&browser, &store, ACCOUNT, SCOPES, DEFAULT_LISTEN_TIMEOUT)
        .await
        .unwrap();

    let opened = browser.opened.lock().unwrap().clone();
    assert_eq!(opened.len(), 1);
    let consent = &opened[0];
    assert!(consent.starts_with(&fake.auth_url()), "{consent}");
    assert_eq!(query(consent, "code_challenge_method"), "S256");
    assert_eq!(query(consent, "response_type"), "code");
    assert_eq!(query(consent, "client_id"), "fake-client-id");
    assert_eq!(query(consent, "scope"), "calendar.read mail.read");
    assert_eq!(query(consent, "access_type"), "offline");
    assert_eq!(query(consent, "prompt"), "consent");
    assert!(query(consent, "state").len() >= 32);
    assert_eq!(query(consent, "code_challenge").len(), 43);

    let redirect = query(consent, "redirect_uri");
    assert!(redirect.starts_with("http://127.0.0.1:"), "{redirect}");

    let exchange = &fake.token_requests()[0];
    assert_eq!(exchange["grant_type"], "authorization_code");
    assert_eq!(exchange["code_verifier"].len(), 43);
    assert_eq!(exchange["redirect_uri"], redirect);

    assert_eq!(token.secret(), fake.issued_access_tokens()[0]);
    assert!(
        !is_listening(port_of(&redirect)).await,
        "the listener is still open after the code"
    );
}

/// AC-2: the listener binds the IPv4 loopback address only, on a port the OS picks.
#[tokio::test]
async fn the_listener_binds_only_loopback_on_a_free_port() {
    let first = Loopback::bind().await.unwrap();
    let second = Loopback::bind().await.unwrap();
    assert_eq!(first.local_addr().ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
    assert_ne!(first.local_addr().port(), 0);
    assert_ne!(first.local_addr().port(), second.local_addr().port());
    assert_eq!(
        first.redirect_uri(),
        format!("http://127.0.0.1:{}/", first.local_addr().port())
    );
}

/// AC-2: a redirect with a state the app did not make is refused (400) and its code is never
/// exchanged; it does not end the sign-in, so the real redirect still completes it.
#[tokio::test]
async fn a_wrong_state_is_rejected() {
    let fake = FakeOAuth::start().await;
    let store = MemorySecretStore::default();
    let browser = ForgingBrowser::default();
    client(&fake)
        .sign_in(&browser, &store, ACCOUNT, SCOPES, DEFAULT_LISTEN_TIMEOUT)
        .await
        .unwrap();

    assert_eq!(*browser.forged_status.lock().unwrap(), Some(400));
    let exchanges = fake.token_requests();
    assert_eq!(exchanges.len(), 1);
    assert_ne!(exchanges[0]["code"], "fake-code-forged");
    assert_eq!(
        store.get(ACCOUNT).unwrap().as_deref(),
        Some(fake.issued_refresh_tokens()[0].as_str())
    );
}

/// AC-2: when the user cancels at the consent page, sign-in ends with the provider's error
/// code, exchanges nothing and closes the listener.
#[tokio::test]
async fn a_cancelled_consent_ends_the_sign_in() {
    let fake = FakeOAuth::start().await;
    let store = MemorySecretStore::default();
    let result = client(&fake)
        .sign_in(
            &DenyingBrowser,
            &store,
            ACCOUNT,
            SCOPES,
            DEFAULT_LISTEN_TIMEOUT,
        )
        .await;
    assert!(
        matches!(&result, Err(OAuthError::Denied(code)) if code == "access_denied"),
        "{result:?}"
    );
    assert!(fake.token_requests().is_empty());
    assert_eq!(store.get(ACCOUNT).unwrap(), None);
}

/// AC-2: when the browser never comes back, sign-in gives up after the default 5 minutes and
/// the listener is closed. The clock is paused, so the test takes no real time.
#[tokio::test(start_paused = true)]
async fn the_listener_closes_after_five_minutes() {
    assert_eq!(DEFAULT_LISTEN_TIMEOUT, Duration::from_secs(5 * 60));
    let fake = FakeOAuth::start().await;
    let store = MemorySecretStore::default();
    let browser = SilentBrowser::default();
    let started = tokio::time::Instant::now();
    let result = client(&fake)
        .sign_in(&browser, &store, ACCOUNT, SCOPES, DEFAULT_LISTEN_TIMEOUT)
        .await;

    assert!(matches!(result, Err(OAuthError::Timeout)), "{result:?}");
    assert!(started.elapsed() >= DEFAULT_LISTEN_TIMEOUT);
    let redirect = query(&browser.opened.lock().unwrap()[0], "redirect_uri");
    assert!(!is_listening(port_of(&redirect)).await);
}

/// AC-2: the timeout is configurable.
#[tokio::test(start_paused = true)]
async fn the_timeout_is_configurable() {
    let fake = FakeOAuth::start().await;
    let store = MemorySecretStore::default();
    let started = tokio::time::Instant::now();
    let result = client(&fake)
        .sign_in(
            &SilentBrowser::default(),
            &store,
            ACCOUNT,
            SCOPES,
            Duration::from_secs(30),
        )
        .await;
    assert!(matches!(result, Err(OAuthError::Timeout)), "{result:?}");
    let waited = started.elapsed();
    assert!(waited >= Duration::from_secs(30) && waited < Duration::from_secs(31));
}

/// AC-3: after sign-in the refresh token is in the (fake) keychain under the account, the
/// access token is not, and neither shows in any debug or error text, which is what a log
/// would hold.
#[tokio::test]
async fn the_refresh_token_is_only_in_the_keychain() {
    let fake = FakeOAuth::start().await;
    let store = MemorySecretStore::default();
    let client = client(&fake);
    let token = client
        .sign_in(
            &AllowingBrowser::default(),
            &store,
            ACCOUNT,
            SCOPES,
            DEFAULT_LISTEN_TIMEOUT,
        )
        .await
        .unwrap();

    let refresh = fake.issued_refresh_tokens()[0].clone();
    let access = fake.issued_access_tokens()[0].clone();
    assert_eq!(
        store.get(ACCOUNT).unwrap().as_deref(),
        Some(refresh.as_str())
    );

    let mut shown = format!("{token:?} {client:?} {store:?}");
    fake.revoke_all_refresh_tokens();
    let error = client.refresh(&store, ACCOUNT).await.unwrap_err();
    shown.push_str(&format!(" {error:?} {error}"));
    for secret in [&refresh, &access, "fake-client-secret"] {
        assert!(!shown.contains(secret), "{secret} leaked: {shown}");
    }
}

/// AC-4 (refresh): an expired access token is refreshed without asking the user; a live one
/// is used as is.
#[tokio::test]
async fn an_expired_access_token_is_refreshed() {
    let fake = FakeOAuth::start().await;
    let store = MemorySecretStore::default();
    fake.set_access_ttl(0);
    let client = signed_in(&fake, &store).await;
    let expired = client.fresh_token(&store, ACCOUNT, None).await.unwrap();
    assert_eq!(fake.refresh_calls(), 1);

    fake.set_access_ttl(3600);
    let fresh = client
        .fresh_token(&store, ACCOUNT, Some(&expired))
        .await
        .unwrap();
    assert_eq!(fake.refresh_calls(), 2);
    assert_ne!(fresh.secret(), expired.secret());
    assert_eq!(fresh.secret(), fake.issued_access_tokens().last().unwrap());

    let again = client
        .fresh_token(&store, ACCOUNT, Some(&fresh))
        .await
        .unwrap();
    assert_eq!(again.secret(), fresh.secret());
    assert_eq!(fake.refresh_calls(), 2, "a live token was refreshed");
}

/// AC-4 (refresh): a refused refresh token gives the distinct "sign in again" error, so the
/// caller can mark the account and raise one alert; the token stays until the user acts.
#[tokio::test]
async fn a_revoked_refresh_token_asks_to_sign_in_again() {
    let fake = FakeOAuth::start().await;
    let store = MemorySecretStore::default();
    let client = signed_in(&fake, &store).await;
    fake.revoke_all_refresh_tokens();

    let error = client.refresh(&store, ACCOUNT).await.unwrap_err();
    assert!(matches!(error, OAuthError::SignInAgain), "{error:?}");
}

/// AC-4: refreshing an account that never signed in says so, without a network call.
#[tokio::test]
async fn refresh_without_a_sign_in_is_not_signed_in() {
    let fake = FakeOAuth::start().await;
    let error = client(&fake)
        .refresh(&MemorySecretStore::default(), ACCOUNT)
        .await
        .unwrap_err();
    assert!(matches!(error, OAuthError::NotSignedIn), "{error:?}");
    assert!(fake.token_requests().is_empty());
}

/// AC-6 (revoke): disconnect calls the revoke endpoint with the refresh token and removes the
/// keychain item; refreshing afterwards finds no sign-in.
#[tokio::test]
async fn revoke_calls_the_endpoint_and_removes_the_keychain_item() {
    let fake = FakeOAuth::start().await;
    let store = MemorySecretStore::default();
    let client = signed_in(&fake, &store).await;
    let refresh = fake.issued_refresh_tokens()[0].clone();

    client.revoke(&store, ACCOUNT).await.unwrap();
    assert_eq!(fake.revoked(), vec![refresh]);
    assert_eq!(store.get(ACCOUNT).unwrap(), None);
    assert!(matches!(
        client.refresh(&store, ACCOUNT).await,
        Err(OAuthError::NotSignedIn)
    ));
}

/// AC-6 (revoke): when the provider cannot be reached, disconnect still removes the keychain
/// item and returns the error, so the caller can say access may still be listed there.
#[tokio::test]
async fn revoke_removes_the_item_even_when_the_provider_is_unreachable() {
    let fake = FakeOAuth::start().await;
    let store = MemorySecretStore::default();
    signed_in(&fake, &store).await;
    let closed = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let unreachable = format!("http://{}/revoke", closed.local_addr().unwrap());
    drop(closed);
    let client = OAuthClient::new(OAuthConfig {
        revoke_url: Some(unreachable),
        ..config(&fake)
    });

    let error = client.revoke(&store, ACCOUNT).await.unwrap_err();
    assert!(matches!(error, OAuthError::Network(_)), "{error:?}");
    assert_eq!(store.get(ACCOUNT).unwrap(), None);
    assert!(fake.revoked().is_empty());
}
