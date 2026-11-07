//! Four bus instances: A/B × PC/headset. Each captures a mix sink and plays
//! the processed audio to the destination. Empty chains copy.

use super::dsp::Processor;
use crate::config::{EffectSlot, FxBus};
use crate::graph::Graph;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

const RATE: u32 = 48_000;
/// One speaker period. Reading 512 from a 480-period device drops a 480-frame hole.
const FRAMES: usize = 480;
/// Sample count for `pw-cat --latency`. A fraction is rejected here.
const STREAM_LATENCY: &str = "480";

fn pipe_args(target: &str, props: &str) -> Vec<String> {
    vec![
        "--raw".into(),
        "--format".into(),
        "f32".into(),
        "--channels".into(),
        "2".into(),
        "--rate".into(),
        RATE.to_string(),
        "--latency".into(),
        STREAM_LATENCY.into(),
        "--target".into(),
        target.into(),
        "-P".into(),
        props.into(),
        "-".into(),
    ]
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum InstanceKey {
    APc,
    APhones,
    BPc,
    BPhones,
}

impl InstanceKey {
    pub const ALL: [InstanceKey; 4] = [InstanceKey::APc, InstanceKey::APhones, InstanceKey::BPc, InstanceKey::BPhones];

    pub fn id(self) -> &'static str {
        match self {
            InstanceKey::APc => "a-pc",
            InstanceKey::APhones => "a-phones",
            InstanceKey::BPc => "b-pc",
            InstanceKey::BPhones => "b-phones",
        }
    }

    pub fn sink(self) -> String {
        match self {
            InstanceKey::APc => "frame_amix_fx_a_pc".into(),
            InstanceKey::APhones => "frame_amix_fx_a_phones".into(),
            InstanceKey::BPc => "frame_amix_fx_b_pc".into(),
            InstanceKey::BPhones => "frame_amix_fx_b_phones".into(),
        }
    }

    pub(crate) fn is_a(self) -> bool {
        matches!(self, InstanceKey::APc | InstanceKey::APhones)
    }
}

struct Live {
    stop: Arc<AtomicBool>,
    rec: Child,
    play: Child,
    thread: Option<JoinHandle<()>>,
    capture: String,
    playback: String,
    chain: Arc<Mutex<Vec<EffectSlot>>>,
}

pub struct Engine {
    live: HashMap<InstanceKey, Live>,
}

impl Engine {
    pub fn new() -> Self {
        Self { live: HashMap::new() }
    }

    pub fn sync(&mut self, graph: &Graph, fx_a: &FxBus, fx_b: &FxBus, wanted: &HashMap<InstanceKey, String>) {
        for key in InstanceKey::ALL {
            let Some(play_to) = wanted.get(&key) else {
                self.stop(key);
                continue;
            };
            let Some(sink) = graph.node_by_name(&key.sink()) else {
                self.stop(key);
                continue;
            };
            let capture = if sink.serial != 0 { sink.serial.to_string() } else { sink.name.clone() };
            let chain = if key.is_a() { &fx_a.chain } else { &fx_b.chain };
            if let Some(live) = self.live.get_mut(&key) {
                if live.capture == capture && live.playback == *play_to {
                    if let Ok(mut guard) = live.chain.lock() {
                        *guard = chain.clone();
                    }
                    continue;
                }
            }
            self.stop(key);
            if let Some(live) = spawn(key, &capture, play_to, chain.clone()) {
                self.live.insert(key, live);
            }
        }
    }

    pub fn push_params(&mut self, fx_a: &FxBus, fx_b: &FxBus) {
        for (key, live) in &self.live {
            let chain = if key.is_a() { &fx_a.chain } else { &fx_b.chain };
            if let Ok(mut guard) = live.chain.lock() {
                *guard = chain.clone();
            }
        }
    }

