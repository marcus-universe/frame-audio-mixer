//! The mix graph lives in one process. The panel sends one JSON line per
//! request over a Unix socket and reads one JSON line back.

mod link;
mod pw;

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

pub fn self_test() -> Result<(), String> {
    link::self_test()
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
}

/// One request to the engine. `None` means the socket is not there yet.
pub fn call(op: &str, config: Option<&Config>) -> Option<Status> {
    let body = match config {
        Some(config) => serde_json::json!({ "op": op, "config": config, "output": config.check_output }),
        None => serde_json::json!({ "op": op }),
    };
    let line = roundtrip(&body.to_string())?;
    serde_json::from_str(&line).ok()
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
