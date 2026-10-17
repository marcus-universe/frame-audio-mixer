//! systemctl, wpctl and pw-metadata run here, off the UI thread.
//! Nothing in this module calls `wpctl set-default`. The Microphone fader
//! writes only `frame_amix_mic_virtual_sink`.

use crate::config::{Channel, Config, is_channel_id};
use crate::graph::{AudioNode, Graph, parse_dump};
use crate::apps::{self, Entry};
use crate::updater::{self, UpdateView};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckPhase {
    Idle,
    Recording,
    Ready,
    Playing,
}

#[derive(Debug, Clone)]
pub struct MixClip {
    pub id: u64,
    pub seconds: f64,
    pub clock: String,
    pub wave: Vec<f32>,
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
    pub check_secs: f64,
    pub playing_id: u64,
    pub clips: Vec<MixClip>,
    pub message: String,
    pub apps: Vec<Entry>,
    pub menu_busy: bool,
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
            playing_id: 0,
            clips: Vec::new(),
            message: String::new(),
            apps: Vec::new(),
            menu_busy: false,
        }
    }
}

pub enum Job {
    Apply(Config),
    Refresh,
    Autostart(bool),
    Record,
    StopCheck,
    PlayClip(u64),
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
    levels: HashMap<String, (u32, String)>,
    check: Option<Child>,
    check_phase: CheckPhase,
    check_started: Option<Instant>,
    message: String,
    apps: apps::Manager,
    mutes: HashMap<String, bool>,
    last_check: Option<Instant>,
    clips: Vec<StoredClip>,
    next_clip: u64,
    playing_id: u64,
    record_path: Option<PathBuf>,
    booted: bool,
}

struct StoredClip {
    id: u64,
    seconds: f64,
    clock: String,
    wave: Vec<f32>,
    path: PathBuf,
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
        levels: HashMap::new(),
        check: None,
        check_phase: CheckPhase::Idle,
        check_started: None,
        message: String::new(),
        apps: apps::Manager::new(),
        mutes: HashMap::new(),
        last_check: None,
        clips: Vec::new(),
        next_clip: 0,
        playing_id: 0,
        record_path: None,
        booted: false,
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
        match job {
            Ok(Job::Quit) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(job) => urgent = apply_job(&mut state, &snap, job),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        while let Ok(extra) = rx.try_recv() {
            if matches!(extra, Job::Quit) {
                return;
            }
            if apply_job(&mut state, &snap, extra) {
                urgent = true;
            }
        }
        if urgent {
            // The button and the clip list update before the next pw-dump.
            publish_check(&snap, &state);
        }
        state.apps.tick(state.config.check_updates);
        let check_due = state.config.check_updates
            && state.last_check.is_none_or(|start| start.elapsed() >= Duration::from_secs(3600));
        if check_due {
            let _ = updater::check(false);
            state.last_check = Some(Instant::now());
        }
        tick_check(&mut state);
        refresh_facts(&mut state, false);
        sync_audio(&mut state);
        publish(&snap, &state);
    }
    stop_child(&mut state);
    if state.config.mix_on {
        // Leaving the process does not turn Mix off. The graph unit keeps running.
    }
}

