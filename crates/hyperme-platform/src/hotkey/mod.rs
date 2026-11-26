//! The dictation hotkey (R-1, R-6, R-8): hold Fn by default, Right Option or a chord, and Esc
//! to cancel. [`Matcher`] turns raw key events into the state machine's events; it is pure, so
//! it is tested on every OS. On macOS, [`macos::HotkeyListener`] feeds it from an event tap.
//!
//! Key codes and flag bits are macOS's (`Events.h` virtual key codes and `CGEventFlags`); a
//! Linux or Windows listener maps its own codes to these before calling the matcher.

#[cfg(target_os = "macos")]
pub mod macos;

use hyperme_core::config::{Hotkey, Modifier};
use hyperme_core::dictation::Event;
use hyperme_core::traits::{Error, ErrorKind};

/// Virtual key codes used by the matcher.
pub mod keycode {
    pub const ESCAPE: u16 = 0x35;
    pub const FUNCTION: u16 = 0x3F;
    pub const RIGHT_OPTION: u16 = 0x3D;
}

/// `CGEventFlags` bits used by the matcher.
pub mod flag {
    pub const SHIFT: u64 = 0x0002_0000;
    pub const CONTROL: u64 = 0x0004_0000;
    pub const OPTION: u64 = 0x0008_0000;
    pub const COMMAND: u64 = 0x0010_0000;
    pub const FUNCTION: u64 = 0x0080_0000;
    /// The device-dependent bit for the right Option key (`NX_DEVICERALTKEYMASK`).
    pub const RIGHT_OPTION_DEVICE: u64 = 0x0000_0040;
}

/// What kind of key event arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A modifier went down or up; the flags say which are down now.
    FlagsChanged,
    KeyDown,
    KeyUp,
}

/// One key event from the OS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawKey {
    pub kind: Kind,
    pub keycode: u16,
    pub flags: u64,
    /// A key-down repeated because the key is held.
    pub repeat: bool,
}

/// Turns raw key events into `KeyDown`, `KeyUp` and `Esc` for the configured hotkey.
#[derive(Debug, Clone)]
pub struct Matcher {
    target: Target,
    held: bool,
}

#[derive(Debug, Clone)]
enum Target {
    Fn,
    RightOption,
    Chord {
        modifiers: u64,
        needs_fn: bool,
        keycode: u16,
    },
}

/// The modifiers a chord compares; Fn is left out because macOS sets it on arrow and
/// function keys.
const CHORD_MODIFIERS: u64 = flag::SHIFT | flag::CONTROL | flag::OPTION | flag::COMMAND;

impl Matcher {
    /// Fails when a chord names a key this build does not know.
    pub fn new(hotkey: &Hotkey) -> Result<Self, Error> {
        let target = match hotkey {
            Hotkey::Fn => Target::Fn,
            Hotkey::RightOption => Target::RightOption,
            Hotkey::Chord { modifiers, key } => {
                let keycode = keycode_named(key).ok_or_else(|| Error {
                    kind: ErrorKind::Other,
                    message: format!("dictation.hotkey: {key:?} is not a key HyperMe knows"),
                })?;
                let mut mask = 0;
                for modifier in modifiers {
                    mask |= match modifier {
                        Modifier::Cmd => flag::COMMAND,
                        Modifier::Ctrl => flag::CONTROL,
                        Modifier::Option => flag::OPTION,
                        Modifier::Shift => flag::SHIFT,
                        Modifier::Fn => 0,
                    };
                }
                Target::Chord {
                    modifiers: mask,
                    needs_fn: modifiers.contains(&Modifier::Fn),
                    keycode,
                }
            }
        };
        Ok(Matcher {
            target,
            held: false,
        })
    }

