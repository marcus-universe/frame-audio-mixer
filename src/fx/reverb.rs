//! Feedback delay network reverb: pre-delay, early reflections, input diffusion,
//! then eight damped, slowly modulated lines mixed by a Hadamard matrix. The
//! lines are allocated once at their longest, so moving a slider never allocates.

const RATE: f32 = 48_000.0;
const LINES: usize = 8;
const LINE_LEN: usize = 1 << 13;
const LINE_MASK: usize = LINE_LEN - 1;
const PRE_LEN: usize = 1 << 13;
const PRE_MASK: usize = PRE_LEN - 1;
/// Mutually prime lengths at size 1, so the echoes never line up.
const BASE: [f32; LINES] = [1423.0, 1693.0, 1949.0, 2243.0, 2647.0, 3001.0, 3469.0, 3911.0];
const OUT_LEFT: [f32; LINES] = [1.0, -1.0, 1.0, 1.0, -1.0, 1.0, -1.0, -1.0];
const OUT_RIGHT: [f32; LINES] = [1.0, 1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0];
const MIN_SIZE: f32 = 0.12;
const MAX_SIZE: f32 = 2.0;
const MOD_DEPTH: f32 = 3.0;
const MOD_RATE: f32 = 0.7;
const MOD_EVERY: u32 = 32;
/// Reflection times in samples at size 1, with gain. The right side uses other times.
const EARLY_LEFT: [(f32, f32); 6] = [(311.0, 0.84), (547.0, 0.7), (823.0, -0.56), (1103.0, 0.48), (1447.0, -0.37), (1789.0, 0.28)];
const EARLY_RIGHT: [(f32, f32); 6] = [(373.0, 0.8), (613.0, -0.66), (887.0, 0.58), (1213.0, -0.44), (1511.0, 0.35), (1867.0, 0.25)];
const DIFFUSER_LEFT: [usize; 4] = [142, 107, 379, 277];
const DIFFUSER_RIGHT: [usize; 4] = [151, 113, 397, 263];
const DIFFUSION: f32 = 0.62;

struct Diffuser {
    buf: Vec<f32>,
    pos: usize,
}

impl Diffuser {
    fn new(len: usize) -> Self {
        Self { buf: vec![0.0; len], pos: 0 }
    }

    fn tick(&mut self, x: f32) -> f32 {
        let delayed = self.buf[self.pos];
        let input = x + delayed * DIFFUSION;
        self.buf[self.pos] = input;
        self.pos = (self.pos + 1) % self.buf.len();
        delayed - input * DIFFUSION
    }
}

pub struct Reverb {
    lines: Vec<f32>,
    write: usize,
    damp_state: [f32; LINES],
    damping: [f32; LINES],
    feedback: [f32; LINES],
    size: f32,
    size_now: f32,
    offsets: [f32; LINES],
    lfo: f32,
    mod_count: u32,
    pre_left: Vec<f32>,
    pre_right: Vec<f32>,
    pre_pos: usize,
    pre_delay: usize,
    diff_left: [Diffuser; 4],
    diff_right: [Diffuser; 4],
    early: f32,
    wet_gain: f32,
    mix: f32,
    dc_left: f32,
    dc_right: f32,
}

/// Three butterfly stages of an orthonormal 8-point Hadamard transform.
fn hadamard(v: &mut [f32; LINES]) {
    let mut span = 1;
    while span < LINES {
        let mut start = 0;
        while start < LINES {
            for i in start..start + span {
                let (a, b) = (v[i], v[i + span]);
                v[i] = a + b;
                v[i + span] = a - b;
            }
            start += span * 2;
        }
        span *= 2;
    }
    v.iter_mut().for_each(|x| *x *= 0.353_553_4);
}

impl Reverb {
    pub fn new() -> Self {
        let mut reverb = Self {
            lines: vec![0.0; LINES * LINE_LEN],
            write: 0,
            damp_state: [0.0; LINES],
            damping: [0.0; LINES],
            feedback: [0.0; LINES],
            size: 0.5,
            size_now: 0.5,
            offsets: [0.0; LINES],
            lfo: 0.0,
            mod_count: 0,
            pre_left: vec![0.0; PRE_LEN],
            pre_right: vec![0.0; PRE_LEN],
            pre_pos: 0,
            pre_delay: 0,
            diff_left: DIFFUSER_LEFT.map(Diffuser::new),
            diff_right: DIFFUSER_RIGHT.map(Diffuser::new),
            early: 0.5,
            wet_gain: 0.5,
            mix: 0.25,
            dc_left: 0.0,
            dc_right: 0.0,
        };
        reverb.set([0.5, 0.5, 0.25], [0.5; 3]);
        reverb
    }

