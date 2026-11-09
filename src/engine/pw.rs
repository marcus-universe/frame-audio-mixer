//! In-process PipeWire graph. Loopback modules are the devices apps can see.
//! Gain, effects and meters run in realtime filter nodes. Routes are ordinary links.

use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use libspa::pod::Pod;
use pipewire::context::ContextRc;
use pipewire::core::CoreRc;
use pipewire::link::Link;
use pipewire::main_loop::MainLoopRc;
use pipewire::metadata::Metadata;
use pipewire::node::Node;
use pipewire::properties::properties;
use pipewire::proxy::Listener as ProxyListener;
use pipewire::types::ObjectType;

use crate::config::{self, Config, EffectSlot};
use crate::engine::bluetooth;
use crate::engine::filter::{Dsp, Filter, Ports};
use crate::engine::link::{self, Direction, Port, Wanted};
use crate::fx::analyzer::{Analyzer, Scope};
use crate::fx::dsp::{soft_ceiling, Processor};
use crate::graph::{self, AppRef, AudioNode, Graph};

const CHECK_MAX: usize = 48_000 * 10 * 2;

pub fn socket_path() -> std::path::PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    std::path::PathBuf::from(runtime).join("frame-audio-mixer").join("engine.sock")
}

pub fn serve(spike: bool) -> Result<(), String> {
    let _lock = if spike { None } else { Some(lock_instance()?) };
    unsafe {
        libc::signal(libc::SIGTERM, on_stop as *const () as libc::sighandler_t);
        libc::signal(libc::SIGINT, on_stop as *const () as libc::sighandler_t);
    }
    pipewire::init();
    let mainloop = MainLoopRc::new(None).map_err(|err| err.to_string())?;
    let context = ContextRc::new(&mainloop, None).map_err(|err| err.to_string())?;
    let core = context.connect_rc(None).map_err(|err| err.to_string())?;
    let registry = core.get_registry_rc().map_err(|err| err.to_string())?;
    let engine = Rc::new(std::cell::RefCell::new(Engine::new(context, core.clone())));

    let registry_weak = registry.downgrade();
    let engine_add = engine.clone();
    let engine_rm = engine.clone();
    let _registry_listener = registry
        .add_listener_local()
        .global(move |global| {
            let Some(props) = global.props else { return };
            let engine_md = engine_add.clone();
            let mut engine = engine_add.borrow_mut();
            match global.type_ {
                ObjectType::Node => {
                    if let Some(node) = read_node(global.id, props) {
                        let name = node.name.clone();
                        let device = is_device_sink(&node);
                        if name.starts_with("bluez_output") {
                            if let Some(device) = props.get("device.id").and_then(|id| id.parse().ok()) {
                                engine.bt_sinks.insert(global.id, bluetooth::BtSink { device });
                                engine.publish_bt();
                            }
                        }
                        if node.media_class == "Stream/Input/Audio" && !node.is_virtual {
                            unity_vrlink(node.clone());
                        }
                        engine.model.nodes.insert(global.id, node);
                        let want = name.starts_with("frame_amix_") && (name.ends_with("_virtual_sink") || engine.pending_volume.contains_key(&name));
                        if want && !engine.bound.contains_key(&name) {
                            if let Some(registry) = registry_weak.upgrade() {
                                if let Ok(proxy) = registry.bind::<Node, _>(global) {
                                    engine.apply_volume(&name, &proxy);
                                    engine.bound.insert(name, proxy);
                                }
                            }
                        }
                        // A Bluetooth or USB device came up: its output starts playing at once.
                        if device {
                            engine.sync_routes();
                        }
                    }
                }
                ObjectType::Port => {
                    if let Some(port) = read_port(global.id, props) {
                        engine.model.ports.insert(global.id, port);
                        engine.reconcile();
                    }
                }
                ObjectType::Link => {
                    if let Some((id, ports)) = read_link(global.id, props) {
                        engine.model.links.insert(id, ports);
                        if let Some(held) = engine.held_links.get_mut(&ports) {
                            held.confirmed = true;
                        }
                    }
                }
                ObjectType::Client => {
                    let name = props.get("application.name").unwrap_or("").to_string();
                    let binary = props.get("application.process.binary").unwrap_or("").to_string();
                    if !name.is_empty() || !binary.is_empty() {
                        engine.model.clients.push(AppRef { name, binary });
                    }
                }
                ObjectType::Metadata
                    if props.get("metadata.name") == Some("default") && engine.metadata.is_none() => {
                        let Some(registry) = registry_weak.upgrade() else { return };
                        if let Ok(md) = registry.bind::<Metadata, _>(global) {
                            let listener = md
                                .add_listener_local()
                                .property(move |_subject, key, _type, value| {
                                    let mut engine = engine_md.borrow_mut();
                                    let name = value.and_then(parse_default_name);
                                    match key {
                                        Some("default.audio.sink") => {
                                            if let Some(name) = name {
                                                if name != engine.model.default_sink {
                                                    engine.model.default_sink = name;
                                                    engine.sync_routes();
                                                }
                                            }
                                        }
                                        Some("default.audio.source") => {
                                            if let Some(name) = name {
                                                let moved = name != engine.model.default_source;
                                                engine.model.default_source = name;
                                                if moved && engine.mic.is_some() && engine.config.mic_source.is_empty() {
                                                    let config = engine.config.clone();
                                                    engine.apply_config(&config);
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                    0
                                })
                                .register();
                            engine.metadata = Some((md, Box::new(listener)));
                        }
                    }
                _ => {}
            }
        })
        .global_remove(move |id| {
            let mut engine = engine_rm.borrow_mut();
            let mut device_gone = false;
            if let Some(node) = engine.model.nodes.remove(&id) {
                device_gone = is_device_sink(&node);
                engine.bound.remove(&node.name);
            }
            if engine.bt_sinks.remove(&id).is_some() {
                engine.publish_bt();
            }
            engine.model.ports.remove(&id);
            // The server dropped a link this engine still wants (a session manager
            // cleanup, or a node restart while the ports survived). Recreate it;
            // nothing else would, and the route stays silent until then.
            if let Some(ports) = engine.model.links.remove(&id) {
                let still_linked = engine.model.links.values().any(|other| *other == ports);
                if !still_linked && engine.held_links.remove(&ports).is_some() {
                    engine.reconcile();
                }
            }
            let live: HashSet<u32> = engine.model.ports.keys().copied().collect();
            engine.held_links.retain(|ports, _| live.contains(&ports.0) && live.contains(&ports.1));
            if device_gone {
                engine.sync_routes();
            }
        })
        .register();

    let engine_done = engine.clone();
    let _core_listener = core
        .add_listener_local()
        .done(move |_id, _seq| {
            engine_done.borrow_mut().ready = true;
        })
        .register();
    let _ = core.sync(0).map_err(|err| err.to_string())?;

    let (tx, rx) = pipewire::channel::channel::<Request>();
    if !spike {
        std::thread::spawn(move || accept_loop(tx));
    }

    let engine_cmd = engine.clone();
    let mainloop_cmd = mainloop.clone();
    let _attached = rx.attach(mainloop.loop_(), move |request| {
        let reply = engine_cmd.borrow_mut().handle(&request.body);
        let _ = request.reply.send(reply);
        if engine_cmd.borrow().quit {
            mainloop_cmd.quit();
        }
    });

    let engine_tick = engine.clone();
    let mainloop_tick = mainloop.clone();
    let timer = mainloop.loop_().add_timer(move |_| {
        if STOP.load(Ordering::Relaxed) {
            mainloop_tick.quit();
            return;
        }
        let mut engine = engine_tick.borrow_mut();
        engine.poll_check();
        engine.heal_links();
        engine.vst_hosts.reap();
        engine.expire_spectrum();
        if !spike {
            // The mix comes back after a reboot without the panel open. Waiting a second
            // lets the default source arrive, so the mic loopback is not built twice.
            if engine.ready && !engine.applied {
                engine.boot_waits = engine.boot_waits.saturating_add(1);
                if engine.boot_waits >= 5 {
                    let config = config::load(&config::config_path());
                    engine.apply_config(&config);
                }
            }
            return;
        }
        if !engine.ready {
            return;
        }
        if !engine.spiked {
            engine.spiked = true;
            engine.apply_config(&spike_config());
            return;
        }
        engine.spike_waits = engine.spike_waits.saturating_add(1);
        if engine.spike_waits < 8 {
            return;
        }
        let names: Vec<_> = engine.model.nodes.values().map(|node| node.name.clone()).collect();
        let ok = names.iter().any(|name| name == "frame_amix_ch1") && names.iter().any(|name| name == "frame_amix_source");
        let wrapped = names.iter().any(|name| name.contains("spatialize") && name.contains("frame_amix_"));
        if ok && !wrapped {
            eprintln!("SPIKE_OK");
        } else {
            eprintln!("SPIKE_FAIL");
            for name in names.iter().filter(|name| name.contains("frame_amix")) {
                eprintln!("node {name}");
            }
        }
        mainloop_tick.quit();
    });
    timer
        .update_timer(Some(Duration::from_millis(200)), Some(Duration::from_millis(200)))
        .into_result()
        .map_err(|err| err.to_string())?;
    std::mem::forget(timer);

    mainloop.run();
    // Every module and filter is owned by this process, so PipeWire removes them all
    // when it exits. Only the WirePlumber setting outlives it.
    engine.borrow_mut().vst_hosts.stop_all();
    engine.borrow().bt.stop();
    Ok(())
}

static STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn on_stop(_: libc::c_int) {
    STOP.store(true, Ordering::Relaxed);
}

/// One engine per user. A second one would build every device twice.
fn lock_instance() -> Result<std::fs::File, String> {
    use std::os::unix::io::AsRawFd;
    let path = socket_path().with_file_name("engine.lock");
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&path).map_err(|err| err.to_string())?;
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("another engine is already running".into());
    }
    Ok(file)
}

/// VRLink is the PC streaming microphone. The PC fader sets its level, so the stream
/// itself stays at unity. WirePlumber restores a stored volume for it (an older
/// build left 8.0 there, +18 dB into the Opus encoder); setting 1.0 also replaces
/// that stored value. The delay lets WirePlumber's restore land first.
/// The registry leaves out `media.name` and the binary, so the stream's own
/// properties decide whether a capture stream is VRLink.
fn unity_vrlink(mut node: AudioNode) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1500));
        let Ok(out) = std::process::Command::new("pw-dump").arg(node.id.to_string()).output() else { return };
        let objects: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap_or_default();
        let Some(props) = objects.first().and_then(|object| object.pointer("/info/props")) else { return };
        let text = |key: &str| props[key].as_str().unwrap_or("").to_string();
        node.media_name = text("media.name");
        node.app_name = text("application.name");
        node.binary = text("application.process.binary");
        if Graph::is_vrlink(&node) {
            let _ = std::process::Command::new("wpctl").args(["set-volume", &node.id.to_string(), "1.0"]).status();
        }
    });
}

