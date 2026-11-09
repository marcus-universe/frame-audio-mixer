//! Stereo effects at 48 kHz. Parameters are 0..1.

use std::sync::Arc;

use super::analyzer::Scope;
use super::pitch::Pitcher;
use super::reverb::Reverb;
use crate::config::{EffectKind, EffectSlot};

const RATE: f64 = 48_000.0;
/// Scratch size. The engine hands over one speaker period, which is 480 frames.
const FRAMES: usize = 480;

pub struct Processor {
    stages: Vec<Stage>,
    /// Taps one chain position for the EQ's analyzer while a panel is watching.
    scope: Option<Arc<Scope>>,
}

enum Stage {
    Reverb(Box<Reverb>),
    Comp(Comp),
    /// Boxed: the biquad pairs dwarf every other variant.
    Eq(Box<Eq>),
    Pitcher(Box<Pitcher>),
    Flanger(Flanger),
    Phaser(Box<Phaser>),
    Delay(Delay),
    Vst(Box<VstLink>),
}

impl Processor {
    pub fn new() -> Self {
        Self { stages: Vec::new(), scope: None }
    }

    pub fn with_scope(scope: Arc<Scope>) -> Self {
        Self { stages: Vec::new(), scope: Some(scope) }
    }

    pub fn set_chain(&mut self, chain: &[EffectSlot]) {
        let same = self.stages.len() == chain.len() && self.stages.iter().zip(chain).all(|(stage, slot)| stage.matches(slot));
        if !same {
            self.stages = chain.iter().map(Stage::new).collect();
        }
        for (stage, slot) in self.stages.iter_mut().zip(chain) {
            stage.apply(slot);
        }
    }

    /// The same chain on separate left and right buffers, as a PipeWire filter hands them over.
    pub fn process_planar(&mut self, left: &mut [f32], right: &mut [f32]) {
        if self.stages.is_empty() {
            return;
        }
        let n = left.len().min(right.len());
        let mut offset = 0;
        while offset < n {
            let end = (offset + FRAMES).min(n);
            let tapped = self.scope.as_ref().map_or(u32::MAX, |scope| scope.armed_slot());
            for (index, stage) in self.stages.iter_mut().enumerate() {
                stage.process(&mut left[offset..end], &mut right[offset..end]);
                if index as u32 == tapped {
                    if let Some(scope) = &self.scope {
                        scope.push(&left[offset..end], &right[offset..end]);
                    }
                }
            }
            for i in offset..end {
                left[i] = soft_ceiling(left[i]);
                right[i] = soft_ceiling(right[i]);
            }
            offset = end;
        }
    }

    pub fn process(&mut self, interleaved: &mut [f32]) {
        if self.stages.is_empty() {
            return;
        }
        let mut offset = 0;
        while offset + 1 < interleaved.len() {
            let frames = ((interleaved.len() - offset) / 2).min(FRAMES);
            let mut left = [0.0f32; FRAMES];
            let mut right = [0.0f32; FRAMES];
            for i in 0..frames {
                left[i] = interleaved[offset + i * 2];
                right[i] = interleaved[offset + i * 2 + 1];
            }
            for stage in &mut self.stages {
                stage.process(&mut left[..frames], &mut right[..frames]);
            }
            for i in 0..frames {
                interleaved[offset + i * 2] = soft_ceiling(left[i]);
                interleaved[offset + i * 2 + 1] = soft_ceiling(right[i]);
            }
            offset += frames * 2;
        }
    }
}

/// Linear through most of the range, then a knee so a hot send does not crackle.
pub(crate) fn soft_ceiling(sample: f32) -> f32 {
    let x = sample.clamp(-8.0, 8.0);
    let ax = x.abs();
    let shaped = if ax <= 0.8 {
        ax
    } else {
        let t = (ax - 0.8) / 0.8;
        0.8 + 0.2 * (1.0 - 1.0 / (1.0 + t))
    };
    shaped.copysign(x)
}

impl Stage {
    fn new(slot: &EffectSlot) -> Self {
        if let (EffectKind::Vst, Some(plugin)) = (slot.kind, &slot.vst) {
            return Stage::Vst(Box::new(VstLink::new(&plugin.instance)));
        }
        match slot.kind {
            EffectKind::Reverb => Stage::Reverb(Box::new(Reverb::new())),
            EffectKind::Comp => Stage::Comp(Comp::new()),
            EffectKind::Eq => Stage::Eq(Box::new(Eq::new())),
            EffectKind::Pitcher => Stage::Pitcher(Box::new(Pitcher::new())),
            EffectKind::Flanger => Stage::Flanger(Flanger::new()),
            EffectKind::Phaser => Stage::Phaser(Box::new(Phaser::new())),
            EffectKind::Delay => Stage::Delay(Delay::new()),
            // A VST slot without its plugin passes audio through.
            EffectKind::Vst => Stage::Vst(Box::new(VstLink::new(""))),
        }
    }

