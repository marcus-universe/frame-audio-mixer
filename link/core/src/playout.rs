//! One incoming stream: coded frames in, steady audio out.
//!
//! Frames wait here, sorted, until the playback ring needs them. That wait is
//! the jitter buffer: it absorbs packets that arrive late or in bursts. A
//! frame that never arrives is concealed by the decoder. The total amount
//! buffered also steers the resampler, which keeps this end in step with the
//! sender's clock.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::codec::{CodecError, Decoder};
use crate::pcm::PlaybackFeed;
use crate::proto::{Blob, BLOB_END, BLOB_REPEAT, BLOB_START};
use crate::resample::{Resampler, MAX_PULL};
use crate::{FRAME, FRAME_SAMPLES, RATE};

/// Jitter cushion a stream starts with, in frames (10 ms each).
const JITTER_MIN: usize = 2;
/// The cushion never grows past this: 120 ms.
const JITTER_MAX: usize = 12;
/// A cushion that ran dry grows by one frame, at most this often.
const GROW_EVERY: Duration = Duration::from_secs(2);
/// A cushion that held shrinks by one frame after this long.
const SHRINK_AFTER: Duration = Duration::from_secs(30);
/// Slack in the playback ring beyond one read of the audio thread, in frames.
/// It covers the link thread being scheduled late.
const RING_SLACK: usize = FRAME;
/// Frames concealed in a row while nothing at all is buffered. Then the
/// stream is taken to have stopped.
const HOLD_FRAMES: u32 = 5;
/// Frames concealed in a row while later frames are already here. Then
/// playback jumps ahead to them.
const GAP_FRAMES: u32 = 5;
/// A frame this far from the expected one belongs to a new run.
const RESYNC_DISTANCE: i32 = 100;
/// Frames beyond the target at which the excess is dropped instead of being
/// played off slowly.
const OVERFLOW_FRAMES: usize = 10;
/// Around the target, no correction is applied: half a frame either way.
const DEAD_BAND: f64 = FRAME as f64 / 2.0;
/// Resampling ratio per sample of buffer error beyond the dead band.
const PULL_PER_SAMPLE: f64 = 1.0e-5;
/// Weight of one new measurement in the smoothed buffer level: about a second.
const LEVEL_SMOOTHING: f64 = 0.01;

/// Counters and the current delay of one incoming stream.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct StreamStats {
    /// Audio buffered between the network and the sound device, in milliseconds.
    pub delay_ms: f32,
    /// Frames that never arrived and were concealed.
    pub lost: u64,
    /// Frames that arrived after their turn.
    pub late: u64,
    /// Times the buffer ran dry and playback had to wait for the network.
    pub stalls: u64,
    /// Times playback had to start over.
    pub restarts: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    /// Nothing to play. The ring runs dry and the audio thread reads silence.
    Idle,
    /// Collecting the cushion before the first frame is played.
    Buffering { since: Instant },
    Playing,
}

pub(crate) struct Playout {
    decoder: Decoder,
    resampler: Resampler,
    feed: Option<PlaybackFeed>,
    /// Frames not yet decoded, by sequence number.
    pending: BTreeMap<u32, Vec<u8>>,
    /// The frame the decoder wants next.
    next_seq: u32,
    phase: Phase,
    jitter_frames: usize,
    concealed_run: u32,
    /// Smoothed total of buffered audio, in samples per channel.
    level: f64,
    last_grow: Instant,
    last_underrun: Instant,
    /// Set once the sender has closed the run: the sequence number after its
    /// last frame. Running dry there is the end of the sound, not a stall.
    ends_at: Option<u32>,
    stats: StreamStats,
    pcm: [f32; FRAME_SAMPLES],
    resampled: Vec<f32>,
}