    /// `params` are size, decay and mix. `extra` holds damping, pre-delay and early level.
    pub fn set(&mut self, params: [f64; 3], extra: [f64; 3]) {
        let size = MIN_SIZE * (MAX_SIZE / MIN_SIZE).powf(params[0] as f32);
        let seconds = 0.2 * 40f32.powf(params[1] as f32);
        let damping = 0.9 * extra[0] as f32;
        self.size = size;
        self.mix = params[2] as f32;
        self.early = extra[2] as f32;
        let pre = extra[1] as f32;
        self.pre_delay = ((pre * pre * 0.08 * RATE) as usize).min(PRE_LEN - 1);
        self.wet_gain = 0.9 / (1.0 + 0.6 * seconds).sqrt();
        for (i, base) in BASE.iter().enumerate() {
            let len = base * size;
            self.feedback[i] = 10f32.powf(-3.0 * len / (seconds * RATE));
            // A short line loops more often, so each pass darkens less.
            self.damping[i] = damping.powf(2000.0 / len.max(64.0));
        }
    }

    fn update_modulation(&mut self) {
        self.lfo = (self.lfo + MOD_RATE * MOD_EVERY as f32 / RATE).fract();
        let angle = self.lfo * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        let depth = MOD_DEPTH * self.size_now.min(1.0);
        for (i, offset) in self.offsets.iter_mut().enumerate() {
            let value = if i % 2 == 0 { sin } else { cos };
            *offset = value * depth * if i % 4 < 2 { 1.0 } else { -1.0 };
        }
    }

    fn read_line(&self, line: usize, delay: f32) -> f32 {
        let delay = delay.clamp(2.0, (LINE_LEN - 2) as f32);
        let whole = delay as usize;
        let frac = delay - whole as f32;
        let older = self.write.wrapping_sub(whole + 1) & LINE_MASK;
        let newer = self.write.wrapping_sub(whole) & LINE_MASK;
        let base = line * LINE_LEN;
        self.lines[base + newer] * (1.0 - frac) + self.lines[base + older] * frac
    }

    fn early_tap(buf: &[f32], pos: usize, taps: &[(f32, f32); 6], size: f32) -> f32 {
        taps.iter().map(|(at, gain)| buf[pos.wrapping_sub((at * size) as usize) & PRE_MASK] * gain).sum()
    }

    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let (dry_l, dry_r) = (*l, *r);
            self.size_now += (self.size - self.size_now) * 0.000_3;
            self.mod_count += 1;
            if self.mod_count >= MOD_EVERY {
                self.mod_count = 0;
                self.update_modulation();
            }

            self.pre_left[self.pre_pos] = dry_l;
            self.pre_right[self.pre_pos] = dry_r;
            let tap = self.pre_pos.wrapping_sub(self.pre_delay) & PRE_MASK;
            let mut in_l = self.pre_left[tap];
            let mut in_r = self.pre_right[tap];
            let early_l = Self::early_tap(&self.pre_left, tap, &EARLY_LEFT, self.size_now.min(1.2));
            let early_r = Self::early_tap(&self.pre_right, tap, &EARLY_RIGHT, self.size_now.min(1.2));
            self.pre_pos = (self.pre_pos + 1) & PRE_MASK;
            for diffuser in &mut self.diff_left {
                in_l = diffuser.tick(in_l);
            }
            for diffuser in &mut self.diff_right {
                in_r = diffuser.tick(in_r);
            }

            let mut state = [0.0f32; LINES];
            let (mut tail_l, mut tail_r) = (0.0, 0.0);
            for i in 0..LINES {
                let raw = self.read_line(i, BASE[i] * self.size_now + self.offsets[i]);
                let mut damped = raw * (1.0 - self.damping[i]) + self.damp_state[i] * self.damping[i];
                if damped.abs() < 1e-15 {
                    damped = 0.0;
                }
                self.damp_state[i] = damped;
                state[i] = damped * self.feedback[i];
                tail_l += damped * OUT_LEFT[i];
                tail_r += damped * OUT_RIGHT[i];
            }
            hadamard(&mut state);
            for (i, value) in state.iter().enumerate() {
                let input = if i % 2 == 0 { in_l } else { in_r };
                self.lines[i * LINE_LEN + self.write] = value + input * 0.5;
            }
            self.write = (self.write + 1) & LINE_MASK;

            let wet_l = tail_l * self.wet_gain * 0.5 + early_l * self.early * 0.35;
            let wet_r = tail_r * self.wet_gain * 0.5 + early_r * self.early * 0.35;
            self.dc_left += 0.01 * (wet_l - self.dc_left);
            self.dc_right += 0.01 * (wet_r - self.dc_right);
            *l = dry_l * (1.0 - self.mix) + (wet_l - self.dc_left) * self.mix;
            *r = dry_r * (1.0 - self.mix) + (wet_r - self.dc_right) * self.mix;
        }
    }
}