    /// The state machine's event for this key event, if any.
    pub fn on(&mut self, key: RawKey) -> Option<Event> {
        // R-6: Esc cancels. It is passed on to the app too; the state machine ignores it when
        // no dictation is running.
        if key.kind == Kind::KeyDown && key.keycode == keycode::ESCAPE && !key.repeat {
            return Some(Event::Esc);
        }
        let down = match &self.target {
            Target::Fn => match key.kind {
                Kind::FlagsChanged if key.keycode == keycode::FUNCTION => {
                    key.flags & flag::FUNCTION != 0
                }
                _ => return None,
            },
            Target::RightOption => match key.kind {
                Kind::FlagsChanged if key.keycode == keycode::RIGHT_OPTION => {
                    key.flags & flag::RIGHT_OPTION_DEVICE != 0
                }
                _ => return None,
            },
            Target::Chord {
                modifiers,
                needs_fn,
                keycode,
            } => {
                let modifiers_held = key.flags & CHORD_MODIFIERS == *modifiers
                    && (!needs_fn || key.flags & flag::FUNCTION != 0);
                match key.kind {
                    Kind::KeyDown if key.keycode == *keycode && !key.repeat && modifiers_held => {
                        true
                    }
                    Kind::KeyUp if key.keycode == *keycode => false,
                    // Letting go of a modifier ends the hold too.
                    Kind::FlagsChanged if self.held && key.flags & modifiers != *modifiers => false,
                    _ => return None,
                }
            }
        };
        match (down, self.held) {
            (true, false) => {
                self.held = true;
                Some(Event::KeyDown)
            }
            (false, true) => {
                self.held = false;
                Some(Event::KeyUp)
            }
            _ => None,
        }
    }

    /// The OS stopped delivering keys for a while (the tap was disabled, for example by secure
    /// input), so a release may have been missed. A dictation in progress is cancelled.
    pub fn reset(&mut self) -> Option<Event> {
        std::mem::take(&mut self.held).then_some(Event::Esc)
    }
}

/// The virtual key code of a chord key name: letters, digits, punctuation, space, return,
/// tab, delete and F1 to F12 on an ANSI layout.
pub fn keycode_named(name: &str) -> Option<u16> {
    const KEYS: &[(&str, u16)] = &[
        ("a", 0x00),
        ("s", 0x01),
        ("d", 0x02),
        ("f", 0x03),
        ("h", 0x04),
        ("g", 0x05),
        ("z", 0x06),
        ("x", 0x07),
        ("c", 0x08),
        ("v", 0x09),
        ("b", 0x0B),
        ("q", 0x0C),
        ("w", 0x0D),
        ("e", 0x0E),
        ("r", 0x0F),
        ("y", 0x10),
        ("t", 0x11),
        ("1", 0x12),
        ("2", 0x13),
        ("3", 0x14),
        ("4", 0x15),
        ("6", 0x16),
        ("5", 0x17),
        ("=", 0x18),
        ("9", 0x19),
        ("7", 0x1A),
        ("-", 0x1B),
        ("8", 0x1C),
        ("0", 0x1D),
        ("]", 0x1E),
        ("o", 0x1F),
        ("u", 0x20),
        ("[", 0x21),
        ("i", 0x22),
        ("p", 0x23),
        ("return", 0x24),
        ("l", 0x25),
        ("j", 0x26),
        ("'", 0x27),
        ("k", 0x28),
        (";", 0x29),
        ("\\", 0x2A),
        (",", 0x2B),
        ("/", 0x2C),
        ("n", 0x2D),
        ("m", 0x2E),
        (".", 0x2F),
        ("tab", 0x30),
        ("space", 0x31),
        ("`", 0x32),
        ("delete", 0x33),
        ("f1", 0x7A),
        ("f2", 0x78),
        ("f3", 0x63),
        ("f4", 0x76),
        ("f5", 0x60),
        ("f6", 0x61),
        ("f7", 0x62),
        ("f8", 0x64),
        ("f9", 0x65),
        ("f10", 0x6D),
        ("f11", 0x67),
        ("f12", 0x6F),
    ];
    KEYS.iter().find(|(n, _)| *n == name).map(|(_, code)| *code)
}
