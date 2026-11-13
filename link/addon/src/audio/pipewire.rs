//! Linux: the eight streams as PipeWire nodes.
//!
//! B1–B4 are `Audio/Source` nodes: every app that records sees them as
//! microphones. PC1–PC4 are `Audio/Sink` nodes: every app that plays sees them
//! as outputs. The nodes belong to this process, so they exist while the
//! add-on runs and are gone when it stops; nothing is installed.
//!
//! Each node is a `pw_stream` with a device class. A stream brings PipeWire's
//! own converter with it, so an app may record or play at any rate, channel
//! count or block size while this side always sees 48 kHz stereo float.
//! PulseAudio-API apps find the nodes through `pipewire-pulse` like any card.
//!
//! One thread runs the PipeWire loop. When the daemon is not there yet, or goes
//! away, the thread tears the nodes down and tries again every two seconds.

use std::cell::Cell;
use std::ffi::{c_char, c_void, CStr, CString};
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::Context;
use frame_link_core::{Link, PcmReader, PcmWriter, CHANNELS, RATE};
use libspa::sys as spa;
use pipewire::context::ContextRc;
use pipewire::core::CoreRc;
use pipewire::main_loop::MainLoopRc;
use pipewire::properties::properties;
use pipewire::sys as pw;

use crate::config::Config;
use crate::model::{AudioStatus, DeviceState, Kind, STREAMS};

/// Pause between two attempts to reach PipeWire, or to rebuild broken nodes.
const RETRY: Duration = Duration::from_secs(2);
/// How often the loop looks at the stop flag. Stopping takes at most this long.
const STOP_POLL: Duration = Duration::from_millis(100);
/// How long `start` waits for the nodes to come up, so the first status the
/// add-on reads is the real one. PipeWire answers in a few milliseconds.
const START_WAIT: Duration = Duration::from_secs(1);
/// Bytes of one stereo float frame.
const STRIDE: usize = CHANNELS * std::mem::size_of::<f32>();
/// The first PipeWire whose stream buffers carry `requested`. On an older
/// library that field is memory past the end of the buffer header.
const REQUESTED_SINCE: (u32, u32, u32) = (0, 3, 49);
/// The id PipeWire reports errors of the connection itself under.
const CORE_ID: u32 = 0;

const STATE_MISSING: u8 = 0;
const STATE_READY: u8 = 1;
const STATE_FAILED: u8 = 2;

/// What the loop thread and the owner share.
struct Shared {
    stop: AtomicBool,
    /// Set while the last attempt to reach the daemon failed.
    unreachable: AtomicBool,
    /// One `STATE_*` per node: the microphones first, then the outputs.
    states: [AtomicU8; STREAMS * 2],
}

impl Shared {
    fn set_all(&self, state: u8) {
        for slot in &self.states {
            slot.store(state, Ordering::Relaxed);
        }
    }

    /// True once every node has reported in, one way or the other.
    fn settled(&self) -> bool {
        self.unreachable.load(Ordering::Relaxed) || self.states.iter().all(|state| state.load(Ordering::Relaxed) != STATE_MISSING)
    }
}

pub struct Backend {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Backend {
    /// Starts the loop thread. PipeWire not running is not an error: the
    /// nodes appear once it is.
    ///
    /// # Errors
    /// When the thread cannot be started.
    pub fn start(link: &Link, config: &Config) -> anyhow::Result<Self> {
        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            unreachable: AtomicBool::new(false),
            states: std::array::from_fn(|_| AtomicU8::new(STATE_MISSING)),
        });
        let mics = (0..STREAMS).map(|index| Slot {
            spec: Spec::new(Kind::Mic, index, &config.mic_devices[index]),
            end: End::Mic(link.playback(index)),
        });
        let outputs = (0..STREAMS).map(|index| Slot {
            spec: Spec::new(Kind::Output, index, &config.output_devices[index]),
            end: End::Output(link.capture(index)),
        });
        let slots: Vec<Slot> = mics.chain(outputs).collect();
        let thread_shared = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("frame-link-pipewire".into())
            .spawn(move || run(&thread_shared, slots))
            .context("starting the PipeWire thread")?;
        let deadline = Instant::now() + START_WAIT;
        while !shared.settled() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(Self { shared, thread: Some(thread) })
    }

    #[must_use]
    pub fn status(&self) -> AudioStatus {
        let state = |slot: usize| device_state(self.shared.states[slot].load(Ordering::Relaxed));
        AudioStatus { mics: std::array::from_fn(state), outputs: std::array::from_fn(|index| state(STREAMS + index)) }
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            // Ends the wait between two attempts; a running loop polls the flag.
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}

