//! Dashboard panel: the mix strip, Mix Check, dialogs, and Apps & updates.
//!
//! Geometry follows Frame Mic Tuner's Mix tab. The microphone chosen here is
//! only the mixer's input. It is never made the system default.
//!
//! How the module is laid out:
//! - this file: the panel's state, and what it tells the app to do ([`Action`]).
//! - [`ui`]: `Ui`, the painter every page draws with. It also records hit areas.
//! - [`hit`]: `Hit`, the typed name of everything that can be clicked or dragged.
//! - [`layout`]: where the cards sit for the current window size.
//! - [`frame`]: one frame, from the background to the open dropdown.
//! - [`input`]: pointer events, drags and faders.
//! - [`actions`]: what a click on each `Hit` does.
//! - [`naming`]: the name dialog and the headset keyboard.
//! - [`art`]: app icons, loaded once and cached.
//! - one file per page, drawer and dialog: `mix`, `outputs`, `fx`, `flow`, `apps`, ...
//! - [`demo`] and [`self_test`]: the screenshots and the checks behind `--self-test`.

mod actions;
mod apps;
mod art;
mod demo;
mod dialogs;
mod eq;
mod flow;
mod footer;
mod frame;
mod fx;
mod hit;
mod input;
mod layout;
mod menu;
mod mix;
mod naming;
mod outputs;
mod rail;
mod self_test;
mod spectrum;
mod ui;
mod vst;

pub use demo::render_png;
pub use self_test::self_test;

pub(crate) use hit::{BusId, EqSlider, FaderId, Hit, Pick, Source};
pub(crate) use layout::{CONTENT_TOP, GUTTER, MARGIN};
pub(crate) use ui::{HitBounds, Ui};

use crate::config::{self, Config};
use crate::draw::Rect;
use crate::graph::Graph;
use crate::i18n::{self, Lang, Text};
use hit::Drag;
use std::collections::HashMap;
use std::time::Instant;
use ui::{FaderArea, HotArea};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Page {
    Mix,
    Flow,
    Apps,
}

/// One effect in a chain: the bus (0 is FX1, 1 is FX2) and its position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FxSlot {
    pub bus: usize,
    pub index: usize,
}

#[derive(Clone)]
enum NameEdit {
    Add,
    /// A channel id.
    Rename(String),
    Bus(usize),
    /// Saving the current setup as a project.
    Project,
    /// The folder scanned for VST3 plugins.
    VstFolder,
}

enum Overlay {
    None,
    Add,
    Mic,
    Settings,
    AppsList,
    /// The app key the installer is about to run for.
    Confirm(String),
    Name(NameEdit),
    Eq(FxSlot),
    /// Saved routing setups: load, save, delete.
    Projects,
    /// Built-in effects and VST plugins, to add to one FX bus.
    FxPicker(usize),
    /// VST folder, scan, and the plugins found.
    Vst,
}

impl Overlay {
    /// Dialogs with a list that the vertical stick scrolls.
    fn scrolls(&self) -> bool {
        matches!(self, Overlay::Add | Overlay::Mic | Overlay::AppsList | Overlay::Projects | Overlay::FxPicker(_) | Overlay::Vst)
    }
}

/// Something that needs a second press within three seconds.
#[derive(Clone, PartialEq, Eq)]
enum Armed {
    /// A channel id.
    Channel(String),
    /// A row of the Projects dialog.
    Project(usize),
}

