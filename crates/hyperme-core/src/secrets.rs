//! Provider keys. They live in the OS keychain only (R-25), reached through [`SecretStore`];
//! the config file never holds one (R-26).

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Mutex;

/// Reads and writes one key per provider. The macOS keychain implements it in
/// `hyperme-platform`; [`MemorySecretStore`] stands in for tests.
pub trait SecretStore {
    /// The key for `provider` (the `provider` name in the config), if one is set.
    fn get(&self, provider: &str) -> Result<Option<String>, SecretError>;
    /// Sets or replaces the key for `provider`.
    fn set(&self, provider: &str, key: &str) -> Result<(), SecretError>;
    /// Removes the key for `provider`. Removing a key that is not there is not an error.
    fn delete(&self, provider: &str) -> Result<(), SecretError>;
}

/// Why the store could not be read or written. Never carries the key itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretError(pub String);

impl fmt::Display for SecretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "keychain: {}", self.0)
    }
}

impl std::error::Error for SecretError {}

/// An in-memory [`SecretStore`] for tests. Its `Debug` output names providers, never keys.
#[derive(Default)]
pub struct MemorySecretStore {
    keys: Mutex<BTreeMap<String, String>>,
}

impl MemorySecretStore {
    fn keys(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, String>> {
        // A panic while holding the lock leaves the map intact, so a poisoned lock is usable.
        self.keys
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl SecretStore for MemorySecretStore {
    fn get(&self, provider: &str) -> Result<Option<String>, SecretError> {
        Ok(self.keys().get(provider).cloned())
    }

    fn set(&self, provider: &str, key: &str) -> Result<(), SecretError> {
        self.keys().insert(provider.to_owned(), key.to_owned());
        Ok(())
    }

    fn delete(&self, provider: &str) -> Result<(), SecretError> {
        self.keys().remove(provider);
        Ok(())
    }
}

impl fmt::Debug for MemorySecretStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemorySecretStore")
            .field("providers", &self.keys().keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R-25: debug output, which can end up in a log, never shows a key.
    #[test]
    fn debug_output_hides_keys() {
        let store = MemorySecretStore::default();
        store.set("groq", "fake-key-dddd").unwrap();
        let shown = format!("{store:?}");
        assert!(shown.contains("groq"), "{shown}");
        assert!(!shown.contains("fake-key-dddd"), "{shown}");
    }
}
