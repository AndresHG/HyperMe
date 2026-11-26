//! Slice 6: turning microphone blocks into the speech provider's frames. Pure code, so it is
//! tested here on every OS; `hyperme-platform` feeds it from the real microphone.

use hyperme_core::audio::{Framer, Resampler, frame_samples};

fn resample(input_rate: u32, input: &[f32], block: usize) -> Vec<i16> {
    let mut resampler = Resampler::new(input_rate, 24_000);
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
fn resampler_handles_44_1_khz_16_khz_and_24_khz() {
    let down = resample(44_100, &vec![0.25; 44_100], 512);
    assert!((23_990..=24_000).contains(&down.len()), "{}", down.len());
    let up = resample(16_000, &vec![0.25; 16_000], 160);
    assert!((23_990..=24_000).contains(&up.len()), "{}", up.len());
    assert!(up.iter().all(|&s| (s - 8_191).abs() <= 1));
    let same = resample(24_000, &vec![0.25; 24_000], 333);
    assert!((23_999..=24_000).contains(&same.len()), "{}", same.len());
}

#[test]
fn resampler_output_does_not_depend_on_block_size() {
    let input: Vec<f32> = (0..9_600).map(|i| (i as f32 * 0.01).sin() * 0.8).collect();
    assert_eq!(resample(48_000, &input, 7), resample(48_000, &input, 9_600));
    assert_eq!(resample(16_000, &input, 7), resample(16_000, &input, 9_600));
}

#[test]
fn resampler_keeps_a_speech_band_tone() {
    // 1 kHz at 48 kHz in: the 24 kHz output keeps roughly the same amplitude.
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

#[test]
fn resampler_clips_instead_of_wrapping() {
    let out = resample(24_000, &[2.0, -2.0, 0.0], 3);
    assert_eq!(&out[..2], &[i16::MAX, -i16::MAX]);
}

/// Design, "Audio": frames of 20 ms.
#[test]
fn framer_cuts_20_ms_frames_and_keeps_the_rest() {
    assert_eq!(frame_samples(24_000), 480);
    let mut framer = Framer::new(480);
    let mut frames = Vec::new();
    framer.push(&vec![1i16; 1000], |f| frames.push(f));
    assert_eq!(frames.len(), 2);
    assert!(frames.iter().all(|f| f.len() == 480));
    framer.push(&vec![2i16; 500], |f| frames.push(f));
    assert_eq!(frames.len(), 3);
    assert_eq!(frames[2][..40], [1i16; 40]);
    assert_eq!(frames[2][40..], [2i16; 440]);
    // 60 samples are left; flush sends them as a short last frame.
    let mut last = Vec::new();
    framer.flush(|f| last.push(f));
    assert_eq!(last, [vec![2i16; 60]]);
    framer.flush(|f| last.push(f));
    assert_eq!(last.len(), 1, "a second flush sends nothing");
}

#[test]
fn mono_mixdown_averages_channels() {
    assert_eq!(
        hyperme_core::audio::mix_to_mono(&[0.5, -0.5, 1.0, 0.0], 2),
        vec![0.0, 0.5]
    );
    assert_eq!(
        hyperme_core::audio::mix_to_mono(&[0.25, 0.5], 1),
        vec![0.25, 0.5]
    );
}
