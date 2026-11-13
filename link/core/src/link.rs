//! The link thread: finds the other end, keeps the session alive, and moves
//! coded frames between the socket and the sample rings.

use std::hash::{BuildHasher, Hasher};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::codec::CodecError;
use crate::pcm::{capture_ring, playback_ring, CaptureTap, PcmReader, PcmWriter, PlaybackFeed, Waker};
use crate::playout::Playout;
pub use crate::playout::StreamStats;
use crate::proto::{self, Hello, Packet, Refuse, State, Welcome, PROTOCOL_VERSION};
use crate::sender::{Coded, StreamSender};
use crate::{mask_has, Mask, STREAMS};

/// The PC asks again this often while it has no headset.
const PROBE_EVERY: Duration = Duration::from_secs(1);
/// After the first answer, the PC waits this long for answers over its other
/// networks before it picks the fastest.
const COLLECT_ANSWERS: Duration = Duration::from_millis(150);
/// Each end says what it wants this often, and at once when that changes.
const STATE_EVERY: Duration = Duration::from_millis(500);
/// Silence from the other end for this long ends the session.
const PEER_TIMEOUT: Duration = Duration::from_secs(3);
/// How long a refusal stays on display after the headset sent it.
const REFUSAL_SHOWN: Duration = Duration::from_secs(4);
/// The status other threads read is refreshed this often.
const PUBLISH_EVERY: Duration = Duration::from_millis(100);
/// Datagrams waiting between the socket thread and the link thread.
const DATAGRAM_QUEUE: usize = 256;
/// Largest datagram the socket thread accepts.
const RECV_BUFFER: usize = 2048;

/// Addresses the headset might answer at. Asked before every probe, so a
/// network that comes up later is found.
pub type Candidates = Box<dyn FnMut() -> Vec<SocketAddr> + Send>;
/// Whether the headset should talk to a PC at this address.
pub type AllowPeer = Box<dyn FnMut(IpAddr) -> bool + Send>;

/// Which end of the link this is.
pub enum Role {
    /// The PC. It looks for the headset and decides which streams are on.
    Host { candidates: Candidates },
    /// The headset. It waits for a PC it is allowed to talk to.
    Frame { allow: AllowPeer },
}