    fn matches(&self, slot: &EffectSlot) -> bool {
        match (self, &slot.vst) {
            (Stage::Vst(link), Some(plugin)) => link.instance == plugin.instance,
            _ => self.kind() == slot.kind,
        }
    }

    fn kind(&self) -> EffectKind {
        match self {
            Stage::Reverb(_) => EffectKind::Reverb,
            Stage::Comp(_) => EffectKind::Comp,
            Stage::Eq(_) => EffectKind::Eq,
            Stage::Pitcher(_) => EffectKind::Pitcher,
            Stage::Flanger(_) => EffectKind::Flanger,
            Stage::Phaser(_) => EffectKind::Phaser,
            Stage::Delay(_) => EffectKind::Delay,
            Stage::Vst(_) => EffectKind::Vst,
        }
    }

    fn apply(&mut self, slot: &EffectSlot) {
        let extra = slot.extra.map(|value| value.clamp(0.0, 1.0));
        match self {
            Stage::Reverb(fx) => fx.set(slot.params, extra),
            Stage::Comp(fx) => fx.set(slot.params, extra),
            Stage::Eq(fx) => fx.apply(slot),
            Stage::Pitcher(fx) => fx.set(slot.params),
            Stage::Flanger(fx) => fx.set(slot.params, extra),
            Stage::Phaser(fx) => fx.set(slot.params, extra),
            Stage::Delay(fx) => fx.set(slot.params, extra),
            Stage::Vst(_) => {}
        }
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        match self {
            Stage::Reverb(fx) => fx.process(left, right),
            Stage::Comp(fx) => fx.process(left, right),
            Stage::Eq(fx) => fx.process(left, right),
            Stage::Pitcher(fx) => fx.process(left, right),
            Stage::Flanger(fx) => fx.process(left, right),
            Stage::Phaser(fx) => fx.process(left, right),
            Stage::Delay(fx) => fx.process(left, right),
            Stage::Vst(link) => link.process(left, right),
        }
    }
}

/// Retry the region about once a second while the host is starting.
const VST_RETRY_BLOCKS: u32 = 100;

/// Trades blocks with a plugin host through shared memory. Output is one block
/// late, so this thread never waits. While the host is starting, crashed or
/// behind, audio passes through unchanged, like a bypassed plugin.
struct VstLink {
    instance: String,
    region: Option<crate::vst::shared::Region>,
    retry: u32,
    held: [Vec<f32>; 2],
}