fn spike_config() -> Config {
    let mut config = Config::default();
    config.mix_on = true;
    config.channels.push(config::Channel::new("ch1", "DJ"));
    config
}

/// A hardware or Bluetooth sink one of the listen outputs may play on.
fn is_device_sink(node: &AudioNode) -> bool {
    node.media_class == "Audio/Sink" && !node.name.starts_with("frame_amix_")
}

struct Request {
    body: String,
    reply: std::sync::mpsc::Sender<String>,
}

fn accept_loop(tx: pipewire::channel::Sender<Request>) {
    let path = socket_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::remove_file(&path);
    let Ok(listener) = UnixListener::bind(&path) else {
        eprintln!("frame-audio-mixer: engine socket is busy");
        return;
    };
    for conn in listener.incoming().flatten() {
        serve_one(conn, &tx);
    }
}

fn serve_one(conn: UnixStream, tx: &pipewire::channel::Sender<Request>) {
    let _ = conn.set_read_timeout(Some(Duration::from_secs(2)));
    let mut reader = BufReader::new(match conn.try_clone() {
        Ok(cloned) => cloned,
        Err(_) => return,
    });
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let (reply_tx, reply_rx) = std::sync::mpsc::channel();
    if tx.send(Request { body: line, reply: reply_tx }).is_err() {
        return;
    }
    let Ok(reply) = reply_rx.recv_timeout(Duration::from_secs(3)) else { return };
    let mut out = reader.into_inner();
    let _ = out.write_all(reply.as_bytes());
    let _ = out.write_all(b"\n");
}

/// A link this engine created, by (output port, input port).
///
/// `create_object` only returns a proxy; whether the link really came up shows
/// in the registry. `confirmed` is set when the registry announces it. A link
/// that is never announced failed on the server and is recreated by
/// `heal_links`, so a route does not stay silent forever.
struct HeldLink {
    _link: Link,
    since: Instant,
    confirmed: bool,
}

struct Engine {
    strips: HashMap<String, Strip>,
    mic: Option<Strip>,
    buses_a: Vec<Bus>,
    buses_b: Vec<Bus>,
    /// Feeds `frame_amix_source`, which VRLink records for the PC.
    bus_pc: Option<Bus>,
    fx: Vec<Strip>,
    check_live: Option<CheckLive>,
    held_links: HashMap<(u32, u32), HeldLink>,
    bound: HashMap<String, Node>,
    metadata: Option<(Metadata, Box<dyn ProxyListener>)>,
    core: CoreRc,
    context: ContextRc,
    model: Model,
    config: Config,
    pending_volume: HashMap<String, (f32, bool)>,
    check: Check,
    share: Arc<CheckShare>,
    ready: bool,
    spiked: bool,
    spike_waits: u8,
    /// Set by the first config, from the panel or from disk.
    applied: bool,
    boot_waits: u8,
    bt: bluetooth::Guard,
    bt_sinks: HashMap<u32, bluetooth::BtSink>,
    quit: bool,
    held_routes: Vec<Wanted>,
    analyzer: Analyzer,
    vst_hosts: super::vst::Hosts,
    /// Last spectrum request. The capture stops soon after a panel stops asking.
    spectrum_at: Option<Instant>,
}

struct Model {
    nodes: HashMap<u32, AudioNode>,
    ports: HashMap<u32, Port>,
    links: HashMap<u32, (u32, u32)>,
    clients: Vec<AppRef>,
    default_sink: String,
    default_source: String,
}

impl Model {
    fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            ports: HashMap::new(),
            links: HashMap::new(),
            clients: Vec::new(),
            default_sink: String::new(),
            default_source: String::new(),
        }
    }

    /// The default sink, or the first ALSA sink before WirePlumber has published one.
    fn default_sink(&self) -> String {
        if !self.default_sink.is_empty() {
            return self.default_sink.clone();
        }
        self.nodes
            .values()
            .find(|node| node.media_class == "Audio/Sink" && node.name.contains("alsa_output"))
            .map(|node| node.name.clone())
            .unwrap_or_default()
    }
}

/// Loudest left and right sample since the panel last asked, as f32 bits.
/// Non-negative floats order like their bits, so `fetch_max` holds the peak.
struct Peaks {
    left: AtomicU32,
    right: AtomicU32,
}

