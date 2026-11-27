//! Local traces (specs/182-trace-store): the span names and fields a dictation records, the
//! layer that turns closed spans into one [`FinishedTrace`] per dictation, and the timing log
//! rebuilt on those traces (spec 25 R-30, spec 182 R-12).
//!
//! The core only creates spans; where they go is chosen when the app installs its subscriber.
//! Fields are declared in [`SCHEMA`]: the layer keeps those and drops anything else, so text can
//! never reach a trace (spec 182 R-5).

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use tokio::time::Instant;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Span, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::traits::{TimingLine, TimingLog};

/// Runs a future (such as [`crate::dictation::Engine::run`]) with a given subscriber, for tools
/// and tests that do not install a global one.
pub use tracing::instrument::WithSubscriber;

/// A subscriber that collects traces into `sinks`: what the app installs, and what tools and
/// tests run the engine with.
pub fn subscriber(sinks: Vec<Arc<dyn TraceSink>>) -> tracing::Dispatch {
    use tracing_subscriber::layer::SubscriberExt;
    tracing::Dispatch::new(tracing_subscriber::registry().with(TraceLayer::new(sinks)))
}

/// [`subscriber`] with the warnings and errors file (R-14) as well.
pub fn subscriber_with(
    sinks: Vec<Arc<dyn TraceSink>>,
    warnings: crate::warnings::WarningsLayer,
) -> tracing::Dispatch {
    use tracing_subscriber::layer::SubscriberExt;
    tracing::Dispatch::new(
        tracing_subscriber::registry()
            .with(TraceLayer::new(sinks))
            .with(warnings),
    )
}

/// Installs [`subscriber_with`] for the whole process, once, as the app does at start-up.
/// Returns `false` if a subscriber was already installed.
pub fn install_with(
    sinks: Vec<Arc<dyn TraceSink>>,
    warnings: crate::warnings::WarningsLayer,
) -> bool {
    tracing::dispatcher::set_global_default(subscriber_with(sinks, warnings)).is_ok()
}

/// Installs [`subscriber`] for the whole process, once, as the app does at start-up. Returns
/// `false` if a subscriber was already installed.
pub fn install(sinks: Vec<Arc<dyn TraceSink>>) -> bool {
    tracing::dispatcher::set_global_default(subscriber(sinks)).is_ok()
}

/// The root span of one dictation or retry.
pub const DICTATION: &str = "dictation";
/// Key-down to the first captured sample.
pub const MIC_START: &str = "mic_start";
/// Key-down to key release (or the maximum length).
pub const CAPTURE: &str = "capture";
/// Key release (or the retry's start) to the final transcript.
pub const SPEECH: &str = "speech";
/// The dictionary pass over the transcript.
pub const DICTIONARY: &str = "dictionary";
/// The cleanup call, with a `first_token` event.
pub const CLEANUP: &str = "cleanup";
/// The text ready to the paste (or copy) sent.
pub const INSERT: &str = "insert";
/// The event on [`CLEANUP`] when its first cleaned token arrives.
pub const FIRST_TOKEN: &str = "first_token";

/// What a declared field may hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    U64,
    Bool,
    /// A short name: an enum value, a language code, or a provider or model name. Never text.
    Name,
}

/// Every span the layer keeps, with the only fields it keeps for each (spec 182 R-3, R-4, R-5).
pub const SCHEMA: &[(&str, &[(&str, Kind)])] = &[
    (
        DICTATION,
        &[
            ("history_id", Kind::U64),
            ("retry_of", Kind::U64),
            ("outcome", Kind::Name),
            ("error_kind", Kind::Name),
            ("language", Kind::Name),
            ("words", Kind::U64),
            ("fallback", Kind::Bool),
            ("speech_provider", Kind::Name),
            ("speech_model", Kind::Name),
            ("cleanup_provider", Kind::Name),
            ("cleanup_model", Kind::Name),
            ("cleanup_level", Kind::Name),
        ],
    ),
    (MIC_START, &[("no_audio", Kind::Bool)]),
    (CAPTURE, &[]),
    (
        SPEECH,
        &[("audio_bytes", Kind::U64), ("attempts", Kind::U64)],
    ),
    (DICTIONARY, &[("entries_applied", Kind::U64)]),
    (
        CLEANUP,
        &[
            ("tokens_in", Kind::U64),
            ("tokens_out", Kind::U64),
            ("attempts", Kind::U64),
        ],
    ),
    (INSERT, &[]),
];

