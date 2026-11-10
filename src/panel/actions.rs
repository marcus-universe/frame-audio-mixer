//! What a click does. [`Panel::handle`] is the one place that maps a [`Hit`] to
//! a change of the panel's state, the config, or an [`Action`] for the app.

use super::hit::{BusId, Drag, Hit, Pick, Source};
use super::{eq, fx, Action, Armed, FxSlot, NameEdit, Overlay, Page, Panel};
use crate::config::{self, EffectKind, EffectSlot, EqView, ParamEq, Pin};
use crate::fx::presets;
use crate::i18n;
use std::time::{Duration, Instant};

/// An effect chain holds at most this many effects.
const CHAIN_MAX: usize = 3;

impl Panel {
    pub(super) fn handle(&mut self, hit: Hit) -> Action {
        match hit {
            Hit::Noop | Hit::PickMenuSurface(_) => Action::None,
            Hit::Dismiss => self.dismiss(),

            // Header and footer.
            Hit::Page(page) => {
                self.page = page;
                self.language_open = false;
                self.overlay = Overlay::None;
                Action::None
            }
            Hit::Back => {
                self.page = Page::Mix;
                self.overlay = Overlay::None;
                Action::None
            }
            Hit::Wide(on) => {
                self.config.wide = on;
                Action::Save
            }
            Hit::Projects => {
                self.projects = config::list_projects();
                self.language_open = false;
                self.open(Overlay::Projects)
            }
            Hit::LanguageMenu => {
                self.language_open = !self.language_open;
                self.overlay = Overlay::None;
                Action::None
            }
            Hit::Language(lang) => {
                self.lang = lang;
                self.config.language = lang.code().into();
                self.language_open = false;
                Action::Save
            }
            Hit::Settings => {
                self.overlay = Overlay::Settings;
                self.language_open = false;
                self.color_open = false;
                Action::None
            }
            Hit::Autostart(on) => Action::Autostart(on),
            Hit::Quit => {
                if self.quit_armed() {
                    return Action::Quit;
                }
                self.quit_until = Some(Instant::now() + Duration::from_secs(3));
                self.language_open = false;
                Action::None
            }

            // Settings dialog.
            Hit::Meters(on) => {
                self.config.meters = on;
                Action::Save
            }
            Hit::AccentMenu => {
                self.color_open = !self.color_open;
                Action::None
            }
            Hit::Accent(name) => {
                self.config.accent = name.to_string();
                self.color_open = false;
                Action::Save
            }
            Hit::VstSettings => self.open(Overlay::Vst),

            // Mixer card.
            Hit::Snap(on) => {
                self.config.snap = on;
                Action::Save
            }
            Hit::InputRows(on) => {
                self.config.input_list = on;
                self.scroll = 0.0;
                Action::Save
            }
            Hit::OutputColumns(on) => {
                self.config.list = on;
                self.out_scroll = 0.0;
                Action::Save
            }
            Hit::Split => {
                self.drag = Some(Drag::Split);
                if let Some((x, _)) = self.pointer {
                    self.set_split(x);
                }
                Action::None
            }
            Hit::Drawer(drawer) => {
                self.set_drawer(drawer);
                Action::None
            }
            Hit::MicPicker => self.open(Overlay::Mic),
            Hit::MicSource(source) => {
                self.config.mic_source = source;
                self.overlay = Overlay::None;
                Action::Save
            }
            Hit::AddChannel => {
                if config::next_channel_id(&self.config.channels).is_none() {
                    return Action::None;
                }
                self.pin_app = None;
                if self.icon_rx.is_none() && self.icons.is_none() {
                    self.icon_rx = Some(crate::icons::spawn());
                }
                self.open(Overlay::Add)
            }
            Hit::RenameChannel(index) => match self.config.channels.get(index) {
                Some(channel) => {
                    let (id, name) = (channel.id.clone(), channel.name.clone());
                    self.begin_name(NameEdit::Rename(id), name)
                }
                None => Action::None,
            },
            Hit::RemoveChannel(index) => self.remove_channel(index),
            Hit::Mute(source) => {
                self.toggle_mute(source);
                Action::Save
            }
            Hit::Route { from, to } => {
                self.toggle_route(from, to);
                Action::Save
            }
            Hit::Send { from, fx } => {
                self.toggle_send(from, fx);
                Action::Save
            }

            // Add-channel dialog.
            Hit::RefreshApps => {
                self.icons = None;
                self.icon_rx = Some(crate::icons::spawn());
                Action::None
            }
            Hit::CustomName => self.begin_name(NameEdit::Add, String::new()),
            Hit::UseApp(app) => self.add_app_channel(&app),
            Hit::Suggest(name) => self.add_channel(name.to_string(), None),

            // Name dialog.
            Hit::NameField => self.show_keyboard(),
            Hit::NameOk => self.commit_name(),

            // Outputs drawer and Mix Check.
            Hit::BusMute(bus) => {
                self.toggle_bus_mute(bus);
                Action::Save
            }
            Hit::BusRename(index) => match self.config.b.get(index) {
                Some(bus) => {
                    let name = bus.name.clone();
                    self.begin_name(NameEdit::Bus(index), name)
                }
                None => Action::None,
            },
            Hit::Record => {
                self.record_pending = true;
                self.record_epoch = self.seen_epoch;
                self.record_at = Some(Instant::now());
                Action::Record
            }
            Hit::StopCheck => {
                self.record_pending = false;
                self.record_at = None;
                Action::StopCheck
            }
            Hit::PlayTake => Action::PlayTake,
            Hit::PauseTake => Action::PauseTake,
            Hit::PickMenu(Pick::Record) => {
                self.check_open = !self.check_open;
                self.play_open = false;
                Action::None
            }
            Hit::PickMenu(Pick::Play) => {
                self.play_open = !self.play_open;
                self.check_open = false;
                Action::None
            }
            Hit::PickBus(Pick::Record, bus) => {
                self.check_open = false;
                let value = bus.value();
                if config::check_output_valid(&value) && self.config.check_output != value {
                    self.config.check_output = value;
                    return Action::Save;
                }
                Action::None
            }
            Hit::PickBus(Pick::Play, bus) => {
                self.play_open = false;
                let value = bus.value();
                if config::check_play_valid(&value) && self.config.check_play != value {
                    self.config.check_play = value;
                    return Action::Save;
                }
                Action::None
            }

            // Effect drawers.
            Hit::FxPicker(bus) => self.open(Overlay::FxPicker(bus.min(1))),
            Hit::FxAdd { bus, kind } => self.add_effect(bus, kind),
            Hit::FxAddVst { bus, plugin } => self.add_vst(bus, plugin),
            Hit::FxRemove { bus, index } => {
                let chain = &mut fx::bus_mut(&mut self.config, bus).chain;
                if index < chain.len() {
                    chain.remove(index);
                }
                self.fx_edit = None;
                Action::Save
            }
            Hit::FxEdit { bus, index } => {
                let slot = FxSlot { bus, index };
                let is_eq = fx::bus_mut(&mut self.config, bus).chain.get(index).is_some_and(|slot| slot.kind == EffectKind::Eq);
                if is_eq {
                    self.open_eq(slot);
                } else {
                    self.fx_edit = if self.fx_edit == Some(slot) { None } else { Some(slot) };
                }
                Action::None
            }
            Hit::FxPitchSnap { bus, index } => {
                if let Some(slot) = fx::bus_mut(&mut self.config, bus).chain.get_mut(index) {
                    if slot.kind == EffectKind::Pitcher {
                        slot.pitch_snap = !slot.pitch_snap;
                        if slot.pitch_snap {
                            slot.params[0] = config::pitch_snapped(slot.params[0]);
                        }
                    }
                }
                Action::Save
            }
            Hit::FxGrip { bus, index } => {
                self.fx_drag = Some(FxSlot { bus, index });
                self.drag = Some(Drag::FxGrip);
                Action::None
            }
            Hit::VstEditor(instance) => Action::OpenVst(instance),
            Hit::VstFolder => {
                let folder = self.config.vst_folder.clone();
                self.begin_name(NameEdit::VstFolder, folder)
            }
            Hit::VstScan => Action::ScanVst,

            // Dropdown menus.
            Hit::OpenMenu(menu) => {
                self.menu = Some(menu);
                Action::None
            }
            Hit::MenuClose => {
                self.menu = None;
                Action::None
            }
            Hit::PickEqView(row) => {
                self.menu = None;
                let Some(view) = EqView::ALL.get(row) else { return Action::None };
                self.config.eq_view = *view;
                self.spectrum.clear();
                Action::Save
            }
            Hit::PickPreset { bus, index, row } => {
                self.menu = None;
                let Some(slot) = fx::bus_mut(&mut self.config, bus).chain.get_mut(index) else { return Action::None };
                let Some(preset) = presets::of(slot.kind).get(row) else { return Action::None };
                preset.apply(slot);
                Action::Save
            }

            // EQ sheet. These only exist while it is open.
            Hit::EqBand(_) | Hit::EqPoints(_) | Hit::EqReset | Hit::EqBypass | Hit::EqDone | Hit::EqType | Hit::EqBandOn => {
                let Overlay::Eq(slot) = self.overlay else { return Action::None };
                self.handle_eq(slot, hit)
            }

            // Projects dialog.
            Hit::ProjectSave => self.begin_name(NameEdit::Project, String::new()),
            Hit::ProjectLoad(index) => {
                let Some(info) = self.projects.get(index) else { return Action::None };
                let Some(config) = config::load_project(&info.path, &self.config) else { return Action::None };
                self.config = config;
                self.overlay = Overlay::None;
                self.scroll = 0.0;
                self.channels_changed();
                Action::Save
            }
            Hit::ProjectRemove(index) => {
                if self.confirm(Armed::Project(index)) {
                    if let Some(info) = self.projects.get(index) {
                        config::delete_project(&info.path);
                    }
                    self.projects = config::list_projects();
                }
                Action::None
            }

            // Signal Flow page.
            Hit::FlowApps => {
                self.config.flow_apps = !self.config.flow_apps;
                Action::Save
            }
            Hit::FlowNode(key) => {
                self.flow.sel = if self.flow.sel.as_deref() == Some(key.as_str()) { None } else { Some(key) };
                Action::None
            }
            Hit::FlowZoomIn => {
                self.flow.zoom_by(1.25);
                Action::None
            }
            Hit::FlowZoomOut => {
                self.flow.zoom_by(1.0 / 1.25);
                Action::None
            }
            Hit::FlowFit => {
                self.flow.fit();
                Action::None
            }

            // Apps & updates page.
            Hit::CheckUpdates(on) => {
                self.config.check_updates = on;
                Action::Save
            }
            Hit::CheckUpdatesNow => Action::CheckUpdates,
            Hit::InstallUpdate => Action::InstallUpdate,
            Hit::AppsList => self.open(Overlay::AppsList),
            Hit::AppInstall(key) => {
                self.overlay = Overlay::Confirm(key);
                Action::None
            }
            Hit::ConfirmNo => {
                self.overlay = if self.page == Page::Apps { Overlay::AppsList } else { Overlay::None };
                Action::None
            }
            Hit::ConfirmYes(key) => {
                self.overlay = Overlay::AppsList;
                Action::OpenInstaller(key)
            }
        }
    }