impl Peaks {
    fn new() -> Arc<Self> {
        Arc::new(Self { left: AtomicU32::new(0), right: AtomicU32::new(0) })
    }

    fn hold(&self, left: &[f32], right: &[f32]) {
        self.left.fetch_max(peak_of(left).to_bits(), Ordering::Relaxed);
        self.right.fetch_max(peak_of(right).to_bits(), Ordering::Relaxed);
    }

    /// The held peak, cleared for the next reader. Only the fast meter feed clears it.
    fn take(&self) -> (f32, f32) {
        (f32::from_bits(self.left.swap(0, Ordering::Relaxed)), f32::from_bits(self.right.swap(0, Ordering::Relaxed)))
    }

    fn peek(&self) -> (f32, f32) {
        (f32::from_bits(self.left.load(Ordering::Relaxed)), f32::from_bits(self.right.load(Ordering::Relaxed)))
    }
}

fn peak_of(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |peak, sample| {
        let level = sample.abs();
        if level > peak && level.is_finite() { level } else { peak }
    })
}

struct Controls {
    gain: Arc<AtomicU32>,
    mute: Arc<AtomicBool>,
    hold: Arc<AtomicBool>,
    chain: Arc<Mutex<Vec<EffectSlot>>>,
    peak: Arc<Peaks>,
    scope: Arc<Scope>,
}

impl Controls {
    fn new() -> Self {
        Self {
            gain: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            mute: Arc::new(AtomicBool::new(false)),
            hold: Arc::new(AtomicBool::new(false)),
            chain: Arc::new(Mutex::new(Vec::new())),
            peak: Peaks::new(),
            scope: Arc::new(Scope::new()),
        }
    }

    fn set_gain_db(&self, db: f64, extra: f64) {
        let linear = (config::db_to_lin(db) * extra) as f32;
        self.gain.store(linear.to_bits(), Ordering::Relaxed);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Channel,
    Mic,
    Fx,
}

struct Strip {
    _modules: Vec<LoadedModule>,
    _filter: Filter<StripDsp>,
    controls: Controls,
    base: String,
    label: String,
    capture: String,
    kind: Kind,
}

impl Strip {
    /// The filter node: its inputs take the loopback, its outputs feed the routes.
    fn dsp(&self) -> String {
        format!("{}_virtual_sink_dsp", self.base)
    }
    fn loop_out(&self) -> String {
        format!("{}_virtual_sink_out", self.base)
    }
    fn sink(&self) -> String {
        match self.kind {
            Kind::Fx => format!("{}_virtual_sink", self.base),
            _ => self.base.clone(),
        }
    }
}

/// Gain, mute, the effect chain and the meter of one strip, in the graph cycle.
struct StripDsp {
    proc: Processor,
    applied: Vec<EffectSlot>,
    gain: f32,
    target: Arc<AtomicU32>,
    mute: Arc<AtomicBool>,
    hold: Arc<AtomicBool>,
    chain: Arc<Mutex<Vec<EffectSlot>>>,
    peak: Arc<Peaks>,
}

impl Dsp for StripDsp {
    fn process(&mut self, input: Option<[&[f32]; 2]>, output: Option<[&mut [f32]; 2]>) {
        let (Some([in_l, in_r]), Some([out_l, out_r])) = (input, output) else { return };
        if let Ok(chain) = self.chain.try_lock() {
            if chain.as_slice() != self.applied.as_slice() {
                self.proc.set_chain(chain.as_slice());
                self.applied.clone_from(&chain);
            }
        }
        out_l.copy_from_slice(in_l);
        out_r.copy_from_slice(in_r);
        self.proc.process_planar(out_l, out_r);
        let muted = self.mute.load(Ordering::Relaxed) || self.hold.load(Ordering::Relaxed);
        let dest = if muted { 0.0 } else { f32::from_bits(self.target.load(Ordering::Relaxed)) };
        // The ceiling is transparent below 0.8. Above it, the +12 dB mic fader rounds off
        // instead of clipping hard in VRLink's encoder.
        for (left, right) in out_l.iter_mut().zip(out_r.iter_mut()) {
            self.gain += 0.02 * (dest - self.gain);
            *left = soft_ceiling(*left * self.gain);
            *right = soft_ceiling(*right * self.gain);
        }
        self.peak.hold(out_l, out_r);
    }
}

struct Bus {
    _module: LoadedModule,
    _source: Option<LoadedModule>,
    _meter: Option<Filter<MeterDsp>>,
    sink_name: String,
    play_name: String,
    /// What leaves this output after its fader: the listen playback, or the PC and B sources.
    emit_name: String,
    meter_name: String,
    peak: Arc<Peaks>,
    /// Identifies this output to the Mix Check recorder. See `bus_key`.
    key: u32,
}

/// Recorder key of one output: "pc", "a:N" or "b:N". 0 matches no bus.
fn bus_key(output: &str) -> u32 {
    if output == "pc" {
        return 1;
    }
    let Some((side, index)) = output.split_once(':') else { return 0 };
    let Ok(index) = index.parse::<u32>() else { return 0 };
    match side {
        "a" => 10 + index,
        "b" => 20 + index,
        _ => 0,
    }
}

/// The permanent tap on what an output emits. It holds the level meter, and it is
/// the Mix Check recorder: it is linked all the time, so a take captures from the
/// first cycle after Record instead of losing the start to link setup.
struct MeterDsp {
    peak: Arc<Peaks>,
    share: Arc<CheckShare>,
    key: u32,
}

impl Dsp for MeterDsp {
    fn process(&mut self, input: Option<[&[f32]; 2]>, _output: Option<[&mut [f32]; 2]>) {
        let Some([left, right]) = input else { return };
        self.peak.hold(left, right);
        if !self.share.recording.load(Ordering::Relaxed) || self.share.rec_key.load(Ordering::Relaxed) != self.key {
            return;
        }
        self.share.peak.store(peak_of(left).max(peak_of(right)).to_bits(), Ordering::Relaxed);
        // Held only for a moment by the main thread, when a take starts or ends.
        let Ok(mut buf) = self.share.samples.try_lock() else { return };
        let room = CHECK_MAX.saturating_sub(buf.len()) / 2;
        let room = room.min(buf.capacity().saturating_sub(buf.len()) / 2);
        for (l, r) in left.iter().zip(right.iter()).take(room) {
            buf.push(*l);
            buf.push(*r);
        }
        self.share.recorded.store(buf.len() as u32, Ordering::Relaxed);
    }
}

struct CheckShare {
    recording: AtomicBool,
    /// `bus_key` of the output being recorded. Meter taps compare against it.
    rec_key: AtomicU32,
    playing: AtomicBool,
    /// Interleaved stereo. Capacity is reserved before a take, so the realtime side never allocates.
    samples: Mutex<Vec<f32>>,
    /// Samples recorded so far, readable without the lock.
    recorded: AtomicU32,
    play: Mutex<Vec<f32>>,
    play_at: AtomicU32,
    /// Peak of the output being recorded, for the Mix Check level bar.
    peak: AtomicU32,
}

impl CheckShare {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            recording: AtomicBool::new(false),
            rec_key: AtomicU32::new(0),
            playing: AtomicBool::new(false),
            samples: Mutex::new(Vec::new()),
            recorded: AtomicU32::new(0),
            play: Mutex::new(Vec::new()),
            play_at: AtomicU32::new(0),
            peak: AtomicU32::new(0),
        })
    }
}

struct CheckLive {
    _rec: Filter<WakeDsp>,
    _play: Filter<PlayDsp>,
}

