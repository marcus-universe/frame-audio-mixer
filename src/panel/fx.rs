//! Effect-bus drawer: up to three chained effects, drag to reorder.

use super::menu::{self, Menu};
use super::{FaderId, FxSlot, Hit, Panel, Ui};
use crate::config::{self, EffectKind, EffectSlot, FxBus};
use crate::draw::{self, ButtonKind, Fader, FaderGlyph, Font, Rect, Rgb, BUTTON, INSET, MUTED, R_CONTROL, TEXT};
use crate::i18n;

const ROW_H: f64 = 52.0;
const ROW_GAP: f64 = 8.0;
/// The level fader of the bus, under the title.
const LEVEL_TOP: f64 = 68.0;
const LEVEL_H: f64 = 36.0;
/// First chain row, from the top of the drawer.
pub(crate) const CHAIN_TOP: f64 = 124.0;
/// Distance from one chain row to the next.
pub(crate) const CHAIN_STEP: f64 = ROW_H + ROW_GAP;
/// A chain holds at most this many effects.
const CHAIN_MAX: usize = 3;
/// Height of the preset dropdown and its gap, above an effect's sliders.
const PRESET_ROW: f64 = 46.0;
/// One parameter slider and its gap.
const PARAM_STEP: f64 = 48.0;
const TRACK_H: f64 = 24.0;
/// Edit, Open and × on a chain row.
const ROW_BUTTON_H: f64 = 32.0;

const SMALL: Font = Font::regular(12.0);

fn bus_color(bus: usize) -> Rgb {
    if bus == 0 { draw::FX1 } else { draw::FX2 }
}

/// The drawer of one effect bus: its level, then its chain. Where the result goes
/// is not chosen here: a strip that switches the bus on plays the result on the
/// strip's own outputs.
pub(crate) fn draw(panel: &mut Panel, ui: &mut Ui, bus: usize) {
    let t = ui.t;
    let card = panel.stage().drawer;
    if card.w < 40.0 || card.h <= 0.0 {
        return;
    }
    ui.card(card);
    if card.w < 80.0 {
        return;
    }
    ui.fill(Rect::new(card.x + 20.0, card.y + 28.0, 8.0, 20.0), 4.0, bus_color(bus));
    ui.label(Font::bold(18.0), TEXT, card.x + 36.0, card.y + 44.0, if bus == 0 { t.fx1 } else { t.fx2 });
    level_fader(panel, ui, bus, Rect::new(card.x + 12.0, card.y + LEVEL_TOP, card.w - 28.0, LEVEL_H));

    let chain = bus_mut(&mut panel.config, bus).chain.clone();
    let left = card.x + 20.0;
    let row_w = card.w - 40.0;
    let list_bottom = card.y + card.h - 16.0;
    let mut y = card.y + CHAIN_TOP;
    for (index, slot) in chain.iter().enumerate() {
        if y + ROW_H > list_bottom {
            break;
        }
        chain_row(panel, ui, FxSlot { bus, index }, slot, Rect::new(left, y, row_w, ROW_H));
        y += CHAIN_STEP;
        if panel.fx_edit == Some(FxSlot { bus, index }) {
            let editor_h = editor_height(slot.kind);
            if y + editor_h < list_bottom {
                editor(panel, ui, FxSlot { bus, index }, slot, left, y, row_w);
                y += editor_h;
            }
        }
    }
    if chain.len() < CHAIN_MAX && y + ROW_H <= list_bottom {
        add_row(ui, bus, Rect::new(left, y, row_w, ROW_H));
    }
}

/// One wide target with a plus. The picker it opens names everything.
fn add_row(ui: &mut Ui, bus: usize, row: Rect) {
    ui.fill(row, R_CONTROL, ui.control_fill(row, draw::mix(draw::CONTROL, INSET, 0.5)));
    let glyph = 28.0;
    ui.icon(draw::ADD_PATH, 28.0, row.x + (row.w - glyph) / 2.0, row.y + (ROW_H - glyph) / 2.0, glyph, draw::SOFT);
    ui.hot(Hit::FxPicker(bus), row);
}

