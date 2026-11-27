//! Spec 93, AC-15 (R-20, R-21): the rules of watching a field after a paste, with fake fields
//! and a fake clock.

use std::cell::Cell;
use std::time::{Duration, Instant};

use hyperme_core::field_watch::{DEBOUNCE, End, Field, FieldWatch, WINDOW};

const PASTED: &str = "Send the sun trix report today.";

fn field(value: &str) -> Field {
    Field {
        value: Some(value.to_owned()),
        ..Field::default()
    }
}

fn s(secs: f32) -> Duration {
    Duration::from_secs_f32(secs)
}

/// AC-15: a secure field, secure input, an unreadable field or a field without the pasted text
/// is never watched.
#[test]
fn a_secure_or_unreadable_field_is_not_watched() {
    let now = Instant::now();
    let value = format!("Hi. {PASTED}");
    let secure = Field {
        secure_field: true,
        ..field(&value)
    };
    assert!(FieldWatch::start(PASTED, secure, now).is_none());
    let input = Field {
        secure_input: true,
        ..field(&value)
    };
    assert!(FieldWatch::start(PASTED, input, now).is_none());
    assert!(FieldWatch::start(PASTED, Field::default(), now).is_none());
    assert!(FieldWatch::start(PASTED, field("something else"), now).is_none());
    assert!(FieldWatch::start(PASTED, field(&value), now).is_some());
}

/// AC-15 (R-21): changes are read once the field is quiet for a second, and only the pasted
/// stretch is kept, even when the user also types before and after it.
#[test]
fn changes_are_read_after_a_quiet_second_and_only_the_stretch_is_kept() {
    let t0 = Instant::now();
    let before = "Dear team,\n";
    let after = "\nBest,\nAna";
    let mut watch =
        FieldWatch::start(PASTED, field(&format!("{before}{PASTED}{after}")), t0).unwrap();
    assert_eq!(watch.next_wake(), t0 + WINDOW);

    let reads = Cell::new(0);
    let read = |v: &str| {
        reads.set(reads.get() + 1);
        Some(v.to_owned())
    };
    watch.changed(t0 + s(2.0));
    watch.changed(t0 + s(2.5));
    assert_eq!(watch.next_wake(), t0 + s(2.5) + DEBOUNCE);
    assert!(!watch.wake(t0 + s(3.0), || read("too early")));
    assert_eq!(reads.get(), 0);
    let edited = format!("Hello all,\n{before}Send the Zentrix report today.{after}\nPS: thanks");
    assert!(!watch.wake(t0 + s(3.6), || read(&edited)));
    assert_eq!(reads.get(), 1);
    watch.end(End::NextDictation, || read("not read: nothing pending"));
    assert_eq!(reads.get(), 1);
    assert_eq!(
        watch.finish(),
        Some((
            PASTED.to_owned(),
            "Send the Zentrix report today.".to_owned()
        ))
    );
}

/// AC-15 (R-20): the watch ends on a focus change (with no read of a pending change), at the
/// next dictation (reading a pending change first), and 60 s after the paste.
#[test]
fn the_watch_stops_on_focus_change_next_dictation_and_time_up() {
    let t0 = Instant::now();
    let edited = "Send the Zentrix report today.";

    let mut focus = FieldWatch::start(PASTED, field(PASTED), t0).unwrap();
    focus.changed(t0 + s(1.0));
    focus.end(End::FocusChanged, || panic!("read after the focus moved"));
    assert_eq!(focus.ended(), Some(End::FocusChanged));
    assert!(focus.wake(t0 + s(5.0), || panic!("read after the end")));
    assert_eq!(focus.finish(), None);

    let mut next = FieldWatch::start(PASTED, field(PASTED), t0).unwrap();
    next.changed(t0 + s(1.0));
    next.end(End::NextDictation, || Some(edited.to_owned()));
    assert_eq!(next.finish().map(|(_, now)| now).as_deref(), Some(edited));

    let mut time = FieldWatch::start(PASTED, field(PASTED), t0).unwrap();
    assert!(!time.wake(t0 + s(59.0), || None));
    assert!(time.wake(t0 + WINDOW, || None));
    assert_eq!(time.ended(), Some(End::TimeUp));
    // Unchanged: nothing to learn.
    assert_eq!(time.finish(), None);
}

/// The caret picks the pasted occurrence when the text appears twice; UTF-16 offsets count
/// emoji as two units.
#[test]
fn the_caret_picks_which_occurrence_was_pasted() {
    let t0 = Instant::now();
    let value = format!("😀 {PASTED} and {PASTED}");
    let first_end_utf16 = "😀 ".encode_utf16().count() + PASTED.encode_utf16().count();
    let with_caret = Field {
        caret_utf16: Some(first_end_utf16),
        ..field(&value)
    };
    let mut watch = FieldWatch::start(PASTED, with_caret, t0).unwrap();
    watch.changed(t0);
    let edited = format!("😀 Send the Zentrix report today. and {PASTED}");
    watch.wake(t0 + DEBOUNCE, || Some(edited.clone()));
    assert_eq!(
        watch.finish().map(|(_, now)| now).as_deref(),
        Some("Send the Zentrix report today.")
    );
}
