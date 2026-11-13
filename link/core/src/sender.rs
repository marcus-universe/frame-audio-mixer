//! One outgoing stream: frames from the capture ring, coded, with a gate that
//! stops sending while the stream is silent.

use std::time::{Duration, Instant};

use crate::codec::{CodecError, Encoder, MAX_PACKET};
use crate::pcm::CaptureTap;
use crate::proto::{Blob, BLOB_END, BLOB_REPEAT, BLOB_START};
use crate::{FRAME, FRAME_SAMPLES};

/// Peak below which a frame counts as silent: about -90 dBFS, under the noise
/// floor of 16-bit audio. Digital silence from an idle device is exactly zero.
const SILENCE_PEAK: f32 = 3.0e-5;
/// Silent frames before the stream stops sending: a quarter of a second, so
/// the pauses inside speech and between songs stay in one piece.
const GATE_FRAMES: u32 = 25;
/// Queued frames at which the link has fallen behind the audio thread. The
/// oldest are dropped, so a stall cannot turn into standing latency.
const BACKLOG_FRAMES: usize = 12;
/// Frames kept when a backlog is cut.
const BACKLOG_KEEP: usize = 2;
/// A capture that delivers nothing for this long has stopped: a PC output
/// with nothing playing on it produces no samples at all, not silent ones.
const STARVED_AFTER: Duration = Duration::from_millis(150);

/// One coded frame, ready for the wire.
pub(crate) struct Coded {
    pub stream: usize,
    pub seq: u32,
    pub flags: u8,
    len: usize,
    bytes: [u8; MAX_PACKET],
}

impl Coded {
    pub(crate) fn blob(&self) -> Blob<'_> {
        Blob { stream: self.stream, flags: self.flags, seq: self.seq, payload: &self.bytes[..self.len] }
    }

    /// The same frame, marked as a second copy sent with the following one.
    pub(crate) fn repeat_blob(&self) -> Blob<'_> {
        Blob { flags: self.flags | BLOB_REPEAT, ..self.blob() }
    }

    /// The empty frame that closes a run. `seq` is the number the next coded
    /// frame would have had.
    fn end(stream: usize, seq: u32) -> Self {
        Self { stream, seq, flags: BLOB_END, len: 0, bytes: [0; MAX_PACKET] }
    }
}

pub(crate) struct StreamSender {
    stream: usize,
    tap: Option<CaptureTap>,
    encoder: Encoder,
    seq: u32,
    silent_frames: u32,
    /// Closed while the stream is silent or not wanted. Opening it starts a
    /// new run of frames the receiver decodes without history.
    gate_open: bool,
    /// Frames dropped because the audio thread or this thread fell behind.
    pub overruns: u64,
    /// When the capture last delivered a frame.
    last_frame: Option<Instant>,
    pcm: [f32; FRAME_SAMPLES],
}

impl StreamSender {
    pub(crate) fn new(stream: usize, bitrate: i32, complexity: i32) -> Result<Self, CodecError> {
        Ok(Self {
            stream,
            tap: None,
            encoder: Encoder::new(bitrate, complexity)?,
            seq: 0,
            silent_frames: GATE_FRAMES,
            gate_open: false,
            overruns: 0,
            last_frame: None,
            pcm: [0.0; FRAME_SAMPLES],
        })
    }

    pub(crate) fn attach(&mut self, tap: CaptureTap) {
        tap.set_wanted(false);
        self.tap = Some(tap);
        self.close_gate();
    }

    /// Stops the stream: the audio thread stops queueing and the next frame
    /// starts a new run.
    pub(crate) fn stop(&mut self) {
        if let Some(tap) = self.tap.as_mut() {
            tap.set_wanted(false);
            let waiting = tap.available_frames();
            tap.skip(waiting);
        }
        self.close_gate();
    }

    /// True while frames of this stream are going out.
    pub(crate) fn is_active(&self) -> bool {
        self.gate_open
    }

    pub(crate) fn close_gate(&mut self) {
        self.gate_open = false;
        self.silent_frames = GATE_FRAMES;
    }

