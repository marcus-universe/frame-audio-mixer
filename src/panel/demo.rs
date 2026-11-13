//! The demo panel behind `--dump-png`: one PNG per page, dialog and view, drawn
//! from made-up channels, devices and levels. The screenshots in the README and
//! the pixel comparison of a refactor both come from here.

use super::hit::Hit;
use super::menu::Menu;
use super::{Drawer, FxSlot, Overlay, Page, Panel};
use crate::apps::Entry;
use crate::config::{self, Config, EffectKind, EffectSlot, Pin};
use crate::draw::Fonts;
use crate::graph::Graph;
use crate::i18n::Lang;
use crate::worker::{CheckPhase, Snapshot};
use std::collections::HashMap;

impl Panel {
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
    panel.page = Page::Flow;
    panel.config.flow_apps = true;
    // One unassigned app playing on the speakers, for the Other-apps lane.
    let flow_extra = snap.graph.nodes.len();
    let stream_node = |id: u32, name: &str, class: &str, app: &str, binary: &str| crate::graph::AudioNode {
        id,
        serial: u64::from(id),
        name: name.into(),
        description: String::new(),
        media_class: class.into(),
        media_name: "Playback".into(),
        app_name: app.into(),
        binary: binary.into(),
        is_virtual: false,
        target: String::new(),
    };
    snap.graph.nodes.push(stream_node(950, "chromium-play", "Stream/Output/Audio", "Chromium", "chromium"));
    snap.graph.nodes.push(crate::graph::AudioNode {
        description: "Frame Speakers".into(),
        ..stream_node(951, &snap.graph.default_sink.clone(), "Audio/Sink", "", "")
    });
    snap.graph.edges.push((950, 951));
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "flow"))?;
    snap.graph.nodes.truncate(flow_extra);
    snap.graph.edges.clear();
    panel.config.flow_apps = false;
    panel.flow.sel = Some("ch1".into());
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "flow-selected"))?;
    panel.flow.sel = None;
    panel.page = Page::Mix;
    panel.overlay = Overlay::Projects;
    panel.projects = vec![
        config::ProjectInfo { name: "VR Night".into(), path: std::path::PathBuf::from("/tmp/vr-night.json") },
        config::ProjectInfo { name: "Streaming".into(), path: std::path::PathBuf::from("/tmp/streaming.json") },
    ];
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "projects"))?;
    panel.projects.clear();
    panel.overlay = Overlay::None;
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
    panel.config.fx_a.chain.push(EffectSlot::new(EffectKind::Reverb));
    panel.config.fx_a.chain.push(EffectSlot::new(EffectKind::Delay));
    panel.fx_edit = Some(FxSlot { bus: 0, index: 0 });
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "fx"))?;
    panel.config.fx_a.chain[0].extra = [0.42, 0.6, 0.35];
    panel.config.fx_a.chain[0].params = [0.85, 0.8, 0.32];
    let _ = panel.handle(Hit::OpenMenu(Menu::Preset { bus: 0, index: 0 }));
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "fx-presets"))?;
    panel.menu = None;
    // The Pitcher as Male to female, with its preset list open.
    let reverb = std::mem::replace(&mut panel.config.fx_a.chain[0], EffectSlot::new(EffectKind::Pitcher));
    crate::fx::presets::of(EffectKind::Pitcher)[0].apply(&mut panel.config.fx_a.chain[0]);
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "fx-pitcher"))?;
    let _ = panel.handle(Hit::OpenMenu(Menu::Preset { bus: 0, index: 0 }));
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "fx-pitcher-presets"))?;
    panel.menu = None;
    panel.config.fx_a.chain[0] = reverb;
    let _ = panel.handle(Hit::FxAdd { bus: 0, kind: EffectKind::Eq });
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "fx-eq"))?;
    for (view, name) in [(config::EqView::Line, "eq-line"), (config::EqView::Area, "eq-area"), (config::EqView::Bars, "eq-bars"), (config::EqView::Heat, "eq-heat")] {
        panel.config.eq_view = view;
        panel.spectrum.clear();
        for frame in 0..120 {
            panel.ingest_spectrum(&demo_spectrum(frame));
        }
        write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, name))?;
    }
    panel.overlay = Overlay::None;
    let demo_vsts = vec![
        crate::vst::VstInfo { name: "Valhalla Supermassive".into(), vendor: "Valhalla DSP".into(), path: "/p/a.vst3".into(), cid: String::new(), arch: crate::vst::VstArch::Windows },
        crate::vst::VstInfo { name: "Dragonfly Room Reverb".into(), vendor: "Michael Willis".into(), path: "/p/b.vst3".into(), cid: String::new(), arch: crate::vst::VstArch::Native },
        crate::vst::VstInfo { name: "OldVerb".into(), vendor: String::new(), path: "/p/c.vst3".into(), cid: String::new(), arch: crate::vst::VstArch::Unsupported },
    ];
    snap.vsts = demo_vsts.clone();
    let _ = panel.handle(Hit::FxPicker(0));
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "fx-picker"))?;
    let _ = panel.handle(Hit::VstSettings);
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "vst-settings"))?;
    panel.overlay = Overlay::None;
    panel.config.fx_a.chain.truncate(1);
    let _ = panel.handle(Hit::FxAddVst { bus: 0, plugin: 0 });
    let instance = panel.config.fx_a.chain.last().and_then(|slot| slot.vst.as_ref()).map(|v| v.instance.clone()).unwrap_or_default();
    snap.vst_status.insert(instance, "starting".into());
    panel.fx_edit = None;
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "fx-vst"))?;
    snap.vsts.clear();
    let _ = panel.handle(Hit::FxPicker(0));
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "fx-picker-empty"))?;
    panel.overlay = Overlay::None;
    let _ = panel.handle(Hit::FxAdd { bus: 0, kind: EffectKind::Eq });
    panel.config.eq_view = config::EqView::Heat;
    let _ = panel.handle(Hit::OpenMenu(Menu::EqView));
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "eq-menu"))?;
    // A linked PC: it takes B1 and B3, and sends PC1 but not PC2.
    panel.menu = None;
    panel.overlay = Overlay::None;
    panel.fx_edit = None;
    panel.config.channels.truncate(2);
    panel.channels_changed();
    let _ = panel.handle(Hit::AddPcInput(0));
    let _ = panel.handle(Hit::AddPcInput(1));
    panel.scroll = 0.0;
    panel.config.b[0].name = "Discord".into();
    snap.link.state = frame_link_core::LinkState::Connected;
    snap.link.peer = "DESKTOP-PC".into();
    snap.link.remote_recv = 0b0101;
    snap.link.remote_send = 0b0001;
    panel.set_drawer(Drawer::Outputs);
    panel.drawer_progress = 1.0;
    panel.drawer = Drawer::Outputs;
    // Scrolled to the end, where the virtual microphones are.
    panel.out_scroll = f64::MAX;
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "link"))?;
    panel.config.list = true;
    panel.config.input_list = true;
    panel.out_scroll = f64::MAX;
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "link-rows"))?;
    panel.config.list = false;
    panel.config.input_list = false;
    let _ = panel.handle(Hit::AddChannel);
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "link-add"))?;
    let _ = panel.handle(Hit::Settings);
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "link-settings"))?;
    panel.overlay = Overlay::None;
    panel.page = Page::Flow;
    write_surface(&mut panel, fonts, &snap, &loud, &suffix(path, "link-flow"))?;
    Ok(())
}

/// A moving, harmonic spectrum for the screenshots.
fn demo_spectrum(frame: usize) -> Vec<f32> {
    let pulse = (frame as f32 * 0.21).sin() * 7.0;
    (0..crate::fx::analyzer::BANDS)
        .map(|band| {
            let t = band as f32;
            -38.0 - t * 0.3 + (t * 0.55 + frame as f32 * 0.05).sin() * 12.0 + (t * 0.13).cos() * 8.0 + pulse
        })
        .collect()
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
