//! Spec 182, AC-9: the developer exports. Run with `--features otel,trace-chrome`; without them
//! only the "nothing is exported" test runs.

use std::collections::HashMap;

use hyperme_devtrace::Exports;

fn vars(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    move |name| map.get(name).cloned()
}

/// R-15, R-16: with no variable set, nothing is exported, whatever was compiled in.
#[test]
fn ac9_no_variable_no_export() {
    let exports = Exports::from_vars(vars(&[]));
    assert!(exports.is_empty());
}

#[cfg(feature = "trace-chrome")]
mod chrome {
    use super::*;
    use std::path::Path;
    use tracing_subscriber::layer::SubscriberExt;

    /// R-15: `HYPERME_TRACE_CHROME` writes a Perfetto-readable JSON trace of the session.
    #[test]
    fn ac9_chrome_file_holds_the_spans() {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("devtrace-chrome");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session.json");
        let _ = std::fs::remove_file(&path);
        let mut exports =
            Exports::from_vars(vars(&[("HYPERME_TRACE_CHROME", path.to_str().unwrap())]));
        assert!(!exports.is_empty());
        let subscriber = tracing_subscriber::registry().with(exports.take_layers());
        tracing::subscriber::with_default(subscriber, || {
            let _span = tracing::info_span!("dictation").entered();
        });
        exports.shutdown();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.trim_start().starts_with('['), "{text}");
        assert!(text.contains("\"dictation\""), "{text}");
    }
}

#[cfg(feature = "otel")]
mod otel {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;

    use hyperme_core::trace;
    use hyperme_core::warnings::{LineSink, WarningsLayer};

    #[derive(Default)]
    struct Lines(Mutex<Vec<String>>);

    impl LineSink for Lines {
        fn write_line(&self, line: &str) {
            self.0.lock().unwrap().push(line.to_owned());
        }
    }

    struct Request {
        path: String,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    }

    /// A local OTLP/HTTP receiver that answers 200 to every request and keeps them.
    fn receiver() -> (String, Arc<Mutex<Vec<Request>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let keep = seen.clone();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let path = line.split_whitespace().nth(1).unwrap_or("").to_owned();
                let mut headers = Vec::new();
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    let header = header.trim_end();
                    if header.is_empty() {
                        break;
                    }
                    if let Some((name, value)) = header.split_once(':') {
                        headers.push((name.trim().to_lowercase(), value.trim().to_owned()));
                    }
                }
                let length = headers
                    .iter()
                    .find(|(n, _)| n == "content-length")
                    .and_then(|(_, v)| v.parse().ok())
                    .unwrap_or(0);
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                keep.lock().unwrap().push(Request {
                    path,
                    headers,
                    body,
                });
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                );
            }
        });
        (address, seen)
    }

    /// R-16: spans arrive as OTLP/HTTP protobuf at `<endpoint>/v1/traces`, with the configured
    /// headers; R-16 and R-14: the headers appear in no log line.
    #[test]
    fn ac9_otlp_spans_arrive_with_headers_and_no_log_holds_them() {
        let (endpoint, seen) = receiver();
        let mut exports = Exports::from_vars(vars(&[
            ("HYPERME_OTLP_ENDPOINT", &endpoint),
            (
                "HYPERME_OTLP_HEADERS",
                "x-test-team=sekret-team-key,x-other=two",
            ),
        ]));
        assert!(!exports.is_empty());
        let (file, echo) = (Arc::new(Lines::default()), Arc::new(Lines::default()));
        let warnings = WarningsLayer::new(file.clone(), None).echo(echo.clone());
        let dispatch = trace::subscriber_layers(Vec::new(), warnings, exports.take_layers());
        tracing::dispatcher::with_default(&dispatch, || {
            let _span = tracing::info_span!("dictation", outcome = "pasted").entered();
            tracing::warn!(target: "hyperme::test", "a warning while exporting");
        });
        exports.shutdown();

        let seen = seen.lock().unwrap();
        let request = seen.first().expect("no OTLP request arrived");
        assert_eq!(request.path, "/v1/traces");
        let header = |name: &str| {
            request
                .headers
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(header("x-test-team"), Some("sekret-team-key"));
        assert_eq!(header("x-other"), Some("two"));
        assert_eq!(header("content-type"), Some("application/x-protobuf"));
        assert!(
            request.body.windows(9).any(|w| w == b"dictation"),
            "span name not in the body"
        );
        let logged = [
            file.0.lock().unwrap().clone(),
            echo.0.lock().unwrap().clone(),
        ]
        .concat();
        assert!(!logged.is_empty());
        assert!(logged.iter().all(|l| !l.contains("sekret")), "{logged:?}");
    }
}
