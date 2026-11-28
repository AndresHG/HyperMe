//! Approvals that connector servers ask for (specs/166-gmail-approvals): what one ask holds,
//! and who answers it. An MCP server such as Gmail may answer a call with "input required" and
//! a form of choices ("Allow once", "Always allow this tool", "Don't allow"); the call's
//! [`Approver`] decides what happens.
//!
//! - The setup sheet and Review use an interactive approver that shows the card (R-6).
//! - Scheduler jobs use [`Unattended`], which never answers for the user (R-7).
//!
//! An ask lives in memory only: its title and message are never logged or written (R-20), so
//! its `Debug` leaves them out.

use std::fmt;
use std::future::Future;
use std::pin::Pin;

/// One choice the server offers, in the server's order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// What is sent back, such as `ALLOW_TOOL_ALWAYS`.
    pub value: String,
    /// What the card shows, such as "Always allow this tool"; the value when the server gave
    /// no label (R-10).
    pub label: String,
}

impl Choice {
    /// A choice that refuses the call (R-15).
    pub fn is_deny(&self) -> bool {
        self.value.starts_with("DENY")
    }
}

/// What a server asks the user, ready for a card.
#[derive(Clone, PartialEq, Eq)]
pub struct ApprovalAsk {
    /// The account asked for, such as `google:personal`.
    pub account: String,
    /// The connector server, such as `gmail`.
    pub server: String,
    pub tool: String,
    /// The question (R-10).
    pub title: String,
    /// The server's Markdown message, drawn as safe text (R-11).
    pub message: String,
    pub choices: Vec<Choice>,
}

impl fmt::Debug for ApprovalAsk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApprovalAsk")
            .field("account", &self.account)
            .field("server", &self.server)
            .field("tool", &self.tool)
            .field("choices", &self.choices)
            .finish_non_exhaustive()
    }
}

/// What the approver decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Approval {
    /// The user picked this value, one of the ask's choices.
    Choice(String),
    /// Nothing is sent: Esc, a timeout, or nobody there to ask (R-7, R-9).
    NoAnswer,
}

/// The future an [`Approver`] returns; boxed so approvers can be shared as `Arc<dyn Approver>`.
pub type ApprovalFuture<'a> = Pin<Box<dyn Future<Output = Approval> + Send + 'a>>;

/// Answers what a server asks during one call.
pub trait Approver: Send + Sync {
    fn answer(&self, ask: ApprovalAsk) -> ApprovalFuture<'_>;
}

/// The approver of scheduler jobs: nobody is there, so it never answers (R-7).
#[derive(Debug, Clone, Copy, Default)]
pub struct Unattended;

impl Approver for Unattended {
    fn answer(&self, _ask: ApprovalAsk) -> ApprovalFuture<'_> {
        Box::pin(std::future::ready(Approval::NoAnswer))
    }
}

/// One piece of a server's Markdown message, drawn as plain text by the card (A-5, R-11): no
/// HTML is ever produced, and a link is shown with its address, never followed.
/// Serialised as `{"kind": "text" | "bold" | "italic" | "code" | "break" | "item" | "link",
/// "text": ..., "url": ...}`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type), specta(type = SpanShape))]
pub enum Span {
    Text(String),
    Bold(String),
    Italic(String),
    Code(String),
    /// A paragraph break.
    Break,
    /// The start of a list item.
    Item,
    Link {
        text: String,
        url: String,
    },
}

/// The shape `Span` is serialised as, for the app's TypeScript bindings (spec 257, R-9).
#[cfg(feature = "specta")]
#[derive(serde::Serialize, specta::Type)]
#[serde(rename = "Span")]
#[allow(dead_code)]
struct SpanShape {
    kind: SpanKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

#[cfg(feature = "specta")]
#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)]
enum SpanKind {
    Text,
    Bold,
    Italic,
    Code,
    Break,
    Item,
    Link,
}

impl serde::Serialize for Span {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let (kind, text, url) = match self {
            Span::Text(t) => ("text", Some(t), None),
            Span::Bold(t) => ("bold", Some(t), None),
            Span::Italic(t) => ("italic", Some(t), None),
            Span::Code(t) => ("code", Some(t), None),
            Span::Break => ("break", None, None),
            Span::Item => ("item", None, None),
            Span::Link { text, url } => ("link", Some(text), Some(url)),
        };
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("kind", kind)?;
        if let Some(text) = text {
            map.serialize_entry("text", text)?;
        }
        if let Some(url) = url {
            map.serialize_entry("url", url)?;
        }
        map.end()
    }
}

/// The message as spans: paragraphs, `**bold**`, `*italic*` or `_italic_`, `` `code` ``,
/// `- ` or `* ` or `1. ` list items, and `[text](url)` links. Anything else, HTML included,
/// stays text.
pub fn spans(markdown: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let mut blank = false;
    for line in markdown.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            blank = true;
            continue;
        }
        let trimmed = line.trim_start();
        let item = ["- ", "* ", "+ "]
            .iter()
            .find_map(|m| trimmed.strip_prefix(m))
            .or_else(|| {
                let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
                (digits > 0)
                    .then(|| trimmed[digits..].strip_prefix(". "))
                    .flatten()
            });
        if let Some(rest) = item {
            out.push(Span::Item);
            inline(rest, &mut out);
        } else {
            if !out.is_empty() {
                // A new paragraph after a blank line or a list; a wrapped line joins with a space.
                if blank || last_was_item(&out) {
                    out.push(Span::Break);
                } else {
                    out.push(Span::Text(" ".to_owned()));
                }
            }
            inline(trimmed, &mut out);
        }
        blank = false;
    }
    merge_text(out)
}

