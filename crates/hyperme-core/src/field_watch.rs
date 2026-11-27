//! The rules of watching a field after a paste, for learning from the user's fixes (spec 93,
//! R-16, R-20, R-21; design.md, "The watcher on macOS"). Pure code with no clock or OS calls of
//! its own, so the platform watcher (a [`crate::traits::FieldWatcher`]) only reports what
//! happens and does what this says, and the rules are tested with fakes (AC-15).
//!
//! - Nothing is read from a secure field, or while secure input is on.
//! - A value change is read once the field has been quiet for [`DEBOUNCE`].
//! - The watch ends on a focus change, at the next dictation, or [`WINDOW`] after the paste.
//! - Only the pasted stretch is kept; the rest of the field is dropped as soon as it is read.

use std::time::{Duration, Instant};

/// A change is read after this long without another (design.md).
pub const DEBOUNCE: Duration = Duration::from_secs(1);
/// The watch ends this long after the paste (R-16, R-20).
pub const WINDOW: Duration = Duration::from_secs(60);
/// Characters of the text around the paste kept to find the stretch again.
const CONTEXT: usize = 32;

/// What the platform saw in the focused element right after the paste.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Field {
    /// A secure text field (a password field).
    pub secure_field: bool,
    /// Secure input is on, in any app.
    pub secure_input: bool,
    /// The field's whole text, if it could be read.
    pub value: Option<String>,
    /// The insertion point, in UTF-16 units as macOS gives it, if known.
    pub caret_utf16: Option<usize>,
}

/// Why a watch ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    FocusChanged,
    NextDictation,
    TimeUp,
}

/// One watch of the stretch HyperMe pasted.
#[derive(Debug)]
pub struct FieldWatch {
    pasted: String,
    /// The end of the text before the paste and the start of the text after it, at most
    /// [`CONTEXT`] characters each, with how long each side was.
    before: (String, usize),
    after: (String, usize),
    started: Instant,
    read_at: Option<Instant>,
    stretch: Option<String>,
    ended: Option<End>,
}

/// The char index of a UTF-16 offset in `text`.
fn char_at_utf16(text: &str, utf16: usize) -> Option<usize> {
    let mut units = 0;
    for (i, c) in text.chars().enumerate() {
        if units >= utf16 {
            return (units == utf16).then_some(i);
        }
        units += c.len_utf16();
    }
    (units == utf16).then(|| text.chars().count())
}

impl FieldWatch {
    /// Starts a watch, or `None` when the field must not or cannot be read (R-20): a secure
    /// field, secure input on, no readable text, or the pasted text is not in it.
    pub fn start(pasted: &str, field: Field, now: Instant) -> Option<Self> {
        if field.secure_field || field.secure_input || pasted.trim().is_empty() {
            return None;
        }
        let value: Vec<char> = field.value?.chars().collect();
        let paste: Vec<char> = pasted.chars().collect();
        let ends_at = |end: usize| end >= paste.len() && value[end - paste.len()..end] == paste[..];
        // The occurrence that ends at the caret, or else the last one.
        let end = field
            .caret_utf16
            .and_then(|c| char_at_utf16(&value.iter().collect::<String>(), c))
            .filter(|&end| end <= value.len() && ends_at(end))
            .or_else(|| (paste.len()..=value.len()).rev().find(|&end| ends_at(end)))?;
        let start = end - paste.len();
        let before: String = value[start.saturating_sub(CONTEXT)..start].iter().collect();
        let after: String = value[end..(end + CONTEXT).min(value.len())]
            .iter()
            .collect();
        Some(FieldWatch {
            pasted: pasted.to_owned(),
            before: (before, start),
            after: (after, value.len() - end),
            started: now,
            read_at: None,
            stretch: None,
            ended: None,
        })
    }

    /// The field's value changed: read it once it has been quiet for [`DEBOUNCE`].
    pub fn changed(&mut self, now: Instant) {
        if self.ended.is_none() {
            self.read_at = Some(now + DEBOUNCE);
        }
    }

    /// When the platform should call [`FieldWatch::wake`] next.
    pub fn next_wake(&self) -> Instant {
        let deadline = self.started + WINDOW;
        self.read_at.map_or(deadline, |at| at.min(deadline))
    }

    /// Reads the field if a read is due, then ends the watch if the time is up. `read` gives the
    /// field's whole text, or `None` when it cannot be read or secure input is on (R-20); it is
    /// only called when a read is due. Returns whether the watch has ended.
    pub fn wake(&mut self, now: Instant, read: impl FnOnce() -> Option<String>) -> bool {
        if self.ended.is_some() {
            return true;
        }
        if self.read_at.is_some_and(|at| at <= now) {
            self.read_at = None;
            if let Some(value) = read() {
                self.keep(&value);
            }
        }
        if now >= self.started + WINDOW {
            self.end(End::TimeUp, || None);
        }
        self.ended.is_some()
    }

    /// Ends the watch (R-20). A change still waiting for its read is read now, unless the
    /// focus moved or `read` declines.
    pub fn end(&mut self, why: End, read: impl FnOnce() -> Option<String>) {
        if self.ended.is_some() {
            return;
        }
        if self.read_at.take().is_some()
            && why != End::FocusChanged
            && let Some(value) = read()
        {
            self.keep(&value);
        }
        self.ended = Some(why);
    }

    pub fn ended(&self) -> Option<End> {
        self.ended
    }

    /// Keeps only the pasted stretch of `value` (R-21): the text between the context before
    /// and after the paste. If either side cannot be found, the stretch is not updated.
    fn keep(&mut self, value: &str) {
        let chars: Vec<char> = value.chars().collect();
        let (tail, before_len) = (&self.before.0, self.before.1);
        let (head, after_len) = (&self.after.0, self.after.1);
        let tail: Vec<char> = tail.chars().collect();
        let head: Vec<char> = head.chars().collect();
        let start = if before_len <= chars.len()
            && chars[before_len - tail.len()..before_len] == tail[..]
        {
            Some(before_len)
        } else {
            (tail.len()..=chars.len()).find(|&i| chars[i - tail.len()..i] == tail[..])
        };
        let end = chars
            .len()
            .checked_sub(after_len)
            .filter(|&e| e + head.len() <= chars.len() && chars[e..e + head.len()] == head[..])
            .or_else(|| {
                (0..=chars.len().saturating_sub(head.len()))
                    .rev()
                    .find(|&i| chars[i..i + head.len()] == head[..])
            });
        if let (Some(start), Some(end)) = (start, end)
            && start <= end
        {
            self.stretch = Some(chars[start..end].iter().collect());
        }
    }

    /// The pasted text and what its stretch holds now, if the user changed it.
    pub fn finish(self) -> Option<(String, String)> {
        let stretch = self.stretch?;
        (stretch != self.pasted).then_some((self.pasted, stretch))
    }
}
