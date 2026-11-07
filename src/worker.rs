//! systemctl, wpctl and pw-metadata run here, off the UI thread.
//! Nothing in this module calls `wpctl set-default`. The Microphone fader
//! writes only `frame_amix_mic_virtual_sink`.

use crate::config::{self, Channel, Config, is_channel_id};
use crate::fx::{Engine, InstanceKey};
use crate::graph::{AudioNode, Graph, parse_dump};
use crate::apps::{self, Entry};
use crate::updater::{self, UpdateView};
use std::collections::{HashMap, HashSet};
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
    /// Bumps when a Mix Check attempt finishes without staying in Recording.
    pub check_epoch: u64,
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
            check_epoch: 0,
            apps: Vec::new(),
            menu_busy: false,
        }
    }
}

pub enum Job {
    Apply(Config),
    /// Fader volumes during a drag. Applied on this worker turn, nothing else changes.
    Levels(Config),
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
    /// VRLink capture id to the Frame Mix serial written for it.
    vrlink_pins: HashMap<u32, u64>,
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
    /// Immediate exits of `pw-record` retry once. A second miss is reported.
    record_tries: u8,
    check_epoch: u64,
    booted: bool,
    engine: Engine,
    /// Playback node name to (node id, target serial). `None` means the pin was deleted.
    fx_pins: HashMap<String, (u32, Option<u64>)>,
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
        vrlink_pins: HashMap::new(),
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
        record_tries: 0,
        check_epoch: 0,
        booted: false,
        engine: Engine::new(),
        fx_pins: HashMap::new(),
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
            // The button and the clip list update before the next pw-dump.
            publish_check(&snap, &state);
        }
        if dragging {
            // A drag must not wait behind pw-dump or the update check.
            sync_levels(&mut state);
            continue;
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
    // Effect processing lives in this process. Put sends back on the dry path so Mix
    // still reaches the PC after the panel quits. The graph unit itself stays up.
    stop_fx(&mut state);
}