impl Playout {
    pub(crate) fn new(now: Instant) -> Result<Self, CodecError> {
        Ok(Self {
            decoder: Decoder::new()?,
            resampler: Resampler::new(),
            feed: None,
            pending: BTreeMap::new(),
            next_seq: 0,
            phase: Phase::Idle,
            jitter_frames: JITTER_MIN,
            concealed_run: 0,
            level: 0.0,
            last_grow: now,
            last_underrun: now,
            ends_at: None,
            stats: StreamStats::default(),
            pcm: [0.0; FRAME_SAMPLES],
            resampled: Vec::with_capacity(FRAME_SAMPLES + 64),
        })
    }

    pub(crate) fn attach(&mut self, feed: PlaybackFeed) {
        self.feed = Some(feed);
        self.stop();
    }

    /// Whether the audio thread should wake the link after each read. Only a
    /// connected link has anything to top the ring up with.
    pub(crate) fn set_live(&self, live: bool) {
        if let Some(feed) = &self.feed {
            feed.set_live(live);
        }
    }

    pub(crate) fn stats(&self) -> StreamStats {
        self.stats
    }

    /// True while frames are being played or collected.
    pub(crate) fn is_active(&self) -> bool {
        self.phase != Phase::Idle
    }

    /// Drops everything buffered. The next frame starts a new run.
    pub(crate) fn stop(&mut self) {
        self.pending.clear();
        self.phase = Phase::Idle;
        self.concealed_run = 0;
        self.ends_at = None;
        self.stats.delay_ms = 0.0;
    }

    /// The sender reports it has gone silent. If its end marker was lost,
    /// whatever is buffered now is the rest of the run.
    pub(crate) fn end_after_pending(&mut self) {
        if self.phase == Phase::Idle || self.ends_at.is_some() {
            return;
        }
        let after_last = self.pending.keys().next_back().map_or(self.next_seq, |seq| seq.wrapping_add(1));
        self.ends_at = Some(after_last);
    }

    /// What the playback ring should hold right after it was topped up.
    fn ring_target(&self) -> usize {
        self.feed.as_ref().map_or(FRAME, PlaybackFeed::quantum) + RING_SLACK
    }

    /// Audio that should sit between the network and the sound device, in
    /// samples per channel.
    fn target(&self) -> usize {
        self.ring_target() + self.jitter_frames * FRAME
    }

    fn restart(&mut self, seq: u32, now: Instant) {
        self.pending.clear();
        self.decoder.reset();
        self.resampler.reset();
        self.next_seq = seq;
        self.concealed_run = 0;
        self.ends_at = None;
        self.phase = Phase::Buffering { since: now };
        self.stats.restarts += 1;
    }

    /// Takes one frame off the wire.
    pub(crate) fn push(&mut self, blob: &Blob<'_>, now: Instant) {
        let attached = self.feed.as_ref().is_some_and(|feed| !feed.is_abandoned());
        if !attached {
            self.feed = None;
            self.stop();
            return;
        }
        let distance = blob.seq.wrapping_sub(self.next_seq) as i32;
        if blob.flags & BLOB_END != 0 {
            // Only a run in progress can end.
            if self.phase != Phase::Idle && (0..RESYNC_DISTANCE).contains(&distance) {
                self.ends_at = Some(blob.seq);
            }
            return;
        }
        // A frame at or past the end means the run goes on: the end was a
        // stale report that overtook the frames before it.
        if self.ends_at.is_some_and(|end| blob.seq.wrapping_sub(end) as i32 >= 0) {
            self.ends_at = None;
        }
        let repeat = blob.flags & BLOB_REPEAT != 0;
        let fresh_run = match self.phase {
            // A repeat that is not far ahead is the tail of the run that just ended.
            Phase::Idle => !(repeat && distance.abs() < RESYNC_DISTANCE && distance < 0),
            _ => (blob.flags & BLOB_START != 0 && distance > 0) || distance.abs() >= RESYNC_DISTANCE,
        };
        if fresh_run {
            self.restart(blob.seq, now);
        } else if distance < 0 {
            if !repeat {
                self.stats.late += 1;
            }
            return;
        }
        self.pending.entry(blob.seq).or_insert_with(|| blob.payload.to_vec());
    }

