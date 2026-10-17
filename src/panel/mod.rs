//! Dashboard panel: the mix strip, Mix Check, dialogs, and Apps & updates.
//!
//! Geometry follows Frame Mic Tuner's Mix tab. The microphone chosen here is
//! only the mixer's input. It is never made the system default.

mod apps;
mod check;
mod dialogs;
mod footer;
mod mix;

use crate::apps::Entry;
use crate::config::{self, Config, Pin};
use crate::draw::{self, ButtonKind, Fonts, Look, Rgb, BG, TEXT};
use crate::graph::Graph;
use crate::i18n::{self, Lang, Text};
use crate::worker::{CheckPhase, Snapshot};
use cairo::Context;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Mix,
    Apps,
}

#[derive(Clone)]
enum NameEdit {
    Add,
    Rename(String),
}

enum Overlay {
    None,
    Add,
    Mic,
    Settings,
    AppsList,
    Confirm(String),
    Name(NameEdit),
}

struct Hot {
    id: String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

struct FaderHit {
    id: String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    horizontal: bool,
}

/// Scrollbar for the channel strip. `thumb_w` is the thumb's length along the scroll axis.
#[derive(Clone, Copy)]
pub(crate) struct HBar {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub thumb_w: f64,
    pub max_scroll: f64,
    pub vertical: bool,
}

/// Where the mixer, Mix Check, and the footer sit for the current window.
pub(crate) struct Stage {
    pub w: f64,
    pub portrait: bool,
    pub footer: (f64, f64, f64, f64),
    pub picker: (f64, f64, f64, f64),
    pub mix: (f64, f64, f64, f64),
    pub check: (f64, f64, f64, f64),
}

const HSCROLL: &str = "hscroll";
const PAN: &str = "pan";
/// How close, in pixels, a fader drag has to come to a snap point.
const SNAP_REACH_PX: f64 = 8.0;

pub struct Panel {
    pub config: Config,
    lang: Lang,
    page: Page,
    overlay: Overlay,
    language_open: bool,
    color_open: bool,
    quit_until: Option<Instant>,
    delete_until: Option<(String, Instant)>,
    scroll: f64,
    dialog_scroll: f64,
    drag: Option<String>,
    pointer: Option<(f64, f64)>,
    pressed: bool,
    notice: String,
    /// Draft shown in the channel-name dialog.
    name_draft: String,
    name_rejected: bool,
    /// The dashboard keyboard is open for the name field.
    name_keyboard: bool,
    hots: Vec<Hot>,
    faders: Vec<FaderHit>,
    pub(crate) hbar: Option<HBar>,
    /// Channel strip from the last frame. Empty space here drag-scrolls.
    strip: Option<(f64, f64, f64, f64)>,
    pan_at: Option<(f64, f64)>,
    /// Channels drawn in the last frame. Only these get a meter tap.
    pub(crate) visible_channels: Vec<String>,
    icons: Option<crate::icons::IconIndex>,
    icon_rx: Option<std::sync::mpsc::Receiver<crate::icons::IconIndex>>,
    pngs: HashMap<String, cairo::ImageSurface>,
    /// App waiting for a channel choice: a new channel named after it, or an existing one.
    pub(crate) pin_app: Option<String>,
    /// Name and binary of each app in the last Running list, for the channel icon.
    pub(crate) app_binaries: Vec<(String, String)>,
    /// Record was clicked and the worker has not published Recording yet.
    pub(crate) record_pending: bool,
    record_message: String,
    record_at: Option<Instant>,
    seen_message: String,
}

pub enum Action {
    None,
    Save,
    Quit,
    ShowKeyboard { description: String, existing: String },
    Record,
    StopCheck,
    PlayClip(u64),
    CheckUpdates,
    InstallUpdate,
    OpenInstaller(String),
    Autostart(bool),
}

pub const SUGGESTIONS: [&str; 5] = ["Soundboard", "Music", "Video", "Browser", "Game"];

impl Panel {
    pub fn new(config: Config) -> Self {
        let lang = i18n::resolve(&config.language);
        Self {
            config,
            lang,
            page: Page::Mix,
            overlay: Overlay::None,
            language_open: false,
            color_open: false,
            quit_until: None,
            delete_until: None,
            scroll: 0.0,
            dialog_scroll: 0.0,
            drag: None,
            pointer: None,
            pressed: false,
            notice: String::new(),
            name_draft: String::new(),
            name_rejected: false,
            name_keyboard: false,
            hots: Vec::new(),
            faders: Vec::new(),
            hbar: None,
            strip: None,
            pan_at: None,
            visible_channels: Vec::new(),
            icons: None,
            icon_rx: None,
            pngs: HashMap::new(),
            pin_app: None,
            app_binaries: Vec::new(),
            record_pending: false,
            record_message: String::new(),
            record_at: None,
            seen_message: String::new(),
        }
    }

    pub fn demo(lang: Lang) -> Self {
        let mut config = Config::default();
        config.language = lang.code().into();
        config.mix_on = true;
        config.channels.push(config::Channel::new("ch1", "Music"));
        let mut game = config::Channel::new("ch2", "Game");
        game.to_mic = 0.8;
        game.phones = 0.6;
        config.channels.push(game);
        let mut zen = config::Channel::new("ch3", "Zen");
        zen.app = "Zen".into();
        zen.binary = "zen".into();
        zen.to_mic = 0.2;
        config.channels.push(zen);
        for (n, name) in ["Video", "Browser", "Discord", "Soundboard", "Voice", "Alerts", "Radio"].iter().enumerate() {
            config.channels.push(config::Channel::new(format!("ch{}", n + 4), *name));
        }
        config.pins.push(Pin { app: "Zen".into(), channel: "ch3".into() });
        Self::new(config)
    }