/// Record, stop, and play return true so the panel can show that phase before `pw-dump`.
fn apply_job(state: &mut State, snap: &Mutex<Snapshot>, job: Job) -> bool {
    match job {
        Job::Apply(config) => apply_config(state, config),
        Job::Levels(config) => take_levels(state, &config),
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

fn check_fields(state: &State) -> (CheckPhase, f64, u64, Vec<MixClip>, String, u64) {
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
    (state.check_phase, secs, state.playing_id, clips, state.message.clone(), state.check_epoch)
}

fn publish_check(snap: &Mutex<Snapshot>, state: &State) {
    let Ok(mut guard) = snap.lock() else { return };
    let (phase, secs, playing, clips, message, epoch) = check_fields(state);
    guard.check = phase;
    guard.check_secs = secs;
    guard.playing_id = playing;
    guard.clips = clips;
    guard.message = message;
    guard.check_epoch = epoch;
}

fn publish(snap: &Mutex<Snapshot>, state: &State) {
    let mut guard = match snap.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    let (phase, secs, playing, clips, message, epoch) = check_fields(state);
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
        check_epoch: epoch,
        apps: state.apps.entries().to_vec(),
        menu_busy: state.apps.menu_busy(),
    };
}

fn apply_config(state: &mut State, config: Config) {
    let was = state.config.mix_on;
    let old_accent = state.config.accent.clone();
    let old_ids: Vec<String> = state.config.channels.iter().map(|c| c.id.clone()).collect();
    let old_names: Vec<(String, String)> = state.config.channels.iter().map(|c| (c.id.clone(), c.name.clone())).collect();
    state.config = config;
    if state.config.accent != old_accent {
        install_icons(&state.config.accent);
    }
    write_names(&state.config.channels);
    let renamed = restart_renamed(&old_names, &state.config.channels);
    let ids_now: Vec<String> = state.config.channels.iter().map(|channel| channel.id.clone()).collect();
    let first = !state.booted;
    state.booted = true;
    if !first && ids_now == old_ids && state.config.mix_on == was {
        if renamed {
            state.facts_at = None;
        }
        return;
    }
    // At start the saved config already says Mix is on, so there is no off-to-on edge.
    // WirePlumber's copy of the switch is written anyway.
    if state.config.mix_on && (first || !was) {
        save_setting("frame-audio-mixer.enabled", "true");
    } else if !state.config.mix_on && (first || was) {
        save_setting("frame-audio-mixer.enabled", "false");
        restore_vrlink();
        clear_pins(state);
        unpin_vrlink(state);
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

/// Queued drag updates collapse: each one only copies values, and `sync_audio` runs
/// `wpctl` once at the end of the turn for whatever changed.
fn take_levels(state: &mut State, config: &Config) {
    state.config.mic_volume = config.mic_volume;
    state.config.fx_a = config.fx_a.clone();
    state.config.fx_b = config.fx_b.clone();
    for channel in &mut state.config.channels {
        if let Some(new) = config.channels.iter().find(|item| item.id == channel.id) {
            channel.to_mic = new.to_mic;
            channel.phones = new.phones;
        }
    }
    let fx_a = state.config.fx_a.clone();
    let fx_b = state.config.fx_b.clone();
    state.engine.push_params(&fx_a, &fx_b);
}

fn sync_audio(state: &mut State) {
    sync_levels(state);
    let graph = state.graph.clone();
    pin_targets(state, &graph);
    if !state.config.mix_on {
        if !state.pinned.is_empty() {
            clear_pins(state);
        }
        unpin_vrlink(state);
        restore_vrlink();
        stop_fx(state);
        return;
    }
    pin_apps(state, &graph);
    pin_vrlink(state, &graph);
    touch_vrlink(state, &graph);
    sync_fx(state);
}

fn solo_mute(saved: bool, this_solo: bool, any_solo: bool) -> bool {
    if any_solo { saved || !this_solo } else { saved }
}

/// Dry copies of the mix bus and the four effect sinks. `node.mute` on the
/// loopback command does not stick, and with no target these streams follow
/// the speakers. That is the mic the user hears after arming an effect, and
/// the loop that howls.
fn mute_discards(state: &mut State, graph: &Graph) {
    set_mute(state, "frame_amix_bus_virtual_sink_discard", true, graph);
    for key in InstanceKey::ALL {
        let name = format!("{}_virtual_sink_discard", key.sink());
        set_mute(state, &name, true, graph);
    }
}

fn sync_levels(state: &mut State) {
    let graph = state.graph.clone();
    let pc_any = state.config.mic_solo || state.config.channels.iter().any(|channel| channel.to_mic_solo);
    let ph_any = state.config.channels.iter().any(|channel| channel.phones_solo);
    apply_level(
        state,
        "frame_amix_mic_virtual_sink",
        config::mic_pipewire_volume(state.config.mic_volume),
        &graph,
    );
    // Loopback playback follows the speakers unless it is muted. The bus copy
    // and each effect sink's dry copy are muted, so a send is not monitored
    // locally. The wet signal leaves through the effect return onto the bus.
    mute_discards(state, &graph);
    // Reviewing a clip plays on the speakers. Hold the mic out of the mix so
    // that playback cannot re-enter the effect returns and howl.
    let reviewing = state.check_phase == CheckPhase::Playing;
    set_mute(
        state,
        "frame_amix_mic_virtual_sink",
        reviewing || solo_mute(state.config.mic_muted, state.config.mic_solo, pc_any),
        &graph,
    );
    // A headset effect return is the local monitor for a channel. Mute it while
    // Mix Check records or plays. PC returns stay on the bus, which is the take.
    let hold_phones = matches!(state.check_phase, CheckPhase::Recording | CheckPhase::Playing);
    set_mute(state, "frame_amix_fx_a_phones_virtual_sink_return", hold_phones, &graph);
    set_mute(state, "frame_amix_fx_b_phones_virtual_sink_return", hold_phones, &graph);
    let channel_levels: Vec<(String, f64, f64, bool, bool)> = state
        .config
        .channels
        .iter()
        .map(|channel| {
            (
                channel.id.clone(),
                channel.phones,
                channel.to_mic,
                solo_mute(channel.phones_muted, channel.phones_solo, ph_any),
                solo_mute(channel.to_mic_muted, channel.to_mic_solo, pc_any),
            )
        })
        .collect();
    for (id, phones, to_mic, phones_muted, to_mic_muted) in channel_levels {
        apply_level(state, &format!("frame_amix_{id}_virtual_sink_playback"), phones, &graph);
        apply_level(state, &format!("frame_amix_{id}_virtual_sink_to_bus"), to_mic, &graph);
        set_mute(state, &format!("frame_amix_{id}_virtual_sink_playback"), phones_muted, &graph);
        set_mute(state, &format!("frame_amix_{id}_virtual_sink_to_bus"), to_mic_muted, &graph);
    }
}

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

fn apply_level(state: &mut State, name: &str, volume: f64, graph: &Graph) {
    let Some(node) = graph.node_by_name(name) else { return };
    // The mic fader is a linear gain (unity at the +12 dB trim). wpctl's number
    // is cubic, so 4.0 linear is 1.587 here and channelVolumes stays near 4.
    // Channel faders are already on wpctl's cubic scale and are passed through.
    let volume = if name == "frame_amix_mic_virtual_sink" { config::wpctl_cubic(volume) } else { volume };
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

fn fx_unit(id: &str) -> String {
    format!("frame-audio-mixer-fx@{id}.service")
}

fn fx_needed(config: &Config) -> HashSet<InstanceKey> {
    let mut needed = HashSet::new();
    if config.mic_fx_a || config.channels.iter().any(|channel| channel.to_mic_fx_a) {
        needed.insert(InstanceKey::APc);
    }
    if config.mic_fx_b || config.channels.iter().any(|channel| channel.to_mic_fx_b) {
        needed.insert(InstanceKey::BPc);
    }
    if config.channels.iter().any(|channel| channel.phones_fx_a) {
        needed.insert(InstanceKey::APhones);
    }
    if config.channels.iter().any(|channel| channel.phones_fx_b) {
        needed.insert(InstanceKey::BPhones);
    }
    needed
}

fn fx_play_targets(config: &Config, graph: &Graph) -> HashMap<InstanceKey, String> {
    let mut wanted = HashMap::new();
    let Some(bus) = graph.node_by_name("frame_amix_bus").filter(|node| node.serial != 0) else {
        return wanted;
    };
    let bus_serial = bus.serial.to_string();
    let phones = if graph.default_sink.is_empty() {
        None
    } else {
        let target = graph.link_target(&graph.default_sink);
        (!target.is_empty()).then_some(target)
    };
    let needed = fx_needed(config);
    let pc_both = (config.mic_fx_a && config.mic_fx_b) || config.channels.iter().any(|channel| channel.to_mic_fx_a && channel.to_mic_fx_b);
    let ph_both = config.channels.iter().any(|channel| channel.phones_fx_a && channel.phones_fx_b);
    if needed.contains(&InstanceKey::APc) && graph.node_by_name(&InstanceKey::APc.sink()).is_some() {
        let dest = if pc_both {
            graph
                .node_by_name(&InstanceKey::BPc.sink())
                .filter(|node| node.serial != 0)
                .map(|node| node.serial.to_string())
                .unwrap_or_else(|| bus_serial.clone())
        } else {
            bus_serial.clone()
        };
        wanted.insert(InstanceKey::APc, dest);
    }
    if needed.contains(&InstanceKey::BPc) && graph.node_by_name(&InstanceKey::BPc.sink()).is_some() {
        wanted.insert(InstanceKey::BPc, bus_serial);
    }
    if let Some(phones) = phones {
        if needed.contains(&InstanceKey::APhones) && graph.node_by_name(&InstanceKey::APhones.sink()).is_some() {
            let dest = if ph_both {
                graph
                    .node_by_name(&InstanceKey::BPhones.sink())
                    .filter(|node| node.serial != 0)
                    .map(|node| node.serial.to_string())
                    .unwrap_or_else(|| phones.clone())
            } else {
                phones.clone()
            };
            wanted.insert(InstanceKey::APhones, dest);
        }
        if needed.contains(&InstanceKey::BPhones) && graph.node_by_name(&InstanceKey::BPhones.sink()).is_some() {
            wanted.insert(InstanceKey::BPhones, phones);
        }
    }
    wanted
}

fn sync_fx(state: &mut State) {
    let needed = fx_needed(&state.config);
    let mut started = false;
    for key in InstanceKey::ALL {
        let unit = fx_unit(key.id());
        let want = needed.contains(&key);
        let active = unit_active(&unit);
        if want && !active {
            let _ = Command::new("systemctl").args(["--user", "enable", "--now", &unit]).status();
            started = true;
        } else if !want && active {
            let _ = Command::new("systemctl").args(["--user", "disable", "--now", &unit]).status();
            let _ = Command::new("systemctl").args(["--user", "reset-failed", &unit]).status();
        }
    }
    if started {
        refresh_facts(state, true);
        // The sink can register just after this dump. Look again next turn.
        state.facts_at = None;
    }
    let graph = state.graph.clone();
    let wanted = fx_play_targets(&state.config, &graph);
    let fx_a = state.config.fx_a.clone();
    let fx_b = state.config.fx_b.clone();
    state.engine.sync(&graph, &fx_a, &fx_b, &wanted);
    pin_fx_sends(state, &graph);
    let graph = state.graph.clone();
    mute_discards(state, &graph);
}

fn stop_fx(state: &mut State) {
    let empty = HashMap::new();
    let graph = state.graph.clone();
    let fx_a = state.config.fx_a.clone();
    let fx_b = state.config.fx_b.clone();
    state.engine.sync(&graph, &fx_a, &fx_b, &empty);
    if !state.fx_pins.is_empty() {
        restore_fx_pins(state);
    }
    for key in InstanceKey::ALL {
        let unit = fx_unit(key.id());
        if unit_active(&unit) {
            let _ = Command::new("systemctl").args(["--user", "disable", "--now", &unit]).status();
            let _ = Command::new("systemctl").args(["--user", "reset-failed", &unit]).status();
        }
    }
}

fn pin_fx_sends(state: &mut State, graph: &Graph) {
    let bus = graph.node_by_name("frame_amix_bus").map(|node| node.serial).filter(|serial| *serial != 0);
    pin_fx_send(state, graph, "frame_amix_mic_virtual_sink", bus, false, state.config.mic_fx_a, state.config.mic_fx_b);
    let routes: Vec<(String, bool, bool, bool, bool)> = state
        .config
        .channels
        .iter()
        .map(|channel| {
            (
                channel.id.clone(),
                channel.to_mic_fx_a,
                channel.to_mic_fx_b,
                channel.phones_fx_a,
                channel.phones_fx_b,
            )
        })
        .collect();
    for (id, a, b, pa, pb) in routes {
        pin_fx_send(state, graph, &format!("frame_amix_{id}_virtual_sink_to_bus"), bus, false, a, b);
        pin_fx_send(state, graph, &format!("frame_amix_{id}_virtual_sink_playback"), bus, true, pa, pb);
    }
}

fn pin_fx_send(state: &mut State, graph: &Graph, name: &str, bus: Option<u64>, phones: bool, a: bool, b: bool) {
    if !a && !b && !state.fx_pins.contains_key(name) {
        return;
    }
    set_fx_pin(state, graph, name, fx_send_serial(graph, bus, phones, a, b));
}

fn fx_send_serial(graph: &Graph, bus: Option<u64>, phones: bool, a: bool, b: bool) -> Option<u64> {
    if a || b {
        let key = match (phones, a) {
            (false, true) => InstanceKey::APc,
            (false, false) => InstanceKey::BPc,
            (true, true) => InstanceKey::APhones,
            (true, false) => InstanceKey::BPhones,
        };
        return graph.node_by_name(&key.sink()).map(|node| node.serial).filter(|serial| *serial != 0);
    }
    if phones { None } else { bus }
}

fn set_fx_pin(state: &mut State, graph: &Graph, name: &str, serial: Option<u64>) {
    let Some(node) = graph.node_by_name(name) else { return };
    if state.fx_pins.get(name) == Some(&(node.id, serial)) {
        return;
    }
    let current = graph.targets.get(&node.id);
    match serial {
        Some(value) => {
            let wanted = value.to_string();
            if current != Some(&wanted) {
                let _ = Command::new("pw-metadata")
                    .args(["-n", "default", &node.id.to_string(), "target.object", &wanted, "Spa:Id"])
                    .status();
            }
        }
        None => {
            if current.is_some() {
                let _ = Command::new("pw-metadata")
                    .args(["-n", "default", "-d", &node.id.to_string(), "target.object"])
                    .status();
                state.graph.targets.remove(&node.id);
            }
        }
    }
    state.fx_pins.insert(name.to_string(), (node.id, serial));
}

fn restore_fx_pins(state: &mut State) {
    let graph = state.graph.clone();
    let bus = graph.node_by_name("frame_amix_bus").map(|node| node.serial).filter(|serial| *serial != 0);
    let names: Vec<String> = state.fx_pins.keys().cloned().collect();
    for name in names {
        let phones = name.ends_with("_virtual_sink_playback");
        let serial = if phones { None } else { bus };
        set_fx_pin(state, &graph, &name, serial);
    }
    state.fx_pins.clear();
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
            .args(["set-volume", &node.id.to_string(), &format!("{:.3}", config::wpctl_cubic(state.config.vrlink_gain))])
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

/// `pw-cat --latency` is a sample count on this PipeWire. A fraction is rejected
/// ("bad unit"). 480 samples is the speaker period, 10 ms at 48 kHz.
const GRAPH_LATENCY: &str = "480";
const TAKE_MISSED: &str = "Mix Check could not open Frame Mix.";

fn bump_check(state: &mut State) {
    state.check_epoch = state.check_epoch.wrapping_add(1);
}

/// Object serial of Frame Mix. A node name is not a target on this headset.
fn mix_serial(state: &State) -> Option<String> {
    let node = state.graph.node_by_name("frame_amix_source")?;
    (node.serial != 0).then(|| node.serial.to_string())
}

fn start_record(state: &mut State) {
    if state.check_phase == CheckPhase::Recording {
        finish_recording(state);
        return;
    }
    begin_record(state, true);
}

fn begin_record(state: &mut State, reset_tries: bool) {
    stop_child(state);
    state.playing_id = 0;
    state.check_phase = CheckPhase::Idle;
    state.check_started = None;
    if reset_tries {
        state.record_tries = 0;
    }
    let path = check_dir().join(format!("mix-{}.wav", state.next_clip + 1));
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    // The first try uses the serial already in hand. A retry always re-reads it,
    // because an immediate exit means that serial was stale.
    if !reset_tries || mix_serial(state).is_none() {
        refresh_facts(state, true);
    }
    let Some(target) = mix_serial(state) else {
        if let Some(path) = state.record_path.take() {
            let _ = fs::remove_file(path);
        }
        state.message = TAKE_MISSED.into();
        bump_check(state);
        return;
    };
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
        Err(err) => {
            state.record_path = None;
            state.message = err.to_string();
            bump_check(state);
        }
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
        state.message = TAKE_MISSED.into();
        bump_check(state);
        return;
    }
    state.message.clear();
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
    let sink = state.graph.link_target(&state.graph.default_sink);
    let child = Command::new("pw-play")
        .args(check_play_args(&sink, &path.to_string_lossy()))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
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
        let _ = Command::new("kill").args(["-INT", &child.id().to_string()]).status();
        let _ = child.wait();
    }
}

fn tick_check(state: &mut State) {
    let finished = state.check.as_mut().is_some_and(|child| child.try_wait().ok().flatten().is_some());
    let timed_out = state.check_phase == CheckPhase::Recording
        && state.check_started.is_some_and(|start| start.elapsed() >= Duration::from_secs(10));
    if state.check_phase == CheckPhase::Recording && finished && !timed_out && state.record_tries == 0 {
        // Polling can be late (a fact refresh sits in front of it). Judge the take
        // by the file, not by how long the button has been down.
        let short = state.record_path.as_ref().is_none_or(|path| wav_peaks(path, 1).0 < 0.2);
        if short {
            state.record_tries = 1;
            begin_record(state, false);
            let died = state.check.as_mut().is_some_and(|child| child.try_wait().ok().flatten().is_some());
            if state.check_phase != CheckPhase::Recording || died {
                finish_recording(state);
            }
            return;
        }
    }
    if state.check_phase == CheckPhase::Recording && (timed_out || finished) {
        finish_recording(state);
    } else if state.check_phase == CheckPhase::Playing && finished {
        state.check = None;
        state.check_started = None;
        state.playing_id = 0;
        state.check_phase = CheckPhase::Idle;
    }
}

/// Play a finished clip on the headset speakers only. The name contains
/// `virtual` and `sink`, so the spatializer does not wrap it, and the target
/// is the current output rather than the mix bus.
pub fn check_play_args(target: &str, path: &str) -> Vec<String> {
    let mut args = Vec::new();
    if !target.is_empty() {
        args.extend([
            "--target".into(),
            target.into(),
            "--rate".into(),
            "48000".into(),
            "--latency".into(),
            GRAPH_LATENCY.into(),
            "-P".into(),
            "{ node.name = frame_amix_check_virtual_sink_play node.virtual = true node.dont-fallback = true }".into(),
        ]);
    }
    args.push(path.into());
    args
}

/// Stereo float32 at 48 kHz, the same format and quantum as the mix graph.
/// 16-bit clipped the +12 dB mic trim, and 20 ms moved the quantum (the short crackle).
/// `--target` follows an object serial on this headset. `node.dont-fallback` keeps a stale
/// serial from silently recording the default microphone. The stream stays non-virtual,
/// like VRLink, so Valve's mic filters run while recording, as they do while streaming.
pub fn check_record_args(target: &str, path: &str) -> Vec<String> {
    vec![
        "--target".into(),
        target.into(),
        "--rate".into(),
        "48000".into(),
        "--channels".into(),
        "2".into(),
        "--format".into(),
        "f32".into(),
        "--latency".into(),
        GRAPH_LATENCY.into(),
        "-P".into(),
        "{ node.dont-fallback=true }".into(),
        path.into(),
    ]
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
    let args = check_record_args("42", "/tmp/mix.wav");
    let joined = args.join(" ");
    if !joined.contains("--target 42") || !joined.contains("--rate 48000") || !joined.contains("--channels 2") || !joined.contains("--format f32") || !joined.contains("--latency 480") || joined.contains("s16") || joined.contains("512/48000") || !joined.contains("node.dont-fallback=true") {
        return Err(joined);
    }
    if args.last().map(String::as_str) != Some("/tmp/mix.wav") {
        return Err("record path last".into());
    }
    let play = check_play_args("77", "/tmp/mix.wav").join(" ");
    if !play.contains("--target 77") || !play.contains("--rate 48000") || !play.contains("--latency 480") || play.contains("512/48000") || !play.contains("frame_amix_check_virtual_sink_play") || !play.contains("node.dont-fallback = true") || !play.ends_with("/tmp/mix.wav") {
        return Err(play);
    }
    if check_play_args("", "/tmp/mix.wav") != vec!["/tmp/mix.wav".to_string()] {
        return Err("play without sink".into());
    }
    if format!("{}_virtual_sink_discard", InstanceKey::APc.sink()) != "frame_amix_fx_a_pc_virtual_sink_discard" {
        return Err("fx discard".into());
    }
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
fn restart_renamed(old: &[(String, String)], channels: &[Channel]) -> bool {
    let mut any = false;
    for channel in channels {
        if !is_channel_id(&channel.id) {
            continue;
        }
        let changed = old.iter().any(|(id, name)| id == &channel.id && name != &channel.name);
        if !changed {
            continue;
        }
        let unit = format!("frame-audio-mixer-channel@{}.service", channel.id);
        let _ = Command::new("systemctl").args(["--user", "restart", &unit]).status();
        any = true;
    }
    any
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
