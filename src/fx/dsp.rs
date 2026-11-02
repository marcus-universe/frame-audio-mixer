//! Stereo effects at 48 kHz. Parameters are 0..1.

use crate::config::{EffectKind, EffectSlot};

const RATE: f64 = 48_000.0;
const FRAMES: usize = 512;

pub struct Processor {
    stages: Vec<Stage>,
}

enum Stage {
    Reverb(Reverb),
    Comp(Comp),
    Eq(Eq),
    Pitcher(Pitcher),
    Flanger(Flanger),
    Phaser(Phaser),
    Delay(Delay),
}

impl Processor {
    pub fn new() -> Self {
        Self { stages: Vec::new() }
    }

    pub fn set_chain(&mut self, chain: &[EffectSlot]) {
        let kinds: Vec<EffectKind> = chain.iter().map(|slot| slot.kind).collect();
        let same = self.stages.len() == kinds.len()
            && self.stages.iter().zip(&kinds).all(|(stage, kind)| stage.kind() == *kind);
        if !same {
            self.stages = kinds.into_iter().map(Stage::new).collect();
        }
        for (stage, slot) in self.stages.iter_mut().zip(chain) {
            stage.set_params(slot.params);
        }
    }

    pub fn process(&mut self, interleaved: &mut [f32]) {
        if self.stages.is_empty() {
            return;
        }
        let mut left = [0.0f32; FRAMES];
        let mut right = [0.0f32; FRAMES];
        let frames = (interleaved.len() / 2).min(FRAMES);
        for i in 0..frames {
            left[i] = interleaved[i * 2];
            right[i] = interleaved[i * 2 + 1];
        }
        for stage in &mut self.stages {
            stage.process(&mut left[..frames], &mut right[..frames]);
        }
        for i in 0..frames {
            interleaved[i * 2] = left[i].clamp(-1.0, 1.0);
            interleaved[i * 2 + 1] = right[i].clamp(-1.0, 1.0);
        }
    }
}

impl Stage {
    fn new(kind: EffectKind) -> Self {
        match kind {
            EffectKind::Reverb => Stage::Reverb(Reverb::new()),
            EffectKind::Comp => Stage::Comp(Comp::new()),
            EffectKind::Eq => Stage::Eq(Eq::new()),
            EffectKind::Pitcher => Stage::Pitcher(Pitcher::new()),
            EffectKind::Flanger => Stage::Flanger(Flanger::new()),
            EffectKind::Phaser => Stage::Phaser(Phaser::new()),
            EffectKind::Delay => Stage::Delay(Delay::new()),
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
        }
    }

