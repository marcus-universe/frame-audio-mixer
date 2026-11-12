//! Phase-vocoder pitch shifter, after Laroche and Dolson's peak shifting.
//!
//! Each spectral peak moves with the bins around it, so the phase relations
//! inside a partial survive. That is what removes the warble a grain shifter
//! has at its splices. Left and right share one set of rotations, so the stereo
//! image does not drift. The fixed latency is one frame, about 43 ms.
//!
//! Pitch and formants are set apart. Moving the harmonics also moves the
//! resonances of the vocal tract with them, which is the chipmunk sound. So the
//! spectral envelope of each frame is measured from the harmonic peaks, and every
//! shifted bin is rescaled from the envelope it brought along to the envelope it
//! should have: the voice's own, or one stretched by the formant amount. A male
//! voice raised by half an octave, with its formants raised by a quarter octave,
//! sounds like a woman rather than a sped-up man. The envelope comes from the
//! spectrum the shifter computes anyway, so it costs no extra FFT.

use std::f32::consts::TAU;

use super::fft::{hann, Complex, Fft};

/// Long enough to separate the harmonics of a low voice.
const SIZE: usize = 2048;
const HOP: usize = 256;
const BINS: usize = SIZE / 2 + 1;
/// A Hann analysis and synthesis window at this overlap add up to 3.
const OVERLAP_GAIN: f32 = 3.0;
const PEAK_FLOOR: f32 = 1e-10;
/// Hz per bin.
const BIN_HZ: f32 = 48_000.0 / SIZE as f32;
/// Harmonic spacing assumed when a frame has too few peaks to measure it: 7 bins, about 160 Hz.
const DEFAULT_SPACING: usize = 7;
/// Spacing limits in bins: about 70 Hz to 350 Hz, the range of speaking voices.
const MIN_SPACING: usize = 3;
const MAX_SPACING: usize = 15;
/// The spacing is measured on peaks within 30 dB of the loudest: clear harmonics only.
const HARMONIC_FLOOR: f32 = 1e-3;
/// The envelope traces peaks down to 90 dB under the loudest. The upper harmonics
/// of a voice sit 40 to 60 dB down and carry its air; dropping them leaves the
/// top of the envelope guessed, and a shift then over- or under-shoots it there.
const TRACE_FLOOR: f32 = 1e-9;
/// The correction stays between −36 and +30 dB. A partial is only brought to the
/// level the envelope has where it lands, so a large gain cannot overshoot; the
/// limit only matters when the envelope itself is wrong. A formant moved into
/// a valley, or out of one, needs about 30 dB, so less would leave it short.
const MAX_BOOST: f32 = 32.0;
const MAX_CUT: f32 = 1.0 / 64.0;
/// Brightness at either end of its slider, in dB per octave around 1 kHz.
const MAX_TILT_DB: f32 = 3.0;

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
    /// Where the formants go: 1 keeps the voice's own, `ratio` moves them with the pitch.
    formant: f32,
    /// Gain per output bin for the brightness slider. Empty while it is flat.
    tilt: Vec<f32>,
    /// Spectral envelope of the frame being shifted, by input bin.
    envelope: Vec<f32>,
    scratch: Vec<f32>,
    /// Gaps between neighbouring harmonic peaks, to find the harmonic spacing.
    gaps: Vec<usize>,
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
            formant: 1.0,
            tilt: Vec::with_capacity(BINS),
            envelope: vec![0.0; BINS],
            scratch: vec![0.0; BINS],
            gaps: Vec::with_capacity(BINS),
        }
    }

    /// `params` are pitch, formant and brightness, each 0..1 with 0.5 as neutral.
    /// Pitch and formant span ±12 semitones. A formant of 0.5 keeps the voice's
    /// own resonances; a formant equal to the pitch moves them along (chipmunk).
    pub fn set(&mut self, params: [f64; 3]) {
        let semitones = (params[0] - 0.5) * 24.0;
        self.ratio = 2f64.powf(semitones / 12.0) as f32;
        let formant = (params[1].clamp(0.0, 1.0) - 0.5) * 24.0;
        self.formant = 2f64.powf(formant / 12.0) as f32;
        let tilt = ((params[2].clamp(0.0, 1.0) - 0.5) * 2.0) as f32 * MAX_TILT_DB;
        self.tilt.clear();
        if tilt.abs() > 0.01 {
            self.tilt.extend((0..BINS).map(|k| {
                let hz = (k as f32 * BIN_HZ).max(100.0);
                let db = (tilt * (hz / 1000.0).log2()).clamp(-12.0, 12.0);
                10f32.powf(db / 20.0)
            }));
        }
    }

    /// The formants travel with the pitch, as with a plain shifter.
    fn formants_follow(&self) -> bool {
        (self.formant - self.ratio).abs() < 1e-4
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
        let reshape = !self.formants_follow();
        if reshape {
            self.measure_envelope();
        }
        self.shift_peaks(reshape);
        self.tilt();
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

    /// Moves each peak with the bins around it. With `reshape`, each moved peak is
    /// also scaled from the envelope it carries to the envelope it lands in. A whole
    /// partial takes one gain: on a downward shift the moved partials overlap their
    /// neighbours, and a gain per output bin would hit a mix of two of them.
    fn shift_peaks(&mut self, reshape: bool) {
        self.shifted_left.fill(Complex::default());
        self.shifted_right.fill(Complex::default());
        if self.peaks.is_empty() {
            return;
        }
        let per_bin = TAU * HOP as f32 / SIZE as f32;
        let bend = self.ratio - 1.0;
        let to_target = 1.0 / self.formant;
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
            let mut phasor = Complex::from_angle(angle);
            if reshape {
                // The partial carries the envelope at its source and lands at true_bin * ratio.
                let carried = envelope_at(&self.envelope, true_bin);
                let wanted = envelope_at(&self.envelope, true_bin * self.ratio * to_target);
                phasor = phasor.scale((wanted / carried).clamp(MAX_CUT, MAX_BOOST));
            }
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

    /// Brightness: a gentle slope around 1 kHz on the shifted spectrum.
    fn tilt(&mut self) {
        if self.tilt.is_empty() {
            return;
        }
        for k in 1..BINS - 1 {
            self.shifted_left[k] = self.shifted_left[k].scale(self.tilt[k]);
            self.shifted_right[k] = self.shifted_right[k].scale(self.tilt[k]);
        }
    }

    /// The envelope of the input frame. Only ratios of it are used, so its scale
    /// does not matter. A voiced frame has harmonics, and their tops are the
    /// envelope: it runs straight from one harmonic peak to the next. Smoothing
    /// would flatten the formants of a low voice, whose harmonics sit close.
    /// A frame without clear harmonics (a hiss, a breath) gets a smoothed one.
    fn measure_envelope(&mut self) {
        let spacing = self.harmonic_spacing();
        for (magnitude, power) in self.scratch.iter_mut().zip(&self.power) {
            *magnitude = power.sqrt();
        }
        if !self.trace_harmonics(spacing) {
            self.smooth_envelope(spacing);
        }
        // A floor 100 dB under the loudest part, so silence never divides by zero.
        let loudest = self.envelope.iter().copied().fold(0.0, f32::max);
        let floor = loudest * 1e-5 + 1e-12;
        for value in &mut self.envelope {
            *value = value.max(floor);
        }
    }

    /// Joins the harmonic tops with straight lines and holds the level past the
    /// first and the last. A harmonic is a strong peak that is the loudest bin
    /// within half a spacing. False when there are too few to trace.
    fn trace_harmonics(&mut self, spacing: usize) -> bool {
        let loudest = self.peaks.iter().map(|&k| self.power[k]).fold(0.0, f32::max);
        let floor = loudest * TRACE_FLOOR;
        let half = (spacing / 2).max(1);
        let magnitude = &self.scratch;
        let mut previous: Option<usize> = None;
        let mut traced = 0;
        for &k in &self.peaks {
            if self.power[k] < floor {
                continue;
            }
            let from = k.saturating_sub(half);
            let to = (k + half).min(BINS - 1);
            if magnitude[from..=to].iter().any(|&other| other > magnitude[k]) {
                continue;
            }
            match previous {
                None => self.envelope[..=k].fill(magnitude[k]),
                Some(before) => {
                    let (start, end) = (magnitude[before], magnitude[k]);
                    let span = (k - before) as f32;
                    for (step, value) in self.envelope[before..=k].iter_mut().enumerate() {
                        *value = start + (end - start) * step as f32 / span;
                    }
                }
            }
            previous = Some(k);
            traced += 1;
        }
        match previous {
            Some(last) if traced >= 3 => {
                self.envelope[last..].fill(magnitude[last]);
                true
            }
            _ => false,
        }
    }

    /// The peaks joined by a max filter, then smoothed, both about half a harmonic
    /// spacing wide, for frames without harmonics to trace.
    fn smooth_envelope(&mut self, spacing: usize) {
        let reach = (spacing * 3).div_ceil(5);
        let smooth = spacing.div_ceil(2);
        for k in 0..BINS {
            let from = k.saturating_sub(reach);
            let to = (k + reach).min(BINS - 1);
            self.envelope[k] = self.scratch[from..=to].iter().copied().fold(0.0, f32::max);
        }
        // A running sum keeps the smoothing linear in the number of bins.
        let mut sum: f32 = self.envelope[..smooth].iter().sum();
        for k in 0..BINS {
            if let Some(entering) = self.envelope.get(k + smooth) {
                sum += entering;
            }
            if k > smooth {
                sum -= self.envelope[k - smooth - 1];
            }
            let from = k.saturating_sub(smooth);
            let to = (k + smooth).min(BINS - 1);
            self.scratch[k] = sum.max(0.0) / (to - from + 1) as f32;
        }
        self.envelope.copy_from_slice(&self.scratch);
    }

    /// Bins between harmonics: the median gap between neighbouring strong peaks.
    fn harmonic_spacing(&mut self) -> usize {
        let loudest = self.peaks.iter().map(|&k| self.power[k]).fold(0.0, f32::max);
        let floor = loudest * HARMONIC_FLOOR;
        self.gaps.clear();
        let mut previous = None;
        for &k in self.peaks.iter().filter(|&&k| self.power[k] >= floor) {
            if let Some(before) = previous {
                self.gaps.push(k - before);
            }
            previous = Some(k);
        }
        if self.gaps.len() < 3 {
            return DEFAULT_SPACING;
        }
        let middle = self.gaps.len() / 2;
        let (_, median, _) = self.gaps.select_nth_unstable(middle);
        (*median).clamp(MIN_SPACING, MAX_SPACING)
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


/// The envelope at a fractional bin, by linear interpolation. Past the top it holds the last value.
fn envelope_at(envelope: &[f32], bin: f32) -> f32 {
    let last = envelope.len() - 1;
    if bin <= 0.0 {
        return envelope[0];
    }
    let whole = bin as usize;
    if whole >= last {
        return envelope[last];
    }
    let frac = bin - whole as f32;
    envelope[whole] * (1.0 - frac) + envelope[whole + 1] * frac
}
