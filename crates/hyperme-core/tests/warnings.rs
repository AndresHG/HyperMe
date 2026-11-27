//! Spec 182, R-14: the warnings and errors file, with the no-text rule of R-5. The events name
//! a `hyperme` target, as the app's own modules do.

use std::sync::{Arc, Mutex};

use hyperme_core::trace::{self, TraceSink};
use hyperme_core::warnings::{LineSink, WarningsLayer};
use tracing::dispatcher;

#[derive(Default)]
struct Lines(Mutex<Vec<String>>);

impl LineSink for Lines {
    fn write_line(&self, line: &str) {
        self.0.lock().unwrap().push(line.to_owned());
    }
}

impl Lines {
    fn all(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

fn run(file: &Arc<Lines>, echo: &Arc<Lines>, body: impl FnOnce()) {
    let layer =
        WarningsLayer::new(file.clone(), Some("/Users/someone".to_owned())).echo(echo.clone());
    let sinks: Vec<Arc<dyn TraceSink>> = Vec::new();
    dispatcher::with_default(&trace::subscriber_with(sinks, layer), body);
}

/// AC-3 (warnings file): warnings and errors from HyperMe reach the file without an email
/// address, a home-folder path or anything shaped like a key.
#[test]
fn ac3_warnings_file_holds_no_personal_text() {
    let (file, echo) = (Arc::new(Lines::default()), Arc::new(Lines::default()));
    run(&file, &echo, || {
        tracing::warn!(
            target: "hyperme::test",
            "sign-in failed for someone@example.com: /Users/someone/Library/HyperMe/config.toml"
        );
        tracing::error!(target: "hyperme::test", account = "someone@example.com", "token sk-proj-a1B2c3D4e5F6g7H8i9J0 refused");
    });
    let lines = file.all();
    assert_eq!(lines.len(), 2, "{lines:?}");
    let text = lines.join("\n");
    for secret in [
        "someone@example.com",
        "/Users/someone",
        "a1B2c3D4e5F6g7H8i9J0",
    ] {
        assert!(!text.contains(secret), "{secret} reached the file: {text}");
    }
    assert!(lines[0].contains("\tWARN\t"), "{}", lines[0]);
    assert!(lines[0].contains("sign-in failed for <email>: ~/Library/HyperMe/config.toml"));
    assert!(lines[1].contains("\tERROR\t"), "{}", lines[1]);
    assert!(lines[1].contains("token <secret> refused"), "{}", lines[1]);
    assert!(lines[1].contains("account=<email>"), "{}", lines[1]);
}

/// R-14: only warnings and errors go to the file; information is echoed to standard error
/// for a terminal run, and so are warnings.
#[test]
fn r14_information_is_echoed_not_kept() {
    let (file, echo) = (Arc::new(Lines::default()), Arc::new(Lines::default()));
    run(&file, &echo, || {
        tracing::info!(target: "hyperme::test", "Listening");
        tracing::debug!(target: "hyperme::test", "noise");
        tracing::warn!(target: "hyperme::test", "providers unchanged: no key");
    });
    assert_eq!(file.all().len(), 1);
    assert_eq!(
        echo.all(),
        ["HyperMe: Listening", "HyperMe: providers unchanged: no key"]
    );
}

/// R-14: events from other crates (an MCP client, an HTTP stack) never reach the file, since
/// their messages may carry connector content.
#[test]
fn r14_only_hypermes_own_events_are_kept() {
    let (file, echo) = (Arc::new(Lines::default()), Arc::new(Lines::default()));
    run(&file, &echo, || {
        tracing::warn!(target: "rmcp::service", "tool result: Dear Ana, about the contract");
    });
    assert!(file.all().is_empty());
    assert!(echo.all().is_empty());
}

/// Spec 166, AC-12 and R-20: rmcp logs whole requests and results, which can hold a server's
/// approval message, title and state; none of its events reach the warnings file, at any level.
#[test]
fn rmcp_events_never_reach_the_file() {
    let (file, echo) = (Arc::new(Lines::default()), Arc::new(Lines::default()));
    let secret = r#"approval_choice "Allow HyperMe to read 3 threads" requestState {"b":"b-1"}"#;
    run(&file, &echo, || {
        tracing::debug!(target: "rmcp::service", "{secret}");
        tracing::info!(target: "rmcp::service", "{secret}");
        tracing::warn!(target: "rmcp::service", "{secret}");
        tracing::error!(target: "rmcp::transport::streamable_http_client", "{secret}");
    });
    let text = [file.all(), echo.all()].concat().join("\n");
    assert!(!text.contains("3 threads"), "{text}");
    assert!(!text.contains("b-1"), "{text}");
}