/// Input-only and not passive. Linked to an output while a take runs, it wakes
/// a suspended output; the samples themselves come from the output's meter tap.
struct WakeDsp;

impl Dsp for WakeDsp {
    fn process(&mut self, _input: Option<[&[f32]; 2]>, _output: Option<[&mut [f32]; 2]>) {}
}

struct PlayDsp {
    share: Arc<CheckShare>,
}

impl Dsp for PlayDsp {
    fn process(&mut self, _input: Option<[&[f32]; 2]>, output: Option<[&mut [f32]; 2]>) {
        let Some([left, right]) = output else { return };
        left.fill(0.0);
        right.fill(0.0);
        if !self.share.playing.load(Ordering::Relaxed) {
            return;
        }
        let Ok(play) = self.share.play.try_lock() else { return };
        let mut at = self.share.play_at.load(Ordering::Relaxed) as usize;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            if at + 1 >= play.len() {
                self.share.playing.store(false, Ordering::Relaxed);
                break;
            }
            *l = play[at];
            *r = play[at + 1];
            at += 2;
        }
        self.share.play_at.store(at as u32, Ordering::Relaxed);
    }
}

/// One take. Phases: idle, recording, ready, playing, paused.
struct Check {
    phase: String,
    /// Output being recorded: "pc", "a:N" or "b:N".
    output: String,
    /// Listen output the take plays on: "a:N", or empty for the default output.
    play_to: String,
    take: Vec<f32>,
    /// When a playing take is over, even if the play node never pulled it.
    play_until: Option<Instant>,
}

impl Check {
    fn new() -> Self {
        Self { phase: "idle".into(), output: "pc".into(), play_to: String::new(), take: Vec::new(), play_until: None }
    }
}

/// Seconds in an interleaved stereo 48 kHz buffer.
fn stereo_secs(samples: usize) -> f64 {
    samples as f64 / 48_000.0 / 2.0
}

impl Engine {
    fn new(context: ContextRc, core: CoreRc) -> Self {
        Self {
            strips: HashMap::new(),
            mic: None,
            buses_a: Vec::new(),
            buses_b: Vec::new(),
            bus_pc: None,
            fx: Vec::new(),
            check_live: None,
            held_links: HashMap::new(),
            bound: HashMap::new(),
            metadata: None,
            core,
            context,
            model: Model::new(),
            config: Config::default(),
            pending_volume: HashMap::new(),
            check: Check::new(),
            share: CheckShare::new(),
            ready: false,
            spiked: false,
            spike_waits: 0,
            applied: false,
            boot_waits: 0,
            bt: bluetooth::Guard::start(),
            bt_sinks: HashMap::new(),
            quit: false,
            held_routes: Vec::new(),
            analyzer: Analyzer::new(),
            vst_hosts: super::vst::Hosts::default(),
            spectrum_at: None,
        }
    }

    fn handle(&mut self, body: &str) -> String {
        let value: serde_json::Value = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
        let op = value.get("op").and_then(|op| op.as_str()).unwrap_or("");
        if op == "meters" {
            return serde_json::json!({ "ok": true, "meters": self.meters(true) }).to_string();
        }
        if op == "spectrum" {
            let index = |key: &str| value.get(key).and_then(serde_json::Value::as_u64).unwrap_or(0) as usize;
            let bands = self.spectrum(index("bus"), index("slot"));
            return serde_json::json!({ "bands": bands }).to_string();
        }
        self.poll_check();
        match op {
            "apply" | "levels" => {
                if let Some(config) = value.get("config").and_then(|config| serde_json::from_value::<Config>(config.clone()).ok()) {
                    if op == "apply" {
                        if config::check_play_valid(&config.check_play) {
                            self.check.play_to = config.check_play.clone();
                        }
                        self.apply_config(&config);
                    } else {
                        self.apply_levels(&config);
                    }
                }
            }
            "record" => {
                let output = value.get("output").and_then(|output| output.as_str()).unwrap_or("pc").to_string();
                self.start_record(output);
            }
            "stop" => self.finish_record(),
            "play" => {
                let play_to = value.get("play_to").and_then(|play_to| play_to.as_str()).unwrap_or("");
                self.check.play_to = if config::check_play_valid(play_to) { play_to.to_string() } else { String::new() };
                self.play_take();
            }
            "pause" => self.pause_take(),
            "vst_gui" => {
                if let Some(instance) = value.get("instance").and_then(serde_json::Value::as_str) {
                    self.vst_hosts.open_editor(instance);
                }
            }
            "quit" => {}
            _ => {}
        }
        self.status()
    }

    /// Spectrum after chain position `slot` of one effect bus, in tenths of a dB.
    fn spectrum(&mut self, bus: usize, slot: usize) -> Vec<f32> {
        let Some(strip) = self.fx.get(bus) else { return Vec::new() };
        for (index, other) in self.fx.iter().enumerate() {
            if index != bus {
                other.controls.scope.disarm();
            }
        }
        strip.controls.scope.arm(slot);
        let bands = self.analyzer.bands(&strip.controls.scope);
        self.spectrum_at = Some(Instant::now());
        bands.into_iter().map(|db| (db * 10.0).round() / 10.0).collect()
    }

    fn expire_spectrum(&mut self) {
        if self.spectrum_at.is_some_and(|at| at.elapsed() > Duration::from_secs(1)) {
            self.spectrum_at = None;
            for strip in &self.fx {
                strip.controls.scope.disarm();
            }
        }
    }

    fn poll_check(&mut self) {
        if self.check.phase == "recording" && self.share.recorded.load(Ordering::Relaxed) as usize >= CHECK_MAX {
            self.finish_record();
        }
        let overdue = self.check.play_until.is_some_and(|until| Instant::now() > until);
        if self.check.phase == "playing" && (!self.share.playing.load(Ordering::Relaxed) || overdue) {
            self.share.playing.store(false, Ordering::Relaxed);
            self.check.phase = "ready".into();
            self.check.play_until = None;
            self.share.play_at.store(0, Ordering::Relaxed);
            self.apply_levels(&self.config.clone());
        }
    }

    fn check_secs(&self) -> f64 {
        match self.check.phase.as_str() {
            "recording" => stereo_secs(self.share.recorded.load(Ordering::Relaxed) as usize),
            "playing" | "paused" => stereo_secs(self.share.play_at.load(Ordering::Relaxed) as usize),
            _ => 0.0,
        }
    }

    /// Peaks by key. `take` clears them, so each fast poll shows what happened since the last one.
    fn meters(&self, take: bool) -> serde_json::Map<String, serde_json::Value> {
        let read = |peak: &Peaks| if take { peak.take() } else { peak.peek() };
        let mut meters = serde_json::Map::new();
        if let Some(mic) = &self.mic {
            let (left, right) = read(&mic.controls.peak);
            meters.insert("mic".into(), serde_json::json!(left.max(right)));
        }
        for (id, strip) in &self.strips {
            let (left, right) = read(&strip.controls.peak);
            meters.insert(format!("{id}:l"), serde_json::json!(left));
            meters.insert(format!("{id}:r"), serde_json::json!(right));
        }
        let outputs = self
            .bus_pc
            .iter()
            .map(|bus| ("pc".to_string(), bus))
            .chain(self.buses_a.iter().enumerate().map(|(index, bus)| (format!("a:{index}"), bus)))
            .chain(self.buses_b.iter().enumerate().map(|(index, bus)| (format!("b:{index}"), bus)));
        for (id, bus) in outputs {
            let (left, right) = read(&bus.peak);
            meters.insert(format!("out:{id}:l"), serde_json::json!(left));
            meters.insert(format!("out:{id}:r"), serde_json::json!(right));
        }
        if self.check.phase == "recording" {
            meters.insert("check".into(), serde_json::json!(f32::from_bits(self.share.peak.load(Ordering::Relaxed))));
        }
        meters
    }

