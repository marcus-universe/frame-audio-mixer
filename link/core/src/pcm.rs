//! Lock-free sample rings between an audio thread and the link thread.
//!
//! Samples are interleaved stereo f32. The audio side never blocks, allocates
//! or waits: a full ring drops the block, an empty one reads as silence.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::Thread;

use rtrb::{Consumer, Producer, RingBuffer};

use crate::{CHANNELS, FRAME, FRAME_SAMPLES};

/// Frames a ring holds: a third of a second. The link keeps far less in it; the
/// rest absorbs a stalled thread without dropping samples.
const RING_FRAMES: usize = 16_384;

/// Wakes the link thread. Safe to call from a realtime thread: when the link
/// is already awake it is one atomic swap.
#[derive(Clone, Default)]
pub(crate) struct Waker(Option<Thread>);

impl Waker {
    pub(crate) fn new(thread: Thread) -> Self {
        Self(Some(thread))
    }

    pub(crate) fn wake(&self) {
        if let Some(thread) = &self.0 {
            thread.unpark();
        }
    }
}

/// The audio thread's end of a capture ring: samples on their way to the peer.
pub struct PcmWriter {
    producer: Producer<f32>,
    waker: Waker,
    dropped: Arc<AtomicU64>,
    /// Cleared by the link while nobody would hear this stream. Writing then
    /// costs one atomic load.
    wanted: Arc<AtomicBool>,
}

impl PcmWriter {
    /// Queues one block given as separate left and right channels.
    ///
    /// A block that does not fit is dropped whole, so the two channels can
    /// never slip against each other.
    pub fn write_planar(&mut self, left: &[f32], right: &[f32]) {
        if !self.wanted.load(Ordering::Relaxed) {
            return;
        }
        let frames = left.len().min(right.len());
        match self.producer.write_chunk_uninit(frames * CHANNELS) {
            Ok(chunk) => {
                chunk.fill_from_iter(left.iter().zip(right).flat_map(|(l, r)| [*l, *r]));
            }
            Err(_) => {
                self.dropped.fetch_add(frames as u64, Ordering::Relaxed);
            }
        }
        self.waker.wake();
    }

    /// Queues one block of interleaved stereo samples.
    pub fn write_interleaved(&mut self, samples: &[f32]) {
        if !self.wanted.load(Ordering::Relaxed) {
            return;
        }
        let whole = samples.len() - samples.len() % CHANNELS;
        if self.producer.push_entire_slice(&samples[..whole]).is_err() {
            self.dropped.fetch_add((whole / CHANNELS) as u64, Ordering::Relaxed);
        }
        self.waker.wake();
    }

    /// True while the peer is taking this stream. A capture that is costly to
    /// keep running can pause while this is false.
    #[must_use]
    pub fn is_wanted(&self) -> bool {
        self.wanted.load(Ordering::Relaxed)
    }

    /// True once the link has let go of this stream. The writer is then useless.
    #[must_use]
    pub fn is_detached(&self) -> bool {
        self.producer.is_abandoned()
    }
}

/// The link thread's end of a capture ring.
pub(crate) struct CaptureTap {
    consumer: Consumer<f32>,
    dropped: Arc<AtomicU64>,
    wanted: Arc<AtomicBool>,
}

impl CaptureTap {
    pub(crate) fn available_frames(&self) -> usize {
        self.consumer.slots() / CHANNELS
    }

    /// Takes exactly one coded frame. False when less than a frame is waiting.
    pub(crate) fn pop_frame(&mut self, out: &mut [f32; FRAME_SAMPLES]) -> bool {
        self.consumer.pop_entire_slice(out).is_ok()
    }

    /// Throws away `frames` queued frames.
    pub(crate) fn skip(&mut self, frames: usize) {
        let samples = (frames * CHANNELS).min(self.consumer.slots());
        if let Ok(chunk) = self.consumer.read_chunk(samples) {
            chunk.commit_all();
        }
    }

    pub(crate) fn is_abandoned(&self) -> bool {
        self.consumer.is_abandoned()
    }

    /// Tells the audio thread whether its samples are being sent.
    pub(crate) fn set_wanted(&self, wanted: bool) {
        self.wanted.store(wanted, Ordering::Relaxed);
    }

    /// Frames the audio thread could not queue since the last call.
    pub(crate) fn take_dropped(&self) -> u64 {
        self.dropped.swap(0, Ordering::Relaxed)
    }
}

pub(crate) fn capture_ring(waker: Waker) -> (PcmWriter, CaptureTap) {
    let (producer, consumer) = RingBuffer::new(RING_FRAMES * CHANNELS);
    let dropped = Arc::new(AtomicU64::new(0));
    let wanted = Arc::new(AtomicBool::new(true));
    (
        PcmWriter { producer, waker, dropped: Arc::clone(&dropped), wanted: Arc::clone(&wanted) },
        CaptureTap { consumer, dropped, wanted },
    )
}

/// The audio thread's end of a playback ring: samples that came from the peer.
pub struct PcmReader {
    consumer: Consumer<f32>,
    waker: Waker,
    quantum: Arc<AtomicUsize>,
    /// Set by the link while this stream plays. Only then does a read wake it.
    live: Arc<AtomicBool>,
}

impl PcmReader {
    /// Fills separate left and right channels. Whatever the link has not
    /// delivered yet reads as silence.
    pub fn read_planar(&mut self, left: &mut [f32], right: &mut [f32]) {
        let frames = left.len().min(right.len());
        let ready = (self.consumer.slots() / CHANNELS).min(frames);
        if let Ok(chunk) = self.consumer.read_chunk(ready * CHANNELS) {
            let mut samples = chunk.into_iter();
            for (l, r) in left.iter_mut().zip(right.iter_mut()).take(ready) {
                *l = samples.next().unwrap_or(0.0);
                *r = samples.next().unwrap_or(0.0);
            }
        }
        left[ready..].fill(0.0);
        right[ready..].fill(0.0);
        self.after_read(frames);
    }

