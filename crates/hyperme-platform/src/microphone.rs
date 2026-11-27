//! Microphone capture with `cpal`: the chosen input device, mixed to mono, resampled to the
//! speech provider's rate and cut into 20 ms frames, each sent to the state machine as
//! `Event::Audio` (design.md, "Audio"). `cpal` covers macOS (Core Audio), Linux and Windows.
//!
//! The input stream is built on the first `start`, then resumed on each `start` and paused on
//! each `stop` (the owner's option B, 26 November 2026): resuming is faster than building, and a
//! paused stream does not hold the device, so the microphone is in use only while the user
//! dictates. The stream is rebuilt when the chosen device changes or the stream reports an
//! error. Start latency is in `docs/results/2026-11-26-mic-start-latency.md`.
//!
//! Which device (#221): the first of the user's preferred microphones that is connected, else
//! the system default (`hyperme_core::microphones::pick`). With no preference, start costs what
//! it did before: no device list is read. When the device in use goes away mid-dictation, the
//! stream's error callback starts a short-lived thread that moves capture to the next choice,
//! and the dictation goes on (AC-4). Nothing polls: idle CPU stays at zero.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use hyperme_core::audio::{Framer, Resampler, frame_samples, mix_to_mono};
use hyperme_core::dictation::Event;
use hyperme_core::microphones::{Preference, pick};
use hyperme_core::traits::{Error, ErrorKind, Microphone};
use tokio::sync::mpsc;

/// The chosen microphone, delivering frames at `rate` to the state machine.
pub struct CpalMicrophone {
    core: Arc<Core>,
}

/// Shared with the stream's error callback, which moves a dictation to another device.
struct Core {
    events: mpsc::Sender<Event>,
    rate: u32,
    preference: Preference,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// Built on the first start, paused between dictations. Dropping it releases the device.
    stream: Option<Built>,
    /// Between a start and a stop.
    recording: bool,
}

struct Built {
    stream: cpal::Stream,
    /// The device it was built for, to notice when the choice changes.
    device: Option<cpal::DeviceId>,
    /// Its name, which a preference names and a rescue skips.
    name: String,
    shared: Arc<Shared>,
}

/// Flags shared with the audio callback.
#[derive(Debug, Default)]
struct Shared {
    /// Set on each start: the callback drops audio left from the last dictation.
    reset: AtomicBool,
    /// Set by the stream's error callback, for example when the device goes away.
    broken: AtomicBool,
}

impl fmt::Debug for CpalMicrophone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CpalMicrophone")
            .field("rate", &self.core.rate)
            .field("built", &self.core.lock().stream.is_some())
            .finish()
    }
}

/// Tries to find another microphone after one goes away: macOS changes its default input a
/// moment after the old device reports itself gone.
const RESCUE_TRIES: u32 = 6;
const RESCUE_WAIT: Duration = Duration::from_millis(50);

/// A device's name as the system shows it, which Settings lists and saves.
fn device_name(device: &cpal::Device) -> String {
    device
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_default()
}

/// Starts Core Audio in this process on a short-lived thread, so the first dictation after
/// launch does not pay for it on key-down: the first default-device query costs about 89 ms
/// cold and 18 µs warm (docs/results/2026-11-27-mic-choice-cost.md, #250). It reads device
/// properties only: no device is opened, so the microphone indicator stays off and macOS asks
/// for no permission.
pub fn warm_up() {
    let _ = std::thread::Builder::new()
        .name("hyperme-audio-warm-up".into())
        .spawn(|| {
            let _ = cpal::default_host()
                .default_input_device()
                .map(|device| device.id());
        });
}

/// The names of the connected microphones, for Settings, and the system default's name.
pub fn microphones() -> (Vec<String>, Option<String>) {
    let host = cpal::default_host();
    let names = host
        .input_devices()
        .map(|devices| devices.map(|d| device_name(&d)).collect())
        .unwrap_or_default();
    (names, host.default_input_device().map(|d| device_name(&d)))
}

