//! The microphone: try the streaming pipeline by voice, or record a sample.

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::time::Instant;

use crate::audio::{self, CHUNK_SAMPLES, Resampler, SAMPLE_RATE};
use crate::cleanup::CleanupProvider;
use crate::pipeline::{self, Marks, Measured, Pipeline, SampleResult};
use crate::report;
use crate::stt::{LiveSession, SpeechProvider};

/// Below this peak a recording is treated as silence: usually a missing permission.
const SILENCE_PEAK: u16 = 200;

/// The default microphone, delivering mono blocks at the device's own sample rate.
struct Capture {
    /// Dropping the stream stops the capture.
    stream: cpal::Stream,
    blocks: UnboundedReceiver<Vec<f32>>,
    rate: u32,
}

impl Capture {
    fn start() -> Result<Capture> {
        let device = cpal::default_host()
            .default_input_device()
            .context("no microphone found")?;
        let supported = device
            .default_input_config()
            .context("reading the microphone format")?;
        let (tx, blocks) = unbounded_channel();
        let config = supported.config();
        let stream = match supported.sample_format() {
            SampleFormat::F32 => input_stream::<f32>(&device, config, tx),
            SampleFormat::I16 => input_stream::<i16>(&device, config, tx),
            SampleFormat::I32 => input_stream::<i32>(&device, config, tx),
            other => bail!("unsupported microphone sample format: {other}"),
        }
        .context("opening the microphone")?;
        stream.play().context("starting the microphone")?;
        Ok(Capture {
            stream,
            blocks,
            rate: supported.sample_rate(),
        })
    }
}

fn input_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    tx: UnboundedSender<Vec<f32>>,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels as usize;
    device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            // Mix the channels down to mono.
            let mono = data
                .chunks(channels)
                .map(|frame| {
                    frame.iter().map(|&s| f32::from_sample(s)).sum::<f32>() / frame.len() as f32
                })
                .collect();
            let _ = tx.send(mono);
        },
        |err| eprintln!("microphone: {err}"),
        None,
    )
}

/// The moment of each press of Enter on standard input.
fn enter_presses() -> UnboundedReceiver<Instant> {
    let (tx, rx) = unbounded_channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        while matches!(std::io::stdin().read_line(&mut line), Ok(n) if n > 0) {
            if tx.send(Instant::now()).is_err() {
                break;
            }
            line.clear();
        }
    });
    rx
}

/// Live mode: Enter is key-down, Enter again is key release. Runs pipeline B on each dictation
/// and prints its stage log and texts. No audio is written to disk.
pub async fn dictate<S: SpeechProvider, C: CleanupProvider>(
    stt: &S,
    llm: &C,
    deadline: Duration,
) -> Result<()> {
    let mut enter = enter_presses();
    for n in 1.. {
        println!("\nPress Enter to start dictating, Enter again to stop. Ctrl-C quits.");
        if enter.recv().await.is_none() {
            break;
        }
        let sample = format!("mic-{n:02}");
        let (audio_ms, outcome) = match dictate_once(stt, llm, deadline, &sample, &mut enter).await
        {
            Ok((audio_ms, measured)) => (audio_ms, Ok(measured)),
            Err(err) => (0, Err(format!("{err:#}"))),
        };
        let result = SampleResult {
            sample,
            lang: None,
            pipeline: Pipeline::B,
            run: 1,
            audio_ms,
            outcome,
        };
        println!("{}", report::stage_line(&result));
        match &result.outcome {
            Ok(measured) => print!("{}", report::text_lines(measured)),
            Err(error) if error.contains("transcript is empty") => println!(
                "  If you did speak, allow this terminal to use the microphone in System \
                 Settings > Privacy & Security > Microphone."
            ),
            Err(_) => {}
        }
    }
    Ok(())
}

async fn dictate_once<S: SpeechProvider, C: CleanupProvider>(
    stt: &S,
    llm: &C,
    deadline: Duration,
    label: &str,
    enter: &mut UnboundedReceiver<Instant>,
) -> Result<(u64, Measured)> {
    // Key-down: recording starts at once; the connections open while the user is speaking.
    let Capture {
        stream,
        mut blocks,
        rate,
    } = Capture::start()?;
    println!("Recording. Press Enter to stop.");
    let (session, warmed) = tokio::join!(stt.open_live(label), llm.warm());
    let mut session = session?;
    warmed?;
    let open = Instant::now();

    let mut resampler = Resampler::new(rate);
    let mut pending: Vec<i16> = Vec::new();
    let mut sent = 0;
    let pressed = loop {
        tokio::select! {
            block = blocks.recv() => {
                let block = block.context("the microphone stopped")?;
                resampler.push(&block, &mut pending);
                while pending.len() >= CHUNK_SAMPLES {
                    let rest = pending.split_off(CHUNK_SAMPLES);
                    session.send_audio(&pending).await?;
                    sent += pending.len();
                    pending = rest;
                }
            }
            pressed = enter.recv() => break pressed,
        }
    };

    // Key release is the moment Enter was pressed, even if the connections were still opening
    // then. Stop the microphone, send what is left, commit.
    let release = pressed.unwrap_or_else(Instant::now);
    drop(stream);
    while let Ok(block) = blocks.try_recv() {
        resampler.push(&block, &mut pending);
    }
    if !pending.is_empty() {
        session.send_audio(&pending).await?;
        sent += pending.len();
    }
    let marks = Marks {
        open,
        release,
        last_audio: Some(Instant::now()),
    };
    let measured = pipeline::finish_live(session, llm, deadline, marks, None).await?;
    Ok((audio::duration_of(sent).as_millis() as u64, measured))
}

/// Records one sample to `<dir>/<name>.wav` as 24 kHz mono 16-bit: Enter to start, Enter to stop.
pub async fn record(name: &str, dir: &Path) -> Result<()> {
    if audio::lang_of(name).is_none() {
        bail!("name the sample es-NN or en-NN, for example es-01, so the language spoken is known");
    }
    let path = dir.join(format!("{name}.wav"));
    if path.exists() {
        bail!(
            "{} already exists. Delete it first to record it again.",
            path.display()
        );
    }
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;

    let mut enter = enter_presses();
    println!("{name}: press Enter to start recording, Enter again to stop.");
    enter.recv().await.context("standard input closed")?;
    let Capture {
        stream,
        mut blocks,
        rate,
    } = Capture::start()?;
    println!("Recording. Press Enter to stop.");
    enter.recv().await;
    drop(stream);

    let mut resampler = Resampler::new(rate);
    let mut pcm = Vec::new();
    while let Ok(block) = blocks.try_recv() {
        resampler.push(&block, &mut pcm);
    }
    let peak = pcm.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
    if peak < SILENCE_PEAK {
        bail!(
            "the recording is silent, so nothing was saved. Allow this terminal to use the \
             microphone in System Settings > Privacy & Security > Microphone, then try again."
        );
    }
    std::fs::write(&path, audio::encode_wav(&pcm))
        .with_context(|| format!("writing {}", path.display()))?;
    let seconds = pcm.len() as f64 / SAMPLE_RATE as f64;
    println!("Saved {} ({seconds:.1} s)", path.display());
    if !(5.0..=15.0).contains(&seconds) {
        println!("Note: the plan asks for dictations of 5 to 15 seconds.");
    }
    Ok(())
}
