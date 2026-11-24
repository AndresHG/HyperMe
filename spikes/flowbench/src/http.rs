//! Shared HTTP details: reading keys and turning error responses into messages.

use anyhow::{Context, Result, bail};

/// Reads an API key from the environment. The value is never printed or logged.
pub fn key_from_env(name: &str) -> Result<String> {
    let key = std::env::var(name)
        .ok()
        .filter(|k| !k.trim().is_empty())
        .with_context(|| format!("environment variable {name} is not set"))?;
    Ok(key.trim().to_owned())
}

/// Passes a successful response through; turns any other into an error with the body.
pub async fn ensure_ok(resp: reqwest::Response, key_env: &str) -> Result<reqwest::Response> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    if status == reqwest::StatusCode::UNAUTHORIZED {
        // The body of a 401 can echo part of the key, so it is not shown.
        bail!(
            "401 Unauthorized from {}: check the key in ${key_env}",
            resp.url().host_str().unwrap_or("the API")
        );
    }
    let body = resp.text().await.unwrap_or_default();
    bail!("HTTP {status}: {body}")
}
