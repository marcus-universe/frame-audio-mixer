//! A stand-in for the other end of the link, for testing one end by itself.
//!
//!   cargo run --release --example peer -- frame [PORT]
//!       Acts as the headset: accepts any PC, sends a tone on B1-B4 (each at
//!       its own pitch) and prints the level of what arrives on PC1-PC4.
//!
//!   cargo run --release --example peer -- host ADDRESS [PORT]
//!       Acts as the PC: connects to a headset at ADDRESS, sends a tone on
//!       PC1-PC4 and prints the level of what arrives on B1-B4.
//!
//! PEER_LEVEL sets the tone's peak (default 0.25), for tests on sound devices
//! someone may be listening to.

use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use frame_link_core::{Link, LinkConfig, LinkError, Role, ALL_STREAMS, CHANNELS, DEFAULT_PORT, FRAME, RATE, STREAMS};

fn main() -> Result<(), LinkError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let as_host = args.first().map(String::as_str) == Some("host");
    let (role, port) = if as_host {
        let address: Option<IpAddr> = args.get(1).and_then(|text| text.parse().ok());
        let port = args.get(2).and_then(|text| text.parse().ok()).unwrap_or(DEFAULT_PORT);
        let Some(address) = address else {
            eprintln!("usage: peer host ADDRESS [PORT]");
            return Ok(());
        };
        let target = SocketAddr::new(address, port);
        (Role::Host { candidates: Box::new(move || vec![target]) }, port)
    } else {
        let port = args.get(1).and_then(|text| text.parse().ok()).unwrap_or(DEFAULT_PORT);
        (Role::Frame { allow: Box::new(|_| true) }, port)
    };
    let link = Link::spawn(LinkConfig { role, name: "test peer".into(), port, bitrate: 128_000, complexity: 5, redundancy: true })?;
    link.set_masks(ALL_STREAMS, ALL_STREAMS);
    link.set_names((1..=STREAMS).map(|index| format!("Test {index}")).collect());
    let mut writers: Vec<_> = (0..STREAMS).map(|stream| link.capture(stream)).collect();
    let mut readers: Vec<_> = (0..STREAMS).map(|stream| link.playback(stream)).collect();
    println!("{} on UDP port {}", if as_host { "host" } else { "frame" }, link.port());

    let level: f32 = std::env::var("PEER_LEVEL").ok().and_then(|text| text.parse().ok()).unwrap_or(0.25);
    let mut phases = [0.0f64; STREAMS];
    let mut peaks = [0.0f32; STREAMS];
    let mut block = [0.0f32; FRAME * CHANNELS];
    let start = Instant::now();
    for tick in 1..=u32::MAX {
        for (stream, writer) in writers.iter_mut().enumerate() {
            let hz = 220.0 * (stream + 1) as f64;
            for pair in block.as_chunks_mut::<CHANNELS>().0 {
                let sample = phases[stream].sin() as f32 * level;
                pair.fill(sample);
                phases[stream] += std::f64::consts::TAU * hz / f64::from(RATE);
            }
            writer.write_interleaved(&block);
        }
        for (stream, reader) in readers.iter_mut().enumerate() {
            reader.read_interleaved(&mut block);
            peaks[stream] = block.iter().fold(peaks[stream], |peak, sample| peak.max(sample.abs()));
        }
        if tick % 100 == 0 {
            let status = link.status();
            let levels: Vec<String> = peaks.iter().map(|peak| format!("{peak:.5}")).collect();
            let delays: Vec<String> = status.streams.iter().map(|stats| format!("{:.0}ms/{}lost/{}stall", stats.delay_ms, stats.lost, stats.stalls)).collect();
            println!(
                "{:?} peer `{}` {} | remote send {:04b} recv {:04b} | sending {:04b} receiving {:04b} | in peaks [{}] | {}",
                status.state,
                status.peer,
                status.peer_addr,
                status.remote_send,
                status.remote_recv,
                status.sending,
                status.receiving,
                levels.join(" "),
                delays.join(" ")
            );
            peaks = [0.0; STREAMS];
        }
        let due = start + Duration::from_millis(10) * tick;
        std::thread::sleep(due.saturating_duration_since(Instant::now()));
    }
    Ok(())
}