    /// Tops the playback ring up. Called whenever the link thread wakes.
    pub(crate) fn pump(&mut self, now: Instant) {
        if self.feed.as_ref().is_none_or(PlaybackFeed::is_abandoned) {
            self.feed = None;
            self.stop();
            return;
        }
        if let Phase::Buffering { since } = self.phase {
            let cushion = self.target().div_ceil(FRAME);
            // Frames that trickle in slower than they are played would never fill
            // the cushion; after its own length in time, play what is here.
            let waited = now.duration_since(since) >= frames_duration(cushion + 1);
            // A run that has already ended will not grow: play it now.
            if self.pending.len() < cushion && !waited && self.ends_at.is_none() {
                return;
            }
            if self.pending.is_empty() {
                self.phase = Phase::Idle;
                return;
            }
            self.phase = Phase::Playing;
            self.level = self.target() as f64;
        }
        if self.phase != Phase::Playing {
            return;
        }
        self.shed_overflow();
        // Bounded, so a ring that never fills cannot keep this thread busy.
        for _ in 0..64 {
            let buffered = self.feed.as_ref().map_or(usize::MAX, PlaybackFeed::buffered_frames);
            if buffered >= self.ring_target() {
                break;
            }
            if !self.play_one(now) {
                break;
            }
        }
        self.relax(now);
    }

    /// Decodes the next frame into the ring. False once the stream has stopped.
    fn play_one(&mut self, now: Instant) -> bool {
        let mut fade_out = false;
        let decoded = if let Some(payload) = self.pending.remove(&self.next_seq) {
            self.concealed_run = 0;
            self.next_seq = self.next_seq.wrapping_add(1);
            self.decoder.decode(Some(&payload), &mut self.pcm)
        } else if self.pending.is_empty() && self.ends_at == Some(self.next_seq) {
            // Every frame of the run has been played and the sender said that
            // was all. One concealed frame, faded out, ends it without a click.
            self.phase = Phase::Idle;
            self.ends_at = None;
            self.concealed_run = 0;
            self.stats.delay_ms = 0.0;
            if self.decoder.decode(None, &mut self.pcm).is_ok() {
                fade(&mut self.pcm);
                self.resampled.clear();
                self.resampler.process(&self.pcm, 1.0, &mut self.resampled);
                if let Some(feed) = self.feed.as_mut() {
                    let _ = feed.push(&self.resampled);
                }
            }
            return false;
        } else if self.pending.is_empty() {
            // Nothing is here yet: the network is late, or the stream ended.
            // Conceal without stepping on, so a late frame is still played
            // and the cushion is that much deeper from now on.
            self.concealed_run += 1;
            self.note_underrun(now);
            if self.concealed_run > HOLD_FRAMES {
                self.phase = Phase::Idle;
                self.stats.delay_ms = 0.0;
                return false;
            }
            fade_out = self.concealed_run == HOLD_FRAMES;
            self.decoder.decode(None, &mut self.pcm)
        } else if self.concealed_run < GAP_FRAMES {
            // Later frames are here, so this one is lost for good.
            self.concealed_run += 1;
            self.stats.lost += 1;
            self.next_seq = self.next_seq.wrapping_add(1);
            self.decoder.decode(None, &mut self.pcm)
        } else {
            // A long hole: jump to the frames that did arrive.
            if let Some(first) = self.pending.keys().next().copied() {
                self.next_seq = first;
            }
            self.concealed_run = 0;
            return true;
        };
        if decoded.is_err() {
            // Not a frame of this link. Its place stays silent.
            self.stats.lost += 1;
        }
        if fade_out {
            fade(&mut self.pcm);
        }
        let ratio = self.steer();
        self.resampled.clear();
        self.resampler.process(&self.pcm, ratio, &mut self.resampled);
        if let Some(feed) = self.feed.as_mut() {
            // A full ring means the audio thread stopped reading; the frame is dropped.
            let _ = feed.push(&self.resampled);
        }
        true
    }