    fn status(&self) -> String {
        serde_json::json!({
            "ok": true,
            "mix_active": self.config.mix_on && (self.mic.is_some() || !self.strips.is_empty()),
            "meters": self.meters(false),
            "graph": self.graph(),
            "check": self.check.phase,
            "check_secs": self.check_secs(),
            "check_output": self.check.output,
            "has_take": !self.check.take.is_empty(),
            "vst": self.vst_hosts.statuses(),
            "take_secs": stereo_secs(self.check.take.len()),
        })
        .to_string()
    }

    fn graph(&self) -> Graph {
        let mut graph = Graph::default();
        graph.nodes = self.model.nodes.values().cloned().collect();
        graph.clients = self.model.clients.clone();
        // Port-level links folded to node pairs, for the Signal Flow view.
        let mut edges: Vec<(u32, u32)> = self
            .model
            .links
            .values()
            .filter_map(|(out_port, in_port)| {
                let from = self.model.ports.get(out_port)?.node_id;
                let to = self.model.ports.get(in_port)?.node_id;
                Some((from, to))
            })
            .collect();
        edges.sort_unstable();
        edges.dedup();
        graph.edges = edges;
        graph.default_sink = self.model.default_sink();
        graph.default_source = if self.model.default_source.is_empty() {
            Graph::BUILTIN_SOURCE.into()
        } else {
            self.model.default_source.clone()
        };
        graph
    }

    fn publish_bt(&self) {
        self.bt.set_sinks(self.bt_sinks.values().cloned().collect());
    }

    fn apply_config(&mut self, config: &Config) {
        self.config = config.clone();
        self.applied = true;
        self.bt.set_guard(!mic_target(config, &self.model.default_source).starts_with("bluez_"));
        let source = mic_target(&self.config, &self.model.default_source);
        let recreate = self.mic.as_ref().map(|mic| mic.capture != source).unwrap_or(true);
        if recreate {
            self.mic = None;
            match make_strip(&self.context, &self.core, "mic", "Mic", Kind::Mic, Some(&source)) {
                Ok(strip) => self.mic = Some(strip),
                Err(err) => eprintln!("frame-audio-mixer: mic: {err}"),
            }
        }
        let wanted: HashSet<_> = self.config.channels.iter().map(|channel| channel.id.clone()).collect();
        self.strips.retain(|id, _| wanted.contains(id));
        for channel in &self.config.channels {
            let renamed = self.strips.get(&channel.id).is_some_and(|strip| strip.label != channel.name);
            if renamed {
                self.strips.remove(&channel.id);
            }
            if self.strips.contains_key(&channel.id) {
                continue;
            }
            match make_strip(&self.context, &self.core, &channel.id, &channel.name, Kind::Channel, None) {
                Ok(strip) => {
                    self.strips.insert(channel.id.clone(), strip);
                }
                Err(err) => eprintln!("frame-audio-mixer: {}: {err}", channel.id),
            }
        }
        if self.bus_pc.is_none() {
            match make_send_bus(&self.context, &self.core, "frame_amix_pc", "frame_amix_source", "Frame Mix", bus_key("pc"), &self.share) {
                Ok(bus) => self.bus_pc = Some(bus),
                Err(err) => eprintln!("frame-audio-mixer: pc bus: {err}"),
            }
        }
        while self.buses_a.len() < config::A_SLOTS {
            let index = self.buses_a.len();
            let name = format!("frame_amix_a{}", index + 1);
            match make_listen_bus(&self.context, &self.core, &name, bus_key(&format!("a:{index}")), &self.share) {
                Ok(bus) => self.buses_a.push(bus),
                Err(err) => {
                    eprintln!("frame-audio-mixer: {name}: {err}");
                    break;
                }
            }
        }
        while self.buses_b.len() < config::B_SLOTS {
            let index = self.buses_b.len();
            let custom = self.config.b.get(index).map(|bus| bus.name.clone()).unwrap_or_default();
            let label = if custom.is_empty() { format!("B{}", index + 1) } else { custom };
            let name = format!("frame_amix_b{}", index + 1);
            match make_send_bus(&self.context, &self.core, &name, &name, &label, bus_key(&format!("b:{index}")), &self.share) {
                Ok(bus) => self.buses_b.push(bus),
                Err(err) => {
                    eprintln!("frame-audio-mixer: {name}: {err}");
                    break;
                }
            }
        }
        if self.fx.len() != 2 {
            self.fx.clear();
            for label in ["fx1", "fx2"] {
                if let Ok(strip) = make_strip(&self.context, &self.core, label, label, Kind::Fx, None) {
                    self.fx.push(strip);
                }
            }
        }
        if self.check_live.is_none() {
            match make_check(&self.core, &self.share) {
                Ok(live) => self.check_live = Some(live),
                Err(err) => eprintln!("frame-audio-mixer: mix check: {err}"),
            }
        }
        self.apply_levels(config);
    }

    fn apply_levels(&mut self, config: &Config) {
        self.config = config.clone();
        let plugins: Vec<crate::vst::VstRef> =
            [&config.fx_a, &config.fx_b].iter().flat_map(|bus| bus.chain.iter()).filter_map(|slot| slot.vst.clone()).collect();
        self.vst_hosts.sync(&plugins);
        // A take is recorded with the mic in it. Only playback on the headset holds the
        // mic and the headset effect returns, so the clip cannot feed back.
        let hold_mic = self.check.phase == "playing";
        if let Some(mic) = &self.mic {
            mic.controls.set_gain_db(config.mic_gain_db, config::MIC_PREAMP);
            mic.controls.mute.store(config.mic_muted, Ordering::Relaxed);
            mic.controls.hold.store(hold_mic, Ordering::Relaxed);
        }
        for channel in &config.channels {
            if let Some(strip) = self.strips.get(&channel.id) {
                strip.controls.set_gain_db(channel.gain_db, 1.0);
                strip.controls.mute.store(channel.muted, Ordering::Relaxed);
            }
        }
        let pc_name = self.bus_pc.as_ref().map(|bus| bus.sink_name.clone());
        if let Some(name) = pc_name {
            self.set_named_volume(&name, config.pc.gain_db, config.pc.muted);
        }
        for index in 0..config::A_SLOTS {
            let a_name = self.buses_a.get(index).map(|bus| bus.sink_name.clone());
            if let Some(name) = a_name {
                self.set_named_volume(&name, config.a[index].gain_db, config.a[index].muted);
            }
        }
        for index in 0..config::B_SLOTS {
            let b_name = self.buses_b.get(index).map(|bus| bus.sink_name.clone());
            if let Some(name) = b_name {
                self.set_named_volume(&name, config.b[index].gain_db, config.b[index].muted);
            }
        }
        for (index, fx) in [&config.fx_a, &config.fx_b].into_iter().enumerate() {
            if let Some(strip) = self.fx.get(index) {
                strip.controls.set_gain_db(fx.gain_db, 1.0);
                let hold = hold_mic && config::route_on(fx.routes, true, 0);
                strip.controls.mute.store(fx.muted, Ordering::Relaxed);
                strip.controls.hold.store(hold, Ordering::Relaxed);
                if let Ok(mut chain) = strip.controls.chain.lock() {
                    *chain = fx.chain.clone();
                }
            }
        }
        self.sync_routes();
    }

