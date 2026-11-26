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
pub fn width_for(view: &View) -> f64 {
    let chars = view.label.chars().count() as f64;
    if chars == 0.0 {
        120.0
    } else {
        (52.0 + chars * 7.2).clamp(120.0, 420.0).round()
    }
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
}
