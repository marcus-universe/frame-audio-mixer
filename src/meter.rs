//! Peak meters for the mix faders.
//!
//! One `pw-record` child per target, and only while the panel is visible, Mix
//! is on and meters are enabled. Channel taps are `node.virtual` and passive.
//! The microphone tap is not virtual, so Frame Mic Tuner's tracker keeps the
//! built-in chain running and the system-default mic can show a level.

use std::collections::HashMap;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub key: String,
    pub object: String,
    /// Listen to a sink's monitor (`stream.capture.sink`).
    pub capture_sink: bool,
    /// False only for the microphone, so a level appears while talking.
    pub passive: bool,
    /// Channel taps stay virtual. The microphone tap does not, so the Frame's
    /// filter tracker keeps the built-in chain running while the meter is up.
    pub virtual_node: bool,
}

struct Tap {
    child: Child,
    object: String,
    peak: Arc<AtomicU32>,
    updated: Arc<AtomicU32>,
}

pub struct Meter {
    taps: HashMap<String, Tap>,
    shown: HashMap<String, f32>,
    last: Instant,
}

impl Meter {
    pub fn new() -> Self {
        Self { taps: HashMap::new(), shown: HashMap::new(), last: Instant::now() }
    }

    /// Start or stop taps so they match `targets`. An empty list stops them all.
    pub fn sync(&mut self, targets: &[Target]) {
        let wanted: HashMap<_, _> = targets.iter().map(|target| (target.key.clone(), target.clone())).collect();
        let stale: Vec<_> = self.taps.keys().filter(|key| !wanted.contains_key(*key)).cloned().collect();
        for key in stale {
            if let Some(mut tap) = self.taps.remove(&key) {
                stop_child(&mut tap.child);
            }
            self.shown.remove(&key);
        }
        for (key, target) in wanted {
            if self.taps.get(&key).is_some_and(|tap| tap.object == target.object) {
                continue;
            }
            if let Some(mut old) = self.taps.remove(&key) {
                stop_child(&mut old.child);
            }
            if let Some(tap) = start_tap(&target) {
                self.taps.insert(key, tap);
            }
        }
    }

    /// Decayed linear peaks, 0 to 1, keyed like the targets.
    /// The map is reused so a frame does not allocate a new one.
    pub fn peaks(&mut self) -> &HashMap<String, f32> {
        let dt = self.last.elapsed().as_secs_f32().min(1.0);
        self.last = Instant::now();
        let decay = (-dt / 0.12).exp();
        self.shown.retain(|key, _| self.taps.contains_key(key));
        for (key, tap) in &self.taps {
            let raw = f32::from_bits(tap.peak.load(Ordering::Relaxed));
            let age_ms = tap.updated.load(Ordering::Relaxed);
            let sample = if age_ms == 0 { 0.0 } else { raw };
            let prev = self.shown.get(key).copied().unwrap_or(0.0);
            let next = prev.max(sample) * if sample >= prev { 1.0 } else { decay };
            // A silent block still decays. A louder block jumps up at once.
            let shown = (if sample > prev { sample } else { next }).clamp(0.0, 1.0);
            if let Some(slot) = self.shown.get_mut(key) {
                *slot = shown;
            } else {
                self.shown.insert(key.clone(), shown);
            }
        }
        &self.shown
    }
}

impl Drop for Meter {
    fn drop(&mut self) {
        for (_, mut tap) in self.taps.drain() {
            stop_child(&mut tap.child);
        }
    }
}

impl Default for Meter {
    fn default() -> Self {
        Self::new()
    }
}

pub fn record_args(target: &Target) -> Vec<String> {
    let virtual_prop = if target.virtual_node { "node.virtual=true " } else { "" };
    let props = format!(
        "{{ {virtual_prop}node.passive={} stream.capture.sink={} }}",
        if target.passive { "true" } else { "false" },
        if target.capture_sink { "true" } else { "false" }
    );
    vec![
        "--raw".into(),
        "--format".into(),
        "f32".into(),
        "--channels".into(),
        "1".into(),
        "--rate".into(),
        "16000".into(),
        "--latency".into(),
        "20ms".into(),
        "--target".into(),
        target.object.clone(),
        "-P".into(),
        props,
        "-".into(),
    ]
}