impl VstLink {
    fn new(instance: &str) -> Self {
        let frames = crate::vst::shared::MAX_FRAMES;
        Self { instance: instance.to_string(), region: None, retry: 0, held: [vec![0.0; frames], vec![0.0; frames]] }
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        use crate::vst::shared::{Plane, Region, STATUS_RUNNING};
        use std::sync::atomic::Ordering;
        if self.instance.is_empty() {
            return;
        }
        if self.region.is_none() {
            if self.retry == 0 {
                // Only a few syscalls, about once a second, until the engine has made the region.
                self.region = Region::open(&crate::vst::region_path(&self.instance)).ok();
                self.retry = VST_RETRY_BLOCKS;
            }
            self.retry -= 1;
            return;
        }
        let Some(region) = &self.region else { return };
        let header = region.header();
        let frames = left.len().min(right.len()).min(crate::vst::shared::MAX_FRAMES);
        if header.status.load(Ordering::Acquire) != STATUS_RUNNING {
            return;
        }
        let input = header.input_seq.load(Ordering::Relaxed);
        if header.output_seq.load(Ordering::Acquire) != input {
            header.late.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let [held_l, held_r] = &mut self.held;
        region.read(Plane::OutLeft, &mut held_l[..frames]);
        region.read(Plane::OutRight, &mut held_r[..frames]);
        region.write(Plane::InLeft, &left[..frames]);
        region.write(Plane::InRight, &right[..frames]);
        header.frames.store(frames as u32, Ordering::Relaxed);
        header.input_seq.store(input.wrapping_add(1), Ordering::Release);
        left[..frames].copy_from_slice(&held_l[..frames]);
        right[..frames].copy_from_slice(&held_r[..frames]);
    }
}

struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Biquad {
    fn peaking(freq: f64, q: f64, gain_db: f64) -> Self {
        let a = 10f64.powf(gain_db / 40.0);
        let w = 2.0 * std::f64::consts::PI * freq / RATE;
        let alpha = w.sin() / (2.0 * q);
        let cos = w.cos();
        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos;
        let a2 = 1.0 - alpha / a;
        Self { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0, z1: 0.0, z2: 0.0 }
    }

    fn lowshelf(freq: f64, gain_db: f64) -> Self {
        let a = 10f64.powf(gain_db / 40.0);
        let w = 2.0 * std::f64::consts::PI * freq / RATE;
        let cos = w.cos();
        let sin = w.sin();
        let beta = (a + a).sqrt();
        let b0 = a * ((a + 1.0) - (a - 1.0) * cos + beta * sin);
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos);
        let b2 = a * ((a + 1.0) - (a - 1.0) * cos - beta * sin);
        let a0 = (a + 1.0) + (a - 1.0) * cos + beta * sin;
        let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos);
        let a2 = (a + 1.0) + (a - 1.0) * cos - beta * sin;
        Self { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0, z1: 0.0, z2: 0.0 }
    }

    fn lowpass(freq: f64, q: f64) -> Self {
        let w = 2.0 * std::f64::consts::PI * freq / RATE;
        let alpha = w.sin() / (2.0 * q.max(0.05));
        let cos = w.cos();
        let b0 = (1.0 - cos) / 2.0;
        let b1 = 1.0 - cos;
        let b2 = (1.0 - cos) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos;
        let a2 = 1.0 - alpha;
        Self { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0, z1: 0.0, z2: 0.0 }
    }

    fn highpass(freq: f64, q: f64) -> Self {
        let w = 2.0 * std::f64::consts::PI * freq / RATE;
        let alpha = w.sin() / (2.0 * q.max(0.05));
        let cos = w.cos();
        let b0 = (1.0 + cos) / 2.0;
        let b1 = -(1.0 + cos);
        let b2 = (1.0 + cos) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos;
        let a2 = 1.0 - alpha;
        Self { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0, z1: 0.0, z2: 0.0 }
    }

    fn highshelf(freq: f64, gain_db: f64) -> Self {
        let a = 10f64.powf(gain_db / 40.0);
        let w = 2.0 * std::f64::consts::PI * freq / RATE;
        let cos = w.cos();
        let sin = w.sin();
        let beta = (a + a).sqrt();
        let b0 = a * ((a + 1.0) + (a - 1.0) * cos + beta * sin);
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos);
        let b2 = a * ((a + 1.0) + (a - 1.0) * cos - beta * sin);
        let a0 = (a + 1.0) - (a - 1.0) * cos + beta * sin;
        let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos);
        let a2 = (a + 1.0) - (a - 1.0) * cos - beta * sin;
        Self { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0, z1: 0.0, z2: 0.0 }
    }

    fn allpass(freq: f64) -> Self {
        let w = 2.0 * std::f64::consts::PI * freq / RATE;
        let tan = (w / 2.0).tan();
        let a1 = (tan - 1.0) / (tan + 1.0);
        Self { b0: a1, b1: 1.0, b2: 0.0, a1, a2: 0.0, z1: 0.0, z2: 0.0 }
    }

    fn mag_db(&self, freq: f64) -> f64 {
        let w = 2.0 * std::f64::consts::PI * freq / RATE;
        let c1 = w.cos();
        let s1 = w.sin();
        let c2 = (2.0 * w).cos();
        let s2 = (2.0 * w).sin();
        let num_re = self.b0 + self.b1 * c1 + self.b2 * c2;
        let num_im = -(self.b1 * s1 + self.b2 * s2);
        let den_re = 1.0 + self.a1 * c1 + self.a2 * c2;
        let den_im = -(self.a1 * s1 + self.a2 * s2);
        let num = num_re * num_re + num_im * num_im;
        let den = den_re * den_re + den_im * den_im;
        if den <= 1e-24 || num <= 0.0 {
            return -120.0;
        }
        10.0 * (num / den).log10()
    }

    fn tick(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }

    /// New coefficients, same history. Replacing the filter zeroed the tail every block.
    fn apply_coeffs(&mut self, next: Self) {
        self.b0 = next.b0;
        self.b1 = next.b1;
        self.b2 = next.b2;
        self.a1 = next.a1;
        self.a2 = next.a2;
    }
}

const EQ_BANDS: usize = 12;

struct Eq {
    left: [Biquad; EQ_BANDS],
    right: [Biquad; EQ_BANDS],
    count: usize,
    bypass: bool,
    /// Last design, so a repeated set does not rebuild identical coefficients.
    design: Vec<(f64, f64, f64, u8)>,
}

impl Eq {
    fn new() -> Self {
        Self {
            left: std::array::from_fn(|_| Biquad::peaking(1000.0, 0.71, 0.0)),
            right: std::array::from_fn(|_| Biquad::peaking(1000.0, 0.71, 0.0)),
            count: 0,
            bypass: false,
            design: Vec::new(),
        }
    }

