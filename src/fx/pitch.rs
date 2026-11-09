//! Phase-vocoder pitch shifter, after Laroche and Dolson's peak shifting.
//!
//! Each spectral peak moves with the bins around it, so the phase relations
//! inside a partial survive. That is what removes the warble a grain shifter
//! has at its splices. Left and right share one set of rotations, so the stereo
//! image does not drift. The fixed latency is one frame, about 43 ms.

use std::f32::consts::TAU;

use super::fft::{hann, Complex, Fft};

/// Long enough to separate the harmonics of a low voice.
const SIZE: usize = 2048;
const HOP: usize = 256;
const BINS: usize = SIZE / 2 + 1;
/// A Hann analysis and synthesis window at this overlap add up to 3.
const OVERLAP_GAIN: f32 = 3.0;
const PEAK_FLOOR: f32 = 1e-10;

pub struct Pitcher {
    fft: Fft,
    window: Vec<f32>,
    input: [Vec<f32>; 2],
    output: [Vec<f32>; 2],
    pos: usize,
    since_frame: usize,
    frame: Vec<Complex>,
    left: Vec<Complex>,
    right: Vec<Complex>,
    shifted_left: Vec<Complex>,
    shifted_right: Vec<Complex>,
    reference: Vec<Complex>,
    previous: Vec<Complex>,
    power: Vec<f32>,
    rotation: Vec<f32>,
    next_rotation: Vec<f32>,
    peaks: Vec<usize>,
    ratio: f32,
}

fn wrap(angle: f32) -> f32 {
    angle - TAU * (angle / TAU).round()
}

impl Pitcher {
    pub fn new() -> Self {
        Self {
            fft: Fft::new(SIZE),
            window: hann(SIZE),
            input: [vec![0.0; SIZE], vec![0.0; SIZE]],
            output: [vec![0.0; SIZE], vec![0.0; SIZE]],
            pos: 0,
            since_frame: 0,
            frame: vec![Complex::default(); SIZE],
            left: vec![Complex::default(); BINS],
            right: vec![Complex::default(); BINS],
            shifted_left: vec![Complex::default(); BINS],
            shifted_right: vec![Complex::default(); BINS],
            reference: vec![Complex::default(); BINS],
            previous: vec![Complex::default(); BINS],
            power: vec![0.0; BINS],
            rotation: vec![0.0; BINS],
            next_rotation: vec![0.0; BINS],
            peaks: Vec::with_capacity(BINS),
            ratio: 1.0,
        }
    }

    pub fn set(&mut self, params: [f64; 3]) {
        let semitones = (params[0] - 0.5) * 24.0;
        self.ratio = 2f64.powf(semitones / 12.0) as f32;
    }

    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            self.input[0][self.pos] = *l;
            self.input[1][self.pos] = *r;
            *l = std::mem::take(&mut self.output[0][self.pos]);
            *r = std::mem::take(&mut self.output[1][self.pos]);
            self.pos = (self.pos + 1) % SIZE;
            self.since_frame += 1;
            if self.since_frame == HOP {
                self.since_frame = 0;
                self.run_frame();
            }
        }
    }

    fn run_frame(&mut self) {
        self.analyze();
        self.find_peaks();
        self.shift_peaks();
        self.synthesize();
        std::mem::swap(&mut self.previous, &mut self.reference);
    }

    /// One complex FFT carries both channels. The window is centered on sample
    /// zero, so a bin moved in frequency keeps its phase at the window's middle.
    fn analyze(&mut self) {
        for i in 0..SIZE {
            let at = (self.pos + i) % SIZE;
            let w = self.window[i];
            self.frame[(i + SIZE / 2) % SIZE] = Complex::new(self.input[0][at] * w, self.input[1][at] * w);
        }
        self.fft.forward(&mut self.frame);
        for k in 0..BINS {
            let a = self.frame[k];
            let b = self.frame[(SIZE - k) % SIZE].conj();
            let left = a.add(b).scale(0.5);
            let difference = a.sub(b);
            let right = Complex::new(difference.im * 0.5, -difference.re * 0.5);
            let power = left.norm_sqr() + right.norm_sqr();
            let mid = left.add(right);
            // Out-of-phase stereo cancels in the sum, so its phase means nothing there.
            self.reference[k] = if mid.norm_sqr() > 0.25 * power {
                mid
            } else if left.norm_sqr() >= right.norm_sqr() {
                left
            } else {
                right
            };
            self.left[k] = left;
            self.right[k] = right;
            self.power[k] = power;
        }
    }

    fn find_peaks(&mut self) {
        self.peaks.clear();
        let power = &self.power;
        for k in 2..BINS - 2 {
            let value = power[k];
            if value > PEAK_FLOOR && value > power[k - 1] && value >= power[k + 1] && value > power[k - 2] && value >= power[k + 2] {
                self.peaks.push(k);
            }
        }
    }

    fn shift_peaks(&mut self) {
        self.shifted_left.fill(Complex::default());
        self.shifted_right.fill(Complex::default());
        if self.peaks.is_empty() {
            return;
        }
        let per_bin = TAU * HOP as f32 / SIZE as f32;
        let bend = self.ratio - 1.0;
        for index in 0..self.peaks.len() {
            let peak = self.peaks[index];
            let start = if index == 0 { 0 } else { (self.peaks[index - 1] + peak).div_ceil(2) };
            let end = self.peaks.get(index + 1).map_or(BINS, |next| (peak + next).div_ceil(2));
            let advance = self.reference[peak].mul(self.previous[peak].conj()).arg();
            let expected = per_bin * peak as f32;
            let deviation = wrap(advance - expected);
            let true_bin = peak as f32 + deviation / per_bin;
            let shift = (true_bin * bend).round() as isize;
            let angle = wrap(self.rotation[peak] + (expected + deviation) * bend);
            let phasor = Complex::from_angle(angle);
            for k in start..end {
                self.next_rotation[k] = angle;
                let target = k as isize + shift;
                if target > 0 && target < BINS as isize - 1 {
                    let target = target as usize;
                    self.shifted_left[target] = self.shifted_left[target].add(self.left[k].mul(phasor));
                    self.shifted_right[target] = self.shifted_right[target].add(self.right[k].mul(phasor));
                }
            }
        }
        std::mem::swap(&mut self.rotation, &mut self.next_rotation);
    }

    fn synthesize(&mut self) {
        self.frame[0] = Complex::default();
        self.frame[SIZE / 2] = Complex::default();
        for k in 1..BINS - 1 {
            let l = self.shifted_left[k];
            let r = self.shifted_right[k];
            self.frame[k] = Complex::new(l.re - r.im, l.im + r.re);
            self.frame[SIZE - k] = Complex::new(l.re + r.im, r.re - l.im);
        }
        self.fft.inverse(&mut self.frame);
        let gain = 1.0 / (SIZE as f32 * OVERLAP_GAIN);
        for i in 0..SIZE {
            let at = (self.pos + i) % SIZE;
            let value = self.frame[(i + SIZE / 2) % SIZE];
            let w = self.window[i] * gain;
            self.output[0][at] += value.re * w;
            self.output[1][at] += value.im * w;
        }
    }
}