    /// Drop an armed Quit or delete, and take the icon index when the scan finishes.
    pub fn tick(&mut self) {
        let now = Instant::now();
        if self.quit_until.is_some_and(|until| now > until) {
            self.quit_until = None;
        }
        if self.delete_until.as_ref().is_some_and(|(_, until)| now > *until) {
            self.delete_until = None;
        }
        let app_channels = self.config.channels.iter().any(|channel| !channel.app.is_empty());
        if app_channels && self.icons.is_none() && self.icon_rx.is_none() {
            self.icon_rx = Some(crate::icons::spawn());
        }
        if let Some(rx) = &self.icon_rx {
            if let Ok(index) = rx.try_recv() {
                self.icons = Some(index);
                self.icon_rx = None;
            }
        }
    }

    pub fn wants_meters(&self) -> bool {
        self.config.meters && matches!(self.page, Page::Mix) && matches!(self.overlay, Overlay::None)
    }

    /// PipeWire nodes to meter. The mic tap is not passive. Channel taps are.
    pub fn meter_targets(&self, snap: &Snapshot) -> Vec<crate::meter::Target> {
        if !self.wants_meters() {
            return Vec::new();
        }
        let mut targets = Vec::new();
        if let Some(name) = resolved_source(&self.config.mic_source, &snap.graph) {
            targets.push(crate::meter::Target {
                key: "mic".into(),
                object: snap.graph.link_target(&name),
                capture_sink: false,
                passive: false,
                virtual_node: false,
            });
        }
        for channel in self.config.channels.iter().filter(|channel| self.visible_channels.contains(&channel.id)) {
            let name = format!("frame_amix_{}", channel.id);
            targets.push(crate::meter::Target {
                key: channel.id.clone(),
                object: snap.graph.link_target(&name),
                capture_sink: true,
                passive: true,
                virtual_node: true,
            });
        }
        targets
    }

    /// True while the name dialog is waiting on the dashboard keyboard.
    pub fn keyboard_live(&self) -> bool {
        self.name_keyboard && matches!(self.overlay, Overlay::Name(_))
    }

    /// Copy headset-keyboard text into the name field.
    /// `full` is GetKeyboardText. `key` is the new key from a char event, which is not the whole line.
    pub fn apply_keyboard_text(&mut self, full: &str, key: &str, finished: bool) -> bool {
        if !matches!(self.overlay, Overlay::Name(_)) {
            self.name_keyboard = false;
            return false;
        }
        let next = if finished {
            self.name_keyboard = false;
            let full_n = full.chars().count();
            let draft_n = self.name_draft.chars().count();
            if !full.is_empty() && full_n >= draft_n {
                truncate_name(full)
            } else {
                self.name_draft.clone()
            }
        } else {
            merge_keyboard(&self.name_draft, full, key)
        };
        if next == self.name_draft {
            return false;
        }
        self.name_draft = next;
        self.name_rejected = false;
        true
    }

    fn begin_name(&mut self, mode: NameEdit, draft: String) -> Action {
        self.name_draft = draft;
        self.name_rejected = false;
        self.name_keyboard = false;
        self.overlay = Overlay::Name(mode);
        Action::None
    }

    fn commit_name(&mut self) -> Action {
        let Some(mode) = self.name_mode() else { return Action::None };
        let name = match config::normalize_name(&self.name_draft) {
            Ok(name) => name,
            Err(_) => {
                self.name_rejected = true;
                return Action::None;
            }
        };
        let except = match &mode {
            NameEdit::Rename(id) => Some(id.as_str()),
            NameEdit::Add => None,
        };
        if config::name_taken(&self.config.channels, &name, except) {
            self.name_rejected = true;
            return Action::None;
        }
        self.name_keyboard = false;
        match mode {
            NameEdit::Add => self.add_channel(name, None),
            NameEdit::Rename(id) => {
                if let Some(channel) = self.config.channels.iter_mut().find(|c| c.id == id) {
                    channel.name = name;
                }
                self.overlay = Overlay::None;
                self.name_draft.clear();
                Action::Save
            }
        }
    }

    fn name_mode(&self) -> Option<NameEdit> {
        match &self.overlay {
            Overlay::Name(mode) => Some(mode.clone()),
            _ => None,
        }
    }

    fn add_channel(&mut self, name: String, pin_app: Option<String>) -> Action {
        if let Some(existing) = self.config.channels.iter().find(|c| c.name.eq_ignore_ascii_case(&name)) {
            if let Some(app) = pin_app {
                self.pin(app, existing.id.clone());
            }
            self.pin_app = None;
            self.overlay = Overlay::None;
            return Action::Save;
        }
        let Some(id) = config::next_channel_id(&self.config.channels) else {
            self.notice = i18n::text(self.lang).name_rejected.to_string();
            return Action::None;
        };
        self.config.channels.push(config::Channel::new(id.clone(), name));
        if let Some(app) = pin_app {
            self.pin(app, id);
        }
        self.pin_app = None;
        self.overlay = Overlay::None;
        // The new channel is last in the row. The next frame clamps this to the end.
        self.scroll = f64::MAX;
        Action::Save
    }

    /// A channel named after a running app, with that app pinned to it and its icon in the strip.
    fn add_app_channel(&mut self, app: &str) -> Action {
        let Some(name) = config::name_from_app(app) else {
            self.notice = i18n::text(self.lang).name_rejected.to_string();
            return Action::None;
        };
        let binary = self.binary_of(app);
        let action = self.add_channel(name.clone(), Some(app.to_string()));
        if let Some(channel) = self.config.channels.iter_mut().find(|c| c.name.eq_ignore_ascii_case(&name)) {
            if channel.app.is_empty() {
                channel.app = app.to_string();
                channel.binary = binary;
            }
        }
        action
    }