pub struct LinkConfig {
    pub role: Role,
    /// Shown at the other end: the computer's or the headset's name.
    pub name: String,
    /// The port the headset listens on.
    pub port: u16,
    /// Opus bit rate of one stereo stream, in bits per second.
    pub bitrate: i32,
    /// Opus effort, 0–10. Higher sounds the same at this bit rate on less
    /// favourable material and costs more CPU.
    pub complexity: i32,
    /// Sends every frame a second time with the next one. One lost packet is
    /// then inaudible, for about twice the bit rate.
    pub redundancy: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("the link port could not be opened: {0}")]
    Bind(#[source] std::io::Error),
    #[error(transparent)]
    Codec(#[from] CodecError),
    #[error("the link thread could not be started: {0}")]
    Spawn(#[source] std::io::Error),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkState {
    /// No session. The PC is looking; the headset is waiting.
    #[default]
    Searching,
    Connected,
    /// The headset answered and said no. `LinkStatus::refused` says why.
    Refused,
}

/// What the link is doing, for a user interface.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LinkStatus {
    pub state: LinkState,
    /// Name of the other end.
    pub peer: String,
    pub peer_addr: String,
    /// Why the headset refused: `busy`, `version` or `not-allowed`.
    pub refused: String,
    /// Round trip measured when the session was set up.
    pub rtt_ms: f32,
    /// Streams this end would send, and wants to receive.
    pub local_send: Mask,
    pub local_recv: Mask,
    /// Streams the other end would send, and wants to receive.
    pub remote_send: Mask,
    pub remote_recv: Mask,
    /// Streams carrying sound right now, out and in.
    pub sending: Mask,
    pub receiving: Mask,
    /// The other end's names for the streams it sends.
    pub names: Vec<String>,
    /// One entry per incoming stream.
    pub streams: [StreamStats; STREAMS],
}

impl LinkStatus {
    #[must_use]
    pub fn connected(&self) -> bool {
        self.state == LinkState::Connected
    }
}

enum Command {
    Capture(usize, CaptureTap),
    Playback(usize, PlaybackFeed),
}

/// What the owner sets and reads while the link thread runs.
struct Shared {
    stop: AtomicBool,
    send: AtomicU8,
    recv: AtomicU8,
    names: Mutex<Vec<String>>,
    status: Mutex<LinkStatus>,
}

/// A running link. Dropping it says goodbye to the other end and stops.
pub struct Link {
    shared: Arc<Shared>,
    commands: Sender<Command>,
    waker: Waker,
    port: u16,
    threads: Vec<JoinHandle<()>>,
}

impl Link {
    /// Opens the socket and starts the link.
    ///
    /// # Errors
    /// When the port is taken, the codec cannot be set up, or a thread cannot
    /// be started.
    pub fn spawn(config: LinkConfig) -> Result<Self, LinkError> {
        let bind_port = match config.role {
            Role::Host { .. } => 0,
            Role::Frame { .. } => config.port,
        };
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, bind_port)).map_err(LinkError::Bind)?;
        socket.set_broadcast(true).map_err(LinkError::Bind)?;
        let port = socket.local_addr().map_err(LinkError::Bind)?.port();
        let listener = socket.try_clone().map_err(LinkError::Bind)?;
        // The socket thread looks at the stop flag between reads.
        listener.set_read_timeout(Some(Duration::from_millis(250))).map_err(LinkError::Bind)?;

        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            send: AtomicU8::new(0),
            recv: AtomicU8::new(0),
            names: Mutex::new(Vec::new()),
            status: Mutex::new(LinkStatus::default()),
        });
        let (commands, command_rx) = mpsc::channel();
        let (datagram_tx, datagram_rx) = mpsc::sync_channel(DATAGRAM_QUEUE);
        let runner = Runner::new(config, socket, Arc::clone(&shared), command_rx, datagram_rx)?;

        let link_thread = std::thread::Builder::new().name("frame-link".into()).spawn(move || runner.run()).map_err(LinkError::Spawn)?;
        let waker = Waker::new(link_thread.thread().clone());
        let listen_shared = Arc::clone(&shared);
        let listen_waker = waker.clone();
        let listen_thread = std::thread::Builder::new()
            .name("frame-link-recv".into())
            .spawn(move || listen(&listener, &listen_shared, &datagram_tx, &listen_waker));
        let listen_thread = match listen_thread {
            Ok(thread) => thread,
            Err(err) => {
                shared.stop.store(true, Ordering::Relaxed);
                waker.wake();
                let _ = link_thread.join();
                return Err(LinkError::Spawn(err));
            }
        };
        Ok(Self { shared, commands, waker, port, threads: vec![link_thread, listen_thread] })
    }

    /// The UDP port this end is bound to.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Hands out the writer for outgoing stream `stream`, replacing any
    /// earlier one. Out of range, the writer is detached from the start.
    #[must_use]
    pub fn capture(&self, stream: usize) -> PcmWriter {
        let (writer, tap) = capture_ring(self.waker.clone());
        if stream < STREAMS {
            let _ = self.commands.send(Command::Capture(stream, tap));
            self.waker.wake();
        }
        writer
    }

    /// Hands out the reader for incoming stream `stream`, replacing any
    /// earlier one. Out of range, the reader only ever reads silence.
    #[must_use]
    pub fn playback(&self, stream: usize) -> PcmReader {
        let (feed, reader) = playback_ring(self.waker.clone());
        if stream < STREAMS {
            let _ = self.commands.send(Command::Playback(stream, feed));
            self.waker.wake();
        }
        reader
    }

    /// Which streams this end would send and wants to receive. A stream flows
    /// only while the sender would send it and the receiver wants it.
    pub fn set_masks(&self, send: Mask, recv: Mask) {
        let send_changed = self.shared.send.swap(send, Ordering::Relaxed) != send;
        let recv_changed = self.shared.recv.swap(recv, Ordering::Relaxed) != recv;
        if send_changed || recv_changed {
            self.waker.wake();
        }
    }

    /// Names of the streams this end sends, shown at the other end.
    pub fn set_names(&self, names: Vec<String>) {
        if let Ok(mut current) = self.shared.names.lock() {
            if *current != names {
                *current = names;
                self.waker.wake();
            }
        }
    }

    #[must_use]
    pub fn status(&self) -> LinkStatus {
        self.shared.status.lock().map(|status| status.clone()).unwrap_or_default()
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.waker.wake();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

/// The socket thread: blocks on the socket and hands datagrams to the link.
fn listen(socket: &UdpSocket, shared: &Shared, datagrams: &SyncSender<(SocketAddr, Vec<u8>)>, waker: &Waker) {
    let mut buffer = [0u8; RECV_BUFFER];
    while !shared.stop.load(Ordering::Relaxed) {
        match socket.recv_from(&mut buffer) {
            Ok((len, from)) => {
                match datagrams.try_send((from, buffer[..len].to_vec())) {
                    // A full queue means the link thread is stalled; audio that old is useless.
                    Ok(()) | Err(TrySendError::Full(_)) => waker.wake(),
                    Err(TrySendError::Disconnected(_)) => break,
                }
            }
            Err(err) if matches!(err.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            // Windows reports an ICMP "port unreachable" for an earlier send as
            // an error on the next read. The socket is fine; the pause only
            // keeps a socket that is truly broken from spinning.
            Err(_) => std::thread::sleep(Duration::from_millis(5)),
        }
    }
}

struct Peer {
    addr: SocketAddr,
    session: u32,
    name: String,
    /// The PC's run id. Zero on the PC's own side.
    host: u64,
    rtt: Duration,
    last_heard: Instant,
}

/// The PC's search for the headset.
struct Search {
    host: u64,
    candidates: Candidates,
    next_probe: Instant,
    nonce: u64,
    sent_at: Instant,
    /// The fastest answer so far and when to stop waiting for a faster one.
    best: Option<(SocketAddr, Welcome, Duration)>,
    decide_at: Option<Instant>,
    refused: Option<(Instant, String)>,
    /// Where the headset was last time. Asked first.
    last_addr: Option<SocketAddr>,
}

enum Side {
    Host(Box<Search>),
    Frame { allow: AllowPeer, last_bye: Option<Instant> },
}

struct Runner {
    socket: UdpSocket,
    shared: Arc<Shared>,
    commands: Receiver<Command>,
    datagrams: Receiver<(SocketAddr, Vec<u8>)>,
    side: Side,
    name: String,
    port: u16,
    redundancy: bool,
    peer: Option<Peer>,
    remote: State,
    senders: Vec<StreamSender>,
    playouts: Vec<Playout>,
    /// The newest frame of each stream, sent again with the one after it.
    previous: [Option<Coded>; STREAMS],
    coded: Vec<Coded>,
    wire: Vec<u8>,
    sent_state: Option<State>,
    state_sent_at: Instant,
    published: LinkStatus,
    published_at: Instant,
}

impl Runner {
    fn new(
        config: LinkConfig,
        socket: UdpSocket,
        shared: Arc<Shared>,
        commands: Receiver<Command>,
        datagrams: Receiver<(SocketAddr, Vec<u8>)>,
    ) -> Result<Self, CodecError> {
        let now = Instant::now();
        let side = match config.role {
            Role::Host { candidates } => Side::Host(Box::new(Search {
                host: random_u64() | 1,
                candidates,
                next_probe: now,
                nonce: 0,
                sent_at: now,
                best: None,
                decide_at: None,
                refused: None,
                last_addr: None,
            })),
            Role::Frame { allow } => Side::Frame { allow, last_bye: None },
        };
        let senders = (0..STREAMS).map(|stream| StreamSender::new(stream, config.bitrate, config.complexity)).collect::<Result<_, _>>()?;
        let playouts = (0..STREAMS).map(|_| Playout::new(now)).collect::<Result<_, _>>()?;
        Ok(Self {
            socket,
            shared,
            commands,
            datagrams,
            side,
            name: config.name,
            port: config.port,
            redundancy: config.redundancy,
            peer: None,
            remote: State::default(),
            senders,
            playouts,
            previous: [const { None }; STREAMS],
            coded: Vec::with_capacity(STREAMS * 2),
            wire: Vec::with_capacity(proto::MAX_DATAGRAM),
            sent_state: None,
            state_sent_at: now,
            published: LinkStatus::default(),
            published_at: now,
        })
    }

    fn run(mut self) {
        while !self.shared.stop.load(Ordering::Relaxed) {
            let now = Instant::now();
            self.take_commands();
            self.take_datagrams(now);
            self.keep_session(now);
            if self.peer.is_some() {
                self.send_audio(now);
                for playout in &mut self.playouts {
                    playout.pump(now);
                }
                self.send_state(now);
            }
            self.publish(now);
            // Audio threads and the socket thread wake this thread; the timeout
            // only covers what no one announces, such as a peer going quiet.
            let nap = if self.peer.is_some() { Duration::from_millis(10) } else { Duration::from_millis(100) };
            std::thread::park_timeout(nap);
        }
        if let Some(peer) = &self.peer {
            proto::write_bye(&mut self.wire, peer.session);
            let _ = self.socket.send_to(&self.wire, peer.addr);
        }
    }

    fn take_commands(&mut self) {
        while let Ok(command) = self.commands.try_recv() {
            match command {
                Command::Capture(stream, tap) => {
                    if let Some(sender) = self.senders.get_mut(stream) {
                        sender.attach(tap);
                    }
                }
                Command::Playback(stream, feed) => {
                    feed.set_live(self.peer.is_some());
                    if let Some(playout) = self.playouts.get_mut(stream) {
                        playout.attach(feed);
                    }
                }
            }
        }
    }

    fn take_datagrams(&mut self, now: Instant) {
        while let Ok((from, datagram)) = self.datagrams.try_recv() {
            let Ok((session, packet)) = proto::parse(&datagram) else { continue };
            match packet {
                Packet::Hello(hello) => self.on_hello(from, &hello, now),
                Packet::Welcome(welcome) => self.on_welcome(from, welcome, now),
                Packet::Refuse(refuse) => self.on_refuse(refuse, now),
                Packet::Bye => {
                    if self.is_from_peer(from, session) {
                        self.disconnect(now);
                    }
                }
                Packet::State(state) => {
                    if self.accept(from, session, now) {
                        // The end marker of a run travels with its last frames.
                        // Should that packet be lost, this report ends the run.
                        let stopped = self.remote.active & !state.active;
                        for (stream, playout) in self.playouts.iter_mut().enumerate() {
                            if mask_has(stopped, stream) {
                                playout.end_after_pending();
                            }
                        }
                        self.remote = state;
                    }
                }
                Packet::Audio(blobs) => {
                    if self.accept(from, session, now) {
                        let wanted = self.shared.recv.load(Ordering::Relaxed);
                        for blob in blobs.filter(|blob| mask_has(wanted, blob.stream)) {
                            self.playouts[blob.stream].push(&blob, now);
                        }
                    }
                }
            }
        }
    }

    /// True when the packet belongs to the current session. The headset then
    /// follows the PC to whichever of its addresses it sent from.
    fn accept(&mut self, from: SocketAddr, session: u32, now: Instant) -> bool {
        if !self.is_from_peer(from, session) {
            self.turn_away(from, session, now);
            return false;
        }
        if let Some(peer) = self.peer.as_mut() {
            peer.last_heard = now;
            if matches!(self.side, Side::Frame { .. }) {
                peer.addr = from;
            }
        }
        true
    }

    fn is_from_peer(&mut self, from: SocketAddr, session: u32) -> bool {
        let Some(peer) = &self.peer else { return false };
        if peer.session != session {
            return false;
        }
        if peer.addr.ip() == from.ip() {
            return true;
        }
        // The same PC may reach the headset over a second network.
        match &mut self.side {
            Side::Frame { allow, .. } => allow(from.ip()),
            Side::Host(_) => false,
        }
    }

    /// Tells a PC that still talks into a session this headset no longer has
    /// (the mixer was restarted) to start over, at most once a second.
    fn turn_away(&mut self, from: SocketAddr, session: u32, now: Instant) {
        let Side::Frame { last_bye, .. } = &mut self.side else { return };
        if session == 0 || last_bye.is_some_and(|at| now.duration_since(at) < Duration::from_secs(1)) {
            return;
        }
        *last_bye = Some(now);
        proto::write_bye(&mut self.wire, session);
        let _ = self.socket.send_to(&self.wire, from);
    }

    fn on_hello(&mut self, from: SocketAddr, hello: &Hello, now: Instant) {
        let Side::Frame { allow, .. } = &mut self.side else { return };
        let refusal = if hello.v != PROTOCOL_VERSION {
            Some(proto::REFUSE_VERSION)
        } else if !allow(from.ip()) {
            Some(proto::REFUSE_NOT_ALLOWED)
        } else if self.peer.as_ref().is_some_and(|peer| peer.host != hello.host) {
            // One PC at a time. A PC that went away is forgotten after the timeout.
            Some(proto::REFUSE_BUSY)
        } else {
            None
        };
        if let Some(reason) = refusal {
            proto::write_refuse(&mut self.wire, &Refuse { reason: reason.into(), nonce: hello.nonce });
            let _ = self.socket.send_to(&self.wire, from);
            return;
        }
        if self.peer.is_none() {
            self.connect(Peer { addr: from, session: random_session(), name: hello.name.clone(), host: hello.host, rtt: Duration::ZERO, last_heard: now });
        }
        let Some(peer) = self.peer.as_mut() else { return };
        peer.last_heard = now;
        let welcome = Welcome { v: PROTOCOL_VERSION, name: self.name.clone(), session: peer.session, nonce: hello.nonce };
        proto::write_welcome(&mut self.wire, &welcome);
        let _ = self.socket.send_to(&self.wire, from);
    }

    fn on_welcome(&mut self, from: SocketAddr, welcome: Welcome, now: Instant) {
        let Side::Host(search) = &mut self.side else { return };
        if self.peer.is_some() || welcome.nonce != search.nonce || welcome.v != PROTOCOL_VERSION {
            return;
        }
        let rtt = now.duration_since(search.sent_at);
        if search.best.as_ref().is_none_or(|(_, _, best)| rtt < *best) {
            search.best = Some((from, welcome, rtt));
        }
        search.decide_at.get_or_insert(now + COLLECT_ANSWERS);
        search.refused = None;
    }

    fn on_refuse(&mut self, refuse: Refuse, now: Instant) {
        let Side::Host(search) = &mut self.side else { return };
        if refuse.nonce == search.nonce && self.peer.is_none() {
            search.refused = Some((now, refuse.reason));
        }
    }

    /// Probes for the headset, settles on an answer, and ends a session whose
    /// other end went quiet.
    fn keep_session(&mut self, now: Instant) {
        if self.peer.as_ref().is_some_and(|peer| now.duration_since(peer.last_heard) > PEER_TIMEOUT) {
            self.disconnect(now);
        }
        if self.peer.is_some() {
            return;
        }
        let Side::Host(search) = &mut self.side else { return };
        if search.decide_at.is_some_and(|at| now >= at) {
            search.decide_at = None;
            if let Some((addr, welcome, rtt)) = search.best.take() {
                search.last_addr = Some(addr);
                self.connect(Peer { addr, session: welcome.session, name: welcome.name, host: 0, rtt, last_heard: now });
                return;
            }
        }
        if now < search.next_probe {
            return;
        }
        search.next_probe = now + PROBE_EVERY;
        search.nonce = random_u64();
        search.sent_at = now;
        search.best = None;
        search.decide_at = None;
        let hello = Hello { v: PROTOCOL_VERSION, host: search.host, name: self.name.clone(), nonce: search.nonce };
        proto::write_hello(&mut self.wire, &hello);
        let mut targets: Vec<SocketAddr> = search.last_addr.into_iter().collect();
        for addr in (search.candidates)() {
            // A candidate without a port means the default one.
            let addr = if addr.port() == 0 { SocketAddr::new(addr.ip(), self.port) } else { addr };
            if !targets.contains(&addr) {
                targets.push(addr);
            }
        }
        for addr in targets {
            let _ = self.socket.send_to(&self.wire, addr);
        }
    }

    fn connect(&mut self, peer: Peer) {
        self.peer = Some(peer);
        self.remote = State::default();
        self.sent_state = None;
        for playout in &mut self.playouts {
            playout.set_live(true);
        }
    }

    fn disconnect(&mut self, now: Instant) {
        self.peer = None;
        self.remote = State::default();
        self.previous = [const { None }; STREAMS];
        for sender in &mut self.senders {
            sender.stop();
        }
        for playout in &mut self.playouts {
            playout.stop();
            playout.set_live(false);
        }
        if let Side::Host(search) = &mut self.side {
            search.next_probe = now;
        }
    }

    fn local_state(&self) -> State {
        let active = self.senders.iter().enumerate().fold(0, |mask, (stream, sender)| mask | (Mask::from(sender.is_active()) << stream));
        State {
            send: self.shared.send.load(Ordering::Relaxed),
            recv: self.shared.recv.load(Ordering::Relaxed),
            active,
            names: self.shared.names.lock().map(|names| names.clone()).unwrap_or_default(),
        }
    }

    fn send_state(&mut self, now: Instant) {
        let Some(peer) = &self.peer else { return };
        let state = self.local_state();
        let due = now.duration_since(self.state_sent_at) >= STATE_EVERY;
        if !due && self.sent_state.as_ref() == Some(&state) {
            return;
        }
        proto::write_state(&mut self.wire, peer.session, &state);
        let _ = self.socket.send_to(&self.wire, peer.addr);
        self.sent_state = Some(state);
        self.state_sent_at = now;
    }

    /// Codes what the audio threads queued and sends it, with the frame before
    /// it once more where that still fits the datagram.
    fn send_audio(&mut self, now: Instant) {
        let Some(peer) = &self.peer else { return };
        let (session, addr) = (peer.session, peer.addr);
        let wanted = self.shared.send.load(Ordering::Relaxed) & self.remote.recv;
        self.coded.clear();
        for (stream, sender) in self.senders.iter_mut().enumerate() {
            sender.poll(mask_has(wanted, stream), now, &mut self.coded);
        }
        if self.coded.is_empty() {
            return;
        }
        proto::audio_begin(&mut self.wire, session);
        for coded in &self.coded {
            let blob = coded.blob();
            if !proto::audio_fits(&self.wire, &blob) {
                let _ = self.socket.send_to(&self.wire, addr);
                proto::audio_begin(&mut self.wire, session);
            }
            proto::audio_push(&mut self.wire, &blob);
        }
        if self.redundancy {
            for (stream, previous) in self.previous.iter().enumerate() {
                let Some(previous) = previous else { continue };
                let first_new = self.coded.iter().find(|coded| coded.stream == stream);
                // Only the frame directly before the new one: an older run is over.
                let follows = first_new.is_some_and(|coded| coded.seq == previous.seq.wrapping_add(1) && coded.flags & proto::BLOB_START == 0);
                let blob = previous.repeat_blob();
                if follows && proto::audio_fits(&self.wire, &blob) {
                    proto::audio_push(&mut self.wire, &blob);
                }
            }
        }
        if !proto::audio_is_empty(&self.wire) {
            let _ = self.socket.send_to(&self.wire, addr);
        }
        for coded in self.coded.drain(..) {
            let stream = coded.stream;
            self.previous[stream] = Some(coded);
        }
    }

    fn status(&self, now: Instant) -> LinkStatus {
        let local_send = self.shared.send.load(Ordering::Relaxed);
        let local_recv = self.shared.recv.load(Ordering::Relaxed);
        let mut status = LinkStatus { local_send, local_recv, ..LinkStatus::default() };
        if let Some(peer) = &self.peer {
            status.state = LinkState::Connected;
            status.peer.clone_from(&peer.name);
            status.peer_addr = peer.addr.ip().to_string();
            status.rtt_ms = peer.rtt.as_secs_f32() * 1000.0;
            status.remote_send = self.remote.send;
            status.remote_recv = self.remote.recv;
            status.names.clone_from(&self.remote.names);
            for (stream, sender) in self.senders.iter().enumerate() {
                status.sending |= Mask::from(sender.is_active()) << stream;
            }
            for (stream, playout) in self.playouts.iter().enumerate() {
                status.receiving |= Mask::from(playout.is_active()) << stream;
                status.streams[stream] = playout.stats();
            }
        } else if let Side::Host(search) = &self.side {
            if let Some((_, reason)) = search.refused.as_ref().filter(|(at, _)| now.duration_since(*at) < REFUSAL_SHOWN) {
                status.state = LinkState::Refused;
                status.refused.clone_from(reason);
            }
        }
        status
    }

    fn publish(&mut self, now: Instant) {
        let status = self.status(now);
        // A session that starts or ends shows at once; meters can wait.
        let urgent = status.state != self.published.state;
        if !urgent && (now.duration_since(self.published_at) < PUBLISH_EVERY || status == self.published) {
            return;
        }
        if let Ok(mut shared) = self.shared.status.lock() {
            shared.clone_from(&status);
        }
        self.published = status;
        self.published_at = now;
    }
}

/// A number no one else can guess from the outside. Not cryptographic: it
/// keeps strangers on the network from stumbling into a session, no more.
fn random_u64() -> u64 {
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_nanos()));
    hasher.finish()
}