fn device_state(state: u8) -> DeviceState {
    match state {
        STATE_READY => DeviceState::Ready,
        STATE_FAILED => DeviceState::Failed,
        _ => DeviceState::Missing,
    }
}

/// What one node is called and which way its audio goes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Spec {
    kind: Kind,
    /// `frame_link_b1` … `frame_link_pc4`: the stable name scripts and saved
    /// app settings refer to.
    node_name: String,
    /// The name a person sees in a device list.
    description: String,
}

impl Spec {
    fn new(kind: Kind, index: usize, description: &str) -> Self {
        let node_name = format!("frame_link_{}{}", kind.prefix().to_ascii_lowercase(), index + 1);
        let description = description.trim();
        // An empty name in the settings would leave a nameless device in every list.
        let description = if description.is_empty() { crate::config::device_name(kind, index) } else { description.to_string() };
        Self { kind, node_name, description }
    }

    fn media_class(&self) -> &'static str {
        match self.kind {
            Kind::Mic => "Audio/Source",
            Kind::Output => "Audio/Sink",
        }
    }

    /// A microphone node puts samples out; an output node takes them in.
    fn direction(&self) -> spa::spa_direction {
        match self.kind {
            Kind::Mic => spa::SPA_DIRECTION_OUTPUT,
            Kind::Output => spa::SPA_DIRECTION_INPUT,
        }
    }
}

/// The link's end of one stream: where a node's samples come from or go to.
enum End {
    Mic(PcmReader),
    Output(PcmWriter),
}

/// One stream for the life of the backend. The node built on it comes and
/// goes with the PipeWire session; the ring end stays.
struct Slot {
    spec: Spec,
    end: End,
}

/// Why a session with the daemon ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ended {
    /// The backend is being dropped.
    Stopped,
    /// PipeWire closed the connection: it quit or was restarted.
    DaemonGone,
    /// A node failed. Everything is built again.
    NodeFailed,
}

/// The thread body: one session after another until the backend is dropped.
fn run(shared: &Arc<Shared>, mut slots: Vec<Slot>) {
    pipewire::init();
    // Logged on a change only, so a PC without PipeWire does not fill the log.
    let mut last_problem = String::new();
    while !shared.stop.load(Ordering::Relaxed) {
        match session(shared, &mut slots) {
            Ok(Ended::Stopped) => break,
            Ok(ended) => {
                let reason = if ended == Ended::DaemonGone { "PipeWire went away" } else { "a sound device failed" };
                crate::log::line(&format!("{reason}; trying again every {} s", RETRY.as_secs()));
                last_problem.clear();
            }
            Err(err) => {
                shared.unreachable.store(true, Ordering::Relaxed);
                let problem = format!("{err:#}");
                if problem != last_problem {
                    crate::log::line(&format!("PipeWire is not available ({problem}); trying again every {} s", RETRY.as_secs()));
                    last_problem = problem;
                }
            }
        }
        shared.set_all(STATE_MISSING);
        std::thread::park_timeout(RETRY);
    }
    shared.set_all(STATE_MISSING);
}