/// The device a dictation uses: the first connected preference, else the system default.
/// `failed` is a device that just went away and may still be listed for a moment.
fn choose(preferred: &[String], failed: Option<&str>) -> Result<cpal::Device, Error> {
    let host = cpal::default_host();
    let mut listed: Option<Vec<cpal::Device>> = None;
    if !preferred.is_empty() || failed.is_some() {
        let devices: Vec<cpal::Device> = host
            .input_devices()
            .map(|d| d.collect())
            .unwrap_or_default();
        let names: Vec<String> = devices.iter().map(device_name).collect();
        if let Some(index) = pick(preferred, &names, failed) {
            return Ok(devices
                .into_iter()
                .nth(index)
                .expect("index from the same list"));
        }
        listed = Some(devices);
    }
    let default = host
        .default_input_device()
        .ok_or_else(|| device_error("no microphone found"))?;
    match failed {
        Some(failed) if device_name(&default) == failed => {
            // The default has not moved off the device that went away yet: any other input.
            listed
                .unwrap_or_default()
                .into_iter()
                .find(|d| device_name(d) != failed)
                .ok_or_else(|| device_error("no other microphone found"))
        }
        _ => Ok(default),
    }
}

impl CpalMicrophone {
    /// `rate` is the speech provider's sample rate; frames go to `events`. The system default
    /// microphone, as in the walkthrough's test.
    pub fn new(events: mpsc::Sender<Event>, rate: u32) -> Self {
        Self::with_preference(events, rate, Preference::default())
    }

    /// As `new`, with the user's preferred microphones, which Settings may change at any time.
    pub fn with_preference(events: mpsc::Sender<Event>, rate: u32, preference: Preference) -> Self {
        CpalMicrophone {
            core: Arc::new(Core {
                events,
                rate,
                preference,
                state: Mutex::default(),
            }),
        }
    }
}

impl Core {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn build(self: &Arc<Self>, device: &cpal::Device) -> Result<Built, Error> {
        let supported = device
            .default_input_config()
            .map_err(|e| device_error(format!("could not read the microphone format: {e}")))?;
        let config = supported.config();
        let shared = Arc::new(Shared::default());
        let pipeline = Pipeline::new(
            supported.sample_rate(),
            self.rate,
            config.channels,
            self.events.clone(),
            shared.clone(),
        );
        let core = Arc::downgrade(self);
        let stream = match supported.sample_format() {
            SampleFormat::F32 => {
                input_stream::<f32>(device, &config, pipeline, shared.clone(), core)
            }
            SampleFormat::I16 => {
                input_stream::<i16>(device, &config, pipeline, shared.clone(), core)
            }
            SampleFormat::I32 => {
                input_stream::<i32>(device, &config, pipeline, shared.clone(), core)
            }
            other => {
                return Err(device_error(format!(
                    "unsupported microphone sample format: {other}"
                )));
            }
        }
        .map_err(|e| device_error(format!("could not open the microphone: {e}")))?;
        Ok(Built {
            stream,
            device: device.id().ok(),
            name: device_name(device),
            shared,
        })
    }

    /// AC-4: the stream behind `shared` broke. If it is the one a dictation is using, capture
    /// moves to the next choice, skipping the device when it went away (`gone`); a device that
    /// is still there (its sample rate changed, say) is opened again. Between dictations nothing
    /// happens: the next start rebuilds.
    fn rescue(self: &Arc<Self>, shared: &Arc<Shared>, gone: bool) {
        let mut failed: Option<String> = None;
        for _ in 0..RESCUE_TRIES {
            let mut state = self.lock();
            if !state.recording {
                return;
            }
            match &state.stream {
                Some(built) if Arc::ptr_eq(&built.shared, shared) => {
                    failed = Some(built.name.clone());
                    // Release the device that went away.
                    state.stream = None;
                }
                // Another stream took over (a stop and a start meanwhile).
                Some(_) => return,
                None => {}
            }
            let Some(old) = failed.as_deref() else { return };
            let next = choose(&self.preference.get(), gone.then_some(old))
                .and_then(|device| self.build(&device))
                .and_then(|built| match built.stream.play() {
                    Ok(()) => Ok(built),
                    Err(e) => Err(device_error(format!("could not start the microphone: {e}"))),
                });
            match next {
                Ok(built) => {
                    // Device names are not dictation content, so they are safe to log.
                    tracing::warn!("microphone: {old} stopped; recording from {}", built.name);
                    state.stream = Some(built);
                    return;
                }
                Err(e) => {
                    drop(state);
                    tracing::warn!("microphone: no replacement yet: {}", e.message);
                    std::thread::sleep(RESCUE_WAIT);
                }
            }
        }
    }
}