    fn apply(&mut self, slot: &EffectSlot) {
        let eq = slot.eq.clone().unwrap_or_else(|| legacy_eq(slot.params));
        self.bypass = eq.bypass;
        let mut design = Vec::with_capacity(eq.bands.len());
        for (index, band) in eq.bands.iter().enumerate() {
            let kind = if index == 0 {
                eq.low
            } else if index + 1 == eq.bands.len() {
                eq.high
            } else {
                crate::config::BandType::Bell
            };
            let tag = match kind {
                crate::config::BandType::Bell => 0,
                crate::config::BandType::Shelf => 1,
                crate::config::BandType::HighPass => 2,
                crate::config::BandType::LowPass => 3,
            };
            design.push((band.freq, band.gain_db, band.q, if band.on { tag } else { 9 }));
        }
        if design == self.design {
            return;
        }
        self.count = design.len().min(EQ_BANDS);
        for (index, (freq, gain, q, tag)) in design.iter().take(EQ_BANDS).enumerate() {
            let coeff = band_coeff(index, design.len(), *freq, *gain, *q, *tag);
            self.left[index].apply_coeffs(coeff);
            self.right[index].apply_coeffs(band_coeff(index, design.len(), *freq, *gain, *q, *tag));
        }
        self.design = design;
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        if self.bypass || self.count == 0 {
            return;
        }
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let mut sample_l = *l as f64;
            let mut sample_r = *r as f64;
            for index in 0..self.count {
                sample_l = self.left[index].tick(sample_l);
                sample_r = self.right[index].tick(sample_r);
            }
            *l = sample_l as f32;
            *r = sample_r as f32;
        }
    }
}

fn legacy_eq(params: [f64; 3]) -> crate::config::ParamEq {
    let mut eq = crate::config::ParamEq::new(3);
    eq.low = crate::config::BandType::Shelf;
    eq.high = crate::config::BandType::Shelf;
    let gains = [(params[0] - 0.5) * 24.0, (params[1] - 0.5) * 24.0, (params[2] - 0.5) * 24.0];
    let freqs = [120.0, 1000.0, 8000.0];
    for (band, (freq, gain)) in eq.bands.iter_mut().zip(freqs.into_iter().zip(gains)) {
        band.freq = freq;
        band.gain_db = gain;
        band.q = 0.8;
    }
    eq
}

fn band_coeff(index: usize, count: usize, freq: f64, gain: f64, q: f64, tag: u8) -> Biquad {
    if tag == 9 {
        return Biquad::peaking(1000.0, 0.71, 0.0);
    }
    let freq = freq.clamp(20.0, 20_000.0);
    let q = q.clamp(0.1, 10.0);
    match tag {
        2 => Biquad::highpass(freq, q),
        3 => Biquad::lowpass(freq, q),
        1 if index == 0 => Biquad::lowshelf(freq, gain),
        1 => Biquad::highshelf(freq, gain),
        _ if count == 1 => Biquad::peaking(freq, q, gain),
        _ => Biquad::peaking(freq, q, gain),
    }
}

/// Magnitude in dB of a parametric EQ at one frequency. Used by the curve.
pub fn eq_response_db(eq: &crate::config::ParamEq, freq: f64) -> f64 {
    if eq.bypass {
        return 0.0;
    }
    let mut sum = 0.0;
    let count = eq.bands.len();
    for (index, band) in eq.bands.iter().enumerate() {
        if !band.on {
            continue;
        }
        let kind = if index == 0 {
            eq.low
        } else if index + 1 == count {
            eq.high
        } else {
            crate::config::BandType::Bell
        };
        let tag = match kind {
            crate::config::BandType::Bell => 0,
            crate::config::BandType::Shelf => 1,
            crate::config::BandType::HighPass => 2,
            crate::config::BandType::LowPass => 3,
        };
        sum += band_coeff(index, count, band.freq, band.gain_db, band.q, tag).mag_db(freq);
    }
    sum
}

struct Comp {
    threshold: f32,
    ratio: f32,
    makeup: f32,
    attack: f32,
    release: f32,
    env: f32,
}

/// One-pole smoothing coefficient for a time constant in seconds.
fn time_coeff(seconds: f64) -> f32 {
    (1.0 - (-1.0 / (seconds * RATE)).exp()) as f32
}

impl Comp {
    fn new() -> Self {
        Self { threshold: 0.3, ratio: 4.0, makeup: 1.0, attack: time_coeff(0.010), release: time_coeff(0.120), env: 0.0 }
    }

    fn set(&mut self, params: [f64; 3], extra: [f64; 3]) {
        let thresh_db = -40.0 + params[0] * 40.0;
        self.threshold = 10f32.powf((thresh_db as f32) / 20.0);
        self.ratio = 1.0 + params[1] as f32 * 19.0;
        self.makeup = 10f32.powf((params[2] as f32) * 24.0 / 20.0);
        // 0.5 is 10 ms attack and 120 ms release; each end is four times that way.
        self.attack = time_coeff(0.010 * 4f64.powf(2.0 * extra[0] - 1.0));
        self.release = time_coeff(0.120 * 4f64.powf(2.0 * extra[1] - 1.0));
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let peak = l.abs().max(r.abs());
            let coeff = if peak > self.env { self.attack } else { self.release };
            self.env += coeff * (peak - self.env);
            let mut gain = 1.0;
            if self.env > self.threshold && self.threshold > 1e-6 {
                let over = self.env / self.threshold;
                let compressed = over.powf(1.0 / self.ratio);
                gain = compressed / over;
            }
            *l *= gain * self.makeup;
            *r *= gain * self.makeup;
        }
    }
}