    fn set_named_volume(&mut self, name: &str, db: f64, mute: bool) {
        if name.is_empty() {
            return;
        }
        let linear = config::db_to_lin(db) as f32;
        self.pending_volume.insert(name.to_string(), (linear, mute));
        if let Some(node) = self.bound.get(name) {
            apply_volume_pod(node, linear, mute);
        }
    }

    fn apply_volume(&self, name: &str, node: &Node) {
        if let Some((linear, mute)) = self.pending_volume.get(name) {
            apply_volume_pod(node, *linear, *mute);
        }
    }

    fn sync_routes(&mut self) {
        let mut routes = Vec::new();
        let dest = Dest { a: &self.buses_a, b: &self.buses_b, pc: self.bus_pc.as_ref(), fx: &self.fx };
        if let Some(mic) = &self.mic {
            let flow = Flow { bits: self.config.mic_routes, to_pc: self.config.mic_to_pc, fx1: self.config.mic_send_fx1, fx2: self.config.mic_send_fx2 };
            push_routes(&mut routes, &mic.dsp(), &dest, flow);
            routes.push(Wanted { output_node: mic.loop_out(), input_node: mic.dsp() });
        }
        for channel in &self.config.channels {
            if let Some(strip) = self.strips.get(&channel.id) {
                let flow = Flow { bits: channel.routes, to_pc: channel.to_pc, fx1: channel.send_fx1, fx2: channel.send_fx2 };
                push_routes(&mut routes, &strip.dsp(), &dest, flow);
                routes.push(Wanted { output_node: strip.loop_out(), input_node: strip.dsp() });
            }
        }
        for (index, fx) in [&self.config.fx_a, &self.config.fx_b].into_iter().enumerate() {
            if let Some(strip) = self.fx.get(index) {
                let flow = Flow { bits: fx.routes, to_pc: fx.to_pc, fx1: false, fx2: false };
                push_routes(&mut routes, &strip.dsp(), &dest, flow);
                routes.push(Wanted { output_node: strip.loop_out(), input_node: strip.dsp() });
            }
        }
        let sink = self.model.default_sink();
        let targets = graph::output_sinks(self.model.nodes.values(), &sink);
        let play_on = self
            .check
            .play_to
            .strip_prefix("a:")
            .and_then(|index| index.parse::<usize>().ok())
            .and_then(|index| targets.get(index).cloned().flatten())
            .unwrap_or_else(|| sink.clone());
        for (bus, target) in self.buses_a.iter().zip(targets) {
            if let Some(target) = target {
                routes.push(Wanted { output_node: bus.play_name.clone(), input_node: target });
            }
        }
        for bus in self.bus_pc.iter().chain(self.buses_a.iter()).chain(self.buses_b.iter()) {
            routes.push(Wanted { output_node: bus.emit_name.clone(), input_node: bus.meter_name.clone() });
        }
        if self.check.phase == "recording" {
            if let Some(bus) = self.check_bus(&self.check.output) {
                routes.push(Wanted { output_node: bus.emit_name.clone(), input_node: "frame_amix_check_virtual_sink_rec".into() });
            }
        }
        if self.check.phase == "playing" && !play_on.is_empty() {
            routes.push(Wanted { output_node: "frame_amix_check_virtual_sink_play".into(), input_node: play_on });
        }
        self.held_routes = routes;
        self.reconcile();
    }

    fn check_bus(&self, output: &str) -> Option<&Bus> {
        if output == "pc" {
            return self.bus_pc.as_ref();
        }
        let (side, index) = output.split_once(':')?;
        let index: usize = index.parse().ok()?;
        match side {
            "a" => self.buses_a.get(index),
            "b" => self.buses_b.get(index),
            _ => None,
        }
    }

    fn reconcile(&mut self) {
        let nodes: HashMap<String, u32> = self.model.nodes.values().map(|node| (node.name.clone(), node.id)).collect();
        let ports: Vec<Port> = self.model.ports.values().cloned().collect();
        let wanted = link::wanted_links(&self.held_routes, &nodes, &ports);
        let wanted_set: HashSet<(u32, u32)> = wanted.iter().map(|(output, input, _, _)| (*output, *input)).collect();
        self.held_links.retain(|key, _| wanted_set.contains(key));
        for (output_port, input_port, output_node, input_node) in wanted {
            if self.held_links.contains_key(&(output_port, input_port)) {
                continue;
            }
            let props = properties! {
                "link.output.node" => output_node.to_string(),
                "link.output.port" => output_port.to_string(),
                "link.input.node" => input_node.to_string(),
                "link.input.port" => input_port.to_string(),
                "object.linger" => "false",
            };
            if let Ok(link) = self.core.create_object::<Link>("link-factory", &props) {
                self.held_links.insert(
                    (output_port, input_port),
                    HeldLink { _link: link, since: Instant::now(), confirmed: false },
                );
            }
        }
    }

    /// Drops held links the registry never announced and creates them again.
    /// Runs from the 200 ms tick, so a failed link retries within ~2 s.
    fn heal_links(&mut self) {
        let grace = Duration::from_secs(2);
        let before = self.held_links.len();
        self.held_links.retain(|ports, held| {
            let stale = !held.confirmed && held.since.elapsed() >= grace;
            if stale {
                eprintln!("frame-audio-mixer: link {} -> {} never came up; retrying", ports.0, ports.1);
            }
            !stale
        });
        if self.held_links.len() != before {
            self.reconcile();
        }
    }

    fn start_record(&mut self, output: String) {
        if self.check.phase == "recording" {
            return;
        }
        if let Ok(mut samples) = self.share.samples.lock() {
            samples.clear();
            samples.reserve_exact(CHECK_MAX);
        }
        self.share.recorded.store(0, Ordering::Relaxed);
        self.share.peak.store(0, Ordering::Relaxed);
        self.check.output = if config::check_output_valid(&output) { output } else { "pc".into() };
        self.share.playing.store(false, Ordering::Relaxed);
        self.share.play_at.store(0, Ordering::Relaxed);
        // The key must land before the flag: the meter taps capture as soon as
        // `recording` is set, on whichever bus matches the key.
        self.share.rec_key.store(bus_key(&self.check.output), Ordering::Relaxed);
        self.share.recording.store(true, Ordering::Relaxed);
        self.check.phase = "recording".into();
        self.apply_levels(&self.config.clone());
    }

    fn finish_record(&mut self) {
        self.share.recording.store(false, Ordering::Relaxed);
        if self.check.phase != "recording" {
            return;
        }
        let samples = self.share.samples.lock().map(|mut samples| std::mem::take(&mut *samples)).unwrap_or_default();
        self.check.take = samples;
        self.check.phase = if self.check.take.is_empty() { "idle".into() } else { "ready".into() };
        self.apply_levels(&self.config.clone());
    }

    /// Plays the take on the current headset output, past every fader, or resumes it
    /// where Pause left it.
    fn play_take(&mut self) {
        if self.check.take.is_empty() || self.check.phase == "recording" || self.check.phase == "playing" {
            return;
        }
        if self.check.phase != "paused" {
            if let Ok(mut play) = self.share.play.lock() {
                play.clone_from(&self.check.take);
            }
            self.share.play_at.store(0, Ordering::Relaxed);
        }
        let left = self.check.take.len().saturating_sub(self.share.play_at.load(Ordering::Relaxed) as usize);
        self.check.play_until = Some(Instant::now() + Duration::from_secs_f64(stereo_secs(left) + 0.5));
        self.share.playing.store(true, Ordering::Relaxed);
        self.check.phase = "playing".into();
        self.apply_levels(&self.config.clone());
    }