fn device_error(what: impl fmt::Display) -> Error {
    Error {
        kind: ErrorKind::Device,
        message: what.to_string(),
    }
}

impl Microphone for CpalMicrophone {
    fn start(&mut self) -> Result<(), Error> {
        let core = &self.core;
        let preferred = core.preference.get();
        let mut state = core.lock();
        let healthy = |built: &&Built| !built.shared.broken.load(Ordering::SeqCst);
        // Listing the devices costs 2 to 8 ms (docs/results/2026-11-27-mic-choice-cost.md), so
        // a stream on the first preference, which nothing can beat, is reused without it.
        let on_first = state
            .stream
            .as_ref()
            .filter(healthy)
            .is_some_and(|built| preferred.first() == Some(&built.name));
        if !on_first {
            let device = choose(&preferred, None)?;
            let reusable =
                state.stream.as_ref().filter(healthy).is_some_and(|built| {
                    built.device.is_some() && built.device == device.id().ok()
                });
            if !reusable {
                state.stream = None;
                state.stream = Some(core.build(&device)?);
            }
        }
        let Some(built) = &state.stream else {
            return Err(device_error("the microphone stream is missing"));
        };
        built.shared.reset.store(true, Ordering::SeqCst);
        if let Err(e) = built.stream.play() {
            state.stream = None;
            return Err(device_error(format!("could not start the microphone: {e}")));
        }
        state.recording = true;
        Ok(())
    }

    fn stop(&mut self) {
        let mut state = self.core.lock();
        state.recording = false;
        // Up to 20 ms of audio still in the framer is dropped by the next start's reset.
        // A stream that cannot pause would keep the microphone in use: release it.
        if let Some(built) = &state.stream
            && built.stream.pause().is_err()
        {
            state.stream = None;
        }
    }
}

/// Runs in the audio callback: mono, resample, frame, send. Never blocks.
struct Pipeline {
    channels: usize,
    resampler: Resampler,
    framer: Framer,
    resampled: Vec<i16>,
    events: mpsc::Sender<Event>,
    shared: Arc<Shared>,
    device_rate: u32,
    rate: u32,
}

impl Pipeline {
    fn new(
        device_rate: u32,
        rate: u32,
        channels: u16,
        events: mpsc::Sender<Event>,
        shared: Arc<Shared>,
    ) -> Self {
        Pipeline {
            channels: usize::from(channels),
            resampler: Resampler::new(device_rate, rate),
            framer: Framer::new(frame_samples(rate)),
            resampled: Vec::new(),
            events,
            shared,
            device_rate,
            rate,
        }
    }

    fn push(&mut self, interleaved: &[f32]) {
        if self.shared.reset.swap(false, Ordering::SeqCst) {
            self.resampler = Resampler::new(self.device_rate, self.rate);
            self.framer = Framer::new(frame_samples(self.rate));
        }
        self.resampled.clear();
        self.resampler.push(
            &mix_to_mono(interleaved, self.channels),
            &mut self.resampled,
        );
        let events = &self.events;
        self.framer.push(&self.resampled, |frame| {
            // A full channel means the state machine is seconds behind; dropping a frame is
            // better than blocking the audio thread.
            let _ = events.try_send(Event::Audio(frame));
        });
    }
}

