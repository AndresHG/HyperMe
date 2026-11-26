//! Slice 9 (keychain half): the macOS keychain behind `SecretStore` (R-25, AC-18). The live
//! tests write to the login keychain under a test service name and clean up after themselves;
//! they are ignored by default so `cargo test` never touches the user's keychain. Run them with
//! `cargo test -p hyperme-platform --test keychain -- --ignored`.

#![cfg(target_os = "macos")]

use hyperme_core::secrets::SecretStore;
use hyperme_platform::keychain::KeychainStore;

const FAKE: &str = "fake-key-eeee";

fn store() -> KeychainStore {
    KeychainStore::with_service("HyperMe tests")
}

fn provider(name: &str) -> String {
    format!("{name}-{}", std::process::id())
}

/// R-25: an empty or blank key is refused rather than stored.
#[test]
fn blank_keys_are_refused() {
    let error = store().set("groq", "  ").unwrap_err();
    assert!(error.to_string().contains("empty"), "{error}");
}

/// R-25: the store's debug output names its service, never a key.
#[test]
fn debug_output_names_only_the_service() {
    assert_eq!(
        format!("{:?}", store()),
        r#"KeychainStore { service: "HyperMe tests" }"#
    );
}

/// AC-18: keys are written to and read from the keychain through `SecretStore`; replacing and
/// deleting work, and a missing key is `None`, not an error.
#[test]
#[ignore = "writes to the login keychain"]
fn keys_round_trip_through_the_keychain() {
    let store = store();
    let provider = provider("roundtrip");
    assert_eq!(store.get(&provider).unwrap(), None);
    store.set(&provider, FAKE).unwrap();
    assert_eq!(store.get(&provider).unwrap().as_deref(), Some(FAKE));
    store.set(&provider, " fake-key-ffff \n").unwrap();
    assert_eq!(
        store.get(&provider).unwrap().as_deref(),
        Some("fake-key-ffff"),
        "keys are trimmed"
    );
    store.delete(&provider).unwrap();
    assert_eq!(store.get(&provider).unwrap(), None);
    store.delete(&provider).unwrap();
}