    fn pause_take(&mut self) {
        if self.check.phase != "playing" {
            return;
        }
        self.share.playing.store(false, Ordering::Relaxed);
        self.check.phase = "paused".into();
        self.check.play_until = None;
        self.apply_levels(&self.config.clone());
    }
}

struct Dest<'a> {
    a: &'a [Bus],
    b: &'a [Bus],
    pc: Option<&'a Bus>,
    fx: &'a [Strip],
}

#[derive(Clone, Copy)]
struct Flow {
    bits: u8,
    to_pc: bool,
    fx1: bool,
    fx2: bool,
}

fn push_routes(routes: &mut Vec<Wanted>, from: &str, dest: &Dest, flow: Flow) {
    let Flow { bits, to_pc, fx1, fx2 } = flow;
    let fx = dest.fx;
    for (index, bus) in dest.a.iter().enumerate() {
        if config::route_on(bits, true, index) {
            routes.push(Wanted { output_node: from.into(), input_node: bus.sink_name.clone() });
        }
    }
    for (index, bus) in dest.b.iter().enumerate() {
        if config::route_on(bits, false, index) {
            routes.push(Wanted { output_node: from.into(), input_node: bus.sink_name.clone() });
        }
    }
    if to_pc {
        if let Some(bus) = dest.pc {
            routes.push(Wanted { output_node: from.into(), input_node: bus.sink_name.clone() });
        }
    }
    if fx1 {
        if let Some(strip) = fx.first() {
            routes.push(Wanted { output_node: from.into(), input_node: strip.sink() });
        }
    }
    if fx2 {
        if let Some(strip) = fx.get(1) {
            routes.push(Wanted { output_node: from.into(), input_node: strip.sink() });
        }
    }
}

fn mic_target(config: &Config, default_source: &str) -> String {
    match config.mic_source.as_str() {
        "builtin" => Graph::BUILTIN_SOURCE.into(),
        "" => Graph::mixer_default_source(default_source).into(),
        named => named.into(),
    }
}

fn make_strip(context: &ContextRc, core: &CoreRc, id: &str, label: &str, kind: Kind, capture_target: Option<&str>) -> Result<Strip, String> {
    let base = format!("frame_amix_{id}");
    let module = match kind {
        Kind::Mic => load_loopback(context, &mic_loopback_args(&base, capture_target.unwrap_or(Graph::BUILTIN_SOURCE)))?,
        Kind::Channel => load_loopback(context, &channel_loopback_args(&base, label))?,
        Kind::Fx => load_loopback(context, &fx_loopback_args(&base, label))?,
    };
    let controls = Controls::new();
    let dsp = StripDsp {
        proc: Processor::with_scope(controls.scope.clone()),
        applied: Vec::new(),
        gain: 1.0,
        target: controls.gain.clone(),
        mute: controls.mute.clone(),
        hold: controls.hold.clone(),
        chain: controls.chain.clone(),
        peak: controls.peak.clone(),
    };
    // Not passive, so a routed strip wakes the output it feeds.
    let filter = Filter::new(core, &format!("{base}_virtual_sink_dsp"), Ports { input: true, output: true, passive: false }, dsp)?;
    Ok(Strip {
        _modules: vec![module],
        _filter: filter,
        controls,
        base,
        label: label.to_string(),
        capture: capture_target.unwrap_or("").to_string(),
        kind,
    })
}

fn make_listen_bus(context: &ContextRc, core: &CoreRc, name: &str, key: u32, share: &Arc<CheckShare>) -> Result<Bus, String> {
    let module = load_loopback(context, &listen_bus_args(name))?;
    let play_name = format!("{name}_virtual_sink_play");
    Ok(with_meter(core, name, share, Bus {
        _module: module,
        _source: None,
        _meter: None,
        sink_name: format!("{name}_virtual_sink"),
        emit_name: play_name.clone(),
        play_name,
        meter_name: String::new(),
        peak: Peaks::new(),
        key,
    }))
}

fn make_send_bus(context: &ContextRc, core: &CoreRc, name: &str, source_name: &str, label: &str, key: u32, share: &Arc<CheckShare>) -> Result<Bus, String> {
    let module = load_loopback(context, &send_bus_args(name))?;
    let source = load_loopback(context, &source_args(name, source_name, label))?;
    Ok(with_meter(core, name, share, Bus {
        _module: module,
        _source: Some(source),
        _meter: None,
        sink_name: format!("{name}_virtual_sink"),
        play_name: format!("{name}_virtual_sink_discard"),
        emit_name: source_name.to_string(),
        meter_name: String::new(),
        peak: Peaks::new(),
        key,
    }))
}

/// A passive tap on what the output emits, for its level meter and the Mix Check
/// recorder. Passive, so an idle output stays suspended. A bus without a meter
/// still plays; only its meter and takes stay silent.
fn with_meter(core: &CoreRc, name: &str, share: &Arc<CheckShare>, mut bus: Bus) -> Bus {
    bus.meter_name = format!("{name}_virtual_sink_meter");
    let dsp = MeterDsp { peak: bus.peak.clone(), share: share.clone(), key: bus.key };
    bus._meter = Filter::new(core, &bus.meter_name, Ports { input: true, output: false, passive: true }, dsp).ok();
    bus
}

/// The Mix Check waker and player. Neither is passive: a take must wake the
/// output it records, and playback must wake the speakers.
fn make_check(core: &CoreRc, share: &Arc<CheckShare>) -> Result<CheckLive, String> {
    let rec = Filter::new(core, "frame_amix_check_virtual_sink_rec", Ports { input: true, output: false, passive: false }, WakeDsp)?;
    let play = Filter::new(core, "frame_amix_check_virtual_sink_play", Ports { input: false, output: true, passive: false }, PlayDsp { share: share.clone() })?;
    Ok(CheckLive { _rec: rec, _play: play })
}

/// Buses and effect sinks are linked by the engine only. pipewire-pulse lists exact
/// `Audio/Sink` nodes, so this class keeps them and their monitors out of every
/// device picker, and WirePlumber never chooses one as a default or a target.
const INTERNAL_SINK: &str = "Audio/Sink/Internal";

fn channel_loopback_args(base: &str, label: &str) -> String {
    serde_json::json!({
        "capture.props": {
            "media.class": "Audio/Sink",
            "node.name": base,
            "node.description": label,
            "node.nick": label,
            "device.description": label,
            "audio.position": ["FL", "FR"],
            "priority.session": 1,
            "audio.rate": 48000,
            "node.latency": "480/48000",
            "stream.dont-remix": true,
            "state.restore-props": false
        },
        "playback.props": playback_props(&format!("{base}_virtual_sink_out"), true)
    })
    .to_string()
}

fn mic_loopback_args(base: &str, target: &str) -> String {
    serde_json::json!({
        "capture.props": {
            "node.name": format!("{base}_from_mic"),
            "target.object": target,
            "node.passive": false,
            "node.dont-reconnect": true,
            "node.dont-fallback": true,
            "audio.position": ["FL", "FR"],
            "audio.rate": 48000,
            "node.latency": "480/48000",
            "stream.dont-remix": true,
            "state.restore-props": false
        },
        "playback.props": playback_props(&format!("{base}_virtual_sink_out"), false)
    })
    .to_string()
}

fn fx_loopback_args(base: &str, label: &str) -> String {
    serde_json::json!({
        "capture.props": {
            "media.class": INTERNAL_SINK,
            "node.name": format!("{base}_virtual_sink"),
            "node.description": label,
            "node.virtual": true,
            "audio.position": ["FL", "FR"],
            "priority.session": 1,
            "audio.rate": 48000,
            "node.latency": "480/48000",
            "state.restore-props": false
        },
        "playback.props": playback_props(&format!("{base}_virtual_sink_out"), true)
    })
    .to_string()
}

