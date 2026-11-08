//! Dashboard panel: the mix strip, Mix Check, dialogs, and Apps & updates.
//!
//! Geometry follows Frame Mic Tuner's Mix tab. The microphone chosen here is
//! only the mixer's input. It is never made the system default.

mod apps;
mod dialogs;
mod eq;
mod footer;
mod fx;
mod mix;
mod outputs;
mod rail;

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
    Bus(usize),
}

enum Overlay {
    None,
    Add,
    Mic,
    Settings,
    AppsList,
    Confirm(String),
    Name(NameEdit),
    Eq(String),
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
    pub mix: (f64, f64, f64, f64),
    pub check: (f64, f64, f64, f64),
    pub rail: (f64, f64, f64, f64),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Drawer {
    Closed,
    FxA,
    FxB,
    Outputs,
}

/// Window margin, the gap between cards, and the header and footer bands.
pub(crate) const MARGIN: f64 = 24.0;
pub(crate) const GUTTER: f64 = 16.0;
pub(crate) const HEADER_H: f64 = 48.0;
pub(crate) const CONTENT_TOP: f64 = MARGIN + HEADER_H + GUTTER;
const FOOTER_H: f64 = 80.0;
const RAIL_W: f64 = 48.0;
const DRAWER_W: f64 = 384.0;
const DRAWER_MS: f64 = 0.22;

const HSCROLL: &str = "hscroll";
const PAN: &str = "pan";
const OUT_PAN: &str = "outpan";
const SPLIT: &str = "split";

/// The two sections share `span`. Each one keeps at least 20% of the window.
fn split_limits(w: f64) -> (f64, f64) {
    let span = (w - 2.0 * MARGIN - RAIL_W - 2.0 * GUTTER).max(0.0);
    let min_w = (0.20 * w).min(span * 0.5);
    (min_w, span)
}
/// How close, in pixels, a fader drag has to come to a snap point.
const SNAP_REACH_PX: f64 = 8.0;
const LEVELS_EVERY: Duration = Duration::from_millis(40);

pub struct Panel {
    pub config: Config,
    lang: Lang,
    page: Page,
    overlay: Overlay,
    language_open: bool,
    color_open: bool,
    /// The Mix Check output menu is open.
    pub(crate) check_open: bool,
    /// The Mix Check playback output menu is open.
    pub(crate) play_open: bool,
    /// Vertical scroll of the Outputs drawer.
    pub(crate) out_scroll: f64,
    /// Output containers from the last frame. Empty space here drag-scrolls them.
    pub(crate) out_list: Option<(f64, f64, f64, f64)>,
    eq_band: usize,
    eq_graph: Option<(f64, f64, f64, f64)>,
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
    /// Snapshot epoch when Record was clicked. A later epoch means the attempt ended.
    record_epoch: u64,
    record_at: Option<Instant>,
    seen_epoch: u64,
    levels_sent: Option<Instant>,
    drawer: Drawer,
    drawer_target: Drawer,
    drawer_progress: f64,
    anim_at: Instant,
    pub(crate) fx_edit: Option<(u8, usize)>,
    fx_drag: Option<(u8, usize)>,
}

pub enum Action {
    None,
    Save,
    /// Fader or effect values changed during a drag. Applied, not written to disk.
    Levels,
    Quit,
    ShowKeyboard { description: String, existing: String },
    Record,
    StopCheck,
    PlayTake,
    PauseTake,
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
            check_open: false,
            play_open: false,
            out_scroll: 0.0,
            out_list: None,
            eq_band: 0,
            eq_graph: None,
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
            record_epoch: 0,
            record_at: None,
            seen_epoch: 0,
            levels_sent: None,
            drawer: Drawer::Outputs,
            drawer_target: Drawer::Outputs,
            drawer_progress: 1.0,
            anim_at: Instant::now(),
            fx_edit: None,
            fx_drag: None,
        }
    }

    pub fn demo(lang: Lang) -> Self {
        let mut config = Config::default();
        config.language = lang.code().into();
        config.mix_on = true;
        config.channels.push(config::Channel::new("ch1", "Music"));
        let mut game = config::Channel::new("ch2", "Game");
        game.gain_db = -2.0;
        config.channels.push(game);
        let mut zen = config::Channel::new("ch3", "Zen");
        zen.app = "Zen".into();
        zen.binary = "zen".into();
        zen.gain_db = -8.0;
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
        let dt = now.saturating_duration_since(self.anim_at).as_secs_f64().min(0.05);
        self.anim_at = now;
        let target = if self.drawer_target == Drawer::Closed { 0.0 } else { 1.0 };
        if (self.drawer_progress - target).abs() > 0.001 {
            let step = dt / DRAWER_MS;
            if self.drawer_progress < target {
                self.drawer_progress = (self.drawer_progress + step).min(1.0);
            } else {
                self.drawer_progress = (self.drawer_progress - step).max(0.0);
            }
        } else {
            self.drawer_progress = target;
        }
        if self.drawer_target != Drawer::Closed {
            self.drawer = self.drawer_target;
        } else if self.drawer_progress <= 0.02 {
            self.drawer = Drawer::Closed;
        }
    }

    pub fn animating(&self) -> bool {
        let target = if self.drawer_target == Drawer::Closed { 0.0 } else { 1.0 };
        (self.drawer_progress - target).abs() > 0.001
    }

    pub(crate) fn drawer_shows_fx(&self, bus: usize) -> bool {
        match bus {
            0 => self.drawer_target == Drawer::FxA,
            _ => self.drawer_target == Drawer::FxB,
        }
    }

    pub(crate) fn drawer_shows_outputs(&self) -> bool {
        self.drawer_target == Drawer::Outputs
    }

    fn set_drawer(&mut self, next: Drawer) {
        if self.drawer_target == next {
            self.drawer_target = Drawer::Closed;
        } else {
            self.drawer_target = next;
            if self.drawer_progress > 0.5 {
                self.drawer = next;
            }
        }
    }

    /// Taps do not follow the page or a dialog. Starting and stopping capture streams is
    /// itself a graph change, and the dashboard opening is the worst time for one.
    pub fn wants_meters(&self) -> bool {
        self.config.meters
    }

    /// PipeWire nodes to meter. The mic tap is not passive. Channel taps are.
    pub fn meter_targets(&self, snap: &Snapshot) -> Vec<crate::meter::Target> {
        let mut targets = Vec::new();
        if !self.wants_meters() {
            return targets;
        }
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
            NameEdit::Bus(_) => None,
        };
        if matches!(mode, NameEdit::Bus(_)) {
            if let NameEdit::Bus(index) = mode {
                if index < 4 {
                    self.config.b[index].name = name;
                }
            }
            self.overlay = Overlay::None;
            self.name_draft.clear();
            self.name_keyboard = false;
            return Action::Save;
        }
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
            NameEdit::Bus(_) => Action::None,
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
                        let delta = if self.config.input_list { y - py } else { x - px };
                        self.scroll = (self.scroll - delta).max(0.0);
                    }
                    self.pan_at = Some((x, y));
                } else if id == OUT_PAN {
                    if let Some((px, py)) = self.pan_at {
                        let delta = if self.config.list { x - px } else { y - py };
                        self.out_scroll = (self.out_scroll - delta).max(0.0);
                    }
                    self.pan_at = Some((x, y));
                } else if id == SPLIT {
                    self.set_split(x);
                } else if id.starts_with("fxgrip:") {
                    self.fx_reorder(y);
                } else if let Some(rest) = id.strip_prefix("eqband:") {
                    if let Overlay::Eq(who) = &self.overlay {
                        let who = who.clone();
                        let index = rest.parse().unwrap_or(0);
                        eq::drag_handle(self, &who, index, x, y);
                    }
                    return self.live_levels();
                } else {
                    self.set_fader(&id, x, y);
                    return self.live_levels();
                }
            }
            return Action::None;
        }
        if kind == crate::overlay::MOUSE_UP {
            self.pressed = false;
            self.pan_at = None;
            let dragged = self.drag.take();
            self.fx_drag = None;
            let save = dragged.as_deref().is_some_and(|id| id != HSCROLL && id != PAN && id != OUT_PAN);
            return if save { Action::Save } else { Action::None };
        }
        if kind == crate::overlay::SCROLL_DISCRETE || kind == crate::overlay::SCROLL_SMOOTH {
            // List rows and dialogs move on the vertical stick. The channel row moves on the horizontal one.
            let vertical = matches!(self.overlay, Overlay::Add | Overlay::Mic | Overlay::AppsList);
            let use_y = if vertical { y.abs() >= x.abs() } else { y.abs() > x.abs() };
            let delta = if use_y { y } else { x };
            // A controller notch is about ±1, which used to move the row by one pixel.
            let step = if delta.abs() <= 3.0 { delta * 150.0 } else { delta * 4.0 };
            if let Overlay::Eq(who) = &self.overlay {
                let who = who.clone();
                eq::nudge_q(self, &who, delta);
                return Action::Levels;
            } else if matches!(self.overlay, Overlay::Add | Overlay::Mic | Overlay::AppsList) {
                self.dialog_scroll = (self.dialog_scroll - step * 0.45).max(0.0);
            } else if self.over_outputs() {
                let delta = if y.abs() >= x.abs() { y } else { x };
                let step = if delta.abs() <= 3.0 { delta * 150.0 } else { delta * 4.0 };
                self.out_scroll = (self.out_scroll - step * 0.45).max(0.0);
            } else if self.config.input_list {
                let delta = if y.abs() >= x.abs() { y } else { x };
                let step = if delta.abs() <= 3.0 { delta * 150.0 } else { delta * 4.0 };
                self.scroll = (self.scroll - step).max(0.0);
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
        if self.check_open || self.play_open {
            let menu = if self.check_open { "check-out" } else { "play-to" };
            let on_menu = self.hot_at(x, y).is_some_and(|id| id.starts_with(menu));
            if !on_menu {
                self.check_open = false;
                self.play_open = false;
                return Action::None;
            }
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
            self.levels_sent = None;
            return self.live_levels();
        }
        if self.hot_at(x, y).is_some() {
            return self.hit_at(x, y);
        }
        if self.strip.is_some_and(|(sx, sy, sw, sh)| x >= sx && x < sx + sw && y >= sy && y < sy + sh) {
            self.drag = Some(PAN.to_string());
            self.pan_at = Some((x, y));
            return Action::None;
        }
        if self.out_list.is_some_and(|(lx, ly, lw, lh)| x >= lx && x < lx + lw && y >= ly && y < ly + lh) {
            self.drag = Some(OUT_PAN.to_string());
            self.pan_at = Some((x, y));
            return Action::None;
        }
        Action::None
    }

    fn over_outputs(&self) -> bool {
        let Some((lx, ly, lw, lh)) = self.out_list else { return false };
        self.pointer.is_some_and(|(x, y)| x >= lx && x < lx + lw && y >= ly && y < ly + lh)
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
            self.out_scroll = 0.0;
            return Action::Save;
        }
        if id == "in-list-on" || id == "in-list-off" {
            self.config.input_list = id == "in-list-on";
            self.scroll = 0.0;
            return Action::Save;
        }
        if id == "wide-on" || id == "wide-off" {
            self.config.wide = id == "wide-on";
            return Action::Save;
        }
        if id == SPLIT {
            self.drag = Some(SPLIT.to_string());
            if let Some((x, _)) = self.pointer {
                self.set_split(x);
            }
            return Action::None;
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
            return self.add_app_channel(app);
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
        if id == "rail-fx-0" {
            self.set_drawer(Drawer::FxA);
            return Action::None;
        }
        if id == "rail-fx-1" {
            self.set_drawer(Drawer::FxB);
            return Action::None;
        }
        if id == "rail-out" {
            self.set_drawer(Drawer::Outputs);
            return Action::None;
        }
        if let Some(action) = self.handle_eq(id) {
            return action;
        }
        if let Some(rest) = id.strip_prefix("bmute:") {
            self.toggle_bus_mute(rest);
            return Action::Save;
        }
        if let Some(rest) = id.strip_prefix("bname:") {
            let index: usize = rest.parse().unwrap_or(9);
            if index < config::B_SLOTS {
                let existing = self.config.b[index].name.clone();
                return self.begin_name(NameEdit::Bus(index), existing);
            }
        }
        if let Some(action) = self.handle_fader_toggle(id) {
            return action;
        }
        if let Some(action) = self.handle_fx(id) {
            return action;
        }
        if id == "rec" {
            self.record_pending = true;
            self.record_epoch = self.seen_epoch;
            self.record_at = Some(Instant::now());
            return Action::Record;
        }
        if id == "stop" {
            self.record_pending = false;
            self.record_at = None;
            return Action::StopCheck;
        }
        if id == "check-play" {
            return Action::PlayTake;
        }
        if id == "check-pause" {
            return Action::PauseTake;
        }
        if id == "check-out-open" {
            self.check_open = !self.check_open;
            self.play_open = false;
            return Action::None;
        }
        if id == "play-to-open" {
            self.play_open = !self.play_open;
            self.check_open = false;
            return Action::None;
        }
        if let Some(value) = id.strip_prefix("play-to:") {
            self.play_open = false;
            if !value.is_empty() && config::check_play_valid(value) && self.config.check_play != value {
                self.config.check_play = value.to_string();
                return Action::Save;
            }
            return Action::None;
        }
        if let Some(value) = id.strip_prefix("check-out:") {
            self.check_open = false;
            if config::check_output_valid(value) && self.config.check_output != value {
                self.config.check_output = value.to_string();
                return Action::Save;
            }
            return Action::None;
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

    fn handle_fader_toggle(&mut self, id: &str) -> Option<Action> {
        if let Some(rest) = id.strip_prefix("route:") {
            self.toggle_route(rest);
            return Some(Action::Save);
        }
        if let Some(rest) = id.strip_prefix("send:") {
            self.toggle_send(rest);
            return Some(Action::Save);
        }
        if let Some(rest) = id.strip_prefix("mute:") {
            self.toggle_mute(rest);
            return Some(Action::Save);
        }
        if let Some(rest) = id.strip_prefix("fxa:") {
            self.toggle_fx(rest, true);
            return Some(Action::Save);
        }
        if let Some(rest) = id.strip_prefix("fxb:") {
            self.toggle_fx(rest, false);
            return Some(Action::Save);
        }
        None
    }

    fn toggle_route(&mut self, id: &str) {
        let mut parts = id.split(':');
        let who = parts.next().unwrap_or("");
        let side = parts.next().unwrap_or("");
        if side == "pc" {
            let to_pc = if who == "mic" {
                Some(&mut self.config.mic_to_pc)
            } else if who == "fx1" {
                Some(&mut self.config.fx_a.to_pc)
            } else if who == "fx2" {
                Some(&mut self.config.fx_b.to_pc)
            } else {
                self.config.channels.iter_mut().find(|channel| channel.id == who).map(|channel| &mut channel.to_pc)
            };
            if let Some(on) = to_pc {
                *on = !*on;
            }
            return;
        }
        let index: usize = parts.next().unwrap_or("0").parse().unwrap_or(9);
        let bit = config::route_bit(side == "a", index);
        if bit == 0 {
            return;
        }
        let routes = if who == "mic" {
            Some(&mut self.config.mic_routes)
        } else if who == "fx1" {
            Some(&mut self.config.fx_a.routes)
        } else if who == "fx2" {
            Some(&mut self.config.fx_b.routes)
        } else {
            self.config.channels.iter_mut().find(|channel| channel.id == who).map(|channel| &mut channel.routes)
        };
        if let Some(routes) = routes {
            *routes ^= bit;
        }
    }

    fn toggle_send(&mut self, id: &str) {
        let Some((who, which)) = id.split_once(':') else { return };
        let second = which == "2";
        if who == "mic" {
            if second { self.config.mic_send_fx2 = !self.config.mic_send_fx2; } else { self.config.mic_send_fx1 = !self.config.mic_send_fx1; }
            return;
        }
        if let Some(channel) = self.config.channels.iter_mut().find(|channel| channel.id == who) {
            if second { channel.send_fx2 = !channel.send_fx2; } else { channel.send_fx1 = !channel.send_fx1; }
        }
    }

    fn toggle_mute(&mut self, id: &str) {
        if id == "mic" {
            self.config.mic_muted = !self.config.mic_muted;
        } else if let Some(item) = self.config.channels.iter_mut().find(|channel| channel.id == id) {
            item.muted = !item.muted;
        }
    }

    fn toggle_fx(&mut self, id: &str, bus_a: bool) {
        let _ = (id, bus_a);
    }

    fn handle_fx(&mut self, id: &str) -> Option<Action> {
        if let Some(rest) = id.strip_prefix("fxadd:") {
            let (bus, kind) = rest.split_once(':')?;
            let bus: usize = bus.parse().ok()?;
            let kind = crate::config::EffectKind::parse(kind)?;
            let chain = &mut fx::bus_mut(&mut self.config, bus).chain;
            if chain.len() >= 3 {
                return Some(Action::None);
            }
            let mut slot = crate::config::EffectSlot::new(kind);
            if kind == crate::config::EffectKind::Eq {
                slot.eq = Some(crate::config::ParamEq::new(6));
            }
            chain.push(slot);
            let index = chain.len() - 1;
            if kind == crate::config::EffectKind::Eq {
                self.fx_edit = None;
                self.eq_band = 0;
                self.overlay = Overlay::Eq(format!("fx:{bus}:{index}"));
            } else {
                self.fx_edit = Some((bus as u8, index));
            }
            return Some(Action::Save);
        }
        if let Some(rest) = id.strip_prefix("fxrm:") {
            let (bus, index) = rest.split_once(':')?;
            let bus: usize = bus.parse().ok()?;
            let index: usize = index.parse().ok()?;
            let chain = &mut fx::bus_mut(&mut self.config, bus).chain;
            if index < chain.len() {
                chain.remove(index);
            }
            self.fx_edit = None;
            return Some(Action::Save);
        }
        if let Some(rest) = id.strip_prefix("fxedit:") {
            let (bus, index) = rest.split_once(':')?;
            let bus: u8 = bus.parse().ok()?;
            let index: usize = index.parse().ok()?;
            let is_eq = fx::bus_mut(&mut self.config, bus as usize).chain.get(index).is_some_and(|slot| slot.kind == crate::config::EffectKind::Eq);
            if is_eq {
                self.fx_edit = None;
                self.eq_band = 0;
                self.overlay = Overlay::Eq(format!("fx:{bus}:{index}"));
                return Some(Action::None);
            }
            self.fx_edit = if self.fx_edit == Some((bus, index)) { None } else { Some((bus, index)) };
            return Some(Action::None);
        }
        if let Some(rest) = id.strip_prefix("fxsnap:") {
            let (bus, index) = rest.split_once(':')?;
            let bus: usize = bus.parse().ok()?;
            let index: usize = index.parse().ok()?;
            if let Some(slot) = fx::bus_mut(&mut self.config, bus).chain.get_mut(index) {
                if slot.kind == crate::config::EffectKind::Pitcher {
                    slot.pitch_snap = !slot.pitch_snap;
                    if slot.pitch_snap {
                        slot.params[0] = crate::config::pitch_snapped(slot.params[0]);
                    }
                }
            }
            return Some(Action::Save);
        }
        if let Some(rest) = id.strip_prefix("fxgrip:") {
            let (bus, index) = rest.split_once(':')?;
            let bus: u8 = bus.parse().ok()?;
            let index: usize = index.parse().ok()?;
            self.fx_drag = Some((bus, index));
            self.drag = Some(id.to_string());
            return Some(Action::None);
        }
        None
    }

    /// Volumes while a fader is held. At most one every `LEVELS_EVERY`, so a drag does not
    /// queue a `wpctl` per pointer event. Release sends `Save` with the final value.
    fn live_levels(&mut self) -> Action {
        let due = self.levels_sent.is_none_or(|at| at.elapsed() >= LEVELS_EVERY);
        if !due {
            return Action::None;
        }
        self.levels_sent = Some(Instant::now());
        Action::Levels
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
        if let Some(rest) = id.strip_prefix("fxp:") {
            let mut parts = rest.split(':');
            let bus: usize = parts.next().unwrap_or("").parse().unwrap_or(99);
            let index: usize = parts.next().unwrap_or("").parse().unwrap_or(99);
            let param: usize = parts.next().unwrap_or("").parse().unwrap_or(99);
            if let Some(slot) = fx::bus_mut(&mut self.config, bus).chain.get_mut(index) {
                if param < slot.params.len() {
                    let snap = slot.kind == crate::config::EffectKind::Pitcher && param == 0 && slot.pitch_snap;
                    slot.params[param] = if snap { crate::config::pitch_snapped(t) } else { t };
                }
            }
            return;
        }
        let mut db = config::t_to_db(t);
        // The snap button sits on Outputs and moves only those faders. Input strips always snap.
        let magnetic = if id.starts_with("bus:") { self.config.snap } else { true };
        if magnetic && db.is_finite() {
            let reach = if span > 0.0 { SNAP_REACH_PX / span * (config::FADER_DB_MAX - config::FADER_DB_MIN) } else { 0.8 };
            for mark in [12.0, 6.0, 0.0, -6.0, -12.0, -24.0, -48.0] {
                if (db - mark).abs() <= reach.max(0.8) {
                    db = mark;
                    break;
                }
            }
        }
        if id == "mic" {
            self.config.mic_gain_db = db;
        } else if let Some(channel) = self.config.channels.iter_mut().find(|channel| channel.id == id) {
            channel.gain_db = db;
        } else if id == "bus:pc" {
            self.config.pc.gain_db = db;
        } else if let Some(rest) = id.strip_prefix("bus:") {
            let mut parts = rest.split(':');
            let side = parts.next().unwrap_or("");
            let index: usize = parts.next().unwrap_or("0").parse().unwrap_or(9);
            if index < 4 {
                let bus = if side == "a" { &mut self.config.a[index] } else { &mut self.config.b[index] };
                bus.gain_db = db;
            }
        } else if let Some(rest) = id.strip_prefix("fxret:") {
            let index: usize = rest.parse().unwrap_or(9);
            let bus = if index == 0 { &mut self.config.fx_a } else { &mut self.config.fx_b };
            bus.gain_db = db;
        } else if let Some(which) = id.strip_prefix("eq-") {
            if let Overlay::Eq(who) = &self.overlay {
                let who = who.clone();
                eq::set_slider(self, &who, which, t);
            }
        }
    }

    fn toggle_bus_mute(&mut self, id: &str) {
        if id == "pc" {
            self.config.pc.muted = !self.config.pc.muted;
            return;
        }
        let mut parts = id.split(':');
        let side = parts.next().unwrap_or("");
        let index: usize = parts.next().unwrap_or("0").parse().unwrap_or(9);
        if index < 4 {
            let bus = if side == "a" { &mut self.config.a[index] } else { &mut self.config.b[index] };
            bus.muted = !bus.muted;
        }
    }

    fn handle_eq(&mut self, id: &str) -> Option<Action> {
        let who = match &self.overlay {
            Overlay::Eq(who) => who.clone(),
            _ => return None,
        };
        if let Some(rest) = id.strip_prefix("eqband:") {
            let index = rest.parse().unwrap_or(0);
            self.eq_band = index;
            self.drag = Some(id.to_string());
            if let Some((x, y)) = self.pointer {
                eq::drag_handle(self, &who, index, x, y);
            }
            return Some(Action::Levels);
        }
        if let Some(rest) = id.strip_prefix("eq-points:") {
            if let Ok(points) = rest.parse::<u8>() {
                eq::set_points(self, &who, points);
                return Some(Action::Save);
            }
        }
        if id == "eq-reset" {
            eq::reset(self, &who);
            return Some(Action::Save);
        }
        if id == "eq-bypass" {
            eq::toggle_bypass(self, &who);
            return Some(Action::Save);
        }
        if id == "eq-done" {
            self.overlay = Overlay::None;
            return Some(Action::Save);
        }
        if id == "eq-type" {
            eq::cycle_type(self, &who);
            return Some(Action::Save);
        }
        if id == "eq-on" {
            eq::toggle_band(self, &who);
            return Some(Action::Save);
        }
        None
    }

    fn fx_reorder(&mut self, y: f64) {
        let Some((bus, from)) = self.fx_drag else { return };
        let top = self.stage().check.1 + fx::CHAIN_TOP;
        let chain = &mut fx::bus_mut(&mut self.config, bus as usize).chain;
        if chain.len() < 2 || from >= chain.len() {
            return;
        }
        let row = 60.0;
        let index = ((y - top) / row).floor().clamp(0.0, (chain.len() - 1) as f64) as usize;
        if index == from {
            return;
        }
        let slot = chain.remove(from);
        chain.insert(index.min(chain.len()), slot);
        self.fx_drag = Some((bus, index.min(chain.len().saturating_sub(1))));
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

    pub fn render(&mut self, fonts: &Fonts, snap: &Snapshot, peaks: &HashMap<String, f32>) -> Vec<u8> {
        let mut surface = self.render_surface(fonts, snap, peaks);
        draw::surface_to_rgba(&mut surface)
    }

    pub fn pixel_size(&self) -> (i32, i32) {
        draw::panel_pixels(self.config.wide, false)
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
        let footer = (MARGIN, h - MARGIN - FOOTER_H, w - 2.0 * MARGIN, FOOTER_H);
        let bottom = footer.1 - GUTTER;
        let mix_top = CONTENT_TOP;
        let portrait = h > w && !self.config.list;
        let rail = (w - MARGIN - RAIL_W, CONTENT_TOP, RAIL_W, bottom - CONTENT_TOP);
        let ease = 1.0 - (1.0 - self.drawer_progress).powi(2);
        let drawer_w = self.drawer_span() * ease;
        let check = (rail.0 - GUTTER - drawer_w, CONTENT_TOP, drawer_w, bottom - CONTENT_TOP);
        let mix_right = if drawer_w > 8.0 { check.0 - GUTTER } else { rail.0 - GUTTER };
        let mix_w = (mix_right - MARGIN).max(0.0);
        let mix = (MARGIN, mix_top, mix_w, bottom - mix_top);
        Stage { w, portrait, footer, mix, check, rail }
    }

    /// Open width of the right column. A saved split follows the pointer; otherwise the
    /// 21:9 view gives Outputs a third of the screen and the regular view keeps 384.
    /// Either card stops at 20% of the window.
    fn drawer_span(&self) -> f64 {
        let (w, _) = self.size_f();
        let (min_w, span) = split_limits(w);
        let raw = if (0.0..1.0).contains(&self.config.split) && self.config.split > 0.0 {
            self.config.split * span
        } else if w <= draw::WIDTH as f64 + 1.0 {
            DRAWER_W
        } else {
            w / 3.0
        };
        raw.clamp(min_w, (span - min_w).max(min_w))
    }

    /// Grip in the gutter between the two sections. The hit is wider than the pill.
    fn split_handle(&mut self, cr: &Context) {
        let stage = self.stage();
        if stage.check.2 < 8.0 {
            return;
        }
        let gap_x = stage.mix.0 + stage.mix.2;
        let gap_w = stage.check.0 - gap_x;
        if gap_w < 4.0 {
            return;
        }
        let cx = gap_x + gap_w / 2.0;
        let hit_w = gap_w + 12.0;
        let hit_x = cx - hit_w / 2.0;
        let y = stage.mix.1;
        let h = stage.mix.3;
        let dragging = self.drag.as_deref() == Some(SPLIT);
        let hover = dragging || self.pointer.is_some_and(|(px, py)| px >= hit_x && px < hit_x + hit_w && py >= y && py < y + h);
        let grip_h = 56.0;
        let grip_w = 6.0;
        fill_round(cr, cx - grip_w / 2.0, y + (h - grip_h) / 2.0, grip_w, grip_h, 3.0, if hover { draw::accent() } else { draw::MUTED });
        self.hot(SPLIT, hit_x, y, hit_w, h);
    }

    /// The pointer sits in the gutter. The right column takes what remains up to the rail.
    fn set_split(&mut self, x: f64) {
        let (w, _) = self.size_f();
        let (min_w, span) = split_limits(w);
        if span <= 1.0 {
            return;
        }
        let rail_x = w - MARGIN - RAIL_W;
        let right = rail_x - GUTTER - (x + GUTTER / 2.0);
        let right = right.clamp(min_w, (span - min_w).max(min_w));
        self.config.split = right / span;
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
            && (snap.check_epoch != self.record_epoch || self.record_at.is_some_and(|start| start.elapsed() > Duration::from_secs(8)));
        if snap.check == CheckPhase::Recording || failed {
            self.record_pending = false;
            self.record_at = None;
        }
        self.seen_epoch = snap.check_epoch;
    }

    fn draw(&mut self, cr: &Context, fonts: &Fonts, snap: &Snapshot, peaks: &HashMap<String, f32>) {
        self.note_check(snap);
        draw::set_accent(&self.config.accent);
        self.hots.clear();
        self.faders.clear();
        self.hbar = None;
        self.strip = None;
        self.out_list = None;
        let (pw, ph) = self.size_f();
        fill_round(cr, 0.0, 0.0, pw, ph, 28.0, BG);
        let t = i18n::text(self.lang);
        self.header(cr, fonts, t);
        if self.page == Page::Apps {
            apps::draw(self, cr, fonts, t, snap);
        } else {
            mix::draw(self, cr, fonts, t, snap, peaks);
            match self.drawer {
                Drawer::Outputs => outputs::draw(self, cr, fonts, t, snap, peaks),
                Drawer::FxA => fx::draw(self, cr, fonts, t, 0, &outputs::present_a(&snap.graph)),
                Drawer::FxB => fx::draw(self, cr, fonts, t, 1, &outputs::present_a(&snap.graph)),
                Drawer::Closed => {}
            }
            self.split_handle(cr);
            rail::draw(self, cr);
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
            Overlay::Eq(_) => 8,
        };
        let confirm_key = if let Overlay::Confirm(key) = &self.overlay { key.clone() } else { String::new() };
        let eq_who = match &self.overlay {
            Overlay::Eq(who) => who.clone(),
            _ => String::new(),
        };
        match overlay {
            1 => dialogs::add(self, cr, fonts, t, snap),
            2 => dialogs::mic(self, cr, fonts, t, snap),
            3 => dialogs::settings(self, cr, fonts, t),
            4 => apps::list(self, cr, fonts, t, snap),
            5 => apps::confirm(self, cr, fonts, t, snap, &confirm_key),
            6 => dialogs::name(self, cr, fonts, t),
            8 => eq::sheet(self, cr, fonts, t, &eq_who),
            _ => {}
        }
    }

    fn header(&mut self, cr: &Context, fonts: &Fonts, t: &Text) {
        let mid = MARGIN + HEADER_H / 2.0;
        let logo = crate::brand::icon_png(self.config.accent == "pink", 128);
        draw::paint_png_bytes(cr, logo, MARGIN, mid - 18.0, 36.0, 36.0);
        draw::label_mid(cr, fonts, true, 24.0, TEXT, MARGIN + 50.0, mid, t.title);
        let (w, _) = self.size_f();
        let rect = (w - MARGIN - 200.0, mid - 24.0, 200.0, 48.0);
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
    fill_round(cr, 0.0, 0.0, w, h, 28.0, BG);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.45);
    let _ = cr.paint();
    panel.hot("dismiss", 0.0, 0.0, w, h);
}

/// A dialog surface: one tone above the cards, no outline.
pub(crate) fn sheet_surface(cr: &Context, x: f64, y: f64, w: f64, h: f64) {
    fill_round(cr, x, y, w, h, 24.0, draw::INSET);
}

/// Close button in a dialog's top-right corner, with a hover layer.
pub(crate) fn close_button(panel: &mut Panel, cr: &Context, x: f64, y: f64) {
    let s = 40.0;
    let look = panel.look(x, y, s, s);
    if look != Look::Rest {
        fill_round(cr, x, y, s, s, s / 2.0, if look == Look::Down { draw::CONTROL_DOWN } else { draw::CONTROL_HOVER });
    }
    draw::icon(cr, draw::CLOSE_PATH, 1024.0, x + 11.0, y + 11.0, 18.0, draw::SOFT);
    panel.hot("dismiss", x, y, s, s);
}

/// On/Off as a pill. The chosen half is filled with the accent.
pub(crate) fn pill_toggle(panel: &mut Panel, cr: &Context, fonts: &Fonts, x: f64, y: f64, w: f64, h: f64, on: bool, labels: (&str, &str), ids: (&str, &str)) {
    fill_round(cr, x, y, w, h, h / 2.0, draw::CONTROL);
    let half = w / 2.0;
    let pad = 4.0;
    let sel_x = if on { x + pad } else { x + half };
    fill_round(cr, sel_x, y + pad, half - pad, h - 2.0 * pad, (h - 2.0 * pad) / 2.0, draw::accent());
    for (index, (text, id)) in [(labels.0, ids.0), (labels.1, ids.1)].into_iter().enumerate() {
        let picked = (index == 0) == on;
        let cx = x + half * index as f64 + half / 2.0;
        let tw = draw::text_width(cr, fonts, true, 15.0, text);
        let color = if picked { draw::on_accent() } else { draw::SOFT };
        draw::label_mid(cr, fonts, true, 15.0, color, cx - tw / 2.0, y + h / 2.0, text);
        panel.hot(id, x + half * index as f64, y, half, h);
    }
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
    snap.has_take = true;
    snap.take_secs = 4.2;
    snap.apps = demo_apps();
    snap.update.state = crate::updater::UpdateState::UpToDate;
    snap.update.latest = crate::updater::VERSION.into();
    let peaks = HashMap::new();
    write_surface(&mut panel, fonts, &snap, &peaks, path)?;
    panel.config.input_list = true;
    write_surface(&mut panel, fonts, &snap, &peaks, &suffix(path, "input-rows"))?;
    panel.config.input_list = false;
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
    panel.overlay = Overlay::None;
    panel.page = Page::Mix;
    panel.scroll = f64::MAX;
    snap.check = CheckPhase::Recording;
    snap.check_secs = 3.0;
    let mut loud = HashMap::new();
    loud.insert("mic".to_string(), 0.4f32);
    loud.insert("check".to_string(), 0.5f32);
    for (key, level) in [("ch1:l", 0.6f32), ("ch1:r", 0.35), ("ch2:l", 0.2), ("ch2:r", 0.25), ("out:pc:l", 0.5), ("out:pc:r", 0.45), ("out:a:0:l", 0.3), ("out:a:0:r", 0.32)] {
        loud.insert(key.to_string(), level);
    }
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "scrolled"))?;
    snap.check = CheckPhase::Paused;
    snap.check_secs = 1.6;
    panel.config.b[0].name = "Discord".into();
    let devices = snap.graph.nodes.len();
    for (id, name, description) in [
        (900, "bluez_output.40_72_18_B4_2C_11.1", "WH-1000XM5"),
        (901, "alsa_output.usb-Generic_USB_Audio-00.analog-stereo", "USB Audio"),
    ] {
        snap.graph.nodes.push(crate::graph::AudioNode {
            id,
            serial: id as u64,
            name: name.into(),
            description: description.into(),
            media_class: "Audio/Sink".into(),
            media_name: String::new(),
            app_name: String::new(),
            binary: String::new(),
            is_virtual: false,
            target: String::new(),
        });
    }
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "eight"))?;
    let many = panel.config.channels.clone();
    panel.config.channels.truncate(1);
    panel.config.wide = true;
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "wide"))?;
    panel.config.list = true;
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "wide-columns"))?;
    panel.config.list = false;
    panel.config.channels = many;
    panel.config.wide = false;
    panel.out_scroll = 400.0;
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "eight-scrolled"))?;
    panel.out_scroll = 0.0;
    panel.check_open = true;
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "check-menu"))?;
    panel.check_open = false;
    panel.play_open = true;
    panel.config.check_play = "a:1".into();
    snap.check = CheckPhase::Playing;
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "play-menu"))?;
    panel.play_open = false;
    panel.config.check_play.clear();
    snap.check = CheckPhase::Paused;
    panel.config.list = true;
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "eight-rows"))?;
    panel.config.list = false;
    snap.graph.nodes.truncate(devices);
    snap.check = CheckPhase::Ready;
    panel.scroll = 0.0;
    panel.overlay = Overlay::Settings;
    panel.color_open = true;
    write_surface(&mut panel, fonts, &snap, &peaks, &suffix(path, "settings"))?;
    panel.overlay = Overlay::None;
    panel.language_open = true;
    write_surface(&mut panel, fonts, &snap, &peaks, &suffix(path, "lang"))?;
    panel.language_open = false;
    panel.overlay = Overlay::Mic;
    write_surface(&mut panel, fonts, &snap, &peaks, &suffix(path, "mic"))?;
    panel.overlay = Overlay::None;
    panel.config.list = true;
    panel.scroll = 0.0;
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "rows"))?;
    panel.scroll = 90.0;
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "rows-scrolled"))?;
    panel.scroll = 0.0;
    panel.config.accent = "pink".into();
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "rows-pink"))?;
    panel.config.list = false;
    panel.config.accent = "blue".into();
    panel.drawer = Drawer::FxA;
    panel.drawer_target = Drawer::FxA;
    panel.drawer_progress = 1.0;
    panel.config.fx_a.chain.push(crate::config::EffectSlot::new(crate::config::EffectKind::Reverb));
    panel.config.fx_a.chain.push(crate::config::EffectSlot::new(crate::config::EffectKind::Delay));
    panel.fx_edit = Some((0, 0));
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "fx"))?;
    let _ = panel.handle("fxadd:0:eq");
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "fx-eq"))?;
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
    fill_round(cr, x, y, size, size, size * 0.22, draw::accent_tint());
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
    fill_round(cr, x, y, size, size, size * 0.25, draw::accent_tint());
    let font = size * 0.5;
    let width = draw::text_width(cr, fonts, true, font, &letter);
    draw::label(cr, fonts, true, font, draw::accent(), x + (size - width) / 2.0, y + size * 0.68, &letter);
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
    // Minimal mode replaces the buffer with the newest letter. A read of only the
    // first letter is a truncated line, so it must not be appended again.
    if key.is_empty() && full_n == 1 && !draft.is_empty() && !draft.starts_with(full) && !draft.ends_with(full) {
        return truncate_name(&format!("{draft}{full}"));
    }
    if !key.is_empty() {
        if !full.is_empty() && full.starts_with(draft) && full_n >= draft_n {
            let key_n = key.chars().count();
            // Already in the line, or the 8-byte event field repeating the start of it.
            if full.ends_with(key) || (key_n < full_n && full.starts_with(key)) {
                return truncate_name(full);
            }
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
    if merge_keyboard("M", "Music", "") != "Music" {
        return Err("keyboard line grows".into());
    }
    if merge_keyboard("A", "B", "") != "AB" {
        return Err("keyboard newest letter".into());
    }
    if merge_keyboard("Music", "M", "") != "Music" {
        return Err("keyboard truncated line".into());
    }
    if merge_keyboard("Music", "Music", "M") != "Music" {
        return Err("keyboard event repeats the start".into());
    }
    let mut config = crate::config::Config::default();
    config.list = true;
    config.wide = true;
    let stage = Panel::new(config).stage();
    if stage.portrait || stage.check.0 <= stage.mix.0 + stage.mix.2 {
        return Err("list extend check on the right".into());
    }
    if (stage.rail.2 - 48.0).abs() > 0.1 {
        return Err("rail width".into());
    }
    let mut wide = crate::config::Config::default();
    wide.wide = true;
    wide.channels.push(crate::config::Channel::new("ch1", "Music"));
    let open = Panel::new(wide.clone()).stage();
    let third = open.w / 3.0;
    if (open.check.2 - third).abs() > 1.0 || open.mix.2 < third {
        return Err(format!("wide outputs {} of {}, mix {}", open.check.2, open.w, open.mix.2));
    }
    for n in 2..14 {
        wide.channels.push(crate::config::Channel::new(format!("ch{n}"), "More"));
    }
    let full = Panel::new(wide).stage();
    if (full.check.2 - third).abs() > 1.0 {
        return Err(format!("wide outputs stay a third {}", full.check.2));
    }
    let mut squeezed = crate::config::Config::default();
    squeezed.wide = true;
    squeezed.split = 0.05;
    let tight = Panel::new(squeezed).stage();
    let min = tight.w * 0.20;
    if tight.check.2 + 0.5 < min || tight.mix.2 + 0.5 < min {
        return Err(format!("split floor mix {} check {} min {}", tight.mix.2, tight.check.2, min));
    }
    let mut wide_side = crate::config::Config::default();
    wide_side.wide = true;
    wide_side.split = 0.95;
    let other = Panel::new(wide_side).stage();
    if other.mix.2 + 0.5 < min || other.check.2 + 0.5 < min {
        return Err(format!("split ceiling mix {} check {}", other.mix.2, other.check.2));
    }
    // The channel scroller used to panic once its track was shorter than the 48px thumb.
    if let Ok(fonts) = draw::Fonts::load() {
        let snap = Snapshot::default();
        let peaks = HashMap::new();
        for split in [0.05, 0.73, 0.95] {
            let mut panel = Panel::demo(Lang::En);
            panel.config.split = split;
            let _ = panel.render_surface(&fonts, &snap, &peaks);
            panel.config.input_list = true;
            let _ = panel.render_surface(&fonts, &snap, &peaks);
            panel.config.wide = true;
            panel.config.list = true;
            let _ = panel.render_surface(&fonts, &snap, &peaks);
        }
    }
    if (draw::scroll_thumb(41.8, 800.0, 48.0) - 41.8).abs() > 0.01 {
        return Err("short scrollbar thumb".into());
    }
    Ok(())
}
