//! Audio helpers: sample clips, WAV in and out, the real-time chunk schedule, resampling.

use std::io::Cursor;
use std::ops::Range;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};

use crate::lang::Lang;

/// The only format sent to providers: 24 kHz mono 16-bit PCM.
pub const SAMPLE_RATE: u32 = 24_000;
/// Length of one streamed chunk, as a microphone would deliver it.
pub const CHUNK_MS: u32 = 100;
/// Samples in one full chunk.
pub const CHUNK_SAMPLES: usize = (SAMPLE_RATE * CHUNK_MS / 1000) as usize;

const WAV_SPEC: hound::WavSpec = hound::WavSpec {
    channels: 1,
    sample_rate: SAMPLE_RATE,
    bits_per_sample: 16,
    sample_format: hound::SampleFormat::Int,
};

/// One dictation: its samples for streaming and the same audio as a WAV file for upload.
pub struct Clip {
    /// File name without extension, such as `es-01`.
    pub name: String,
    /// Language spoken, taken from the name. `None` when the name does not say.
    pub lang: Option<Lang>,
    pub pcm: Vec<i16>,
    pub wav: Vec<u8>,
}

impl Clip {
    pub fn from_pcm(name: &str, pcm: Vec<i16>) -> Clip {
        Clip {
            name: name.to_owned(),
            lang: lang_of(name),
            wav: encode_wav(&pcm),
            pcm,
        }
    }

    pub fn load(path: &Path) -> Result<Clip> {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .with_context(|| format!("unreadable file name: {}", path.display()))?;
        let wav = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let pcm = decode_wav(&wav).with_context(|| format!("{}", path.display()))?;
        Ok(Clip {
            name: name.to_owned(),
            lang: lang_of(name),
            pcm,
            wav,
        })
    }

    pub fn duration_ms(&self) -> u64 {
        duration_of(self.pcm.len()).as_millis() as u64
    }
}

/// `es-01` is Spanish, `en-01` is English.
pub fn lang_of(name: &str) -> Option<Lang> {
    name.split('-').next().and_then(Lang::from_code)
}

/// Loads every `*.wav` in `dir`, sorted by name. With `only`, loads just those names.
pub fn load_clips(dir: &Path, only: &[String]) -> Result<Vec<Clip>> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| {
            format!(
                "no samples folder at {}. See README.md for how to record the samples",
                dir.display()
            )
        })?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|ext| ext == "wav"))
        .collect();
    paths.sort();
    let mut clips = Vec::new();
    for path in &paths {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if only.is_empty() || only.iter().any(|name| name == stem) {
            clips.push(Clip::load(path)?);
        }
    }
    for name in only {
        if !clips.iter().any(|c| &c.name == name) {
            bail!("no sample named {name}.wav in {}", dir.display());
        }
    }
    if clips.is_empty() {
        bail!(
            "no WAV samples in {}. See README.md for how to record them.",
            dir.display()
        );
    }
    if let Some(clip) = clips.iter().find(|c| c.lang.is_none()) {
        bail!(
            "{}.wav: the name must start with es- or en-, so the language spoken is known",
            clip.name
        );
    }
    Ok(clips)
}

pub fn decode_wav(bytes: &[u8]) -> Result<Vec<i16>> {
    let mut reader = hound::WavReader::new(Cursor::new(bytes)).context("not a WAV file")?;
    let spec = reader.spec();
    if spec != WAV_SPEC {
        bail!(
            "expected 24000 Hz, 1 channel, 16-bit integer; found {} Hz, {} channel(s), {}-bit {:?}. \
             Convert with: afconvert -f WAVE -d LEI16@24000 -c 1 in.wav out.wav",
            spec.sample_rate,
            spec.channels,
            spec.bits_per_sample,
            spec.sample_format
        );
    }
    reader
        .samples::<i16>()
        .collect::<Result<Vec<_>, _>>()
        .context("reading WAV samples")
}

pub fn encode_wav(pcm: &[i16]) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    // Writing to memory cannot fail, and the spec is a valid constant.
    let mut writer = hound::WavWriter::new(&mut out, WAV_SPEC).expect("valid WAV spec");
    for &sample in pcm {
        writer.write_sample(sample).expect("in-memory write");
    }
    writer.finalize().expect("in-memory write");
    out.into_inner()
}

pub fn duration_of(samples: usize) -> Duration {
    Duration::from_secs_f64(samples as f64 / SAMPLE_RATE as f64)
}

/// Splits `total` samples into chunks of [`CHUNK_MS`]. Each chunk comes with the time, counted
/// from the start of the audio, at which a microphone would have delivered it: the moment its
/// last sample was captured.
pub fn chunk_schedule(total: usize) -> Vec<(Range<usize>, Duration)> {
    (0..total)
        .step_by(CHUNK_SAMPLES)
        .map(|start| {
            let end = (start + CHUNK_SAMPLES).min(total);
            (start..end, duration_of(end))
        })
        .collect()
}

/// PCM16 as little-endian bytes, the layout `audio/pcm` expects.
pub fn pcm_bytes(pcm: &[i16]) -> Vec<u8> {
    pcm.iter().flat_map(|s| s.to_le_bytes()).collect()
}

/// Converts microphone audio (mono `f32` at the device rate) to 24 kHz PCM16, block by block.
///
/// It averages when the device rate is higher (a simple low-pass) and interpolates linearly when
/// it is lower. That is enough for speech in a spike; it is not a general-purpose resampler.
pub struct Resampler {
    /// Input samples per output sample.
    step: f64,
    /// Position of the next output sample, in input samples from the start of `buf`.
    pos: f64,
    buf: Vec<f32>,
}