fn listen_bus_args(name: &str) -> String {
    serde_json::json!({
        "capture.props": {
            "media.class": INTERNAL_SINK,
            "node.name": format!("{name}_virtual_sink"),
            "node.description": name,
            "node.virtual": true,
            "audio.position": ["FL", "FR"],
            "priority.session": 1,
            "audio.rate": 48000,
            "node.latency": "480/48000",
            "state.restore-props": false
        },
        "playback.props": playback_props(&format!("{name}_virtual_sink_play"), true)
    })
    .to_string()
}

fn send_bus_args(name: &str) -> String {
    serde_json::json!({
        "capture.props": {
            "media.class": INTERNAL_SINK,
            "node.name": format!("{name}_virtual_sink"),
            "node.virtual": true,
            "audio.position": ["FL", "FR"],
            "priority.session": 1,
            "audio.rate": 48000,
            "node.latency": "480/48000",
            "state.restore-props": false
        },
        "playback.props": playback_props(&format!("{name}_virtual_sink_discard"), true)
    })
    .to_string()
}

fn source_args(name: &str, source_name: &str, label: &str) -> String {
    serde_json::json!({
        "capture.props": {
            "node.name": format!("{name}_from_bus"),
            "stream.capture.sink": true,
            "target.object": format!("{name}_virtual_sink"),
            "node.passive": false,
            "node.virtual": true,
            "node.dont-reconnect": true,
            "audio.position": ["FL", "FR"],
            "audio.rate": 48000,
            "node.latency": "480/48000",
            "state.restore-props": false
        },
        "playback.props": {
            "media.class": "Audio/Source",
            "node.name": source_name,
            "node.description": label,
            "node.nick": label,
            "device.description": label,
            "audio.position": ["FL", "FR"],
            "priority.session": 1,
            "audio.rate": 48000,
            "node.latency": "480/48000",
            "state.restore-props": false
        }
    })
    .to_string()
}

fn playback_props(name: &str, passive: bool) -> serde_json::Value {
    serde_json::json!({
        "node.name": name,
        "node.passive": passive,
        "node.virtual": true,
        "node.autoconnect": false,
        "node.dont-reconnect": true,
        "audio.position": ["FL", "FR"],
        "audio.rate": 48000,
        "node.latency": "480/48000",
        "stream.dont-remix": true,
        "state.restore-props": false
    })
}

fn apply_volume_pod(node: &Node, linear: f32, mute: bool) {
    let bytes = volume_pod(linear, mute);
    if let Some(pod) = Pod::from_bytes(&bytes) {
        node.set_param(libspa::param::ParamType::Props, 0, pod);
    }
}

fn volume_pod(linear: f32, mute: bool) -> Vec<u8> {
    use libspa::pod::serialize::PodSerializer;
    use libspa::pod::{Object, Property, PropertyFlags, Value, ValueArray};
    let obj = Value::Object(Object {
        type_: libspa::sys::SPA_TYPE_OBJECT_Props,
        id: libspa::sys::SPA_PARAM_Props,
        properties: vec![
            Property {
                key: libspa::sys::SPA_PROP_channelVolumes,
                flags: PropertyFlags::empty(),
                value: Value::ValueArray(ValueArray::Float(vec![linear, linear])),
            },
            Property {
                key: libspa::sys::SPA_PROP_mute,
                flags: PropertyFlags::empty(),
                value: Value::Bool(mute),
            },
            // These sinks keep `monitor.channel-volumes = false`. Frame Mix and the B
            // sources capture the monitor, so they only follow the fader through these.
            Property {
                key: libspa::sys::SPA_PROP_monitorVolumes,
                flags: PropertyFlags::empty(),
                value: Value::ValueArray(ValueArray::Float(vec![linear, linear])),
            },
            Property {
                key: libspa::sys::SPA_PROP_monitorMute,
                flags: PropertyFlags::empty(),
                value: Value::Bool(mute),
            },
        ],
    });
    PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &obj).map(|cursor| cursor.0.into_inner()).unwrap_or_default()
}

struct LoadedModule {
    ptr: NonNull<pipewire::sys::pw_impl_module>,
    _context: ContextRc,
}

fn load_loopback(context: &ContextRc, args: &str) -> Result<LoadedModule, String> {
    let name = CString::new("libpipewire-module-loopback").map_err(|err| err.to_string())?;
    let cargs = CString::new(args).map_err(|err| err.to_string())?;
    let ptr = unsafe { pipewire::sys::pw_context_load_module(context.as_raw_ptr(), name.as_ptr(), cargs.as_ptr(), std::ptr::null_mut()) };
    NonNull::new(ptr).map(|ptr| LoadedModule { ptr, _context: context.clone() }).ok_or_else(|| "loopback module failed".into())
}

impl Drop for LoadedModule {
    fn drop(&mut self) {
        unsafe { pipewire::sys::pw_impl_module_destroy(self.ptr.as_ptr()) };
    }
}

fn read_node(id: u32, props: &libspa::utils::dict::DictRef) -> Option<AudioNode> {
    let name = props.get("node.name")?.to_string();
    Some(AudioNode {
        id,
        serial: props.get("object.serial").and_then(|value| value.parse().ok()).unwrap_or(0),
        name,
        description: props.get("node.description").unwrap_or("").to_string(),
        media_class: props.get("media.class").unwrap_or("").to_string(),
        media_name: props.get("media.name").unwrap_or("").to_string(),
        app_name: props.get("application.name").unwrap_or("").to_string(),
        binary: props.get("application.process.binary").unwrap_or("").to_string(),
        is_virtual: props.get("node.virtual").is_some_and(|value| value == "true"),
        target: props.get("target.object").unwrap_or("").to_string(),
    })
}

fn read_port(id: u32, props: &libspa::utils::dict::DictRef) -> Option<Port> {
    let node_id = props.get("node.id")?.parse().ok()?;
    let direction = match props.get("port.direction")? {
        "in" => Direction::In,
        "out" => Direction::Out,
        _ => return None,
    };
    Some(Port { id, node_id, direction, channel: props.get("audio.channel").unwrap_or("UNK").to_string() })
}

fn read_link(id: u32, props: &libspa::utils::dict::DictRef) -> Option<(u32, (u32, u32))> {
    let output = props.get("link.output.port")?.parse().ok()?;
    let input = props.get("link.input.port")?.parse().ok()?;
    Some((id, (output, input)))
}

fn parse_default_name(value: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(value).ok()?;
    parsed.get("name").and_then(|name| name.as_str()).map(|name| name.to_string())
}

pub fn self_test() -> Result<(), String> {
    let mut config = Config::default();
    if mic_target(&config, "bluez_input.headset") != Graph::BUILTIN_SOURCE {
        return Err("system default followed a Bluetooth mic".into());
    }
    if mic_target(&config, "alsa_input.usb") != "alsa_input.usb" {
        return Err("system default ignored a USB mic".into());
    }
    config.mic_source = "bluez_input.headset".into();
    if mic_target(&config, "") != "bluez_input.headset" {
        return Err("a picked Bluetooth mic was replaced".into());
    }
    if bus_key("pc") != 1 || bus_key("a:0") != 10 || bus_key("a:2") != 12 || bus_key("b:3") != 23 {
        return Err("bus keys".into());
    }
    if bus_key("") != 0 || bus_key("x:1") != 0 || bus_key("a:x") != 0 {
        return Err("bus key rejects".into());
    }
    Ok(())
}
