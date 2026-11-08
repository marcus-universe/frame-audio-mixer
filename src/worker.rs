//! systemctl, wpctl and pw-metadata run here, off the UI thread.
//! Nothing in this module calls `wpctl set-default`. The Microphone fader
//! writes only `frame_amix_mic_virtual_sink`.

use crate::config::{Channel, Config, is_channel_id};
use crate::graph::{AudioNode, Graph};
use crate::apps::{self, Entry};
use crate::updater::{self, UpdateView};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckPhase {
    Idle,
    Recording,
    Ready,
    Playing,
    Paused,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub graph: Graph,
    pub mix_active: bool,
    pub echo_cancel: Option<bool>,
    pub autostart: bool,
    pub tuner_version: String,
    pub update: UpdateView,
    pub check: CheckPhase,
    /// Seconds recorded so far, or the play position.
    pub check_secs: f64,
    pub has_take: bool,
    pub take_secs: f64,
    pub message: String,
    /// Bumps when a Mix Check attempt finishes without staying in Recording.
    pub check_epoch: u64,
    pub apps: Vec<Entry>,
    pub menu_busy: bool,
    pub meters: HashMap<String, f32>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            graph: Graph::default(),
            mix_active: false,
            echo_cancel: None,
            autostart: false,
            tuner_version: String::new(),
            update: UpdateView::default(),
            check: CheckPhase::Idle,
            check_secs: 0.0,
            has_take: false,
            take_secs: 0.0,
            message: String::new(),
            check_epoch: 0,
            apps: Vec::new(),
            menu_busy: false,
            meters: HashMap::new(),
        }
    }
}

pub enum Job {
    Apply(Config),
    /// Fader volumes during a drag. Applied on this worker turn, nothing else changes.
    Levels(Config),
    Refresh,
    Autostart(bool),
    /// Records the output named by `Config::check_output`.
    Record(Config),
    StopCheck,
    PlayTake,
    PauseTake,
    CheckUpdates,
    InstallUpdate,
    OpenInstaller(String),
    Quit,
}