/// Events the layer keeps, by name, on the span they belong to.
const EVENTS: &[&str] = &[FIRST_TOKEN];

/// The longest [`Kind::Name`] value kept.
const NAME_MAX: usize = 64;

fn declared(span: &str) -> Option<&'static [(&'static str, Kind)]> {
    SCHEMA
        .iter()
        .find(|(name, _)| *name == span)
        .map(|(_, fields)| *fields)
}

/// One field value in a finished trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldValue {
    U64(u64),
    Bool(bool),
    Name(String),
}

/// One closed span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishedSpan {
    pub name: &'static str,
    /// Index of the parent in [`FinishedTrace::spans`]; `None` for the root.
    pub parent: Option<usize>,
    /// Microseconds from the root's start.
    pub start_us: u64,
    pub duration_us: u64,
    pub fields: Vec<(&'static str, FieldValue)>,
    /// Event name and its offset in microseconds from the root's start.
    pub events: Vec<(&'static str, u64)>,
}

impl FinishedSpan {
    pub fn end_us(&self) -> u64 {
        self.start_us + self.duration_us
    }

    pub fn field(&self, name: &str) -> Option<&FieldValue> {
        self.fields.iter().find(|(n, _)| *n == name).map(|(_, v)| v)
    }

    pub fn u64(&self, name: &str) -> Option<u64> {
        match self.field(name) {
            Some(FieldValue::U64(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn bool(&self, name: &str) -> Option<bool> {
        match self.field(name) {
            Some(FieldValue::Bool(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn name(&self, name: &str) -> Option<&str> {
        match self.field(name) {
            Some(FieldValue::Name(v)) => Some(v),
            _ => None,
        }
    }

    pub fn event_us(&self, name: &str) -> Option<u64> {
        self.events
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, at)| *at)
    }
}

/// One dictation's trace: the root first, then its descendants by start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishedTrace {
    pub spans: Vec<FinishedSpan>,
}

impl FinishedTrace {
    pub fn root(&self) -> &FinishedSpan {
        &self.spans[0]
    }

    /// The first span with this name.
    pub fn span(&self, name: &str) -> Option<&FinishedSpan> {
        self.spans.iter().find(|s| s.name == name)
    }
}

/// Where finished traces go: the timing log, and later the trace store. Called on the thread
/// that closed the root, so it must return quickly (spec 182 R-7, R-11).
pub trait TraceSink: Send + Sync + 'static {
    fn finished(&self, trace: &FinishedTrace);
}

/// A span the layer is following, kept in the registry's extensions.
struct Open {
    name: &'static str,
    start: Instant,
    fields: Vec<(&'static str, FieldValue)>,
    events: Vec<(&'static str, Instant)>,
}

/// The spans of a trace closed so far, kept on the root.
#[derive(Default)]
struct Closed {
    spans: Vec<(u64, Option<u64>, Open, Instant)>,
}

/// Keeps only declared fields of the declared kind.
struct Keep<'a> {
    declared: &'static [(&'static str, Kind)],
    fields: &'a mut Vec<(&'static str, FieldValue)>,
}

impl Keep<'_> {
    fn put(&mut self, field: &Field, value: FieldValue) {
        let Some((name, kind)) = self.declared.iter().find(|(n, _)| *n == field.name()) else {
            return;
        };
        let fits = match (&value, kind) {
            (FieldValue::U64(_), Kind::U64) | (FieldValue::Bool(_), Kind::Bool) => true,
            (FieldValue::Name(s), Kind::Name) => {
                s.len() <= NAME_MAX && !s.chars().any(|c| c.is_control())
            }
            _ => false,
        };
        if !fits {
            return;
        }
        self.fields.retain(|(n, _)| n != name);
        self.fields.push((name, value));
    }
}

impl Visit for Keep<'_> {
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.put(field, FieldValue::U64(value));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        if let Ok(value) = u64::try_from(value) {
            self.put(field, FieldValue::U64(value));
        }
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.put(field, FieldValue::Bool(value));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.put(field, FieldValue::Name(value.to_owned()));
    }

    // Debug values (messages, structs) are never kept: they could hold anything.
    fn record_debug(&mut self, _field: &Field, _value: &dyn fmt::Debug) {}
}

