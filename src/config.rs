//! Settings in ~/.config/frame-audio-mixer/config.json.
//! The Microphone fader is stored here and applied to the mixer's own node.
//! It is never written to the system default source.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Each channel is one virtual sink inside the engine process.
pub const MAX_CHANNELS: usize = 32;
pub const NAME_MAX_CHARS: usize = 32;
/// Listen outputs, by slot: the Frame speakers, Bluetooth and USB. Bluetooth and
/// USB only show while such a device is connected.
pub const A_SLOTS: usize = 3;
/// Virtual microphones B1..B4. Always present.
pub const B_SLOTS: usize = 4;
/// Linear gain on `frame_amix_mic_virtual_sink` at a fader of 1.0.
/// The filtered Frame mic sits well below a mix bus; 100% is this
/// trim, not digital unity. 4.0 is +12 dB.
pub const MIC_PREAMP: f64 = 4.0;

/// Fader values a drag snaps to (1.0 is 100%).
pub const FADER_SNAPS: [f64; 4] = [0.2, 0.5, 0.8, 1.0];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Channel {
    pub id: String,
    pub name: String,
    /// Strip fader. Negative infinity is stored as a non-finite value and means silence.
    #[serde(default)]
    pub gain_db: f64,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub solo: bool,
    /// Bit 0 is the Frame speakers, 1 Bluetooth, 2 USB; bits 4..7 are B1..B4. The PC is `to_pc`.
    #[serde(default = "default_routes")]
    pub routes: u8,
    /// Heard on the PC streaming microphone (`frame_amix_source`).
    #[serde(default = "default_true")]
    pub to_pc: bool,
    #[serde(default)]
    pub send_fx1: bool,
    #[serde(default)]
    pub send_fx2: bool,
    /// Set when the channel was created from a running app. Only used to find its icon.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub app: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub binary: String,
}

impl Channel {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            gain_db: 0.0,
            muted: false,
            solo: false,
            routes: default_routes(),
            to_pc: true,
            send_fx1: false,
            send_fx2: false,
            app: String::new(),
            binary: String::new(),
        }
    }
}

/// The Frame speakers. With `to_pc`, a fresh channel is heard in the headset and on the PC.
pub fn default_routes() -> u8 {
    route_bit(true, 0)
}

/// The microphone goes to the PC only (`mic_to_pc`), so the speakers do not howl.
pub fn default_mic_routes() -> u8 {
    0
}

pub fn route_bit(a_side: bool, index: usize) -> u8 {
    if index > 3 {
        return 0;
    }
    1 << (index + if a_side { 0 } else { 4 })
}

pub fn route_on(routes: u8, a_side: bool, index: usize) -> bool {
    routes & route_bit(a_side, index) != 0
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
    #[serde(default)]
    pub mic_gain_db: f64,
    #[serde(default)]
    pub mic_muted: bool,
    #[serde(default)]
    pub mic_solo: bool,
    #[serde(default = "default_mic_routes")]
    pub mic_routes: u8,
    #[serde(default = "default_true")]
    pub mic_to_pc: bool,
    #[serde(default)]
    pub mic_send_fx1: bool,
    #[serde(default)]
    pub mic_send_fx2: bool,
    /// No longer applied: the engine keeps VRLink at unity and the PC fader sets the level.
    /// Kept so older config files load unchanged.
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
    /// "blue" or "pink". Blue is the default logo color, #33C4FF.
    #[serde(default = "default_accent")]
    pub accent: String,
    /// Drag snaps to 20, 50, 80 and 100 %, and those three dots are drawn.
    /// Off leaves only the 100 % line and the drag follows the pointer.
    #[serde(default = "default_true")]
    pub snap: bool,
    /// Outputs stand up as columns. Off keeps one horizontal card per output.
    #[serde(default)]
    pub list: bool,
    /// Inputs are a vertical list of sideways sliders. Off keeps upright faders.
    #[serde(default)]
    pub input_list: bool,
    /// Extended dashboard: 21:9, or 9:10 while `list` is on.
    #[serde(default)]
    pub wide: bool,
    /// Share of the two sections given to the right column. 0 keeps the built-in width.
    #[serde(default)]
    pub split: f64,
    /// Signal Flow also shows apps that are not on any channel, playing on the
    /// normal output.
    #[serde(default)]
    pub flow_apps: bool,
    /// Folder scanned for VST3 plugins. `~/` is the home folder.
    #[serde(default = "default_vst_folder")]
    pub vst_folder: String,
    /// What is drawn behind the parametric EQ curve.
    #[serde(default)]
    pub eq_view: EqView,
    #[serde(default)]
    pub a: [Bus; 4],
    #[serde(default)]
    pub b: [Bus; 4],
    /// Master of the PC streaming microphone. Always present.
    #[serde(default)]
    pub pc: Bus,
    /// Output Mix Check records: "pc", "a:N" or "b:N".
    #[serde(default = "default_check_output")]
    pub check_output: String,
    /// Listen output a take plays on: "a:N", or empty for the current default output.
    #[serde(default)]
    pub check_play: String,
    #[serde(default)]
    pub fx_a: FxBus,
    #[serde(default)]
    pub fx_b: FxBus,
    #[serde(default = "default_version")]
    pub version: u32,
}