pub struct Worker {
    tx: mpsc::Sender<Job>,
    snap: Arc<Mutex<Snapshot>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Worker {
    pub fn start(config: Config) -> Self {
        let (tx, rx) = mpsc::channel();
        let snap = Arc::new(Mutex::new(Snapshot::default()));
        let snap_thread = Arc::clone(&snap);
        let thread = std::thread::spawn(move || run(rx, snap_thread, config));
        Self { tx, snap, thread: Some(thread) }
    }

    pub fn send(&self, job: Job) {
        let _ = self.tx.send(job);
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snap.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.tx.send(Job::Quit);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct State {
    config: Config,
    graph: Graph,
    mix_active: bool,
    echo_cancel: Option<bool>,
    autostart: bool,
    tuner_version: String,
    facts_at: Option<Instant>,
    pinned: HashMap<u64, u32>,
    /// Stream serial to (stream id, channel sink serial) for streams that named a channel sink.
    target_pins: HashMap<u64, (u32, u64)>,
    /// VRLink capture id to the Frame Mix serial written for it.
    vrlink_pins: HashMap<u32, u64>,
    check_phase: CheckPhase,
    check_started: Option<Instant>,
    check_secs: f64,
    message: String,
    apps: apps::Manager,
    last_check: Option<Instant>,
    has_take: bool,
    take_secs: f64,
    check_epoch: u64,
    booted: bool,
    meters: HashMap<String, f32>,
    engine_up: bool,
}

fn run(rx: mpsc::Receiver<Job>, snap: Arc<Mutex<Snapshot>>, config: Config) {
    let mut state = State {
        config,
        graph: Graph::default(),
        mix_active: false,
        echo_cancel: None,
        autostart: false,
        tuner_version: String::new(),
        facts_at: None,
        pinned: HashMap::new(),
        target_pins: HashMap::new(),
        vrlink_pins: HashMap::new(),
        check_phase: CheckPhase::Idle,
        check_started: None,
        check_secs: 0.0,
        message: String::new(),
        apps: apps::Manager::new(),
        last_check: None,
        has_take: false,
        take_secs: 0.0,
        check_epoch: 0,
        booted: false,
        meters: HashMap::new(),
        engine_up: false,
    };
    install_icons(&state.config.accent);
    refresh_facts(&mut state, true);
    let initial = state.config.clone();
    apply_config(&mut state, initial);
    if state.facts_at.is_none() {
        refresh_facts(&mut state, true);
    }
    publish(&snap, &state);
    loop {
        let job = rx.recv_timeout(Duration::from_millis(800));
        let mut urgent = false;
        let mut dragging = false;
        match job {
            Ok(Job::Quit) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(job) => {
                dragging = matches!(job, Job::Levels(_));
                urgent = apply_job(&mut state, &snap, job);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        while let Ok(extra) = rx.try_recv() {
            if matches!(extra, Job::Quit) {
                return;
            }
            dragging |= matches!(extra, Job::Levels(_));
            if apply_job(&mut state, &snap, extra) {
                urgent = true;
            }
        }
        if urgent {
            // Record and Play/Pause update before the next status poll.
            publish_check(&snap, &state);
        }
        if dragging {
            send_engine(&mut state, "levels");
            if let Ok(mut guard) = snap.lock() {
                guard.meters = state.meters.clone();
            }
            continue;
        }
        state.apps.tick(state.config.check_updates);
        let check_due = state.config.check_updates
            && state.last_check.is_none_or(|start| start.elapsed() >= Duration::from_secs(3600));
        if check_due {
            let _ = updater::check(false);
            state.last_check = Some(Instant::now());
        }
        refresh_facts(&mut state, false);
        sync_audio(&mut state);
        publish(&snap, &state);
    }
}

/// Record, stop, and play return true so the panel can show that phase before `pw-dump`.
fn apply_job(state: &mut State, snap: &Mutex<Snapshot>, job: Job) -> bool {
    match job {
        Job::Apply(config) => apply_config(state, config),
        Job::Levels(config) => take_levels(state, &config),
        Job::Autostart(on) => set_autostart(on),
        Job::Record(config) => {
            state.config.check_output = config.check_output;
            ensure_engine(state);
            send_engine(state, "record");
            return true;
        }
        Job::StopCheck => {
            send_engine(state, "stop");
            return true;
        }
        Job::PlayTake => {
            ensure_engine(state);
            send_engine(state, "play");
            return true;
        }
        Job::PauseTake => {
            send_engine(state, "pause");
            return true;
        }
        Job::CheckUpdates => {
            let view = updater::check(true);
            state.last_check = Some(Instant::now());
            if let Ok(mut guard) = snap.lock() {
                guard.update = view;
            }
        }
        Job::InstallUpdate => {
            if let Err(err) = updater::detach() {
                state.message = err;
            }
        }
        Job::OpenInstaller(key) => {
            if let Err(err) = state.apps.launch(&key) {
                state.message = format!("{err:?}");
            }
        }
        Job::Refresh | Job::Quit => {}
    }
    false
}

fn refresh_facts(state: &mut State, force: bool) {
    let due = force || state.facts_at.is_none_or(|start| start.elapsed() >= Duration::from_secs(2));
    if !due {
        return;
    }
    state.graph = Graph::default();
    if let Some(status) = crate::engine::call("status", None) {
        apply_status(state, status);
    }
    state.echo_cancel = echo_cancel();
    state.autostart = unit_enabled("frame-audio-mixer.service");
    if state.tuner_version.is_empty() {
        state.tuner_version = tuner_version();
    }
    state.facts_at = Some(Instant::now());
}

fn publish_check(snap: &Mutex<Snapshot>, state: &State) {
    let Ok(mut guard) = snap.lock() else { return };
    guard.check = state.check_phase;
    guard.check_secs = state.check_secs;
    guard.has_take = state.has_take;
    guard.take_secs = state.take_secs;
    guard.message = state.message.clone();
    guard.check_epoch = state.check_epoch;
}

fn publish(snap: &Mutex<Snapshot>, state: &State) {
    let mut guard = match snap.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    *guard = Snapshot {
        graph: state.graph.clone(),
        mix_active: state.mix_active,
        echo_cancel: state.echo_cancel,
        autostart: state.autostart,
        tuner_version: state.tuner_version.clone(),
        update: updater::current_view(),
        check: state.check_phase,
        check_secs: state.check_secs,
        has_take: state.has_take,
        take_secs: state.take_secs,
        message: state.message.clone(),
        check_epoch: state.check_epoch,
        apps: state.apps.entries().to_vec(),
        menu_busy: state.apps.menu_busy(),
        meters: state.meters.clone(),
    };
}

fn apply_config(state: &mut State, config: Config) {
    let was = state.config.mix_on;
    let old_accent = state.config.accent.clone();
    let first = !state.booted;
    state.booted = true;
    state.config = config;
    if state.config.accent != old_accent {
        install_icons(&state.config.accent);
    }
    write_names(&state.config.channels);
    if state.config.mix_on && (first || !was) {
        save_setting("frame-audio-mixer.enabled", "true");
    } else if !state.config.mix_on && (first || was) {
        save_setting("frame-audio-mixer.enabled", "false");
        clear_pins(state);
        unpin_vrlink(state);
    }
    ensure_engine(state);
    send_engine(state, "apply");
    state.facts_at = None;
}

fn take_levels(state: &mut State, config: &Config) {
    state.config = config.clone();
    send_engine(state, "levels");
}

fn sync_audio(state: &mut State) {
    let graph = state.graph.clone();
    pin_targets(state, &graph);
    if !state.config.mix_on {
        if !state.pinned.is_empty() {
            clear_pins(state);
        }
        unpin_vrlink(state);
        return;
    }
    pin_apps(state, &graph);
    pin_vrlink(state, &graph);
}

/// The engine runs while this app runs. It is started, never enabled, so quitting
/// the app (see `stop_engine`) leaves no virtual device behind.
fn ensure_engine(state: &mut State) {
    let want = state.config.mix_on || !state.config.channels.is_empty();
    let unit = ENGINE_UNIT;
    let running = unit_active(unit);
    if want && !running {
        let _ = Command::new("systemctl").args(["--user", "start", unit]).status();
    } else if !want && running {
        let _ = Command::new("systemctl").args(["--user", "stop", unit]).status();
    }
    state.engine_up = want;
}

const ENGINE_UNIT: &str = "frame-audio-mixer-engine.service";

/// Removes every mixer device. Apps on a channel and VRLink fall back to the defaults.
pub fn stop_engine() {
    let _ = Command::new("systemctl").args(["--user", "stop", ENGINE_UNIT]).status();
}

fn send_engine(state: &mut State, op: &str) {
    if let Some(status) = crate::engine::call(op, Some(&state.config)) {
        apply_status(state, status);
    }
}

fn apply_status(state: &mut State, status: crate::engine::Status) {
    let was = state.check_phase;
    state.graph = status.graph;
    state.mix_active = status.mix_active;
    state.meters = status.meters;
    state.check_secs = status.check_secs;
    state.has_take = status.has_take;
    state.take_secs = status.take_secs;
    state.check_phase = match status.check.as_str() {
        "recording" => CheckPhase::Recording,
        "ready" => CheckPhase::Ready,
        "playing" => CheckPhase::Playing,
        "paused" => CheckPhase::Paused,
        _ => CheckPhase::Idle,
    };
    if was == CheckPhase::Recording && state.check_phase != CheckPhase::Recording {
        state.check_epoch = state.check_epoch.wrapping_add(1);
    }
    if matches!(state.check_phase, CheckPhase::Recording | CheckPhase::Playing) {
        if state.check_started.is_none() {
            state.check_started = Some(Instant::now());
        }
    } else {
        state.check_started = None;
    }
}

fn solo_mute(saved: bool, this_solo: bool, any_solo: bool) -> bool {
    if any_solo { saved || !this_solo } else { saved }
}

/// Dry copies of the mix bus and the four effect sinks. `node.mute` on the
/// loopback command does not stick, and with no target these streams follow
/// the speakers. That is the mic the user hears after arming an effect, and
/// the loop that howls.
/// VRLink follows the default source unless its capture is pinned. The WirePlumber
/// script does this too, but only after WirePlumber has loaded it. A write is skipped
/// when the metadata already holds the serial, because a rewrite relinks the stream.
fn pin_vrlink(state: &mut State, graph: &Graph) {
    let Some(source) = graph.node_by_name("frame_amix_source").filter(|node| node.serial != 0) else {
        unpin_vrlink(state);
        return;
    };
    let serial = source.serial;
    let live: Vec<u32> = graph
        .nodes
        .iter()
        .filter(|node| node.media_class == "Stream/Input/Audio" && Graph::is_vrlink(node))
        .map(|node| node.id)
        .collect();
    state.vrlink_pins.retain(|id, _| live.contains(id));
    for id in live {
        if state.vrlink_pins.get(&id) == Some(&serial) {
            continue;
        }
        let wanted = serial.to_string();
        if graph.targets.get(&id) != Some(&wanted) {
            let _ = Command::new("pw-metadata")
                .args(["-n", "default", &id.to_string(), "target.object", &wanted, "Spa:Id"])
                .status();
        }
        state.vrlink_pins.insert(id, serial);
    }
}

/// Also clears a pin to Frame Mix left by an earlier panel process.
fn unpin_vrlink(state: &mut State) {
    let mut ids: Vec<u32> = state.vrlink_pins.keys().copied().collect();
    if let Some(source) = state.graph.node_by_name("frame_amix_source") {
        let serial = source.serial.to_string();
        for node in state.graph.nodes.iter().filter(|node| Graph::is_vrlink(node)) {
            if state.graph.targets.get(&node.id) == Some(&serial) && !ids.contains(&node.id) {
                ids.push(node.id);
            }
        }
    }
    for id in ids {
        let _ = Command::new("pw-metadata").args(["-n", "default", "-d", &id.to_string(), "target.object"]).status();
        // The next pw-dump is up to two seconds away. Do not delete the same key again before it.
        state.graph.targets.remove(&id);
    }
    state.vrlink_pins.clear();
}

fn pin_apps(state: &mut State, graph: &Graph) {
    for stream in graph.playback_apps() {
        let app = if stream.app_name.is_empty() { stream.binary.clone() } else { stream.app_name.clone() };
        let Some(pin) = state.config.pins.iter().find(|pin| pin.app == app) else { continue };
        if state.pinned.contains_key(&stream.serial) {
            continue;
        }
        let Some(sink) = graph.node_by_name(&format!("frame_amix_{}", pin.channel)) else { continue };
        let _ = Command::new("pw-metadata")
            .args(["-n", "default", &stream.id.to_string(), "target.object", &sink.serial.to_string(), "Spa:Id"])
            .status();
        state.pinned.insert(stream.serial, stream.id);
    }
}

/// An app that picked a channel by name is moved by the sink's serial, like
/// `pin_channel_targets` in the WirePlumber script. A WirePlumber that is already
/// running keeps the script it started with, so a channel it does not know yet
/// still lands on its sink. The entry is cleared when that sink goes away.
fn pin_targets(state: &mut State, graph: &Graph) {
    let mut live = HashMap::new();
    for stream in graph.nodes.iter().filter(|node| node.media_class == "Stream/Output/Audio") {
        let is_channel = stream.target.strip_prefix("frame_amix_").is_some_and(is_channel_id);
        if !is_channel {
            continue;
        }
        let Some(sink) = graph.node_by_name(&stream.target) else { continue };
        if sink.serial == 0 {
            continue;
        }
        live.insert(stream.serial, (stream.id, sink.serial));
        if state.target_pins.get(&stream.serial) == Some(&(stream.id, sink.serial)) {
            continue;
        }
        let _ = Command::new("pw-metadata")
            .args(["-n", "default", &stream.id.to_string(), "target.object", &sink.serial.to_string(), "Spa:Id"])
            .status();
    }
    for (serial, (id, _)) in &state.target_pins {
        let sink_gone = graph.nodes.iter().any(|node| node.serial == *serial && node.target.starts_with("frame_amix_ch"));
        if sink_gone && !live.contains_key(serial) {
            let _ = Command::new("pw-metadata").args(["-n", "default", "-d", &id.to_string(), "target.object"]).status();
        }
    }
    state.target_pins = live;
}

fn clear_pins(state: &mut State) {
    for id in state.pinned.values() {
        let _ = Command::new("pw-metadata").args(["-n", "default", "-d", &id.to_string(), "target.object"]).status();
    }
    state.pinned.clear();
}

/// Object serial of Frame Mix. A node name is not a target on this headset.
/// Play a finished clip on the headset speakers only. The name contains
/// `virtual` and `sink`, so the spatializer does not wrap it, and the target
/// is the current output rather than the mix bus.
/// Stereo float32 at 48 kHz, the same format and quantum as the mix graph.
/// 16-bit clipped the +12 dB mic trim, and 20 ms moved the quantum (the short crackle).
/// `--target` follows an object serial on this headset. `node.dont-fallback` keeps a stale
/// serial from silently recording the default microphone. The stream stays non-virtual,
/// like VRLink, so Valve's mic filters run while recording, as they do while streaming.
struct WavInfo {
    rate: u32,
    channels: u16,
    bits: u16,
    /// 1 is integer PCM, 3 is IEEE float. Extensible files keep the subtype.
    format: u16,
    data_off: usize,
    data_len: usize,
}

fn wav_info(bytes: &[u8]) -> Option<WavInfo> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut cursor = 12usize;
    let mut rate = 0u32;
    let mut channels = 0u16;
    let mut bits = 0u16;
    let mut format = 0u16;
    let mut data_off = 0usize;
    let mut data_len = 0usize;
    let mut saw_fmt = false;
    while cursor + 8 <= bytes.len() {
        let id = &bytes[cursor..cursor + 4];
        let len = u32::from_le_bytes(bytes[cursor + 4..cursor + 8].try_into().ok()?) as usize;
        let body = cursor + 8;
        if id == b"fmt " && body + 16 <= bytes.len() {
            format = u16::from_le_bytes([bytes[body], bytes[body + 1]]);
            channels = u16::from_le_bytes([bytes[body + 2], bytes[body + 3]]);
            rate = u32::from_le_bytes(bytes[body + 4..body + 8].try_into().ok()?);
            bits = u16::from_le_bytes([bytes[body + 14], bytes[body + 15]]);
            // WAVE_FORMAT_EXTENSIBLE: the real subtype is the first field of the GUID.
            if format == 0xFFFE && len >= 26 && body + 26 <= bytes.len() {
                format = u16::from_le_bytes([bytes[body + 24], bytes[body + 25]]);
            }
            saw_fmt = true;
        } else if id == b"data" {
            data_off = body;
            data_len = len;
            break;
        }
        let step = 8 + len + (len % 2);
        if step == 0 {
            break;
        }
        cursor += step;
    }
    if !saw_fmt || rate == 0 || channels == 0 || bits < 8 || data_off == 0 {
        return None;
    }
    Some(WavInfo { rate, channels, bits, format, data_off, data_len })
}

pub fn wav_peaks_bytes(bytes: &[u8], bars: usize) -> (f64, Vec<f32>) {
    let mut wave = vec![0.0f32; bars.max(1)];
    let Some(info) = wav_info(bytes) else { return (0.0, wave) };
    let width = (info.bits as usize / 8).saturating_mul(info.channels as usize);
    if width == 0 {
        return (0.0, wave);
    }
    let end = (info.data_off + info.data_len).min(bytes.len());
    if end <= info.data_off {
        return (0.0, wave);
    }
    let pcm = &bytes[info.data_off..end];
    let frames = pcm.len() / width;
    let seconds = frames as f64 / f64::from(info.rate);
    if frames == 0 {
        return (seconds, wave);
    }
    let per = (frames / wave.len()).max(1);
    for (bar, slot) in wave.iter_mut().enumerate() {
        let start = bar * per;
        if start >= frames {
            break;
        }
        let stop = ((bar + 1) * per).min(frames);
        let mut peak = 0.0f32;
        let sample_bytes = (info.bits as usize / 8).max(1);
        for frame in start..stop {
            let base = frame * width;
            for channel in 0..info.channels as usize {
                let offset = base + channel * sample_bytes;
                if let Some(value) = sample_abs(pcm, offset, info.format, info.bits) {
                    peak = peak.max(value);
                }
            }
        }
        *slot = peak;
    }
    (seconds, wave)
}

fn sample_abs(pcm: &[u8], offset: usize, format: u16, bits: u16) -> Option<f32> {
    if format == 3 && bits == 32 {
        if offset + 4 > pcm.len() {
            return None;
        }
        let value = f32::from_le_bytes([pcm[offset], pcm[offset + 1], pcm[offset + 2], pcm[offset + 3]]);
        return value.is_finite().then_some(value.abs());
    }
    if format == 1 && bits == 16 {
        if offset + 2 > pcm.len() {
            return None;
        }
        let value = i16::from_le_bytes([pcm[offset], pcm[offset + 1]]).unsigned_abs() as f32 / 32768.0;
        return Some(value);
    }
    None
}

pub fn self_test() -> Result<(), String> {
    if solo_mute(false, false, true) != true || solo_mute(false, true, true) != false || solo_mute(true, true, false) != true {
        return Err("solo mute".into());
    }
    let mut frames = vec![[0i16, 0i16]; 480];
    frames[0] = [32767, -32768];
    let bytes = pcm_wav(48000, 2, &frames);
    let (seconds, wave) = wav_peaks_bytes(&bytes, 36);
    if (seconds - 0.01).abs() > 0.000_1 {
        return Err(format!("duration {seconds}"));
    }
    if wave.first().copied().unwrap_or(0.0) < 0.99 {
        return Err(format!("peak {wave:?}"));
    }
    let mut floats = vec![[0.0f32, 0.0f32]; 480];
    floats[0] = [1.25, -0.5];
    let float_bytes = float_wav(48000, 2, &floats);
    let (float_seconds, float_wave) = wav_peaks_bytes(&float_bytes, 36);
    if (float_seconds - 0.01).abs() > 0.000_1 {
        return Err(format!("float duration {float_seconds}"));
    }
    let float_peak = float_wave.first().copied().unwrap_or(0.0);
    if (float_peak - 1.25).abs() > 0.001 {
        return Err(format!("float peak {float_peak}"));
    }
    Ok(())
}

fn pcm_wav(rate: u32, channels: u16, frames: &[[i16; 2]]) -> Vec<u8> {
    let data_len = frames.len() * 4;
    let mut bytes = Vec::with_capacity(44 + data_len);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    let block = u32::from(channels) * 2;
    bytes.extend_from_slice(&(rate * block).to_le_bytes());
    bytes.extend_from_slice(&(block as u16).to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_len as u32).to_le_bytes());
    for frame in frames {
        bytes.extend_from_slice(&frame[0].to_le_bytes());
        bytes.extend_from_slice(&frame[1].to_le_bytes());
    }
    bytes
}

fn float_wav(rate: u32, channels: u16, frames: &[[f32; 2]]) -> Vec<u8> {
    let data_len = frames.len() * 8;
    let mut bytes = Vec::with_capacity(44 + data_len);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&3u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    let block = u32::from(channels) * 4;
    bytes.extend_from_slice(&(rate * block).to_le_bytes());
    bytes.extend_from_slice(&(block as u16).to_le_bytes());
    bytes.extend_from_slice(&32u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_len as u32).to_le_bytes());
    for frame in frames {
        bytes.extend_from_slice(&frame[0].to_le_bytes());
        bytes.extend_from_slice(&frame[1].to_le_bytes());
    }
    bytes
}

fn install_icons(accent: &str) {
    let pink = accent == "pink";
    let home = std::env::var("HOME").unwrap_or_default();
    let data = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{home}/.local/share"));
    for size in [48u32, 128, 256] {
        let path = PathBuf::from(&data)
            .join("icons/hicolor")
            .join(format!("{size}x{size}"))
            .join("apps/frame-audio-mixer.png");
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, crate::brand::icon_png(pink, size));
    }
}

/// The sink description is read only when the channel process starts.
/// Restart that one unit so a rename shows up in other apps.
fn write_names(channels: &[Channel]) {
    for channel in channels {
        if !is_channel_id(&channel.id) {
            continue;
        }
        let path = name_file(&channel.id);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, format!("{}\n", channel.name));
    }
}