    /// Codes every whole frame waiting in the ring and appends it to `out`.
    /// While `flowing` is false the ring is only emptied.
    pub(crate) fn poll(&mut self, flowing: bool, now: Instant, out: &mut Vec<Coded>) {
        let starved = self.last_frame.is_some_and(|at| now.duration_since(at) >= STARVED_AFTER);
        let detached = self.tap.as_ref().is_some_and(CaptureTap::is_abandoned);
        if self.gate_open && (starved || detached || !flowing) {
            out.push(Coded::end(self.stream, self.seq));
            self.close_gate();
        }
        if detached {
            self.tap = None;
        }
        let Some(tap) = self.tap.as_mut() else { return };
        tap.set_wanted(flowing);
        self.overruns += tap.take_dropped();
        let waiting = tap.available_frames() / FRAME;
        if waiting > BACKLOG_FRAMES {
            let cut = waiting - BACKLOG_KEEP;
            tap.skip(cut * FRAME);
            self.overruns += (cut * FRAME) as u64;
        }
        while tap.pop_frame(&mut self.pcm) {
            self.last_frame = Some(now);
            if !flowing {
                self.gate_open = false;
                self.silent_frames = GATE_FRAMES;
                continue;
            }
            let peak = self.pcm.iter().fold(0.0f32, |peak, sample| peak.max(sample.abs()));
            // A NaN or infinite peak counts as silence: it must not reach the codec.
            if peak > SILENCE_PEAK && peak.is_finite() {
                self.silent_frames = 0;
            } else {
                self.silent_frames = self.silent_frames.saturating_add(1);
            }
            if self.silent_frames >= GATE_FRAMES {
                if self.gate_open {
                    out.push(Coded::end(self.stream, self.seq));
                    self.gate_open = false;
                }
                continue;
            }
            let mut flags = 0;
            if !self.gate_open {
                self.encoder.reset();
                self.gate_open = true;
                flags = BLOB_START;
            }
            let mut coded = Coded { stream: self.stream, seq: self.seq, flags, len: 0, bytes: [0; MAX_PACKET] };
            match self.encoder.encode(&self.pcm, &mut coded.bytes) {
                Ok(len) => {
                    coded.len = len;
                    self.seq = self.seq.wrapping_add(1);
                    out.push(coded);
                }
                // The encoder state is suspect; the next frame starts a fresh run.
                Err(_) => {
                    self.gate_open = false;
                    self.silent_frames = GATE_FRAMES;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::tests::sine_frame;
    use crate::pcm::{capture_ring, PcmWriter, Waker};

    fn sender() -> Result<(StreamSender, PcmWriter), CodecError> {
        let (writer, tap) = capture_ring(Waker::default());
        let mut sender = StreamSender::new(1, 128_000, 5)?;
        sender.attach(tap);
        // The link opens the ring on its first poll of a wanted stream.
        sender.poll(true, Instant::now(), &mut Vec::new());
        Ok((sender, writer))
    }

    fn tone(writer: &mut PcmWriter, frames: usize) {
        let mut phase = 0.0;
        for _ in 0..frames {
            writer.write_interleaved(&sine_frame(&mut phase, 440.0, 0.5));
        }
    }

    #[test]
    fn first_audible_frame_should_start_a_run() -> Result<(), CodecError> {
        let (mut sender, mut writer) = sender()?;
        tone(&mut writer, 3);
        let mut out = Vec::new();
        sender.poll(true, Instant::now(), &mut out);

        let flags: Vec<u8> = out.iter().map(|coded| coded.flags).collect();
        assert_eq!(flags, vec![BLOB_START, 0, 0]);
        Ok(())
    }

    #[test]
    fn sequence_numbers_should_count_up_by_one() -> Result<(), CodecError> {
        let (mut sender, mut writer) = sender()?;
        tone(&mut writer, 3);
        let mut out = Vec::new();
        sender.poll(true, Instant::now(), &mut out);

        let seqs: Vec<u32> = out.iter().map(|coded| coded.seq).collect();
        assert_eq!(seqs, vec![0, 1, 2]);
        Ok(())
    }

    #[test]
    fn silence_should_send_nothing() -> Result<(), CodecError> {
        let (mut sender, mut writer) = sender()?;
        for _ in 0..10 {
            writer.write_interleaved(&[0.0; FRAME_SAMPLES]);
        }
        let mut out = Vec::new();
        sender.poll(true, Instant::now(), &mut out);

        assert!(out.is_empty());
        Ok(())
    }

    #[test]
    fn gate_should_stay_open_through_a_short_pause() -> Result<(), CodecError> {
        let (mut sender, mut writer) = sender()?;
        tone(&mut writer, 2);
        let mut out = Vec::new();
        sender.poll(true, Instant::now(), &mut out);
        // Polled in pieces, as the link does, so the backlog guard stays out of it.
        for _ in 0..GATE_FRAMES - 1 {
            writer.write_interleaved(&[0.0; FRAME_SAMPLES]);
            sender.poll(true, Instant::now(), &mut out);
        }

        assert_eq!(out.len(), 2 + GATE_FRAMES as usize - 1);
        Ok(())
    }

    #[test]
    fn sound_after_a_long_pause_should_start_a_new_run() -> Result<(), CodecError> {
        let (mut sender, mut writer) = sender()?;
        tone(&mut writer, 2);
        let mut out = Vec::new();
        sender.poll(true, Instant::now(), &mut out);
        for _ in 0..GATE_FRAMES + 5 {
            writer.write_interleaved(&[0.0; FRAME_SAMPLES]);
            sender.poll(true, Instant::now(), &mut out);
        }
        out.clear();
        tone(&mut writer, 1);
        sender.poll(true, Instant::now(), &mut out);

        assert_eq!(out.first().map(|coded| coded.flags), Some(BLOB_START));
        Ok(())
    }

    #[test]
    fn unwanted_stream_should_be_drained_without_coding() -> Result<(), CodecError> {
        let (mut sender, mut writer) = sender()?;
        tone(&mut writer, 4);
        let mut out = Vec::new();
        sender.poll(false, Instant::now(), &mut out);

        assert!(out.is_empty() && !sender.is_active());
        Ok(())
    }

    #[test]
    fn unwanted_stream_should_tell_the_audio_thread_to_stop() -> Result<(), CodecError> {
        let (mut sender, writer) = sender()?;
        sender.poll(false, Instant::now(), &mut Vec::new());

        assert!(!writer.is_wanted());
        Ok(())
    }

    #[test]
    fn backlog_should_be_cut_to_the_newest_frames() -> Result<(), CodecError> {
        let (mut sender, mut writer) = sender()?;
        tone(&mut writer, BACKLOG_FRAMES + 8);
        let mut out = Vec::new();
        sender.poll(true, Instant::now(), &mut out);

        assert_eq!(out.len(), BACKLOG_KEEP);
        Ok(())
    }

    #[test]
    fn silence_after_sound_should_mark_the_end_of_the_run() -> Result<(), CodecError> {
        let (mut sender, mut writer) = sender()?;
        tone(&mut writer, 2);
        let mut out = Vec::new();
        sender.poll(true, Instant::now(), &mut out);
        for _ in 0..GATE_FRAMES {
            writer.write_interleaved(&[0.0; FRAME_SAMPLES]);
            sender.poll(true, Instant::now(), &mut out);
        }

        let last = out.last().map(|coded| (coded.flags, coded.seq, coded.blob().payload.len()));
        // Two frames of tone and the pause up to the gate went out before it.
        assert_eq!(last, Some((BLOB_END, 2 + GATE_FRAMES - 1, 0)));
        Ok(())
    }

    #[test]
    fn stream_switched_off_mid_sound_should_mark_the_end_of_the_run() -> Result<(), CodecError> {
        let (mut sender, mut writer) = sender()?;
        tone(&mut writer, 2);
        let mut out = Vec::new();
        sender.poll(true, Instant::now(), &mut out);
        out.clear();
        sender.poll(false, Instant::now(), &mut out);

        let flags: Vec<u8> = out.iter().map(|coded| coded.flags).collect();
        assert_eq!(flags, vec![BLOB_END]);
        Ok(())
    }

    #[test]
    fn silent_stream_should_not_mark_an_end() -> Result<(), CodecError> {
        let (mut sender, mut writer) = sender()?;
        let mut out = Vec::new();
        for _ in 0..GATE_FRAMES + 5 {
            writer.write_interleaved(&[0.0; FRAME_SAMPLES]);
            sender.poll(true, Instant::now(), &mut out);
        }

        assert!(out.is_empty());
        Ok(())
    }

    #[test]
    fn capture_that_stops_delivering_should_end_the_run() -> Result<(), CodecError> {
        let (mut sender, mut writer) = sender()?;
        tone(&mut writer, 2);
        let mut out = Vec::new();
        let start = Instant::now();
        sender.poll(true, start, &mut out);
        sender.poll(true, start + STARVED_AFTER, &mut out);

        assert!(!sender.is_active());
        assert_eq!(out.last().map(|coded| coded.flags), Some(BLOB_END));
        Ok(())
    }

    #[test]
    fn dropped_writer_should_detach_the_stream() -> Result<(), CodecError> {
        let (mut sender, writer) = sender()?;
        drop(writer);
        let mut out = Vec::new();
        sender.poll(true, Instant::now(), &mut out);

        assert!(sender.tap.is_none());
        Ok(())
    }
}