fn last_was_item(out: &[Span]) -> bool {
    out.iter()
        .rev()
        .take_while(|s| **s != Span::Break)
        .any(|s| *s == Span::Item)
}

fn inline(text: &str, out: &mut Vec<Span>) {
    let mut plain = String::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        let found = match c {
            '*' if rest.starts_with("**") => {
                closed(&rest[2..], "**").map(|(t, r)| (Span::Bold(t), r))
            }
            '*' => closed(&rest[1..], "*").map(|(t, r)| (Span::Italic(t), r)),
            // `_` marks italics only at word edges, so `search_threads` stays one word.
            '_' if !plain.ends_with(char::is_alphanumeric) => closed(&rest[1..], "_")
                .filter(|(_, after)| !after.starts_with(char::is_alphanumeric))
                .map(|(t, r)| (Span::Italic(t), r)),
            '`' => closed(&rest[1..], "`").map(|(t, r)| (Span::Code(t), r)),
            '[' => link(rest),
            _ => None,
        };
        match found {
            Some((span, after)) => {
                if !plain.is_empty() {
                    out.push(Span::Text(std::mem::take(&mut plain)));
                }
                out.push(span);
                rest = after;
            }
            None => {
                plain.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    if !plain.is_empty() {
        out.push(Span::Text(plain));
    }
}

/// The text up to `end`, if it is closed and not empty or padded with spaces.
fn closed<'a>(text: &'a str, end: &str) -> Option<(String, &'a str)> {
    let at = text.find(end)?;
    let inner = &text[..at];
    if inner.is_empty() || inner.starts_with(' ') || inner.ends_with(' ') {
        return None;
    }
    Some((inner.to_owned(), &text[at + end.len()..]))
}

fn link(text: &str) -> Option<(Span, &str)> {
    let close = text.find("](")?;
    let label = &text[1..close];
    let after = &text[close + 2..];
    // The address ends at the `)` that balances, so `(…)` inside it stays part of it.
    let mut depth = 0usize;
    let end = after.char_indices().find_map(|(i, c)| match c {
        '(' => {
            depth += 1;
            None
        }
        ')' if depth == 0 => Some(i),
        ')' => {
            depth -= 1;
            None
        }
        _ => None,
    })?;
    if label.is_empty() || label.contains('[') {
        return None;
    }
    Some((
        Span::Link {
            text: label.to_owned(),
            url: after[..end].to_owned(),
        },
        &after[end + 1..],
    ))
}

fn merge_text(spans: Vec<Span>) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    for span in spans {
        if let (Some(Span::Text(last)), Span::Text(next)) = (out.last_mut(), &span) {
            last.push_str(next);
            continue;
        }
        out.push(span);
    }
    out
}

/// The choice ↵ picks (A-1, R-13): for a read pull, the broadest persistent choice offered
/// (this tool, then these owners, then this resource, then this audience); otherwise, and when
/// none is offered, the first choice that does not refuse. Never a deny.
pub fn default_choice(choices: &[Choice], read: bool) -> Option<usize> {
    if read {
        for scope in ["TOOL", "OWNER", "RESOURCE", "AUDIENCE"] {
            if let Some(i) = choices
                .iter()
                .position(|c| !c.is_deny() && c.value.contains("ALWAYS") && c.value.contains(scope))
            {
                return Some(i);
            }
        }
    }
    choices
        .iter()
        .position(|c| c.value == "ALLOW_ONCE")
        .or_else(|| choices.iter().position(|c| !c.is_deny()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask() -> ApprovalAsk {
        ApprovalAsk {
            account: "google:personal".into(),
            server: "gmail".into(),
            tool: "search_threads".into(),
            title: "secret title".into(),
            message: "secret message".into(),
            choices: vec![Choice {
                value: "DENY_ONCE".into(),
                label: "Don't allow".into(),
            }],
        }
    }

    /// R-20: the title and message stay out of `Debug`, so they cannot reach a log by accident.
    #[test]
    fn debug_leaves_out_the_title_and_message() {
        let text = format!("{:?}", ask());
        assert!(text.contains("search_threads"));
        assert!(!text.contains("secret"));
    }

    /// R-7: the unattended approver never answers.
    #[tokio::test]
    async fn unattended_never_answers() {
        assert_eq!(Unattended.answer(ask()).await, Approval::NoAnswer);
    }

    #[test]
    fn deny_choices_start_with_deny() {
        assert!(ask().choices[0].is_deny());
        let allow = Choice {
            value: "ALLOW_ONCE".into(),
            label: "Allow once".into(),
        };
        assert!(!allow.is_deny());
    }
}
