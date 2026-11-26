//! Microphone capture with `cpal`: the default input device, mixed to mono, resampled to the
//! speech provider's rate and cut into 20 ms frames, each sent to the state machine as
//! `Event::Audio` (design.md, "Audio"). `cpal` covers macOS (Core Audio), Linux and Windows.
//!
//! The stream is opened on `start` and closed on `stop`, so the microphone is only in use, and
//! the macOS indicator only lit, while the user dictates.

use std::fmt;

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
    /// Open while recording. Dropping it stops the capture.
    stream: Option<cpal::Stream>,
}

impl fmt::Debug for CpalMicrophone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CpalMicrophone")
            .field("rate", &self.rate)
            .field("recording", &self.stream.is_some())
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
}

fn device_error(what: impl fmt::Display) -> Error {
    Error {
        kind: ErrorKind::Device,
        message: what.to_string(),
    }
}

impl Microphone for CpalMicrophone {
    fn start(&mut self) -> Result<(), Error> {
        self.stream = None;
        let device = cpal::default_host()
            .default_input_device()
            .ok_or_else(|| device_error("no microphone found"))?;
        let supported = device
            .default_input_config()
            .map_err(|e| device_error(format!("could not read the microphone format: {e}")))?;
        let config = supported.config();
        let pipeline = Pipeline::new(
            supported.sample_rate(),
            self.rate,
            config.channels,
            self.events.clone(),
        );
        let stream = match supported.sample_format() {
            SampleFormat::F32 => input_stream::<f32>(&device, &config, pipeline),
            SampleFormat::I16 => input_stream::<i16>(&device, &config, pipeline),
            SampleFormat::I32 => input_stream::<i32>(&device, &config, pipeline),
            other => {
                return Err(device_error(format!(
                    "unsupported microphone sample format: {other}"
                )));
            }
        }
        .map_err(|e| device_error(format!("could not open the microphone: {e}")))?;
        stream
            .play()
            .map_err(|e| device_error(format!("could not start the microphone: {e}")))?;
        self.stream = Some(stream);
        Ok(())
    }

    fn stop(&mut self) {
        // Up to 20 ms of audio still in the framer is dropped with the stream.
        self.stream = None;
    }
}

/// Runs in the audio callback: mono, resample, frame, send. Never blocks.
struct Pipeline {
    channels: usize,
    resampler: Resampler,
    framer: Framer,
    resampled: Vec<i16>,
    events: mpsc::Sender<Event>,
}

impl Pipeline {
    fn new(device_rate: u32, rate: u32, channels: u16, events: mpsc::Sender<Event>) -> Self {
        Pipeline {
            channels: usize::from(channels),
            resampler: Resampler::new(device_rate, rate),
            framer: Framer::new(frame_samples(rate)),
            resampled: Vec::new(),
            events,
        }
    }

    fn push(&mut self, interleaved: &[f32]) {
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
        // The error never carries audio, so it is safe to print.
        |err| eprintln!("microphone: {err}"),
        None,
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
        let mut pipeline = Pipeline::new(48_000, 24_000, 2, tx);
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

    #[test]
    fn a_full_channel_drops_frames_instead_of_blocking() {
        let (tx, mut rx) = mpsc::channel(1);
        let mut pipeline = Pipeline::new(24_000, 24_000, 1, tx);
        pipeline.push(&vec![0.0f32; 480 * 5]);
        assert!(rx.try_recv().is_ok());
        assert!(rx.try_recv().is_err());
    }
}
