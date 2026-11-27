//! What the pill shows for each state of the dictation (docs/UX.md, "The pill"). Pure, so the
//! table is tested without a window; `main.rs` sends the view to the page and shows, sizes and
//! hides the panel.

use hyperme_core::dictation::Status;
use serde::Serialize;

/// The look of one state, sent to `pill.html` as an event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct View {
    /// `recording`, `working`, `done`, `raw`, `quiet`, `notice`, `error` or `retry`; the page
    /// picks the icon and colour from it.
    pub kind: &'static str,
    /// The few words shown, or empty.
    pub label: String,
    /// Hide after this many ms; `None` stays until the next state or a click.
    pub hide_after_ms: Option<u64>,
    /// A click retries the failed dictation (R-31).
    pub retry: bool,
}

/// What the panel does for a status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Show(View),
    /// Nothing to show, for example a cancelled dictation.
    Hide,
    /// The previous view stays as it is (its own timer hides it).
    Keep,
}

fn show(kind: &'static str, label: &str, hide_after_ms: Option<u64>) -> Change {
    Change::Show(View {
        kind,
        label: label.to_owned(),
        hide_after_ms,
        retry: false,
    })
}

/// The pill for a status, as docs/UX.md's table of states says.
pub fn change_for(status: &Status) -> Change {
    match status {
        Status::Recording => show("recording", "", None),
        Status::Transcribing | Status::Cleaning | Status::Pasting => show("working", "", None),
        Status::Done { raw: false } => show("done", "Pasted", Some(600)),
        // R-12, R-20.
        Status::Done { raw: true } => show("raw", "Raw text", Some(2000)),
        Status::Copied { raw } => show(if *raw { "raw" } else { "done" }, "Copied", Some(1500)),
        // R-14.
        Status::NothingHeard => show("quiet", "Nothing heard", Some(1500)),
        // R-17.
        Status::SecureInput => show("notice", "Not available in password fields", Some(2000)),
        Status::StillWorking => show("working", "Still working", None),
        // R-6: nothing pasted, and nothing to read; the pill just goes.
        Status::Cancelled => Change::Hide,
        // R-19 to R-21. The message already names the provider and never holds a key.
        Status::Failed {
            message,
            retry: false,
        } => show("error", message, Some(4000)),
        // R-31: stays until clicked.
        Status::Failed {
            message,
            retry: true,
        } => Change::Show(View {
            kind: "retry",
            label: format!("{} · Retry", short(message)),
            hide_after_ms: None,
            retry: true,
        }),
        Status::Idle => Change::Keep,
    }
}

/// What the panel shows once a state is over: the bead when "Show while idle" is on (U-13,
/// brought forward by the owner on 26 November 2026), otherwise nothing.
pub fn rest(show_while_idle: bool) -> Change {
    if show_while_idle {
        show("idle", "", None)
    } else {
        Change::Hide
    }
}

/// Width of the panel at rest: the 14 pt bead with a little room around it, so only the bead
/// itself takes clicks.
pub const BEAD_WIDTH: f64 = 22.0;

/// "Speech: speechmatics: the connection dropped: ..." becomes "Connection lost" for the pill;
/// the full message stays in the menu's status line.
fn short(message: &str) -> &'static str {
    if message.contains("rejected") {
        "Key rejected"
    } else {
        "Connection lost"
    }
}

/// Width of the pill for a label, in points: the 120 pt capsule, wider for longer words.
/// The state's mark sits at the left edge and the label in the exact centre, so the label has
/// 14 pt padding plus the mark's 16 pt (8 pt mark, 8 pt gap) on both sides.
pub fn width_for(view: &View) -> f64 {
    if view.kind == "idle" {
        return BEAD_WIDTH;
    }
    let chars = view.label.chars().count() as f64;
    if chars == 0.0 {
        120.0
    } else {
        (2.0 * (14.0 + 16.0) + chars * 7.2)
            .clamp(120.0, 420.0)
            .round()
    }
}

// ---- The morning card (specs/92-daily-tasks, slice 7: R-15 to R-19, AC-8, AC-9) ----

use hyperme_core::brief::{MorningCard, Part};
use hyperme_core::tasks::Group;