impl Resampler {
    pub fn new(input_rate: u32) -> Resampler {
        Resampler {
            step: input_rate as f64 / SAMPLE_RATE as f64,
            pos: 0.0,
            buf: Vec::new(),
        }
    }

    pub fn push(&mut self, input: &[f32], out: &mut Vec<i16>) {
        self.buf.extend_from_slice(input);
        loop {
            let start = self.pos.floor() as usize;
            let value = if self.step > 1.0 {
                let end = (self.pos + self.step).floor() as usize;
                if end > self.buf.len() {
                    break;
                }
                let window = &self.buf[start..end];
                window.iter().sum::<f32>() / window.len() as f32
            } else {
                if start + 1 >= self.buf.len() {
                    break;
                }
                let frac = (self.pos - start as f64) as f32;
                self.buf[start] * (1.0 - frac) + self.buf[start + 1] * frac
            };
            out.push((value.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
            self.pos += self.step;
        }
        let consumed = (self.pos.floor() as usize).min(self.buf.len());
        self.buf.drain(..consumed);
        self.pos -= consumed as f64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_are_100_ms_and_due_when_their_last_sample_is_captured() {
        // 250 ms of audio: two full chunks and a 50 ms tail.
        let schedule = chunk_schedule(6_000);
        let ranges: Vec<_> = schedule.iter().map(|(r, _)| r.clone()).collect();
        let due_ms: Vec<_> = schedule.iter().map(|(_, d)| d.as_millis()).collect();
        assert_eq!(ranges, vec![0..2_400, 2_400..4_800, 4_800..6_000]);
        assert_eq!(due_ms, vec![100, 200, 250]);
    }

    #[test]
    fn chunk_schedule_covers_every_sample_once() {
        for total in [0, 1, 2_399, 2_400, 2_401, 240_000] {
            let covered: usize = chunk_schedule(total).iter().map(|(r, _)| r.len()).sum();
            assert_eq!(covered, total);
        }
        assert!(chunk_schedule(0).is_empty());
    }

    #[test]
    fn ten_seconds_is_one_hundred_chunks_ending_at_ten_seconds() {
        let schedule = chunk_schedule(240_000);
        assert_eq!(schedule.len(), 100);
        assert_eq!(schedule.last().unwrap().1, Duration::from_secs(10));
    }

    #[test]
    fn pcm_bytes_are_little_endian() {
        assert_eq!(pcm_bytes(&[1, -2]), vec![0x01, 0x00, 0xFE, 0xFF]);
    }

    #[test]
    fn wav_round_trip() {
        let pcm = vec![0, 1_000, -1_000, i16::MAX, i16::MIN];
        assert_eq!(decode_wav(&encode_wav(&pcm)).unwrap(), pcm);
    }

    #[test]
    fn wav_in_another_format_is_rejected_with_the_fix() {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut out = Cursor::new(Vec::new());
        let mut writer = hound::WavWriter::new(&mut out, spec).unwrap();
        writer.write_sample(0i16).unwrap();
        writer.write_sample(0i16).unwrap();
        writer.finalize().unwrap();
        let err = decode_wav(&out.into_inner()).unwrap_err().to_string();
        assert!(err.contains("48000 Hz, 2 channel(s)"), "{err}");
        assert!(err.contains("afconvert"), "{err}");
    }

    #[test]
    fn language_comes_from_the_file_name() {
        assert_eq!(Clip::from_pcm("es-07", vec![]).lang, Some(Lang::Es));
        assert_eq!(Clip::from_pcm("en-25", vec![]).lang, Some(Lang::En));
        assert_eq!(Clip::from_pcm("mic-01", vec![]).lang, None);
    }

    fn resample(input_rate: u32, input: &[f32], block: usize) -> Vec<i16> {
        let mut resampler = Resampler::new(input_rate);
        let mut out = Vec::new();
        for part in input.chunks(block) {
            resampler.push(part, &mut out);
        }
        out
    }

    #[test]
    fn resampler_halves_48_khz() {
        let out = resample(48_000, &vec![0.5; 48_000], 480);
        assert_eq!(out.len(), 24_000);
        assert!(out.iter().all(|&s| (s - 16_383).abs() <= 1));
    }

    #[test]
    fn resampler_handles_44_1_khz_and_16_khz() {
        let down = resample(44_100, &vec![0.25; 44_100], 512);
        assert!((23_990..=24_000).contains(&down.len()), "{}", down.len());
        let up = resample(16_000, &vec![0.25; 16_000], 160);
        assert!((23_990..=24_000).contains(&up.len()), "{}", up.len());
        assert!(up.iter().all(|&s| (s - 8_191).abs() <= 1));
    }

    #[test]
    fn resampler_output_does_not_depend_on_block_size() {
        let input: Vec<f32> = (0..9_600).map(|i| (i as f32 * 0.01).sin() * 0.8).collect();
        assert_eq!(resample(48_000, &input, 7), resample(48_000, &input, 9_600));
        assert_eq!(resample(16_000, &input, 7), resample(16_000, &input, 9_600));
    }

    #[test]
    fn resampler_keeps_a_speech_band_tone() {
        // 1 kHz at 48 kHz in: the 24 kHz output must keep roughly the same amplitude.
        let input: Vec<f32> = (0..48_000)
            .map(|i| (i as f32 * std::f32::consts::TAU * 1_000.0 / 48_000.0).sin() * 0.5)
            .collect();
        let peak = resample(48_000, &input, 480)
            .iter()
            .map(|s| s.unsigned_abs())
            .max()
            .unwrap();
        assert!((15_500..=16_500).contains(&peak), "{peak}");
    }
}
