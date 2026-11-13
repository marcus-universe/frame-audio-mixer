//! The engine's end of the link to Frame Audio Link on the PC.
//!
//! B1..B4 are sent to the PC, where they show up as four more streaming
//! microphones. PC1..PC4 arrive from the PC and are the source of channels
//! added as "PC1".."PC4". The link itself (network, codec, jitter buffer) is
//! `frame_link_core`; this module only connects it to the graph.
//!
//! The realtime side never waits for the link. A B bus hands its samples to a
//! ring through a slot it can always `try_lock`, and a PC channel reads from
//! one the same way. The slots are empty while the link is switched off.

use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use frame_link_core::{Link, LinkConfig, LinkStatus, Mask, PcmReader, PcmWriter, Role, DEFAULT_PORT, STREAMS};

use crate::engine::filter::Dsp;

/// Opus bit rate of one stereo stream. 128 kbit/s is transparent for speech
/// and music at this frame size.
const BITRATE: i32 = 128_000;
/// Opus effort. The headset shares its cores with the compositor; 5 is where
/// the quality curve flattens.
const COMPLEXITY: i32 = 5;
/// How long the list of PCs this headset streams from is trusted.
const PEERS_FRESH: Duration = Duration::from_secs(2);

/// Where a B bus leaves its samples for the link.
pub(super) type TapSlot = Arc<Mutex<Option<PcmWriter>>>;
/// Where a PC channel takes its samples from.
pub(super) type SourceSlot = Arc<Mutex<Option<PcmReader>>>;

pub(super) struct PcLink {
    link: Option<Link>,
    taps: [TapSlot; STREAMS],
    sources: [SourceSlot; STREAMS],
    /// A link that could not be started is reported once, not on every apply.
    failed: bool,
}

impl PcLink {
    pub(super) fn new() -> Self {
        Self { link: None, taps: Default::default(), sources: Default::default(), failed: false }
    }

    /// The slot B bus `index` writes to. Empty while the link is off.
    pub(super) fn tap(&self, index: usize) -> Option<TapSlot> {
        self.taps.get(index).cloned()
    }

    /// The slot the channel for PC input `index` reads from.
    pub(super) fn source(&self, index: usize) -> Option<SourceSlot> {
        self.sources.get(index).cloned()
    }

    /// Starts or stops the link to match the setting.
    pub(super) fn set_enabled(&mut self, on: bool) {
        if on == self.link.is_some() {
            return;
        }
        if !on {
            self.clear_slots();
            self.link = None;
            self.failed = false;
            return;
        }
        if self.failed {
            return;
        }
        let mut gate = PeerGate::default();
        let config = LinkConfig {
            role: Role::Frame { allow: Box::new(move |ip| gate.allows(ip)) },
            name: host_name(),
            port: DEFAULT_PORT,
            bitrate: BITRATE,
            complexity: COMPLEXITY,
            redundancy: true,
        };
        match Link::spawn(config) {
            Ok(link) => {
                for (index, (tap, source)) in self.taps.iter().zip(&self.sources).enumerate() {
                    fill(tap, link.capture(index));
                    fill(source, link.playback(index));
                }
                self.link = Some(link);
            }
            Err(err) => {
                eprintln!("frame-audio-mixer: PC link: {err}");
                self.failed = true;
            }
        }
    }

    fn clear_slots(&self) {
        for slot in &self.taps {
            if let Ok(mut slot) = slot.lock() {
                *slot = None;
            }
        }
        for slot in &self.sources {
            if let Ok(mut slot) = slot.lock() {
                *slot = None;
            }
        }
    }

    /// Tells the PC which of its outputs a channel listens to, and what the
    /// B buses are called. Every B bus is offered.
    pub(super) fn sync(&self, wanted_inputs: Mask, names: Vec<String>) {
        if let Some(link) = &self.link {
            link.set_masks(frame_link_core::ALL_STREAMS, wanted_inputs);
            link.set_names(names);
        }
    }

    pub(super) fn status(&self) -> LinkStatus {
        self.link.as_ref().map(Link::status).unwrap_or_default()
    }