/// The analyzer drawn behind the parametric EQ.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EqView {
    Off,
    Line,
    #[default]
    Area,
    Bars,
    Heat,
}

impl EqView {
    pub const ALL: [EqView; 5] = [EqView::Off, EqView::Line, EqView::Area, EqView::Bars, EqView::Heat];
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EffectKind {
    Reverb,
    Comp,
    Eq,
    Pitcher,
    Flanger,
    Phaser,
    Delay,
    /// A VST3 plugin, run by a host process. Not in `ALL`: it is picked from a scan.
    Vst,
}

impl EffectKind {
    pub const ALL: [EffectKind; 7] = [
        EffectKind::Reverb,
        EffectKind::Comp,
        EffectKind::Eq,
        EffectKind::Pitcher,
        EffectKind::Flanger,
        EffectKind::Phaser,
        EffectKind::Delay,
    ];

    pub fn key(self) -> &'static str {
        match self {
            EffectKind::Reverb => "reverb",
            EffectKind::Comp => "comp",
            EffectKind::Eq => "eq",
            EffectKind::Pitcher => "pitcher",
            EffectKind::Flanger => "flanger",
            EffectKind::Phaser => "phaser",
            EffectKind::Delay => "delay",
            EffectKind::Vst => "vst",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.key() == value)
    }

    pub fn param_count(self) -> usize {
        match self {
            EffectKind::Pitcher => 1,
            EffectKind::Vst => 0,
            _ => 3,
        }
    }

