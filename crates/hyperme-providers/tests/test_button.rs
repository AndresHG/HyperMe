//! Provider settings, slice 5: Test checks the key and the chosen model together
//! (`specs/96-provider-settings/spec.md`, R-13). Tests cite AC-6.

mod chat_server;

use chat_server::*;
use hyperme_core::config::Config;
use hyperme_core::traits::{Error, ErrorKind};
use hyperme_providers::cleanup::ChatCleanup;

const KEY: &str = "sk-not-a-real-key-for-tests";

fn cleanup(base: &str, model: &str) -> ChatCleanup {
    let mut config = Config::default().cleanup;
    config.endpoint = base.to_owned();
    config.model = model.to_owned();
    ChatCleanup::new(&config, KEY).unwrap()
}

fn assert_no_key(error: &Error) {
    assert!(
        !error.message.contains(KEY),
        "the key leaked: {}",
        error.message
    );
    assert!(
        !format!("{error:?}").contains(KEY),
        "the key leaked: {error:?}"
    );
}

/// AC-6: a good key and model work, and the check asks the chosen model for a completion, not
/// only the model list.
#[tokio::test]
async fn ac6_a_good_key_and_model_work() {
    let (base, requests) = serve(models(), streamed(&["Test."])).await;
    cleanup(&base, "fast-1").check().await.unwrap();
    let seen = requests.lock().unwrap();
    let chat = seen
        .iter()
        .find(|r| r.starts_with("POST /v1/chat/completions"))
        .expect("Test sent no completion");
    assert!(chat.contains("\"model\":\"fast-1\""), "{chat}");
}

/// AC-6: a 401 says the key was rejected, without the key, even when the body echoes it.
#[tokio::test]
async fn ac6_a_rejected_key_says_so() {
    let body = format!(r#"{{"error":{{"message":"Incorrect API key provided: {KEY}"}}}}"#);
    let refused = answer("401 Unauthorized", "application/json", &body);
    let (base, _) = serve(refused.clone(), refused).await;
    let error = cleanup(&base, "fast-1").check().await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::Auth);
    assert!(
        error.message.contains("key was rejected"),
        "{}",
        error.message
    );
    assert_no_key(&error);
}

/// AC-6: an unknown model says "model not found", in the shapes providers use for it.
#[tokio::test]
async fn ac6_an_unknown_model_says_model_not_found() {
    for (status, body) in [
        (
            "404 Not Found",
            r#"{"error":{"message":"The model `nope-9` does not exist or you do not have access to it.","type":"invalid_request_error","code":"model_not_found"}}"#,
        ),
        (
            "400 Bad Request",
            r#"{"error":{"message":"model not found: nope-9","code":"model_not_found"}}"#,
        ),
        (
            "404 Not Found",
            r#"{"error":{"message":"models/nope-9 is not found for API version v1beta"}}"#,
        ),
    ] {
        let (base, _) = serve(models(), answer(status, "application/json", body)).await;
        let error = cleanup(&base, "nope-9").check().await.unwrap_err();
        assert!(
            error.message.contains("model nope-9 was not found"),
            "{status}: {}",
            error.message
        );
        assert_no_key(&error);
    }
}

/// R-13: no network says so.
#[tokio::test]
async fn no_network_says_so() {
    let error = cleanup("http://127.0.0.1:1/v1", "fast-1")
        .check()
        .await
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Network);
    assert!(
        error.message.contains("could not reach"),
        "{}",
        error.message
    );
    assert_no_key(&error);
}
