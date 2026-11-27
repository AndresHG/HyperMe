//! Microphone handling (#221): which microphone a dictation uses, and muting the output while
//! the user dictates. Pure code with the platform behind small traits, so the rules are tested
//! without audio hardware.
//!
//! - The user orders preferred microphones by name. Each dictation uses the first one that is
//!   connected; with none connected, or no preference, the system default (AC-1, AC-2, AC-3).
//! - When the microphone in use goes away mid-dictation, capture moves to the next choice that
//!   is not the one that failed (AC-4). A microphone that arrives mid-dictation waits for the
//!   next dictation (AC-5): switching would cut a gap into the recording.
//! - With "Mute audio while dictating" on, the output is muted while recording and given back
//!   as it was (AC-7). The mute waits for the start sound to finish, and is lifted before the
//!   stop sound, so both stay audible.

use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::thread;
use std::time::Duration;

use crate::traits::{Error, Microphone};

/// The microphone to use among `available` (names of the connected inputs, in the system's
/// order): the first of `preferred` that is connected and is not `failed`. `None` means the
/// system default. Names compare exactly: they are what the system shows and what Settings saved.
pub fn pick(preferred: &[String], available: &[String], failed: Option<&str>) -> Option<usize> {
    preferred
        .iter()
        .filter(|name| Some(name.as_str()) != failed)
        .find_map(|name| available.iter().position(|a| a == name))
}

/// The user's order of preferred microphones, shared between Settings and the microphone, so a
/// change reaches the next dictation without a restart.
#[derive(Debug, Clone, Default)]
pub struct Preference(Arc<RwLock<Vec<String>>>);

impl Preference {
    pub fn new(names: Vec<String>) -> Self {
        Preference(Arc::new(RwLock::new(names)))
    }

    pub fn get(&self) -> Vec<String> {
        self.0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn set(&self, names: Vec<String>) {
        *self.0.write().unwrap_or_else(PoisonError::into_inner) = names;
    }
}

/// Mutes and unmutes the system's output (macOS: the default output device).
pub trait OutputMute: Send + 'static {
    /// Mutes the output. True when this call muted it; false when it was already muted or
    /// cannot be muted, and then `unmute` must not be called for it.
    fn mute(&mut self) -> bool;
    /// Gives back what the last successful `mute` took.
    fn unmute(&mut self);
}

/// The settings for muting, shared with Settings so a change applies to the next dictation.
#[derive(Debug, Clone)]
pub struct MuteSettings {
    on: Arc<std::sync::atomic::AtomicBool>,
}

impl MuteSettings {
    pub fn new(on: bool) -> Self {
        MuteSettings {
            on: Arc::new(on.into()),
        }
    }

    pub fn set(&self, on: bool) {
        self.on.store(on, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn is_on(&self) -> bool {
        self.on.load(std::sync::atomic::Ordering::Relaxed)
    }
}

struct MuteState<O> {
    output: O,
    /// Bumped on every start and stop, so a delayed mute from an earlier dictation does nothing.
    generation: u64,
    muted: bool,
}

/// Runs a job after a delay. The app uses a short-lived thread; tests run the job by hand, so no
/// test depends on how fast a machine is.
type Timer = Arc<dyn Fn(Duration, Box<dyn FnOnce() + Send>) + Send + Sync>;

fn thread_timer() -> Timer {
    Arc::new(|delay, job| {
        thread::spawn(move || {
            thread::sleep(delay);
            job();
        });
    })
}

/// A microphone that mutes the output while it records (AC-7). The engine sees a plain
/// `Microphone`, so the dictation state machine does not change.
pub struct Muting<M, O: OutputMute> {
    mic: M,
    settings: MuteSettings,
    /// How long the start sound plays; the mute waits for it. Zero mutes at once.
    delay: Duration,
    timer: Timer,
    state: Arc<Mutex<MuteState<O>>>,
}

impl<M, O: OutputMute> std::fmt::Debug for Muting<M, O> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Muting")
            .field("on", &self.settings.is_on())
            .field("delay", &self.delay)
            .finish()
    }
}

impl<M, O: OutputMute> Muting<M, O> {
    pub fn new(mic: M, output: O, settings: MuteSettings, delay: Duration) -> Self {
        Muting {
            mic,
            settings,
            delay,
            timer: thread_timer(),
            state: Arc::new(Mutex::new(MuteState {
                output,
                generation: 0,
                muted: false,
            })),
        }
    }

    #[cfg(test)]
    fn with_timer(mut self, timer: Timer) -> Self {
        self.timer = timer;
        self
    }

    fn lock(state: &Mutex<MuteState<O>>) -> std::sync::MutexGuard<'_, MuteState<O>> {
        state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn release(&self) {
        let mut state = Self::lock(&self.state);
        state.generation += 1;
        if state.muted {
            state.muted = false;
            state.output.unmute();
        }
    }
}

impl<M: Microphone, O: OutputMute> Microphone for Muting<M, O> {
    fn start(&mut self) -> Result<(), Error> {
        self.mic.start()?;
        let generation = {
            let mut state = Self::lock(&self.state);
            state.generation += 1;
            state.generation
        };
        if !self.settings.is_on() {
            return Ok(());
        }
        let mute = move |state: &Mutex<MuteState<O>>| {
            let mut state = Self::lock(state);
            if state.generation == generation && !state.muted {
                state.muted = state.output.mute();
            }
        };
        if self.delay.is_zero() {
            mute(&self.state);
        } else {
            // Key-down never waits for the start sound.
            let state = Arc::clone(&self.state);
            (self.timer)(self.delay, Box::new(move || mute(&state)));
        }
        Ok(())
    }

