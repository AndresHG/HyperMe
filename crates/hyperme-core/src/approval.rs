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