    /// Opens a dialog with its list scrolled to the top.
    fn open(&mut self, overlay: Overlay) -> Action {
        self.overlay = overlay;
        self.dialog_scroll = 0.0;
        Action::None
    }

    fn dismiss(&mut self) -> Action {
        self.overlay = Overlay::None;
        self.pin_app = None;
        self.color_open = false;
        self.dialog_scroll = 0.0;
        self.name_draft.clear();
        self.name_rejected = false;
        self.name_keyboard = false;
        self.flow.sel = None;
        Action::None
    }

    fn handle_eq(&mut self, slot: FxSlot, hit: Hit) -> Action {
        match hit {
            Hit::EqBand(index) => {
                self.eq_band = index;
                self.drag = Some(Drag::EqBand(index));
                if let Some((x, y)) = self.pointer {
                    eq::drag_handle(self, slot, index, x, y);
                }
                return Action::Levels;
            }
            Hit::EqPoints(points) => eq::set_points(self, slot, points),
            Hit::EqReset => eq::reset(self, slot),
            Hit::EqBypass => eq::toggle_bypass(self, slot),
            Hit::EqDone => self.overlay = Overlay::None,
            Hit::EqType => eq::cycle_type(self, slot),
            Hit::EqBandOn => eq::toggle_band(self, slot),
            _ => return Action::None,
        }
        Action::Save
    }

