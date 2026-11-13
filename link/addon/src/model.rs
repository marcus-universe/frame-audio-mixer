//! What the overlay shows and what it can ask for. The audio side and the
//! overlay only meet through these types.

use frame_link_core::LinkStatus;
pub use frame_link_core::STREAMS;

/// The two kinds of stream, as the PC sees them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// B1–B4 of the headset mixer, arriving on the PC as microphones.
    Mic,
    /// PC1–PC4: PC outputs whose sound is sent to the headset mixer.
    Output,
}

impl Kind {
    /// "B" or "PC": stream `index` of this kind is labelled `{prefix}{index + 1}`.
    #[must_use]
    pub fn prefix(self) -> &'static str {
        match self {
            Kind::Mic => "B",
            Kind::Output => "PC",
        }
    }
}

/// Whether the PC sound device behind a stream is there.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DeviceState {
    /// No device with this stream's name exists: the virtual devices are not installed.
    #[default]
    Missing,
    /// The device is open and audio can flow.
    Ready,
    /// The device exists but could not be opened. It is retried.
    Failed,
}

/// The PC sound devices of all eight streams.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AudioStatus {
    pub mics: [DeviceState; STREAMS],
    pub outputs: [DeviceState; STREAMS],
}

impl AudioStatus {
    #[must_use]
    pub fn of(&self, kind: Kind) -> &[DeviceState; STREAMS] {
        match kind {
            Kind::Mic => &self.mics,
            Kind::Output => &self.outputs,
        }
    }
}

/// Everything the overlay draws, taken at one moment.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    /// Session with the headset. `link.names` are the headset's names for B1–B4.
    /// `link.remote_recv` has a bit per PC output the headset mixer has a channel for.
    /// `link.receiving` and `link.sending` have a bit per stream carrying sound now.
    pub link: LinkStatus,
    pub audio: AudioStatus,
    /// The user's switches: which microphones the PC takes, which outputs it sends.
    pub mics_on: [bool; STREAMS],
    pub outputs_on: [bool; STREAMS],
}

impl Snapshot {
    #[must_use]
    pub fn switched_on(&self, kind: Kind) -> &[bool; STREAMS] {
        match kind {
            Kind::Mic => &self.mics_on,
            Kind::Output => &self.outputs_on,
        }
    }

    /// True while sound is moving on this stream.
    #[must_use]
    pub fn carrying(&self, kind: Kind, index: usize) -> bool {
        let mask = match kind {
            Kind::Mic => self.link.receiving,
            Kind::Output => self.link.sending,
        };
        frame_link_core::mask_has(mask, index)
    }
}

/// What a press in the overlay asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Switch one stream on or off.
    Toggle(Kind, usize),
    /// Show the overlay in the language with this code from now on.
    Language(&'static str),
}
