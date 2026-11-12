//! Checks behind `--self-test`: layout limits, every page drawn at awkward
//! sizes, and the click paths that have broken before.

use super::hit::{BusId, FaderId, Hit, Source};
use super::ui::{HitBounds, Ui};
use super::{flow, menu, naming, spectrum, Action, Drawer, Overlay, Panel};
use crate::config::{Channel, Config, EffectKind};
use crate::draw::{self, Rect};
use crate::i18n::{self, Lang};
use crate::worker::Snapshot;
use std::collections::HashMap;

pub fn self_test() -> Result<(), String> {
    naming::self_test()?;
    layout()?;
    hit_bounds()?;
    clicks()?;
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
        drawn_hits(&fonts)?;
    }
    if (draw::scroll_thumb(41.8, 800.0, 48.0) - 41.8).abs() > 0.01 {
        return Err("short scrollbar thumb".into());
    }
    flow::self_test()?;
    menu::self_test()?;
    spectrum::self_test()?;
    Ok(())
}

fn layout() -> Result<(), String> {
    let mut config = Config::default();
    config.list = true;
    config.wide = true;
    let stage = Panel::new(config).stage();
    if stage.portrait || stage.drawer.x <= stage.mix.right() {
        return Err("list extend check on the right".into());
    }
    if (stage.rail.w - 48.0).abs() > 0.1 {
        return Err("rail width".into());
    }
    let mut wide = Config::default();
    wide.wide = true;
    wide.channels.push(Channel::new("ch1", "Music"));
    let open = Panel::new(wide.clone()).stage();
    let third = open.w / 3.0;
    if (open.drawer.w - third).abs() > 1.0 || open.mix.w < third {
        return Err(format!("wide outputs {} of {}, mix {}", open.drawer.w, open.w, open.mix.w));
    }
    for n in 2..14 {
        wide.channels.push(Channel::new(format!("ch{n}"), "More"));
    }
    let full = Panel::new(wide).stage();
    if (full.drawer.w - third).abs() > 1.0 {
        return Err(format!("wide outputs stay a third {}", full.drawer.w));
    }
    let mut squeezed = Config::default();
    squeezed.wide = true;
    squeezed.split = 0.05;
    let tight = Panel::new(squeezed).stage();
    let min = tight.w * 0.20;
    if tight.drawer.w + 0.5 < min || tight.mix.w + 0.5 < min {
        return Err(format!("split floor mix {} check {} min {}", tight.mix.w, tight.drawer.w, min));
    }
    let mut wide_side = Config::default();
    wide_side.wide = true;
    wide_side.split = 0.95;
    let other = Panel::new(wide_side).stage();
    if other.mix.w + 0.5 < min || other.drawer.w + 0.5 < min {
        return Err(format!("split ceiling mix {} check {}", other.mix.w, other.drawer.w));
    }
    Ok(())
}

/// A hit area inside a scrolling list is cut to the visible part of the list.
fn hit_bounds() -> Result<(), String> {
    let surface = draw::surface_wh(16, 16)?;
    let cr = cairo::Context::new(&surface).map_err(|e| e.to_string())?;
    let Ok(fonts) = draw::Fonts::load() else { return Ok(()) };
    let mut ui = Ui { cr: &cr, fonts: &fonts, t: i18n::text(Lang::En), pointer: None, pressed: false, bounds: None, hots: Vec::new(), faders: Vec::new() };
    ui.bounds = Some(HitBounds::columns(100.0, 200.0));
    ui.hot(Hit::Noop, Rect::new(180.0, 0.0, 40.0, 10.0));
    ui.hot(Hit::Noop, Rect::new(220.0, 0.0, 40.0, 10.0));
    ui.fader(FaderId::Mic, Rect::new(180.0, 0.0, 40.0, 10.0), false);
    ui.fader(FaderId::Mic, Rect::new(120.0, 0.0, 40.0, 10.0), false);
    if ui.hots.len() != 1 || ui.hots[0].rect != Rect::new(180.0, 0.0, 20.0, 10.0) {
        return Err("hit area not cut to the list".into());
    }
    if ui.faders.len() != 1 || ui.faders[0].rect.x != 120.0 {
        return Err("half-hidden fader stays grabbable".into());
    }
    ui.bounds = Some(HitBounds::columns(100.0, 200.0).cutting_faders());
    ui.fader(FaderId::Mic, Rect::new(180.0, 0.0, 40.0, 10.0), false);
    if ui.faders.len() != 2 || ui.faders[1].rect.w != 20.0 {
        return Err("fader not cut to the list".into());
    }
    Ok(())
}