    /// Opens the EQ sheet of `slot` on its first band.
    fn open_eq(&mut self, slot: FxSlot) {
        self.fx_edit = None;
        self.eq_band = 0;
        self.overlay = Overlay::Eq(slot);
    }
}

/// Channels.
impl Panel {
    pub(super) fn add_channel(&mut self, name: String, pin_app: Option<String>) -> Action {
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
        self.channels_changed();
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

    /// The first press arms the ×, the second removes the channel and its pins.
    fn remove_channel(&mut self, index: usize) -> Action {
        let Some(id) = self.config.channels.get(index).map(|channel| channel.id.clone()) else { return Action::None };
        if !self.confirm(Armed::Channel(id.clone())) {
            return Action::None;
        }
        self.config.channels.retain(|channel| channel.id != id);
        self.config.pins.retain(|pin| pin.channel != id);
        self.channels_changed();
        Action::Save
    }
}

/// Routing and mutes.
impl Panel {
    fn toggle_mute(&mut self, source: Source) {
        match source {
            Source::Mic => self.config.mic_muted = !self.config.mic_muted,
            Source::Channel(index) => {
                if let Some(channel) = self.config.channels.get_mut(index) {
                    channel.muted = !channel.muted;
                }
            }
            Source::Fx(_) => {}
        }
    }

