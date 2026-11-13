//! What the link costs in CPU on this machine.
//!
//!   cargo run --release --example bench
//!
//! Runs a pair of links over the loopback interface with every stream carrying
//! a full-band signal, as fast as real time, and reports how much processor
//! time ten seconds of that took. The headset shares its cores with the VR
//! compositor, so this is the number to look at before raising the Opus effort.

use std::net::{Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use frame_link_core::{Link, LinkConfig, LinkError, Role, ALL_STREAMS, CHANNELS, FRAME, RATE, STREAMS};

fn link(role: Role, port: u16, complexity: i32) -> Result<Link, LinkError> {
    Link::spawn(LinkConfig { role, name: "bench".into(), port, bitrate: 128_000, complexity, redundancy: true })
}

/// A chord with moving overtones: closer to music than a sine, which Opus
/// codes with almost no effort.
fn fill(block: &mut [f32], phase: &mut f64, stream: usize) {
    for pair in block.as_chunks_mut::<CHANNELS>().0 {
        let t = *phase;
        let voice = |ratio: f64, level: f64| (t * ratio).sin() * level;
        let sample = voice(1.0, 0.2) + voice(1.5, 0.12) + voice(2.01, 0.08) + voice(3.97, 0.05) + voice(7.3 + (t * 0.001).sin(), 0.04);
        pair[0] = sample as f32;
        pair[1] = (sample * 0.8 + voice(5.03, 0.05)) as f32;
        *phase += std::f64::consts::TAU * (180.0 + 70.0 * stream as f64) / f64::from(RATE);
    }
}

/// Processor time this process has used, user and system.
fn cpu_time() -> Duration {
    #[cfg(unix)]
    {
        let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
        let fields: Vec<&str> = stat.rsplit(')').next().unwrap_or("").split_whitespace().collect();
        let ticks: u64 = [11, 12].iter().filter_map(|index| fields.get(*index)?.parse::<u64>().ok()).sum();
        // USER_HZ is 100 on every Linux this runs on.
        Duration::from_millis(ticks * 10)
    }
    #[cfg(not(unix))]
    {
        Duration::ZERO
    }
}

fn main() -> Result<(), LinkError> {
    for complexity in [5, 10] {
        let frame = link(Role::Frame { allow: Box::new(|_| true) }, 0, complexity)?;
        let target = SocketAddr::from((Ipv4Addr::LOCALHOST, frame.port()));
        let host = link(Role::Host { candidates: Box::new(move || vec![target]) }, frame.port(), complexity)?;
        frame.set_masks(ALL_STREAMS, ALL_STREAMS);
        host.set_masks(ALL_STREAMS, ALL_STREAMS);
        let mut writers: Vec<_> = (0..STREAMS).flat_map(|stream| [frame.capture(stream), host.capture(stream)]).collect();
        let mut readers: Vec<_> = (0..STREAMS).flat_map(|stream| [frame.playback(stream), host.playback(stream)]).collect();
        while !host.status().connected() {
            std::thread::sleep(Duration::from_millis(20));
        }

        let mut phases = vec![0.0f64; writers.len()];
        let mut block = [0.0f32; FRAME * CHANNELS];
        let (start, cpu_start) = (Instant::now(), cpu_time());
        let ticks = 1000u32;
        for tick in 1..=ticks {
            for (index, writer) in writers.iter_mut().enumerate() {
                fill(&mut block, &mut phases[index], index);
                writer.write_interleaved(&block);
            }
            for reader in &mut readers {
                reader.read_interleaved(&mut block);
            }
            let due = start + Duration::from_millis(10) * tick;
            std::thread::sleep(due.saturating_duration_since(Instant::now()));
        }
        let (wall, cpu) = (start.elapsed(), cpu_time() - cpu_start);
        let lost: u64 = host.status().streams.iter().chain(frame.status().streams.iter()).map(|stats| stats.lost).sum();
        println!(
            "complexity {complexity}: {} streams coded and decoded for {:.1} s took {:.2} s of CPU ({:.1} % of one core, both ends together), {lost} frames concealed",
            writers.len(),
            wall.as_secs_f64(),
            cpu.as_secs_f64(),
            100.0 * cpu.as_secs_f64() / wall.as_secs_f64()
        );
    }
    Ok(())
}
