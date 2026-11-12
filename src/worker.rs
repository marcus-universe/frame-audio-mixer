//! systemctl, wpctl and pw-metadata run here, off the UI thread.
//! Nothing in this module calls `wpctl set-default`. The Microphone fader
//! writes only `frame_amix_mic_virtual_sink`.

use crate::config::{Channel, Config, Pin, is_channel_id};
use crate::graph::{AudioNode, Graph};
use crate::apps::{self, Entry};
use crate::updater::{self, UpdateView};
use std::collections::{HashMap, HashSet};
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
    /// Plugins from the last scan.
    pub vsts: Vec<crate::vst::VstInfo>,
    /// Host progress by plugin instance.
    pub vst_status: HashMap<String, String>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            graph: Graph::default(),
            mix_active: false,
            echo_cancel: None,
            autostart: false,
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
            vsts: Vec::new(),
            vst_status: HashMap::new(),
        }
    }
}

pub enum Job {
    Apply(Config),
    /// Fader volumes during a drag. Applied on this worker turn, nothing else changes.
    Levels(Config),
    Autostart(bool),
    /// Records the output named by `Config::check_output`.
    Record(Config),
    StopCheck,
    PlayTake,
    PauseTake,
    CheckUpdates,
    InstallUpdate,
    OpenInstaller(String),
    /// Scans a folder for VST3 plugins.
    ScanVst(String),
    /// Opens the editor of the plugin with this instance id.
    VstGui(String),
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
    facts_at: Option<Instant>,
    /// Until then the graph is polled every loop pass, so apps return to their
    /// channels right after the engine (re)builds them.
    settle_until: Option<Instant>,
    /// Stream serial to (stream id, channel sink serial) written for it. A new
    /// sink serial (engine restarted) pins the stream again.
    pinned: HashMap<u64, (u32, u64)>,
    /// (stream serial, wrong sink serial) pairs already moved once. A stream the
    /// user deliberately puts back is not moved again.
    bounced: HashSet<(u64, u64)>,
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
    vsts: Vec<crate::vst::VstInfo>,
    vst_status: HashMap<String, String>,
}

fn run(rx: mpsc::Receiver<Job>, snap: Arc<Mutex<Snapshot>>, config: Config) {
    let mut state = State {
        config,
        graph: Graph::default(),
        mix_active: false,
        echo_cancel: None,
        autostart: false,
        facts_at: None,
        settle_until: None,
        pinned: HashMap::new(),
        bounced: HashSet::new(),
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
        vsts: crate::vst::load_cache(),
        vst_status: HashMap::new(),
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
        let wait = if settling(&state) { 150 } else { 800 };
        let job = rx.recv_timeout(Duration::from_millis(wait));
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
            // The socket first: the engine is almost always up, and a
            // `systemctl is-active` round trip would delay the take's start.
            if !send_engine(state, "record") {
                ensure_engine(state);
                send_engine(state, "record");
            }
            return true;
        }
        Job::StopCheck => {
            send_engine(state, "stop");
            return true;
        }
        Job::PlayTake => {
            if !send_engine(state, "play") {
                ensure_engine(state);
                send_engine(state, "play");
            }
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
        Job::ScanVst(folder) => {
            state.vsts = crate::vst::scan(&crate::vst::expand(&folder));
            crate::vst::save_cache(&state.vsts);
        }
        Job::VstGui(instance) => {
            crate::engine::open_vst_editor(&instance);
        }
        Job::OpenInstaller(key) => {
            if let Err(err) = state.apps.launch(&key) {
                state.message = format!("{err:?}");
            }
        }
        Job::Quit => {}
    }
    false
}

fn settling(state: &State) -> bool {
    state.settle_until.is_some_and(|until| Instant::now() < until)
}

/// True once every configured channel has its sink in the graph.
fn channels_up(state: &State) -> bool {
    state.config.channels.iter().all(|channel| {
        state.graph.node_by_name(&format!("frame_amix_{}", channel.id)).is_some_and(|node| node.serial != 0)
    })
}