fn start_tap(target: &Target) -> Option<Tap> {
    let mut child = Command::new("pw-record")
        .args(record_args(target))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let peak = Arc::new(AtomicU32::new(0.0f32.to_bits()));
    let updated = Arc::new(AtomicU32::new(0));
    let peak_thread = Arc::clone(&peak);
    let updated_thread = Arc::clone(&updated);
    std::thread::spawn(move || read_peak(&mut stdout, &peak_thread, &updated_thread));
    Some(Tap { child, object: target.object.clone(), peak, updated })
}

fn read_peak(stdout: &mut impl Read, peak: &AtomicU32, updated: &AtomicU32) {
    // 20 ms at 16 kHz, one float per sample. The old 100 ms block is what made the bar lag.
    let mut buf = [0u8; 320 * 4];
    let started = Instant::now();
    loop {
        match stdout.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                peak.store(peak_of(&buf[..n]).to_bits(), Ordering::Relaxed);
                updated.store(started.elapsed().as_millis().min(u128::from(u32::MAX)) as u32, Ordering::Relaxed);
            }
        }
    }
}

/// Loudest finite sample in a little-endian f32 buffer. Length is checked once per chunk of four.
pub fn peak_of(bytes: &[u8]) -> f32 {
    let usable = bytes.len() & !3;
    let mut max = 0.0f32;
    let mut i = 0;
    while i + 16 <= usable {
        let a = f32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]).abs();
        let b = f32::from_le_bytes([bytes[i + 4], bytes[i + 5], bytes[i + 6], bytes[i + 7]]).abs();
        let c = f32::from_le_bytes([bytes[i + 8], bytes[i + 9], bytes[i + 10], bytes[i + 11]]).abs();
        let d = f32::from_le_bytes([bytes[i + 12], bytes[i + 13], bytes[i + 14], bytes[i + 15]]).abs();
        let step = a.max(b).max(c.max(d));
        if step.is_finite() {
            max = max.max(step);
        }
        i += 16;
    }
    while i + 4 <= usable {
        let value = f32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]).abs();
        if value.is_finite() {
            max = max.max(value);
        }
        i += 4;
    }
    max.min(1.0)
}

fn stop_child(child: &mut Child) {
    let pid = child.id() as i32;
    unsafe { libc::kill(pid, libc::SIGTERM) };
    let start = Instant::now();
    loop {
        if child.try_wait().ok().flatten().is_some() || start.elapsed().as_millis() > 400 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

pub fn self_test() -> Result<(), String> {
    let target = Target {
        key: "mic".into(),
        object: "77".into(),
        capture_sink: false,
        passive: false,
        virtual_node: false,
    };
    let args = record_args(&target);
    let joined = args.join(" ");
    if joined.contains("node.virtual=true") || !joined.contains("node.passive=false") || !joined.contains("stream.capture.sink=false") || !joined.contains("20ms") {
        return Err(joined);
    }
    let channel = Target {
        key: "ch1".into(),
        object: "frame_amix_ch1".into(),
        capture_sink: true,
        passive: true,
        virtual_node: true,
    };
    let channel_args = record_args(&channel).join(" ");
    if !channel_args.contains("node.virtual=true") || !channel_args.contains("node.passive=true") || !channel_args.contains("stream.capture.sink=true") {
        return Err("channel tap".into());
    }
    let mut samples = [0u8; 16];
    samples[0..4].copy_from_slice(&0.25f32.to_le_bytes());
    samples[4..8].copy_from_slice(&(-0.5f32).to_le_bytes());
    samples[8..12].copy_from_slice(&0.1f32.to_le_bytes());
    samples[12..16].copy_from_slice(&f32::NAN.to_le_bytes());
    let peak = peak_of(&samples);
    if (peak - 0.5).abs() > 0.001 {
        return Err(format!("peak {peak}"));
    }
    let mut meter = Meter::new();
    meter.sync(&[]);
    if !meter.peaks().is_empty() {
        return Err("idle meter".into());
    }
    Ok(())
}