    fn binary_of(&self, app: &str) -> String {
        self.app_binaries.iter().find(|(name, _)| name == app).map(|(_, binary)| binary.clone()).unwrap_or_default()
    }

    fn pin(&mut self, app: String, channel: String) {
        self.config.pins.retain(|pin| pin.app != app);
        self.config.pins.push(Pin { app, channel });
    }

    pub fn pointer(&mut self, kind: u32, x: f64, y: f64, button: u32) -> Action {
        if kind == crate::overlay::MOUSE_MOVE {
            self.pointer = Some((x, y));
            if let Some(id) = self.drag.clone() {
                if id == HSCROLL {
                    self.set_hscroll(x, y);
                } else if id == PAN {
                    if let Some((px, py)) = self.pan_at {
                        let delta = if self.config.list { y - py } else { x - px };
                        self.scroll = (self.scroll - delta).max(0.0);
                    }
                    self.pan_at = Some((x, y));
                } else {
                    self.set_fader(&id, x, y);
                }
            }
            return Action::None;
        }
        if kind == crate::overlay::MOUSE_UP {
            self.pressed = false;
            self.pan_at = None;
            let save = self.drag.take().is_some_and(|id| id != HSCROLL && id != PAN);
            return if save { Action::Save } else { Action::None };
        }
        if kind == crate::overlay::SCROLL_DISCRETE || kind == crate::overlay::SCROLL_SMOOTH {
            // List rows and dialogs move on the vertical stick. The channel row moves on the horizontal one.
            let vertical = self.config.list || matches!(self.overlay, Overlay::Add | Overlay::Mic | Overlay::AppsList);
            let use_y = if vertical { y.abs() >= x.abs() } else { y.abs() > x.abs() };
            let delta = if use_y { y } else { x };
            // A controller notch is about ±1, which used to move the row by one pixel.
            let step = if delta.abs() <= 3.0 { delta * 150.0 } else { delta * 4.0 };
            if matches!(self.overlay, Overlay::Add | Overlay::Mic | Overlay::AppsList) {
                self.dialog_scroll = (self.dialog_scroll - step * 0.45).max(0.0);
            } else {
                self.scroll = (self.scroll - step).max(0.0);
            }
            return Action::None;
        }
        if kind != crate::overlay::MOUSE_DOWN || !crate::overlay::is_left(button) {
            return Action::None;
        }
        self.pointer = Some((x, y));
        self.pressed = true;
        if self.language_open {
            let action = self.hit_at(x, y);
            if matches!(action, Action::None) {
                self.language_open = false;
            }
            return action;
        }
        if !matches!(self.overlay, Overlay::None) {
            return self.hit_at(x, y);
        }
        if self.hbar.is_some_and(|bar| x >= bar.x - 10.0 && x < bar.x + bar.w + 10.0 && y >= bar.y - 10.0 && y < bar.y + bar.h + 10.0) {
            self.drag = Some(HSCROLL.to_string());
            self.set_hscroll(x, y);
            return Action::None;
        }
        if let Some(id) = self.fader_at(x, y) {
            self.drag = Some(id.clone());
            self.set_fader(&id, x, y);
            return Action::None;
        }
        if self.hot_at(x, y).is_some() {
            return self.hit_at(x, y);
        }
        if self.strip.is_some_and(|(sx, sy, sw, sh)| x >= sx && x < sx + sw && y >= sy && y < sy + sh) {
            self.drag = Some(PAN.to_string());
            self.pan_at = Some((x, y));
            return Action::None;
        }
        Action::None
    }

    fn hit_at(&mut self, x: f64, y: f64) -> Action {
        let id = self
            .hots
            .iter()
            .rev()
            .find(|hot| x >= hot.x && x < hot.x + hot.w && y >= hot.y && y < hot.y + hot.h)
            .map(|hot| hot.id.clone());
        let Some(id) = id else { return Action::None };
        self.handle(&id)
    }