    fn stop(&mut self, key: InstanceKey) {
        let Some(mut live) = self.live.remove(&key) else { return };
        live.stop.store(true, Ordering::Relaxed);
        let _ = Command::new("kill").args(["-TERM", &live.rec.id().to_string()]).status();
        let _ = Command::new("kill").args(["-TERM", &live.play.id().to_string()]).status();
        let _ = live.rec.wait();
        let _ = live.play.wait();
        if let Some(thread) = live.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        for key in InstanceKey::ALL {
            self.stop(key);
        }
    }
}

fn spawn(key: InstanceKey, capture: &str, play_to: &str, chain: Vec<EffectSlot>) -> Option<Live> {
    // Names must contain both `virtual` and `sink` or the spatializer wraps the
    // stream, ignores target.object, and sends the wet return to the speakers.
    let tap = format!("{}_virtual_sink_tap", key.sink());
    let ret = format!("{}_virtual_sink_return", key.sink());
    let rec_props = format!("{{ node.name = {tap} node.virtual = true node.passive = false stream.capture.sink = true }}");
    let play_props = format!("{{ node.name = {ret} node.virtual = true node.passive = false node.dont-fallback = true }}");
    let mut rec = Command::new("pw-record")
        .args(pipe_args(capture, &rec_props))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut play = Command::new("pw-play")
        .args(pipe_args(play_to, &play_props))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut rec_out = rec.stdout.take()?;
    let mut play_in = play.stdin.take()?;
    let stop = Arc::new(AtomicBool::new(false));
    let stop_t = Arc::clone(&stop);
    let chain = Arc::new(Mutex::new(chain));
    let chain_t = Arc::clone(&chain);
    let thread = thread::spawn(move || {
        let mut proc = Processor::new();
        let mut bytes = vec![0u8; FRAMES * 2 * 4];
        let mut samples = vec![0.0f32; FRAMES * 2];
        let mut applied: Option<Vec<EffectSlot>> = None;
        while !stop_t.load(Ordering::Relaxed) {
            if read_exact(&mut rec_out, &mut bytes).is_err() {
                break;
            }
            for (i, chunk) in bytes.chunks_exact(4).enumerate() {
                samples[i] = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            }
            if let Ok(guard) = chain_t.lock() {
                if applied.as_deref() != Some(guard.as_slice()) {
                    proc.set_chain(&guard);
                    applied = Some(guard.clone());
                }
            }
            proc.process(&mut samples);
            for (i, sample) in samples.iter().enumerate() {
                let b = sample.to_le_bytes();
                bytes[i * 4] = b[0];
                bytes[i * 4 + 1] = b[1];
                bytes[i * 4 + 2] = b[2];
                bytes[i * 4 + 3] = b[3];
            }
            if play_in.write_all(&bytes).is_err() {
                break;
            }
        }
    });
    Some(Live {
        stop,
        rec,
        play,
        thread: Some(thread),
        capture: capture.to_string(),
        playback: play_to.to_string(),
        chain,
    })
}

fn read_exact(reader: &mut impl Read, buf: &mut [u8]) -> std::io::Result<()> {
    let mut got = 0;
    while got < buf.len() {
        let n = reader.read(&mut buf[got..])?;
        if n == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "fx capture closed"));
        }
        got += n;
    }
    Ok(())
}

pub fn self_test() -> Result<(), String> {
    if InstanceKey::APc.sink() != "frame_amix_fx_a_pc" {
        return Err("fx sink name".into());
    }
    let ret = format!("{}_virtual_sink_return", InstanceKey::APc.sink());
    if !ret.contains("virtual") || !ret.contains("sink") {
        return Err("fx return name".into());
    }
    let args = pipe_args("42", "{ node.name = frame_amix_fx_a_pc_virtual_sink_tap }").join(" ");
    if !args.contains("--format f32") || !args.contains("--latency 480") || args.contains("512/48000") || !args.contains("--target 42") || args.contains("11ms") {
        return Err(args);
    }
    Ok(())
}