    /// Fills interleaved stereo samples. Whatever the link has not delivered
    /// yet reads as silence.
    pub fn read_interleaved(&mut self, out: &mut [f32]) {
        let frames = out.len() / CHANNELS;
        let ready = (self.consumer.slots() / CHANNELS).min(frames);
        let (filled, rest) = out.split_at_mut(ready * CHANNELS);
        if self.consumer.pop_entire_slice(filled).is_err() {
            filled.fill(0.0);
        }
        rest.fill(0.0);
        self.after_read(frames);
    }

    fn after_read(&self, frames: usize) {
        self.quantum.store(frames, Ordering::Relaxed);
        if self.live.load(Ordering::Relaxed) {
            self.waker.wake();
        }
    }

    /// True once the link has let go of this stream. It then only reads silence.
    #[must_use]
    pub fn is_detached(&self) -> bool {
        self.consumer.is_abandoned()
    }
}

/// The link thread's end of a playback ring.
pub(crate) struct PlaybackFeed {
    producer: Producer<f32>,
    quantum: Arc<AtomicUsize>,
    live: Arc<AtomicBool>,
}

impl PlaybackFeed {
    /// Frames queued and not yet read by the audio thread.
    pub(crate) fn buffered_frames(&self) -> usize {
        (self.producer.buffer().capacity() - self.producer.slots()) / CHANNELS
    }

    /// Frames the audio thread took in its last read. One coded frame until it
    /// has read for the first time.
    pub(crate) fn quantum(&self) -> usize {
        match self.quantum.load(Ordering::Relaxed) {
            0 => FRAME,
            frames => frames.min(RING_FRAMES / 4),
        }
    }

    /// Queues interleaved samples. Returns false when the ring had no room for
    /// all of them; nothing is queued then.
    pub(crate) fn push(&mut self, samples: &[f32]) -> bool {
        self.producer.push_entire_slice(samples).is_ok()
    }

    pub(crate) fn is_abandoned(&self) -> bool {
        self.producer.is_abandoned()
    }

    /// Whether a read of the audio thread should wake the link.
    pub(crate) fn set_live(&self, live: bool) {
        self.live.store(live, Ordering::Relaxed);
    }
}

pub(crate) fn playback_ring(waker: Waker) -> (PlaybackFeed, PcmReader) {
    let (producer, consumer) = RingBuffer::new(RING_FRAMES * CHANNELS);
    let quantum = Arc::new(AtomicUsize::new(0));
    let live = Arc::new(AtomicBool::new(false));
    (
        PlaybackFeed { producer, quantum: Arc::clone(&quantum), live: Arc::clone(&live) },
        PcmReader { consumer, waker, quantum, live },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planar_write_should_interleave_left_and_right() {
        let (mut writer, mut tap) = capture_ring(Waker::default());
        writer.write_planar(&[1.0; FRAME], &[-1.0; FRAME]);

        let mut frame = [0.0; FRAME_SAMPLES];
        assert!(tap.pop_frame(&mut frame));
        assert_eq!(&frame[..4], &[1.0, -1.0, 1.0, -1.0]);
    }

    #[test]
    fn pop_frame_should_wait_for_a_whole_frame() {
        let (mut writer, mut tap) = capture_ring(Waker::default());
        writer.write_interleaved(&[0.5; FRAME_SAMPLES - CHANNELS]);

        let mut frame = [0.0; FRAME_SAMPLES];
        assert!(!tap.pop_frame(&mut frame));
    }

    #[test]
    fn full_capture_ring_should_drop_the_block_and_count_it() {
        let (mut writer, tap) = capture_ring(Waker::default());
        let block = vec![0.1; RING_FRAMES * CHANNELS];
        writer.write_interleaved(&block);
        writer.write_interleaved(&[0.1; FRAME_SAMPLES]);

        assert_eq!(tap.take_dropped(), FRAME as u64);
    }

    #[test]
    fn empty_playback_ring_should_read_silence() {
        let (_feed, mut reader) = playback_ring(Waker::default());
        let (mut left, mut right) = ([9.0; 64], [9.0; 64]);
        reader.read_planar(&mut left, &mut right);

        assert!(left.iter().chain(right.iter()).all(|sample| *sample == 0.0));
    }

    #[test]
    fn short_playback_ring_should_pad_the_tail_with_silence() {
        let (mut feed, mut reader) = playback_ring(Waker::default());
        assert!(feed.push(&[0.25, -0.25, 0.5, -0.5]));
        let (mut left, mut right) = ([9.0; 4], [9.0; 4]);
        reader.read_planar(&mut left, &mut right);

        assert_eq!((left, right), ([0.25, 0.5, 0.0, 0.0], [-0.25, -0.5, 0.0, 0.0]));
    }

    #[test]
    fn playback_feed_should_report_the_readers_block_size() {
        let (feed, mut reader) = playback_ring(Waker::default());
        let mut out = [0.0; 1024 * CHANNELS];
        reader.read_interleaved(&mut out);

        assert_eq!(feed.quantum(), 1024);
    }

    #[test]
    fn unwanted_capture_should_not_queue_samples() {
        let (mut writer, tap) = capture_ring(Waker::default());
        tap.set_wanted(false);
        writer.write_interleaved(&[0.5; FRAME_SAMPLES]);

        assert_eq!(tap.available_frames(), 0);
    }

    #[test]
    fn dropping_the_reader_should_abandon_the_feed() {
        let (feed, reader) = playback_ring(Waker::default());
        drop(reader);

        assert!(feed.is_abandoned());
    }
}
