//! In-process PipeWire graph. Loopback modules are the devices apps can see.
//! Gain, inserts and meters run in stream callbacks. Routes are ordinary links.

use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use libspa::pod::Pod;
use pipewire::context::ContextRc;
use pipewire::core::CoreRc;
use pipewire::link::Link;
use pipewire::main_loop::MainLoopRc;
use pipewire::metadata::Metadata;
use pipewire::node::Node;
use pipewire::properties::properties;
use pipewire::proxy::Listener as ProxyListener;
use pipewire::stream::{Stream, StreamFlags, StreamListener, StreamRc};
use pipewire::types::ObjectType;

use crate::config::{self, Config, EffectSlot};
use crate::engine::link::{self, Direction, Port, Wanted};
use crate::fx::dsp::Processor;
use crate::graph::{AppRef, AudioNode, Graph};

const LATENCY_FRAMES: u32 = 480;
const RING_FRAMES: u32 = 8192;
const CHECK_MAX: usize = 48_000 * 10 * 2;

pub fn socket_path() -> std::path::PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    std::path::PathBuf::from(runtime).join("frame-audio-mixer").join("engine.sock")
}

pub fn serve(spike: bool) -> Result<(), String> {
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
                    }
                }
                ObjectType::Port => {
                    if let Some(port) = read_port(global.id, props) {
                        engine.model.ports.insert(global.id, port);
                        engine.reconcile();
                    }
                }
                ObjectType::Link => {
                    if let Some(link) = read_link(global.id, props) {
                        engine.model.links.insert(link.0, link.1);
                    }
                }
                ObjectType::Client => {
                    let name = props.get("application.name").unwrap_or("").to_string();
                    let binary = props.get("application.process.binary").unwrap_or("").to_string();
                    if !name.is_empty() || !binary.is_empty() {
                        engine.model.clients.push(AppRef { name, binary });
                    }
                }
                ObjectType::Metadata => {
                    if props.get("metadata.name") == Some("default") && engine.metadata.is_none() {
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
                                                engine.model.default_source = name;
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
                }
                _ => {}
            }
        })
        .global_remove(move |id| {
            let mut engine = engine_rm.borrow_mut();
            if let Some(node) = engine.model.nodes.remove(&id) {
                engine.bound.remove(&node.name);
            }
            engine.model.ports.remove(&id);
            engine.model.links.remove(&id);
            let live: HashSet<u32> = engine.model.ports.keys().copied().collect();
            engine.held_links.retain(|ports, _| live.contains(&ports.0) && live.contains(&ports.1));
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
        let mut engine = engine_tick.borrow_mut();
        engine.poll_check();
        if !spike {
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
    Ok(())
}

fn spike_config() -> Config {
    let mut config = Config::default();
    config.mix_on = true;
    config.outputs = 2;
    config.channels.push(config::Channel::new("ch1", "DJ"));
    config
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

struct Engine {
    strips: HashMap<String, Strip>,
    mic: Option<Strip>,
    buses_a: Vec<Bus>,
    buses_b: Vec<Bus>,
    fx: Vec<Strip>,
    check_live: Option<CheckLive>,
    held_links: HashMap<(u32, u32), Link>,
    bound: HashMap<String, Node>,
    metadata: Option<(Metadata, Box<dyn ProxyListener>)>,
    core: CoreRc,
    context: ContextRc,
    model: Model,
    config: Config,
    pending_volume: HashMap<String, (f32, bool)>,
    check: Check,
    share: Arc<CheckShare>,
    any_solo: Arc<AtomicBool>,
    ready: bool,
    spiked: bool,
    spike_waits: u8,
    quit: bool,
    held_routes: Vec<Wanted>,
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
}

struct Controls {
    gain: Arc<AtomicU32>,
    mute: Arc<AtomicBool>,
    solo: Arc<AtomicBool>,
    hold: Arc<AtomicBool>,
    chain: Arc<Mutex<Vec<EffectSlot>>>,
    peak: Arc<AtomicU32>,
}

impl Controls {
    fn new() -> Self {
        Self {
            gain: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            mute: Arc::new(AtomicBool::new(false)),
            solo: Arc::new(AtomicBool::new(false)),
            hold: Arc::new(AtomicBool::new(false)),
            chain: Arc::new(Mutex::new(Vec::new())),
            peak: Arc::new(AtomicU32::new(0)),
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
    _tap: Live<TapIo>,
    _play: Live<PlayIo>,
    controls: Controls,
    base: String,
    label: String,
    capture: String,
    kind: Kind,
}

impl Strip {
    fn dsp(&self) -> String {
        format!("{}_virtual_sink_dsp", self.base)
    }
    fn tap(&self) -> String {
        format!("{}_virtual_sink_tap", self.base)
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

struct Bus {
    _module: LoadedModule,
    _source: Option<LoadedModule>,
    sink_name: String,
    play_name: String,
}

struct Live<T> {
    _stream: StreamRc,
    _listener: StreamListener<T>,
}

struct TapIo {
    ring: Arc<Ring>,
}

struct PlayIo {
    ring: Arc<Ring>,
    proc: Processor,
    applied: Vec<EffectSlot>,
    gain: f32,
    target: Arc<AtomicU32>,
    mute: Arc<AtomicBool>,
    solo: Arc<AtomicBool>,
    hold: Arc<AtomicBool>,
    any_solo: Arc<AtomicBool>,
    chain: Arc<Mutex<Vec<EffectSlot>>>,
    peak: Arc<AtomicU32>,
}

struct Ring {
    left: Vec<AtomicU32>,
    right: Vec<AtomicU32>,
    write: AtomicU32,
}

impl Ring {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            left: (0..RING_FRAMES).map(|_| AtomicU32::new(0)).collect(),
            right: (0..RING_FRAMES).map(|_| AtomicU32::new(0)).collect(),
            write: AtomicU32::new(0),
        })
    }

    fn push(&self, interleaved: &[f32]) {
        let mut cursor = self.write.load(Ordering::Relaxed);
        let mut index = 0;
        while index + 1 < interleaved.len() {
            let slot = (cursor & (RING_FRAMES - 1)) as usize;
            self.left[slot].store(interleaved[index].to_bits(), Ordering::Relaxed);
            self.right[slot].store(interleaved[index + 1].to_bits(), Ordering::Relaxed);
            cursor = cursor.wrapping_add(1);
            index += 2;
        }
        self.write.store(cursor, Ordering::Release);
    }

    fn pull(&self, frames: usize, into: &mut [f32]) {
        let write = self.write.load(Ordering::Acquire);
        let start = write.wrapping_sub(LATENCY_FRAMES);
        for frame in 0..frames {
            let slot = (start.wrapping_add(frame as u32) & (RING_FRAMES - 1)) as usize;
            let at = frame * 2;
            if at + 1 >= into.len() {
                break;
            }
            into[at] = f32::from_bits(self.left[slot].load(Ordering::Relaxed));
            into[at + 1] = f32::from_bits(self.right[slot].load(Ordering::Relaxed));
        }
    }
}

struct CheckShare {
    recording: AtomicBool,
    playing: AtomicBool,
    samples: Mutex<Vec<f32>>,
    play: Mutex<Vec<f32>>,
    play_at: AtomicU32,
}

impl CheckShare {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            recording: AtomicBool::new(false),
            playing: AtomicBool::new(false),
            samples: Mutex::new(Vec::new()),
            play: Mutex::new(Vec::new()),
            play_at: AtomicU32::new(0),
        })
    }
}

struct CheckLive {
    _rec: Live<RecIo>,
    _play: Live<PlayIoCheck>,
}

struct RecIo {
    share: Arc<CheckShare>,
}

struct PlayIoCheck {
    share: Arc<CheckShare>,
}

struct Check {
    phase: String,
    clips: Vec<StoredClip>,
    next_id: u64,
    playing_id: u64,
    started: Option<std::time::Instant>,
}

struct StoredClip {
    id: u64,
    seconds: f64,
    wave: Vec<f32>,
    samples: Vec<f32>,
}

impl Check {
    fn new() -> Self {
        Self { phase: "idle".into(), clips: Vec::new(), next_id: 1, playing_id: 0, started: None }
    }
}

impl Engine {
    fn new(context: ContextRc, core: CoreRc) -> Self {
        Self {
            strips: HashMap::new(),
            mic: None,
            buses_a: Vec::new(),
            buses_b: Vec::new(),
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
            any_solo: Arc::new(AtomicBool::new(false)),
            ready: false,
            spiked: false,
            spike_waits: 0,
            quit: false,
            held_routes: Vec::new(),
        }
    }

    fn handle(&mut self, body: &str) -> String {
        self.poll_check();
        let value: serde_json::Value = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
        let op = value.get("op").and_then(|op| op.as_str()).unwrap_or("");
        match op {
            "apply" | "levels" => {
                if let Some(config) = value.get("config").and_then(|config| serde_json::from_value::<Config>(config.clone()).ok()) {
                    if op == "apply" {
                        self.apply_config(&config);
                    } else {
                        self.apply_levels(&config);
                    }
                }
            }
            "record" => self.start_record(),
            "stop" => self.finish_record(),
            "play" => {
                let id = value.get("id").and_then(|id| id.as_u64()).unwrap_or(0);
                self.play_clip(id);
            }
            "quit" => {}
            _ => {}
        }
        self.status()
    }

    fn poll_check(&mut self) {
        if self.check.phase == "recording" {
            let n = self.share.samples.lock().map(|samples| samples.len()).unwrap_or(0);
            if n >= CHECK_MAX {
                self.finish_record();
            }
        }
        if self.check.phase == "playing" && !self.share.playing.load(Ordering::Relaxed) {
            self.check.phase = "ready".into();
            self.check.started = None;
            self.apply_levels(&self.config.clone());
        }
    }

    fn status(&self) -> String {
        let mut meters = serde_json::Map::new();
        if let Some(mic) = &self.mic {
            meters.insert("mic".into(), serde_json::json!(f32::from_bits(mic.controls.peak.load(Ordering::Relaxed))));
        }
        for (id, strip) in &self.strips {
            meters.insert(id.clone(), serde_json::json!(f32::from_bits(strip.controls.peak.load(Ordering::Relaxed))));
        }
        let clips: Vec<_> = self
            .check
            .clips
            .iter()
            .map(|clip| serde_json::json!({ "id": clip.id, "seconds": clip.seconds, "wave": clip.wave }))
            .collect();
        serde_json::json!({
            "ok": true,
            "mix_active": self.config.mix_on && (self.mic.is_some() || !self.strips.is_empty()),
            "meters": meters,
            "graph": self.graph(),
            "check": self.check.phase,
            "check_secs": self.check.started.map(|start| start.elapsed().as_secs_f64()).unwrap_or(0.0),
            "playing_id": self.check.playing_id,
            "clips": clips,
        })
        .to_string()
    }

    fn graph(&self) -> Graph {
        let mut graph = Graph::default();
        graph.nodes = self.model.nodes.values().cloned().collect();
        graph.clients = self.model.clients.clone();
        graph.default_sink = self.model.default_sink.clone();
        graph.default_source = if self.model.default_source.is_empty() {
            Graph::BUILTIN_SOURCE.into()
        } else {
            self.model.default_source.clone()
        };
        if graph.default_sink.is_empty() {
            if let Some(node) = graph.nodes.iter().find(|node| node.media_class == "Audio/Sink" && node.name.contains("alsa_output")) {
                graph.default_sink = node.name.clone();
            }
        }
        graph
    }

    fn apply_config(&mut self, config: &Config) {
        self.config = config.clone();
        let source = mic_target(&self.config, &self.model.default_source);
        let recreate = self.mic.as_ref().map(|mic| mic.capture != source).unwrap_or(true);
        if recreate {
            self.mic = None;
            match make_strip(&self.context, &self.core, "mic", "Mic", Kind::Mic, Some(&source), &self.any_solo) {
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
            match make_strip(&self.context, &self.core, &channel.id, &channel.name, Kind::Channel, None, &self.any_solo) {
                Ok(strip) => {
                    self.strips.insert(channel.id.clone(), strip);
                }
                Err(err) => eprintln!("frame-audio-mixer: {}: {err}", channel.id),
            }
        }
        let sides = config::side_count(self.config.outputs);
        self.buses_a.truncate(sides);
        self.buses_b.truncate(sides);
        for index in 0..sides {
            if self.buses_a.len() == index {
                let name = format!("frame_amix_a{}", index + 1);
                if let Ok(bus) = make_listen_bus(&self.context, &name) {
                    self.buses_a.push(bus);
                }
            }
            if self.buses_b.len() == index {
                let source_name = if index == 0 { "frame_amix_source".into() } else { format!("frame_amix_b{}", index + 1) };
                let label = if index == 0 {
                    "Frame Mix".into()
                } else {
                    let custom = self.config.b.get(index).map(|bus| bus.name.clone()).unwrap_or_default();
                    if custom.is_empty() { format!("B{}", index + 1) } else { custom }
                };
                let name = format!("frame_amix_b{}", index + 1);
                if let Ok(bus) = make_send_bus(&self.context, &name, &source_name, &label) {
                    self.buses_b.push(bus);
                }
            }
        }
        if self.fx.len() != 2 {
            self.fx.clear();
            for label in ["fx1", "fx2"] {
                if let Ok(strip) = make_strip(&self.context, &self.core, label, label, Kind::Fx, None, &self.any_solo) {
                    self.fx.push(strip);
                }
            }
        }
        if self.check_live.is_none() {
            self.check_live = make_check(&self.core, &self.share).ok();
        }
        self.apply_levels(config);
    }

    fn apply_levels(&mut self, config: &Config) {
        self.config = config.clone();
        let any = config.mic_solo || config.channels.iter().any(|channel| channel.solo);
        self.any_solo.store(any, Ordering::Relaxed);
        let hold_mic = self.check.phase == "recording" || self.check.phase == "playing";
        if let Some(mic) = &self.mic {
            mic.controls.set_gain_db(config.mic_gain_db, config::MIC_PREAMP);
            mic.controls.mute.store(config.mic_muted, Ordering::Relaxed);
            mic.controls.solo.store(config.mic_solo, Ordering::Relaxed);
            mic.controls.hold.store(hold_mic, Ordering::Relaxed);
            if let Ok(mut chain) = mic.controls.chain.lock() {
                *chain = config.mic_inserts.clone();
            }
        }
        for channel in &config.channels {
            if let Some(strip) = self.strips.get(&channel.id) {
                strip.controls.set_gain_db(channel.gain_db, 1.0);
                strip.controls.mute.store(channel.muted, Ordering::Relaxed);
                strip.controls.solo.store(channel.solo, Ordering::Relaxed);
                if let Ok(mut chain) = strip.controls.chain.lock() {
                    *chain = channel.inserts.clone();
                }
            }
        }
        let sides = config::side_count(config.outputs);
        for index in 0..sides {
            let a_name = self.buses_a.get(index).map(|bus| bus.sink_name.clone());
            if let Some(name) = a_name {
                self.set_named_volume(&name, config.a[index].gain_db, config.a[index].muted);
            }
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
        let sides = config::side_count(self.config.outputs);
        if let Some(mic) = &self.mic {
            push_routes(&mut routes, &mic.dsp(), &self.buses_a, &self.buses_b, &self.fx, self.config.mic_routes, self.config.mic_send_fx1, self.config.mic_send_fx2, sides);
            routes.push(Wanted { output_node: mic.loop_out(), input_node: mic.tap() });
        }
        for channel in &self.config.channels {
            if let Some(strip) = self.strips.get(&channel.id) {
                push_routes(&mut routes, &strip.dsp(), &self.buses_a, &self.buses_b, &self.fx, channel.routes, channel.send_fx1, channel.send_fx2, sides);
                routes.push(Wanted { output_node: strip.loop_out(), input_node: strip.tap() });
            }
        }
        for (index, fx) in [&self.config.fx_a, &self.config.fx_b].into_iter().enumerate() {
            if let Some(strip) = self.fx.get(index) {
                push_routes(&mut routes, &strip.dsp(), &self.buses_a, &self.buses_b, &[], fx.routes, false, false, sides);
                routes.push(Wanted { output_node: strip.loop_out(), input_node: strip.tap() });
            }
        }
        let sink = self.graph().default_sink;
        for (index, bus) in self.buses_a.iter().enumerate() {
            let chosen = self.config.a.get(index).map(|item| item.target.clone()).unwrap_or_default();
            let target = if index == 0 { sink.clone() } else { chosen };
            if !target.is_empty() {
                routes.push(Wanted { output_node: bus.play_name.clone(), input_node: target });
            }
        }
        if self.check.phase == "recording" {
            if let Some(bus) = self.buses_b.first() {
                routes.push(Wanted { output_node: bus.play_name.clone(), input_node: "frame_amix_check_virtual_sink_rec".into() });
            }
        }
        if self.check.phase == "playing" {
            if let Some(bus) = self.buses_a.first() {
                routes.push(Wanted { output_node: "frame_amix_check_virtual_sink_play".into(), input_node: bus.sink_name.clone() });
            }
        }
        self.held_routes = routes;
        self.reconcile();
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
                self.held_links.insert((output_port, input_port), link);
            }
        }
    }

    fn start_record(&mut self) {
        if self.check.phase == "recording" {
            return;
        }
        if let Ok(mut samples) = self.share.samples.lock() {
            samples.clear();
        }
        self.share.recording.store(true, Ordering::Relaxed);
        self.share.playing.store(false, Ordering::Relaxed);
        self.check.phase = "recording".into();
        self.check.playing_id = 0;
        self.check.started = Some(std::time::Instant::now());
        self.apply_levels(&self.config.clone());
    }

    fn finish_record(&mut self) {
        self.share.recording.store(false, Ordering::Relaxed);
        if self.check.phase != "recording" {
            self.check.phase = "idle".into();
            self.check.started = None;
            return;
        }
        let samples = self.share.samples.lock().map(|mut samples| std::mem::take(&mut *samples)).unwrap_or_default();
        let id = self.check.next_id;
        self.check.next_id += 1;
        let seconds = samples.len() as f64 / 48_000.0 / 2.0;
        let wave = peaks_of(&samples, 36);
        self.check.clips.insert(0, StoredClip { id, seconds, wave, samples });
        self.check.clips.truncate(5);
        self.check.phase = "ready".into();
        self.check.started = None;
        self.apply_levels(&self.config.clone());
    }

    fn play_clip(&mut self, id: u64) {
        let Some(clip) = self.check.clips.iter().find(|clip| clip.id == id) else { return };
        if let Ok(mut play) = self.share.play.lock() {
            *play = clip.samples.clone();
        }
        self.share.play_at.store(0, Ordering::Relaxed);
        self.share.playing.store(true, Ordering::Relaxed);
        self.share.recording.store(false, Ordering::Relaxed);
        self.check.playing_id = id;
        self.check.phase = "playing".into();
        self.check.started = Some(std::time::Instant::now());
        self.apply_levels(&self.config.clone());
    }
}

fn push_routes(routes: &mut Vec<Wanted>, from: &str, a: &[Bus], b: &[Bus], fx: &[Strip], bits: u8, fx1: bool, fx2: bool, sides: usize) {
    for index in 0..sides {
        if config::route_on(bits, true, index) {
            if let Some(bus) = a.get(index) {
                routes.push(Wanted { output_node: from.into(), input_node: bus.sink_name.clone() });
            }
        }
        if config::route_on(bits, false, index) {
            if let Some(bus) = b.get(index) {
                routes.push(Wanted { output_node: from.into(), input_node: bus.sink_name.clone() });
            }
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
    if config.mic_source == "builtin" || (config.mic_source.is_empty() && default_source.is_empty()) {
        Graph::BUILTIN_SOURCE.into()
    } else if config.mic_source.is_empty() {
        default_source.to_string()
    } else {
        config.mic_source.clone()
    }
}

fn peaks_of(interleaved: &[f32], bars: usize) -> Vec<f32> {
    let frames = interleaved.len() / 2;
    let mut wave = vec![0.0f32; bars.max(1)];
    if frames == 0 {
        return wave;
    }
    let per = (frames / wave.len()).max(1);
    for (bar, slot) in wave.iter_mut().enumerate() {
        let start = bar * per;
        if start >= frames {
            break;
        }
        let stop = ((bar + 1) * per).min(frames);
        let mut peak = 0.0f32;
        for frame in start..stop {
            peak = peak.max(interleaved[frame * 2].abs());
            peak = peak.max(interleaved[frame * 2 + 1].abs());
        }
        *slot = peak;
    }
    wave
}

fn make_strip(context: &ContextRc, core: &CoreRc, id: &str, label: &str, kind: Kind, capture_target: Option<&str>, any_solo: &Arc<AtomicBool>) -> Result<Strip, String> {
    let base = format!("frame_amix_{id}");
    let module = match kind {
        Kind::Mic => load_loopback(context, &mic_loopback_args(&base, capture_target.unwrap_or(Graph::BUILTIN_SOURCE)))?,
        Kind::Channel => load_loopback(context, &channel_loopback_args(&base, label))?,
        Kind::Fx => load_loopback(context, &fx_loopback_args(&base, label))?,
    };
    let controls = Controls::new();
    let ring = Ring::new();
    let tap = connect_stream(core, &format!("{base}_virtual_sink_tap"), true, TapIo { ring: ring.clone() }, |stream, io| {
        if let Some(samples) = take_f32(stream) {
            io.ring.push(&samples);
        }
    })?;
    let play_io = PlayIo {
        ring,
        proc: Processor::new(),
        applied: Vec::new(),
        gain: 1.0,
        target: controls.gain.clone(),
        mute: controls.mute.clone(),
        solo: controls.solo.clone(),
        hold: controls.hold.clone(),
        any_solo: any_solo.clone(),
        chain: controls.chain.clone(),
        peak: controls.peak.clone(),
    };
    let play = connect_stream(core, &format!("{base}_virtual_sink_dsp"), false, play_io, |stream, io| render_play(stream, io))?;
    Ok(Strip {
        _modules: vec![module],
        _tap: tap,
        _play: play,
        controls,
        base,
        label: label.to_string(),
        capture: capture_target.unwrap_or("").to_string(),
        kind,
    })
}

fn make_listen_bus(context: &ContextRc, name: &str) -> Result<Bus, String> {
    let module = load_loopback(context, &listen_bus_args(name))?;
    Ok(Bus {
        _module: module,
        _source: None,
        sink_name: format!("{name}_virtual_sink"),
        play_name: format!("{name}_virtual_sink_play"),
    })
}

fn make_send_bus(context: &ContextRc, name: &str, source_name: &str, label: &str) -> Result<Bus, String> {
    let module = load_loopback(context, &send_bus_args(name))?;
    let source = load_loopback(context, &source_args(name, source_name, label))?;
    Ok(Bus {
        _module: module,
        _source: Some(source),
        sink_name: format!("{name}_virtual_sink"),
        play_name: format!("{name}_virtual_sink_discard"),
    })
}

fn make_check(core: &CoreRc, share: &Arc<CheckShare>) -> Result<CheckLive, String> {
    let rec = connect_stream(core, "frame_amix_check_virtual_sink_rec", true, RecIo { share: share.clone() }, |stream, io| {
        if !io.share.recording.load(Ordering::Relaxed) {
            return;
        }
        let Some(samples) = take_f32(stream) else { return };
        let Ok(mut buf) = io.share.samples.try_lock() else { return };
        if buf.len() >= CHECK_MAX {
            return;
        }
        let room = CHECK_MAX - buf.len();
        buf.extend(samples.into_iter().take(room));
    })?;
    let play = connect_stream(core, "frame_amix_check_virtual_sink_play", false, PlayIoCheck { share: share.clone() }, |stream, io| {
        render_check_play(stream, io);
    })?;
    Ok(CheckLive { _rec: rec, _play: play })
}

fn render_play(stream: &Stream, io: &mut PlayIo) {
    if let Ok(chain) = io.chain.try_lock() {
        if chain.as_slice() != io.applied.as_slice() {
            io.proc.set_chain(chain.as_slice());
            io.applied = chain.clone();
        }
    }
    let Some(mut buffer) = stream.dequeue_buffer() else { return };
    let datas = buffer.datas_mut();
    let Some(data) = datas.first_mut() else { return };
    let n_bytes = data.chunk().size() as usize;
    let Some(bytes) = data.data() else { return };
    let n = n_bytes.min(bytes.len()) / 4;
    if bytes.as_ptr().align_offset(std::mem::align_of::<f32>()) != 0 || n == 0 {
        return;
    }
    let samples = unsafe { std::slice::from_raw_parts_mut(bytes.as_mut_ptr().cast::<f32>(), n) };
    io.ring.pull(samples.len() / 2, samples);
    io.proc.process(samples);
    let dest = desired_gain(io);
    let mut peak = 0.0f32;
    for sample in samples.iter_mut() {
        io.gain += 0.02 * (dest - io.gain);
        *sample *= io.gain;
        peak = peak.max(sample.abs());
    }
    io.peak.store(peak.to_bits(), Ordering::Relaxed);
}

fn render_check_play(stream: &Stream, io: &mut PlayIoCheck) {
    let Some(mut buffer) = stream.dequeue_buffer() else { return };
    let datas = buffer.datas_mut();
    let Some(data) = datas.first_mut() else { return };
    let n_bytes = data.chunk().size() as usize;
    let Some(bytes) = data.data() else { return };
    let n = n_bytes.min(bytes.len()) / 4;
    if bytes.as_ptr().align_offset(std::mem::align_of::<f32>()) != 0 || n == 0 {
        return;
    }
    let samples = unsafe { std::slice::from_raw_parts_mut(bytes.as_mut_ptr().cast::<f32>(), n) };
    for sample in samples.iter_mut() {
        *sample = 0.0;
    }
    if !io.share.playing.load(Ordering::Relaxed) {
        return;
    }
    let Ok(play) = io.share.play.try_lock() else { return };
    let mut at = io.share.play_at.load(Ordering::Relaxed) as usize;
    if at >= play.len() {
        io.share.playing.store(false, Ordering::Relaxed);
        return;
    }
    for sample in samples.iter_mut() {
        if at >= play.len() {
            io.share.playing.store(false, Ordering::Relaxed);
            break;
        }
        *sample = play[at];
        at += 1;
    }
    io.share.play_at.store(at as u32, Ordering::Relaxed);
}

fn desired_gain(io: &PlayIo) -> f32 {
    let muted = io.mute.load(Ordering::Relaxed)
        || io.hold.load(Ordering::Relaxed)
        || (io.any_solo.load(Ordering::Relaxed) && !io.solo.load(Ordering::Relaxed));
    if muted { 0.0 } else { f32::from_bits(io.target.load(Ordering::Relaxed)) }
}

fn take_f32(stream: &Stream) -> Option<Vec<f32>> {
    let mut buffer = stream.dequeue_buffer()?;
    let datas = buffer.datas_mut();
    let data = datas.first_mut()?;
    let n_bytes = data.chunk().size() as usize;
    let bytes = data.data()?;
    let bytes = &bytes[..n_bytes.min(bytes.len())];
    if bytes.as_ptr().align_offset(std::mem::align_of::<f32>()) != 0 {
        return None;
    }
    let n = bytes.len() / 4;
    let samples = unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<f32>(), n) };
    Some(samples.to_vec())
}

fn connect_stream<T: 'static>(
    core: &CoreRc,
    name: &str,
    capture: bool,
    io: T,
    process: impl Fn(&Stream, &mut T) + 'static,
) -> Result<Live<T>, String> {
    let props = properties! {
        *pipewire::keys::MEDIA_TYPE => "Audio",
        *pipewire::keys::MEDIA_CATEGORY => if capture { "Capture" } else { "Playback" },
        *pipewire::keys::NODE_NAME => name,
        *pipewire::keys::NODE_AUTOCONNECT => "false",
        *pipewire::keys::NODE_PASSIVE => "true",
        "node.virtual" => "true",
        "node.latency" => "480/48000",
        "audio.rate" => "48000",
        "state.restore-props" => "false",
        "node.dont-reconnect" => "true",
    };
    let stream = StreamRc::new(core.clone(), name, props).map_err(|err| err.to_string())?;
    let listener = stream
        .add_local_listener_with_user_data(io)
        .process(process)
        .register()
        .map_err(|err| err.to_string())?;
    let mut audio = libspa::param::audio::AudioInfoRaw::new();
    audio.set_format(libspa::param::audio::AudioFormat::F32LE);
    audio.set_channels(2);
    audio.set_rate(48_000);
    let mut position = [0u32; libspa::sys::SPA_AUDIO_MAX_CHANNELS as usize];
    position[0] = libspa::sys::SPA_AUDIO_CHANNEL_FL;
    position[1] = libspa::sys::SPA_AUDIO_CHANNEL_FR;
    audio.set_position(position);
    let obj = libspa::pod::Object {
        type_: libspa::utils::SpaTypes::ObjectParamFormat.as_raw(),
        id: libspa::param::ParamType::EnumFormat.as_raw(),
        properties: audio.into(),
    };
    let bytes = libspa::pod::serialize::PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &libspa::pod::Value::Object(obj))
        .map_err(|err| err.to_string())?
        .0
        .into_inner();
    let pod = Pod::from_bytes(&bytes).ok_or("format pod")?;
    let mut params = [pod];
    let direction = if capture { libspa::utils::Direction::Input } else { libspa::utils::Direction::Output };
    stream
        .connect(direction, None, StreamFlags::MAP_BUFFERS, &mut params)
        .map_err(|err| err.to_string())?;
    Ok(Live { _stream: stream, _listener: listener })
}

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
            "media.class": "Audio/Sink",
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
            "media.class": "Audio/Sink",
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
            "media.class": "Audio/Sink",
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