/// Width of a card from the pill (docs/UX.md: up to 360 pt).
pub const CARD_WIDTH: f64 = 360.0;
/// An unanswered card shrinks back into the dot after this long (docs/UX.md, "Alerts").
pub const CARD_LINGER_MS: u64 = 60_000;

/// One task on the morning card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CardLine {
    pub title: String,
    /// `due 10:00`, `overdue`, `unblocked`, `rolled 2`, or empty.
    pub mark: String,
    /// `due`, `unblocked` or empty: the page's colour for the mark and the line's dot.
    pub tone: &'static str,
}

/// The morning card as `pill.html` draws it (docs/ux/daily-cards.svg). Parts that do not
/// exist are `None` and take no room; parts waiting for a sync say "Updating…" (R-18).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CardView {
    pub title: String,
    pub yesterday: Option<String>,
    pub meetings: Option<String>,
    pub tasks: Vec<CardLine>,
    /// "2 waiting on blockers · 3 more in Today".
    pub footer: Option<String>,
    pub mail: Option<String>,
    /// The stale question, once (R-14).
    pub stale: Option<String>,
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The morning card's words from its model (R-17).
pub fn morning_view(card: &MorningCard) -> CardView {
    let title = format!("Good morning · {} {}", card.date.weekday(), card.date.day());
    let yesterday = card.yesterday.filter(|y| y.done + y.moved > 0).map(|y| {
        if y.moved > 0 {
            format!("Yesterday: {} done, {} moved", y.done, y.moved)
        } else {
            format!("Yesterday: {} done", y.done)
        }
    });
    let meetings = match &card.meetings {
        Part::Pending => Some("Meetings: Updating…".to_owned()),
        Part::Ready(list) if list.is_empty() => None,
        Part::Ready(list) => {
            let first = &list[0];
            Some(format!(
                "{} · first: {}, {:02}:{:02}",
                plural(list.len(), "meeting", "meetings"),
                first.title,
                first.start.hour(),
                first.start.minute()
            ))
        }
    };
    let tasks = card
        .top
        .iter()
        .map(|line| {
            let (mark, tone) = match (line.group, line.due) {
                (Group::Due, Some(due)) if due.date < card.date => ("overdue".to_owned(), "due"),
                (Group::Due, Some(due)) => (
                    due.time.map_or("due today".to_owned(), |t| {
                        format!("due {:02}:{:02}", t.hour(), t.minute())
                    }),
                    "due",
                ),
                (Group::Unblocked { .. }, _) => ("unblocked".to_owned(), "unblocked"),
                (Group::RolledOver, _) if line.rolled > 0 => {
                    (format!("rolled {}", line.rolled), "")
                }
                _ => (String::new(), ""),
            };
            CardLine {
                title: line.title.clone(),
                mark,
                tone,
            }
        })
        .collect();
    let mut footer = Vec::new();
    if card.waiting_on > 0 {
        footer.push(format!("{} waiting on blockers", card.waiting_on));
    }
    if card.more > 0 {
        footer.push(format!("{} more in Today", card.more));
    }
    let mail = match card.mail_to_answer {
        Part::Pending => Some("Mail: Updating…".to_owned()),
        Part::Ready(0) => None,
        Part::Ready(n) => Some(plural(n as usize, "mail to answer", "mails to answer")),
    };
    let stale = (!card.stale.is_empty()).then(|| {
        format!(
            "{} rolled over 3 times or more: keep, pick a day or drop in Today",
            plural(card.stale.len(), "task", "tasks")
        )
    });
    CardView {
        title,
        yesterday,
        meetings,
        tasks,
        footer: (!footer.is_empty()).then(|| footer.join(" · ")),
        mail,
        stale,
    }
}

/// An alert from the dot (docs/UX.md, "Alerts"): a title, one line, and a button that opens
/// where the fix is. Never takes focus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AlertView {
    pub title: String,
    pub detail: String,
    /// The main button's word, such as `Fix`.
    pub action: String,
    /// A `hyperme://settings/...` link the main button opens, when there is one.
    pub link: Option<String>,
}

/// A card the pill grows into: the morning card or an alert.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Card {
    Morning(CardView),
    Alert(AlertView),
}

