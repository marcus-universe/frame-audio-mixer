//! Dashboard panel: mixer, Mix Check, language, and Apps & updates.

use crate::config::{self, Config, Pin};
use crate::draw::{self, Fonts, Rgb, ACCENT, BG, CARD, DANGER, HEIGHT, INSET, MUTED, SOFT, TEXT, WIDTH};
use crate::i18n::{self, Lang, Text, LANGS};
use crate::updates::{self, UpdateView};
use crate::worker::{CheckPhase, Snapshot};
use cairo::Context;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Mix,
    Apps,
}

enum Keyboard {
    Add,
    Rename(String),
}

struct Hot {
    id: String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

struct Fader {
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
    language_open: bool,
    quit_confirm: bool,
    selected: Option<String>,
    scroll: f64,
    drag: Option<String>,
    notice: String,
    keyboard: Option<Keyboard>,
    hots: Vec<Hot>,
    faders: Vec<Fader>,
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
    Open(String),
    Autostart(bool),
}

const SUGGESTIONS: [&str; 5] = ["SoundNinja", "Zen", "Moonlight", "Chromium", "WebCord"];

impl Panel {
    pub fn new(config: Config) -> Self {
        let lang = i18n::resolve(&config.language);
        Self {
            config,
            lang,
            page: Page::Mix,
            language_open: false,
            quit_confirm: false,
            selected: None,
            scroll: 0.0,
            drag: None,
            notice: String::new(),
            keyboard: None,
            hots: Vec::new(),
            faders: Vec::new(),
        }
    }

    pub fn demo(lang: Lang) -> Self {
        let mut config = Config::default();
        config.language = lang.code().into();
        config.mix_on = true;
        config.channels.push(config::Channel { id: "ch1".into(), name: "Music".into(), to_mic: 0.5, phones: 1.0 });
        config.channels.push(config::Channel { id: "ch2".into(), name: "Game".into(), to_mic: 0.8, phones: 0.6 });
        config.pins.push(Pin { app: "Zen".into(), channel: "ch1".into() });
        let mut panel = Self::new(config);
        panel.selected = Some("ch1".into());
        panel
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
        if config::name_taken(&self.config.channels, &name, match &which {
            Keyboard::Rename(id) => Some(id.as_str()),
            Keyboard::Add => None,
        }) {
            self.notice = i18n::text(self.lang).name_rejected.to_string();
            return Action::None;
        }
        match which {
            Keyboard::Add => {
                let Some(id) = config::next_channel_id(&self.config.channels) else { return Action::None };
                self.config.channels.push(config::Channel { id: id.clone(), name, to_mic: 0.5, phones: 1.0 });
                self.selected = Some(id);
            }
            Keyboard::Rename(id) => {
                if let Some(channel) = self.config.channels.iter_mut().find(|c| c.id == id) {
                    channel.name = name;
                }
            }
        }
        Action::Save
    }

