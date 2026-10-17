//! Peak meters for the mix faders.
//!
//! One `pw-record` child per target, and only while the panel is visible, Mix
//! is on and meters are enabled. Every tap is `node.virtual`, so it does not
//! turn Valve's mic filters on. Channel taps are passive. The mic tap is not,
//! so speaking shows a level even when nothing else is capturing.

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
}

struct Tap {
    child: Child,
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
            if self.taps.contains_key(&key) {
                continue;
            }
            if let Some(tap) = start_tap(&target) {
                self.taps.insert(key, tap);
            }
        }
    }

    /// Decayed linear peaks, 0 to 1, keyed like the targets.
    pub fn peaks(&mut self) -> HashMap<String, f32> {
        let dt = self.last.elapsed().as_secs_f32().min(1.0);
        self.last = Instant::now();
        let decay = (-dt / 0.12).exp();
        let mut out = HashMap::new();
        for (key, tap) in &self.taps {
            let raw = f32::from_bits(tap.peak.load(Ordering::Relaxed));
            let age_ms = tap.updated.load(Ordering::Relaxed);
            let sample = if age_ms == 0 { 0.0 } else { raw };
            let prev = self.shown.get(key).copied().unwrap_or(0.0);
            let next = prev.max(sample) * if sample >= prev { 1.0 } else { decay };
            // A silent block still decays. A louder block jumps up at once.
            let shown = if sample > prev { sample } else { next };
            out.insert(key.clone(), shown.clamp(0.0, 1.0));
        }
        self.shown.clone_from(&out);
        out
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
    let props = format!(
        "{{ node.virtual=true node.passive={} stream.capture.sink={} }}",
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
        "40ms".into(),
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
    Some(Tap { child, peak, updated })
}

fn read_peak(stdout: &mut impl Read, peak: &AtomicU32, updated: &AtomicU32) {
    let mut buf = [0u8; 1600 * 4];
    let started = Instant::now();
    loop {
        match stdout.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let samples = n / 4;
                let mut max = 0.0f32;
                for i in 0..samples {
                    let value = f32::from_le_bytes(buf[i * 4..i * 4 + 4].try_into().unwrap_or([0; 4])).abs();
                    if value.is_finite() {
                        max = max.max(value);
                    }
                }
                peak.store(max.min(1.0).to_bits(), Ordering::Relaxed);
                updated.store(started.elapsed().as_millis().min(u128::from(u32::MAX)) as u32, Ordering::Relaxed);
            }
        }
    }
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
    let target = Target { key: "mic".into(), object: "alsa_input.mic".into(), capture_sink: false, passive: false };
    let args = record_args(&target);
    let joined = args.join(" ");
    if !joined.contains("node.virtual=true") || !joined.contains("node.passive=false") || !joined.contains("stream.capture.sink=false") {
        return Err(joined);
    }
    let channel = Target { key: "ch1".into(), object: "frame_amix_ch1".into(), capture_sink: true, passive: true };
    let args = record_args(&channel);
    if !args.join(" ").contains("node.passive=true") || !args.join(" ").contains("stream.capture.sink=true") {
        return Err("channel tap".into());
    }
    let mut meter = Meter::new();
    meter.sync(&[]);
    if !meter.peaks().is_empty() {
        return Err("idle meter".into());
    }
    Ok(())
}