/// Linear read `delay` samples behind `pos` in a ring buffer.
fn read_ring(buf: &[f32], pos: usize, delay: f32) -> f32 {
    let len = buf.len();
    let whole = delay as usize;
    let frac = delay - whole as f32;
    let newer = buf[(pos + len - whole % len) % len];
    let older = buf[(pos + len - (whole + 1) % len) % len];
    newer * (1.0 - frac) + older * frac
}

struct Delay {
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    pos: usize,
    time: f32,
    time_now: f32,
    feedback: f32,
    mix: f32,
    tone: f32,
    spread: f32,
    tone_l: f32,
    tone_r: f32,
}

impl Delay {
    fn new() -> Self {
        Self {
            buf_l: vec![0.0; 48_000],
            buf_r: vec![0.0; 48_000],
            pos: 0,
            time: 12_000.0,
            time_now: 12_000.0,
            feedback: 0.3,
            mix: 0.3,
            tone: 0.5,
            spread: 0.0,
            tone_l: 0.0,
            tone_r: 0.0,
        }
    }

    fn set(&mut self, params: [f64; 3], extra: [f64; 3]) {
        self.time = (((0.02 + params[0] * 0.78) * RATE) as f32).clamp(1.0, (self.buf_l.len() - 2) as f32);
        self.feedback = params[1] as f32 * 0.92;
        self.mix = params[2] as f32;
        self.tone = time_coeff(1.0 / (std::f64::consts::TAU * 1200.0 * 16f64.powf(extra[0])));
        self.spread = ((extra[1] as f32 - 0.5) * 2.0).max(0.0);
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let len = self.buf_l.len();
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            // Gliding to a new time bends the echo instead of clicking.
            self.time_now += (self.time - self.time_now) * 0.000_8;
            let delayed_l = read_ring(&self.buf_l, self.pos, self.time_now);
            let delayed_r = read_ring(&self.buf_r, self.pos, self.time_now);
            self.tone_l += self.tone * (delayed_l - self.tone_l);
            self.tone_r += self.tone * (delayed_r - self.tone_r);
            let back_l = self.tone_l * (1.0 - self.spread) + self.tone_r * self.spread;
            let back_r = self.tone_r * (1.0 - self.spread) + self.tone_l * self.spread;
            self.buf_l[self.pos] = *l + *r * self.spread + back_l * self.feedback;
            self.buf_r[self.pos] = *r * (1.0 - self.spread) + back_r * self.feedback;
            *l = *l * (1.0 - self.mix) + delayed_l * self.mix;
            *r = *r * (1.0 - self.mix) + delayed_r * self.mix;
            self.pos = (self.pos + 1) % len;
        }
    }
}

struct Flanger {
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    pos: usize,
    phase: f64,
    rate: f64,
    depth: f32,
    mix: f32,
    feedback: f32,
    last_l: f32,
    last_r: f32,
}

impl Flanger {
    fn new() -> Self {
        Self {
            buf_l: vec![0.0; 2048],
            buf_r: vec![0.0; 2048],
            pos: 0,
            phase: 0.0,
            rate: 0.3,
            depth: 0.5,
            mix: 0.35,
            feedback: 0.0,
            last_l: 0.0,
            last_r: 0.0,
        }
    }

    fn set(&mut self, params: [f64; 3], extra: [f64; 3]) {
        self.rate = 0.05 + params[0] * 4.0;
        self.depth = params[1] as f32;
        self.mix = params[2] as f32;
        self.feedback = (extra[0] as f32 - 0.5) * 1.7;
    }

    fn sweep(&self, offset: f64) -> f32 {
        let wave = (((self.phase + offset) * std::f64::consts::TAU).sin() * 0.5 + 0.5) as f32;
        40.0 + 400.0 * self.depth * wave
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let len = self.buf_l.len();
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            self.buf_l[self.pos] = *l + self.last_l * self.feedback;
            self.buf_r[self.pos] = *r + self.last_r * self.feedback;
            let wet_l = read_ring(&self.buf_l, self.pos, self.sweep(0.0));
            let wet_r = read_ring(&self.buf_r, self.pos, self.sweep(0.25));
            self.last_l = wet_l;
            self.last_r = wet_r;
            *l = *l * (1.0 - self.mix) + wet_l * self.mix;
            *r = *r * (1.0 - self.mix) + wet_r * self.mix;
            self.pos = (self.pos + 1) % len;
            self.phase = (self.phase + self.rate / RATE).fract();
        }
    }
}

struct Phaser {
    left: [Biquad; 4],
    right: [Biquad; 4],
    phase: f64,
    rate: f64,
    depth: f64,
    mix: f32,
    feedback: f64,
    last_l: f64,
    last_r: f64,
    until_retune: u32,
}

/// Samples between filter updates. Small enough that the sweep has no audible steps.
const PHASER_RETUNE: u32 = 32;

