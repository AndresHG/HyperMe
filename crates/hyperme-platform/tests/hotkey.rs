//! Slice 7: the hotkey matcher. Tests cite the requirements in
//! `specs/25-dictation-core/spec.md`.

use hyperme_core::config::{Hotkey, Modifier};
use hyperme_core::dictation::Event;
use hyperme_platform::hotkey::{Kind, Matcher, RawKey, flag, keycode, keycode_named};

fn flags_changed(code: u16, flags: u64) -> RawKey {
    RawKey {
        kind: Kind::FlagsChanged,
        keycode: code,
        flags,
        repeat: false,
    }
}

fn key(kind: Kind, code: u16, flags: u64) -> RawKey {
    RawKey {
        kind,
        keycode: code,
        flags,
        repeat: false,
    }
}

fn run(matcher: &mut Matcher, keys: &[RawKey]) -> Vec<Event> {
    keys.iter().filter_map(|k| matcher.on(*k)).collect()
}

/// R-1, R-8: hold Fn (the default) is key-down, letting go is key-up.
#[test]
fn hold_fn() {
    let mut m = Matcher::new(&Hotkey::Fn).unwrap();
    let events = run(
        &mut m,
        &[
            flags_changed(keycode::FUNCTION, flag::FUNCTION),
            // macOS sets the Fn flag on arrow keys; typing while holding does not end the hold.
            key(Kind::KeyDown, 0x7B, flag::FUNCTION),
            key(Kind::KeyUp, 0x7B, flag::FUNCTION),
            flags_changed(keycode::FUNCTION, 0),
        ],
    );
    assert_eq!(events, [Event::KeyDown, Event::KeyUp]);
}

#[test]
fn other_modifiers_do_not_trigger_fn() {
    let mut m = Matcher::new(&Hotkey::Fn).unwrap();
    let events = run(
        &mut m,
        &[
            flags_changed(0x38, flag::SHIFT),
            flags_changed(0x38, 0),
            flags_changed(
                keycode::RIGHT_OPTION,
                flag::OPTION | flag::RIGHT_OPTION_DEVICE,
            ),
            // A stray Fn release with nothing held.
            flags_changed(keycode::FUNCTION, 0),
        ],
    );
    assert!(events.is_empty(), "{events:?}");
}

/// R-8: Right Option, told apart from Left Option by its device bit.
#[test]
fn hold_right_option() {
    let mut m = Matcher::new(&Hotkey::RightOption).unwrap();
    let left = 0x3A;
    let events = run(
        &mut m,
        &[
            flags_changed(left, flag::OPTION | 0x20),
            flags_changed(
                keycode::RIGHT_OPTION,
                flag::OPTION | 0x20 | flag::RIGHT_OPTION_DEVICE,
            ),
            // Right released while left is still down: the Option flag stays, the device bit goes.
            flags_changed(keycode::RIGHT_OPTION, flag::OPTION | 0x20),
            flags_changed(left, 0),
        ],
    );
    assert_eq!(events, [Event::KeyDown, Event::KeyUp]);
}

/// R-8: a chord is down when its key goes down with exactly its modifiers, and up when the key
/// or a modifier is let go.
#[test]
fn chords() {
    let chord = Hotkey::Chord {
        modifiers: vec![Modifier::Ctrl, Modifier::Option],
        key: "space".into(),
    };
    let space = keycode_named("space").unwrap();
    let both = flag::CONTROL | flag::OPTION;

    let mut m = Matcher::new(&chord).unwrap();
    let held_repeat = RawKey {
        kind: Kind::KeyDown,
        keycode: space,
        flags: both,
        repeat: true,
    };
    let events = run(
        &mut m,
        &[
            key(Kind::KeyDown, space, both),
            held_repeat,
            key(Kind::KeyUp, space, both),
        ],
    );
    assert_eq!(events, [Event::KeyDown, Event::KeyUp]);

    // Releasing a modifier first ends it too, and the later key-up adds nothing.
    let events = run(
        &mut m,
        &[
            key(Kind::KeyDown, space, both),
            flags_changed(0x3B, flag::OPTION),
            key(Kind::KeyUp, space, flag::OPTION),
        ],
    );
    assert_eq!(events, [Event::KeyDown, Event::KeyUp]);

    // Wrong or extra modifiers, and the key alone, do nothing.
    let events = run(
        &mut m,
        &[
            key(Kind::KeyDown, space, flag::CONTROL),
            key(Kind::KeyDown, space, both | flag::SHIFT),
            key(Kind::KeyDown, space, 0),
        ],
    );
    assert!(events.is_empty(), "{events:?}");
}

#[test]
fn chord_with_fn_needs_the_fn_flag() {
    let chord = Hotkey::Chord {
        modifiers: vec![Modifier::Fn, Modifier::Shift],
        key: "d".into(),
    };
    let d = keycode_named("d").unwrap();
    let mut m = Matcher::new(&chord).unwrap();
    assert_eq!(m.on(key(Kind::KeyDown, d, flag::SHIFT)), None);
    assert_eq!(
        m.on(key(Kind::KeyDown, d, flag::SHIFT | flag::FUNCTION)),
        Some(Event::KeyDown)
    );
}

#[test]
fn unknown_chord_key_is_an_error() {
    let chord = Hotkey::Chord {
        modifiers: vec![Modifier::Cmd],
        key: "hyperspace".into(),
    };
    let error = Matcher::new(&chord).unwrap_err();
    assert!(
        error.message.contains("hyperspace") && error.message.contains("dictation.hotkey"),
        "{}",
        error.message
    );
}

/// R-6: Esc cancels, whatever the hotkey; a held, repeating Esc counts once.
#[test]
fn esc_cancels() {
    for hotkey in [Hotkey::Fn, Hotkey::RightOption] {
        let mut m = Matcher::new(&hotkey).unwrap();
        let repeat = RawKey {
            kind: Kind::KeyDown,
            keycode: keycode::ESCAPE,
            flags: 0,
            repeat: true,
        };
        let events = run(
            &mut m,
            &[
                key(Kind::KeyDown, keycode::ESCAPE, 0),
                repeat,
                key(Kind::KeyUp, keycode::ESCAPE, 0),
            ],
        );
        assert_eq!(events, [Event::Esc]);
    }
}

/// When the OS stops delivering keys mid-hold, the dictation is cancelled and the next press
/// starts clean.
#[test]
fn reset_cancels_a_hold() {
    let mut m = Matcher::new(&Hotkey::Fn).unwrap();
    assert_eq!(m.reset(), None);
    m.on(flags_changed(keycode::FUNCTION, flag::FUNCTION));
    assert_eq!(m.reset(), Some(Event::Esc));
    assert_eq!(m.on(flags_changed(keycode::FUNCTION, 0)), None);
    assert_eq!(
        m.on(flags_changed(keycode::FUNCTION, flag::FUNCTION)),
        Some(Event::KeyDown)
    );
}

#[test]
fn key_names() {
    assert_eq!(keycode_named("a"), Some(0x00));
    assert_eq!(keycode_named("space"), Some(0x31));
    assert_eq!(keycode_named("f12"), Some(0x6F));
    assert_eq!(
        keycode_named("A"),
        None,
        "names are lowercase, as the config stores them"
    );
}
