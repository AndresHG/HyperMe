//! The network side of a connector's setup sheet (specs/97-google-connectors, slice 6): the
//! user's own OAuth client in the keychain (G-1), checking it, signing an account in with only
//! the scopes switched on (R-14, AC-7), refusing a client left in "Testing" (R-10a, AC-4a),
//! and Disconnect (R-12, AC-6). Nothing here is Google code; the preset says what to ask for.

use std::fmt;
use std::time::Duration;

use hyperme_core::connector_setup::{consent_scopes, in_testing};
use hyperme_core::connectors::AccountData;
use hyperme_core::registry::{Auth, Connector};
use hyperme_core::secrets::SecretStore;
use serde_json::{Value, json};

use crate::oauth::{AccessToken, Browser, OAuthClient, OAuthConfig, OAuthError};

/// Why setting up or disconnecting failed. Never carries a token or a secret.
#[derive(Debug)]
pub enum SetupError {
    /// The switches ask for nothing, or for something the connector does not offer.
    Scopes(String),
    /// The connector signs in some other way than OAuth.
    NotOAuth,
    /// No client ID is saved for the connector yet.
    NoClient,
    /// The client's project is in "Testing", so its sign-ins end after 7 days (R-10a). The
    /// sign-in was undone; `fix_url` is where the user sets it to "In production".
    ClientInTesting {
        fix_url: Option<String>,
    },
    Auth(OAuthError),
    /// The account's cached data could not be deleted.
    Data(String),
}

impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetupError::Scopes(reason) => write!(f, "{reason}"),
            SetupError::NotOAuth => write!(f, "this connector does not sign in with OAuth"),
            SetupError::NoClient => write!(f, "save your client ID first"),
            SetupError::ClientInTesting { .. } => write!(
                f,
                "your project is in Testing, so Google would sign you out every 7 days: set it \
                 to In production, then sign in again"
            ),
            SetupError::Auth(error) => write!(f, "{error}"),
            SetupError::Data(reason) => write!(f, "could not delete the account's data: {reason}"),
        }
    }
}

impl std::error::Error for SetupError {}

impl From<OAuthError> for SetupError {
    fn from(error: OAuthError) -> Self {
        SetupError::Auth(error)
    }
}

/// The user's own OAuth client for a connector (G-1). `Debug` never shows the secret.
#[derive(Clone, PartialEq, Eq)]
pub struct ClientCredentials {
    pub client_id: String,
    pub client_secret: Option<String>,
}

impl fmt::Debug for ClientCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientCredentials")
            .field("client_id", &self.client_id)
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "<hidden>"),
            )
            .finish()
    }
}

/// The keychain account that holds a connector's client: `google-client`.
pub fn client_item(connector: &str) -> String {
    format!("{connector}-client")
}

/// Saves the client in the keychain, as one item.
pub fn save_client(
    store: &dyn SecretStore,
    connector: &str,
    client: &ClientCredentials,
) -> Result<(), SetupError> {
    let client_id = client.client_id.trim();
    if client_id.is_empty() {
        return Err(SetupError::NoClient);
    }
    let secret = client
        .client_secret
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let value = json!({ "client_id": client_id, "client_secret": secret }).to_string();
    store
        .set(&client_item(connector), &value)
        .map_err(|e| SetupError::Auth(OAuthError::Keychain(e)))
}

/// The saved client, if there is one.
pub fn load_client(
    store: &dyn SecretStore,
    connector: &str,
) -> Result<Option<ClientCredentials>, SetupError> {
    let Some(value) = store
        .get(&client_item(connector))
        .map_err(|e| SetupError::Auth(OAuthError::Keychain(e)))?
    else {
        return Ok(None);
    };
    let json: Value = serde_json::from_str(&value).map_err(|_| SetupError::NoClient)?;
    let client_id = json["client_id"].as_str().ok_or(SetupError::NoClient)?;
    Ok(Some(ClientCredentials {
        client_id: client_id.to_owned(),
        client_secret: json["client_secret"].as_str().map(str::to_owned),
    }))
}

/// The OAuth client for `connector` with the user's saved client.
pub fn oauth_client(
    connector: &Connector,
    store: &dyn SecretStore,
) -> Result<OAuthClient, SetupError> {
    let Auth::OAuth(endpoints) = &connector.auth else {
        return Err(SetupError::NotOAuth);
    };
    let client = load_client(store, &connector.id)?.ok_or(SetupError::NoClient)?;
    Ok(OAuthClient::new(OAuthConfig::from_endpoints(
        endpoints,
        &client.client_id,
        client.client_secret,
    )))
}

/// Signs the account `<connector>:<label>` in with only the scopes `features` switch on. A
/// client whose refresh token ends within a week is a project in "Testing": the sign-in is
/// revoked and removed, and the error points at the fix (R-10a).
pub async fn sign_in_account(
    oauth: &OAuthClient,
    connector: &Connector,
    browser: &(dyn Browser + Sync),
    store: &(dyn SecretStore + Sync),
    label: &str,
    features: &[String],
    timeout: Duration,
) -> Result<AccessToken, SetupError> {
    let scopes = consent_scopes(connector, features).map_err(SetupError::Scopes)?;
    let scopes: Vec<&str> = scopes.iter().map(String::as_str).collect();
    let account = format!("{}:{label}", connector.id);
    let signed = oauth
        .sign_in_detailed(browser, store, &account, &scopes, timeout)
        .await?;
    if in_testing(signed.refresh_expires_in) {
        // The token would stop in 7 days; take it back rather than keep a sign-in that fails.
        let _ = oauth.revoke(store, &account).await;
        return Err(SetupError::ClientInTesting {
            fix_url: connector
                .setup
                .as_ref()
                .and_then(|s| s.production_url.clone()),
        });
    }
    Ok(signed.access)
}

/// Disconnects `account` (R-12, AC-6): revokes its token at the provider, removes it from the
/// keychain, and deletes everything kept from it. The data goes even when the provider cannot
/// be reached; the revoke error is returned after, so the caller can say so.
pub async fn disconnect(
    oauth: &OAuthClient,
    store: &(dyn SecretStore + Sync),
    data: &dyn AccountData,
    account: &str,
) -> Result<(), SetupError> {
    let revoked = oauth.revoke(store, account).await;
    data.delete_account(account).map_err(SetupError::Data)?;
    revoked.map_err(SetupError::from)
}
