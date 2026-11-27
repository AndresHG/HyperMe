//! The warnings and errors file (specs/182-trace-store, R-14).
//!
//! HyperMe's own `warn!` and `error!` events become one line each in a local file, after
//! [`scrub`] has taken out a home-folder path, an email address and anything shaped like a key.
//! Events from other crates are dropped, since an MCP client or HTTP stack may log connector
//! content. Information and above is also echoed to standard error, for a terminal run.

use std::fmt::{self, Write as _};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

/// Where finished lines go: a rotating file in the app, a list in tests.
pub trait LineSink: Send + Sync {
    /// Writes one line, without its newline. Never fails: a lost line is better than a lost
    /// dictation (R-11).
    fn write_line(&self, line: &str);
}

/// Standard error, for running HyperMe from a terminal.
#[derive(Debug)]
pub struct Stderr;

impl LineSink for Stderr {
    fn write_line(&self, line: &str) {
        eprintln!("{line}");
    }
}

/// The layer that writes HyperMe's warnings and errors to a [`LineSink`].
pub struct WarningsLayer {
    file: Arc<dyn LineSink>,
    echo: Option<Arc<dyn LineSink>>,
    home: Option<String>,
}

impl fmt::Debug for WarningsLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WarningsLayer")
            .field("echo", &self.echo.is_some())
            .finish_non_exhaustive()
    }
}

impl WarningsLayer {
    /// Warnings and errors go to `file`; `home` is the home folder that [`scrub`] replaces
    /// with `~`.
    pub fn new(file: Arc<dyn LineSink>, home: Option<String>) -> Self {
        Self {
            file,
            echo: None,
            home: home.filter(|h| h.len() > 1),
        }
    }

    /// Also writes information, warnings and errors, unscrubbed, to `echo` as
    /// `HyperMe: <message>`.
    pub fn echo(mut self, echo: Arc<dyn LineSink>) -> Self {
        self.echo = Some(echo);
        self
    }
}

impl<S: Subscriber> Layer<S> for WarningsLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        if !meta.target().starts_with("hyperme") || *meta.level() > Level::INFO {
            return;
        }
        let mut text = Text::default();
        event.record(&mut text);
        if let Some(echo) = &self.echo {
            echo.write_line(&format!("HyperMe: {}", text.0));
        }
        if *meta.level() > Level::WARN {
            return;
        }
        let at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        self.file.write_line(&format!(
            "{at}\t{}\t{}\t{}",
            meta.level(),
            meta.target(),
            scrub(&text.0, self.home.as_deref())
        ));
    }
}

/// An event's message followed by its other fields as `name=value`.
#[derive(Default)]
struct Text(String);

impl Visit for Text {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_debug(field, &format_args!("{value}"));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            let rest = std::mem::take(&mut self.0);
            let _ = write!(self.0, "{value:?}");
            if !rest.is_empty() {
                self.0.push_str(&rest);
            }
        } else {
            let _ = write!(self.0, " {}={value:?}", field.name());
        }
    }
}

/// Takes personal text out of a line (R-14 with R-5): the home folder becomes `~`, a word with
/// an `@` and a dot becomes `<email>`, and a long run of letters and digits becomes `<secret>`.
pub fn scrub(text: &str, home: Option<&str>) -> String {
    let text = match home {
        Some(home) => text.replace(home, "~"),
        None => text.to_owned(),
    };
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    for c in text.chars() {
        if is_word(c) {
            word.push(c);
        } else {
            out.push_str(&mask(&word));
            word.clear();
            out.push(c);
        }
    }
    out.push_str(&mask(&word));
    out
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '@' | '.' | '-' | '_' | '+')
}

fn mask(word: &str) -> String {
    let trimmed = word.trim_end_matches('.');
    let tail = &word[trimmed.len()..];
    if let Some((user, domain)) = trimmed.split_once('@')
        && !user.is_empty()
        && domain.contains('.')
    {
        return format!("<email>{tail}");
    }
    let has_digit = trimmed.chars().any(|c| c.is_ascii_digit());
    let has_letter = trimmed.chars().any(|c| c.is_alphabetic());
    let key_like = trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'));
    if trimmed.len() >= 20 && has_digit && has_letter && key_like {
        return format!("<secret>{tail}");
    }
    word.to_owned()
}
