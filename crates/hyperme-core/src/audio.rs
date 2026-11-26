//! From microphone blocks to the speech provider's frames (design.md, "Audio"): mix to mono,
//! resample to the provider's rate, cut into 20 ms frames. Pure code; `hyperme-platform`
//! feeds it from the real microphone.

/// Frames are 20 ms long.
pub const FRAME_MS: u32 = 20;

/// Samples in one 20 ms frame at `rate`.
pub fn frame_samples(rate: u32) -> usize {
    (rate * FRAME_MS / 1000) as usize
}

/// Averages interleaved `channels` into one.
pub fn mix_to_mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect()
}

/// Converts mono `f32` at the device rate to PCM16 at the provider's rate, block by block.
///
/// Ported from flowbench. It averages when the device rate is higher (a simple low-pass) and
/// interpolates linearly when it is lower: enough for speech, not a general-purpose resampler.
#[derive(Debug, Clone)]
pub struct Resampler {
    /// Input samples per output sample.
    step: f64,
    /// Position of the next output sample, in input samples from the start of `buf`.
    pos: f64,
    buf: Vec<f32>,
}

impl Resampler {
    pub fn new(input_rate: u32, output_rate: u32) -> Self {
        Resampler {
            step: f64::from(input_rate) / f64::from(output_rate),
            pos: 0.0,
            buf: Vec::new(),
        }
    }

    /// Adds input and appends every output sample it completes to `out`.
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
            out.push((value.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16);
            self.pos += self.step;
        }
        let consumed = (self.pos.floor() as usize).min(self.buf.len());
        self.buf.drain(..consumed);
        self.pos -= consumed as f64;
    }
}

/// Cuts a stream of samples into frames of a fixed length.
#[derive(Debug, Clone)]
pub struct Framer {
    len: usize,
    pending: Vec<i16>,
}

impl Framer {
    pub fn new(len: usize) -> Self {
        Framer {
            len,
            pending: Vec::with_capacity(len),
        }
    }

    /// Adds samples and calls `emit` with each full frame.
    pub fn push(&mut self, samples: &[i16], mut emit: impl FnMut(Vec<i16>)) {
        let mut rest = samples;
        while !rest.is_empty() {
            let take = (self.len - self.pending.len()).min(rest.len());
            self.pending.extend_from_slice(&rest[..take]);
            rest = &rest[take..];
            if self.pending.len() == self.len {
                emit(std::mem::replace(
                    &mut self.pending,
                    Vec::with_capacity(self.len),
                ));
            }
        }
    }

    /// Sends what is left as a shorter last frame, if anything is.
    pub fn flush(&mut self, mut emit: impl FnMut(Vec<i16>)) {
        if !self.pending.is_empty() {
            emit(std::mem::take(&mut self.pending));
        }
    }
}