    /// The resampling ratio for the next frame, from how far the buffered
    /// audio is from its target.
    fn steer(&mut self) -> f64 {
        let buffered = self.feed.as_ref().map_or(0, PlaybackFeed::buffered_frames);
        // The frame in hand counts: it is decoded but not in the ring yet.
        let total = ((self.pending.len() + 1) * FRAME + buffered) as f64;
        self.level += LEVEL_SMOOTHING * (total - self.level);
        self.stats.delay_ms = (self.level * 1000.0 / f64::from(RATE)) as f32;
        let error = self.level - self.target() as f64;
        let beyond = if error > DEAD_BAND {
            error - DEAD_BAND
        } else if error < -DEAD_BAND {
            error + DEAD_BAND
        } else {
            0.0
        };
        1.0 + (beyond * PULL_PER_SAMPLE).clamp(-MAX_PULL, MAX_PULL)
    }

    /// After a stall the sender's frames arrive in one burst. Playing them off
    /// at half a percent extra speed would take minutes, so the oldest go.
    fn shed_overflow(&mut self) {
        let limit = self.target().div_ceil(FRAME) + OVERFLOW_FRAMES;
        if self.pending.len() <= limit {
            return;
        }
        let keep = self.target().div_ceil(FRAME);
        while self.pending.len() > keep {
            let Some((seq, _)) = self.pending.pop_first() else { break };
            self.next_seq = seq.wrapping_add(1);
            self.stats.lost += 1;
        }
        self.level = self.target() as f64;
    }

    fn note_underrun(&mut self, now: Instant) {
        self.last_underrun = now;
        if self.concealed_run == 1 {
            self.stats.stalls += 1;
        }
        if self.concealed_run == 1 && now.duration_since(self.last_grow) >= GROW_EVERY && self.jitter_frames < JITTER_MAX {
            self.jitter_frames += 1;
            self.last_grow = now;
        }
    }

    /// Gives delay back once the network has behaved for a while.
    fn relax(&mut self, now: Instant) {
        let quiet = now.duration_since(self.last_underrun) >= SHRINK_AFTER && now.duration_since(self.last_grow) >= SHRINK_AFTER;
        if quiet && self.jitter_frames > JITTER_MIN {
            self.jitter_frames -= 1;
            self.last_grow = now;
        }
    }
}

fn frames_duration(frames: usize) -> Duration {
    Duration::from_millis(frames as u64 * 10)
}