fn input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut pipeline: Pipeline,
    shared: Arc<Shared>,
    core: Weak<Core>,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let mut block = Vec::new();
    device.build_input_stream(
        *config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            block.clear();
            block.extend(data.iter().map(|&s| f32::from_sample(s)));
            pipeline.push(&block);
        },
        // The error never carries audio, so it is safe to log.
        move |err| {
            tracing::warn!("microphone: {err}");
            if needs_rebuild(err.kind()) && !shared.broken.swap(true, Ordering::SeqCst) {
                // Not on Core Audio's thread: the rescue takes locks and builds a stream.
                if let Some(core) = core.upgrade() {
                    let shared = shared.clone();
                    let gone = err.kind() == cpal::ErrorKind::DeviceNotAvailable;
                    std::thread::spawn(move || core.rescue(&shared, gone));
                }
            }
        },
        None,
    )
}

/// Whether a stream error means the stream must be rebuilt. A glitch (an overrun, which macOS
/// reports when a stream starts) or a followed route change does not.
fn needs_rebuild(kind: cpal::ErrorKind) -> bool {
    !matches!(
        kind,
        cpal::ErrorKind::Xrun | cpal::ErrorKind::DeviceChanged | cpal::ErrorKind::RealtimeDenied
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The engine requires `Microphone + Send` so the app can spawn it.
    #[test]
    fn microphone_is_send() {
        fn send<T: Send>() {}
        send::<CpalMicrophone>();
    }

    #[test]
    fn pipeline_sends_20_ms_frames_at_the_provider_rate() {
        let (tx, mut rx) = mpsc::channel(16);
        // 48 kHz stereo in, 24 kHz mono out: 40 ms of input is two 480-sample frames.
        let mut pipeline = Pipeline::new(48_000, 24_000, 2, tx, Arc::default());
        pipeline.push(&vec![0.5f32; 48 * 40 * 2]);
        let mut frames = Vec::new();
        while let Ok(Event::Audio(frame)) = rx.try_recv() {
            frames.push(frame);
        }
        assert_eq!(frames.len(), 2);
        assert!(
            frames
                .iter()
                .all(|f| f.len() == 480 && f.iter().all(|&s| (s - 16_383).abs() <= 1))
        );
    }

    /// Option B: the stream is paused between dictations, not rebuilt, so audio left in the
    /// resampler and framer from the last dictation must not leak into the next one.
    #[test]
    fn a_reset_drops_audio_left_from_the_last_dictation() {
        let (tx, mut rx) = mpsc::channel(16);
        let shared = Arc::new(Shared::default());
        let mut pipeline = Pipeline::new(24_000, 24_000, 1, tx, shared.clone());
        pipeline.push(&vec![0.25f32; 300]);
        assert!(rx.try_recv().is_err(), "300 samples are less than a frame");
        shared.reset.store(true, Ordering::SeqCst);
        // The resampler holds back one sample at an equal rate, so send a little more.
        pipeline.push(&vec![0.5f32; 600]);
        let Ok(Event::Audio(frame)) = rx.try_recv() else {
            panic!("one full frame after the reset");
        };
        assert!(
            frame.iter().all(|&s| (s - 16_383).abs() <= 1),
            "old audio leaked into the frame"
        );
        assert!(rx.try_recv().is_err());
    }

    /// A glitch must not force a rebuild on the next start, which would cost the latency that
    /// play and pause save; a lost device must.
    #[test]
    fn only_lasting_stream_errors_force_a_rebuild() {
        assert!(!needs_rebuild(cpal::ErrorKind::Xrun));
        assert!(!needs_rebuild(cpal::ErrorKind::DeviceChanged));
        assert!(needs_rebuild(cpal::ErrorKind::DeviceNotAvailable));
        assert!(needs_rebuild(cpal::ErrorKind::StreamInvalidated));
    }

    /// #250: launch never waits for Core Audio to start; the warm-up runs on its own thread.
    #[test]
    fn warming_up_returns_at_once() {
        let started = std::time::Instant::now();
        warm_up();
        assert!(started.elapsed() < Duration::from_millis(20));
    }

    #[test]
    fn a_full_channel_drops_frames_instead_of_blocking() {
        let (tx, mut rx) = mpsc::channel(1);
        let mut pipeline = Pipeline::new(24_000, 24_000, 1, tx, Arc::default());
        pipeline.push(&vec![0.0f32; 480 * 5]);
        assert!(rx.try_recv().is_ok());
        assert!(rx.try_recv().is_err());
    }
}
