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
pub struct Snapshot {
    pub graph: Graph,
    pub mix_active: bool,
    pub echo_cancel: Option<bool>,
    pub autostart: bool,
    pub tuner_version: String,
    pub update: UpdateView,
    pub check: CheckPhase,
    pub check_secs: f64,
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
    PlayCheck,
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
    pinned: HashMap<u64, u32>,
    levels: HashMap<String, (u32, String)>,
    check: Option<Child>,
    check_phase: CheckPhase,
    check_started: Option<Instant>,
    message: String,
    apps: apps::Manager,
    mutes: HashMap<String, bool>,
    last_check: Option<Instant>,
}

fn run(rx: mpsc::Receiver<Job>, snap: Arc<Mutex<Snapshot>>, config: Config) {
    let mut state = State {
        config,
        pinned: HashMap::new(),
        levels: HashMap::new(),
        check: None,
        check_phase: CheckPhase::Idle,
        check_started: None,
        message: String::new(),
        apps: apps::Manager::new(),
        mutes: HashMap::new(),
        last_check: None,
    };
    publish(&snap, &state);
    loop {
        let job = rx.recv_timeout(Duration::from_millis(800));
        match job {
            Ok(Job::Quit) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(Job::Apply(config)) => apply_config(&mut state, config),
            Ok(Job::Autostart(on)) => set_autostart(on),
            Ok(Job::Record) => start_record(&mut state),
            Ok(Job::StopCheck) => stop_check(&mut state, true),
            Ok(Job::PlayCheck) => play_check(&mut state),
            Ok(Job::CheckUpdates) => {
                let view = updater::check(true);
                state.last_check = Some(Instant::now());
                if let Ok(mut guard) = snap.lock() {
                    guard.update = view;
                }
            }
            Ok(Job::InstallUpdate) => {
                if let Err(err) = updater::detach() {
                    state.message = err;
                }
            }
            Ok(Job::OpenInstaller(key)) => {
                if let Err(err) = state.apps.launch(&key) {
                    state.message = format!("{err:?}");
                }
            }
            Ok(Job::Refresh) | Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        while let Ok(extra) = rx.try_recv() {
            if matches!(extra, Job::Quit) {
                return;
            }
            // The timeout arm already handled one job. Extra ones are applied
            // the same way, except Quit which stops the thread.
            match extra {
                Job::Apply(config) => apply_config(&mut state, config),
                Job::Autostart(on) => set_autostart(on),
                Job::Record => start_record(&mut state),
                Job::StopCheck => stop_check(&mut state, true),
                Job::PlayCheck => play_check(&mut state),
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
        }
        state.apps.tick(state.config.check_updates);
        let check_due = state.config.check_updates
            && state.last_check.is_none_or(|start| start.elapsed() >= Duration::from_secs(3600));
        if check_due {
            let _ = updater::check(false);
            state.last_check = Some(Instant::now());
        }
        tick_check(&mut state);
        sync_audio(&mut state);
        publish(&snap, &state);
    }
    stop_check(&mut state, false);
    if state.config.mix_on {
        // Leaving the process does not turn Mix off. The graph unit keeps running.
    }
}

fn publish(snap: &Mutex<Snapshot>, state: &State) {
    let graph = dump_graph();
    let mut guard = match snap.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    *guard = Snapshot {
        graph,
        mix_active: unit_active("frame-audio-mixer-graph.service"),
        echo_cancel: echo_cancel(),
        autostart: unit_enabled("frame-audio-mixer.service"),
        tuner_version: tuner_version(),
        update: updater::current_view(),
        check: state.check_phase,
        check_secs: state.check_started.map(|start| start.elapsed().as_secs_f64()).unwrap_or(0.0),
        message: state.message.clone(),
        apps: state.apps.entries().to_vec(),
        menu_busy: state.apps.menu_busy(),
    };
}

fn apply_config(state: &mut State, config: Config) {
    let was = state.config.mix_on;
    let old_mic = state.config.mic_source.clone();
    let old_ids: Vec<String> = state.config.channels.iter().map(|c| c.id.clone()).collect();
    state.config = config;
    if state.config.mic_source != old_mic {
        save_setting("frame-audio-mixer.mic-source", &state.config.mic_source);
    }
    write_names(&state.config.channels);
    if state.config.mix_on && !was {
        for channel in &state.config.channels {
            let _ = Command::new("systemctl")
                .args(["--user", "enable", &format!("frame-audio-mixer-channel@{}.service", channel.id)])
                .status();
        }
        let _ = Command::new("systemctl").args(["--user", "enable", "--now", "frame-audio-mixer-graph.service"]).status();
        save_setting("frame-audio-mixer.enabled", "true");
        save_setting("frame-audio-mixer.vrlink-gain", &format!("{:.3}", state.config.vrlink_gain));
    } else if !state.config.mix_on && was {
        save_setting("frame-audio-mixer.enabled", "false");
        restore_vrlink();
        let _ = Command::new("systemctl").args(["--user", "disable", "--now", "frame-audio-mixer-graph.service"]).status();
        clear_pins(state);
        stop_check(state, false);
    } else if state.config.mix_on {
        for channel in &state.config.channels {
            if !old_ids.contains(&channel.id) {
                let _ = Command::new("systemctl")
                    .args(["--user", "enable", "--now", &format!("frame-audio-mixer-channel@{}.service", channel.id)])
                    .status();
            }
        }
        for id in &old_ids {
            if !state.config.channels.iter().any(|c| &c.id == id) && is_channel_id(id) {
                let _ = Command::new("systemctl")
                    .args(["--user", "disable", "--now", &format!("frame-audio-mixer-channel@{id}.service")])
                    .status();
                let _ = fs::remove_file(name_file(id));
            }
        }
    } else {
        for id in &old_ids {
            if !state.config.channels.iter().any(|c| &c.id == id) && is_channel_id(id) {
                let _ = fs::remove_file(name_file(id));
            }
        }
    }
}

fn sync_audio(state: &mut State) {
    let graph = dump_graph();
    if !state.config.mix_on {
        if !state.pinned.is_empty() {
            clear_pins(state);
        }
        restore_vrlink();
        return;
    }
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
    if !state.config.mix_on {
        state.message = "mix off".to_string();
        return;
    }
    stop_check(state, false);
    let path = check_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let child = Command::new("pw-record")
        .args(["--target", "frame_amix_source", "--rate", "48000", "--channels", "2", &path.to_string_lossy()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    match child {
        Ok(child) => {
            state.check = Some(child);
            state.check_phase = CheckPhase::Recording;
            state.check_started = Some(Instant::now());
            state.message.clear();
        }
        Err(err) => state.message = err.to_string(),
    }
}

fn stop_check(state: &mut State, keep: bool) {
    if let Some(mut child) = state.check.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    state.check_started = None;
    if keep && check_path().exists() {
        state.check_phase = CheckPhase::Ready;
    } else if !keep {
        state.check_phase = CheckPhase::Idle;
    }
}

fn play_check(state: &mut State) {
    if state.check_phase == CheckPhase::Recording {
        stop_check(state, true);
    }
    let path = check_path();
    if !path.exists() {
        return;
    }
    stop_check(state, true);
    let child = Command::new("pw-play").arg(&path).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    match child {
        Ok(child) => {
            state.check = Some(child);
            state.check_phase = CheckPhase::Playing;
            state.check_started = Some(Instant::now());
        }
        Err(err) => state.message = err.to_string(),
    }
}

fn tick_check(state: &mut State) {
    let finished = state.check.as_mut().is_some_and(|child| child.try_wait().ok().flatten().is_some());
    let timed_out = state.check_phase == CheckPhase::Recording
        && state.check_started.is_some_and(|start| start.elapsed() >= Duration::from_secs(10));
    if timed_out {
        stop_check(state, true);
    } else if finished {
        let phase = state.check_phase;
        state.check = None;
        state.check_started = None;
        state.check_phase = if phase == CheckPhase::Recording { CheckPhase::Ready } else { CheckPhase::Idle };
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

fn check_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".local/share/frame-audio-mixer/mix-check.wav")
}

/// Used by the self-test to prove a picked app is remembered by serial, not re-sent.
pub fn pin_once(seen: &mut HashMap<u64, u32>, stream: &AudioNode) -> bool {
    if seen.contains_key(&stream.serial) {
        return false;
    }
    seen.insert(stream.serial, stream.id);
    true
}