    fn set_params(&mut self, params: [f64; 3]) {
        match self {
            Stage::Reverb(fx) => fx.set(params),
            Stage::Comp(fx) => fx.set(params),
            Stage::Eq(fx) => fx.set(params),
            Stage::Pitcher(fx) => fx.set(params),
            Stage::Flanger(fx) => fx.set(params),
            Stage::Phaser(fx) => fx.set(params),
            Stage::Delay(fx) => fx.set(params),
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
        }
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

    fn tick(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

struct Eq {
    low_l: Biquad,
    low_r: Biquad,
    mid_l: Biquad,
    mid_r: Biquad,
    high_l: Biquad,
    high_r: Biquad,
}

impl Eq {
    fn new() -> Self {
        Self {
            low_l: Biquad::lowshelf(120.0, 0.0),
            low_r: Biquad::lowshelf(120.0, 0.0),
            mid_l: Biquad::peaking(1000.0, 0.8, 0.0),
            mid_r: Biquad::peaking(1000.0, 0.8, 0.0),
            high_l: Biquad::highshelf(8000.0, 0.0),
            high_r: Biquad::highshelf(8000.0, 0.0),
        }
    }

    fn set(&mut self, params: [f64; 3]) {
        let low = (params[0] - 0.5) * 24.0;
        let mid = (params[1] - 0.5) * 24.0;
        let high = (params[2] - 0.5) * 24.0;
        self.low_l = Biquad::lowshelf(120.0, low);
        self.low_r = Biquad::lowshelf(120.0, low);
        self.mid_l = Biquad::peaking(1000.0, 0.8, mid);
        self.mid_r = Biquad::peaking(1000.0, 0.8, mid);
        self.high_l = Biquad::highshelf(8000.0, high);
        self.high_r = Biquad::highshelf(8000.0, high);
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            *l = self.high_l.tick(self.mid_l.tick(self.low_l.tick(*l as f64))) as f32;
            *r = self.high_r.tick(self.mid_r.tick(self.low_r.tick(*r as f64))) as f32;
        }
    }
}

struct Comp {
    threshold: f32,
    ratio: f32,
    makeup: f32,
    env: f32,
}

impl Comp {
    fn new() -> Self {
        Self { threshold: 0.3, ratio: 4.0, makeup: 1.0, env: 0.0 }
    }

    fn set(&mut self, params: [f64; 3]) {
        let thresh_db = -40.0 + params[0] * 40.0;
        self.threshold = 10f32.powf((thresh_db as f32) / 20.0);
        self.ratio = 1.0 + params[1] as f32 * 19.0;
        self.makeup = 10f32.powf((params[2] as f32) * 24.0 / 20.0);
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let attack = 0.01;
        let release = 0.002;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let peak = l.abs().max(r.abs());
            let coeff = if peak > self.env { attack } else { release };
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

struct Comb {
    buf: Vec<f32>,
    pos: usize,
    feedback: f32,
}

impl Comb {
    fn new(len: usize) -> Self {
        Self { buf: vec![0.0; len.max(1)], pos: 0, feedback: 0.7 }
    }

    fn tick(&mut self, x: f32) -> f32 {
        let y = self.buf[self.pos];
        self.buf[self.pos] = x + y * self.feedback;
        self.pos += 1;
        if self.pos >= self.buf.len() {
            self.pos = 0;
        }
        y
    }
}

struct Allpass {
    buf: Vec<f32>,
    pos: usize,
}

impl Allpass {
    fn new(len: usize) -> Self {
        Self { buf: vec![0.0; len.max(1)], pos: 0 }
    }

    fn tick(&mut self, x: f32) -> f32 {
        let buf = self.buf[self.pos];
        let y = buf - x;
        self.buf[self.pos] = x + buf * 0.5;
        self.pos += 1;
        if self.pos >= self.buf.len() {
            self.pos = 0;
        }
        y
    }
}

struct Reverb {
    combs_l: [Comb; 4],
    combs_r: [Comb; 4],
    ap_l: [Allpass; 2],
    ap_r: [Allpass; 2],
    mix: f32,
}

impl Reverb {
    fn new() -> Self {
        Self {
            combs_l: [Comb::new(1116), Comb::new(1188), Comb::new(1277), Comb::new(1356)],
            combs_r: [Comb::new(1139), Comb::new(1211), Comb::new(1300), Comb::new(1379)],
            ap_l: [Allpass::new(556), Allpass::new(441)],
            ap_r: [Allpass::new(579), Allpass::new(464)],
            mix: 0.25,
        }
    }

    fn set(&mut self, params: [f64; 3]) {
        let size = 0.5 + params[0] * 1.5;
        let decay = 0.35 + params[1] * 0.55;
        self.mix = params[2] as f32;
        for comb in self.combs_l.iter_mut().chain(self.combs_r.iter_mut()) {
            comb.feedback = decay as f32;
            let len = ((comb.buf.len() as f64) * size / 1.2).clamp(64.0, 4000.0) as usize;
            if comb.buf.len() != len {
                comb.buf = vec![0.0; len];
                comb.pos = 0;
            }
        }
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let mix = self.mix;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let dry_l = *l;
            let dry_r = *r;
            let mut wet_l = 0.0;
            let mut wet_r = 0.0;
            for comb in &mut self.combs_l {
                wet_l += comb.tick(dry_l);
            }
            for comb in &mut self.combs_r {
                wet_r += comb.tick(dry_r);
            }
            wet_l *= 0.25;
            wet_r *= 0.25;
            for ap in &mut self.ap_l {
                wet_l = ap.tick(wet_l);
            }
            for ap in &mut self.ap_r {
                wet_r = ap.tick(wet_r);
            }
            *l = dry_l * (1.0 - mix) + wet_l * mix;
            *r = dry_r * (1.0 - mix) + wet_r * mix;
        }
    }
}

struct Delay {
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    pos: usize,
    time: usize,
    feedback: f32,
    mix: f32,
}

impl Delay {
    fn new() -> Self {
        Self {
            buf_l: vec![0.0; 48_000],
            buf_r: vec![0.0; 48_000],
            pos: 0,
            time: 12_000,
            feedback: 0.3,
            mix: 0.3,
        }
    }

    fn set(&mut self, params: [f64; 3]) {
        self.time = ((0.02 + params[0] * 0.78) * RATE) as usize;
        self.time = self.time.clamp(1, self.buf_l.len() - 1);
        self.feedback = params[1] as f32 * 0.92;
        self.mix = params[2] as f32;
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let len = self.buf_l.len();
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let read = (self.pos + len - self.time) % len;
            let delayed_l = self.buf_l[read];
            let delayed_r = self.buf_r[read];
            self.buf_l[self.pos] = *l + delayed_l * self.feedback;
            self.buf_r[self.pos] = *r + delayed_r * self.feedback;
            *l = *l * (1.0 - self.mix) + delayed_l * self.mix;
            *r = *r * (1.0 - self.mix) + delayed_r * self.mix;
            self.pos += 1;
            if self.pos >= len {
                self.pos = 0;
            }
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
        }
    }

    fn set(&mut self, params: [f64; 3]) {
        self.rate = 0.05 + params[0] * 4.0;
        self.depth = params[1] as f32;
        self.mix = params[2] as f32;
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let len = self.buf_l.len();
        let min_d = 40.0;
        let max_d = 40.0 + 400.0 * self.depth as f64;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            self.buf_l[self.pos] = *l;
            self.buf_r[self.pos] = *r;
            let sweep = (self.phase * std::f64::consts::TAU).sin() * 0.5 + 0.5;
            let delay = min_d + (max_d - min_d) * sweep;
            let read = self.pos as f64 - delay;
            let i0 = ((read as i32).rem_euclid(len as i32)) as usize;
            let i1 = (i0 + 1) % len;
            let frac = (read.fract() + 1.0).fract() as f32;
            let wet_l = self.buf_l[i0] * (1.0 - frac) + self.buf_l[i1] * frac;
            let wet_r = self.buf_r[i0] * (1.0 - frac) + self.buf_r[i1] * frac;
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
}

impl Phaser {
    fn new() -> Self {
        Self {
            left: [Biquad::allpass(800.0), Biquad::allpass(1200.0), Biquad::allpass(2000.0), Biquad::allpass(3200.0)],
            right: [Biquad::allpass(820.0), Biquad::allpass(1240.0), Biquad::allpass(2050.0), Biquad::allpass(3300.0)],
            phase: 0.0,
            rate: 0.3,
            depth: 0.5,
            mix: 0.4,
        }
    }

    fn set(&mut self, params: [f64; 3]) {
        self.rate = 0.05 + params[0] * 3.5;
        self.depth = params[1];
        self.mix = params[2] as f32;
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let sweep = (self.phase * std::f64::consts::TAU).sin() * 0.5 + 0.5;
            let freq = 300.0 + (2200.0 * self.depth) * sweep;
            self.left[0] = Biquad::allpass(freq);
            self.left[1] = Biquad::allpass(freq * 1.4);
            self.left[2] = Biquad::allpass(freq * 2.1);
            self.left[3] = Biquad::allpass(freq * 3.0);
            self.right[0] = Biquad::allpass(freq * 1.03);
            self.right[1] = Biquad::allpass(freq * 1.45);
            self.right[2] = Biquad::allpass(freq * 2.15);
            self.right[3] = Biquad::allpass(freq * 3.05);
            let dry_l = *l as f64;
            let dry_r = *r as f64;
            let mut wet_l = dry_l;
            let mut wet_r = dry_r;
            for ap in &mut self.left {
                wet_l = ap.tick(wet_l);
            }
            for ap in &mut self.right {
                wet_r = ap.tick(wet_r);
            }
            *l = (dry_l * (1.0 - self.mix as f64) + wet_l * self.mix as f64) as f32;
            *r = (dry_r * (1.0 - self.mix as f64) + wet_r * self.mix as f64) as f32;
            self.phase = (self.phase + self.rate / RATE).fract();
        }
    }
}

struct Pitcher {
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    write: usize,
    ratio: f32,
}

impl Pitcher {
    fn new() -> Self {
        Self { buf_l: vec![0.0; 4096], buf_r: vec![0.0; 4096], write: 0, ratio: 1.0 }
    }

    fn set(&mut self, params: [f64; 3]) {
        let semitones = (params[0] - 0.5) * 24.0;
        self.ratio = 2f32.powf(semitones as f32 / 12.0);
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let len = self.buf_l.len();
        let grain = 1024.0;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            self.buf_l[self.write] = *l;
            self.buf_r[self.write] = *r;
            let mut acc_l = 0.0;
            let mut acc_r = 0.0;
            let mut wsum = 0.0;
            for grain_i in 0..2 {
                let phase = (self.write as f32 / grain + grain_i as f32 * 0.5).fract();
                let pos = self.write as f32 - phase * grain * self.ratio;
                let i0 = ((pos as i32).rem_euclid(len as i32)) as usize;
                let i1 = (i0 + 1) % len;
                let frac = pos.fract().abs();
                let window = 0.5 - 0.5 * (std::f32::consts::TAU * phase).cos();
                acc_l += (self.buf_l[i0] * (1.0 - frac) + self.buf_l[i1] * frac) * window;
                acc_r += (self.buf_r[i0] * (1.0 - frac) + self.buf_r[i1] * frac) * window;
                wsum += window;
            }
            if wsum > 1e-6 {
                *l = acc_l / wsum;
                *r = acc_r / wsum;
            }
            self.write = (self.write + 1) % len;
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
    Ok(())
}