    fn toggle_route(&mut self, from: Source, to: BusId) {
        let config = &mut self.config;
        let (to_pc, routes) = match from {
            Source::Mic => (&mut config.mic_to_pc, &mut config.mic_routes),
            Source::Fx(bus) => {
                let fx = fx::bus_mut(config, bus);
                (&mut fx.to_pc, &mut fx.routes)
            }
            Source::Channel(index) => match config.channels.get_mut(index) {
                Some(channel) => (&mut channel.to_pc, &mut channel.routes),
                None => return,
            },
        };
        match to {
            BusId::Pc => *to_pc = !*to_pc,
            BusId::A(index) => *routes ^= config::route_bit(true, index),
            BusId::B(index) => *routes ^= config::route_bit(false, index),
        }
    }

    /// `fx` is 0 for FX1 and 1 for FX2. An effect return has no sends.
    fn toggle_send(&mut self, from: Source, fx: usize) {
        let config = &mut self.config;
        let (send_fx1, send_fx2) = match from {
            Source::Mic => (&mut config.mic_send_fx1, &mut config.mic_send_fx2),
            Source::Channel(index) => match config.channels.get_mut(index) {
                Some(channel) => (&mut channel.send_fx1, &mut channel.send_fx2),
                None => return,
            },
            Source::Fx(_) => return,
        };
        let send = if fx == 0 { send_fx1 } else { send_fx2 };
        *send = !*send;
    }

    fn toggle_bus_mute(&mut self, bus: BusId) {
        let bus = match bus {
            BusId::Pc => Some(&mut self.config.pc),
            BusId::A(index) => self.config.a.get_mut(index),
            BusId::B(index) => self.config.b.get_mut(index),
        };
        if let Some(bus) = bus {
            bus.muted = !bus.muted;
        }
    }
}

/// Effect chains.
impl Panel {
    fn add_effect(&mut self, bus: usize, kind: EffectKind) -> Action {
        if matches!(self.overlay, Overlay::FxPicker(_)) {
            self.overlay = Overlay::None;
        }
        let chain = &mut fx::bus_mut(&mut self.config, bus).chain;
        if chain.len() >= CHAIN_MAX {
            return Action::None;
        }
        let mut slot = EffectSlot::new(kind);
        if kind == EffectKind::Eq {
            slot.eq = Some(ParamEq::new(6));
        }
        chain.push(slot);
        let added = FxSlot { bus, index: chain.len() - 1 };
        if kind == EffectKind::Eq {
            self.open_eq(added);
        } else {
            self.fx_edit = Some(added);
        }
        Action::Save
    }

    /// Adds the scanned plugin at `plugin`, if this machine can run it.
    fn add_vst(&mut self, bus: usize, plugin: usize) -> Action {
        let Some(info) = self.vst_plugins.get(plugin).filter(|info| info.arch.runs()) else { return Action::None };
        let chain = &mut fx::bus_mut(&mut self.config, bus).chain;
        if chain.len() >= CHAIN_MAX {
            return Action::None;
        }
        let mut slot = EffectSlot::new(EffectKind::Vst);
        slot.vst = Some(crate::vst::VstRef::place(info));
        chain.push(slot);
        self.overlay = Overlay::None;
        Action::Save
    }
}