    fn handle(&mut self, id: &str) -> Action {
        let t = i18n::text(self.lang);
        if id == "dismiss" {
            self.overlay = Overlay::None;
            self.pin_app = None;
            self.color_open = false;
            self.dialog_scroll = 0.0;
            self.name_draft.clear();
            self.name_rejected = false;
            self.name_keyboard = false;
            return Action::None;
        }
        if id == "quit" {
            let armed = self.quit_until.is_some_and(|until| Instant::now() <= until);
            if armed {
                return Action::Quit;
            }
            self.quit_until = Some(Instant::now() + Duration::from_secs(3));
            self.language_open = false;
            return Action::None;
        }
        if id == "lang" {
            self.language_open = !self.language_open;
            self.overlay = Overlay::None;
            return Action::None;
        }
        if let Some(code) = id.strip_prefix("lang:") {
            if let Some(lang) = i18n::parse_code(code) {
                self.lang = lang;
                self.config.language = lang.code().into();
                self.language_open = false;
                return Action::Save;
            }
        }
        if id == "settings" {
            self.overlay = Overlay::Settings;
            self.language_open = false;
            self.color_open = false;
            return Action::None;
        }
        if id == "meters-on" || id == "meters-off" {
            self.config.meters = id == "meters-on";
            return Action::Save;
        }
        if id == "snap-on" || id == "snap-off" {
            self.config.snap = id == "snap-on";
            return Action::Save;
        }
        if id == "list-on" || id == "list-off" {
            self.config.list = id == "list-on";
            self.scroll = 0.0;
            return Action::Save;
        }
        if id == "wide-on" || id == "wide-off" {
            self.config.wide = id == "wide-on";
            return Action::Save;
        }
        if id == "color-open" {
            self.color_open = !self.color_open;
            return Action::None;
        }
        if let Some(color) = id.strip_prefix("color:") {
            if color == "blue" || color == "pink" {
                self.config.accent = color.to_string();
                self.color_open = false;
                return Action::Save;
            }
        }
        if id == "auto-on" || id == "auto-off" {
            return Action::Autostart(id == "auto-on");
        }
        if id == "apps" {
            self.page = Page::Apps;
            self.language_open = false;
            self.overlay = Overlay::None;
            return Action::None;
        }
        if id == "back" {
            self.page = Page::Mix;
            self.overlay = Overlay::None;
            return Action::None;
        }
        if id == "mix-on" || id == "mix-off" {
            self.config.mix_on = id == "mix-on";
            return Action::Save;
        }
        if id == "mic-open" {
            self.overlay = Overlay::Mic;
            self.dialog_scroll = 0.0;
            return Action::None;
        }
        if let Some(rest) = id.strip_prefix("mic:") {
            self.config.mic_source = if rest == "system" { String::new() } else { rest.to_string() };
            self.overlay = Overlay::None;
            return Action::Save;
        }
        if id == "add" {
            if config::next_channel_id(&self.config.channels).is_none() {
                return Action::None;
            }
            self.overlay = Overlay::Add;
            self.pin_app = None;
            self.dialog_scroll = 0.0;
            if self.icon_rx.is_none() && self.icons.is_none() {
                self.icon_rx = Some(crate::icons::spawn());
            }
            return Action::None;
        }
        if id == "refresh" {
            self.icons = None;
            self.icon_rx = Some(crate::icons::spawn());
            return Action::None;
        }
        if id == "custom" {
            return self.begin_name(NameEdit::Add, String::new());
        }
        if let Some(app) = id.strip_prefix("use:") {
            if self.config.channels.is_empty() {
                return self.add_app_channel(app);
            }
            self.pin_app = Some(app.to_string());
            self.dialog_scroll = 0.0;
            return Action::None;
        }
        if id == "pin-new" {
            let Some(app) = self.pin_app.clone() else { return Action::None };
            return self.add_app_channel(&app);
        }
        if id == "pin-back" {
            self.pin_app = None;
            self.dialog_scroll = 0.0;
            return Action::None;
        }
        if let Some(channel) = id.strip_prefix("pin:") {
            let Some(app) = self.pin_app.clone() else { return Action::None };
            if !self.config.channels.iter().any(|item| item.id == channel) {
                return Action::None;
            }
            self.pin(app, channel.to_string());
            self.pin_app = None;
            self.overlay = Overlay::None;
            return Action::Save;
        }
        if let Some(name) = id.strip_prefix("suggest:") {
            return self.add_channel(name.to_string(), None);
        }
        if let Some(channel) = id.strip_prefix("name:") {
            let existing = self.config.channels.iter().find(|c| c.id == channel).map(|c| c.name.clone()).unwrap_or_default();
            return self.begin_name(NameEdit::Rename(channel.to_string()), existing);
        }
        if id == "name-field" {
            if !matches!(self.overlay, Overlay::Name(_)) {
                return Action::None;
            }
            self.name_keyboard = true;
            return Action::ShowKeyboard { description: t.channel_name.to_string(), existing: self.name_draft.clone() };
        }
        if id == "name-ok" {
            return self.commit_name();
        }
        if let Some(channel) = id.strip_prefix("rm:") {
            let armed = self.delete_until.as_ref().is_some_and(|(id, until)| id == channel && Instant::now() <= *until);
            if !armed {
                self.delete_until = Some((channel.to_string(), Instant::now() + Duration::from_secs(3)));
                return Action::None;
            }
            self.config.channels.retain(|item| item.id != channel);
            self.config.pins.retain(|pin| pin.channel != channel);
            self.delete_until = None;
            return Action::Save;
        }
        if id == "mute:mic" {
            self.config.mic_muted = !self.config.mic_muted;
            return Action::Save;
        }
        if let Some(channel) = id.strip_prefix("mute:tomic:") {
            if let Some(item) = self.config.channels.iter_mut().find(|c| c.id == channel) {
                item.to_mic_muted = !item.to_mic_muted;
            }
            return Action::Save;
        }
        if let Some(channel) = id.strip_prefix("mute:phones:") {
            if let Some(item) = self.config.channels.iter_mut().find(|c| c.id == channel) {
                item.phones_muted = !item.phones_muted;
            }
            return Action::Save;
        }
        if id == "rec" {
            self.record_pending = true;
            self.record_message = self.seen_message.clone();
            self.record_at = Some(Instant::now());
            return Action::Record;
        }
        if id == "stop" {
            self.record_pending = false;
            self.record_at = None;
            return Action::StopCheck;
        }
        if let Some(id) = id.strip_prefix("clip:") {
            if let Ok(clip) = id.parse::<u64>() {
                return Action::PlayClip(clip);
            }
        }
        if id == "upd-on" || id == "upd-off" {
            self.config.check_updates = id == "upd-on";
            return Action::Save;
        }
        if id == "upd-now" {
            return Action::CheckUpdates;
        }
        if id == "upd-install" {
            return Action::InstallUpdate;
        }
        if id == "apps-open" {
            self.overlay = Overlay::AppsList;
            self.dialog_scroll = 0.0;
            return Action::None;
        }
        if let Some(key) = id.strip_prefix("app:") {
            self.overlay = Overlay::Confirm(key.to_string());
            return Action::None;
        }
        if id == "confirm-no" {
            self.overlay = if matches!(self.page, Page::Apps) { Overlay::AppsList } else { Overlay::None };
            return Action::None;
        }
        if let Some(key) = id.strip_prefix("confirm-yes:") {
            self.overlay = Overlay::AppsList;
            return Action::OpenInstaller(key.to_string());
        }
        Action::None
    }