/// Record, stop, and play return true so the panel can show that phase before `pw-dump`.
fn apply_job(state: &mut State, snap: &Mutex<Snapshot>, job: Job) -> bool {
    match job {
        Job::Apply(config) => apply_config(state, config),
        Job::Autostart(on) => set_autostart(on),
        Job::Record => {
            start_record(state);
            return true;
        }
        Job::StopCheck => {
            finish_recording(state);
            return true;
        }
        Job::PlayClip(id) => {
            play_clip(state, id);
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
    state.graph = dump_graph();
    state.mix_active = unit_active("frame-audio-mixer-graph.service");
    state.echo_cancel = echo_cancel();
    state.autostart = unit_enabled("frame-audio-mixer.service");
    if state.tuner_version.is_empty() {
        state.tuner_version = tuner_version();
    }
    state.facts_at = Some(Instant::now());
}

fn check_fields(state: &State) -> (CheckPhase, f64, u64, Vec<MixClip>, String) {
    let clips = state
        .clips
        .iter()
        .map(|clip| MixClip {
            id: clip.id,
            seconds: clip.seconds,
            clock: clip.clock.clone(),
            wave: clip.wave.clone(),
        })
        .collect();
    let secs = state.check_started.map(|start| start.elapsed().as_secs_f64()).unwrap_or(0.0);
    (state.check_phase, secs, state.playing_id, clips, state.message.clone())
}

fn publish_check(snap: &Mutex<Snapshot>, state: &State) {
    let Ok(mut guard) = snap.lock() else { return };
    let (phase, secs, playing, clips, message) = check_fields(state);
    guard.check = phase;
    guard.check_secs = secs;
    guard.playing_id = playing;
    guard.clips = clips;
    guard.message = message;
}

fn publish(snap: &Mutex<Snapshot>, state: &State) {
    let mut guard = match snap.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    let (phase, secs, playing, clips, message) = check_fields(state);
    *guard = Snapshot {
        graph: state.graph.clone(),
        mix_active: state.mix_active,
        echo_cancel: state.echo_cancel,
        autostart: state.autostart,
        tuner_version: state.tuner_version.clone(),
        update: updater::current_view(),
        check: phase,
        check_secs: secs,
        playing_id: playing,
        clips,
        message,
        apps: state.apps.entries().to_vec(),
        menu_busy: state.apps.menu_busy(),
    };
}

fn apply_config(state: &mut State, config: Config) {
    let was = state.config.mix_on;
    let old_mic = state.config.mic_source.clone();
    let old_accent = state.config.accent.clone();
    let old_ids: Vec<String> = state.config.channels.iter().map(|c| c.id.clone()).collect();
    state.config = config;
    if state.config.mic_source != old_mic {
        save_setting("frame-audio-mixer.mic-source", &state.config.mic_source);
    }
    if state.config.accent != old_accent {
        install_icons(&state.config.accent);
    }
    write_names(&state.config.channels);
    let ids_now: Vec<String> = state.config.channels.iter().map(|channel| channel.id.clone()).collect();
    let first = !state.booted;
    state.booted = true;
    if !first && ids_now == old_ids && state.config.mix_on == was {
        return;
    }
    if state.config.mix_on && !was {
        save_setting("frame-audio-mixer.enabled", "true");
        save_setting("frame-audio-mixer.vrlink-gain", &format!("{:.3}", state.config.vrlink_gain));
    } else if !state.config.mix_on && was {
        save_setting("frame-audio-mixer.enabled", "false");
        restore_vrlink();
        clear_pins(state);
    }
    for channel in &state.config.channels {
        if is_channel_id(&channel.id) && !old_ids.contains(&channel.id) {
            let _ = Command::new("systemctl")
                .args(["--user", "enable", "--now", &format!("frame-audio-mixer-channel@{}.service", channel.id)])
                .status();
        }
    }
    for id in &old_ids {
        if !state.config.channels.iter().any(|channel| &channel.id == id) && is_channel_id(id) {
            let unit = format!("frame-audio-mixer-channel@{id}.service");
            let _ = Command::new("systemctl").args(["--user", "disable", "--now", &unit]).status();
            let _ = Command::new("systemctl").args(["--user", "reset-failed", &unit]).status();
            let _ = fs::remove_file(name_file(id));
        }
    }
    // The bus has to be up for a channel sink to exist. VRLink follows the mix
    // only while frame-audio-mixer.enabled is true, which stays false until Mix is on.
    let want_graph = state.config.mix_on || !state.config.channels.is_empty();
    let graph_unit = "frame-audio-mixer-graph.service";
    let running = unit_active(graph_unit);
    if want_graph && !running {
        let _ = Command::new("systemctl").args(["--user", "enable", "--now", graph_unit]).status();
    } else if !want_graph && running {
        let _ = Command::new("systemctl").args(["--user", "disable", "--now", graph_unit]).status();
    }
    // A channel added before the graph is up is ordered After= the graph, and restarts if the bus is late.
    for channel in &state.config.channels {
        if is_channel_id(&channel.id) && !unit_active(&format!("frame-audio-mixer-channel@{}.service", channel.id)) {
            let _ = Command::new("systemctl")
                .args(["--user", "enable", "--now", &format!("frame-audio-mixer-channel@{}.service", channel.id)])
                .status();
        }
    }
    // New nodes show up on the next fact refresh, not on the click that starts a recording.
    state.facts_at = None;
}

fn sync_audio(state: &mut State) {
    let graph = state.graph.clone();
    apply_level(state, "frame_amix_mic_virtual_sink", state.config.mic_volume, &graph);
    set_mute(state, "frame_amix_mic_virtual_sink", state.config.mic_muted, &graph);
    let channel_levels: Vec<(String, f64, f64, bool, bool)> = state
        .config
        .channels
        .iter()
        .map(|channel| (channel.id.clone(), channel.phones, channel.to_mic, channel.phones_muted, channel.to_mic_muted))
        .collect();
    for (id, phones, to_mic, phones_muted, to_mic_muted) in channel_levels {
        apply_level(state, &format!("frame_amix_{id}_virtual_sink_playback"), phones, &graph);
        apply_level(state, &format!("frame_amix_{id}_virtual_sink_to_bus"), to_mic, &graph);
        set_mute(state, &format!("frame_amix_{id}_virtual_sink_playback"), phones_muted, &graph);
        set_mute(state, &format!("frame_amix_{id}_virtual_sink_to_bus"), to_mic_muted, &graph);
    }
    pin_targets(state, &graph);
    if !state.config.mix_on {
        if !state.pinned.is_empty() {
            clear_pins(state);
        }
        restore_vrlink();
        return;
    }
    pin_apps(state, &graph);
    touch_vrlink(state, &graph);
}

fn apply_level(state: &mut State, name: &str, volume: f64, graph: &Graph) {
    let Some(node) = graph.node_by_name(name) else { return };
    let printed = format!("{volume:.3}");
    if state.levels.get(name).is_some_and(|prev| prev.0 == node.id && prev.1 == printed) {
        return;
    }
    let _ = Command::new("wpctl").args(["set-volume", &node.id.to_string(), &printed]).status();
    state.levels.insert(name.to_string(), (node.id, printed));
}

fn set_mute(state: &mut State, name: &str, muted: bool, graph: &Graph) {
    let Some(node) = graph.node_by_name(name) else { return };
    let key = format!("{name}:{}", node.id);
    if state.mutes.get(&key) == Some(&muted) {
        return;
    }
    mute_node(name, muted, graph);
    state.mutes.insert(key, muted);
}

fn mute_node(name: &str, muted: bool, graph: &Graph) {
    let Some(node) = graph.node_by_name(name) else { return };
    let flag = if muted { "1" } else { "0" };
    let _ = Command::new("wpctl").args(["set-mute", &node.id.to_string(), flag]).status();
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
            .args(["-n", "default", &stream.id.to_string(), "target.object", &sink.serial.to_string()])
            .status();
        state.pinned.insert(stream.serial, stream.id);
    }
}

/// An app that picked `Frame Mix: <name>` itself is moved by the sink's serial, like
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
            .args(["-n", "default", &stream.id.to_string(), "target.object", &sink.serial.to_string()])
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

fn touch_vrlink(state: &State, graph: &Graph) {
    if (state.config.vrlink_gain - 1.0).abs() < 0.001 {
        return;
    }
    let path = saved_volume_path();
    if path.exists() {
        return;
    }
    let Some(node) = graph.nodes.iter().find(|node| Graph::is_vrlink(node)) else { return };
    let current = get_volume(node.id);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if fs::write(&path, current).is_ok() {
        let _ = Command::new("wpctl")
            .args(["set-volume", &node.id.to_string(), &format!("{:.3}", state.config.vrlink_gain)])
            .status();
    }
}

fn restore_vrlink() {
    let path = saved_volume_path();
    let Ok(saved) = fs::read_to_string(&path) else { return };
    let graph = dump_graph();
    if let Some(node) = graph.nodes.iter().find(|node| Graph::is_vrlink(node)) {
        let _ = Command::new("wpctl").args(["set-volume", &node.id.to_string(), saved.trim()]).status();
    }
    let _ = fs::remove_file(path);
}

fn get_volume(id: u32) -> String {
    let out = Command::new("wpctl").args(["get-volume", &id.to_string()]).output();
    let Ok(out) = out else { return "1.000".into() };
    let text = String::from_utf8_lossy(&out.stdout);
    text.split_whitespace().nth(1).unwrap_or("1.000").to_string()
}

fn start_record(state: &mut State) {
    if state.check_phase == CheckPhase::Recording {
        finish_recording(state);
        return;
    }
    stop_child(state);
    state.playing_id = 0;
    state.check_phase = CheckPhase::Idle;
    let path = check_dir().join(format!("mix-{}.wav", state.next_clip + 1));
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let target = record_target(state);
    let path_arg = path.to_string_lossy().to_string();
    let child = Command::new("pw-record")
        .args(check_record_args(&target, &path_arg))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    match child {
        Ok(child) => {
            state.record_path = Some(path);
            state.check = Some(child);
            state.check_phase = CheckPhase::Recording;
            state.check_started = Some(Instant::now());
            state.message.clear();
        }
        Err(err) => state.message = err.to_string(),
    }
}

fn finish_recording(state: &mut State) {
    stop_child(state);
    state.check_started = None;
    state.check_phase = CheckPhase::Idle;
    let Some(path) = state.record_path.take() else { return };
    let (seconds, wave) = wav_peaks(&path, 36);
    if seconds < 0.2 {
        let _ = fs::remove_file(&path);
        return;
    }
    state.next_clip += 1;
    state.clips.insert(
        0,
        StoredClip {
            id: state.next_clip,
            seconds,
            clock: clock_now(),
            wave,
            path,
        },
    );
    while state.clips.len() > 5 {
        if let Some(old) = state.clips.pop() {
            let _ = fs::remove_file(old.path);
        }
    }
}

fn play_clip(state: &mut State, id: u64) {
    if state.check_phase == CheckPhase::Recording {
        finish_recording(state);
    }
    if state.playing_id == id && state.check_phase == CheckPhase::Playing {
        stop_child(state);
        state.playing_id = 0;
        state.check_phase = CheckPhase::Idle;
        state.check_started = None;
        return;
    }
    let Some(path) = state.clips.iter().find(|clip| clip.id == id).map(|clip| clip.path.clone()) else {
        return;
    };
    stop_child(state);
    let child = Command::new("pw-play").arg(&path).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    match child {
        Ok(child) => {
            state.check = Some(child);
            state.check_phase = CheckPhase::Playing;
            state.playing_id = id;
            state.check_started = Some(Instant::now());
            state.message.clear();
        }
        Err(err) => state.message = err.to_string(),
    }
}

fn stop_child(state: &mut State) {
    if let Some(mut child) = state.check.take() {
        let _ = Command::new("kill").args(["-TERM", &child.id().to_string()]).status();
        let _ = child.wait();
    }
}

fn tick_check(state: &mut State) {
    let finished = state.check.as_mut().is_some_and(|child| child.try_wait().ok().flatten().is_some());
    let timed_out = state.check_phase == CheckPhase::Recording
        && state.check_started.is_some_and(|start| start.elapsed() >= Duration::from_secs(10));
    if state.check_phase == CheckPhase::Recording && (timed_out || finished) {
        finish_recording(state);
    } else if state.check_phase == CheckPhase::Playing && finished {
        state.check = None;
        state.check_started = None;
        state.playing_id = 0;
        state.check_phase = CheckPhase::Idle;
    }
}

/// Stereo 48 kHz, the same rate the mix graph uses. 16 kHz mono is what made Mix Check sound dull.
pub fn check_record_args(target: &str, path: &str) -> Vec<String> {
    vec![
        "--target".into(),
        target.into(),
        "--rate".into(),
        "48000".into(),
        "--channels".into(),
        "2".into(),
        "--format".into(),
        "s16".into(),
        "--latency".into(),
        "20ms".into(),
        path.into(),
    ]
}

fn record_target(state: &State) -> String {
    let graph = &state.graph;
    if graph.node_by_name("frame_amix_source").is_some() {
        return "frame_amix_source".to_string();
    }
    if state.config.mic_source.is_empty() {
        if !graph.default_source.is_empty() {
            return graph.default_source.clone();
        }
        return Graph::BUILTIN_SOURCE.to_string();
    }
    if state.config.mic_source == "builtin" {
        return Graph::BUILTIN_SOURCE.to_string();
    }
    state.config.mic_source.clone()
}

fn clock_now() -> String {
    let out = Command::new("date").arg("+%H:%M:%S").output();
    match out {
        Ok(out) => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Err(_) => String::new(),
    }
}

struct WavInfo {
    rate: u32,
    channels: u16,
    bits: u16,
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
    let mut data_off = 0usize;
    let mut data_len = 0usize;
    let mut saw_fmt = false;
    while cursor + 8 <= bytes.len() {
        let id = &bytes[cursor..cursor + 4];
        let len = u32::from_le_bytes(bytes[cursor + 4..cursor + 8].try_into().ok()?) as usize;
        let body = cursor + 8;
        if id == b"fmt " && body + 16 <= bytes.len() {
            channels = u16::from_le_bytes([bytes[body + 2], bytes[body + 3]]);
            rate = u32::from_le_bytes(bytes[body + 4..body + 8].try_into().ok()?);
            bits = u16::from_le_bytes([bytes[body + 14], bytes[body + 15]]);
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
    Some(WavInfo { rate, channels, bits, data_off, data_len })
}

fn wav_peaks(path: &std::path::Path, bars: usize) -> (f64, Vec<f32>) {
    let bytes = fs::read(path).unwrap_or_default();
    wav_peaks_bytes(&bytes, bars)
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
        if info.bits == 16 {
            for frame in start..stop {
                let base = frame * width;
                for channel in 0..info.channels as usize {
                    let offset = base + channel * 2;
                    if offset + 2 > pcm.len() {
                        break;
                    }
                    let value = i16::from_le_bytes([pcm[offset], pcm[offset + 1]]).unsigned_abs() as f32 / 32768.0;
                    peak = peak.max(value);
                }
            }
        }
        *slot = peak;
    }
    (seconds, wave)
}

pub fn self_test() -> Result<(), String> {
    let args = check_record_args("frame_amix_source", "/tmp/mix.wav");
    let joined = args.join(" ");
    if !joined.contains("--rate 48000") || !joined.contains("--channels 2") || !joined.contains("--format s16") {
        return Err(joined);
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
    let out = Command::new("wpctl").args(["settings", "frame-mic.echo-cancel"]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
    if text.contains("true") {
        Some(true)
    } else if text.contains("false") {
        Some(false)
    } else {
        None
    }
}

fn dump_graph() -> Graph {
    let out = Command::new("pw-dump").output();
    match out {
        Ok(out) => parse_dump(&String::from_utf8_lossy(&out.stdout)).unwrap_or_default(),
        Err(_) => Graph::default(),
    }
}

fn name_file(id: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    let dir = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{home}/.config"));
    PathBuf::from(dir).join("pipewire").join("frame-audio-mixer").join(format!("{id}.name"))
}

fn saved_volume_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    let dir = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{home}/.config"));
    PathBuf::from(dir).join("frame-audio-mixer").join("vrlink-saved-volume")
}

fn check_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".local/share/frame-audio-mixer")
}

/// Used by the self-test to prove a picked app is remembered by serial, not re-sent.
pub fn pin_once(seen: &mut HashMap<u64, u32>, stream: &AudioNode) -> bool {
    if seen.contains_key(&stream.serial) {
        return false;
    }
    seen.insert(stream.serial, stream.id);
    true
}
