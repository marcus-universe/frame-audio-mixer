//! Settings, kept as JSON in the user's config folder.

use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::model::{Kind, STREAMS};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// B1–B4: which of the headset's virtual microphones the PC takes.
    pub mics: [bool; STREAMS],
    /// PC1–PC4: which PC outputs are sent to the headset.
    pub outputs: [bool; STREAMS],
    /// The headset's address. Empty finds it on the local networks.
    pub address: String,
    /// UDP port the headset mixer listens on.
    pub port: u16,
    /// Opus bit rate of one stereo stream, in kbit/s.
    pub bitrate_kbps: u32,
    /// Sends every frame twice, so a single lost packet is inaudible.
    pub redundancy: bool,
    /// Names of the PC sound devices behind B1–B4 and PC1–PC4. A device whose
    /// name starts with the entry is used; change these to route a stream
    /// through another virtual cable. A microphone entry names the recording
    /// device applications pick, or the playback device that feeds it when
    /// the two ends of the cable are separate devices.
    pub mic_devices: [String; STREAMS],
    pub output_devices: [String; STREAMS],
    /// The overlay's language as chosen in its language menu: `en`, `ja`,
    /// `sc`, `de`, `fr`, `es` or `it`. Empty follows the system's language.
    pub language: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            language: String::new(),
            mics: [true; STREAMS],
            outputs: [true; STREAMS],
            address: String::new(),
            port: frame_link_core::DEFAULT_PORT,
            bitrate_kbps: 128,
            redundancy: true,
            mic_devices: std::array::from_fn(|index| device_name(Kind::Mic, index)),
            output_devices: std::array::from_fn(|index| device_name(Kind::Output, index)),
        }
    }
}

/// The name a virtual device gets when this app creates it, next to Steam's
/// own "Steam Streaming Microphone": "Steam Streaming Mic B1" and
/// "Steam Streaming Out PC1".
#[must_use]
pub fn device_name(kind: Kind, index: usize) -> String {
    match kind {
        Kind::Mic => format!("Steam Streaming Mic B{}", index + 1),
        Kind::Output => format!("Steam Streaming Out PC{}", index + 1),
    }
}

/// The names version 0.1.0 gave its devices. A settings file that still holds
/// one of them follows the rename.
fn old_device_name(kind: Kind, index: usize) -> String {
    format!("Frame {}{}", kind.prefix(), index + 1)
}

impl Config {
    /// The stored settings, or the defaults when there are none yet. A file
    /// that cannot be read is reported and left alone until the next save.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else { return Self::default() };
        match serde_json::from_str::<Self>(&text) {
            Ok(config) => config.sanitized(),
            Err(err) => {
                crate::log::line(&format!("ignoring unreadable settings in {}: {err}", path.display()));
                Self::default()
            }
        }
    }

    /// Writes the settings through a temporary file, so a crash mid-write
    /// cannot leave half a file behind.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let text = serde_json::to_string_pretty(self).context("encoding the settings")?;
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, text).with_context(|| format!("writing {}", temp.display()))?;
        std::fs::rename(&temp, path).with_context(|| format!("replacing {}", path.display()))
    }

    /// Values a hand-edited file may have pushed out of range are pulled back.
    fn sanitized(mut self) -> Self {
        self.bitrate_kbps = self.bitrate_kbps.clamp(32, 320);
        for (kind, names) in [(Kind::Mic, &mut self.mic_devices), (Kind::Output, &mut self.output_devices)] {
            for (index, name) in names.iter_mut().enumerate() {
                if name.trim().is_empty() || *name == old_device_name(kind, index) {
                    *name = device_name(kind, index);
                }
            }
        }
        if self.port == 0 {
            self.port = frame_link_core::DEFAULT_PORT;
        }
        self
    }

    pub fn switches_mut(&mut self, kind: Kind) -> &mut [bool; STREAMS] {
        match kind {
            Kind::Mic => &mut self.mics,
            Kind::Output => &mut self.outputs,
        }
    }
}

/// Overrides where the settings and the log are kept: for a portable copy,
/// and for tests that must not touch the real settings.
const DIR_OVERRIDE: &str = "FRAME_AUDIO_LINK_DIR";

/// The folder this app keeps its settings and log in.
#[must_use]
pub fn dir() -> PathBuf {
    if let Some(dir) = std::env::var_os(DIR_OVERRIDE).filter(|dir| !dir.is_empty()) {
        return PathBuf::from(dir);
    }
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));
    base.unwrap_or_else(std::env::temp_dir).join("frame-audio-link")
}

#[must_use]
pub fn path() -> PathBuf {
    dir().join("config.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_should_switch_every_stream_on() {
        let config = Config::default();

        assert_eq!((config.mics, config.outputs), ([true; STREAMS], [true; STREAMS]));
    }

    #[test]
    fn default_devices_should_be_named_after_their_stream() {
        let config = Config::default();

        assert_eq!((config.mic_devices[0].as_str(), config.output_devices[3].as_str()), ("Steam Streaming Mic B1", "Steam Streaming Out PC4"));
    }

    #[test]
    fn old_device_names_should_follow_the_rename() -> anyhow::Result<()> {
        let config: Config = serde_json::from_str(r#"{"mic_devices":["Frame B1","My cable","",""],"output_devices":["Frame PC1","Frame PC2","Frame PC3","Frame PC4"]}"#)?;
        let config = config.sanitized();

        assert_eq!(
            (config.mic_devices[0].as_str(), config.mic_devices[1].as_str(), config.mic_devices[2].as_str(), config.output_devices[3].as_str()),
            ("Steam Streaming Mic B1", "My cable", "Steam Streaming Mic B3", "Steam Streaming Out PC4")
        );
        Ok(())
    }

    #[test]
    fn older_file_should_keep_its_values_and_fill_in_the_rest() -> anyhow::Result<()> {
        let config: Config = serde_json::from_str(r#"{"mics":[true,false,false,false]}"#)?;

        assert_eq!((config.mics, config.bitrate_kbps), ([true, false, false, false], 128));
        Ok(())
    }

    #[test]
    fn out_of_range_bitrate_should_be_pulled_back() -> anyhow::Result<()> {
        let config: Config = serde_json::from_str(r#"{"bitrate_kbps":9000}"#)?;

        assert_eq!(config.sanitized().bitrate_kbps, 320);
        Ok(())
    }

    #[test]
    fn saved_settings_should_load_back_unchanged() -> anyhow::Result<()> {
        let path = std::env::temp_dir().join(format!("frame-audio-link-test-{}.json", std::process::id()));
        let mut config = Config::default();
        config.outputs[2] = false;
        config.address = "10.0.0.7".into();
        config.save(&path)?;
        let loaded = Config::load(&path);
        std::fs::remove_file(&path)?;

        assert_eq!(loaded, config);
        Ok(())
    }
}