impl Phaser {
    fn new() -> Self {
        Self {
            left: [Biquad::allpass(800.0), Biquad::allpass(1200.0), Biquad::allpass(2000.0), Biquad::allpass(3200.0)],
            right: [Biquad::allpass(820.0), Biquad::allpass(1240.0), Biquad::allpass(2050.0), Biquad::allpass(3300.0)],
            phase: 0.0,
            rate: 0.3,
            depth: 0.5,
            mix: 0.4,
            feedback: 0.0,
            last_l: 0.0,
            last_r: 0.0,
            until_retune: 0,
        }
    }

    fn set(&mut self, params: [f64; 3], extra: [f64; 3]) {
        self.rate = 0.05 + params[0] * 3.5;
        self.depth = params[1];
        self.mix = params[2] as f32;
        self.feedback = (extra[0] - 0.5) * 1.5;
    }

    fn retune(&mut self) {
        let sweep = (self.phase * std::f64::consts::TAU).sin() * 0.5 + 0.5;
        let freq = 300.0 + (2200.0 * self.depth) * sweep;
        self.left[0].apply_coeffs(Biquad::allpass(freq));
        self.left[1].apply_coeffs(Biquad::allpass(freq * 1.4));
        self.left[2].apply_coeffs(Biquad::allpass(freq * 2.1));
        self.left[3].apply_coeffs(Biquad::allpass(freq * 3.0));
        self.right[0].apply_coeffs(Biquad::allpass(freq * 1.03));
        self.right[1].apply_coeffs(Biquad::allpass(freq * 1.45));
        self.right[2].apply_coeffs(Biquad::allpass(freq * 2.15));
        self.right[3].apply_coeffs(Biquad::allpass(freq * 3.05));
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let mix = f64::from(self.mix);
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            if self.until_retune == 0 {
                self.until_retune = PHASER_RETUNE;
                self.retune();
            }
            self.until_retune -= 1;
            let dry_l = f64::from(*l);
            let dry_r = f64::from(*r);
            let mut wet_l = dry_l + self.last_l * self.feedback;
            let mut wet_r = dry_r + self.last_r * self.feedback;
            for ap in &mut self.left {
                wet_l = ap.tick(wet_l);
            }
            for ap in &mut self.right {
                wet_r = ap.tick(wet_r);
            }
            self.last_l = wet_l;
            self.last_r = wet_r;
            *l = (dry_l * (1.0 - mix) + wet_l * mix) as f32;
            *r = (dry_r * (1.0 - mix) + wet_r * mix) as f32;
            self.phase = (self.phase + self.rate / RATE).fract();
        }
    }
}

pub fn self_test() -> Result<(), String> {
    let mut proc = Processor::new();
    proc.set_chain(&[EffectSlot::new(EffectKind::Eq), EffectSlot::new(EffectKind::Delay)]);
    let mut buf = vec![0.0f32; 1024];
    buf[0] = 0.5;
    buf[1] = -0.5;
    proc.process(&mut buf);
    if buf.iter().any(|s| !s.is_finite()) {
        return Err("dsp nan".into());
    }

    let mut eq = Processor::new();
    let eq_slot = EffectSlot { params: [0.9, 0.5, 0.5], ..EffectSlot::new(EffectKind::Eq) };
    eq.set_chain(std::slice::from_ref(&eq_slot));
    let mut impulse = vec![0.0f32; FRAMES * 2];
    // Last frame, so the next block is pure filter tail. A wiped history is silence.
    impulse[FRAMES * 2 - 2] = 1.0;
    impulse[FRAMES * 2 - 1] = 1.0;
    eq.process(&mut impulse);
    eq.set_chain(&[eq_slot]);
    let mut tail = vec![0.0f32; FRAMES * 2];
    eq.process(&mut tail);
    let energy: f32 = tail.iter().map(|sample| sample.abs()).sum();
    if energy < 1.0e-3 {
        return Err(format!("eq state cleared {energy}"));
    }

    let presets = super::presets::of(EffectKind::Reverb);
    let bathroom = reverb_tail_ms(&presets[1]);
    let cathedral = reverb_tail_ms(&presets[7]);
    if cathedral < bathroom * 4.0 || bathroom < 40.0 {
        return Err(format!("reverb tails bathroom {bathroom} ms, cathedral {cathedral} ms"));
    }
    for preset in presets {
        let level = reverb_wet_level(preset);
        if !(0.05..=1.5).contains(&level) {
            return Err(format!("reverb {} wet level {level}", preset.name(crate::i18n::Lang::En)));
        }
    }

    // +12 used to play the buffer backwards at the original speed, so it
    // sounded lower than the dry signal. Measure the shifted tone.
    let mut bell = crate::config::ParamEq::new(3);
    bell.bands[1].freq = 1000.0;
    bell.bands[1].gain_db = 6.0;
    bell.bands[1].q = 2.0;
    bell.bands[0].on = false;
    bell.bands[2].on = false;
    let at = eq_response_db(&bell, 1000.0);
    if (at - 6.0).abs() > 0.75 {
        return Err(format!("bell response {at}"));
    }
    let mut hipass = crate::config::ParamEq::new(3);
    hipass.low = crate::config::BandType::HighPass;
    hipass.bands[0].freq = 1000.0;
    hipass.bands[0].q = std::f64::consts::FRAC_1_SQRT_2;
    hipass.bands[1].on = false;
    hipass.bands[2].on = false;
    let cut = eq_response_db(&hipass, 1000.0);
    if (cut + 3.0).abs() > 1.5 {
        return Err(format!("highpass response {cut}"));
    }

    for (semitones, expect) in [(12.0, 880.0), (0.0, 440.0), (-12.0, 220.0)] {
        let hz = pitched_hz(semitones);
        if (hz - expect).abs() / expect > 0.03 {
            return Err(format!("pitch {semitones:+} measured {hz:.1} Hz, expected {expect}"));
        }
    }
    for semitones in [-12.0, -5.0, 7.0, 12.0] {
        let tone = pitched(semitones, |t| (std::f64::consts::TAU * 440.0 * t).sin() as f32 * 0.5);
        let snr = tone_snr(&tone, 440.0 * 2f64.powf(semitones / 12.0));
        if snr < 20.0 {
            return Err(format!("pitch {semitones:+} tone SNR {snr:.1} dB"));
        }
    }
    Ok(())
}