    pub fn pointer(&mut self, kind: u32, x: f64, y: f64, button: u32) -> Action {
        if kind == crate::overlay::MOUSE_MOVE {
            if let Some(id) = self.drag.clone() {
                self.set_fader(&id, x, y);
            }
            return Action::None;
        }
        if kind == crate::overlay::MOUSE_UP {
            let save = self.drag.take().is_some();
            return if save { Action::Save } else { Action::None };
        }
        if kind == crate::overlay::SCROLL_DISCRETE || kind == crate::overlay::SCROLL_SMOOTH {
            self.scroll = (self.scroll - x).max(0.0);
            return Action::None;
        }
        if kind != crate::overlay::MOUSE_DOWN || !crate::overlay::is_left(button) {
            return Action::None;
        }
        if self.quit_confirm {
            return self.hit_at(x, y);
        }
        if self.language_open {
            let action = self.hit_at(x, y);
            if !matches!(action, Action::None) {
                return action;
            }
            self.language_open = false;
            return Action::None;
        }
        if let Some(id) = self.fader_at(x, y) {
            self.drag = Some(id.clone());
            self.set_fader(&id, x, y);
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
        if id == "quit-yes" {
            return Action::Quit;
        }
        if id == "quit-no" {
            self.quit_confirm = false;
            return Action::None;
        }
        if id == "quit" {
            self.quit_confirm = true;
            self.language_open = false;
            return Action::None;
        }
        if id == "lang" {
            self.language_open = !self.language_open;
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
        if id == "auto-on" || id == "auto-off" {
            return Action::Autostart(id == "auto-on");
        }
        if id == "apps" {
            self.page = Page::Apps;
            self.language_open = false;
            return Action::None;
        }
        if id == "back" {
            self.page = Page::Mix;
            return Action::None;
        }
        if id == "mix-on" || id == "mix-off" {
            self.config.mix_on = id == "mix-on";
            if !self.config.mix_on {
                self.notice.clear();
            }
            return Action::Save;
        }
        if id == "mic-mute" {
            self.config.mic_muted = !self.config.mic_muted;
            return Action::Save;
        }
        if id == "add" {
            if config::next_channel_id(&self.config.channels).is_none() {
                return Action::None;
            }
            self.keyboard = Some(Keyboard::Add);
            return Action::ShowKeyboard { description: t.channel_name.to_string(), existing: String::new() };
        }
        if let Some(channel) = id.strip_prefix("sel:") {
            self.selected = Some(channel.to_string());
            return Action::None;
        }
        if let Some(channel) = id.strip_prefix("rm:") {
            self.config.channels.retain(|item| item.id != channel);
            self.config.pins.retain(|pin| pin.channel != channel);
            if self.selected.as_deref() == Some(channel) {
                self.selected = None;
            }
            return Action::Save;
        }
        if let Some(channel) = id.strip_prefix("name:") {
            let existing = self.config.channels.iter().find(|c| c.id == channel).map(|c| c.name.clone()).unwrap_or_default();
            self.keyboard = Some(Keyboard::Rename(channel.to_string()));
            return Action::ShowKeyboard { description: t.channel_name.to_string(), existing };
        }
        if let Some(app) = id.strip_prefix("pin:") {
            let Some(channel) = self.selected.clone() else {
                self.notice = t.pick_channel.to_string();
                return Action::None;
            };
            self.config.pins.retain(|pin| pin.app != app);
            self.config.pins.push(Pin { app: app.to_string(), channel });
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
        if id == "upd-check" {
            self.config.check_updates = !self.config.check_updates;
            return Action::Save;
        }
        if id == "upd-now" {
            return Action::CheckUpdates;
        }
        if id == "upd-install" {
            return Action::InstallUpdate;
        }
        if let Some(url) = id.strip_prefix("open:") {
            return Action::Open(url.to_string());
        }
        Action::None
    }

    fn fader_at(&self, x: f64, y: f64) -> Option<String> {
        self.faders
            .iter()
            .rev()
            .find(|f| x >= f.x && x < f.x + f.w && y >= f.y && y < f.y + f.h)
            .map(|f| f.id.clone())
    }

    fn set_fader(&mut self, id: &str, _x: f64, y: f64) {
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

    pub fn render(&mut self, fonts: &Fonts, snap: &Snapshot) -> Vec<u8> {
        let mut surface = self.render_surface(fonts, snap);
        draw::surface_to_rgba(&mut surface)
    }

    pub fn render_surface(&mut self, fonts: &Fonts, snap: &Snapshot) -> cairo::ImageSurface {
        let surface = draw::surface().expect("surface");
        let cr = Context::new(&surface).expect("context");
        self.draw(&cr, fonts, snap);
        surface
    }

    fn draw(&mut self, cr: &Context, fonts: &Fonts, snap: &Snapshot) {
        self.hots.clear();
        self.faders.clear();
        rounded_fill(cr, 0.0, 0.0, WIDTH as f64, HEIGHT as f64, 24.0, BG);
        let t = i18n::text(self.lang);
        self.header(cr, fonts, t, snap);
        if self.page == Page::Apps {
            self.apps(cr, fonts, t, snap);
        } else {
            self.mixer(cr, fonts, t, snap);
            self.mix_check(cr, fonts, t, snap);
        }
        self.footer(cr, fonts, t, snap.autostart);
        if self.language_open {
            self.language_menu(cr, fonts);
        }
        if self.quit_confirm {
            self.confirm(cr, fonts, t);
        }
        if !self.notice.is_empty() {
            draw::label_fit(cr, fonts, false, 15.0, DANGER, 40.0, HEIGHT as f64 - 96.0, 700.0, &self.notice);
        }
    }

    pub fn thumbnail(&self, _fonts: &Fonts) -> Vec<u8> {
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 256, 256).expect("thumb");
        {
            let cr = Context::new(&surface).expect("context");
            rounded_fill(&cr, 0.0, 0.0, 256.0, 256.0, 48.0, CARD);
            draw::icon(&cr, draw::HEADPHONES_PATH, 16.0, 64.0, 64.0, 128.0, ACCENT);
        }
        draw::surface_to_rgba(&mut surface)
    }

    fn header(&mut self, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot) {
        draw::label(cr, fonts, true, 28.0, TEXT, 36.0, 58.0, t.title);
        let status = if snap.mix_active { t.status_on } else { t.status_off };
        draw::label_fit(cr, fonts, false, 15.0, MUTED, 110.0, 56.0, 620.0, status);
        if self.page == Page::Mix {
            let w = draw::text_width(cr, fonts, true, 16.0, t.apps) + 36.0;
            let x = WIDTH as f64 - 36.0 - w;
            draw::button(cr, fonts, x, 28.0, w, 40.0, t.apps, false);
            self.hot("apps", x, 28.0, w, 40.0);
        } else {
            draw::button(cr, fonts, WIDTH as f64 - 160.0, 28.0, 124.0, 40.0, t.back, false);
            self.hot("back", WIDTH as f64 - 160.0, 28.0, 124.0, 40.0);
        }
    }

    fn mixer(&mut self, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot) {
        draw::card(cr, 24.0, 88.0, 760.0, 560.0);
        draw::label(cr, fonts, true, 18.0, TEXT, 44.0, 124.0, "Mix");
        let on_x = 44.0;
        self.segment(cr, fonts, on_x, 140.0, 180.0, self.config.mix_on, t.on, t.off, "mix-on", "mix-off");
        let mic_name = if snap.graph.default_source.is_empty() {
            t.mic_default.to_string()
        } else {
            let desc = snap.graph.description_of(&snap.graph.default_source);
            if desc.is_empty() { t.mic_default.to_string() } else { desc }
        };
        self.column(cr, fonts, t, 44.0, &mic_name, true, "", self.config.mic_volume, 0.0, self.config.mic_muted);
        let mut x = 220.0 - self.scroll;
        for channel in self.config.channels.clone() {
            if x > 20.0 && x < 760.0 {
                let selected = self.selected.as_deref() == Some(channel.id.as_str());
                if selected {
                    rounded_stroke(cr, x - 8.0, 196.0, 176.0, 250.0, 16.0, ACCENT);
                }
                self.column(cr, fonts, t, x, &channel.name, false, &channel.id, channel.phones, channel.to_mic, false);
            }
            x += 188.0;
        }
        if x > 20.0 && x < 740.0 {
            draw::icon(cr, draw::ADD_PATH, 28.0, x + 28.0, 280.0, 36.0, ACCENT);
            draw::label(cr, fonts, true, 14.0, SOFT, x + 16.0, 340.0, t.add);
            self.hot("add", x, 260.0, 100.0, 100.0);
        }
        draw::label(cr, fonts, true, 16.0, TEXT, 44.0, 470.0, t.picker);
        if self.selected.is_none() {
            draw::label(cr, fonts, false, 14.0, MUTED, 44.0, 496.0, t.pick_channel);
        }
        let mut apps: Vec<String> = SUGGESTIONS.iter().map(|s| (*s).to_string()).collect();
        for node in snap.graph.playback_apps() {
            let name = if node.app_name.is_empty() { node.binary } else { node.app_name };
            if !name.is_empty() && !apps.contains(&name) {
                apps.push(name);
            }
        }
        let mut y = 510.0;
        for app in apps.into_iter().take(4) {
            let pinned = self.config.pins.iter().any(|pin| pin.app == app && Some(&pin.channel) == self.selected.as_ref());
            let caption = if pinned { format!("{app}  ·  {}", t.pinned) } else { app.clone() };
            draw::button(cr, fonts, 44.0, y, 320.0, 36.0, &caption, pinned);
            self.hot(&format!("pin:{app}"), 44.0, y, 320.0, 36.0);
            y += 44.0;
        }
    }

    fn column(
        &mut self,
        cr: &Context,
        fonts: &Fonts,
        t: &Text,
        x: f64,
        name: &str,
        mic: bool,
        id: &str,
        primary: f64,
        secondary: f64,
        muted: bool,
    ) {
        draw::label_fit(cr, fonts, true, 15.0, TEXT, x, 220.0, 150.0, name);
        if mic {
            let path = if muted { draw::MIC_OFF_PATH } else { draw::MIC_PATH };
            let view = if muted { 512.0 } else { 24.0 };
            draw::icon(cr, path, view, x, 232.0, 28.0, if muted { DANGER } else { SOFT });
            self.hot("mic-mute", x, 228.0, 36.0, 36.0);
            self.fader(cr, "mic", x + 48.0, 236.0, 28.0, 180.0, primary);
            draw::label(cr, fonts, false, 12.0, MUTED, x, 432.0, t.mic);
        } else {
            self.hot(&format!("sel:{id}"), x, 200.0, 150.0, 28.0);
            self.hot(&format!("name:{id}"), x, 200.0, 110.0, 28.0);
            draw::label(cr, fonts, true, 14.0, MUTED, x + 120.0, 220.0, "×");
            self.hot(&format!("rm:{id}"), x + 112.0, 200.0, 28.0, 28.0);
            draw::icon(cr, draw::HEADPHONES_PATH, 16.0, x, 236.0, 18.0, MUTED);
            self.fader(cr, &format!("phones:{id}"), x, 258.0, 22.0, 150.0, primary);
            draw::icon(cr, draw::SPEAKER_PATH, 56.0, x + 70.0, 236.0, 18.0, MUTED);
            self.fader(cr, &format!("tomic:{id}"), x + 70.0, 258.0, 22.0, 150.0, secondary);
            draw::label(cr, fonts, false, 11.0, MUTED, x, 424.0, t.phones);
            draw::label(cr, fonts, false, 11.0, MUTED, x + 70.0, 424.0, t.to_mic);
        }
    }

    fn fader(&mut self, cr: &Context, id: &str, x: f64, y: f64, w: f64, h: f64, value: f64) {
        rounded_fill(cr, x, y, w, h, 8.0, INSET);
        let fill_h = (value / 1.5).clamp(0.0, 1.0) * (h - 8.0);
        if fill_h > 1.0 {
            rounded_fill(cr, x + 4.0, y + h - 4.0 - fill_h, w - 8.0, fill_h, 5.0, ACCENT);
        }
        self.faders.push(Fader { id: id.to_string(), x, y, w, h });
    }

    fn mix_check(&mut self, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot) {
        draw::card(cr, 800.0, 88.0, 376.0, 560.0);
        draw::label(cr, fonts, true, 20.0, TEXT, 824.0, 128.0, t.mix_check);
        self.wrap(cr, fonts, 824.0, 156.0, 330.0, t.mix_check_body, MUTED);
        let recording = snap.check == CheckPhase::Recording;
        let y = 280.0;
        if recording {
            draw::icon(cr, draw::STOP_PATH, 24.0, 836.0, y + 8.0, 20.0, BG);
            draw::button(cr, fonts, 824.0, y, 160.0, 44.0, t.stop, true);
            self.hot("stop", 824.0, y, 160.0, 44.0);
        } else if self.config.mix_on {
            draw::button(cr, fonts, 824.0, y, 160.0, 44.0, t.record, true);
            draw::icon(cr, draw::RECORD_PATH, 24.0, 836.0, y + 12.0, 18.0, BG);
            self.hot("rec", 824.0, y, 160.0, 44.0);
        } else {
            draw::button(cr, fonts, 824.0, y, 160.0, 44.0, t.record, false);
            draw::label(cr, fonts, false, 13.0, MUTED, 824.0, y + 70.0, t.mix_check_need);
        }
        let can_play = snap.check == CheckPhase::Ready || snap.check == CheckPhase::Playing;
        draw::button(cr, fonts, 1000.0, y, 140.0, 44.0, t.play, can_play);
        self.hot("play", 1000.0, y, 140.0, 44.0);
        if speakers_without_echo(snap) {
            self.wrap(cr, fonts, 824.0, 360.0, 330.0, t.echo_hint, DANGER);
        }
        if !snap.message.is_empty() {
            draw::label_fit(cr, fonts, false, 13.0, DANGER, 824.0, 560.0, 330.0, &snap.message);
        }
    }

    fn apps(&mut self, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot) {
        draw::card(cr, 24.0, 88.0, 1152.0, 560.0);
        draw::label(cr, fonts, true, 22.0, TEXT, 48.0, 136.0, "Frame Audio Mixer");
        draw::label(cr, fonts, false, 16.0, MUTED, 48.0, 168.0, &format!("{} {}", t.installed, updates::VERSION));
        let check = if self.config.check_updates { t.on } else { t.off };
        draw::label(cr, fonts, false, 16.0, SOFT, 48.0, 214.0, t.apps_check);
        draw::button(cr, fonts, 360.0, 188.0, 100.0, 40.0, check, self.config.check_updates);
        self.hot("upd-check", 360.0, 188.0, 100.0, 40.0);
        draw::button(cr, fonts, 480.0, 188.0, 180.0, 40.0, t.apps_update, false);
        self.hot("upd-now", 480.0, 188.0, 180.0, 40.0);
        let line = update_line(t, &snap.update);
        draw::label_fit(cr, fonts, false, 15.0, MUTED, 48.0, 252.0, 700.0, &line);
        if snap.update.installable {
            draw::button(cr, fonts, 48.0, 270.0, 180.0, 40.0, t.apps_update, true);
            self.hot("upd-install", 48.0, 270.0, 180.0, 40.0);
        }
        draw::label(cr, fonts, true, 16.0, TEXT, 48.0, 360.0, t.apps_list);
        let tuner = if snap.tuner_version.is_empty() {
            t.not_installed.to_string()
        } else {
            format!("{} {}", t.installed, snap.tuner_version)
        };
        draw::label(cr, fonts, true, 16.0, SOFT, 48.0, 400.0, "Frame Mic Tuner");
        draw::label(cr, fonts, false, 15.0, MUTED, 280.0, 400.0, &tuner);
        draw::button(cr, fonts, 560.0, 374.0, 160.0, 36.0, "GitHub", false);
        self.hot("open:https://github.com/sasaken1102r/frame-mic-tuner", 560.0, 374.0, 160.0, 36.0);
    }

    fn footer(&mut self, cr: &Context, fonts: &Fonts, t: &Text, autostart: bool) {
        let y = 668.0;
        rounded_fill(cr, 24.0, y, WIDTH as f64 - 48.0, 96.0, 18.0, CARD);
        draw::icon(cr, draw::LANG_PATH, 16.0, 44.0, y + 22.0, 22.0, SOFT);
        let endonym = self.lang.endonym();
        draw::button(cr, fonts, 76.0, y + 16.0, 210.0, 40.0, endonym, false);
        self.hot("lang", 40.0, y + 12.0, 250.0, 48.0);
        draw::label_fit(cr, fonts, false, 15.0, SOFT, 310.0, y + 42.0, 280.0, t.footer_start);
        let on = if autostart { t.on } else { t.off };
        draw::button(cr, fonts, 600.0, y + 16.0, 88.0, 40.0, t.on, autostart);
        draw::button(cr, fonts, 696.0, y + 16.0, 88.0, 40.0, t.off, !autostart);
        self.hot("auto-on", 600.0, y + 16.0, 88.0, 40.0);
        self.hot("auto-off", 696.0, y + 16.0, 88.0, 40.0);
        let _ = on;
        draw::button(cr, fonts, WIDTH as f64 - 160.0, y + 16.0, 112.0, 40.0, t.footer_quit, false);
        self.hot("quit", WIDTH as f64 - 160.0, y + 16.0, 112.0, 40.0);
    }

    fn language_menu(&mut self, cr: &Context, fonts: &Fonts) {
        let x = 76.0;
        let h = LANGS.len() as f64 * 40.0 + 12.0;
        let y = 668.0 - h;
        rounded_fill(cr, x, y, 210.0, h, 12.0, INSET);
        for (i, lang) in LANGS.iter().enumerate() {
            let row = y + 6.0 + i as f64 * 40.0;
            if *lang == self.lang {
                rounded_fill(cr, x + 6.0, row, 198.0, 36.0, 8.0, CARD);
            }
            draw::label(cr, fonts, *lang == self.lang, 16.0, TEXT, x + 16.0, row + 24.0, lang.endonym());
            self.hot(&format!("lang:{}", lang.code()), x, row, 210.0, 40.0);
        }
    }

    fn confirm(&mut self, cr: &Context, fonts: &Fonts, t: &Text) {
        rounded_fill(cr, 300.0, 280.0, 600.0, 200.0, 18.0, CARD);
        draw::label(cr, fonts, true, 20.0, TEXT, 330.0, 340.0, t.quit_confirm);
        draw::button(cr, fonts, 360.0, 390.0, 160.0, 44.0, t.cancel, false);
        draw::button(cr, fonts, 560.0, 390.0, 160.0, 44.0, t.quit_yes, true);
        self.hot("quit-no", 360.0, 390.0, 160.0, 44.0);
        self.hot("quit-yes", 560.0, 390.0, 160.0, 44.0);
    }

    fn segment(&mut self, cr: &Context, fonts: &Fonts, x: f64, y: f64, w: f64, on: bool, yes: &str, no: &str, yes_id: &str, no_id: &str) {
        let half = w / 2.0;
        draw::button(cr, fonts, x, y, half - 4.0, 40.0, yes, on);
        draw::button(cr, fonts, x + half, y, half - 4.0, 40.0, no, !on);
        self.hot(yes_id, x, y, half - 4.0, 40.0);
        self.hot(no_id, x + half, y, half - 4.0, 40.0);
    }

    fn wrap(&self, cr: &Context, fonts: &Fonts, x: f64, mut y: f64, max_w: f64, text: &str, color: Rgb) {
        let mut line = String::new();
        for word in text.split_whitespace() {
            let next = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if draw::text_width(cr, fonts, false, 14.0, &next) > max_w && !line.is_empty() {
                draw::label(cr, fonts, false, 14.0, color, x, y, &line);
                y += 20.0;
                line = word.to_string();
            } else {
                line = next;
            }
        }
        if !line.is_empty() {
            draw::label(cr, fonts, false, 14.0, color, x, y, &line);
        }
    }

    fn hot(&mut self, id: &str, x: f64, y: f64, w: f64, h: f64) {
        self.hots.push(Hot { id: id.to_string(), x, y, w, h });
    }
}

fn update_line(t: &Text, update: &UpdateView) -> String {
    match update.status.as_str() {
        "update-available" => format!("{} {}", t.apps_available, update.latest),
        "up-to-date" => t.apps_current.to_string(),
        "" => String::new(),
        _ => {
            if update.message.is_empty() {
                update.status.clone()
            } else {
                update.message.clone()
            }
        }
    }
}

fn speakers_without_echo(snap: &Snapshot) -> bool {
    let sink = snap.graph.default_sink.to_lowercase();
    let speakers = sink.contains("speaker");
    speakers && snap.echo_cancel == Some(false)
}

fn rounded_fill(cr: &Context, x: f64, y: f64, w: f64, h: f64, r: f64, color: Rgb) {
    draw::rounded(cr, x, y, w, h, r);
    draw::fill(cr, color, 1.0);
}

fn rounded_stroke(cr: &Context, x: f64, y: f64, w: f64, h: f64, r: f64, color: Rgb) {
    draw::rounded(cr, x, y, w, h, r);
    cr.set_source_rgb(color.r, color.g, color.b);
    cr.set_line_width(2.0);
    let _ = cr.stroke();
}

pub fn render_png(fonts: &Fonts, lang: Lang, path: &std::path::Path) -> Result<(), String> {
    let mut panel = Panel::demo(lang);
    let surface = panel.render_surface(fonts, &Snapshot::default());
    let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    surface.write_to_png(&mut file).map_err(|e| e.to_string())
}