/// One effect of the chain: its color, its name, a grip, Edit (or Open for a plugin) and ×.
fn chain_row(panel: &mut Panel, ui: &mut Ui, at: FxSlot, slot: &EffectSlot, row: Rect) {
    let t = ui.t;
    let FxSlot { bus, index } = at;
    let Rect { x, y, w, .. } = row;
    ui.fill(row, R_CONTROL, INSET);
    ui.fill(Rect::new(x + 12.0, y + 20.0, 8.0, 12.0), 4.0, draw::effect_color(slot.kind));
    grip(ui, x + w - 200.0, y + 16.0);
    ui.hot(Hit::FxGrip { bus, index }, Rect::new(x + w - 210.0, y, 36.0, ROW_H));
    let action = Rect::new(x + w - 164.0, y + 10.0, 72.0, ROW_BUTTON_H);
    if let Some(plugin) = &slot.vst {
        let status = panel.vst_status.get(&plugin.instance).map(String::as_str).unwrap_or("starting");
        let note = match status {
            "running" => None,
            "starting" => Some(t.vst_starting),
            _ => Some(t.vst_stopped),
        };
        match note {
            Some(note) => {
                ui.label_fit(BUTTON, TEXT, x + 32.0, y + 24.0, w - 250.0, &plugin.name);
                ui.label_fit(SMALL, MUTED, x + 32.0, y + 42.0, w - 250.0, note);
            }
            None => ui.label_fit(BUTTON, TEXT, x + 32.0, y + 32.0, w - 250.0, &plugin.name),
        }
        ui.button_hot(action, t.vst_open, ButtonKind::Plain, Hit::VstEditor(plugin.instance.clone()));
    } else {
        ui.label_fit(BUTTON, TEXT, x + 32.0, y + 32.0, w - 220.0, i18n::effect_name(t, slot.kind));
        ui.button_hot(action, t.edit, ButtonKind::Plain, Hit::FxEdit { bus, index });
    }
    let remove = Rect::new(x + w - 80.0, y + 10.0, 64.0, ROW_BUTTON_H);
    ui.button_hot(remove, "×", ButtonKind::Plain, Hit::FxRemove { bus, index });
}

/// The pitch row of the Pitcher: its caption with Snap, then the wide track.
const PITCH_BLOCK: f64 = 76.0;

fn editor_height(kind: EffectKind) -> f64 {
    let sliders = if kind == EffectKind::Pitcher { PITCH_BLOCK + 2.0 * PARAM_STEP } else { kind.param_count() as f64 * PARAM_STEP };
    PRESET_ROW + sliders + 8.0
}

/// The open effect: its preset dropdown, then one slider per parameter.
fn editor(panel: &mut Panel, ui: &mut Ui, at: FxSlot, slot: &EffectSlot, x: f64, y: f64, w: f64) {
    let t = ui.t;
    let FxSlot { bus, index } = at;
    let caption = menu::preset_caption(slot, panel.lang, t);
    menu::button(panel, ui, Menu::Preset { bus, index }, Rect::new(x, y, w, PRESET_ROW - 8.0), &caption);
    let y = y + PRESET_ROW;
    if slot.kind == EffectKind::Pitcher {
        pitcher(ui, at, slot, x, y, w);
        return;
    }
    let color = draw::effect_color(slot.kind);
    for param in 0..slot.kind.param_count() {
        let py = y + param as f64 * PARAM_STEP;
        ui.label(SMALL, MUTED, x, py + 16.0, i18n::param_name(t, slot.kind, param));
        let track = Rect::new(x + 110.0, py + 8.0, (w - 160.0).max(80.0), TRACK_H);
        let fraction = slot.params[param].clamp(0.0, 1.0);
        ui.fill(track, TRACK_H / 2.0, draw::track());
        ui.fill(track.take_left(track.w * fraction), TRACK_H / 2.0, color);
        ui.fader(FaderId::FxParam { bus, index, param }, track, true);
        ui.label(SMALL, TEXT, track.x + track.w + 8.0, py + 24.0, &format!("{:.0}%", fraction * 100.0));
    }
}

/// Pitch, then formant and brightness. Pitch and formant apart are what make a
/// shifted voice sound like another person instead of a chipmunk.
fn pitcher(ui: &mut Ui, at: FxSlot, slot: &EffectSlot, x: f64, y: f64, w: f64) {
    pitch_row(ui, at, slot, x, y, w);
    let color = draw::effect_color(slot.kind);
    let t = ui.t;
    let FxSlot { bus, index } = at;
    let formant = config::format_pitch(slot.params[1], false);
    let row = y + PITCH_BLOCK;
    bipolar_row(ui, t.param_formant, &formant, slot.params[1], color, FaderId::FxParam { bus, index, param: 1 }, x, row, w);
    let percent = ((slot.params[2] - 0.5) * 200.0).round();
    let brightness = if percent == 0.0 { "0".to_string() } else { format!("{percent:+.0}%") };
    bipolar_row(ui, t.param_brightness, &brightness, slot.params[2], color, FaderId::FxParam { bus, index, param: 2 }, x, row + PARAM_STEP, w);
}