/// Collects the spans of each dictation into a [`FinishedTrace`] and hands it to the sinks when
/// the root closes. Spans not in [`SCHEMA`] are ignored.
pub struct TraceLayer {
    sinks: Vec<Arc<dyn TraceSink>>,
}

impl fmt::Debug for TraceLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TraceLayer")
            .field("sinks", &self.sinks.len())
            .finish()
    }
}

impl TraceLayer {
    pub fn new(sinks: Vec<Arc<dyn TraceSink>>) -> Self {
        TraceLayer { sinks }
    }
}

impl<S> Layer<S> for TraceLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let name = attrs.metadata().name();
        let Some(fields) = declared(name) else {
            return;
        };
        let Some(span) = ctx.span(id) else {
            return;
        };
        let mut open = Open {
            name,
            start: Instant::now(),
            fields: Vec::new(),
            events: Vec::new(),
        };
        attrs.record(&mut Keep {
            declared: fields,
            fields: &mut open.fields,
        });
        let mut extensions = span.extensions_mut();
        if name == DICTATION {
            extensions.insert(Closed::default());
        }
        extensions.insert(open);
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };
        let mut extensions = span.extensions_mut();
        if let Some(open) = extensions.get_mut::<Open>() {
            let Some(fields) = declared(open.name) else {
                return;
            };
            values.record(&mut Keep {
                declared: fields,
                fields: &mut open.fields,
            });
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let name = event.metadata().name();
        let Some(name) = EVENTS.iter().find(|n| **n == name) else {
            return;
        };
        let Some(span) = ctx.event_span(event) else {
            return;
        };
        if let Some(open) = span.extensions_mut().get_mut::<Open>() {
            open.events.push((name, Instant::now()));
        }
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let end = Instant::now();
        let Some(span) = ctx.span(&id) else {
            return;
        };
        let Some(open) = span.extensions_mut().remove::<Open>() else {
            return;
        };
        // The nearest followed ancestor, and the dictation this span belongs to.
        let parent = span
            .scope()
            .skip(1)
            .find(|s| s.extensions().get::<Open>().is_some())
            .map(|s| s.id().into_u64());
        let root = span
            .scope()
            .find(|s| s.extensions().get::<Closed>().is_some());
        let Some(root) = root else {
            return;
        };
        if root.id() != id {
            if let Some(closed) = root.extensions_mut().get_mut::<Closed>() {
                closed.spans.push((id.into_u64(), parent, open, end));
            }
            return;
        }
        let Some(closed) = span.extensions_mut().remove::<Closed>() else {
            return;
        };
        let trace = assemble(id.into_u64(), open, end, closed);
        for sink in &self.sinks {
            sink.finished(&trace);
        }
    }
}