/// Scrollbar of the channel strip. `thumb_len` runs along the scroll axis.
#[derive(Clone, Copy)]
pub(crate) struct Scrollbar {
    /// The grab area, a little larger than the painted thumb.
    pub rect: Rect,
    pub thumb_len: f64,
    pub max_scroll: f64,
    pub vertical: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Drawer {
    Closed,
    FxA,
    FxB,
    Outputs,
}

const DRAWER_MS: f64 = 0.22;

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
    pub(crate) out_list: Option<Rect>,
    eq_band: usize,
    eq_graph: Option<Rect>,
    quit_until: Option<Instant>,
    delete_until: Option<(Armed, Instant)>,
    scroll: f64,
    dialog_scroll: f64,
    drag: Option<Drag>,
    pointer: Option<(f64, f64)>,
    pressed: bool,
    notice: String,
    /// Draft shown in the channel-name dialog.
    name_draft: String,
    name_rejected: bool,
    /// The dashboard keyboard is open for the name field.
    name_keyboard: bool,
    /// Hit areas of the last frame. Later ones lie on top.
    hots: Vec<HotArea>,
    faders: Vec<FaderArea>,
    pub(crate) scrollbar: Option<Scrollbar>,
    /// Channel strip from the last frame. Empty space here drag-scrolls.
    strip: Option<Rect>,
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
    /// The effect whose sliders are open in its drawer.
    pub(crate) fx_edit: Option<FxSlot>,
    /// The effect being dragged to a new place in its chain.
    fx_drag: Option<FxSlot>,
    /// Pan, zoom and selection of the Signal Flow page.
    flow: flow::FlowView,
    /// Saved projects, refreshed when the Projects dialog opens or changes them.
    projects: Vec<config::ProjectInfo>,
    /// The open dropdown and the button it belongs to.
    menu: Option<menu::Menu>,
    menu_anchor: Rect,
    spectrum: spectrum::SpectrumView,
    /// The last scan, and plugin host progress by instance, from the snapshot.
    pub(crate) vst_plugins: Vec<crate::vst::VstInfo>,
    pub(crate) vst_status: HashMap<String, String>,
    /// The session with Frame Audio Link on the PC, from the snapshot.
    pub(crate) link: frame_link_core::LinkStatus,
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
    ScanVst,
    /// Opens the editor of the VST with this instance id.
    OpenVst(String),
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
            scrollbar: None,
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
            flow: flow::FlowView::new(),
            projects: Vec::new(),
            menu: None,
            menu_anchor: Rect::default(),
            spectrum: spectrum::SpectrumView::new(),
            vst_plugins: Vec::new(),
            vst_status: HashMap::new(),
            link: frame_link_core::LinkStatus::default(),
        }
    }

    /// Whether the PC takes virtual microphone B`index` + 1 over the link.
    /// `None` while no PC is linked: the bus is then only a microphone on the headset.
    pub(crate) fn pc_takes_bus(&self, index: usize) -> Option<bool> {
        (self.config.pc_link && self.link.connected()).then(|| frame_link_core::mask_has(self.link.remote_recv, index))
    }

    /// Whether the PC is sending its output PC`index` + 1 over the link.
    pub(crate) fn pc_sends_input(&self, index: usize) -> bool {
        self.config.pc_link && self.link.connected() && frame_link_core::mask_has(self.link.remote_send, index)
    }

    /// Drop an armed Quit or delete, take the icon index when the scan finishes,
    /// and move the drawer animation on.
    pub fn tick(&mut self) {
        let now = Instant::now();
        if self.spectrum_target().is_none() && !self.spectrum.is_clear() {
            self.spectrum.clear();
        }
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
        self.animate_drawer(now);
    }

    fn animate_drawer(&mut self, now: Instant) {
        let dt = now.saturating_duration_since(self.anim_at).as_secs_f64().min(0.05);
        self.anim_at = now;
        let target = self.drawer_goal();
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

    /// Where the drawer animation is heading: 0 closed, 1 open.
    fn drawer_goal(&self) -> f64 {
        if self.drawer_target == Drawer::Closed { 0.0 } else { 1.0 }
    }

    pub fn animating(&self) -> bool {
        (self.drawer_progress - self.drawer_goal()).abs() > 0.001
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

    /// Opens `next`. Pressing the tab of the open drawer closes it.
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

    /// Effect bus and chain position of the open EQ, while its analyzer is on.
    pub fn spectrum_target(&self) -> Option<(usize, usize)> {
        let Overlay::Eq(slot) = self.overlay else { return None };
        if self.config.eq_view == config::EqView::Off {
            return None;
        }
        Some((slot.bus, slot.index))
    }

    pub fn ingest_spectrum(&mut self, bands: &[f32]) {
        self.spectrum.ingest(bands, self.config.eq_view);
    }

    pub fn interacting(&self) -> bool {
        self.drag.is_some()
    }

    pub(crate) fn quit_armed(&self) -> bool {
        self.quit_until.is_some_and(|until| Instant::now() <= until)
    }

    fn armed(&self, what: &Armed) -> bool {
        self.delete_until.as_ref().is_some_and(|(armed, until)| armed == what && Instant::now() <= *until)
    }

    /// The channel with this id was pressed for removal and waits for the second press.
    pub(crate) fn channel_armed(&self, id: &str) -> bool {
        self.delete_until.as_ref().is_some_and(|(armed, until)| matches!(armed, Armed::Channel(armed) if armed == id) && Instant::now() <= *until)
    }

    pub(crate) fn project_armed(&self, index: usize) -> bool {
        self.armed(&Armed::Project(index))
    }

    /// Arms `what` for three seconds. True when it was armed already, so the caller goes ahead.
    fn confirm(&mut self, what: Armed) -> bool {
        if self.armed(&what) {
            self.delete_until = None;
            return true;
        }
        self.delete_until = Some((what, Instant::now() + std::time::Duration::from_secs(3)));
        false
    }

    /// The channel list changed, so the positions in last frame's hit areas are stale.
    fn channels_changed(&mut self) {
        self.hots.clear();
        self.faders.clear();
    }
}

pub(crate) fn resolved_source(choice: &str, graph: &Graph) -> Option<String> {
    let wanted = if choice == "builtin" {
        Graph::BUILTIN_SOURCE.to_string()
    } else if choice.is_empty() {
        Graph::mixer_default_source(&graph.default_source).to_string()
    } else {
        choice.to_string()
    };
    if live_source(graph, &wanted) {
        return Some(wanted);
    }
    if (choice.is_empty() || choice == "builtin") && live_source(graph, Graph::BUILTIN_SOURCE) {
        return Some(Graph::BUILTIN_SOURCE.to_string());
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
        let desc = graph.description_of(Graph::mixer_default_source(&graph.default_source));
        if desc.is_empty() { t.mic_system.to_string() } else { format!("{} · {desc}", t.mic_system) }
    } else {
        let desc = graph.description_of(choice);
        if desc.is_empty() { choice.to_string() } else { desc }
    }
}

/// `template` with `{key}` replaced by `value`.
pub(crate) fn fmt_token(template: &str, key: &str, value: &str) -> String {
    template.replace(&format!("{{{key}}}"), value)
}