fn refresh_facts(state: &mut State, force: bool) {
    let due = force || state.facts_at.is_none_or(|start| start.elapsed() >= Duration::from_secs(2));
    if !due && !settling(state) {
        return;
    }
    state.graph = Graph::default();
    if let Some(status) = crate::engine::call("status", None) {
        apply_status(state, status);
    }
    if settling(state) && channels_up(state) {
        // This pass pins the apps; the normal pace is enough after it.
        state.settle_until = None;
    }
    if !due {
        return;
    }
    state.echo_cancel = echo_cancel();
    state.autostart = unit_enabled("frame-audio-mixer.service");
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
        vsts: state.vsts.clone(),
        vst_status: state.vst_status.clone(),
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
    enforce_channels(state, &graph);
    pin_vrlink(state, &graph);
}

/// The channel a stream is pinned to. A matching binary wins outright; a pin by
/// display name only counts while the binaries do not disagree. Electron apps
/// often all report "Chromium", so a name alone used to move the wrong app.
pub fn pin_channel_for<'a>(pins: &'a [Pin], channels: &[Channel], stream: &AudioNode) -> Option<&'a str> {
    if !stream.binary.is_empty() {
        let by_binary = channels
            .iter()
            .find(|channel| !channel.binary.is_empty() && channel.binary.eq_ignore_ascii_case(&stream.binary))
            .and_then(|channel| pins.iter().find(|pin| pin.channel == channel.id));
        if let Some(pin) = by_binary {
            return Some(&pin.channel);
        }
    }
    let label = if stream.app_name.is_empty() { &stream.binary } else { &stream.app_name };
    let pin = pins.iter().find(|pin| &pin.app == label)?;
    if let Some(channel) = channels.iter().find(|channel| channel.id == pin.channel) {
        let other_binary = !channel.binary.is_empty()
            && !stream.binary.is_empty()
            && !channel.binary.eq_ignore_ascii_case(&stream.binary);
        if other_binary {
            return None;
        }
    }
    Some(&pin.channel)
}

/// Streams sitting on a channel they are not pinned to are moved once: onto
/// their own channel, or off the mixer so unassigned audio plays on the normal
/// output. A stream whose own target names a channel is the app's choice and
/// `pin_targets` already honors it.
fn enforce_channels(state: &mut State, graph: &Graph) {
    let sinks: Vec<(&Channel, &AudioNode)> = state
        .config
        .channels
        .iter()
        .filter_map(|channel| graph.node_by_name(&format!("frame_amix_{}", channel.id)).map(|node| (channel, node)))
        .collect();
    if sinks.is_empty() || graph.edges.is_empty() {
        return;
    }
    let streams = graph.playback_apps();
    let live: HashSet<u64> = streams.iter().map(|stream| stream.serial).collect();
    state.bounced.retain(|(serial, _)| live.contains(serial));
    for stream in &streams {
        if stream.target.strip_prefix("frame_amix_").is_some_and(is_channel_id) {
            continue;
        }
        let Some((channel, sink)) = sinks.iter().find(|(_, sink)| graph.edges.contains(&(stream.id, sink.id))) else {
            continue;
        };
        let expected = pin_channel_for(&state.config.pins, &state.config.channels, stream);
        if expected == Some(channel.id.as_str()) {
            continue;
        }
        if !state.bounced.insert((stream.serial, sink.serial)) {
            continue;
        }
        let wanted = expected.and_then(|id| graph.node_by_name(&format!("frame_amix_{id}")));
        if let Some(wanted) = wanted {
            // Pinned to another channel: send it there.
            let _ = Command::new("pw-metadata")
                .args(["-n", "default", &stream.id.to_string(), "target.object", &wanted.serial.to_string(), "Spa:Id"])
                .status();
            state.pinned.insert(stream.serial, (stream.id, wanted.serial));
        } else if graph.default_sink.starts_with("frame_amix_") {
            // The session default itself is a mixer channel, so dropping the
            // target would land right back here. Aim at the real device.
            let devices = graph.output_sinks();
            let device = devices[1].as_deref().or(devices[2].as_deref()).or(devices[0].as_deref());
            if let Some(serial) = device.and_then(|name| graph.node_by_name(name)).map(|node| node.serial) {
                let _ = Command::new("pw-metadata")
                    .args(["-n", "default", &stream.id.to_string(), "target.object", &serial.to_string(), "Spa:Id"])
                    .status();
            }
        } else {
            // Unassigned: drop the stray target so the stream follows the
            // normal output again.
            let _ = Command::new("pw-metadata").args(["-n", "default", "-d", &stream.id.to_string(), "target.object"]).status();
            state.pinned.remove(&stream.serial);
        }
    }
}