fn assemble(root_id: u64, root: Open, root_end: Instant, closed: Closed) -> FinishedTrace {
    let base = root.start;
    let us = |at: Instant| at.saturating_duration_since(base).as_micros() as u64;
    let mut children = closed.spans;
    children.sort_by_key(|(_, _, open, _)| open.start);
    let ids: Vec<u64> = std::iter::once(root_id)
        .chain(children.iter().map(|(id, ..)| *id))
        .collect();
    let finish = |open: Open, parent: Option<usize>, end: Instant| FinishedSpan {
        name: open.name,
        parent,
        start_us: us(open.start),
        duration_us: end.saturating_duration_since(open.start).as_micros() as u64,
        fields: open.fields,
        events: open.events.into_iter().map(|(n, at)| (n, us(at))).collect(),
    };
    let mut spans = vec![finish(root, None, root_end)];
    for (_, parent, open, end) in children {
        let parent = parent.and_then(|p| ids.iter().position(|id| *id == p));
        spans.push(finish(open, parent.or(Some(0)), end));
    }
    FinishedTrace { spans }
}

impl TimingLine {
    /// The timing log's line for a pasted dictation (spec 182 R-12), with the same columns and
    /// definitions as before traces: `None` for anything that did not paste, and for retries,
    /// which have no capture.
    pub fn from_trace(trace: &FinishedTrace) -> Option<TimingLine> {
        let root = trace.root();
        if root.name != DICTATION || root.name("outcome") != Some("pasted") {
            return None;
        }
        let ms = |us: u64| us / 1000;
        let release = trace.span(CAPTURE)?.end_us();
        let after = |at: u64| ms(at.saturating_sub(release));
        let insert = trace.span(INSERT)?;
        let first_audio = trace
            .span(MIC_START)
            .filter(|s| s.bool("no_audio") != Some(true))
            .map(|s| ms(s.end_us()));
        Some(TimingLine {
            first_audio_ms: first_audio,
            release_ms: ms(release),
            stt_ms: after(trace.span(SPEECH)?.end_us()),
            first_ms: trace
                .span(CLEANUP)
                .and_then(|s| s.event_us(FIRST_TOKEN))
                .map(after),
            total_ms: after(insert.start_us),
            paste_ms: after(insert.end_us()),
            language: root.name("language").map(str::to_owned),
            words: root.u64("words").unwrap_or(0) as usize,
            fallback: root.bool("fallback").unwrap_or(false),
        })
    }
}

/// Writes the timing line of each pasted dictation while the timing switch is on (spec 25 R-30).
#[derive(Debug)]
pub struct TimingSink<T> {
    on: Arc<AtomicBool>,
    log: Mutex<T>,
}

impl<T: TimingLog> TimingSink<T> {
    /// `on` is shared with Settings, so turning timing on or off takes effect at once.
    pub fn new(on: Arc<AtomicBool>, log: T) -> Self {
        TimingSink {
            on,
            log: Mutex::new(log),
        }
    }
}

impl<T: TimingLog + Send + 'static> TraceSink for TimingSink<T> {
    fn finished(&self, trace: &FinishedTrace) {
        if !self.on.load(Ordering::Relaxed) {
            return;
        }
        if let Some(line) = TimingLine::from_trace(trace) {
            self.log
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .record(&line);
        }
    }
}

/// How a dictation ended, recorded on its root span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Pasted,
    Copied,
    Failed,
    Cancelled,
    NothingHeard,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Outcome::Pasted => "pasted",
            Outcome::Copied => "copied",
            Outcome::Failed => "failed",
            Outcome::Cancelled => "cancelled",
            Outcome::NothingHeard => "nothing_heard",
        }
    }
}

/// The spans of one dictation, opened and closed by the engine at each stage. Dropping it closes
/// whatever is still open; a dictation that never set its outcome is recorded as cancelled (the
/// app quit, or an early return).
#[derive(Debug)]
pub struct DictationTrace {
    mic_start: Option<Span>,
    capture: Option<Span>,
    speech: Option<Span>,
    cleanup: Option<Span>,
    insert: Option<Span>,
    outcome: Option<Outcome>,
    // Last, so every child closes before it.
    root: Span,
}