    pub fn defaults(self) -> [f64; 3] {
        match self {
            EffectKind::Reverb => [0.55, 0.45, 0.25],
            EffectKind::Comp => [0.55, 0.4, 0.2],
            EffectKind::Eq => [0.5, 0.5, 0.5],
            EffectKind::Pitcher => [0.5, 0.0, 0.0], // center is 0 semitones; ±1 is ±12
            EffectKind::Flanger => [0.35, 0.5, 0.35],
            EffectKind::Phaser => [0.3, 0.55, 0.4],
            EffectKind::Delay => [0.35, 0.3, 0.3],
            EffectKind::Vst => [0.5; 3],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EffectSlot {
    pub kind: EffectKind,
    #[serde(default = "default_effect_params")]
    pub params: [f64; 3],
    /// Pitcher drag snaps to whole semitones. Other effects ignore it.
    #[serde(default = "default_pitch_snap")]
    pub pitch_snap: bool,
    /// Set for a parametric EQ. Absent means the three legacy Low/Mid/High knobs.
    #[serde(default)]
    pub eq: Option<ParamEq>,
    /// Character the sliders do not show, set by presets. See `fx::presets`.
    #[serde(default = "default_effect_params")]
    pub extra: [f64; 3],
    /// The plugin, for a VST slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vst: Option<crate::vst::VstRef>,
}

impl EffectSlot {
    pub fn new(kind: EffectKind) -> Self {
        Self { kind, params: kind.defaults(), pitch_snap: true, eq: None, extra: default_effect_params(), vst: None }
    }
}

fn default_pitch_snap() -> bool {
    true
}

/// Pitcher param 0 is 0 at −12 semitones, 0.5 at 0, and 1 at +12.
pub fn pitch_semitones(param: f64) -> f64 {
    ((param.clamp(0.0, 1.0) - 0.5) * 24.0).clamp(-12.0, 12.0)
}

pub fn pitch_param(semitones: f64) -> f64 {
    (semitones.clamp(-12.0, 12.0) / 24.0 + 0.5).clamp(0.0, 1.0)
}

pub fn pitch_snapped(param: f64) -> f64 {
    pitch_param(pitch_semitones(param).round())
}

pub fn format_pitch(param: f64, snap: bool) -> String {
    let raw = pitch_semitones(param);
    let shown = if snap { raw.round() } else { (raw * 10.0).round() / 10.0 };
    if shown.abs() < 0.05 {
        "0".to_string()
    } else if snap || (shown - shown.round()).abs() < 0.001 {
        format!("{shown:+.0}")
    } else {
        format!("{shown:+.1}")
    }
}

fn default_effect_params() -> [f64; 3] {
    [0.5, 0.5, 0.5]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Bus {
    #[serde(default)]
    pub gain_db: f64,
    #[serde(default)]
    pub muted: bool,
    /// Display name for B1..B4.
    #[serde(default)]
    pub name: String,
}

impl Default for Bus {
    fn default() -> Self {
        Self { gain_db: 0.0, muted: false, name: String::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FxBus {
    #[serde(default)]
    pub chain: Vec<EffectSlot>,
    #[serde(default)]
    pub gain_db: f64,
    #[serde(default)]
    pub muted: bool,
    /// Where the effect return plays. A fresh return is heard on A1 and the PC.
    #[serde(default = "default_routes")]
    pub routes: u8,
    #[serde(default = "default_true")]
    pub to_pc: bool,
}

impl Default for FxBus {
    fn default() -> Self {
        Self { chain: Vec::new(), gain_db: 0.0, muted: false, routes: default_routes(), to_pc: true }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BandType {
    Bell,
    Shelf,
    HighPass,
    LowPass,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EqBand {
    #[serde(default = "default_eq_freq")]
    pub freq: f64,
    #[serde(default)]
    pub gain_db: f64,
    #[serde(default = "default_eq_q")]
    pub q: f64,
    #[serde(default = "default_true")]
    pub on: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParamEq {
    #[serde(default = "default_eq_points")]
    pub points: u8,
    #[serde(default)]
    pub bands: Vec<EqBand>,
    #[serde(default = "default_low_type")]
    pub low: BandType,
    #[serde(default = "default_high_type")]
    pub high: BandType,
    #[serde(default)]
    pub bypass: bool,
}

fn default_eq_freq() -> f64 {
    1000.0
}
fn default_eq_q() -> f64 {
    0.71
}
fn default_eq_points() -> u8 {
    6
}
fn default_low_type() -> BandType {
    BandType::Bell
}
fn default_high_type() -> BandType {
    BandType::Bell
}
fn default_version() -> u32 {
    CONFIG_VERSION
}
fn default_check_output() -> String {
    "pc".to_string()
}

pub const CONFIG_VERSION: u32 = 3;

/// Whether a Mix Check target names one of the outputs.
pub fn check_output_valid(value: &str) -> bool {
    if value == "pc" {
        return true;
    }
    let Some((side, index)) = value.split_once(':') else { return false };
    let Ok(index) = index.parse::<usize>() else { return false };
    match side {
        "a" => index < A_SLOTS,
        "b" => index < B_SLOTS,
        _ => false,
    }
}

/// Whether a Mix Check playback choice names a listen output, or is the default output.
pub fn check_play_valid(value: &str) -> bool {
    value.is_empty() || (value.starts_with("a:") && check_output_valid(value))
}

impl ParamEq {
    pub fn new(points: u8) -> Self {
        let points = snap_points(points);
        Self {
            points,
            bands: spaced_bands(points),
            low: BandType::Bell,
            high: BandType::Bell,
            bypass: false,
        }
    }

    /// Change the band count. Each new frequency keeps the nearest old band.
    pub fn set_points(&mut self, points: u8) {
        let points = snap_points(points);
        if points == self.points && self.bands.len() == points as usize {
            return;
        }
        let old = self.bands.clone();
        let mut bands = spaced_bands(points);
        for band in &mut bands {
            if let Some(near) = old.iter().min_by(|a, b| {
                (a.freq.ln() - band.freq.ln()).abs().total_cmp(&(b.freq.ln() - band.freq.ln()).abs())
            }) {
                band.gain_db = near.gain_db;
                band.q = near.q;
                band.on = near.on;
            }
        }
        self.points = points;
        self.bands = bands;
    }
}

pub fn snap_points(points: u8) -> u8 {
    for choice in [3u8, 6, 8, 12] {
        if points <= choice {
            return choice;
        }
    }
    12
}

fn spaced_bands(points: u8) -> Vec<EqBand> {
    let n = points.max(1) as usize;
    let lo = 30.0_f64.ln();
    let hi = 16_000.0_f64.ln();
    (0..n)
        .map(|i| {
            let t = if n == 1 { 0.5 } else { i as f64 / (n - 1) as f64 };
            EqBand { freq: (lo + (hi - lo) * t).exp(), gain_db: 0.0, q: default_eq_q(), on: true }
        })
        .collect()
}

/// Fader travel. 0 is silence, 1 is +12 dB, and 0 dB sits on the scale.
pub const FADER_DB_MIN: f64 = -60.0;
pub const FADER_DB_MAX: f64 = 12.0;
const FADER_T_FLOOR: f64 = 0.04;

pub fn t_to_db(t: f64) -> f64 {
    if t <= FADER_T_FLOOR {
        return f64::NEG_INFINITY;
    }
    let u = ((t - FADER_T_FLOOR) / (1.0 - FADER_T_FLOOR)).clamp(0.0, 1.0);
    FADER_DB_MIN + u * (FADER_DB_MAX - FADER_DB_MIN)
}

pub fn db_to_t(db: f64) -> f64 {
    if !db.is_finite() {
        return 0.0;
    }
    let u = ((db.clamp(FADER_DB_MIN, FADER_DB_MAX) - FADER_DB_MIN) / (FADER_DB_MAX - FADER_DB_MIN)).clamp(0.0, 1.0);
    FADER_T_FLOOR + u * (1.0 - FADER_T_FLOOR)
}

pub fn db_to_lin(db: f64) -> f64 {
    if !db.is_finite() { 0.0 } else { 10f64.powf(db / 20.0) }
}

pub fn format_db(db: f64) -> String {
    if !db.is_finite() || db < FADER_DB_MIN {
        return "-inf".into();
    }
    if db.abs() < 0.05 {
        return "0.0".into();
    }
    format!("{db:+.1}")
}

/// Old faders were `wpctl` cubic volumes. Amplitude is the cube, so dB is `60 log10(v)`.
pub fn cubic_to_db(value: f64) -> f64 {
    if value <= 0.001 { f64::NEG_INFINITY } else { 60.0 * value.max(1e-6).log10() }
}

fn default_language() -> String {
    "auto".to_string()
}
fn default_unity() -> f64 {
    1.0
}
fn default_true() -> bool {
    true
}
fn default_accent() -> String {
    "blue".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            language: default_language(),
            check_updates: false,
            mix_on: true,
            mic_gain_db: 0.0,
            mic_muted: false,
            mic_solo: false,
            mic_routes: default_mic_routes(),
            mic_to_pc: true,
            mic_send_fx1: false,
            mic_send_fx2: false,
            vrlink_gain: 1.0,
            channels: Vec::new(),
            pins: Vec::new(),
            mic_source: String::new(),
            meters: true,
            accent: default_accent(),
            snap: true,
            list: false,
            input_list: false,
            wide: false,
            split: 0.0,
            a: [Bus::default(), Bus::default(), Bus::default(), Bus::default()],
            b: [Bus::default(), Bus::default(), Bus::default(), Bus::default()],
            pc: Bus::default(),
            check_output: default_check_output(),
            check_play: String::new(),
            fx_a: FxBus::default(),
            fx_b: FxBus::default(),
            flow_apps: false,
            eq_view: EqView::default(),
            vst_folder: default_vst_folder(),
            version: CONFIG_VERSION,
        }
    }
}

fn default_vst_folder() -> String {
    "~/.vst3".into()
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
    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("frame-audio-mixer: ignoring unreadable config ({err})");
            return Config::default();
        }
    };
    let version = value.get("version").and_then(|v| v.as_u64());
    let config = if version == Some(2) || version == Some(u64::from(CONFIG_VERSION)) {
        match serde_json::from_value::<Config>(value) {
            Ok(config) if version == Some(2) => migrate_v2(config),
            Ok(config) => config,
            Err(err) => {
                eprintln!("frame-audio-mixer: ignoring unreadable config ({err})");
                return Config::default();
            }
        }
    } else {
        migrate_v1(&value)
    };
    sanitize(config)
}

/// v2 had no PC output: B1 was the PC streaming microphone. Its bit becomes
/// `to_pc`, and B2..B4 move down to B1..B3.
fn migrate_v2(mut config: Config) -> Config {
    fn split(routes: u8) -> (u8, bool) {
        let pc = routes & route_bit(false, 0) != 0;
        ((routes & 0x0f) | ((routes & 0xe0) >> 1), pc)
    }
    let (routes, pc) = split(config.mic_routes);
    config.mic_routes = routes;
    config.mic_to_pc = pc;
    for channel in &mut config.channels {
        let (routes, pc) = split(channel.routes);
        channel.routes = routes;
        channel.to_pc = pc;
    }
    for fx in [&mut config.fx_a, &mut config.fx_b] {
        let (routes, pc) = split(fx.routes);
        fx.routes = routes;
        fx.to_pc = pc;
    }
    let old = config.b.clone();
    config.pc = Bus { gain_db: old[0].gain_db, muted: old[0].muted, name: String::new() };
    config.b = [old[1].clone(), old[2].clone(), old[3].clone(), Bus::default()];
    config.check_output = default_check_output();
    config.version = CONFIG_VERSION;
    config
}

/// v1 stored two cubic faders per channel and two effect buses named A and B.
fn migrate_v1(value: &serde_json::Value) -> Config {
    let mut config = Config::default();
    let get = |key: &str| value.get(key);
    if let Some(lang) = get("language").and_then(|v| v.as_str()) {
        config.language = lang.to_string();
    }
    config.check_updates = get("check_updates").and_then(|v| v.as_bool()).unwrap_or(false);
    config.mix_on = get("mix_on").and_then(|v| v.as_bool()).unwrap_or(false);
    config.mic_gain_db = cubic_to_db(get("mic_volume").and_then(|v| v.as_f64()).unwrap_or(1.0));
    config.mic_muted = get("mic_muted").and_then(|v| v.as_bool()).unwrap_or(false);
    config.mic_solo = get("mic_solo").and_then(|v| v.as_bool()).unwrap_or(false);
    config.mic_send_fx1 = get("mic_fx_a").and_then(|v| v.as_bool()).unwrap_or(false);
    config.mic_send_fx2 = get("mic_fx_b").and_then(|v| v.as_bool()).unwrap_or(false);
    config.mic_routes = default_mic_routes();
    config.vrlink_gain = get("vrlink_gain").and_then(|v| v.as_f64()).unwrap_or(1.0);
    config.mic_source = get("mic_source").and_then(|v| v.as_str()).unwrap_or("").to_string();
    config.meters = get("meters").and_then(|v| v.as_bool()).unwrap_or(true);
    config.accent = get("accent").and_then(|v| v.as_str()).unwrap_or("blue").to_string();
    config.snap = get("snap").and_then(|v| v.as_bool()).unwrap_or(true);
    config.list = get("list").and_then(|v| v.as_bool()).unwrap_or(false);
    config.wide = get("wide").and_then(|v| v.as_bool()).unwrap_or(false);
    if let Some(channels) = get("channels").and_then(|v| v.as_array()) {
        for item in channels {
            let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let mut channel = Channel::new(id, name);
            let to_mic = item.get("to_mic").and_then(|v| v.as_f64()).unwrap_or(0.5);
            let phones = item.get("phones").and_then(|v| v.as_f64()).unwrap_or(1.0);
            let to_mic_muted = item.get("to_mic_muted").and_then(|v| v.as_bool()).unwrap_or(false);
            let phones_muted = item.get("phones_muted").and_then(|v| v.as_bool()).unwrap_or(false);
            channel.gain_db = cubic_to_db(to_mic);
            channel.muted = to_mic_muted && phones_muted;
            channel.solo = item.get("to_mic_solo").and_then(|v| v.as_bool()).unwrap_or(false)
                || item.get("phones_solo").and_then(|v| v.as_bool()).unwrap_or(false);
            channel.routes = if phones > 0.02 && !phones_muted { route_bit(true, 0) } else { 0 };
            channel.to_pc = to_mic > 0.02 && !to_mic_muted;
            channel.send_fx1 = item.get("to_mic_fx_a").and_then(|v| v.as_bool()).unwrap_or(false)
                || item.get("phones_fx_a").and_then(|v| v.as_bool()).unwrap_or(false);
            channel.send_fx2 = item.get("to_mic_fx_b").and_then(|v| v.as_bool()).unwrap_or(false)
                || item.get("phones_fx_b").and_then(|v| v.as_bool()).unwrap_or(false);
            channel.app = item.get("app").and_then(|v| v.as_str()).unwrap_or("").to_string();
            channel.binary = item.get("binary").and_then(|v| v.as_str()).unwrap_or("").to_string();
            config.channels.push(channel);
        }
    }
    if let Some(pins) = get("pins").and_then(|v| v.as_array()) {
        for item in pins {
            let app = item.get("app").and_then(|v| v.as_str()).unwrap_or("");
            let channel = item.get("channel").and_then(|v| v.as_str()).unwrap_or("");
            if !app.is_empty() && !channel.is_empty() {
                config.pins.push(Pin { app: app.to_string(), channel: channel.to_string() });
            }
        }
    }
    config.fx_a = migrate_bus(get("fx_a"));
    config.fx_b = migrate_bus(get("fx_b"));
    config.version = CONFIG_VERSION;
    config
}

fn migrate_bus(value: Option<&serde_json::Value>) -> FxBus {
    let mut bus = FxBus::default();
    let Some(chain) = value.and_then(|v| v.get("chain")).and_then(|v| v.as_array()) else {
        return bus;
    };
    for item in chain {
        let Some(kind) = item.get("kind").and_then(|v| v.as_str()).and_then(EffectKind::parse) else { continue };
        let mut slot = EffectSlot::new(kind);
        if let Some(params) = item.get("params").and_then(|v| v.as_array()) {
            for (index, value) in params.iter().take(3).enumerate() {
                if let Some(number) = value.as_f64() {
                    slot.params[index] = number;
                }
            }
        }
        if kind == EffectKind::Eq {
            slot.eq = Some(eq_from_legacy(slot.params));
        }
        bus.chain.push(slot);
    }
    bus
}

/// The old EQ was a low shelf, a mid bell and a high shelf, each ±12 dB.
fn eq_from_legacy(params: [f64; 3]) -> ParamEq {
    let mut eq = ParamEq::new(3);
    eq.low = BandType::Shelf;
    eq.high = BandType::Shelf;
    let gains = [(params[0] - 0.5) * 24.0, (params[1] - 0.5) * 24.0, (params[2] - 0.5) * 24.0];
    let freqs = [120.0, 1000.0, 8000.0];
    for (band, (freq, gain)) in eq.bands.iter_mut().zip(freqs.into_iter().zip(gains)) {
        band.freq = freq;
        band.gain_db = gain;
        band.q = 0.8;
    }
    eq
}

pub fn save(path: &Path, config: &Config) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| e.to_string())
}

fn sanitize(mut config: Config) -> Config {
    config.version = CONFIG_VERSION;
    // The PC streaming output fader replaced the Mix switch, and inputs have no Solo.
    config.mix_on = true;
    config.mic_solo = false;
    for channel in &mut config.channels {
        channel.solo = false;
    }
    if !check_output_valid(&config.check_output) {
        config.check_output = default_check_output();
    }
    if !check_play_valid(&config.check_play) {
        config.check_play.clear();
    }
    config.mic_gain_db = clamp_db(config.mic_gain_db);
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
        channel.gain_db = clamp_db(channel.gain_db);
    }
    config.pc.gain_db = clamp_db(config.pc.gain_db);
    config.pc.name.clear();
    for bus in config.a.iter_mut().chain(config.b.iter_mut()) {
        bus.gain_db = clamp_db(bus.gain_db);
        if bus.name.chars().count() > NAME_MAX_CHARS {
            bus.name = bus.name.chars().take(NAME_MAX_CHARS).collect();
        }
    }
    sanitize_bus(&mut config.fx_a);
    sanitize_bus(&mut config.fx_b);
    config.pins.retain(|pin| is_channel_id(&pin.channel) && !pin.app.is_empty());
    config.mic_source = sanitize_mic_source(&config.mic_source);
    if config.accent != "pink" {
        config.accent = "blue".into();
    }
    if !config.split.is_finite() || config.split <= 0.0 || config.split >= 1.0 {
        config.split = 0.0;
    }
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

fn sanitize_bus(bus: &mut FxBus) {
    bus.gain_db = clamp_db(bus.gain_db);
    sanitize_chain(&mut bus.chain);
}

fn sanitize_chain(chain: &mut Vec<EffectSlot>) {
    chain.truncate(3);
    for slot in chain.iter_mut() {
        for value in &mut slot.params {
            *value = value.clamp(0.0, 1.0);
        }
        if slot.kind == EffectKind::Pitcher {
            slot.params[1] = 0.0;
            slot.params[2] = 0.0;
            slot.eq = None;
            if slot.pitch_snap {
                slot.params[0] = pitch_snapped(slot.params[0]);
            }
        }
        if slot.kind == EffectKind::Eq {
            if slot.eq.is_none() {
                slot.eq = Some(eq_from_legacy(slot.params));
            }
            if let Some(eq) = slot.eq.as_mut() {
                sanitize_eq(eq);
            }
        } else {
            slot.eq = None;
        }
    }
}

fn sanitize_eq(eq: &mut ParamEq) {
    eq.points = snap_points(eq.points);
    if eq.bands.len() != eq.points as usize {
        eq.set_points(eq.points);
    }
    eq.low = match eq.low {
        BandType::LowPass => BandType::Bell,
        other => other,
    };
    eq.high = match eq.high {
        BandType::HighPass => BandType::Bell,
        other => other,
    };
    for band in &mut eq.bands {
        band.freq = band.freq.clamp(20.0, 20_000.0);
        band.gain_db = band.gain_db.clamp(-18.0, 18.0);
        band.q = band.q.clamp(0.1, 10.0);
    }
}

pub fn clamp_volume(value: f64) -> f64 {
    value.clamp(0.0, 1.5)
}

pub fn clamp_db(value: f64) -> f64 {
    if !value.is_finite() {
        return f64::NEG_INFINITY;
    }
    value.clamp(FADER_DB_MIN, FADER_DB_MAX)
}

/// Linear gain for the microphone fader, including the speaking-level trim.
/// `wpctl` does not take this number: its volume is cubic (`channelVolumes` is the cube).
pub fn mic_pipewire_volume(fader: f64) -> f64 {
    clamp_volume(fader) * MIC_PREAMP
}

/// Value for `wpctl set-volume` that produces `linear` as PipeWire's channel volume.
pub fn wpctl_cubic(linear: f64) -> f64 {
    if linear <= 0.0 { 0.0 } else { linear.cbrt() }
}

/// `ch1` to `ch<MAX_CHANNELS>`, without leading zeros. The id is part of unit and node names.
pub fn is_channel_id(id: &str) -> bool {
    let Some(digits) = id.strip_prefix("ch") else { return false };
    if digits.is_empty() || digits.starts_with('0') || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    digits.parse::<usize>().is_ok_and(|n| (1..=MAX_CHANNELS).contains(&n))
}

/// Pull `value` onto a snap point when it is within `reach` of one.
pub fn snap_volume(value: f64, reach: f64) -> f64 {
    FADER_SNAPS
        .iter()
        .copied()
        .filter(|point| (value - point).abs() <= reach)
        .min_by(|a, b| (value - a).abs().total_cmp(&(value - b).abs()))
        .unwrap_or(value)
}

/// A channel name built from an app name, or None when nothing usable is left.
pub fn name_from_app(app: &str) -> Option<String> {
    let cleaned: String = app.chars().filter(|c| !matches!(c, '"' | '\\' | '{' | '}' | '\r' | '\n')).collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut: String = collapsed.chars().take(NAME_MAX_CHARS).collect();
    normalize_name(&cut).ok()
}

/// Trim, collapse spaces, and reject names that would break a PipeWire property.
/// One saved routing setup. The whole config is stored; the live config's
/// screen preferences win when a project is loaded, so loading a setup does not
/// change language, theme or layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    /// Unix seconds when the project was written.
    #[serde(default)]
    pub saved: u64,
    pub config: Config,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectInfo {
    pub name: String,
    pub path: PathBuf,
}

pub fn projects_dir() -> PathBuf {
    config_path().with_file_name("projects")
}

pub fn list_projects() -> Vec<ProjectInfo> {
    list_projects_in(&projects_dir())
}

pub fn list_projects_in(dir: &Path) -> Vec<ProjectInfo> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<ProjectInfo> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .filter_map(|path| {
            let text = fs::read_to_string(&path).ok()?;
            let project: Project = serde_json::from_str(&text).ok()?;
            Some(ProjectInfo { name: project.name, path })
        })
        .collect();
    out.sort_by_key(|info| info.name.to_lowercase());
    out
}

/// The file a project saves under. Saving the same name overwrites its file.
fn project_slug(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c.to_lowercase().next().unwrap_or(c) } else { '-' })
        .collect();
    let slug = slug.trim_matches('-');
    if slug.is_empty() { "project".into() } else { slug.to_string() }
}

pub fn save_project(name: &str, config: &Config) -> Result<(), String> {
    save_project_in(&projects_dir(), name, config).map(|_| ())
}

pub fn save_project_in(dir: &Path, name: &str, config: &Config) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    let saved = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let project = Project { name: name.to_string(), saved, config: config.clone() };
    let text = serde_json::to_string_pretty(&project).map_err(|err| err.to_string())?;
    let path = dir.join(format!("{}.json", project_slug(name)));
    fs::write(&path, text).map_err(|err| err.to_string())?;
    Ok(path)
}

/// The project's routing, with the live config's screen preferences kept.
pub fn load_project(path: &Path, current: &Config) -> Option<Config> {
    let text = fs::read_to_string(path).ok()?;
    let project: Project = serde_json::from_str(&text).ok()?;
    Some(adopt_project(project.config, current))
}

fn adopt_project(mut loaded: Config, current: &Config) -> Config {
    loaded.language = current.language.clone();
    loaded.accent = current.accent.clone();
    loaded.meters = current.meters;
    loaded.snap = current.snap;
    loaded.list = current.list;
    loaded.input_list = current.input_list;
    loaded.wide = current.wide;
    loaded.split = current.split;
    loaded.flow_apps = current.flow_apps;
    loaded.eq_view = current.eq_view;
    loaded.vst_folder = current.vst_folder.clone();
    loaded.check_updates = current.check_updates;
    loaded.version = current.version;
    loaded
}

pub fn delete_project(path: &Path) {
    let _ = fs::remove_file(path);
}

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
    let fresh = Config::default();
    if !fresh.snap || fresh.list || fresh.wide {
        return Err("slider defaults".into());
    }
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
    const { assert!(MAX_CHANNELS > 8) };
    for good in ["ch1", "ch9", "ch10", &format!("ch{MAX_CHANNELS}")] {
        assert!(is_channel_id(good), "{good}");
    }
    for bad in ["ch", "ch0", "ch01", "ch-1", "chx", "ch1 ", &format!("ch{}", MAX_CHANNELS + 1)] {
        assert!(!is_channel_id(bad), "{bad}");
    }
    if project_slug("VR Night!") != "vr-night" || project_slug("  ") != "project" {
        return Err(format!("project slug {}", project_slug("VR Night!")));
    }
    let dir = std::env::temp_dir().join(format!("famx-projects-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let mut setup = Config::default();
    setup.mix_on = true;
    setup.channels.push(Channel::new("ch1", "Music"));
    save_project_in(&dir, "VR Night", &setup)?;
    save_project_in(&dir, "Stream", &setup)?;
    let listed = list_projects_in(&dir);
    if listed.len() != 2 || listed[0].name != "Stream" || listed[1].name != "VR Night" {
        return Err(format!("project list {listed:?}"));
    }
    let mut current = Config::default();
    current.language = "de".into();
    current.wide = true;
    let loaded = load_project(&listed[1].path, &current).ok_or("project load")?;
    if !loaded.mix_on || loaded.channels.len() != 1 || loaded.language != "de" || !loaded.wide {
        return Err("project merge".into());
    }
    delete_project(&listed[0].path);
    if list_projects_in(&dir).len() != 1 {
        return Err("project delete".into());
    }
    let _ = fs::remove_dir_all(&dir);
    assert_eq!(snap_volume(0.49, 0.02), 0.5);
    assert_eq!(snap_volume(1.015, 0.02), 1.0);
    assert_eq!(snap_volume(0.65, 0.02), 0.65);
    let cubic = wpctl_cubic(mic_pipewire_volume(1.0));
    if (cubic.powi(3) - MIC_PREAMP).abs() > 0.001 || cubic > 1.6 {
        return Err(format!("mic cubic {cubic}"));
    }
    assert_eq!(snap_volume(0.19, 0.02), 0.2);
    assert!((pitch_semitones(0.0) + 12.0).abs() < 1e-9);
    assert!(pitch_semitones(0.5).abs() < 1e-9);
    assert!((pitch_semitones(1.0) - 12.0).abs() < 1e-9);
    assert!((pitch_snapped(0.51) - 0.5).abs() < 1e-9);
    assert!((pitch_snapped(0.53) - pitch_param(1.0)).abs() < 1e-9);
    assert_eq!(format_pitch(0.5, true), "0");
    assert_eq!(format_pitch(1.0, false), "+12");
    assert_eq!(format_pitch(0.0, true), "-12");
    let mut pitched = EffectSlot::new(EffectKind::Pitcher);
    assert!(pitched.pitch_snap && (pitched.params[0] - 0.5).abs() < 1e-9);
    pitched.params[0] = 0.53;
    let mut snapped = Config::default();
    snapped.fx_a.chain.push(pitched.clone());
    let stepped = sanitize(snapped);
    assert!(stepped.fx_a.chain[0].pitch_snap);
    assert!((stepped.fx_a.chain[0].params[0] - pitch_param(1.0)).abs() < 1e-9);
    pitched.pitch_snap = false;
    let mut held = Config::default();
    held.fx_a.chain.push(pitched);
    let kept = sanitize(held);
    assert!(!kept.fx_a.chain[0].pitch_snap);
    assert!((kept.fx_a.chain[0].params[0] - 0.53).abs() < 1e-9);
    assert_eq!(name_from_app("  Zen \"Browser\"  ").as_deref(), Some("Zen Browser"));
    assert_eq!(name_from_app(&"y".repeat(40)).map(|n| n.chars().count()), Some(NAME_MAX_CHARS));
    assert!(name_from_app("\"\"").is_none());
    let dir = std::env::temp_dir().join(format!("frame-amix-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("config.json");
    let mut config = Config::default();
    config.language = "de".into();
    config.mic_source = "builtin".into();
    config.meters = false;
    let mut game = Channel::new("ch3", "Game");
    game.gain_db = -12.0;
    game.muted = true;
    game.app = "Zen".into();
    game.binary = "zen".into();
    config.channels.push(game);
    config.channels.push(Channel::new("ch12", "Video"));
    if sanitize_mic_source("bad name").is_empty() && sanitize_mic_source("builtin") == "builtin" {
    } else {
        return Err("mic source".into());
    }
    save(&path, &config)?;
    let loaded = load(&path);
    if loaded != sanitize(config) {
        return Err("config round trip".into());
    }
    let mut fx = Config::default();
    fx.fx_a.chain.push(EffectSlot::new(EffectKind::Delay));
    fx.fx_a.chain.push(EffectSlot::new(EffectKind::Reverb));
    fx.fx_a.chain.push(EffectSlot::new(EffectKind::Eq));
    fx.fx_a.chain.push(EffectSlot::new(EffectKind::Comp));
    fx.fx_a.chain[0].params[0] = 2.0;
    let fx = sanitize(fx);
    if fx.fx_a.chain.len() != 3 || fx.fx_a.chain[0].params[0] > 1.0 {
        return Err("fx chain".into());
    }
    if fx.fx_a.chain.iter().find(|slot| slot.kind == EffectKind::Eq).and_then(|slot| slot.eq.as_ref()).map(|eq| eq.points) != Some(3) {
        return Err("legacy eq".into());
    }
    let mut resized = ParamEq::new(3);
    resized.bands[1].gain_db = 4.0;
    resized.set_points(6);
    if resized.points != 6 || resized.bands.len() != 6 {
        return Err("eq points".into());
    }
    if resized.bands.iter().all(|band| band.gain_db.abs() < 0.01) {
        return Err("eq resize dropped the boost".into());
    }
    let legacy = r#"{"channels":[{"id":"ch1","name":"DJ","to_mic":1.0,"phones":0.0,"phones_muted":true}],"mic_volume":1.0}"#;
    let path_v1 = dir.join("v1.json");
    fs::write(&path_v1, legacy).map_err(|e| e.to_string())?;
    let migrated = load(&path_v1);
    let dj = migrated.channels.iter().find(|channel| channel.id == "ch1").ok_or("migrate")?;
    if dj.gain_db.abs() > 0.05 || dj.routes != 0 || !dj.to_pc {
        return Err(format!("migrate {} {:08b} {}", dj.gain_db, dj.routes, dj.to_pc));
    }
    if migrated.version != CONFIG_VERSION || !migrated.mic_to_pc || migrated.mic_routes != 0 {
        return Err("version".into());
    }
    let v2 = r#"{"version":2,"outputs":4,"mic_routes":16,"channels":[
        {"id":"ch1","name":"DJ","routes":49,"inserts":[{"kind":"eq"}]},
        {"id":"ch2","name":"Game","routes":1}],
        "b":[{"gain_db":-6.0,"muted":true},{"name":"Discord"},{},{}],
        "fx_a":{"routes":17}}"#;
    let path_v2 = dir.join("v2.json");
    fs::write(&path_v2, v2).map_err(|e| e.to_string())?;
    let moved = load(&path_v2);
    let dj = moved.channels.iter().find(|channel| channel.id == "ch1").ok_or("migrate v2")?;
    let game = moved.channels.iter().find(|channel| channel.id == "ch2").ok_or("migrate v2")?;
    if !dj.to_pc || dj.routes != (route_bit(true, 0) | route_bit(false, 0)) || game.to_pc || game.routes != route_bit(true, 0) {
        return Err(format!("migrate v2 routes {:08b} {:08b}", dj.routes, game.routes));
    }
    if !moved.mic_to_pc || moved.mic_routes != 0 || !moved.fx_a.to_pc || moved.fx_a.routes != route_bit(true, 0) {
        return Err("migrate v2 mic/fx".into());
    }
    if !moved.pc.muted || (moved.pc.gain_db + 6.0).abs() > 0.01 || moved.b[0].name != "Discord" || moved.version != CONFIG_VERSION {
        return Err("migrate v2 buses".into());
    }
    if !check_output_valid("pc") || !check_output_valid("b:3") || !check_output_valid("a:2") || check_output_valid("a:3") || check_output_valid("x") {
        return Err("check output".into());
    }
    if !check_play_valid("") || !check_play_valid("a:1") || check_play_valid("pc") || check_play_valid("b:0") || check_play_valid("a:3") {
        return Err("check play".into());
    }
    let _ = fs::remove_dir_all(&dir);
    Ok(())
}
