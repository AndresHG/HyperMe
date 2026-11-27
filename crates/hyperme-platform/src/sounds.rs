//! The start and stop sounds on macOS (R-32): two of the system's own short sounds, played
//! softly through `NSSound`, so HyperMe ships no audio files.

use hyperme_core::traits::{Sound, Sounds};
use objc2_app_kit::NSSound;
use objc2_foundation::NSString;

/// Plays "Tink" when recording starts and "Pop" when it stops, at reduced volume.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemSounds;

/// The system sound for each event, from `/System/Library/Sounds`.
pub fn sound_name(sound: Sound) -> &'static str {
    match sound {
        Sound::Start => "Tink",
        Sound::Stop => "Pop",
    }
}

/// How long the start sound plays: "Tink" is about 0.1 s long, with a margin. Muting the output
/// waits for it (#221).
pub const START_SOUND: std::time::Duration = std::time::Duration::from_millis(150);

/// Soft, as the owner asked (U-5).
const VOLUME: f32 = 0.35;

impl Sounds for SystemSounds {
    fn play(&self, sound: Sound) {
        if let Some(player) = NSSound::soundNamed(&NSString::from_str(sound_name(sound))) {
            // A sound still playing from the last dictation is restarted.
            player.stop();
            player.setVolume(VOLUME);
            player.play();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R-32: two different sounds, both present on every Mac.
    #[test]
    fn start_and_stop_use_different_system_sounds() {
        assert_ne!(sound_name(Sound::Start), sound_name(Sound::Stop));
        for sound in [Sound::Start, Sound::Stop] {
            let path = format!("/System/Library/Sounds/{}.aiff", sound_name(sound));
            assert!(std::path::Path::new(&path).exists(), "{path}");
        }
    }
}
