//! The mix graph lives in one process. The panel sends one JSON line per
//! request over a Unix socket and reads one JSON line back.

mod bluetooth;
mod filter;
mod link;
mod pclink;
mod pw;
mod vst;

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use serde::Deserialize;

use crate::config::Config;
use crate::graph::Graph;

pub fn serve(spike: bool) -> Result<(), String> {
    pw::serve(spike)
}

pub fn vst_bridge_test(bundle: &str, arch: &str) -> Result<(), String> {
    vst::bridge_test(bundle, arch)
}

pub fn self_test() -> Result<(), String> {
    link::self_test()?;
    pclink::self_test()?;
    pw::self_test()
}

#[derive(Debug, Clone, Deserialize)]
pub struct Status {
    #[serde(default)]
    pub mix_active: bool,
    #[serde(default)]
    pub meters: HashMap<String, f32>,
    #[serde(default)]
    pub graph: Graph,
    #[serde(default)]
    pub check: String,
    #[serde(default)]
    pub check_secs: f64,
    #[serde(default)]
    pub has_take: bool,
    #[serde(default)]
    pub take_secs: f64,
    /// Plugin host progress by instance: starting, running, failed or stopped.
    #[serde(default)]
    pub vst: HashMap<String, String>,
    /// The session with Frame Audio Link on the PC.
    #[serde(default)]
    pub link: frame_link_core::LinkStatus,
}

/// One request to the engine. `None` means the socket is not there yet.
pub fn call(op: &str, config: Option<&Config>) -> Option<Status> {
    let body = match config {
        Some(config) => serde_json::json!({ "op": op, "config": config, "output": config.check_output, "play_to": config.check_play }),
        None => serde_json::json!({ "op": op }),
    };
    let line = roundtrip(&body.to_string())?;
    serde_json::from_str(&line).ok()
}

#[derive(Deserialize)]
struct MeterReply {
    #[serde(default)]
    meters: HashMap<String, f32>,
}

/// Peaks since the last call, without the graph. Cheap enough to poll every frame.
pub fn meters() -> Option<HashMap<String, f32>> {
    let line = roundtrip(r#"{"op":"meters"}"#)?;
    serde_json::from_str::<MeterReply>(&line).ok().map(|reply| reply.meters)
}

#[derive(Deserialize)]
struct SpectrumReply {
    #[serde(default)]
    bands: Vec<f32>,
}

/// Spectrum after one effect of one bus, in dBFS. Also keeps its capture running.
pub fn spectrum(bus: usize, slot: usize) -> Option<Vec<f32>> {
    let line = roundtrip(&format!(r#"{{"op":"spectrum","bus":{bus},"slot":{slot}}}"#))?;
    serde_json::from_str::<SpectrumReply>(&line).ok().map(|reply| reply.bands)
}

/// Asks the host of one plugin to open its editor.
pub fn open_vst_editor(instance: &str) -> bool {
    let line = serde_json::json!({ "op": "vst_gui", "instance": instance }).to_string();
    roundtrip(&line).is_some()
}

fn roundtrip(line: &str) -> Option<String> {
    let mut stream = UnixStream::connect(pw::socket_path()).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    stream.set_write_timeout(Some(Duration::from_secs(2))).ok()?;
    stream.write_all(line.as_bytes()).ok()?;
    stream.write_all(b"\n").ok()?;
    let mut reader = BufReader::new(stream);
    let mut out = String::new();
    reader.read_line(&mut out).ok()?;
    Some(out)
}