/// Clicks change exactly the state they name.
fn clicks() -> Result<(), String> {
    let mut panel = Panel::demo(Lang::En);
    let routes = panel.config.channels[1].routes;
    let _ = panel.handle(Hit::Route { from: Source::Channel(1), to: BusId::B(2) });
    if panel.config.channels[1].routes != routes ^ crate::config::route_bit(false, 2) || panel.config.channels[0].routes != routes {
        return Err("route toggle".into());
    }
    let to_pc = panel.config.mic_to_pc;
    let _ = panel.handle(Hit::Route { from: Source::Mic, to: BusId::Pc });
    if panel.config.mic_to_pc == to_pc {
        return Err("mic to pc".into());
    }
    let _ = panel.handle(Hit::Insert { from: Source::Channel(0), fx: 1 });
    if !panel.config.channels[0].send_fx2 || panel.config.channels[0].send_fx1 {
        return Err("FX2 on a channel".into());
    }
    let _ = panel.handle(Hit::Mute(Source::Channel(2)));
    let _ = panel.handle(Hit::BusMute(BusId::A(0)));
    if !panel.config.channels[2].muted || !panel.config.a[0].muted || panel.config.mic_muted {
        return Err("mute".into());
    }
    // Removing a channel takes two presses, and drops the positions of the old frame.
    let count = panel.config.channels.len();
    let second = panel.config.channels[1].id.clone();
    if !matches!(panel.handle(Hit::RemoveChannel(1)), Action::None) || panel.config.channels.len() != count || !panel.channel_armed(&second) {
        return Err("remove arms first".into());
    }
    if !matches!(panel.handle(Hit::RemoveChannel(1)), Action::Save) || panel.config.channels.iter().any(|channel| channel.id == second) {
        return Err("remove on the second press".into());
    }
    if !panel.hots.is_empty() || !panel.faders.is_empty() {
        return Err("stale hit areas after a removal".into());
    }
    let _ = panel.handle(Hit::Drawer(Drawer::FxA));
    if !panel.drawer_shows_fx(0) {
        return Err("rail opens FX1".into());
    }
    let _ = panel.handle(Hit::FxAdd { bus: 0, kind: EffectKind::Eq });
    if !matches!(panel.overlay, Overlay::Eq(slot) if slot.bus == 0 && slot.index == 0) || panel.spectrum_target().is_some() == (panel.config.eq_view == crate::config::EqView::Off) {
        return Err("adding an EQ opens its sheet".into());
    }
    let _ = panel.handle(Hit::Dismiss);
    for _ in 0..4 {
        let _ = panel.handle(Hit::FxAdd { bus: 0, kind: EffectKind::Delay });
    }
    if panel.config.fx_a.chain.len() != 3 {
        return Err("chain holds three effects".into());
    }
    Ok(())
}

/// Every page records hit areas, and a press on a recorded fader moves it.
fn drawn_hits(fonts: &draw::Fonts) -> Result<(), String> {
    let snap = Snapshot::default();
    let peaks = HashMap::new();
    let mut panel = Panel::demo(Lang::En);
    let _ = panel.render_surface(fonts, &snap, &peaks);
    for (what, wanted) in [
        ("mic mute", Hit::Mute(Source::Mic)),
        ("rename first channel", Hit::RenameChannel(0)),
        ("pc mute", Hit::BusMute(BusId::Pc)),
        ("record", Hit::Record),
        ("settings", Hit::Settings),
        ("quit", Hit::Quit),
        ("rail", Hit::Drawer(Drawer::FxB)),
    ] {
        if !panel.hots.iter().any(|hot| hot.hit == wanted) {
            return Err(format!("no hit area for {what}"));
        }
    }
    let Some(mic) = panel.faders.iter().find(|fader| fader.id == FaderId::Mic).copied() else {
        return Err("no mic fader".into());
    };
    let before = panel.config.mic_gain_db;
    let _ = panel.pointer(crate::overlay::MOUSE_DOWN, mic.rect.mid_x(), mic.rect.y + mic.rect.h * 0.9, 1);
    if !panel.interacting() || (panel.config.mic_gain_db - before).abs() < 0.5 {
        return Err("fader press".into());
    }
    let _ = panel.pointer(crate::overlay::MOUSE_UP, 0.0, 0.0, 1);
    if panel.interacting() {
        return Err("fader release".into());
    }
    Ok(())
}