    fn stop(&mut self) {
        self.mic.stop();
        self.release();
    }
}

impl<M, O: OutputMute> Drop for Muting<M, O> {
    /// Quitting mid-dictation must not leave the Mac muted.
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ErrorKind;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    /// AC-1: no preference is the system default.
    #[test]
    fn no_preference_uses_the_system_default() {
        assert_eq!(pick(&[], &names(&["MacBook Pro Microphone"]), None), None);
    }

    /// AC-2, AC-3: the first preferred microphone that is connected wins; one that is not
    /// connected is skipped, and with none connected it is the system default.
    #[test]
    fn the_first_connected_preference_wins() {
        let preferred = names(&["Shure MV7", "AirPods Pro", "MacBook Pro Microphone"]);
        let available = names(&["MacBook Pro Microphone", "AirPods Pro"]);
        assert_eq!(pick(&preferred, &available, None), Some(1));
        assert_eq!(pick(&preferred, &names(&["Zoom H1"]), None), None);
    }

    /// AC-4: after the microphone in use fails, the next choice is another one, even while the
    /// failed one is still listed.
    #[test]
    fn a_failed_microphone_is_skipped() {
        let preferred = names(&["Shure MV7", "MacBook Pro Microphone"]);
        let available = names(&["Shure MV7", "MacBook Pro Microphone"]);
        assert_eq!(pick(&preferred, &available, Some("Shure MV7")), Some(1));
    }

    #[derive(Default)]
    struct FakeMic {
        fail: bool,
    }

    impl Microphone for FakeMic {
        fn start(&mut self) -> Result<(), Error> {
            if self.fail {
                return Err(Error {
                    kind: ErrorKind::Device,
                    message: "no microphone found".into(),
                });
            }
            Ok(())
        }
        fn stop(&mut self) {}
    }

    /// The output's mute state, shared with the test.
    #[derive(Clone, Default)]
    struct FakeOutput {
        muted: Arc<AtomicBool>,
    }

    impl OutputMute for FakeOutput {
        fn mute(&mut self) -> bool {
            !self.muted.swap(true, Ordering::SeqCst)
        }
        fn unmute(&mut self) {
            self.muted.store(false, Ordering::SeqCst);
        }
    }

    fn muting(on: bool, delay: Duration) -> (Muting<FakeMic, FakeOutput>, Arc<AtomicBool>) {
        let output = FakeOutput::default();
        let muted = output.muted.clone();
        let mic = Muting::new(FakeMic::default(), output, MuteSettings::new(on), delay);
        (mic, muted)
    }

    /// AC-7: muted while recording, given back at the stop.
    #[test]
    fn the_output_is_muted_while_recording() {
        let (mut mic, muted) = muting(true, Duration::ZERO);
        mic.start().unwrap();
        assert!(muted.load(Ordering::SeqCst));
        mic.stop();
        assert!(!muted.load(Ordering::SeqCst));
    }

    /// AC-7: an output the user had muted stays muted after the dictation.
    #[test]
    fn an_output_already_muted_stays_muted() {
        let (mut mic, muted) = muting(true, Duration::ZERO);
        muted.store(true, Ordering::SeqCst);
        mic.start().unwrap();
        mic.stop();
        assert!(muted.load(Ordering::SeqCst));
    }

    /// AC-7: off leaves the output alone; a failed start mutes nothing.
    #[test]
    fn off_or_a_failed_start_leaves_the_output_alone() {
        let (mut mic, muted) = muting(false, Duration::ZERO);
        mic.start().unwrap();
        assert!(!muted.load(Ordering::SeqCst));
        let (mut mic, muted) = muting(true, Duration::ZERO);
        mic.mic.fail = true;
        assert!(mic.start().is_err());
        assert!(!muted.load(Ordering::SeqCst));
    }

    /// AC-7: the mute waits for the start sound, and a dictation that ends first (a tap) never
    /// mutes, so a late mute cannot leave the Mac silent.
    #[test]
    fn a_tap_shorter_than_the_delay_never_mutes() {
        // The delayed jobs wait here until the test runs them: no real sleeps (regression: the
        // sleeping version failed on a slow CI runner, PR 254).
        type Jobs = Arc<Mutex<Vec<(Duration, Box<dyn FnOnce() + Send>)>>>;
        let jobs: Jobs = Arc::default();
        let queue = jobs.clone();
        let (mic, muted) = muting(true, Duration::from_millis(250));
        let mut mic = mic.with_timer(Arc::new(move |delay, job| {
            queue.lock().unwrap().push((delay, job));
        }));
        let run_due = || {
            let due: Vec<_> = jobs.lock().unwrap().drain(..).collect();
            for (delay, job) in due {
                assert_eq!(delay, Duration::from_millis(250));
                job();
            }
        };

        mic.start().unwrap();
        assert!(
            !muted.load(Ordering::SeqCst),
            "muted before the start sound ended"
        );
        mic.stop();
        run_due();
        assert!(!muted.load(Ordering::SeqCst), "a late mute after a tap");

        mic.start().unwrap();
        run_due();
        assert!(muted.load(Ordering::SeqCst));
        mic.stop();
        assert!(!muted.load(Ordering::SeqCst));
    }

    /// Quitting mid-dictation gives the sound back.
    #[test]
    fn dropping_mid_dictation_unmutes() {
        let (mut mic, muted) = muting(true, Duration::ZERO);
        mic.start().unwrap();
        drop(mic);
        assert!(!muted.load(Ordering::SeqCst));
    }
}