/// A slider that is neutral in the middle: the fill grows left or right from there.
fn bipolar_row(ui: &mut Ui, caption: &str, value: &str, param: f64, color: Rgb, id: FaderId, x: f64, y: f64, w: f64) {
    ui.label_fit(SMALL, MUTED, x, y + 16.0, 100.0, caption);
    let track = Rect::new(x + 110.0, y + 8.0, (w - 160.0).max(80.0), TRACK_H);
    bipolar_track(ui, track, param, color);
    ui.fader(id, track, true);
    ui.label(SMALL, TEXT, track.x + track.w + 8.0, y + 24.0, value);
}

/// The track, the fill from the center to the value, the center mark and the thumb.
fn bipolar_track(ui: &mut Ui, track: Rect, param: f64, color: Rgb) {
    let fraction = param.clamp(0.0, 1.0);
    ui.fill(track, TRACK_H / 2.0, draw::track());
    let center = track.x + track.w * 0.5;
    let thumb = track.x + track.w * fraction;
    let span_w = (thumb - center).abs();
    if span_w > 1.0 {
        let radius = (TRACK_H / 2.0).min(span_w / 2.0);
        ui.fill(Rect::new(thumb.min(center), track.y, span_w, TRACK_H), radius, color);
    }
    ui.fill(Rect::new(center - 1.0, track.y + 4.0, 2.0, TRACK_H - 8.0), 1.0, MUTED);
    ui.circle(thumb, track.y + TRACK_H / 2.0, 9.0, color);
}

/// Bipolar pitch, −12 at the left, 0 in the middle, +12 at the right. Snap locks whole semitones.
fn pitch_row(ui: &mut Ui, at: FxSlot, slot: &EffectSlot, x: f64, y: f64, w: f64) {
    let t = ui.t;
    let FxSlot { bus, index } = at;
    let snap = slot.pitch_snap;
    let snap_button = Rect::new(x + w - 76.0, y, 76.0, 28.0);
    ui.label_fit(SMALL, MUTED, x, y + 18.0, (w - snap_button.w - 72.0).max(40.0), t.param_semitones);
    let shown = config::format_pitch(slot.params[0], snap);
    ui.label(Font::bold(13.0), TEXT, x + w - snap_button.w - 56.0, y + 18.0, &shown);
    let kind = if snap { ButtonKind::Accent } else { ButtonKind::Plain };
    ui.button_hot(snap_button, t.pitch_snap, kind, Hit::FxPitchSnap { bus, index });

    // The track sits between its two end labels.
    let (mark, gap) = (28.0, 8.0);
    let track = Rect::new(x + mark + gap, y + 40.0, (w - 2.0 * (mark + gap)).max(48.0), TRACK_H);
    ui.label(SMALL, MUTED, x, track.y + 16.0, "-12");
    ui.label(SMALL, MUTED, track.x + track.w + gap, track.y + 16.0, "+12");
    bipolar_track(ui, track, slot.params[0], draw::effect_color(slot.kind));
    ui.fader(FaderId::FxParam { bus, index, param: 0 }, track, true);
}

/// Six dots that say "drag me".
fn grip(ui: &Ui, x: f64, y: f64) {
    for row in 0..3 {
        for col in 0..2 {
            ui.circle(x + col as f64 * 6.0, y + 6.0 + row as f64 * 7.0, 1.6, MUTED);
        }
    }
}

/// The level of the bus after its chain, the same for every strip that uses it.
fn level_fader(panel: &mut Panel, ui: &mut Ui, bus: usize, area: Rect) {
    let fx = bus_mut(&mut panel.config, bus);
    let level = Fader { db: fx.gain_db, levels: &[], meters: false, muted: fx.muted, glyph: FaderGlyph::Speaker, snaps: false };
    let geom = draw::paint_side_fader(ui.cr, ui.fonts, area, &level);
    ui.fader(FaderId::FxLevel(bus), geom.capsule, true);
}

pub(crate) fn bus_mut(config: &mut config::Config, bus: usize) -> &mut FxBus {
    if bus == 0 { &mut config.fx_a } else { &mut config.fx_b }
}