/// The engine runs while this app runs. It is started, never enabled, so quitting
/// the app (see `stop_engine`) leaves no virtual device behind.
fn ensure_engine(state: &mut State) {
    let want = state.config.mix_on || !state.config.channels.is_empty();
    let unit = ENGINE_UNIT;
    let running = unit_active(unit);
    if want && !running {
        let _ = Command::new("systemctl").args(["--user", "start", unit]).status();
        // `systemctl start` returns once the process runs, before its socket is up.
        // Waiting here lets the "apply" that follows build the channels at once,
        // instead of the engine loading the config from disk a second later.
        let until = Instant::now() + Duration::from_secs(3);
        while Instant::now() < until && crate::engine::call("status", None).is_none() {
            std::thread::sleep(Duration::from_millis(50));
        }
        state.settle_until = Some(Instant::now() + Duration::from_secs(8));
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

/// One request to the engine. False means the engine socket did not answer.
fn send_engine(state: &mut State, op: &str) -> bool {
    match crate::engine::call(op, Some(&state.config)) {
        Some(status) => {
            apply_status(state, status);
            true
        }
        None => false,
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
    state.vst_status = status.vst;
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
        let Some(channel) = pin_channel_for(&state.config.pins, &state.config.channels, &stream) else { continue };
        let Some(sink) = graph.node_by_name(&format!("frame_amix_{channel}")).filter(|sink| sink.serial != 0) else { continue };
        if state.pinned.get(&stream.serial) == Some(&(stream.id, sink.serial)) {
            continue;
        }
        let _ = Command::new("pw-metadata")
            .args(["-n", "default", &stream.id.to_string(), "target.object", &sink.serial.to_string(), "Spa:Id"])
            .status();
        state.pinned.insert(stream.serial, (stream.id, sink.serial));
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
    for (id, _) in state.pinned.values() {
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
    if !solo_mute(false, false, true) || solo_mute(false, true, true) || !solo_mute(true, true, false) {
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
    let channel = |id: &str, name: &str, app: &str, binary: &str| {
        let mut channel = Channel::new(id, name);
        channel.app = app.into();
        channel.binary = binary.into();
        channel
    };
    let stream = |app_name: &str, binary: &str| AudioNode {
        id: 1,
        serial: 1,
        name: "stream".into(),
        description: String::new(),
        media_class: "Stream/Output/Audio".into(),
        media_name: String::new(),
        app_name: app_name.into(),
        binary: binary.into(),
        is_virtual: false,
        target: String::new(),
    };
    let channels = vec![channel("ch1", "Feishin", "Feishin", "feishin")];
    let pins = vec![Pin { app: "Feishin".into(), channel: "ch1".into() }];
    if pin_channel_for(&pins, &channels, &stream("Feishin", "feishin")) != Some("ch1") {
        return Err("feishin not pinned to its channel".into());
    }
    if pin_channel_for(&pins, &channels, &stream("Chromium", "chromium")).is_some() {
        return Err("chromium stole the feishin channel".into());
    }
    // Electron apps often report the shared name; the binary still finds the channel.
    if pin_channel_for(&pins, &channels, &stream("Chromium", "feishin")) != Some("ch1") {
        return Err("binary match ignored".into());
    }
    // A name pin with a different binary behind the channel must not match.
    let shared = vec![channel("ch2", "Browser", "Chromium", "chromium")];
    let shared_pin = vec![Pin { app: "Chromium".into(), channel: "ch2".into() }];
    if pin_channel_for(&shared_pin, &shared, &stream("Chromium", "feishin")).is_some() {
        return Err("shared name matched across binaries".into());
    }
    // A custom channel without a stored binary still pins by name alone.
    let open = vec![channel("ch3", "Music", "", "")];
    let open_pin = vec![Pin { app: "Spotify".into(), channel: "ch3".into() }];
    if pin_channel_for(&open_pin, &open, &stream("Spotify", "spotify")) != Some("ch3") {
        return Err("name pin without binary".into());
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
