//! Follows the sender's clock.
//!
//! The PC's sound device and the headset's run on two crystals that differ by
//! a few dozen parts per million. Left alone, the receiver's buffer would
//! slowly run dry or overflow, and every few minutes a frame would have to be
//! dropped or repeated. Instead the received audio is resampled by a ratio
//! within half a percent of 1, steered by how full the buffer is.
//!
//! The interpolator is a windowed sinc whose zero crossings sit on the input
//! samples. While the ratio is exactly 1 and no correction has been made yet,
//! every output sample is an input sample, bit for bit.

use std::sync::OnceLock;

use crate::CHANNELS;

/// Input frames each output sample is built from.
const TAPS: usize = 48;
const HALF: usize = TAPS / 2;
/// Fractional positions the kernel is tabulated at. Positions in between are
/// interpolated linearly from the two nearest rows.
const PHASES: usize = 256;
/// Kaiser window shape. 9 puts the first sidelobe near -90 dB.
const KAISER_BETA: f64 = 9.0;

/// Furthest the ratio may move from 1. Sound devices differ by far less; the
/// headroom is for catching up after a stall.
pub(crate) const MAX_PULL: f64 = 0.005;

fn kernel() -> &'static [[f32; TAPS]] {
    static KERNEL: OnceLock<Vec<[f32; TAPS]>> = OnceLock::new();
    KERNEL.get_or_init(|| {
        // One extra row, so `phase + 1` is always there to interpolate towards.
        (0..=PHASES)
            .map(|phase| {
                let frac = phase as f64 / PHASES as f64;
                let mut row = [0.0f32; TAPS];
                for (tap, weight) in row.iter_mut().enumerate() {
                    // Tap `HALF - 1` is the input frame just before the output position.
                    let x = tap as f64 - (HALF as f64 - 1.0) - frac;
                    *weight = (sinc(x) * kaiser(x / HALF as f64)) as f32;
                }
                row
            })
            .collect()
    })
}

fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-9 {
        1.0
    } else {
        let px = std::f64::consts::PI * x;
        px.sin() / px
    }
}

/// Kaiser window at `x` in -1..=1; zero outside.
fn kaiser(x: f64) -> f64 {
    if x.abs() >= 1.0 {
        return 0.0;
    }
    bessel_i0(KAISER_BETA * (1.0 - x * x).sqrt()) / bessel_i0(KAISER_BETA)
}

/// Modified Bessel function of the first kind, order zero, by its power series.
fn bessel_i0(x: f64) -> f64 {
    let quarter = x * x / 4.0;
    let (mut sum, mut term) = (1.0, 1.0);
    for k in 1..40 {
        term *= quarter / f64::from(k * k);
        sum += term;
        if term < sum * 1e-12 {
            break;
        }
    }
    sum
}

/// Interleaved stereo resampler with a ratio that may change on every call.
pub(crate) struct Resampler {
    /// The last `TAPS` input frames, then whatever the current call appended.
    history: Vec<f32>,
    /// Position of the next output sample, in input frames, measured from
    /// tap `HALF - 1` of `history`. Between calls it is below the ratio, so
    /// at most a hair over 1.
    pos: f64,
}

impl Resampler {
    pub(crate) fn new() -> Self {
        Self { history: vec![0.0; TAPS * CHANNELS], pos: 0.0 }
    }

    /// Forgets the past. The next output starts from silence.
    pub(crate) fn reset(&mut self) {
        self.history.clear();
        self.history.resize(TAPS * CHANNELS, 0.0);
        self.pos = 0.0;
    }