/// Connects to PipeWire, creates the nodes and runs the loop until it has a
/// reason to end.
fn session(shared: &Arc<Shared>, slots: &mut [Slot]) -> anyhow::Result<Ended> {
    let mainloop = MainLoopRc::new(None).context("creating the PipeWire loop")?;
    let context = ContextRc::new(&mainloop, Some(properties! { "application.name" => "Frame Audio Link" })).context("creating the PipeWire context")?;
    let core = context.connect_rc(None).context("connecting to the PipeWire daemon")?;
    shared.unreachable.store(false, Ordering::Relaxed);
    let ended = Rc::new(Cell::new(Ended::Stopped));
    let honor_request = library_has_requested();

    let lost_loop = mainloop.clone();
    let lost = Rc::clone(&ended);
    let _core_listener = core
        .add_listener_local()
        .error(move |id, _seq, res, message| {
            if id == CORE_ID && res == -libc::EPIPE {
                lost.set(Ended::DaemonGone);
                lost_loop.quit();
            } else {
                crate::log::line(&format!("PipeWire reported an error on object {id}: {message} ({res})"));
            }
        })
        .register();

    let mut nodes = Vec::with_capacity(slots.len());
    for (index, slot) in slots.iter_mut().enumerate() {
        let name = slot.spec.description.clone();
        match Node::new(&core, slot, Arc::clone(shared), index, honor_request) {
            Ok(node) => nodes.push(node),
            Err(err) => {
                shared.states[index].store(STATE_FAILED, Ordering::Relaxed);
                crate::log::line(&format!("the sound device {name} could not be created: {err:#}"));
            }
        }
    }
    crate::log::line(&format!("connected to PipeWire: {} of {} sound devices created", nodes.len(), STREAMS * 2));

    let poll_loop = mainloop.clone();
    let poll_shared = Arc::clone(shared);
    let poll_ended = Rc::clone(&ended);
    // A failed node is given this long to be noticed in the status before
    // everything is rebuilt.
    let failed_polls = Cell::new(0u32);
    let rebuild_after = (RETRY.as_millis() / STOP_POLL.as_millis()) as u32;
    let timer = mainloop.loop_().add_timer(move |_| {
        if poll_shared.stop.load(Ordering::Relaxed) {
            poll_ended.set(Ended::Stopped);
            poll_loop.quit();
            return;
        }
        let failed = poll_shared.states.iter().any(|state| state.load(Ordering::Relaxed) == STATE_FAILED);
        failed_polls.set(if failed { failed_polls.get() + 1 } else { 0 });
        if failed_polls.get() >= rebuild_after {
            poll_ended.set(Ended::NodeFailed);
            poll_loop.quit();
        }
    });
    timer.update_timer(Some(STOP_POLL), Some(STOP_POLL)).into_result().context("arming the PipeWire timer")?;

    mainloop.run();
    // The nodes go before the connection they were made on.
    drop(nodes);
    Ok(ended.get())
}

/// What the stream callbacks get: the ring end, and where to report state.
struct NodeData {
    stream: *mut pw::pw_stream,
    end: *mut End,
    shared: Arc<Shared>,
    index: usize,
    /// Whether the library running here fills in `pw_buffer::requested`.
    honor_request: bool,
}

/// One live node. It borrows its slot for as long as it exists, so the ring
/// end cannot be touched or dropped while the realtime thread may use it.
struct Node<'slot> {
    raw: NonNull<pw::pw_stream>,
    _hook: Box<spa::spa_hook>,
    _events: Box<pw::pw_stream_events>,
    _data: Box<NodeData>,
    _slot: PhantomData<&'slot mut Slot>,
}

impl<'slot> Node<'slot> {
    fn new(core: &CoreRc, slot: &'slot mut Slot, shared: Arc<Shared>, index: usize, honor_request: bool) -> anyhow::Result<Self> {
        let spec = &slot.spec;
        let props = properties! {
            "media.type" => "Audio",
            "media.class" => spec.media_class(),
            "node.name" => spec.node_name.as_str(),
            "node.description" => spec.description.as_str(),
            "node.nick" => spec.description.as_str(),
            "device.description" => spec.description.as_str(),
            "node.virtual" => "true",
            // Low, so a real microphone or sound card stays the default device.
            "priority.session" => "1",
            "priority.driver" => "1",
            // The node is a device. Nothing should be linked to it by itself.
            "node.autoconnect" => "false",
            "node.dont-reconnect" => "true",
        };
        let name = CString::new(spec.description.as_str()).context("the device name contains a zero byte")?;
        // SAFETY: `core` is a live connection, `name` outlives the call and
        // the stream takes ownership of the properties.
        let raw = unsafe { pw::pw_stream_new(core.as_raw_ptr(), name.as_ptr(), props.into_raw()) };
        let raw = NonNull::new(raw).context("pw_stream_new failed")?;

        let data = Box::new(NodeData { stream: raw.as_ptr(), end: &raw mut slot.end, shared, index, honor_request });
        // SAFETY: an all-zero `pw_stream_events` is valid: every callback is
        // an `Option` and zero is `None`.
        let mut events: Box<pw::pw_stream_events> = Box::new(unsafe { std::mem::zeroed() });
        events.version = pw::PW_VERSION_STREAM_EVENTS;
        events.state_changed = Some(on_state_changed);
        events.process = Some(on_process);
        // SAFETY: an all-zero `spa_hook` is the unlinked state the listener
        // call expects.
        let hook: Box<spa::spa_hook> = Box::new(unsafe { std::mem::zeroed() });
        // Owned from here on, so an error below destroys the stream.
        let mut node = Self { raw, _hook: hook, _events: events, _data: data, _slot: PhantomData };
        // SAFETY: hook, events and data are boxed and live as long as the
        // node; the stream is destroyed (which unhooks them) before they drop.
        unsafe {
            let data: *mut NodeData = &raw mut *node._data;
            pw::pw_stream_add_listener(raw.as_ptr(), &raw mut *node._hook, &raw const *node._events, data.cast());
        }

        let format = format_pod()?;
        let pod = libspa::pod::Pod::from_bytes(&format).context("the audio format could not be encoded")?;
        let mut params = [pod.as_raw_ptr().cast_const()];
        // No AUTOCONNECT: apps link to the node, it does not go looking for one.
        let flags = pw::pw_stream_flags_PW_STREAM_FLAG_MAP_BUFFERS | pw::pw_stream_flags_PW_STREAM_FLAG_RT_PROCESS;
        // SAFETY: `raw` is a live stream and `params` holds one valid pod for
        // the duration of the call.
        let rc = unsafe { pw::pw_stream_connect(raw.as_ptr(), spec.direction(), pw::PW_ID_ANY, flags, params.as_mut_ptr(), 1) };
        anyhow::ensure!(rc >= 0, "pw_stream_connect failed ({rc})");
        Ok(node)
    }
}