    /// The virtual microphones the linked PC takes right now. None while no
    /// PC is linked.
    pub(super) fn taken(&self) -> Mask {
        let status = self.status();
        if status.connected() { status.remote_recv } else { 0 }
    }
}

fn fill<T>(slot: &Mutex<Option<T>>, value: T) {
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(value);
    }
}

/// Hands one cycle of a B bus to the link. Called from the realtime thread.
pub(super) fn write_tap(slot: &TapSlot, left: &[f32], right: &[f32]) {
    // Held by the main thread only while the link starts or stops.
    let Ok(mut slot) = slot.try_lock() else { return };
    if let Some(writer) = slot.as_mut() {
        writer.write_planar(left, right);
    }
}

/// The source node of a PC channel: what the PC sent, or silence.
pub(super) struct PcSourceDsp {
    pub(super) source: SourceSlot,
}

impl Dsp for PcSourceDsp {
    fn process(&mut self, _input: Option<[&[f32]; 2]>, output: Option<[&mut [f32]; 2]>) {
        let Some([left, right]) = output else { return };
        if let Ok(mut slot) = self.source.try_lock() {
            if let Some(reader) = slot.as_mut() {
                reader.read_planar(left, right);
                return;
            }
        }
        left.fill(0.0);
        right.fill(0.0);
    }
}

/// Decides which PC may link: the one this headset is streaming from.
///
/// The link carries the mix of the B buses, so it must not answer any device
/// on the network that asks. VRLink is started with the addresses of the PC it
/// streams from (`-c ADDRESS`, once per network path); only those are let in.
#[derive(Default)]
struct PeerGate {
    checked: Option<Instant>,
    /// `None` while VRLink is not running: nothing is streaming.
    peers: Option<Vec<IpAddr>>,
}

impl PeerGate {
    fn allows(&mut self, ip: IpAddr) -> bool {
        // A process on the headset itself can already reach the graph.
        if ip.is_loopback() {
            return true;
        }
        if self.checked.is_none_or(|at| at.elapsed() >= PEERS_FRESH) {
            self.peers = vrlink_peers();
            self.checked = Some(Instant::now());
        }
        allowed(self.peers.as_deref(), ip)
    }
}

fn allowed(peers: Option<&[IpAddr]>, ip: IpAddr) -> bool {
    match peers {
        None => false,
        // A VRLink that names no address: a build with other arguments. The
        // stream is up, so a PC on a local network is let in.
        Some([]) => is_local(ip),
        Some(peers) => peers.contains(&ip),
    }
}

fn is_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_unique_local() || v6.is_unicast_link_local(),
    }
}

/// Addresses of the PC every running VRLink streams from. `None` when no
/// VRLink is running.
fn vrlink_peers() -> Option<Vec<IpAddr>> {
    let mut peers = None;
    for entry in std::fs::read_dir("/proc").ok()?.flatten() {
        let path = entry.path();
        let is_pid = entry.file_name().to_str().is_some_and(|name| name.bytes().all(|byte| byte.is_ascii_digit()));
        if !is_pid {
            continue;
        }
        let Ok(cmdline) = std::fs::read(path.join("cmdline")) else { continue };
        let exe_is_vrlink = || std::fs::read_link(path.join("exe")).is_ok_and(|exe| exe.file_name().is_some_and(|name| name == VRLINK));
        if !runs_vrlink(&cmdline) && !exe_is_vrlink() {
            continue;
        }
        peers.get_or_insert_with(Vec::new).extend(peers_in_cmdline(&cmdline));
    }
    peers
}

const VRLINK: &str = "vrlink";

/// True when a NUL-separated command line starts the `vrlink` program.
///
/// The process name (`comm`) cannot be used: VRLink's main thread renames
/// itself to `vrlinkrunthread`, so no process is ever called `vrlink`.
fn runs_vrlink(cmdline: &[u8]) -> bool {
    let program = cmdline.split(|byte| *byte == 0).next().unwrap_or_default();
    program.rsplit(|byte| *byte == b'/').next() == Some(VRLINK.as_bytes())
}

