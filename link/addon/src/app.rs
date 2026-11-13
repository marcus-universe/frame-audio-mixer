//! The running add-on: the link to the headset, the PC sound devices, and the
//! user's switches between them.

use std::path::PathBuf;

use anyhow::Context;
use frame_link_core::{Link, LinkConfig, Mask, Role};

use crate::audio::Audio;
use crate::config::Config;
use crate::model::{Action, DeviceState, Snapshot, STREAMS};
use crate::net;

/// Opus effort on the PC. A desktop CPU has room for the best setting.
const COMPLEXITY: i32 = 10;
/// Ticks before the open devices are first written to the log. They open a
/// moment after the start; an earlier line would always say none.
const SETTLE_TICKS: u32 = 8;

pub struct App {
    config: Config,
    path: PathBuf,
    link: Link,
    audio: Audio,
    /// The masks last given to the link, so they are only sent on a change.
    masks: (Mask, Mask),
    ticks: u32,
    /// Open microphones and outputs as last written to the log.
    logged: Option<(usize, usize)>,
}

impl App {
    /// Starts the link and opens the sound devices.
    ///
    /// # Errors
    /// When the network socket, the codec or the sound system cannot be set up.
    pub fn start(config: Config, path: PathBuf) -> anyhow::Result<Self> {
        let (address, port) = (config.address.clone(), config.port);
        let link = Link::spawn(LinkConfig {
            role: Role::Host { candidates: Box::new(move || net::candidates(&address, port)) },
            name: net::host_name(),
            port,
            bitrate: i32::try_from(config.bitrate_kbps.saturating_mul(1000)).unwrap_or(128_000),
            complexity: COMPLEXITY,
            redundancy: config.redundancy,
        })
        .context("starting the link to the headset")?;
        let audio = Audio::start(&link, &config).context("opening the sound devices")?;
        let mut app = Self { config, path, link, audio, masks: (0, 0), ticks: 0, logged: None };
        app.tick();
        Ok(app)
    }

    /// Keeps the link's masks in step with the switches and the devices.
    /// Call a few times a second.
    pub fn tick(&mut self) {
        let audio = self.audio.status();
        let masks = (mask(&self.config.outputs, &audio.outputs), mask(&self.config.mics, &audio.mics));
        if masks != self.masks {
            self.link.set_masks(masks.0, masks.1);
            self.masks = masks;
        }
        self.ticks = self.ticks.saturating_add(1);
        let open = (ready(&audio.mics), ready(&audio.outputs));
        if self.ticks >= SETTLE_TICKS && self.logged != Some(open) {
            self.logged = Some(open);
            let hint = if open == (0, 0) { ". Run `frame-audio-link install` if they were never created" } else { "" };
            crate::log::line(&format!("sound devices: {} of {STREAMS} microphones and {} of {STREAMS} outputs are open{hint}", open.0, open.1));
        }
    }

    /// The overlay language saved in the settings. Empty when none was chosen.
    #[must_use]
    pub fn language(&self) -> &str {
        &self.config.language
    }

    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        Snapshot { link: self.link.status(), audio: self.audio.status(), mics_on: self.config.mics, outputs_on: self.config.outputs }
    }

    /// Carries out a press in the overlay and remembers the result.
    pub fn apply(&mut self, action: Action) {
        match action {
            Action::Toggle(kind, index) => {
                let Some(switch) = self.config.switches_mut(kind).get_mut(index) else { return };
                *switch = !*switch;
            }
            Action::Language(code) => self.config.language = code.to_string(),
        }
        self.tick();
        if let Err(err) = self.config.save(&self.path) {
            crate::log::line(&format!("the settings were not saved: {err:#}"));
        }
    }

}

fn ready(devices: &[DeviceState; STREAMS]) -> usize {
    devices.iter().filter(|state| **state == DeviceState::Ready).count()
}

/// A stream is offered while it is switched on and its device is open. A
/// stream without a device would only spend bandwidth.
fn mask(switches: &[bool; STREAMS], devices: &[DeviceState; STREAMS]) -> Mask {
    switches
        .iter()
        .zip(devices)
        .enumerate()
        .filter(|(_, (on, device))| **on && **device == DeviceState::Ready)
        .fold(0, |mask, (index, _)| mask | (1 << index))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_should_need_the_switch_and_the_device() {
        let switches = [true, true, false, false];
        let devices = [DeviceState::Ready, DeviceState::Missing, DeviceState::Ready, DeviceState::Failed];

        assert_eq!(mask(&switches, &devices), 0b0001);
    }
}