impl Drop for Node<'_> {
    fn drop(&mut self) {
        // SAFETY: created by `pw_stream_new` and destroyed exactly once.
        // Destroying waits for the realtime thread to leave the callbacks.
        unsafe { pw::pw_stream_destroy(self.raw.as_ptr()) };
    }
}

/// The one format every node offers: 48 kHz stereo float, left then right.
fn format_pod() -> anyhow::Result<Vec<u8>> {
    use libspa::param::audio::{AudioFormat, AudioInfoRaw, MAX_CHANNELS};
    use libspa::pod::serialize::PodSerializer;
    use libspa::pod::{Object, Value};

    let mut info = AudioInfoRaw::new();
    info.set_format(AudioFormat::F32LE);
    info.set_rate(RATE);
    info.set_channels(CHANNELS as u32);
    let mut position = [0; MAX_CHANNELS];
    position[0] = spa::SPA_AUDIO_CHANNEL_FL;
    position[1] = spa::SPA_AUDIO_CHANNEL_FR;
    info.set_position(position);
    let object = Value::Object(Object { type_: spa::SPA_TYPE_OBJECT_Format, id: spa::SPA_PARAM_EnumFormat, properties: info.into() });
    let (cursor, _) = PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &object).map_err(|err| anyhow::anyhow!("encoding the audio format: {err:?}"))?;
    Ok(cursor.into_inner())
}

/// Maps a stream state to what the overlay shows. `None` leaves it as it is.
fn node_state(state: pw::pw_stream_state) -> Option<u8> {
    match state {
        pw::pw_stream_state_PW_STREAM_STATE_PAUSED | pw::pw_stream_state_PW_STREAM_STATE_STREAMING => Some(STATE_READY),
        pw::pw_stream_state_PW_STREAM_STATE_ERROR => Some(STATE_FAILED),
        pw::pw_stream_state_PW_STREAM_STATE_UNCONNECTED => Some(STATE_MISSING),
        // Connecting: not there yet, and not gone either.
        _ => None,
    }
}

/// Called on the loop thread when a node comes up, idles, runs or breaks.
unsafe extern "C" fn on_state_changed(data: *mut c_void, _old: pw::pw_stream_state, state: pw::pw_stream_state, error: *const c_char) {
    // SAFETY: `data` is the `NodeData` registered with this listener.
    let Some(data) = (unsafe { data.cast::<NodeData>().as_ref() }) else { return };
    if let Some(mapped) = node_state(state) {
        data.shared.states[data.index].store(mapped, Ordering::Relaxed);
    }
    if state == pw::pw_stream_state_PW_STREAM_STATE_ERROR && !error.is_null() {
        // SAFETY: PipeWire passes a NUL-terminated string that lives for the call.
        let message = unsafe { CStr::from_ptr(error) }.to_string_lossy();
        crate::log::line(&format!("sound device {} failed: {message}", data.index + 1));
    }
}