/// One second of 440 Hz through the pitcher, then the frequency of the tail.
fn pitched_hz(semitones: f64) -> f64 {
    let mut proc = Processor::new();
    let mut slot = EffectSlot::new(EffectKind::Pitcher);
    slot.params[0] = crate::config::pitch_param(semitones);
    proc.set_chain(std::slice::from_ref(&slot));
    let rate = 48_000.0;
    let total = rate as usize;
    let mut out = Vec::with_capacity(total);
    let mut n = 0;
    while n < total {
        let frames = FRAMES.min(total - n);
        let mut block = vec![0.0f32; frames * 2];
        for i in 0..frames {
            let t = (n + i) as f64 / rate;
            let sample = (std::f64::consts::TAU * 440.0 * t).sin() as f32;
            block[i * 2] = sample;
            block[i * 2 + 1] = sample;
        }
        proc.process(&mut block);
        out.extend(block.iter().step_by(2).copied());
        n += frames;
    }
    let start = (0.25 * rate) as usize;
    let mut last: Option<f64> = None;
    let mut periods = Vec::new();
    for i in (start + 1)..out.len() {
        if out[i - 1] <= 0.0 && out[i] > 0.0 {
            let prev = f64::from(out[i - 1]);
            let cur = f64::from(out[i]);
            let delta = cur - prev;
            let frac = if delta.abs() < 1e-9 { 0.0 } else { -prev / delta };
            let at = (i - 1) as f64 + frac;
            if let Some(prev) = last {
                let period = at - prev;
                if period > 8.0 {
                    periods.push(period);
                }
            }
            last = Some(at);
        }
    }
    if periods.is_empty() {
        return 0.0;
    }
    rate / (periods.iter().sum::<f64>() / periods.len() as f64)
}

fn reverb_with(preset: &super::presets::Preset) -> Processor {
    let mut slot = EffectSlot::new(EffectKind::Reverb);
    preset.apply(&mut slot);
    slot.params[2] = 1.0;
    let mut proc = Processor::new();
    proc.set_chain(std::slice::from_ref(&slot));
    proc
}

fn test_noise() -> impl FnMut() -> f32 {
    let mut seed = 0x9E37_79B9u32;
    move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32 / u32::MAX as f32 - 0.5) * 0.5
    }
}

/// Milliseconds for the fully wet tail to fall 30 dB after a noise burst stops.
fn reverb_tail_ms(preset: &super::presets::Preset) -> f64 {
    let mut proc = reverb_with(preset);
    let mut noise = test_noise();
    for _ in 0..30 {
        let mut left = [0.0f32; FRAMES];
        let mut right = [0.0f32; FRAMES];
        left.iter_mut().for_each(|s| *s = noise());
        right.iter_mut().for_each(|s| *s = noise());
        proc.process_planar(&mut left, &mut right);
    }
    let mut start = None;
    for block in 0..1500 {
        let mut left = [0.0f32; FRAMES];
        let mut right = [0.0f32; FRAMES];
        proc.process_planar(&mut left, &mut right);
        let energy: f32 = left.iter().map(|s| s * s).sum();
        // Skips pre-delay and the early reflections as the reference level.
        if block == 12 {
            start = Some(energy);
        }
        if let Some(reference) = start {
            if energy < reference * 0.001 {
                return (block - 12) as f64 * 10.0;
            }
        }
    }
    15_000.0
}