    fn fader_at(&self, x: f64, y: f64) -> Option<String> {
        self.faders.iter().rev().find(|f| x >= f.x && x < f.x + f.w && y >= f.y && y < f.y + f.h).map(|f| f.id.clone())
    }

    fn set_fader(&mut self, id: &str, x: f64, y: f64) {
        let Some((horizontal, fx, fy, fw, fh)) = self.faders.iter().find(|f| f.id == id).map(|f| (f.horizontal, f.x, f.y, f.w, f.h)) else {
            return;
        };
        let span = if horizontal { fw } else { fh };
        let along = if horizontal { x - fx } else { fy + fh - y };
        let t = if span <= 0.0 { 0.0 } else { (along / span).clamp(0.0, 1.0) };
        let mut value = config::clamp_volume(t * 1.5);
        if self.config.snap {
            let reach = if span > 0.0 { SNAP_REACH_PX / span * 1.5 } else { 0.0 };
            value = config::snap_volume(value, reach);
        }
        if id == "mic" {
            self.config.mic_volume = value;
        } else if let Some(rest) = id.strip_prefix("phones:") {
            if let Some(channel) = self.config.channels.iter_mut().find(|c| c.id == rest) {
                channel.phones = value;
            }
        } else if let Some(rest) = id.strip_prefix("tomic:") {
            if let Some(channel) = self.config.channels.iter_mut().find(|c| c.id == rest) {
                channel.to_mic = value;
            }
        }
    }

    /// The thumb follows the pointer; a click on the track jumps there.
    fn set_hscroll(&mut self, x: f64, y: f64) {
        let Some(bar) = self.hbar else { return };
        let (pos, origin, span) = if bar.vertical { (y, bar.y, bar.h) } else { (x, bar.x, bar.w) };
        let travel = span - bar.thumb_w;
        if travel <= 0.0 {
            return;
        }
        let t = ((pos - origin - bar.thumb_w / 2.0) / travel).clamp(0.0, 1.0);
        self.scroll = t * bar.max_scroll;
    }

    pub(crate) fn dragging(&self, id: &str) -> bool {
        self.drag.as_deref() == Some(id)
    }

    pub fn render(&mut self, fonts: &Fonts, snap: &Snapshot, peaks: &HashMap<String, f32>) -> Vec<u8> {
        let mut surface = self.render_surface(fonts, snap, peaks);
        draw::surface_to_rgba(&mut surface)
    }

    pub fn pixel_size(&self) -> (i32, i32) {
        draw::panel_pixels(self.config.wide, self.config.list)
    }

    pub fn interacting(&self) -> bool {
        self.drag.is_some()
    }

    pub(crate) fn size_f(&self) -> (f64, f64) {
        let (w, h) = self.pixel_size();
        (w as f64, h as f64)
    }

    /// Center a dialog, keeping `margin` clear of the panel edges.
    pub(crate) fn dialog_rect(&self, max_w: f64, max_h: f64, margin: f64) -> (f64, f64, f64, f64) {
        let (pw, ph) = self.size_f();
        let w = max_w.min(pw - margin * 2.0).max(280.0);
        let h = max_h.min(ph - margin * 2.0).max(200.0);
        ((pw - w) / 2.0, (ph - h) / 2.0, w, h)
    }

    pub(crate) fn sheet(&self, margin: f64) -> (f64, f64, f64, f64) {
        let (pw, ph) = self.size_f();
        (margin, margin, (pw - margin * 2.0).max(200.0), (ph - margin * 2.0).max(200.0))
    }

    pub(crate) fn stage(&self) -> Stage {
        let (w, h) = self.size_f();
        let footer = (28.0, h - 114.0, w - 56.0, 88.0);
        // The 9:10 list window is taller than it is wide. Mix Check still sits beside the sliders.
        let portrait = h > w && !self.config.list;
        if portrait {
            let check_h = 520.0_f64.min((h - 280.0) * 0.42).max(280.0);
            let check = (28.0, footer.1 - 14.0 - check_h, w - 56.0, check_h);
            let picker = (28.0, 92.0, w - 56.0, 60.0);
            let mix = (28.0, 168.0, w - 56.0, (check.1 - 16.0 - 168.0).max(180.0));
            return Stage { w, portrait, footer, picker, mix, check };
        }
        let check_w = 381.0;
        let check = (w - 28.0 - check_w, 92.0, check_w, footer.1 - 14.0 - 92.0);
        let mix_w = check.0 - 16.0 - 28.0;
        let picker = (28.0, 92.0, mix_w, 60.0);
        let mix = (28.0, 168.0, mix_w, check.1 + check.3 - 168.0);
        Stage { w, portrait, footer, picker, mix, check }
    }

    pub fn render_surface(&mut self, fonts: &Fonts, snap: &Snapshot, peaks: &HashMap<String, f32>) -> cairo::ImageSurface {
        let (w, h) = self.pixel_size();
        let surface = draw::surface_wh(w, h).expect("surface");
        let cr = Context::new(&surface).expect("context");
        self.draw(&cr, fonts, snap, peaks);
        surface
    }