/// Parses a library version such as `1.0.5` or `0.3.48`. Anything after the
/// third number is ignored; a number that is missing counts as zero.
fn parse_version(text: &str) -> (u32, u32, u32) {
    let mut numbers = text.split(['.', '-', ' ']).map(|part| part.parse::<u32>().unwrap_or(0));
    (numbers.next().unwrap_or(0), numbers.next().unwrap_or(0), numbers.next().unwrap_or(0))
}

/// Whether the PipeWire library loaded into this process is new enough to
/// say how many frames it wants. The add-on is built against newer headers
/// than some systems run, so the struct layout has to be checked at run time.
fn library_has_requested() -> bool {
    // SAFETY: returns a static NUL-terminated string, or null.
    let version = unsafe { pw::pw_get_library_version() };
    if version.is_null() {
        return false;
    }
    // SAFETY: checked for null above; the string lives as long as the library.
    let version = unsafe { CStr::from_ptr(version) }.to_string_lossy();
    parse_version(&version) >= REQUESTED_SINCE
}

/// Frames to put into a buffer that holds `capacity` of them. PipeWire asks
/// for a number when its converter needs one; otherwise the buffer is filled.
fn frames_to_fill(requested: u64, capacity: usize) -> usize {
    match usize::try_from(requested) {
        Ok(0) | Err(_) => capacity,
        Ok(requested) => requested.min(capacity),
    }
}

/// The part of a buffer of `maxsize` bytes a chunk describes, as a byte
/// offset and a count of whole frames. A chunk that claims more than the
/// buffer holds is cut to what is there.
fn chunk_frames(offset: u32, size: u32, maxsize: u32) -> (usize, usize) {
    if maxsize == 0 {
        return (0, 0);
    }
    let offset = (offset % maxsize) as usize;
    let size = (size as usize).min(maxsize as usize - offset);
    (offset, size / STRIDE)
}

