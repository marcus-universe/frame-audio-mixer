//! Settings in ~/.config/frame-audio-mixer/config.json.
//! The Microphone fader is stored here and applied to the mixer's own node.
//! It is never written to the system default source.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const MAX_CHANNELS: usize = 8;
pub const NAME_MAX_CHARS: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Channel {
    pub id: String,
    pub name: String,
    #[serde(default = "default_to_mic")]
    pub to_mic: f64,
    #[serde(default = "default_phones")]
    pub phones: f64,
    #[serde(default)]
    pub to_mic_muted: bool,
    #[serde(default)]
    pub phones_muted: bool,
}

impl Channel {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            to_mic: default_to_mic(),
            phones: default_phones(),
            to_mic_muted: false,
            phones_muted: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Pin {
    pub app: String,
    pub channel: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub check_updates: bool,
    #[serde(default)]
    pub mix_on: bool,
    #[serde(default = "default_unity")]
    pub mic_volume: f64,
    #[serde(default)]
    pub mic_muted: bool,
    /// Applied to VRLink only while Mix is on, and only when it is not 1.0.
    #[serde(default = "default_unity")]
    pub vrlink_gain: f64,
    #[serde(default)]
    pub channels: Vec<Channel>,
    #[serde(default)]
    pub pins: Vec<Pin>,
    /// "" follows the system default source. "builtin" is the Frame mic after
    /// Frame Mic Tuner's filters. Anything else is a PipeWire source node name.
    /// This never becomes the system default.
    #[serde(default)]
    pub mic_source: String,
    /// Level meters on the faders. Off skips the PipeWire taps entirely.
    #[serde(default = "default_true")]
    pub meters: bool,
}

fn default_language() -> String {
    "auto".to_string()
}
fn default_unity() -> f64 {
    1.0
}
fn default_to_mic() -> f64 {
    0.5
}
fn default_phones() -> f64 {
    1.0
}
fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            language: default_language(),
            check_updates: false,
            mix_on: false,
            mic_volume: 1.0,
            mic_muted: false,
            vrlink_gain: 1.0,
            channels: Vec::new(),
            pins: Vec::new(),
            mic_source: String::new(),
            meters: true,
        }
    }
}

pub fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let base = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{home}/.config"));
    PathBuf::from(base).join("frame-audio-mixer").join("config.json")
}

pub fn load(path: &Path) -> Config {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(_) => return Config::default(),
    };
    match serde_json::from_str(&text) {
        Ok(config) => sanitize(config),
        Err(err) => {
            eprintln!("frame-audio-mixer: ignoring unreadable config ({err})");
            Config::default()
        }
    }
}

pub fn save(path: &Path, config: &Config) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| e.to_string())
}

fn sanitize(mut config: Config) -> Config {
    config.mic_volume = clamp_volume(config.mic_volume);
    config.vrlink_gain = clamp_volume(config.vrlink_gain);
    config.channels.truncate(MAX_CHANNELS);
    let mut seen = Vec::new();
    config.channels.retain(|channel| {
        if !is_channel_id(&channel.id) {
            return false;
        }
        if seen.contains(&channel.id) {
            return false;
        }
        seen.push(channel.id.clone());
        true
    });
    for channel in &mut config.channels {
        channel.to_mic = clamp_volume(channel.to_mic);
        channel.phones = clamp_volume(channel.phones);
    }
    config.pins.retain(|pin| is_channel_id(&pin.channel) && !pin.app.is_empty());
    config.mic_source = sanitize_mic_source(&config.mic_source);
    config
}

/// Keep only a source name that is safe to hand to WirePlumber.
pub fn sanitize_mic_source(value: &str) -> String {
    if value.is_empty() || value == "builtin" {
        return value.to_string();
    }
    let ok = value.len() <= 200
        && !value.chars().any(|c| c.is_control() || matches!(c, '"' | '\\' | '{' | '}' | ' '));
    if ok { value.to_string() } else { String::new() }
}

pub fn clamp_volume(value: f64) -> f64 {
    value.clamp(0.0, 1.5)
}

pub fn is_channel_id(id: &str) -> bool {
    matches!(id, "ch1" | "ch2" | "ch3" | "ch4" | "ch5" | "ch6" | "ch7" | "ch8")
}

/// Trim, collapse spaces, and reject names that would break a PipeWire property.
pub fn normalize_name(raw: &str) -> Result<String, &'static str> {
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return Err("empty");
    }
    if collapsed.chars().count() > NAME_MAX_CHARS {
        return Err("too-long");
    }
    if collapsed.chars().any(|c| matches!(c, '"' | '\\' | '{' | '}' | '\r' | '\n')) {
        return Err("bad-char");
    }
    Ok(collapsed)
}

pub fn name_taken(channels: &[Channel], name: &str, except: Option<&str>) -> bool {
    let folded = name.to_lowercase();
    channels.iter().any(|channel| {
        Some(channel.id.as_str()) != except && channel.name.to_lowercase() == folded
    })
}

pub fn next_channel_id(channels: &[Channel]) -> Option<String> {
    for n in 1..=MAX_CHANNELS {
        let id = format!("ch{n}");
        if !channels.iter().any(|channel| channel.id == id) {
            return Some(id);
        }
    }
    None
}

pub fn self_test() -> Result<(), String> {
    assert_eq!(normalize_name("  Music   bed ").unwrap(), "Music bed");
    assert!(normalize_name("").is_err());
    assert!(normalize_name("a\"b").is_err());
    assert!(normalize_name(&"x".repeat(40)).is_err());
    let mut channels = vec![Channel::new("ch1", "Music")];
    assert!(name_taken(&channels, "music", None));
    assert!(!name_taken(&channels, "music", Some("ch1")));
    assert_eq!(next_channel_id(&channels).as_deref(), Some("ch2"));
    channels.resize(
        MAX_CHANNELS,
        Channel::new("ch1", "x"),
    );
    for (n, channel) in channels.iter_mut().enumerate() {
        channel.id = format!("ch{}", n + 1);
    }
    assert!(next_channel_id(&channels).is_none());
    let dir = std::env::temp_dir().join(format!("frame-amix-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("config.json");
    let mut config = Config::default();
    config.language = "de".into();
    config.mic_source = "builtin".into();
    config.meters = false;
    let mut game = Channel::new("ch3", "Game");
    game.to_mic = 0.25;
    game.phones = 1.2;
    game.to_mic_muted = true;
    config.channels.push(game);
    if sanitize_mic_source("bad name").is_empty() && sanitize_mic_source("builtin") == "builtin" {
    } else {
        return Err("mic source".into());
    }
    save(&path, &config)?;
    let loaded = load(&path);
    if loaded != sanitize(config) {
        return Err("config round trip".into());
    }
    let _ = fs::remove_dir_all(&dir);
    Ok(())
}