    fn note_check(&mut self, snap: &Snapshot) {
        let failed = self.record_pending
            && snap.check != CheckPhase::Recording
            && (snap.message != self.record_message || self.record_at.is_some_and(|start| start.elapsed() > Duration::from_secs(8)));
        if snap.check == CheckPhase::Recording || failed {
            self.record_pending = false;
            self.record_at = None;
        }
        self.seen_message = snap.message.clone();
    }

    fn draw(&mut self, cr: &Context, fonts: &Fonts, snap: &Snapshot, peaks: &HashMap<String, f32>) {
        self.note_check(snap);
        draw::set_accent(&self.config.accent);
        self.hots.clear();
        self.faders.clear();
        self.hbar = None;
        self.strip = None;
        let (pw, ph) = self.size_f();
        fill_round(cr, 0.0, 0.0, pw, ph, 24.0, BG);
        let t = i18n::text(self.lang);
        self.header(cr, fonts, t);
        if self.page == Page::Apps {
            apps::draw(self, cr, fonts, t, snap);
        } else {
            mix::draw(self, cr, fonts, t, snap, peaks);
            check::draw(self, cr, fonts, t, snap, peaks);
        }
        footer::draw(self, cr, fonts, t, snap.autostart);
        if self.language_open {
            footer::language_menu(self, cr, fonts);
        }
        let overlay = match &self.overlay {
            Overlay::None => 0,
            Overlay::Add => 1,
            Overlay::Mic => 2,
            Overlay::Settings => 3,
            Overlay::AppsList => 4,
            Overlay::Confirm(_) => 5,
            Overlay::Name(_) => 6,
        };
        let confirm_key = if let Overlay::Confirm(key) = &self.overlay { key.clone() } else { String::new() };
        match overlay {
            1 => dialogs::add(self, cr, fonts, t, snap),
            2 => dialogs::mic(self, cr, fonts, t, snap),
            3 => dialogs::settings(self, cr, fonts, t),
            4 => apps::list(self, cr, fonts, t, snap),
            5 => apps::confirm(self, cr, fonts, t, snap, &confirm_key),
            6 => dialogs::name(self, cr, fonts, t),
            _ => {}
        }
    }

    fn header(&mut self, cr: &Context, fonts: &Fonts, t: &Text) {
        draw::label(cr, fonts, true, 28.0, TEXT, 36.0, 58.0, t.title);
        let (w, _) = self.size_f();
        let rect = (w - 236.0, 22.0, 208.0, 56.0);
        if self.page == Page::Mix {
            self.button(cr, fonts, rect.0, rect.1, rect.2, rect.3, t.apps, ButtonKind::Plain);
            self.hot("apps", rect.0, rect.1, rect.2, rect.3);
        } else {
            self.button(cr, fonts, rect.0, rect.1, rect.2, rect.3, &format!("‹  {}", t.back_mixer), ButtonKind::Plain);
            self.hot("back", rect.0, rect.1, rect.2, rect.3);
        }
    }

    pub fn thumbnail(&self, _fonts: &Fonts) -> Vec<u8> {
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 256, 256).expect("thumb");
        {
            let cr = Context::new(&surface).expect("context");
            let bytes = crate::brand::icon_png(self.config.accent == "pink", 256);
            draw::paint_png_bytes(&cr, bytes, 0.0, 0.0, 256.0, 256.0);
        }
        draw::surface_to_rgba(&mut surface)
    }

    pub(crate) fn hot(&mut self, id: &str, x: f64, y: f64, w: f64, h: f64) {
        self.hots.push(Hot { id: id.to_string(), x, y, w, h });
    }

    pub(crate) fn fader_hit(&mut self, id: &str, x: f64, y: f64, w: f64, h: f64, horizontal: bool) {
        self.faders.push(FaderHit { id: id.to_string(), x, y, w, h, horizontal });
    }

    pub(crate) fn set_strip(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.strip = Some((x, y, w, h));
    }

    fn hot_at(&self, x: f64, y: f64) -> Option<String> {
        self.hots.iter().rev().find(|hot| x >= hot.x && x < hot.x + hot.w && y >= hot.y && y < hot.y + hot.h).map(|hot| hot.id.clone())
    }

    pub(crate) fn look(&self, x: f64, y: f64, w: f64, h: f64) -> Look {
        let inside = self.pointer.is_some_and(|(px, py)| px >= x && px < x + w && py >= y && py < y + h);
        if inside && self.pressed {
            Look::Down
        } else if inside {
            Look::Hover
        } else {
            Look::Rest
        }
    }

    pub(crate) fn button(&mut self, cr: &Context, fonts: &Fonts, x: f64, y: f64, w: f64, h: f64, caption: &str, kind: ButtonKind) {
        draw::button(cr, fonts, x, y, w, h, caption, kind, self.look(x, y, w, h));
    }

    pub(crate) fn quit_armed(&self) -> bool {
        self.quit_until.is_some_and(|until| Instant::now() <= until)
    }

    pub(crate) fn delete_armed(&self, id: &str) -> bool {
        self.delete_until.as_ref().is_some_and(|(armed, until)| armed == id && Instant::now() <= *until)
    }
}