/// The `-c ADDRESS` arguments of a NUL-separated command line.
fn peers_in_cmdline(cmdline: &[u8]) -> Vec<IpAddr> {
    let args: Vec<&[u8]> = cmdline.split(|byte| *byte == 0).collect();
    args.windows(2)
        .filter(|pair| pair[0] == b"-c")
        .filter_map(|pair| std::str::from_utf8(pair[1]).ok()?.parse().ok())
        .collect()
}

fn host_name() -> String {
    let name = std::fs::read_to_string("/proc/sys/kernel/hostname").unwrap_or_default();
    let name = name.trim();
    if name.is_empty() { "Steam Frame".into() } else { name.to_string() }
}

pub(super) fn self_test() -> Result<(), String> {
    // The arguments as /proc/PID/cmdline holds them: separated by NUL bytes.
    let cmdline = ["./vrlink", "-I", "76561198000000000", "-G", "250820", "-c", "192.168.50.78", "-c", "10.35.78.29", "-e", "secret", ""].join("\0");
    let peers = peers_in_cmdline(cmdline.as_bytes());
    let lan: IpAddr = [192, 168, 50, 78].into();
    let direct: IpAddr = [10, 35, 78, 29].into();
    let stranger: IpAddr = [192, 168, 50, 99].into();
    if peers != [lan, direct] {
        return Err(format!("vrlink peers {peers:?}"));
    }
    if !peers_in_cmdline(b"./vrlink\0-c\0").is_empty() || !peers_in_cmdline(b"./vrlink\0-c\0not-an-address\0").is_empty() {
        return Err("vrlink peers from a broken command line".into());
    }
    // VRLink is found by its program, however it was started.
    let by_path = ["/opt/steamvr/tools/vrlink/bin/linuxarm64/vrlink", "-c", "10.35.78.29", ""].join("\0");
    let started: [&[u8]; 3] = [cmdline.as_bytes(), by_path.as_bytes(), b"vrlink\0"];
    if started.iter().any(|cmdline| !runs_vrlink(cmdline)) {
        return Err("vrlink not recognized by its command line".into());
    }
    let helper = ["/usr/bin/vrlinkhelper", "-c", "10.35.78.29", ""].join("\0");
    let others: [&[u8]; 4] = [b"", helper.as_bytes(), b"bash\0-c\0vrlink\0", b"/opt/vrlink/steam\0"];
    if others.iter().any(|cmdline| runs_vrlink(cmdline)) {
        return Err("another program taken for vrlink".into());
    }
    if !allowed(Some(&peers), direct) || allowed(Some(&peers), stranger) {
        return Err("PC link let the wrong address in".into());
    }
    if allowed(None, lan) {
        return Err("PC link answered while nothing is streaming".into());
    }
    let public: IpAddr = [8, 8, 8, 8].into();
    if !allowed(Some(&[]), lan) || allowed(Some(&[]), public) {
        return Err("PC link fallback for a VRLink without addresses".into());
    }
    let mut gate = PeerGate::default();
    if !gate.allows([127, 0, 0, 1].into()) {
        return Err("PC link refused the headset itself".into());
    }
    // A switched-off link hands out silence and takes nothing.
    let link = PcLink::new();
    let Some(source) = link.source(0) else { return Err("PC link source slot".into()) };
    let mut dsp = PcSourceDsp { source };
    let (mut left, mut right) = ([1.0f32; 64], [1.0f32; 64]);
    dsp.process(None, Some([&mut left, &mut right]));
    if left.iter().chain(right.iter()).any(|sample| *sample != 0.0) {
        return Err("PC channel without a link is not silent".into());
    }
    if link.source(STREAMS).is_some() || link.tap(STREAMS).is_some() {
        return Err("PC link slot out of range".into());
    }
    if link.status().connected() || link.taken() != 0 {
        return Err("switched-off PC link reports a session".into());
    }
    // A link tap without a link drops what it is given, without blocking.
    let Some(tap) = link.tap(0) else { return Err("PC link tap slot".into()) };
    write_tap(&tap, &[0.5; 64], &[0.5; 64]);
    Ok(())
}