fn set_autostart(on: bool) {
    let verb = if on { "enable" } else { "disable" };
    let _ = Command::new("systemctl").args(["--user", verb, "frame-audio-mixer.service"]).status();
    let home = std::env::var("HOME").unwrap_or_default();
    let dir = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{home}/.config"));
    let path = PathBuf::from(dir).join("frame-audio-mixer").join("install-args");
    let _ = fs::create_dir_all(path.parent().unwrap());
    let body = if on { "--autostart\n" } else { "" };
    let _ = fs::write(path, body);
}

fn save_setting(key: &str, value: &str) {
    let _ = Command::new("wpctl").args(["settings", "--save", key, value]).status();
}

fn unit_active(unit: &str) -> bool {
    Command::new("systemctl").args(["--user", "is-active", "--quiet", unit]).status().is_ok_and(|s| s.success())
}

fn unit_enabled(unit: &str) -> bool {
    Command::new("systemctl").args(["--user", "is-enabled", "--quiet", unit]).status().is_ok_and(|s| s.success())
}

fn tuner_version() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let bin = PathBuf::from(home).join(".local/bin/frame-mic-tuner");
    if !bin.exists() {
        return String::new();
    }
    let out = Command::new(&bin).arg("--version").output();
    match out {
        Ok(out) => String::from_utf8_lossy(&out.stdout).split_whitespace().last().unwrap_or("").to_string(),
        Err(_) => "installed".into(),
    }
}

fn echo_cancel() -> Option<bool> {
    // `wpctl settings` with no key crashes in WirePlumber's JSON parser on this
    // headset. The saved file is the same value, without starting wpctl.
    let home = std::env::var("HOME").unwrap_or_default();
    let text = fs::read_to_string(PathBuf::from(home).join(".local/state/wireplumber/sm-settings")).ok()?;
    for line in text.lines() {
        let Some(value) = line.trim().strip_prefix("frame-mic.echo-cancel=") else { continue };
        return match value.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        };
    }
    None
}

fn name_file(id: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    let dir = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{home}/.config"));
    PathBuf::from(dir).join("pipewire").join("frame-audio-mixer").join(format!("{id}.name"))
}

/// Used by the self-test to prove a picked app is remembered by serial, not re-sent.
pub fn pin_once(seen: &mut HashMap<u64, u32>, stream: &AudioNode) -> bool {
    if seen.contains_key(&stream.serial) {
        return false;
    }
    seen.insert(stream.serial, stream.id);
    true
}
