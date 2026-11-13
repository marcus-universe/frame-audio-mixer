//! The PC sound devices behind the eight streams.
//!
//! B1–B4 arrive from the headset and are played into virtual microphones.
//! PC1–PC4 are virtual outputs whose sound is captured and sent to the headset.
//! Windows opens devices that `devices install` created; Linux creates its own
//! PipeWire nodes for as long as the add-on runs.
//!
//! Each platform module provides a `Backend` with the same two functions:
//! `start(&Link, &Config) -> anyhow::Result<Backend>` and
//! `status(&self) -> AudioStatus`. Dropping it closes every device.

#[cfg(target_os = "linux")]
mod pipewire;
#[cfg(windows)]
mod wasapi;

#[cfg(target_os = "linux")]
use self::pipewire::Backend;
#[cfg(windows)]
use self::wasapi::Backend;

use frame_link_core::Link;

use crate::config::Config;
use crate::model::AudioStatus;

pub struct Audio {
    backend: Backend,
}

impl Audio {
    /// Opens the devices that exist and keeps looking for the ones that do not.
    ///
    /// # Errors
    /// Only when the sound system itself is unusable. A missing device is not
    /// an error: it shows in `status` and is picked up when it appears.
    pub fn start(link: &Link, config: &Config) -> anyhow::Result<Self> {
        Ok(Self { backend: Backend::start(link, config)? })
    }

    #[must_use]
    pub fn status(&self) -> AudioStatus {
        self.backend.status()
    }
}