/// One graph cycle of one node, on PipeWire's realtime thread. It moves
/// samples between the stream's buffer and the link's ring and nothing else:
/// no allocation, no lock, no system call that can block.
unsafe extern "C" fn on_process(data: *mut c_void) {
    // SAFETY: `data` is the `NodeData` registered with this listener. Only
    // this callback uses the ring end while the node exists (see `Node`).
    let Some(data) = (unsafe { data.cast::<NodeData>().as_mut() }) else { return };
    // SAFETY: the stream is alive for as long as its callbacks run.
    let buffer = unsafe { pw::pw_stream_dequeue_buffer(data.stream) };
    if buffer.is_null() {
        return;
    }
    // SAFETY: a dequeued buffer and its first data block are valid until the
    // buffer is queued again; the pointers are checked before use.
    unsafe {
        let spa_buffer = (*buffer).buffer;
        if !spa_buffer.is_null() && (*spa_buffer).n_datas > 0 && !(*spa_buffer).datas.is_null() {
            let block = &mut *(*spa_buffer).datas;
            let samples = block.data.cast::<f32>();
            if !samples.is_null() && samples.is_aligned() && !block.chunk.is_null() {
                match &mut *data.end {
                    End::Mic(reader) => {
                        let requested = if data.honor_request { (*buffer).requested } else { 0 };
                        let frames = frames_to_fill(requested, block.maxsize as usize / STRIDE);
                        reader.read_interleaved(std::slice::from_raw_parts_mut(samples, frames * CHANNELS));
                        let chunk = &mut *block.chunk;
                        chunk.offset = 0;
                        chunk.stride = STRIDE as i32;
                        chunk.size = (frames * STRIDE) as u32;
                        (*buffer).size = frames as u64;
                    }
                    End::Output(writer) => {
                        let chunk = &*block.chunk;
                        let (offset, frames) = chunk_frames(chunk.offset, chunk.size, block.maxsize);
                        let start = block.data.cast::<u8>().add(offset).cast::<f32>();
                        if start.is_aligned() {
                            writer.write_interleaved(std::slice::from_raw_parts(start, frames * CHANNELS));
                        }
                    }
                }
            }
        }
        pw::pw_stream_queue_buffer(data.stream, buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn microphone_should_be_a_source_node() {
        let spec = Spec::new(Kind::Mic, 0, "Frame B1");

        assert_eq!((spec.media_class(), spec.node_name.as_str(), spec.direction()), ("Audio/Source", "frame_link_b1", spa::SPA_DIRECTION_OUTPUT));
    }

    #[test]
    fn output_should_be_a_sink_node() {
        let spec = Spec::new(Kind::Output, 3, "Frame PC4");

        assert_eq!((spec.media_class(), spec.node_name.as_str(), spec.direction()), ("Audio/Sink", "frame_link_pc4", spa::SPA_DIRECTION_INPUT));
    }

    #[test]
    fn description_should_come_from_the_settings() {
        let spec = Spec::new(Kind::Mic, 1, "  Discord mic ");

        assert_eq!(spec.description, "Discord mic");
    }

    #[test]
    fn empty_description_should_fall_back_to_the_stream_name() {
        let spec = Spec::new(Kind::Output, 1, "  ");

        assert_eq!(spec.description, "Steam Streaming Out PC2");
    }

    #[test]
    fn running_and_idle_nodes_should_be_ready() {
        let states = [pw::pw_stream_state_PW_STREAM_STATE_PAUSED, pw::pw_stream_state_PW_STREAM_STATE_STREAMING].map(node_state);

        assert_eq!(states, [Some(STATE_READY), Some(STATE_READY)]);
    }

    #[test]
    fn broken_node_should_be_failed() {
        assert_eq!(node_state(pw::pw_stream_state_PW_STREAM_STATE_ERROR), Some(STATE_FAILED));
    }

    #[test]
    fn connecting_node_should_keep_its_state() {
        assert_eq!(node_state(pw::pw_stream_state_PW_STREAM_STATE_CONNECTING), None);
    }

    fn shared(states: [u8; STREAMS * 2], unreachable: bool) -> Shared {
        Shared { stop: AtomicBool::new(false), unreachable: AtomicBool::new(unreachable), states: states.map(AtomicU8::new) }
    }

    #[test]
    fn start_should_keep_waiting_while_a_node_is_still_coming_up() {
        let mut states = [STATE_READY; STREAMS * 2];
        states[5] = STATE_MISSING;

        assert!(!shared(states, false).settled());
    }

    #[test]
    fn start_should_stop_waiting_once_every_node_reported() {
        let mut states = [STATE_READY; STREAMS * 2];
        states[2] = STATE_FAILED;

        assert!(shared(states, false).settled());
    }

    #[test]
    fn start_should_not_wait_for_a_daemon_that_is_not_there() {
        assert!(shared([STATE_MISSING; STREAMS * 2], true).settled());
    }

    #[test]
    fn unknown_state_byte_should_read_as_missing() {
        assert_eq!(device_state(77), DeviceState::Missing);
    }

    #[test]
    fn release_version_should_parse_to_three_numbers() {
        assert_eq!(parse_version("1.0.5"), (1, 0, 5));
    }

    #[test]
    fn version_with_a_suffix_should_ignore_the_suffix() {
        assert_eq!(parse_version("0.3.48-rc1"), (0, 3, 48));
    }

    #[test]
    fn library_before_0_3_49_should_not_be_asked_for_a_request() {
        assert!(parse_version("0.3.48") < REQUESTED_SINCE);
    }

    #[test]
    fn library_from_0_3_49_on_should_be_asked_for_a_request() {
        assert!(parse_version("0.3.49") >= REQUESTED_SINCE && parse_version("1.2.0") >= REQUESTED_SINCE);
    }

    #[test]
    fn buffer_without_a_request_should_be_filled() {
        assert_eq!(frames_to_fill(0, 1024), 1024);
    }

    #[test]
    fn request_should_be_cut_to_the_buffer() {
        assert_eq!((frames_to_fill(480, 1024), frames_to_fill(4096, 1024)), (480, 1024));
    }

    #[test]
    fn chunk_should_give_whole_frames() {
        assert_eq!(chunk_frames(16, 100, 4096), (16, 12));
    }

    #[test]
    fn chunk_past_the_buffer_should_be_cut() {
        assert_eq!(chunk_frames(4000, 8192, 4096), (4000, 12));
    }

    #[test]
    fn chunk_of_an_empty_buffer_should_be_empty() {
        assert_eq!(chunk_frames(0, 64, 0), (0, 0));
    }

    #[test]
    fn format_should_encode_to_a_pod() -> anyhow::Result<()> {
        let bytes = format_pod()?;

        assert!(libspa::pod::Pod::from_bytes(&bytes).is_some());
        Ok(())
    }
}