impl DictationTrace {
    /// A dictation from key-down: the root, `mic_start` and `capture` start now.
    pub fn dictation() -> Self {
        let root = new_root();
        DictationTrace {
            mic_start: Some(
                tracing::info_span!(parent: &root, "mic_start", no_audio = tracing::field::Empty),
            ),
            capture: Some(tracing::info_span!(parent: &root, "capture")),
            speech: None,
            cleanup: None,
            insert: None,
            outcome: None,
            root,
        }
    }

    /// A retry of a saved dictation: the root and `speech` start now.
    pub fn retry(of: Option<i64>) -> Self {
        let root = new_root();
        if let Some(of) = of {
            root.record("retry_of", of);
        }
        let mut trace = DictationTrace {
            mic_start: None,
            capture: None,
            speech: None,
            cleanup: None,
            insert: None,
            outcome: None,
            root,
        };
        trace.speech = Some(trace.speech_span());
        trace
    }

    fn speech_span(&self) -> Span {
        tracing::info_span!(
            parent: &self.root,
            "speech",
            audio_bytes = tracing::field::Empty,
            attempts = tracing::field::Empty
        )
    }

    pub fn history_id(&self, id: i64) {
        self.root.record("history_id", id);
    }

    /// The first captured sample.
    pub fn first_audio(&mut self) {
        self.mic_start = None;
    }

    /// Key release: capture ends and speech starts.
    pub fn released(&mut self) {
        if let Some(mic) = self.mic_start.take() {
            mic.record("no_audio", true);
        }
        self.capture = None;
        self.speech = Some(self.speech_span());
    }

    /// The final transcript.
    pub fn transcript(&mut self, language: Option<&str>) {
        self.speech = None;
        if let Some(language) = language {
            self.root.record("language", language);
        }
    }

    /// The dictionary pass, already done, with how many entries it applied.
    pub fn dictionary(&self, applied: usize) {
        let span = tracing::info_span!(parent: &self.root, "dictionary", entries_applied = tracing::field::Empty);
        span.record("entries_applied", applied as u64);
    }

    pub fn words(&self, words: usize) {
        self.root.record("words", words as u64);
    }

    pub fn cleanup_started(&mut self) {
        self.cleanup = Some(tracing::info_span!(
            parent: &self.root,
            "cleanup",
            tokens_in = tracing::field::Empty,
            tokens_out = tracing::field::Empty,
            attempts = tracing::field::Empty
        ));
    }

    pub fn first_token(&self) {
        if let Some(cleanup) = &self.cleanup {
            tracing::info!(name: "first_token", parent: cleanup, "first_token");
        }
    }

    /// The text is ready: cleanup (if any) ends and the insert starts.
    pub fn ready(&mut self, fallback: bool) {
        self.cleanup = None;
        self.root.record("fallback", fallback);
        self.insert = Some(tracing::info_span!(parent: &self.root, "insert"));
    }

    /// The paste or copy was sent.
    pub fn inserted(&mut self) {
        self.insert = None;
    }

    pub fn end(&mut self, outcome: Outcome) {
        self.outcome = Some(outcome);
    }

    pub fn fail(&mut self, error_kind: &'static str) {
        self.root.record("error_kind", error_kind);
        self.outcome = Some(Outcome::Failed);
    }
}

fn new_root() -> Span {
    tracing::info_span!(
        "dictation",
        history_id = tracing::field::Empty,
        retry_of = tracing::field::Empty,
        outcome = tracing::field::Empty,
        error_kind = tracing::field::Empty,
        language = tracing::field::Empty,
        words = tracing::field::Empty,
        fallback = tracing::field::Empty,
    )
}

impl Drop for DictationTrace {
    fn drop(&mut self) {
        let outcome = self.outcome.unwrap_or(Outcome::Cancelled);
        self.root.record("outcome", outcome.as_str());
        // Close the children before the root, in stage order.
        self.mic_start = None;
        self.capture = None;
        self.speech = None;
        self.cleanup = None;
        self.insert = None;
    }
}
