//! Keeps Bluetooth headphones on A2DP while the mixer runs.
//!
//! WirePlumber moves a headset to the hands-free profile (mono, 16 kHz, mSBC) as
//! soon as any capture stream links to its microphone. That drops the A2DP sink
//! the listen output plays on, and some headsets then lose A2DP altogether until
//! they reconnect. The mixer's mic is the Frame mic, so the switch is never wanted.
//!
//! A worker thread owns every `wpctl` and `pw-dump` call, so the
//! PipeWire loop never waits on a child process.

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const AUTOSWITCH: &str = "bluetooth.autoswitch-to-headset-profile";
const RETRY: Duration = Duration::from_secs(20);
const MAX_TRIES: u32 = 4;

/// A Bluetooth output as the registry announces it. The registry leaves out
/// `api.bluez5.profile`, so the worker reads the profile from the device.
#[derive(Clone, Debug, PartialEq)]
pub struct BtSink {
    pub device: u32,
}

struct Shared {
    /// False while the mixer's own mic is a Bluetooth headset: then hands-free is the point.
    guard: AtomicBool,
    sinks: Mutex<Vec<BtSink>>,
    /// Set when this process turned the autoswitch off, so exit can turn it back on.
    switched_off: AtomicBool,
    stop: AtomicBool,
}

pub struct Guard {
    shared: Arc<Shared>,
}

impl Guard {
    pub fn start() -> Self {
        let shared = Arc::new(Shared {
            guard: AtomicBool::new(true),
            sinks: Mutex::new(Vec::new()),
            switched_off: AtomicBool::new(false),
            stop: AtomicBool::new(false),
        });
        let worker = shared.clone();
        std::thread::spawn(move || run(&worker));
        Self { shared }
    }

    pub fn set_guard(&self, on: bool) {
        self.shared.guard.store(on, Ordering::Relaxed);
    }

    pub fn set_sinks(&self, sinks: Vec<BtSink>) {
        if let Ok(mut current) = self.shared.sinks.lock() {
            *current = sinks;
        }
    }

    /// Hands the autoswitch back to WirePlumber. Called once the PipeWire loop has stopped.
    pub fn finish(&self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if self.shared.switched_off.swap(false, Ordering::Relaxed) {
            set_autoswitch(true);
        }
    }
}

fn run(shared: &Shared) {
    let mut tries: Vec<(u32, u32, Instant)> = Vec::new();
    while !shared.stop.load(Ordering::Relaxed) {
        let guard = shared.guard.load(Ordering::Relaxed);
        // WirePlumber forgets a runtime setting when it restarts, so this is checked again.
        match autoswitch() {
            Some(true) if guard => {
                if set_autoswitch(false) {
                    shared.switched_off.store(true, Ordering::Relaxed);
                }
            }
            Some(false) if !guard && shared.switched_off.load(Ordering::Relaxed) => {
                if set_autoswitch(true) {
                    shared.switched_off.store(false, Ordering::Relaxed);
                }
            }
            _ => {}
        }
        let mut sinks = shared.sinks.lock().map(|sinks| sinks.clone()).unwrap_or_default();
        sinks.sort_by_key(|sink| sink.device);
        sinks.dedup_by_key(|sink| sink.device);
        let hands_free: Vec<u32> = if guard {
            sinks.iter().map(|sink| sink.device).filter(|&device| profile(device).starts_with("headset")).collect()
        } else {
            Vec::new()
        };
        tries.retain(|(device, _, _)| hands_free.contains(device));
        if guard {
            for &device in &hands_free {
                let entry = tries.iter().position(|(tried, _, _)| *tried == device);
                let due = entry.is_none_or(|index| tries[index].1 < MAX_TRIES && tries[index].2.elapsed() >= RETRY);
                if !due {
                    continue;
                }
                match entry {
                    Some(index) => {
                        tries[index].1 += 1;
                        tries[index].2 = Instant::now();
                    }
                    None => tries.push((device, 1, Instant::now())),
                }
                restore_a2dp(device);
            }
        }
        for _ in 0..10 {
            if shared.stop.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }
}

fn autoswitch() -> Option<bool> {
    let out = Command::new("wpctl").args(["settings", AUTOSWITCH]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().find(|line| line.trim_start().starts_with("Value:"))?;
    Some(line.contains("true"))
}

fn set_autoswitch(on: bool) -> bool {
    Command::new("wpctl")
        .args(["settings", AUTOSWITCH, if on { "true" } else { "false" }])
        .status()
        .is_ok_and(|status| status.success())
}

/// Current profile name of one device, from a dump of that object only.
fn profile(device: u32) -> String {
    let Ok(out) = Command::new("pw-dump").arg(device.to_string()).output() else { return String::new() };
    let objects: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap_or_default();
    objects
        .first()
        .and_then(|card| card.pointer("/info/params/Profile/0/name"))
        .and_then(|name| name.as_str())
        .unwrap_or("")
        .to_string()
}

struct DeviceState {
    hands_free: bool,
    /// Highest-priority A2DP profile that is available right now.
    a2dp: Option<i64>,
    /// An app records from this headset's microphone.
    mic_in_use: bool,
}

/// Back to A2DP, unless an app is using the headset mic. BlueZ is never asked to
/// reconnect A2DP itself: bluetoothd 5.79 crashes when the headset does not answer.
fn restore_a2dp(device: u32) {
    let Some(state) = device_state(device) else { return };
    if !state.hands_free || state.mic_in_use {
        return;
    }
    match state.a2dp {
        Some(index) => set_profile(device, index),
        None => eprintln!("frame-audio-mixer: Bluetooth: device {device} offers no A2DP; it stays hands-free until it reconnects"),
    }
}

fn set_profile(device: u32, index: i64) {
    eprintln!("frame-audio-mixer: Bluetooth: device {device} back to A2DP (profile {index})");
    let _ = Command::new("wpctl").args(["set-profile", &device.to_string(), &index.to_string()]).status();
}

fn device_state(device: u32) -> Option<DeviceState> {
    let out = Command::new("pw-dump").output().ok()?;
    let objects: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).ok()?;
    let props = |object: &serde_json::Value| object.pointer("/info/props").cloned().unwrap_or_default();
    let card = objects.iter().find(|object| object["id"].as_u64() == Some(device as u64))?;
    let params = card.pointer("/info/params")?;
    let current = params["Profile"].get(0).and_then(|profile| profile["name"].as_str()).unwrap_or("");
    let a2dp = params["EnumProfile"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|profile| profile["name"].as_str().is_some_and(|name| name.starts_with("a2dp")))
        .filter(|profile| profile["available"].as_str() != Some("no"))
        .max_by_key(|profile| profile["priority"].as_i64().unwrap_or(0))
        .and_then(|profile| profile["index"].as_i64());
    let mics: Vec<u64> = objects
        .iter()
        .filter(|object| {
            let props = props(object);
            props["media.class"].as_str() == Some("Audio/Source")
                && props["device.id"].as_u64() == Some(device as u64)
                && props["bluez5.loopback"].as_bool() == Some(true)
        })
        .filter_map(|object| object["id"].as_u64())
        .collect();
    let mic_in_use = objects.iter().any(|object| {
        object["type"].as_str().is_some_and(|kind| kind.ends_with("Link"))
            && object.pointer("/info/output-node-id").and_then(|id| id.as_u64()).is_some_and(|id| mics.contains(&id))
    });
    Some(DeviceState {
        hands_free: current.starts_with("headset"),
        a2dp,
        mic_in_use,
    })
}