pub(crate) fn resolved_source(choice: &str, graph: &Graph) -> Option<String> {
    let wanted = if choice == "builtin" {
        Graph::BUILTIN_SOURCE.to_string()
    } else if choice.is_empty() {
        if !graph.default_source.is_empty() {
            graph.default_source.clone()
        } else if graph.node_by_name(Graph::BUILTIN_SOURCE).is_some() {
            Graph::BUILTIN_SOURCE.to_string()
        } else {
            String::new()
        }
    } else {
        choice.to_string()
    };
    if live_source(graph, &wanted) {
        return Some(wanted);
    }
    if choice.is_empty() || choice == "builtin" {
        if live_source(graph, Graph::BUILTIN_SOURCE) {
            return Some(Graph::BUILTIN_SOURCE.to_string());
        }
    }
    if live_source(graph, &graph.default_source) {
        return Some(graph.default_source.clone());
    }
    None
}

fn live_source(graph: &Graph, name: &str) -> bool {
    !name.is_empty() && name != "frame_amix_source" && graph.node_by_name(name).is_some()
}

pub(crate) fn source_label(choice: &str, graph: &Graph, t: &Text) -> String {
    if choice == "builtin" {
        return t.mic_builtin.to_string();
    }
    if choice.is_empty() {
        let desc = graph.description_of(&graph.default_source);
        if desc.is_empty() { t.mic_system.to_string() } else { format!("{} · {desc}", t.mic_system) }
    } else {
        let desc = graph.description_of(choice);
        if desc.is_empty() { choice.to_string() } else { desc }
    }
}

pub(crate) fn fill_round(cr: &Context, x: f64, y: f64, w: f64, h: f64, r: f64, color: Rgb) {
    draw::rounded(cr, x, y, w, h, r);
    draw::fill(cr, color, 1.0);
}

pub(crate) fn stroke_round(cr: &Context, x: f64, y: f64, w: f64, h: f64, r: f64, color: Rgb, width: f64) {
    draw::rounded(cr, x, y, w, h, r);
    draw::stroke(cr, color, width);
}

pub(crate) fn backdrop(panel: &mut Panel, cr: &Context) {
    let (w, h) = panel.size_f();
    fill_round(cr, 0.0, 0.0, w, h, 24.0, BG);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.45);
    let _ = cr.paint();
    panel.hot("dismiss", 0.0, 0.0, w, h);
}

pub(crate) fn fmt_token(template: &str, key: &str, value: &str) -> String {
    template.replace(&format!("{{{key}}}"), value)
}

pub fn render_png(fonts: &Fonts, lang: Lang, path: &std::path::Path) -> Result<(), String> {
    let mut panel = Panel::demo(lang);
    let mut snap = Snapshot::default();
    snap.mix_active = true;
    snap.echo_cancel = Some(false);
    snap.graph.default_sink = "alsa_output.platform-sound.HiFi__Speaker__sink".into();
    snap.graph.default_source = Graph::BUILTIN_SOURCE.into();
    snap.check = CheckPhase::Ready;
    snap.clips.push(crate::worker::MixClip {
        id: 1,
        seconds: 4.2,
        clock: "21:52:10".into(),
        wave: (0..36).map(|i| (i as f32 * 0.7).sin().abs() * 0.6).collect(),
    });
    snap.apps = demo_apps();
    snap.update.state = crate::updater::UpdateState::UpToDate;
    snap.update.latest = crate::updater::VERSION.into();
    let peaks = HashMap::new();
    write_surface(&mut panel, fonts, &snap, &peaks, path)?;
    panel.page = Page::Apps;
    write_surface(&mut panel, fonts, &snap, &peaks, &suffix(path, "apps"))?;
    panel.overlay = Overlay::AppsList;
    write_surface(&mut panel, fonts, &snap, &peaks, &suffix(path, "list"))?;
    panel.overlay = Overlay::Confirm("mic".into());
    write_surface(&mut panel, fonts, &snap, &peaks, &suffix(path, "confirm"))?;
    panel.overlay = Overlay::Add;
    snap.graph.clients.push(crate::graph::AppRef { name: "Zen".into(), binary: "zen".into() });
    snap.graph.clients.push(crate::graph::AppRef { name: "Steam".into(), binary: "steam".into() });
    write_surface(&mut panel, fonts, &snap, &peaks, &suffix(path, "add"))?;
    panel.pin_app = Some("Steam".into());
    write_surface(&mut panel, fonts, &snap, &peaks, &suffix(path, "pin"))?;
    panel.pin_app = None;
    panel.overlay = Overlay::None;
    panel.page = Page::Mix;
    panel.scroll = f64::MAX;
    snap.check = CheckPhase::Recording;
    snap.check_secs = 3.0;
    let mut loud = HashMap::new();
    loud.insert("mic".to_string(), 0.4f32);
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "scrolled"))?;
    snap.check = CheckPhase::Ready;
    panel.scroll = 0.0;
    panel.overlay = Overlay::Settings;
    panel.color_open = true;
    write_surface(&mut panel, fonts, &snap, &peaks, &suffix(path, "settings"))?;
    panel.overlay = Overlay::None;
    panel.language_open = true;
    write_surface(&mut panel, fonts, &snap, &peaks, &suffix(path, "lang"))?;
    Ok(())
}

fn write_surface(panel: &mut Panel, fonts: &Fonts, snap: &Snapshot, peaks: &HashMap<String, f32>, path: &std::path::Path) -> Result<(), String> {
    let surface = panel.render_surface(fonts, snap, peaks);
    let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    surface.write_to_png(&mut file).map_err(|e| e.to_string())
}

fn suffix(path: &std::path::Path, name: &str) -> std::path::PathBuf {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("panel");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("png");
    path.with_file_name(format!("{stem}-{name}.{ext}"))
}