    /// Appends the resampled `input` to `out`. `ratio` is input frames consumed
    /// per output frame: above 1 plays the input faster and drains the buffer.
    pub(crate) fn process(&mut self, input: &[f32], ratio: f64, out: &mut Vec<f32>) {
        let ratio = ratio.clamp(1.0 - MAX_PULL, 1.0 + MAX_PULL);
        let frames = input.len() / CHANNELS;
        if ratio == 1.0 && self.pos == 0.0 {
            // No correction in effect: the output is the input, delayed by the
            // kernel's reach so a later correction joins without a jump.
            self.history.extend_from_slice(&input[..frames * CHANNELS]);
            let delayed = (HALF - 1) * CHANNELS;
            out.extend_from_slice(&self.history[delayed..delayed + frames * CHANNELS]);
            self.history.drain(..frames * CHANNELS);
            return;
        }
        self.history.extend_from_slice(&input[..frames * CHANNELS]);
        let kernel = kernel();
        let limit = frames as f64;
        while self.pos < limit {
            let base = self.pos.floor();
            let phase = (self.pos - base) * PHASES as f64;
            let row = phase.floor() as usize;
            let blend = (phase - phase.floor()) as f32;
            let (lower, upper) = (&kernel[row], &kernel[row + 1]);
            let window = &self.history[base as usize * CHANNELS..][..TAPS * CHANNELS];
            let (mut left, mut right) = (0.0f32, 0.0f32);
            for ((pair, low), high) in window.as_chunks::<CHANNELS>().0.iter().zip(lower).zip(upper) {
                let weight = low + (high - low) * blend;
                left += pair[0] * weight;
                right += pair[1] * weight;
            }
            out.push(left);
            out.push(right);
            self.pos += ratio;
        }
        self.pos -= limit;
        self.history.drain(..frames * CHANNELS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::tests::{rms, sine_frame};
    use crate::{FRAME, FRAME_SAMPLES};

    fn run(resampler: &mut Resampler, frames: usize, ratio: f64, hz: f64) -> Vec<f32> {
        let mut out = Vec::new();
        let mut phase = 0.0;
        for _ in 0..frames {
            resampler.process(&sine_frame(&mut phase, hz, 0.5), ratio, &mut out);
        }
        out
    }

    #[test]
    fn unity_ratio_should_pass_samples_through_bit_for_bit() {
        let mut resampler = Resampler::new();
        let mut phase = 0.0;
        let input: Vec<f32> = (0..4).flat_map(|_| sine_frame(&mut phase, 1000.0, 0.5)).collect();
        let mut out = Vec::new();
        resampler.process(&input, 1.0, &mut out);

        // The newest input frame sits `HALF + 1` frames behind the output.
        let delay = (HALF + 1) * CHANNELS;
        assert_eq!(&out[delay..], &input[..input.len() - delay]);
    }

    #[test]
    fn unity_ratio_should_keep_the_frame_count() {
        let out = run(&mut Resampler::new(), 10, 1.0, 1000.0);

        assert_eq!(out.len(), 10 * FRAME_SAMPLES);
    }

    #[test]
    fn faster_ratio_should_produce_fewer_frames() {
        let out = run(&mut Resampler::new(), 100, 1.004, 1000.0);

        let expected = (100 * FRAME) as f64 / 1.004;
        assert!((out.len() as f64 / CHANNELS as f64 - expected).abs() <= 1.0, "frames {}", out.len() / CHANNELS);
    }

    #[test]
    fn slower_ratio_should_produce_more_frames() {
        let out = run(&mut Resampler::new(), 100, 0.996, 1000.0);

        let expected = (100 * FRAME) as f64 / 0.996;
        assert!((out.len() as f64 / CHANNELS as f64 - expected).abs() <= 1.0, "frames {}", out.len() / CHANNELS);
    }

    #[test]
    fn resampled_tone_should_keep_its_level_up_to_18_khz() {
        for hz in [100.0, 1000.0, 10_000.0, 18_000.0] {
            let out = run(&mut Resampler::new(), 50, 1.003, hz);
            let settled = &out[FRAME_SAMPLES..];
            let expected = 0.5 / std::f32::consts::SQRT_2;

            assert!((rms(settled) - expected).abs() < 0.004, "{hz} Hz rms {}", rms(settled));
        }
    }

    #[test]
    fn resampled_tone_should_stay_a_clean_sine() {
        // The output of a pure tone must still fit a pure tone of the shifted
        // frequency; what is left over is the interpolator's own error.
        let ratio = 1.003;
        let hz = 5000.0;
        let out = run(&mut Resampler::new(), 50, ratio, hz);
        let left: Vec<f32> = out.as_chunks::<CHANNELS>().0.iter().skip(FRAME).map(|pair| pair[0]).collect();
        let step = std::f64::consts::TAU * hz * ratio / f64::from(crate::RATE);
        let (mut sin_sum, mut cos_sum) = (0.0f64, 0.0f64);
        for (n, sample) in left.iter().enumerate() {
            sin_sum += f64::from(*sample) * (step * n as f64).sin();
            cos_sum += f64::from(*sample) * (step * n as f64).cos();
        }
        let (a, b) = (2.0 * sin_sum / left.len() as f64, 2.0 * cos_sum / left.len() as f64);
        let residual: Vec<f32> =
            left.iter().enumerate().map(|(n, sample)| sample - (a * (step * n as f64).sin() + b * (step * n as f64).cos()) as f32).collect();

        let error_db = 20.0 * (rms(&residual) / rms(&left)).log10();
        assert!(error_db < -70.0, "interpolation error {error_db:.1} dB");
    }

    #[test]
    fn changing_ratio_should_not_click() {
        let mut resampler = Resampler::new();
        let mut out = Vec::new();
        let mut phase = 0.0;
        for index in 0..60 {
            let ratio = if index % 2 == 0 { 0.997 } else { 1.003 };
            resampler.process(&sine_frame(&mut phase, 200.0, 0.5), ratio, &mut out);
        }
        let left: Vec<f32> = out.as_chunks::<CHANNELS>().0.iter().skip(FRAME).map(|pair| pair[0]).collect();
        // A 200 Hz sine at 0.5 moves at most 0.013 per sample.
        let biggest_step = left.windows(2).map(|pair| (pair[1] - pair[0]).abs()).fold(0.0f32, f32::max);

        assert!(biggest_step < 0.02, "step {biggest_step}");
    }

    #[test]
    fn reset_should_silence_the_tail() {
        let mut resampler = Resampler::new();
        let _ = run(&mut resampler, 4, 1.002, 1000.0);
        resampler.reset();
        let mut out = Vec::new();
        resampler.process(&[0.0; FRAME_SAMPLES], 1.0, &mut out);

        assert!(out.iter().all(|sample| *sample == 0.0));
    }
}
