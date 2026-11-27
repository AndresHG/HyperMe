//! The approval card (specs/166-gmail-approvals, slice 3): when a connector server asks the user
//! to approve a call, the card shows its question, its message as safe text and one key per
//! choice, in the Settings window (the setup sheet's check and Review on the account page). The
//! user's choice goes back to the waiting call; Esc, closing it or 5 minutes send nothing
//! (R-9, R-10, R-11, R-13).
//!
//! The ask lives only in memory, for as long as the card is open (R-20).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use hyperme_core::approval::{
    Approval, ApprovalAsk, ApprovalFuture, Approver, Span, default_choice, spans,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

/// The event that opens a card in the Settings window.
pub const ASK: &str = "connector-approval";
/// The event that closes a card that timed out.
pub const CLOSED: &str = "connector-approval-closed";
/// How long a card waits for the user (R-9).
pub const WAIT: Duration = Duration::from_secs(5 * 60);

/// One choice as the card draws it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ChoiceView {
    pub value: String,
    pub label: String,
    pub deny: bool,
}

/// What the card shows (R-10). Its `Debug` leaves out the title and message (R-20).
#[derive(Clone, Serialize, PartialEq, Eq)]
pub struct CardView {
    pub id: u64,
    /// "Google · personal · Mail"
    pub who: String,
    pub title: String,
    pub message: Vec<Span>,
    pub choices: Vec<ChoiceView>,
    /// The choice ↵ picks (A-1).
    pub default: Option<usize>,
    /// "Background updates need an Always allow choice." for a read pull (R-14).
    pub note: Option<String>,
}

impl std::fmt::Debug for CardView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CardView")
            .field("id", &self.id)
            .field("who", &self.who)
            .field("choices", &self.choices)
            .finish_non_exhaustive()
    }
}

/// The card for `ask`. `read` is true for a read pull, which picks the broadest "Always
/// allow" for ↵ and says background updates need one (A-1, R-14).
pub fn card_view(id: u64, ask: &ApprovalAsk, who: &str, read: bool) -> CardView {
    CardView {
        id,
        who: who.to_owned(),
        title: ask.title.clone(),
        message: spans(&ask.message),
        choices: ask
            .choices
            .iter()
            .map(|c| ChoiceView {
                value: c.value.clone(),
                label: c.label.clone(),
                deny: c.is_deny(),
            })
            .collect(),
        default: default_choice(&ask.choices, read),
        note: read.then(|| "Background updates need an Always allow choice.".to_owned()),
    }
}

/// The cards waiting for an answer. Kept apart from Tauri so it can be tested.
#[derive(Default)]
pub struct Pending {
    next: AtomicU64,
    waiting: Mutex<HashMap<u64, oneshot::Sender<Option<String>>>>,
    /// One card at a time: a second ask waits until the first is answered, so no card is ever
    /// replaced on screen and left waiting unseen.
    one: tokio::sync::Mutex<()>,
}

impl Pending {
    /// Shows a card through `show` and waits up to `wait` for [`Pending::answer`]; on timeout
    /// it calls `close` and answers nothing.
    pub async fn ask(
        &self,
        view: impl FnOnce(u64) -> CardView,
        wait: Duration,
        show: impl FnOnce(&CardView),
        close: impl FnOnce(u64),
    ) -> Approval {
        let _one = self.one.lock().await;
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let (sender, receiver) = oneshot::channel();
        self.lock().insert(id, sender);
        show(&view(id));
        match tokio::time::timeout(wait, receiver).await {
            Ok(Ok(Some(value))) => Approval::Choice(value),
            Ok(_) => Approval::NoAnswer,
            Err(_) => {
                self.lock().remove(&id);
                close(id);
                Approval::NoAnswer
            }
        }
    }

    /// The user's answer to card `id`: a choice's value, or nothing (Esc). False when the card
    /// is no longer waiting.
    pub fn answer(&self, id: u64, value: Option<String>) -> bool {
        match self.lock().remove(&id) {
            Some(sender) => sender.send(value).is_ok(),
            None => false,
        }
    }

