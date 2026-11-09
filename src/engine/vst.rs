//! Plugin hosts the engine starts for the VST effects in the FX chains. Each
//! host is a child process; the engine owns its audio region and its stdin.

use std::collections::HashMap;
use std::io::Write;
use std::process::{Child, ChildStdin, Stdio};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::vst::shared::{Region, STATUS_FAILED, STATUS_RUNNING};
use crate::vst::{host_command, region_path, VstRef};

struct Host {
    child: Child,
    stdin: Option<ChildStdin>,
    region: Region,
    exited: bool,
}

#[derive(Default)]
pub struct Hosts {
    running: HashMap<String, Host>,
    /// Why a plugin could not start, by instance.
    errors: HashMap<String, String>,
}

impl Hosts {
    /// Starts hosts for new plugins and stops hosts whose plugin left the chains.
    pub fn sync(&mut self, wanted: &[VstRef]) {
        let keep: Vec<&str> = wanted.iter().map(|plugin| plugin.instance.as_str()).collect();
        let gone: Vec<String> = self.running.keys().filter(|instance| !keep.contains(&instance.as_str())).cloned().collect();
        for instance in gone {
            if let Some(host) = self.running.remove(&instance) {
                // Saving and quitting can take seconds; the PipeWire loop must not wait.
                std::thread::spawn(move || stop(host, &instance));
            }
        }
        self.errors.retain(|instance, _| keep.contains(&instance.as_str()));
        for plugin in wanted {
            if self.running.contains_key(&plugin.instance) || self.errors.contains_key(&plugin.instance) {
                continue;
            }
            match start(plugin) {
                Ok(host) => {
                    self.running.insert(plugin.instance.clone(), host);
                }
                Err(err) => {
                    eprintln!("frame-audio-mixer: VST {}: {err}", plugin.name);
                    self.errors.insert(plugin.instance.clone(), err);
                }
            }
        }
    }

    /// Notes hosts that exited. Cheap enough for the engine's 200 ms timer.
    pub fn reap(&mut self) {
        for (instance, host) in &mut self.running {
            if !host.exited && host.child.try_wait().ok().flatten().is_some() {
                host.exited = true;
                eprintln!("frame-audio-mixer: VST host {instance} exited");
            }
        }
    }

    /// Opens the plugin's editor window.
    pub fn open_editor(&mut self, instance: &str) -> bool {
        let Some(host) = self.running.get_mut(instance) else { return false };
        let Some(stdin) = host.stdin.as_mut() else { return false };
        stdin.write_all(b"gui\n").and_then(|()| stdin.flush()).is_ok()
    }

    /// `starting`, `running`, `failed` or `stopped`, by instance.
    pub fn statuses(&self) -> HashMap<String, String> {
        let mut out: HashMap<String, String> = self.errors.keys().map(|instance| (instance.clone(), "failed".to_string())).collect();
        for (instance, host) in &self.running {
            let status = match (host.exited, host.region.header().status.load(Ordering::Acquire)) {
                (true, _) => "stopped",
                (false, STATUS_RUNNING) => "running",
                (false, STATUS_FAILED) => "failed",
                _ => "starting",
            };
            out.insert(instance.clone(), status.into());
        }
        out
    }

    pub fn stop_all(&mut self) {
        for (instance, host) in self.running.drain() {
            stop(host, &instance);
        }
    }
}

fn start(plugin: &VstRef) -> Result<Host, String> {
    let region_file = region_path(&plugin.instance);
    let region = Region::create(&region_file).map_err(|err| format!("region: {err}"))?;
    if let Some(dir) = crate::vst::state_path(&plugin.instance).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut command = host_command(plugin)?;
    let mut child = command.stdin(Stdio::piped()).stdout(Stdio::null()).spawn().map_err(|err| format!("start host: {err}"))?;
    let stdin = child.stdin.take();
    Ok(Host { child, stdin, region, exited: false })
}

/// Asks the host to save and quit, then makes sure it is gone.
fn stop(mut host: Host, instance: &str) {
    if let Some(mut stdin) = host.stdin.take() {
        let _ = stdin.write_all(b"quit\n");
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if host.child.try_wait().ok().flatten().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if host.child.try_wait().ok().flatten().is_none() {
        let _ = host.child.kill();
        let _ = host.child.wait();
    }
    let _ = std::fs::remove_file(region_path(instance));
}

/// End-to-end check without PipeWire: starts a real host for `bundle`, plays
/// four seconds of a sine through the engine's own VST stage at the speaker
/// period, and reports what came back. `frame-audio-mixer --vst-bridge-test <bundle> <native|windows>`.
pub fn bridge_test(bundle: &str, arch: &str) -> Result<(), String> {
    use crate::config::{EffectKind, EffectSlot};
    use crate::vst::{VstArch, VstInfo};
    let arch = match arch {
        "native" => VstArch::Native,
        "windows" => VstArch::Windows,
        other => return Err(format!("unknown arch {other}")),
    };
    let info = VstInfo { name: "Bridge test".into(), vendor: String::new(), path: bundle.into(), cid: String::new(), arch };
    let plugin = VstRef::place(&info);
    let mut hosts = Hosts::default();
    let started = Instant::now();
    hosts.sync(std::slice::from_ref(&plugin));
    loop {
        hosts.reap();
        let status = hosts.statuses().get(&plugin.instance).cloned().unwrap_or_default();
        match status.as_str() {
            "running" => break,
            "starting" if started.elapsed() < Duration::from_secs(180) => std::thread::sleep(Duration::from_millis(100)),
            other => {
                hosts.stop_all();
                return Err(format!("host {other} after {:.1} s", started.elapsed().as_secs_f64()));
            }
        }
    }
    println!("host running after {:.1} s", started.elapsed().as_secs_f64());
    let mut slot = EffectSlot::new(EffectKind::Vst);
    slot.vst = Some(plugin.clone());
    let mut proc = crate::fx::dsp::Processor::new();
    proc.set_chain(std::slice::from_ref(&slot));
    const BLOCK: usize = 480;
    let blocks = 400;
    let (mut energy_in, mut energy_out, mut changed) = (0.0f64, 0.0f64, 0usize);
    let period = Duration::from_micros(10_000);
    let mut next = Instant::now();
    for block in 0..blocks {
        let mut left = [0.0f32; BLOCK];
        for (i, sample) in left.iter_mut().enumerate() {
            let t = (block * BLOCK + i) as f64 / 48_000.0;
            *sample = (std::f64::consts::TAU * 440.0 * t).sin() as f32 * 0.25;
        }
        let dry = left;
        let mut right = left;
        proc.process_planar(&mut left, &mut right);
        if block >= 100 {
            energy_in += dry.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>();
            energy_out += left.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>();
            // A processed block is one period late, so it differs from this block's dry input.
            if left.iter().zip(&dry).any(|(a, b)| (a - b).abs() > 1e-4) {
                changed += 1;
            }
        }
        next += period;
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
    }
    let late = crate::vst::shared::Region::open(&region_path(&plugin.instance))
        .map(|region| region.header().late.load(Ordering::Relaxed))
        .unwrap_or(u32::MAX);
    hosts.stop_all();
    let _ = std::fs::remove_file(crate::vst::state_path(&plugin.instance));
    println!(
        "blocks 300: {changed} came back through the host, {late} late, level {:+.2} dB",
        10.0 * (energy_out / energy_in.max(1e-30)).log10()
    );
    if changed < 290 {
        return Err("audio did not come back through the host".into());
    }
    Ok(())
}