fn demo_apps() -> Vec<Entry> {
    crate::apps::bundled()
        .into_iter()
        .enumerate()
        .map(|(i, info)| Entry {
            related: info.name == "frame-mic-tuner",
            state: if info.name == "frame-mic-tuner" {
                crate::apps::AppState::Running
            } else if i % 2 == 0 {
                crate::apps::AppState::Installed
            } else {
                crate::apps::AppState::Missing
            },
            name: info.name,
            key: info.key,
            mono: info.mono,
            desc_ja: info.desc_ja,
            desc_en: info.desc_en,
            busy: false,
            icon_path: String::new(),
        })
        .collect()
}

pub(crate) fn paint_app_icon(panel: &mut Panel, cr: &Context, fonts: &Fonts, name: &str, mono: &str, icon_path: &str, x: f64, y: f64, size: f64) -> bool {
    if !icon_path.is_empty() {
        if paint_png(panel, cr, icon_path, x, y, size) {
            return true;
        }
    }
    if let Some((bytes, _)) = crate::apps::embedded_png(name) {
        let key = format!("embedded:{name}");
        if panel.pngs.get(&key).is_none() {
            if let Ok(surface) = cairo::ImageSurface::create_from_png(&mut std::io::Cursor::new(bytes)) {
                panel.pngs.insert(key.clone(), surface);
            }
        }
        if paint_cached(cr, panel.pngs.get(&key), x, y, size) {
            return true;
        }
    }
    fill_round(cr, x, y, size, size, size * 0.22, draw::CONTROL);
    let letter: String = mono.chars().take(4).collect();
    let width = draw::text_width(cr, fonts, true, size * 0.28, &letter);
    draw::label(cr, fonts, true, size * 0.28, draw::accent(), x + (size - width) / 2.0, y + size * 0.62, &letter);
    false
}

fn paint_png(panel: &mut Panel, cr: &Context, path: &str, x: f64, y: f64, size: f64) -> bool {
    if !panel.pngs.contains_key(path) {
        let Ok(mut file) = std::fs::File::open(path) else { return false };
        let Ok(surface) = cairo::ImageSurface::create_from_png(&mut file) else { return false };
        panel.pngs.insert(path.to_string(), surface);
    }
    paint_cached(cr, panel.pngs.get(path), x, y, size)
}

fn paint_cached(cr: &Context, surface: Option<&cairo::ImageSurface>, x: f64, y: f64, size: f64) -> bool {
    let Some(surface) = surface else { return false };
    let _ = cr.save();
    cr.translate(x, y);
    let w = surface.width() as f64;
    if w > 0.0 {
        cr.scale(size / w, size / w);
    }
    cr.set_source_surface(surface, 0.0, 0.0).ok();
    let _ = cr.paint();
    let _ = cr.restore();
    true
}

/// Running-app icon from the desktop index, or a letter tile.
pub(crate) fn paint_desktop_icon(panel: &mut Panel, cr: &Context, fonts: &Fonts, app: &str, binary: &str, x: f64, y: f64, size: f64) {
    let path = panel.icons.as_mut().and_then(|index| index.lookup(app, binary));
    if let Some(path) = path {
        let path = path.to_string_lossy().to_string();
        if paint_png(panel, cr, &path, x, y, size) {
            return;
        }
    }
    let letter: String = app.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".into());
    fill_round(cr, x, y, size, size, size * 0.25, draw::CONTROL);
    let font = size * 0.5;
    let width = draw::text_width(cr, fonts, true, font, &letter);
    draw::label(cr, fonts, true, font, TEXT, x + (size - width) / 2.0, y + size * 0.68, &letter);
}

fn truncate_name(text: &str) -> String {
    text.chars().take(config::NAME_MAX_CHARS).collect()
}

/// The headset keyboard reports either the whole line (`full`) or only the new key (`key`).
fn merge_keyboard(draft: &str, full: &str, key: &str) -> String {
    if key == "\u{8}" || key == "\u{7f}" {
        let mut chars: Vec<char> = draft.chars().collect();
        chars.pop();
        return chars.into_iter().collect();
    }
    let full_n = full.chars().count();
    let draft_n = draft.chars().count();
    if !full.is_empty() && full.starts_with(draft) && full_n > draft_n {
        return truncate_name(full);
    }
    // A one-character read is the newest key, not the line, so it must not wipe the name.
    if key.is_empty() && full_n > 1 && draft.starts_with(full) && draft_n > full_n && draft_n - full_n <= 2 {
        return truncate_name(full);
    }
    if !key.is_empty() {
        if !full.is_empty() && full.starts_with(draft) && full.ends_with(key) && full_n >= draft_n {
            return truncate_name(full);
        }
        if key.starts_with(draft) && key.chars().count() >= draft_n {
            return truncate_name(key);
        }
        return truncate_name(&format!("{draft}{key}"));
    }
    draft.to_string()
}

pub fn self_test() -> Result<(), String> {
    if merge_keyboard("Mu", "Music", "s") != "Music" {
        return Err("keyboard full line".into());
    }
    if merge_keyboard("Musi", "c", "c") != "Music" {
        return Err("keyboard one key".into());
    }
    if merge_keyboard("Music", "M", "c") != "Musicc" {
        return Err("keyboard keeps earlier letters".into());
    }
    if merge_keyboard("Music", "Music", "c") != "Music" {
        return Err("keyboard repeats the last key".into());
    }
    if merge_keyboard("Music", "", "\u{8}") != "Musi" {
        return Err("keyboard backspace".into());
    }
    let mut config = crate::config::Config::default();
    config.list = true;
    config.wide = true;
    let stage = Panel::new(config).stage();
    if stage.portrait || stage.check.0 <= stage.mix.0 + stage.mix.2 {
        return Err("list extend check on the right".into());
    }
    Ok(())
}
