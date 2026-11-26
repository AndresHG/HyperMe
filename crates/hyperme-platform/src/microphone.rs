//! Microphone capture with `cpal`: the default input device, mixed to mono, resampled to the
//! speech provider's rate and cut into 20 ms frames, each sent to the state machine as
//! `Event::Audio` (design.md, "Audio"). `cpal` covers macOS (Core Audio), Linux and Windows.
//!
//! The input stream is built on the first `start`, then resumed on each `start` and paused on
//! each `stop` (the owner's option B, 26 November 2026): resuming is faster than building, and a
//! paused stream does not hold the device, so the microphone is in use only while the user
//! dictates. The stream is rebuilt when the default input device changes or the stream reports
//! an error. Start latency is in `docs/results/2026-11-26-mic-start-latency.md`.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use hyperme_core::audio::{Framer, Resampler, frame_samples, mix_to_mono};
use hyperme_core::dictation::Event;
use hyperme_core::traits::{Error, ErrorKind, Microphone};
use tokio::sync::mpsc;

/// The default microphone, delivering frames at `rate` to the state machine.
pub struct CpalMicrophone {
    events: mpsc::Sender<Event>,
    rate: u32,
    /// Built on the first start, paused between dictations. Dropping it releases the device.
    stream: Option<Built>,
}

struct Built {
    stream: cpal::Stream,
    /// The device it was built for, to notice when the default input changes.
    device: Option<cpal::DeviceId>,
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
            .field("rate", &self.rate)
            .field("built", &self.stream.is_some())
            .finish()
    }
}

impl CpalMicrophone {
    /// `rate` is the speech provider's sample rate; frames go to `events`.
    pub fn new(events: mpsc::Sender<Event>, rate: u32) -> Self {
        CpalMicrophone {
            events,
            rate,
            stream: None,
        }
    }

    fn build(&self, device: &cpal::Device) -> Result<Built, Error> {
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
        let stream = match supported.sample_format() {
            SampleFormat::F32 => input_stream::<f32>(device, &config, pipeline, shared.clone()),
            SampleFormat::I16 => input_stream::<i16>(device, &config, pipeline, shared.clone()),
            SampleFormat::I32 => input_stream::<i32>(device, &config, pipeline, shared.clone()),
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
            shared,
        })
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
        let device = cpal::default_host()
            .default_input_device()
            .ok_or_else(|| device_error("no microphone found"))?;
        let reusable = self.stream.as_ref().is_some_and(|built| {
            !built.shared.broken.load(Ordering::SeqCst)
                && built.device.is_some()
                && built.device == device.id().ok()
        });
        if !reusable {
            self.stream = None;
            self.stream = Some(self.build(&device)?);
        }
        let Some(built) = &self.stream else {
            return Err(device_error("the microphone stream is missing"));
        };
        built.shared.reset.store(true, Ordering::SeqCst);
        if let Err(e) = built.stream.play() {
            self.stream = None;
            return Err(device_error(format!("could not start the microphone: {e}")));
        }
        Ok(())
    }

    fn stop(&mut self) {
        // Up to 20 ms of audio still in the framer is dropped by the next start's reset.
        if let Some(built) = &self.stream {
            if built.stream.pause().is_err() {
                // A stream that cannot pause would keep the microphone in use: release it.
                self.stream = None;
            }
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
        // The error never carries audio, so it is safe to print. The next start rebuilds.
        move |err| {
            eprintln!("microphone: {err}");
            if needs_rebuild(err.kind()) {
                shared.broken.store(true, Ordering::SeqCst);
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

    #[test]
    fn a_full_channel_drops_frames_instead_of_blocking() {
        let (tx, mut rx) = mpsc::channel(1);
        let mut pipeline = Pipeline::new(24_000, 24_000, 1, tx, Arc::default());
        pipeline.push(&vec![0.0f32; 480 * 5]);
        assert!(rx.try_recv().is_ok());
        assert!(rx.try_recv().is_err());
    }
}