    /// Answers nothing to every waiting card, as when the window that shows them closes.
    pub fn cancel_all(&self) {
        for (_, sender) in self.lock().drain() {
            let _ = sender.send(None);
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<u64, oneshot::Sender<Option<String>>>> {
        self.waiting.lock().unwrap_or_else(|p| p.into_inner())
    }
}

static PENDING: LazyLock<Pending> = LazyLock::new(Pending::default);

/// The Settings window closed: its cards can no longer be answered, so they answer nothing.
pub fn window_closed() {
    PENDING.cancel_all();
}

/// The interactive approver: the card in the Settings window (R-6).
pub struct UiApprover {
    pub app: AppHandle,
    /// "Google · personal"
    pub who: String,
    /// The server's words, "Mail".
    pub what: String,
    /// A read pull (A-1, R-14).
    pub read: bool,
}

impl Approver for UiApprover {
    fn answer(&self, ask: ApprovalAsk) -> ApprovalFuture<'_> {
        Box::pin(async move {
            // Nobody can see a card without the Settings sheet: answer nothing at once.
            if !crate::settings::is_open() {
                return Approval::NoAnswer;
            }
            let who = format!("{} · {}", self.who, self.what);
            PENDING
                .ask(
                    |id| card_view(id, &ask, &who, self.read),
                    WAIT,
                    |view| {
                        let _ = self.app.emit_to(crate::home::HOME, ASK, view);
                    },
                    |id| {
                        let _ = self.app.emit_to(crate::home::HOME, CLOSED, id);
                    },
                )
                .await
        })
    }
}

/// The card's answer: a choice's value, or nothing for Esc (R-13).
#[tauri::command]
pub fn connectors_answer(id: u64, value: Option<String>) -> bool {
    PENDING.answer(id, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperme_core::approval::Choice;

    fn ask() -> ApprovalAsk {
        ApprovalAsk {
            account: "google:personal".into(),
            server: "gmail".into(),
            tool: "search_threads".into(),
            title: "Allow HyperMe to use search_threads?".into(),
            message: "Read **3 threads**.".into(),
            choices: ["ALLOW_ONCE", "ALLOW_TOOL_ALWAYS", "DENY_ONCE"]
                .iter()
                .zip(["Allow once", "Always allow this tool", "Don't allow"])
                .map(|(v, l)| Choice {
                    value: (*v).into(),
                    label: l.into(),
                })
                .collect(),
        }
    }

    /// AC-16, R-10, R-14: the card lists the server's choices in order with their labels,
    /// marks the deny, and ↵ picks "Always allow this tool" for a read pull.
    #[test]
    fn the_card_shows_the_choices_and_the_default() {
        let view = card_view(7, &ask(), "Google · personal · Mail", true);
        let labels: Vec<&str> = view.choices.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(
            labels,
            ["Allow once", "Always allow this tool", "Don't allow"]
        );
        assert_eq!(view.default, Some(1));
        assert!(view.choices[2].deny);
        assert_eq!(view.message[1], Span::Bold("3 threads".into()));
        assert!(view.note.is_some());
        assert_eq!(card_view(7, &ask(), "x", false).default, Some(0));
    }

    /// AC-2 (app part): a choice made on the card reaches the waiting call.
    #[tokio::test]
    async fn an_answer_reaches_the_waiting_call() {
        let pending = Arc::new(Pending::default());
        let shown = Arc::new(Mutex::new(None));
        let call = {
            let (pending, shown) = (pending.clone(), shown.clone());
            tokio::spawn(async move {
                pending
                    .ask(
                        |id| card_view(id, &ask(), "w", true),
                        WAIT,
                        |view| *shown.lock().unwrap() = Some(view.id),
                        |_| {},
                    )
                    .await
            })
        };
        let id = loop {
            if let Some(id) = *shown.lock().unwrap() {
                break id;
            }
            tokio::task::yield_now().await;
        };
        assert!(pending.answer(id, Some("ALLOW_TOOL_ALWAYS".into())));
        assert_eq!(
            call.await.unwrap(),
            Approval::Choice("ALLOW_TOOL_ALWAYS".into())
        );
        assert!(!pending.answer(id, None), "answered once");
    }

    /// AC-6: Esc answers nothing; 5 minutes with no answer closes the card and answers nothing.
    #[tokio::test(start_paused = true)]
    async fn esc_and_timeouts_answer_nothing() {
        let pending = Arc::new(Pending::default());
        let closed = Arc::new(Mutex::new(Vec::new()));
        let call = {
            let (pending, closed) = (pending.clone(), closed.clone());
            tokio::spawn(async move {
                pending
                    .ask(
                        |id| card_view(id, &ask(), "w", true),
                        WAIT,
                        |_| {},
                        |id| closed.lock().unwrap().push(id),
                    )
                    .await
            })
        };
        tokio::time::sleep(WAIT + Duration::from_secs(1)).await;
        assert_eq!(call.await.unwrap(), Approval::NoAnswer);
        assert_eq!(closed.lock().unwrap().len(), 1);

        let esc = {
            let pending = pending.clone();
            tokio::spawn(async move {
                pending
                    .ask(|id| card_view(id, &ask(), "w", true), WAIT, |_| {}, |_| {})
                    .await
            })
        };
        while pending.lock().is_empty() {
            tokio::task::yield_now().await;
        }
        let id = *pending.lock().keys().next().unwrap();
        assert!(pending.answer(id, None));
        assert_eq!(esc.await.unwrap(), Approval::NoAnswer);
        assert_eq!(
            closed.lock().unwrap().len(),
            1,
            "Esc does not close as a timeout"
        );
    }

    /// R-9: closing the window answers nothing to the waiting card; a second ask waits for the
    /// first instead of replacing it.
    #[tokio::test]
    async fn closing_answers_nothing_and_cards_come_one_at_a_time() {
        let pending = Arc::new(Pending::default());
        let shown = Arc::new(Mutex::new(Vec::new()));
        let spawn = |pending: Arc<Pending>, shown: Arc<Mutex<Vec<u64>>>| {
            tokio::spawn(async move {
                pending
                    .ask(
                        |id| card_view(id, &ask(), "w", true),
                        WAIT,
                        |view| shown.lock().unwrap().push(view.id),
                        |_| {},
                    )
                    .await
            })
        };
        let first = spawn(pending.clone(), shown.clone());
        let second = spawn(pending.clone(), shown.clone());
        while shown.lock().unwrap().is_empty() {
            tokio::task::yield_now().await;
        }
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        assert_eq!(shown.lock().unwrap().len(), 1, "the second card waits");
        pending.cancel_all();
        assert_eq!(first.await.unwrap(), Approval::NoAnswer);
        while shown.lock().unwrap().len() < 2 {
            tokio::task::yield_now().await;
        }
        pending.cancel_all();
        assert_eq!(second.await.unwrap(), Approval::NoAnswer);
    }

    use std::sync::Arc;
}
