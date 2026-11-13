//! The audio link between Frame Audio Mixer on the headset and Frame Audio Link
//! on the PC.
//!
//! Four stereo streams run each way over one UDP socket: B1–B4 from the headset
//! to the PC, and PC1–PC4 from the PC to the headset. Every stream is 48 kHz
//! Opus in 10 ms frames, the codec the stock PC streaming microphone uses.
//!
//! Both ends run the same [`Link`]. The PC is the [`Role::Host`]: it finds the
//! headset and decides which streams are switched on. The headset is the
//! [`Role::Frame`]: it answers, and says which streams it has a use for. A
//! stream only costs bandwidth while both ends want it and it carries sound.
//!
//! Audio threads never touch the network or the codec. They trade samples with
//! the link thread through lock-free rings ([`PcmWriter`], [`PcmReader`]).

mod codec;
mod link;
mod pcm;
mod playout;
mod proto;
mod resample;
mod sender;

pub use codec::CodecError;
pub use link::{AllowPeer, Candidates, Link, LinkConfig, LinkError, LinkState, LinkStatus, Role, StreamStats};
pub use pcm::{PcmReader, PcmWriter};
pub use proto::{DEFAULT_PORT, PROTOCOL_VERSION};

/// Sample rate of every stream.
pub const RATE: u32 = 48_000;
/// Samples per channel in one coded frame: 10 ms.
pub const FRAME: usize = 480;
/// Every stream is stereo.
pub const CHANNELS: usize = 2;
/// Streams in each direction: B1–B4 up, PC1–PC4 down.
pub const STREAMS: usize = 4;
/// Interleaved samples in one frame.
pub(crate) const FRAME_SAMPLES: usize = FRAME * CHANNELS;

/// A set of streams, one bit per stream. Bit 0 is stream 1.
pub type Mask = u8;

/// Every stream switched on.
pub const ALL_STREAMS: Mask = (1 << STREAMS) - 1;

/// Whether `mask` has the bit of stream `index` set.
#[must_use]
pub fn mask_has(mask: Mask, index: usize) -> bool {
    index < STREAMS && mask & (1 << index) != 0
}
