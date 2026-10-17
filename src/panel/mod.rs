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
use crate::draw::{self, ButtonKind, Fonts, Look, Rgb, ACCENT, BG, CARD, HEIGHT, TEXT, WIDTH};
use crate::graph::Graph;
use crate::i18n::{self, Lang, Text, LANGS};
use crate::worker::{CheckPhase, Snapshot};
use cairo::Context;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Mix,
    Apps,
}

enum Keyboard {
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
}

pub struct Panel {
    pub config: Config,
    lang: Lang,
    page: Page,
    overlay: Overlay,
    language_open: bool,
    quit_until: Option<Instant>,
    delete_until: Option<(String, Instant)>,
    scroll: f64,
    dialog_scroll: f64,
    drag: Option<String>,
    pointer: Option<(f64, f64)>,
    pressed: bool,
    notice: String,
    keyboard: Option<Keyboard>,
    hots: Vec<Hot>,
    faders: Vec<FaderHit>,
    icons: Option<crate::icons::IconIndex>,
    icon_rx: Option<std::sync::mpsc::Receiver<crate::icons::IconIndex>>,
    pngs: HashMap<String, cairo::ImageSurface>,
}

pub enum Action {
    None,
    Save,
    Quit,
    ShowKeyboard { description: String, existing: String },
    Record,
    StopCheck,
    PlayCheck,
    CheckUpdates,
    InstallUpdate,
    OpenInstaller(String),
    Autostart(bool),
}

pub const SUGGESTIONS: [&str; 5] = ["SoundNinja", "Zen", "Moonlight", "Chromium", "WebCord"];

