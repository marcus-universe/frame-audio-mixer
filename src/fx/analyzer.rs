//! Spectrum behind the parametric EQ. The realtime thread only copies samples
//! into `Scope`; the FFT runs on the engine's main loop while a panel is asking.

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use super::fft::{hann, Complex, Fft};

/// Log-spaced bands from 20 Hz to 20 kHz, the same axis as the EQ graph.
pub const BANDS: usize = 128;
const SIZE: usize = 4096;
const RATE: f32 = 48_000.0;
const NOT_ARMED: u32 = u32::MAX;
pub const SILENCE_DB: f32 = -120.0;

/// A lock-free ring of the newest mono samples. One writer, one reader. A sample
/// read while being overwritten only makes one analyzer frame slightly wrong.
pub struct Scope {
    slot: AtomicU32,
    write: AtomicUsize,
    samples: Box<[AtomicU32]>,
}

impl Scope {
    pub fn new() -> Self {
        Self {
            slot: AtomicU32::new(NOT_ARMED),
            write: AtomicUsize::new(0),
            samples: (0..SIZE).map(|_| AtomicU32::new(0)).collect(),
        }
    }

    /// Captures after the effect at chain position `slot`.
    pub fn arm(&self, slot: usize) {
        self.slot.store(slot as u32, Ordering::Relaxed);
    }

    pub fn disarm(&self) {
        self.slot.store(NOT_ARMED, Ordering::Relaxed);
    }

    pub fn armed_slot(&self) -> u32 {
        self.slot.load(Ordering::Relaxed)
    }

    pub fn push(&self, left: &[f32], right: &[f32]) {
        let mut at = self.write.load(Ordering::Relaxed);
        for (l, r) in left.iter().zip(right) {
            self.samples[at].store(((l + r) * 0.5).to_bits(), Ordering::Relaxed);
            at = (at + 1) % SIZE;
        }
        self.write.store(at, Ordering::Relaxed);
    }

    fn copy_newest(&self, out: &mut [Complex], window: &[f32]) {
        let start = self.write.load(Ordering::Relaxed);
        for (i, value) in out.iter_mut().enumerate() {
            let sample = f32::from_bits(self.samples[(start + i) % SIZE].load(Ordering::Relaxed));
            *value = Complex::new(if sample.is_finite() { sample * window[i] } else { 0.0 }, 0.0);
        }
    }
}

pub struct Analyzer {
    fft: Fft,
    window: Vec<f32>,
    frame: Vec<Complex>,
    power: Vec<f32>,
}

impl Analyzer {
    pub fn new() -> Self {
        Self { fft: Fft::new(SIZE), window: hann(SIZE), frame: vec![Complex::default(); SIZE], power: vec![0.0; SIZE / 2] }
    }

    /// Peak level of each band in dBFS. A full-scale sine reads 0.
    pub fn bands(&mut self, scope: &Scope) -> Vec<f32> {
        scope.copy_newest(&mut self.frame, &self.window);
        self.fft.forward(&mut self.frame);
        // A Hann window has a coherent gain of one half.
        let scale = 4.0 / SIZE as f32;
        for (power, value) in self.power.iter_mut().zip(&self.frame) {
            *power = value.norm_sqr() * scale * scale;
        }
        (0..BANDS).map(|band| self.band_db(band)).collect()
    }

    fn band_db(&self, band: usize) -> f32 {
        let step = 1000f32.powf(0.5 / (BANDS - 1) as f32);
        let bin_hz = RATE / SIZE as f32;
        let center = band_hz(band) / bin_hz;
        let (low, high) = (center / step, center * step);
        let last = self.power.len() - 1;
        let power = if high - low < 1.0 {
            // Narrower than a bin, which is the low end: read between two bins.
            let at = center.min(last as f32 - 1.0);
            let whole = at as usize;
            let frac = at - whole as f32;
            self.power[whole] * (1.0 - frac) + self.power[whole + 1] * frac
        } else {
            let (from, to) = ((low.ceil() as usize).min(last), (high.floor() as usize).min(last));
            self.power[from..=to.max(from)].iter().copied().fold(0.0, f32::max)
        };
        if power <= 1e-14 { SILENCE_DB } else { (10.0 * power.log10()).max(SILENCE_DB) }
    }
}

pub fn band_hz(band: usize) -> f32 {
    20.0 * 1000f32.powf(band as f32 / (BANDS - 1) as f32)
}

pub fn self_test() -> Result<(), String> {
    let scope = Scope::new();
    let tone: Vec<f32> = (0..SIZE).map(|i| (std::f32::consts::TAU * 1000.0 * i as f32 / RATE).sin()).collect();
    scope.push(&tone, &tone);
    let bands = Analyzer::new().bands(&scope);
    let loudest = (0..BANDS).max_by(|a, b| bands[*a].total_cmp(&bands[*b])).unwrap_or(0);
    let ratio = band_hz(loudest) / 1000.0;
    if !(0.94..=1.06).contains(&ratio) {
        return Err(format!("analyzer peak at {} Hz", band_hz(loudest)));
    }
    if bands[loudest].abs() > 1.5 {
        return Err(format!("analyzer level {} dB", bands[loudest]));
    }
    Ok(())
}
