//! Provider keys in the macOS keychain (R-25): one generic password per provider, under the
//! service "HyperMe" and the account named by the provider in the config. Nothing here logs or
//! returns a key except `get`.

use std::fmt;

use hyperme_core::secrets::{SecretError, SecretStore};
use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};

/// `errSecItemNotFound`.
const ITEM_NOT_FOUND: i32 = -25300;

/// The keychain, as a [`SecretStore`].
#[derive(Clone)]
pub struct KeychainStore {
    service: String,
}

impl fmt::Debug for KeychainStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeychainStore")
            .field("service", &self.service)
            .finish()
    }
}

impl Default for KeychainStore {
    fn default() -> Self {
        Self::with_service("HyperMe")
    }
}

impl KeychainStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// A store under another service name, so tests never touch the app's entries.
    pub fn with_service(service: &str) -> Self {
        KeychainStore {
            service: service.to_owned(),
        }
    }
}

/// The keychain's own message for an error code. It never contains the key.
fn keychain_error(
    what: &str,
    provider: &str,
    error: security_framework::base::Error,
) -> SecretError {
    let reason = error
        .message()
        .unwrap_or_else(|| format!("error {}", error.code()));
    SecretError(format!("could not {what} the {provider} key: {reason}"))
}

impl SecretStore for KeychainStore {
    fn get(&self, provider: &str) -> Result<Option<String>, SecretError> {
        match get_generic_password(&self.service, provider) {
            Ok(bytes) => String::from_utf8(bytes).map(Some).map_err(|_| {
                SecretError(format!("the {provider} key in the keychain is not text"))
            }),
            Err(error) if error.code() == ITEM_NOT_FOUND => Ok(None),
            Err(error) => Err(keychain_error("read", provider, error)),
        }
    }

    fn set(&self, provider: &str, key: &str) -> Result<(), SecretError> {
        let key = key.trim();
        if key.is_empty() {
            return Err(SecretError(format!("the {provider} key is empty")));
        }
        set_generic_password(&self.service, provider, key.as_bytes())
            .map_err(|error| keychain_error("save", provider, error))
    }

    fn delete(&self, provider: &str) -> Result<(), SecretError> {
        match delete_generic_password(&self.service, provider) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == ITEM_NOT_FOUND => Ok(()),
            Err(error) => Err(keychain_error("delete", provider, error)),
        }
    }
}