/// Output against input RMS of a fully wet reverb on steady noise.
fn reverb_wet_level(preset: &super::presets::Preset) -> f32 {
    let mut proc = reverb_with(preset);
    let mut noise = test_noise();
    let (mut input, mut output) = (0.0f32, 0.0f32);
    for block in 0..400 {
        let mut left = [0.0f32; FRAMES];
        let mut right = [0.0f32; FRAMES];
        left.iter_mut().for_each(|s| *s = noise());
        right.iter_mut().for_each(|s| *s = noise());
        let energy: f32 = left.iter().map(|s| s * s).sum();
        proc.process_planar(&mut left, &mut right);
        if block > 200 {
            input += energy;
            output += left.iter().map(|s| s * s).sum::<f32>();
        }
    }
    (output / input.max(1e-12)).sqrt()
}

/// Prints CPU load per effect and pitch-shift quality. Run a release build:
/// `frame-audio-mixer --dsp-report`. Debug timings mean nothing here.
pub fn report() {
    const SECONDS: usize = 10;
    let mut seed = 0x1234_5678u32;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32 / u32::MAX as f32 - 0.5) * 0.5
    };
    println!("effect      load of one core");
    for kind in EffectKind::ALL {
        let mut proc = Processor::new();
        proc.set_chain(&[EffectSlot::new(kind)]);
        let mut left = [0.0f32; FRAMES];
        let mut right = [0.0f32; FRAMES];
        let blocks = SECONDS * RATE as usize / FRAMES;
        let mut worst = std::time::Duration::ZERO;
        let start = std::time::Instant::now();
        for _ in 0..blocks {
            left.iter_mut().for_each(|s| *s = noise());
            right.iter_mut().for_each(|s| *s = noise());
            let block = std::time::Instant::now();
            proc.process_planar(&mut left, &mut right);
            worst = worst.max(block.elapsed());
        }
        let share = start.elapsed().as_secs_f64() / SECONDS as f64 * 100.0;
        println!("{:<10}  {share:5.2}%   worst block {:.0} us of 10000", kind.key(), worst.as_secs_f64() * 1e6);
    }
    println!("semitones   tone SNR dB   tone ripple dB   voice ripple dB");
    for semitones in [-12.0, -7.0, -5.0, -2.0, 2.0, 5.0, 7.0, 12.0] {
        let ratio = 2f64.powf(semitones / 12.0);
        let tone = pitched(semitones, |t| (std::f64::consts::TAU * 440.0 * t).sin() as f32 * 0.5);
        let voice = pitched(semitones, |t| {
            (1..=24).map(|h| (std::f64::consts::TAU * 130.0 * f64::from(h) * t).sin() as f32 / h as f32).sum::<f32>() * 0.25
        });
        println!("{semitones:+6.0}      {:8.1}   {:12.2}   {:12.2}", tone_snr(&tone, 440.0 * ratio), ripple_db(&tone), ripple_db(&voice));
    }
}

/// The settled second of a two-second signal through the pitcher, left channel.
fn pitched(semitones: f64, signal: impl Fn(f64) -> f32) -> Vec<f32> {
    let mut proc = Processor::new();
    let mut slot = EffectSlot::new(EffectKind::Pitcher);
    slot.params[0] = crate::config::pitch_param(semitones);
    proc.set_chain(std::slice::from_ref(&slot));
    let total = RATE as usize * 2;
    let mut out = Vec::with_capacity(total);
    let mut at = 0;
    while at < total {
        let mut left = [0.0f32; FRAMES];
        for (i, sample) in left.iter_mut().enumerate() {
            *sample = signal((at + i) as f64 / RATE);
        }
        let mut right = left;
        proc.process_planar(&mut left, &mut right);
        out.extend_from_slice(&left);
        at += FRAMES;
    }
    out.split_off(RATE as usize)
}

/// Signal against everything that is not a pure sine at `hz`, by least squares.
fn tone_snr(samples: &[f32], hz: f64) -> f64 {
    let w = std::f64::consts::TAU * hz / RATE;
    let (mut a, mut b, n) = (0.0, 0.0, samples.len() as f64);
    for (i, s) in samples.iter().enumerate() {
        a += f64::from(*s) * (w * i as f64).sin();
        b += f64::from(*s) * (w * i as f64).cos();
    }
    let (a, b) = (2.0 * a / n, 2.0 * b / n);
    let (mut signal, mut rest) = (0.0, 0.0);
    for (i, s) in samples.iter().enumerate() {
        let fit = a * (w * i as f64).sin() + b * (w * i as f64).cos();
        signal += fit * fit;
        rest += (f64::from(*s) - fit).powi(2);
    }
    10.0 * (signal / rest.max(1e-30)).log10()
}

/// Loudest against quietest 20 ms of a steady signal. Grain splices show up here.
fn ripple_db(samples: &[f32]) -> f64 {
    let levels: Vec<f64> = samples.as_chunks::<960>().0.iter().map(|c| c.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>()).collect();
    let high = levels.iter().copied().fold(0.0, f64::max);
    let low = levels.iter().copied().fold(f64::MAX, f64::min);
    10.0 * (high / low.max(1e-30)).log10()
}