impl Panel {
    pub fn new(config: Config) -> Self {
        let lang = i18n::resolve(&config.language);
        Self {
            config,
            lang,
            page: Page::Mix,
            overlay: Overlay::None,
            language_open: false,
            quit_until: None,
            delete_until: None,
            scroll: 0.0,
            dialog_scroll: 0.0,
            drag: None,
            pointer: None,
            pressed: false,
            notice: String::new(),
            keyboard: None,
            hots: Vec::new(),
            faders: Vec::new(),
            icons: None,
            icon_rx: None,
            pngs: HashMap::new(),
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
        config.pins.push(Pin { app: "Zen".into(), channel: "ch1".into() });
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
        if let Some(rx) = &self.icon_rx {
            if let Ok(index) = rx.try_recv() {
                self.icons = Some(index);
                self.icon_rx = None;
            }
        }
    }

    pub fn wants_meters(&self) -> bool {
        self.config.meters && self.config.mix_on && matches!(self.page, Page::Mix) && matches!(self.overlay, Overlay::None)
    }

    /// PipeWire nodes to meter. The mic tap is not passive. Channel taps are.
    pub fn meter_targets(&self, snap: &Snapshot) -> Vec<crate::meter::Target> {
        if !self.wants_meters() {
            return Vec::new();
        }
        let mut targets = Vec::new();
        if let Some(object) = resolved_source(&self.config.mic_source, &snap.graph) {
            targets.push(crate::meter::Target { key: "mic".into(), object, capture_sink: false, passive: false });
        }
        for channel in &self.config.channels {
            targets.push(crate::meter::Target {
                key: channel.id.clone(),
                object: format!("frame_amix_{}", channel.id),
                capture_sink: true,
                passive: true,
            });
        }
        targets
    }

    pub fn on_keyboard(&mut self, text: &str) -> Action {
        let Some(which) = self.keyboard.take() else { return Action::None };
        let name = match config::normalize_name(text) {
            Ok(name) => name,
            Err(_) => {
                self.notice = i18n::text(self.lang).name_rejected.to_string();
                return Action::None;
            }
        };
        let except = match &which {
            Keyboard::Rename(id) => Some(id.as_str()),
            Keyboard::Add => None,
        };
        if config::name_taken(&self.config.channels, &name, except) {
            self.notice = i18n::text(self.lang).name_rejected.to_string();
            return Action::None;
        }
        match which {
            Keyboard::Add => return self.add_channel(name, None),
            Keyboard::Rename(id) => {
                if let Some(channel) = self.config.channels.iter_mut().find(|c| c.id == id) {
                    channel.name = name;
                }
            }
        }
        Action::Save
    }

    fn add_channel(&mut self, name: String, pin_app: Option<String>) -> Action {
        if let Some(existing) = self.config.channels.iter().find(|c| c.name.eq_ignore_ascii_case(&name)) {
            if let Some(app) = pin_app {
                self.pin(app, existing.id.clone());
            }
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
        self.overlay = Overlay::None;
        Action::Save
    }

    fn pin(&mut self, app: String, channel: String) {
        self.config.pins.retain(|pin| pin.app != app);
        self.config.pins.push(Pin { app, channel });
    }

    pub fn pointer(&mut self, kind: u32, x: f64, y: f64, button: u32) -> Action {
        if kind == crate::overlay::MOUSE_MOVE {
            self.pointer = Some((x, y));
            if let Some(id) = self.drag.clone() {
                self.set_fader(&id, y);
            }
            return Action::None;
        }
        if kind == crate::overlay::MOUSE_UP {
            self.pressed = false;
            let save = self.drag.take().is_some();
            return if save { Action::Save } else { Action::None };
        }
        if kind == crate::overlay::SCROLL_DISCRETE || kind == crate::overlay::SCROLL_SMOOTH {
            let delta = if y.abs() > 0.01 && y.abs() < 400.0 { y } else { x };
            if matches!(self.overlay, Overlay::Add | Overlay::Mic | Overlay::AppsList) {
                self.dialog_scroll = (self.dialog_scroll - delta).max(0.0);
            } else {
                self.scroll = (self.scroll - delta).max(0.0);
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
        if let Some(id) = self.fader_at(x, y) {
            self.drag = Some(id.clone());
            self.set_fader(&id, y);
            return Action::None;
        }
        self.hit_at(x, y)
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
            self.dialog_scroll = 0.0;
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
            return Action::None;
        }
        if id == "meters-on" || id == "meters-off" {
            self.config.meters = id == "meters-on";
            return Action::Save;
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
            self.keyboard = Some(Keyboard::Add);
            self.overlay = Overlay::None;
            return Action::ShowKeyboard { description: t.channel_name.to_string(), existing: String::new() };
        }
        if let Some(app) = id.strip_prefix("use:") {
            return self.add_channel(app.to_string(), Some(app.to_string()));
        }
        if let Some(name) = id.strip_prefix("suggest:") {
            return self.add_channel(name.to_string(), None);
        }
        if let Some(channel) = id.strip_prefix("name:") {
            let existing = self.config.channels.iter().find(|c| c.id == channel).map(|c| c.name.clone()).unwrap_or_default();
            self.keyboard = Some(Keyboard::Rename(channel.to_string()));
            return Action::ShowKeyboard { description: t.channel_name.to_string(), existing };
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
            return Action::Record;
        }
        if id == "stop" {
            return Action::StopCheck;
        }
        if id == "play" {
            return Action::PlayCheck;
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

    fn set_fader(&mut self, id: &str, y: f64) {
        let Some(fader) = self.faders.iter().find(|f| f.id == id) else { return };
        let t = (1.0 - ((y - fader.y) / fader.h)).clamp(0.0, 1.0);
        let value = config::clamp_volume(t * 1.5);
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

    pub fn render(&mut self, fonts: &Fonts, snap: &Snapshot, peaks: &HashMap<String, f32>) -> Vec<u8> {
        let mut surface = self.render_surface(fonts, snap, peaks);
        draw::surface_to_rgba(&mut surface)
    }

    pub fn render_surface(&mut self, fonts: &Fonts, snap: &Snapshot, peaks: &HashMap<String, f32>) -> cairo::ImageSurface {
        let surface = draw::surface().expect("surface");
        let cr = Context::new(&surface).expect("context");
        self.draw(&cr, fonts, snap, peaks);
        surface
    }

    fn draw(&mut self, cr: &Context, fonts: &Fonts, snap: &Snapshot, peaks: &HashMap<String, f32>) {
        self.hots.clear();
        self.faders.clear();
        fill_round(cr, 0.0, 0.0, WIDTH as f64, HEIGHT as f64, 24.0, BG);
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
        };
        let confirm_key = if let Overlay::Confirm(key) = &self.overlay { key.clone() } else { String::new() };
        match overlay {
            1 => dialogs::add(self, cr, fonts, t, snap),
            2 => dialogs::mic(self, cr, fonts, t, snap),
            3 => dialogs::settings(self, cr, fonts, t),
            4 => apps::list(self, cr, fonts, t, snap),
            5 => apps::confirm(self, cr, fonts, t, snap, &confirm_key),
            _ => {}
        }
    }

    fn header(&mut self, cr: &Context, fonts: &Fonts, t: &Text) {
        draw::label(cr, fonts, true, 28.0, TEXT, 36.0, 58.0, t.title);
        let rect = (964.0, 22.0, 208.0, 56.0);
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
            fill_round(&cr, 0.0, 0.0, 256.0, 256.0, 48.0, CARD);
            draw::icon(&cr, draw::HEADPHONES_PATH, 16.0, 64.0, 64.0, 128.0, ACCENT);
        }
        draw::surface_to_rgba(&mut surface)
    }

    pub(crate) fn hot(&mut self, id: &str, x: f64, y: f64, w: f64, h: f64) {
        self.hots.push(Hot { id: id.to_string(), x, y, w, h });
    }

    pub(crate) fn fader_hit(&mut self, id: &str, x: f64, y: f64, w: f64, h: f64) {
        self.faders.push(FaderHit { id: id.to_string(), x, y, w, h });
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
        graph.default_source.clone()
    } else {
        choice.to_string()
    };
    if !wanted.is_empty() && wanted != "frame_amix_source" && graph.node_by_name(&wanted).is_some() {
        Some(wanted)
    } else if !graph.default_source.is_empty() && graph.default_source != "frame_amix_source" {
        Some(graph.default_source.clone())
    } else {
        None
    }
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
    fill_round(cr, 0.0, 0.0, WIDTH as f64, HEIGHT as f64, 24.0, BG);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.45);
    let _ = cr.paint();
    panel.hot("dismiss", 0.0, 0.0, WIDTH as f64, HEIGHT as f64);
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
    draw::label(cr, fonts, true, size * 0.28, ACCENT, x + (size - width) / 2.0, y + size * 0.62, &letter);
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
    fill_round(cr, x, y, size, size, 8.0, draw::CONTROL);
    let width = draw::text_width(cr, fonts, true, 16.0, &letter);
    draw::label(cr, fonts, true, 16.0, TEXT, x + (size - width) / 2.0, y + size * 0.68, &letter);
}