/// Never zero: zero is the session of packets sent before there is one.
fn random_session() -> u32 {
    (random_u64() as u32) | 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::tests::{rms, sine_frame};
    use crate::{ALL_STREAMS, FRAME_SAMPLES};

    fn frame(allow: bool) -> Result<Link, LinkError> {
        Link::spawn(LinkConfig {
            role: Role::Frame { allow: Box::new(move |_| allow) },
            name: "Frame".into(),
            // Port 0: the system picks a free one, so tests can run side by side.
            port: 0,
            bitrate: 128_000,
            complexity: 5,
            redundancy: true,
        })
    }

    fn host(frame_port: u16) -> Result<Link, LinkError> {
        let target = SocketAddr::from((Ipv4Addr::LOCALHOST, frame_port));
        Link::spawn(LinkConfig {
            role: Role::Host { candidates: Box::new(move || vec![target]) },
            name: "PC".into(),
            port: frame_port,
            bitrate: 128_000,
            complexity: 5,
            redundancy: true,
        })
    }

    fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !done() {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Plays a tone into `writer` in real time and returns what `reader` heard
    /// during the last half second of two.
    fn pass_tone(writer: &mut PcmWriter, reader: &mut PcmReader) -> Vec<f32> {
        let mut phase = 0.0;
        let mut heard = Vec::new();
        let start = Instant::now();
        for tick in 0..200u32 {
            writer.write_interleaved(&sine_frame(&mut phase, 440.0, 0.5));
            let mut out = [0.0; FRAME_SAMPLES];
            reader.read_interleaved(&mut out);
            if tick >= 150 {
                heard.extend_from_slice(&out);
            }
            let due = start + Duration::from_millis(10) * (tick + 1);
            std::thread::sleep(due.saturating_duration_since(Instant::now()));
        }
        heard
    }

    #[test]
    fn host_and_frame_should_find_each_other() -> Result<(), LinkError> {
        let frame = frame(true)?;
        let host = host(frame.port())?;
        wait_for("both ends to connect", || host.status().connected() && frame.status().connected());

        assert_eq!((host.status().peer, frame.status().peer), ("Frame".to_string(), "PC".to_string()));
        Ok(())
    }

    #[test]
    fn frame_should_refuse_a_pc_it_does_not_know() -> Result<(), LinkError> {
        let frame = frame(false)?;
        let host = host(frame.port())?;
        wait_for("the refusal", || host.status().state == LinkState::Refused);

        assert_eq!(host.status().refused, proto::REFUSE_NOT_ALLOWED);
        Ok(())
    }

    #[test]
    fn frame_should_refuse_a_second_pc() -> Result<(), LinkError> {
        let frame = frame(true)?;
        let first = host(frame.port())?;
        wait_for("the first PC to connect", || first.status().connected());
        let second = host(frame.port())?;
        wait_for("the refusal", || second.status().state == LinkState::Refused);

        assert_eq!(second.status().refused, proto::REFUSE_BUSY);
        Ok(())
    }

    #[test]
    fn masks_should_reach_the_other_end() -> Result<(), LinkError> {
        let frame = frame(true)?;
        let host = host(frame.port())?;
        host.set_masks(0b0101, 0b0011);
        wait_for("the PC's masks", || frame.status().remote_send == 0b0101);

        assert_eq!(frame.status().remote_recv, 0b0011);
        Ok(())
    }

    #[test]
    fn names_should_reach_the_other_end() -> Result<(), LinkError> {
        let frame = frame(true)?;
        let host = host(frame.port())?;
        frame.set_names(vec!["Discord".into(), "OBS".into()]);
        wait_for("the headset's names", || host.status().names.len() == 2);

        assert_eq!(host.status().names, vec!["Discord".to_string(), "OBS".to_string()]);
        Ok(())
    }

    #[test]
    fn tone_should_travel_from_frame_to_host() -> Result<(), LinkError> {
        let frame = frame(true)?;
        let host = host(frame.port())?;
        frame.set_masks(ALL_STREAMS, 0);
        host.set_masks(0, ALL_STREAMS);
        let mut writer = frame.capture(2);
        let mut reader = host.playback(2);
        wait_for("both ends to connect", || host.status().connected() && frame.status().remote_recv == ALL_STREAMS);
        let heard = pass_tone(&mut writer, &mut reader);

        let expected = 0.5 / std::f32::consts::SQRT_2;
        assert!((rms(&heard) - expected).abs() < 0.03, "rms {}", rms(&heard));
        Ok(())
    }

    #[test]
    fn tone_should_travel_from_host_to_frame() -> Result<(), LinkError> {
        let frame = frame(true)?;
        let host = host(frame.port())?;
        frame.set_masks(0, ALL_STREAMS);
        host.set_masks(ALL_STREAMS, 0);
        let mut writer = host.capture(0);
        let mut reader = frame.playback(0);
        wait_for("both ends to connect", || frame.status().connected() && host.status().remote_recv == ALL_STREAMS);
        let heard = pass_tone(&mut writer, &mut reader);

        let expected = 0.5 / std::f32::consts::SQRT_2;
        assert!((rms(&heard) - expected).abs() < 0.03, "rms {}", rms(&heard));
        Ok(())
    }

    #[test]
    fn stream_the_receiver_switched_off_should_stay_silent() -> Result<(), LinkError> {
        let frame = frame(true)?;
        let host = host(frame.port())?;
        frame.set_masks(ALL_STREAMS, 0);
        // The PC takes every stream but the third.
        host.set_masks(0, ALL_STREAMS & !0b0100);
        let mut writer = frame.capture(2);
        let mut reader = host.playback(2);
        wait_for("both ends to connect", || host.status().connected() && frame.status().remote_recv == 0b1011);
        let heard = pass_tone(&mut writer, &mut reader);

        assert!(heard.iter().all(|sample| *sample == 0.0));
        assert_eq!(frame.status().sending, 0);
        Ok(())
    }

    #[test]
    fn host_should_search_again_when_the_frame_goes_away() -> Result<(), LinkError> {
        let frame = frame(true)?;
        let host = host(frame.port())?;
        wait_for("the PC to connect", || host.status().connected());
        // Dropping the link says goodbye, so the PC does not wait for the timeout.
        drop(frame);
        wait_for("the PC to notice", || !host.status().connected());

        assert_eq!(host.status().state, LinkState::Searching);
        Ok(())
    }

    #[test]
    fn out_of_range_stream_should_be_detached() -> Result<(), LinkError> {
        let frame = frame(true)?;
        let writer = frame.capture(STREAMS);

        assert!(writer.is_detached());
        Ok(())
    }
}