/// Ramps a frame down to silence, so the stream ends without a click.
fn fade(pcm: &mut [f32; FRAME_SAMPLES]) {
    for (index, pair) in pcm.as_chunks_mut::<{ crate::CHANNELS }>().0.iter_mut().enumerate() {
        let gain = 1.0 - index as f32 / FRAME as f32;
        pair[0] *= gain;
        pair[1] *= gain;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::tests::{rms, sine_frame};
    use crate::codec::{Encoder, MAX_PACKET};
    use crate::pcm::{playback_ring, PcmReader, Waker};
    use crate::CHANNELS;

    struct Rig {
        playout: Playout,
        reader: PcmReader,
        encoder: Encoder,
        phase: f64,
        seq: u32,
        now: Instant,
    }

    impl Rig {
        fn new() -> Result<Self, CodecError> {
            let now = Instant::now();
            let (feed, reader) = playback_ring(Waker::default());
            let mut playout = Playout::new(now)?;
            playout.attach(feed);
            Ok(Self { playout, reader, encoder: Encoder::new(128_000, 5)?, phase: 0.0, seq: 0, now })
        }

        fn code(&mut self) -> Result<Vec<u8>, CodecError> {
            let mut packet = [0u8; MAX_PACKET];
            let len = self.encoder.encode(&sine_frame(&mut self.phase, 440.0, 0.5), &mut packet)?;
            Ok(packet[..len].to_vec())
        }

        /// Sends the next frame, or loses it on the way.
        fn send(&mut self, arrives: bool) -> Result<(), CodecError> {
            let payload = self.code()?;
            let flags = if self.seq == 0 { BLOB_START } else { 0 };
            if arrives {
                self.playout.push(&Blob { stream: 0, flags, seq: self.seq, payload: &payload }, self.now);
            }
            self.seq += 1;
            Ok(())
        }

        /// One 10 ms tick of the audio thread and the link thread.
        fn tick(&mut self) -> [f32; FRAME_SAMPLES] {
            self.now += Duration::from_millis(10);
            self.playout.pump(self.now);
            let mut out = [0.0; FRAME_SAMPLES];
            self.reader.read_interleaved(&mut out);
            self.playout.pump(self.now);
            out
        }

        /// Runs `ticks` ticks with a frame arriving on each and returns the audio.
        fn run(&mut self, ticks: usize) -> Result<Vec<f32>, CodecError> {
            let mut out = Vec::new();
            for _ in 0..ticks {
                self.send(true)?;
                out.extend_from_slice(&self.tick());
            }
            Ok(out)
        }
    }

    const TONE_RMS: f32 = 0.5 / std::f32::consts::SQRT_2;

    #[test]
    fn steady_stream_should_play_at_full_level() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        let out = rig.run(100)?;

        let settled = &out[50 * FRAME_SAMPLES..];
        assert!((rms(settled) - TONE_RMS).abs() < 0.02, "rms {}", rms(settled));
        Ok(())
    }

    #[test]
    fn steady_stream_should_never_conceal() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        rig.run(300)?;

        assert_eq!(rig.playout.stats().lost, 0);
        Ok(())
    }

    #[test]
    fn steady_stream_should_settle_near_its_target_delay() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        rig.run(300)?;

        let target_ms = rig.playout.target() as f32 * 1000.0 / RATE as f32;
        let delay = rig.playout.stats().delay_ms;
        assert!((delay - target_ms).abs() <= 10.0, "delay {delay} ms, target {target_ms} ms");
        Ok(())
    }

    #[test]
    fn playback_should_wait_for_the_cushion() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        rig.send(true)?;
        let out = rig.tick();

        assert!(out.iter().all(|sample| *sample == 0.0));
        Ok(())
    }

    #[test]
    fn one_lost_frame_should_be_concealed() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        rig.run(50)?;
        rig.send(false)?;
        let mut out = rig.tick().to_vec();
        out.extend(rig.run(20)?);

        // No tick of the output falls silent around the hole.
        let quietest = out.as_chunks::<FRAME_SAMPLES>().0.iter().map(|tick| rms(tick)).fold(f32::MAX, f32::min);
        assert!(quietest > 0.2, "quietest tick rms {quietest}");
        assert_eq!(rig.playout.stats().lost, 1);
        Ok(())
    }

    #[test]
    fn repeated_frame_should_fill_the_hole_it_arrives_for() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        rig.run(50)?;
        let payload = rig.code()?;
        let lost_seq = rig.seq;
        rig.seq += 1;
        rig.tick();
        // The next packet carries the lost frame again.
        rig.playout.push(&Blob { stream: 0, flags: BLOB_REPEAT, seq: lost_seq, payload: &payload }, rig.now);
        rig.run(20)?;

        assert_eq!(rig.playout.stats().lost, 0);
        Ok(())
    }

    #[test]
    fn repeat_of_a_played_frame_should_not_count_as_late() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        rig.run(50)?;
        let payload = rig.code()?;
        rig.playout.push(&Blob { stream: 0, flags: BLOB_REPEAT, seq: 3, payload: &payload }, rig.now);

        assert_eq!(rig.playout.stats().late, 0);
        Ok(())
    }

    #[test]
    fn frames_out_of_order_should_play_in_order() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        rig.run(50)?;
        let first = rig.code()?;
        let second = rig.code()?;
        let seq = rig.seq;
        rig.seq += 2;
        rig.playout.push(&Blob { stream: 0, flags: 0, seq: seq + 1, payload: &second }, rig.now);
        rig.playout.push(&Blob { stream: 0, flags: 0, seq, payload: &first }, rig.now);
        rig.tick();
        rig.tick();
        rig.run(10)?;

        assert_eq!((rig.playout.stats().lost, rig.playout.stats().late), (0, 0));
        Ok(())
    }

    #[test]
    fn stream_that_stops_should_fall_silent_and_go_idle() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        rig.run(50)?;
        let mut last = [1.0; FRAME_SAMPLES];
        for _ in 0..40 {
            last = rig.tick();
        }

        assert!(last.iter().all(|sample| *sample == 0.0));
        assert!(!rig.playout.is_active());
        Ok(())
    }

    #[test]
    fn ended_run_should_stop_without_counting_a_stall() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        rig.run(100)?;
        rig.playout.push(&Blob { stream: 0, flags: BLOB_END, seq: rig.seq, payload: &[] }, rig.now);
        for _ in 0..20 {
            rig.tick();
        }

        assert_eq!((rig.playout.stats().stalls, rig.playout.is_active()), (0, false));
        Ok(())
    }

    #[test]
    fn ended_run_should_not_deepen_the_cushion() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        rig.run(300)?;
        rig.playout.push(&Blob { stream: 0, flags: BLOB_END, seq: rig.seq, payload: &[] }, rig.now);
        for _ in 0..20 {
            rig.tick();
        }

        assert_eq!(rig.playout.jitter_frames, JITTER_MIN);
        Ok(())
    }

    #[test]
    fn ended_run_should_fall_silent() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        rig.run(100)?;
        rig.playout.push(&Blob { stream: 0, flags: BLOB_END, seq: rig.seq, payload: &[] }, rig.now);
        let mut last = [1.0; FRAME_SAMPLES];
        for _ in 0..20 {
            last = rig.tick();
        }

        assert!(last.iter().all(|sample| *sample == 0.0));
        Ok(())
    }

    #[test]
    fn run_shorter_than_the_cushion_should_play_once_it_has_ended() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        rig.send(true)?;
        rig.send(true)?;
        rig.playout.push(&Blob { stream: 0, flags: BLOB_END, seq: rig.seq, payload: &[] }, rig.now);
        let mut heard = Vec::new();
        for _ in 0..4 {
            heard.extend_from_slice(&rig.tick());
        }

        assert!(rms(&heard) > 0.05, "rms {}", rms(&heard));
        Ok(())
    }

    #[test]
    fn end_reported_without_a_marker_should_stop_after_the_buffered_frames() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        rig.run(100)?;
        rig.playout.end_after_pending();
        for _ in 0..20 {
            rig.tick();
        }

        assert_eq!((rig.playout.stats().stalls, rig.playout.is_active()), (0, false));
        Ok(())
    }

    #[test]
    fn run_that_continues_past_a_stale_end_should_keep_playing() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        rig.run(50)?;
        rig.playout.end_after_pending();
        rig.run(50)?;

        let stats = rig.playout.stats();
        assert_eq!((stats.lost, stats.restarts, rig.playout.is_active()), (0, 1, true));
        Ok(())
    }

    #[test]
    fn stream_that_resumes_should_play_again() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        rig.run(50)?;
        for _ in 0..40 {
            rig.tick();
        }
        // The sender's gate reopens: a new run, numbered on from the old one.
        let payload = rig.code()?;
        rig.playout.push(&Blob { stream: 0, flags: BLOB_START, seq: rig.seq, payload: &payload }, rig.now);
        rig.seq += 1;
        let out = rig.run(60)?;

        let settled = &out[40 * FRAME_SAMPLES..];
        assert!((rms(settled) - TONE_RMS).abs() < 0.02, "rms {}", rms(settled));
        Ok(())
    }

    #[test]
    fn short_stall_should_not_lose_the_late_frames() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        rig.run(100)?;
        // 40 ms without packets, then the four held frames arrive at once.
        let held: Vec<Vec<u8>> = (0..4).map(|_| rig.code()).collect::<Result<_, _>>()?;
        let first = rig.seq;
        rig.seq += 4;
        for _ in 0..4 {
            rig.tick();
        }
        for (offset, payload) in held.iter().enumerate() {
            rig.playout.push(&Blob { stream: 0, flags: 0, seq: first + offset as u32, payload }, rig.now);
        }
        rig.run(50)?;

        assert_eq!((rig.playout.stats().lost, rig.playout.stats().late), (0, 0));
        Ok(())
    }

    #[test]
    fn stall_should_deepen_the_cushion() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        rig.run(300)?;
        for _ in 0..4 {
            rig.tick();
        }

        assert_eq!(rig.playout.jitter_frames, JITTER_MIN + 1);
        Ok(())
    }

    #[test]
    fn burst_after_a_long_stall_should_be_cut_to_the_cushion() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        rig.run(50)?;
        for _ in 0..40 {
            rig.send(true)?;
        }
        rig.tick();

        let cushion = rig.playout.target().div_ceil(FRAME);
        assert!(rig.playout.pending.len() <= cushion, "{} frames still pending", rig.playout.pending.len());
        Ok(())
    }

    #[test]
    fn fast_sender_should_be_followed_without_dropping_frames() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        // The sender's clock runs 0.2 % fast: one extra frame every 500.
        for tick in 0..6000 {
            rig.send(true)?;
            if tick % 500 == 499 {
                rig.send(true)?;
            }
            rig.tick();
        }

        let stats = rig.playout.stats();
        assert_eq!((stats.lost, stats.late, stats.restarts), (0, 0, 1));
        Ok(())
    }

    #[test]
    fn slow_sender_should_be_followed_without_concealing() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        // The sender's clock runs 0.2 % slow: one frame short every 500.
        for tick in 0..6000 {
            if tick % 500 != 499 {
                rig.send(true)?;
            }
            rig.tick();
        }

        let stats = rig.playout.stats();
        assert_eq!((stats.lost, stats.late, stats.restarts), (0, 0, 1));
        Ok(())
    }

    #[test]
    fn garbage_frame_should_leave_a_silent_gap_and_keep_playing() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        rig.run(50)?;
        rig.playout.push(&Blob { stream: 0, flags: 0, seq: rig.seq, payload: &[0x08, 0xff, 0xfe] }, rig.now);
        rig.seq += 1;
        rig.tick();
        let out = rig.run(40)?;

        let settled = &out[20 * FRAME_SAMPLES..];
        assert!((rms(settled) - TONE_RMS).abs() < 0.02, "rms {}", rms(settled));
        Ok(())
    }

    #[test]
    fn detached_stream_should_discard_frames() -> Result<(), CodecError> {
        let mut rig = Rig::new()?;
        let (feed, reader) = playback_ring(Waker::default());
        rig.playout.attach(feed);
        drop(reader);
        rig.send(true)?;

        assert!(rig.playout.pending.is_empty());
        Ok(())
    }

    #[test]
    fn fade_should_end_at_silence() {
        let mut pcm = [1.0; FRAME_SAMPLES];
        fade(&mut pcm);

        assert!(pcm[FRAME_SAMPLES - CHANNELS].abs() < 0.01 && pcm[0] == 1.0);
    }
}