impl Card {
    /// The panel's size for this card.
    pub fn size(&self) -> (f64, f64) {
        match self {
            Card::Morning(view) => card_size(view),
            Card::Alert(_) => (
                CARD_WIDTH,
                2.0 * CARD_PAD + CARD_TITLE + CARD_LINE + CARD_KEYS,
            ),
        }
    }
}

/// Heights in points; `pill.html` uses the same ones, so the panel fits the card exactly.
pub const CARD_PAD: f64 = 18.0;
pub const CARD_TITLE: f64 = 24.0;
pub const CARD_LINE: f64 = 20.0;
pub const CARD_ROW: f64 = 32.0;
pub const CARD_KEYS: f64 = 42.0;

/// The panel's size for a card.
pub fn card_size(view: &CardView) -> (f64, f64) {
    let lines = [
        view.yesterday.is_some(),
        view.meetings.is_some(),
        view.footer.is_some(),
        view.mail.is_some(),
        view.stale.is_some(),
    ]
    .iter()
    .filter(|&&on| on)
    .count() as f64;
    let tasks = view.tasks.len() as f64;
    let height = 2.0 * CARD_PAD + CARD_TITLE + lines * CARD_LINE + tasks * CARD_ROW + CARD_KEYS;
    (CARD_WIDTH, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(status: Status) -> View {
        match change_for(&status) {
            Change::Show(view) => view,
            other => panic!("{status:?} gives {other:?}"),
        }
    }

    /// docs/UX.md, "The pill": each state, its copy and how long it stays.
    #[test]
    fn the_states_match_the_ux_table() {
        assert_eq!(view(Status::Recording).kind, "recording");
        assert_eq!(view(Status::Transcribing).kind, "working");
        assert_eq!(view(Status::Cleaning).kind, "working");
        let done = view(Status::Done { raw: false });
        assert_eq!(
            (done.kind, done.label.as_str(), done.hide_after_ms),
            ("done", "Pasted", Some(600))
        );
        let raw = view(Status::Done { raw: true });
        assert_eq!(
            (raw.kind, raw.label.as_str(), raw.hide_after_ms),
            ("raw", "Raw text", Some(2000))
        );
        let quiet = view(Status::NothingHeard);
        assert_eq!(
            (quiet.label.as_str(), quiet.hide_after_ms),
            ("Nothing heard", Some(1500))
        );
        let notice = view(Status::SecureInput);
        assert_eq!(
            (notice.label.as_str(), notice.hide_after_ms),
            ("Not available in password fields", Some(2000))
        );
        assert_eq!(change_for(&Status::Cancelled), Change::Hide);
        assert_eq!(change_for(&Status::Idle), Change::Keep);
        assert_eq!(view(Status::StillWorking).label, "Still working");
    }

    /// R-19 to R-21: errors show the message for 4 s; R-31: a saved dictation offers Retry until
    /// clicked.
    #[test]
    fn errors_and_retry() {
        let error = view(Status::Failed {
            message: "speechmatics: the key was rejected. Check it in Settings".into(),
            retry: false,
        });
        assert_eq!(
            (error.kind, error.hide_after_ms, error.retry),
            ("error", Some(4000), false)
        );
        assert!(error.label.contains("speechmatics"));

        let retry = view(Status::Failed {
            message: "Speech: speechmatics: the connection dropped".into(),
            retry: true,
        });
        assert_eq!(
            (
                retry.kind,
                retry.label.as_str(),
                retry.hide_after_ms,
                retry.retry
            ),
            ("retry", "Connection lost · Retry", None, true)
        );
    }

    /// The bead at rest (U-13, brought forward by the owner): with "Show while idle" on, the
    /// pill rests as the small bead instead of hiding; off, it hides as before.
    #[test]
    fn at_rest_the_pill_becomes_the_bead_or_hides() {
        let Change::Show(bead) = rest(true) else {
            panic!("the bead shows while idle");
        };
        assert_eq!(
            (bead.kind, bead.label.as_str(), bead.hide_after_ms),
            ("idle", "", None)
        );
        assert_eq!(width_for(&bead), BEAD_WIDTH);
        assert_eq!(rest(false), Change::Hide);
    }

    /// The label sits in the exact centre: the state's mark is drawn at the left edge, so the
    /// pill keeps the same room on the right (owner, 26 November 2026: "Pasted" sat right of
    /// centre).
    #[test]
    fn labels_have_room_for_the_mark_on_both_sides() {
        let quiet = width_for(&view(Status::NothingHeard));
        assert!(quiet >= 2.0 * (14.0 + 16.0) + 13.0 * 7.2, "{quiet}");
    }

    #[test]
    fn longer_labels_widen_the_pill() {
        assert_eq!(width_for(&view(Status::Recording)), 120.0);
        assert_eq!(width_for(&view(Status::Done { raw: false })), 120.0);
        let notice = width_for(&view(Status::SecureInput));
        assert!(notice > 250.0 && notice <= 420.0, "{notice}");
        // The state's line, the gap and "Nothing heard" fit without an ellipsis: 14 px padding
        // each side, 14 px line, 8 px gap and about 7.2 px a character.
        let quiet = width_for(&view(Status::NothingHeard));
        assert!(quiet >= 14.0 * 2.0 + 14.0 + 8.0 + 13.0 * 7.2, "{quiet}");
    }
}

/// The input level for the meter, 0 to 1, from one frame of PCM16 (R-4): its RMS on a decibel
/// scale from -50 dBFS (silence) to 0 dBFS.
pub fn level(frame: &[i16]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    let mean_square = frame
        .iter()
        .map(|&s| f64::from(s) * f64::from(s))
        .sum::<f64>()
        / frame.len() as f64;
    let rms = mean_square.sqrt() / f64::from(i16::MAX);
    if rms <= 0.0 {
        return 0.0;
    }
    let db = 20.0 * rms.log10();
    ((db + 50.0) / 50.0).clamp(0.0, 1.0) as f32
}

#[cfg(test)]
mod level_tests {
    use super::level;

    /// R-4: silence reads 0, full scale reads 1, speech-like levels fall in between.
    #[test]
    fn level_follows_loudness() {
        assert_eq!(level(&[]), 0.0);
        assert_eq!(level(&[0; 480]), 0.0);
        assert!((level(&[i16::MAX; 480]) - 1.0).abs() < 0.01);
        let quiet = level(&[300; 480]);
        let loud = level(&[8000; 480]);
        assert!(0.0 < quiet && quiet < loud && loud < 1.0, "{quiet} {loud}");
    }

    /// The page must ask for the current view once its listeners exist: the bead's view is sent
    /// at launch, before the page has loaded, and was lost, so the bead showed the hidden
    /// recording bars in an opaque square (owner, 26 November 2026).
    #[test]
    fn the_page_asks_for_the_current_view_when_it_loads() {
        let page = include_str!("../../ui/pill.html");
        assert!(page.contains("invoke('pill_view')"));
        // Hidden means nothing drawn: no capsule, no bars left over from the warm-up.
        assert!(page.contains(".pill.hidden { display: none; }"));
    }

    // ---- The morning card ----

    use super::{
        CARD_KEYS, CARD_LINE, CARD_PAD, CARD_ROW, CARD_TITLE, CARD_WIDTH, card_size, morning_view,
    };

    use hyperme_core::brief::{DayCounts, Meeting, MorningCard, Part, TaskLine};
    use hyperme_core::tasks::{Due, Group};
    use time::{Date, Month, Time};

    fn day(d: u8) -> Date {
        Date::from_calendar_date(2026, Month::November, d).unwrap()
    }

    fn line(title: &str, group: Group, due: Option<Due>, rolled: u32) -> TaskLine {
        TaskLine {
            id: 1,
            title: title.into(),
            group,
            due,
            rolled,
            stale: false,
            new: false,
        }
    }

    fn card() -> MorningCard {
        MorningCard {
            date: day(27),
            meetings: Part::Pending,
            top: Vec::new(),
            more: 0,
            waiting_on: 0,
            proposals: 0,
            mail_to_answer: Part::Pending,
            yesterday: None,
            stale: Vec::new(),
        }
    }

    /// AC-9, R-18: before the syncs, the card says "Updating…" for meetings and mail, and
    /// nothing else takes room.
    #[test]
    fn a_card_before_the_syncs_says_updating() {
        let view = morning_view(&card());
        assert_eq!(view.title, "Good morning · Friday 27");
        assert_eq!(view.meetings.as_deref(), Some("Meetings: Updating…"));
        assert_eq!(view.mail.as_deref(), Some("Mail: Updating…"));
        assert_eq!(
            (view.yesterday, view.footer, view.stale),
            (None, None, None)
        );
        assert_eq!(
            card_size(&morning_view(&card())),
            (
                CARD_WIDTH,
                2.0 * CARD_PAD + CARD_TITLE + 2.0 * CARD_LINE + CARD_KEYS
            )
        );
    }

    /// R-17, docs/ux/daily-cards.svg: the date, yesterday, the meetings with the first one's
    /// time, up to five tasks with their marks, waiting and more, and mail to answer; empty
    /// parts take no room.
    #[test]
    fn a_full_card_matches_the_mockup() {
        let mut c = card();
        c.meetings = Part::Ready(vec![
            Meeting {
                title: "Weekly planning".into(),
                start: Time::from_hms(10, 30, 0).unwrap(),
                end: Time::from_hms(11, 0, 0).unwrap(),
                account: None,
                link: None,
            },
            Meeting {
                title: "1:1".into(),
                start: Time::from_hms(15, 0, 0).unwrap(),
                end: Time::from_hms(15, 30, 0).unwrap(),
                account: None,
                link: None,
            },
        ]);
        c.top = vec![
            line(
                "Call the dentist",
                Group::Due,
                Some(Due {
                    date: day(27),
                    time: Some(Time::from_hms(10, 0, 0).unwrap()),
                }),
                0,
            ),
            line(
                "Send Ana the numbers",
                Group::Unblocked {
                    blocker_dropped: false,
                },
                None,
                0,
            ),
            line("Review the copy", Group::Planned, None, 0),
            line("Book the room", Group::RolledOver, None, 2),
        ];
        c.more = 3;
        c.waiting_on = 2;
        c.mail_to_answer = Part::Ready(4);
        c.yesterday = Some(DayCounts {
            captured: 1,
            done: 6,
            moved: 2,
            dropped: 0,
        });
        let view = morning_view(&c);
        assert_eq!(
            view.yesterday.as_deref(),
            Some("Yesterday: 6 done, 2 moved")
        );
        assert_eq!(
            view.meetings.as_deref(),
            Some("2 meetings · first: Weekly planning, 10:30")
        );
        let marks: Vec<(&str, &str)> = view
            .tasks
            .iter()
            .map(|t| (t.mark.as_str(), t.tone))
            .collect();
        assert_eq!(
            marks,
            [
                ("due 10:00", "due"),
                ("unblocked", "unblocked"),
                ("", ""),
                ("rolled 2", "")
            ]
        );
        assert_eq!(
            view.footer.as_deref(),
            Some("2 waiting on blockers · 3 more in Today")
        );
        assert_eq!(view.mail.as_deref(), Some("4 mails to answer"));
        let (_, h) = card_size(&view);
        assert_eq!(
            h,
            2.0 * CARD_PAD + CARD_TITLE + 4.0 * CARD_LINE + 4.0 * CARD_ROW + CARD_KEYS
        );

        c.meetings = Part::Ready(Vec::new());
        c.mail_to_answer = Part::Ready(0);
        let view = morning_view(&c);
        assert_eq!(
            (view.meetings, view.mail),
            (None, None),
            "nothing to say, no line"
        );
    }

    /// R-36, AC-16: an alert card has one size, and serialises with its kind for the page.
    #[test]
    fn an_alert_card_is_tagged_and_sized() {
        use super::{AlertView, Card};
        let alert = Card::Alert(AlertView {
            title: "Job failed".into(),
            detail: "Couldn't reach your calendar twice today".into(),
            action: "Fix".into(),
            link: Some("hyperme://settings/connectors/google:personal".into()),
        });
        assert_eq!(
            alert.size(),
            (
                CARD_WIDTH,
                2.0 * CARD_PAD + CARD_TITLE + CARD_LINE + CARD_KEYS
            )
        );
        let json = serde_json::to_value(&alert).unwrap();
        assert_eq!(json["kind"], "alert");
        assert_eq!(json["action"], "Fix");
        let morning = Card::Morning(morning_view(&card()));
        assert_eq!(serde_json::to_value(&morning).unwrap()["kind"], "morning");
        assert_eq!(morning.size(), card_size(&morning_view(&card())));
    }
}
