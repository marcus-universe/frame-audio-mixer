//! Effect-bus drawer: up to three chained effects, drag to reorder.

use super::{fill_round, Panel};
use crate::config::{EffectKind, EffectSlot, FxBus};
use crate::draw::{self, ButtonKind, Fonts, INSET, MUTED, R_CONTROL, TEXT};
use crate::i18n::{self, Text};
use cairo::Context;

const ROW_H: f64 = 52.0;

pub(crate) fn draw(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, bus: usize) {
    let (x, y, w, h) = panel.stage().check;
    if w < 40.0 || h <= 0.0 {
        return;
    }
    draw::card(cr, x, y, w, h);
    if w < 80.0 {
        return;
    }
    let color = if bus == 0 { draw::FX1 } else { draw::FX2 };
    fill_round(cr, x + 20.0, y + 28.0, 8.0, 20.0, 4.0, color);
    let title = if bus == 0 { t.fx1 } else { t.fx2 };
    draw::label(cr, fonts, true, 18.0, TEXT, x + 36.0, y + 44.0, title);
    return_row(panel, cr, fonts, t, bus, x + 16.0, y + 56.0, w - 32.0);

    let chain = if bus == 0 { panel.config.fx_a.chain.clone() } else { panel.config.fx_b.chain.clone() };
    let left = x + 20.0;
    let row_w = w - 40.0;
    let mut ry = y + 148.0;
    let list_bottom = y + h - 16.0;
    for (index, slot) in chain.iter().enumerate() {
        if ry + ROW_H > list_bottom {
            break;
        }
        chain_row(panel, cr, fonts, t, bus, index, slot, left, ry, row_w);
        ry += ROW_H + 8.0;
        if panel.fx_edit == Some((bus as u8, index)) {
            let extra = editor_height(slot.kind);
            if ry + extra < list_bottom {
                params(panel, cr, fonts, t, bus, index, slot, left, ry, row_w);
                ry += extra;
            }
        }
    }
    if chain.len() < 3 && ry + 36.0 < list_bottom {
        draw::label(cr, fonts, false, 13.0, MUTED, left, ry + 16.0, t.add_effect);
        ry += 28.0;
        for kind in EffectKind::ALL {
            if ry + ROW_H > list_bottom {
                break;
            }
            pick_row(panel, cr, fonts, t, bus, kind, left, ry, row_w);
            ry += ROW_H + 4.0;
        }
    }
}

fn chain_row(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, bus: usize, index: usize, slot: &EffectSlot, x: f64, y: f64, w: f64) {
    fill_round(cr, x, y, w, ROW_H, R_CONTROL, INSET);
    fill_round(cr, x + 12.0, y + 20.0, 8.0, 12.0, 4.0, draw::effect_color(slot.kind));
    let name = i18n::effect_name(t, slot.kind);
    draw::label_fit(cr, fonts, true, 15.0, TEXT, x + 32.0, y + 32.0, w - 220.0, name);
    grip(cr, x + w - 200.0, y + 16.0);
    panel.hot(&format!("fxgrip:{bus}:{index}"), x + w - 210.0, y, 36.0, ROW_H);
    panel.button(cr, fonts, x + w - 164.0, y + 10.0, 72.0, 32.0, t.edit, ButtonKind::Plain);
    panel.hot(&format!("fxedit:{bus}:{index}"), x + w - 164.0, y + 10.0, 72.0, 32.0);
    panel.button(cr, fonts, x + w - 80.0, y + 10.0, 64.0, 32.0, "×", ButtonKind::Plain);
    panel.hot(&format!("fxrm:{bus}:{index}"), x + w - 80.0, y + 10.0, 64.0, 32.0);
}

fn pick_row(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, bus: usize, kind: EffectKind, x: f64, y: f64, w: f64) {
    let look = panel.look(x, y, w, ROW_H);
    let fill = match look {
        draw::Look::Down => draw::CONTROL_DOWN,
        draw::Look::Hover => draw::CONTROL_HOVER,
        draw::Look::Rest => INSET,
    };
    fill_round(cr, x, y, w, ROW_H, R_CONTROL, fill);
    fill_round(cr, x + 12.0, y + 20.0, 8.0, 12.0, 4.0, draw::effect_color(kind));
    draw::label_fit(cr, fonts, false, 15.0, TEXT, x + 32.0, y + 32.0, w - 48.0, i18n::effect_name(t, kind));
    panel.hot(&format!("fxadd:{bus}:{}", kind.key()), x, y, w, ROW_H);
}

fn editor_height(kind: EffectKind) -> f64 {
    if kind == EffectKind::Pitcher { 76.0 } else { kind.param_count() as f64 * 48.0 + 8.0 }
}

fn params(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, bus: usize, index: usize, slot: &EffectSlot, x: f64, y: f64, w: f64) {
    if slot.kind == EffectKind::Pitcher {
        pitcher(panel, cr, fonts, t, bus, index, slot, x, y, w);
        return;
    }
    let mut py = y;
    for p in 0..slot.kind.param_count() {
        let label = i18n::param_name(t, slot.kind, p);
        draw::label(cr, fonts, false, 12.0, MUTED, x, py + 16.0, label);
        let track_x = x + 110.0;
        let track_w = (w - 160.0).max(80.0);
        let track_y = py + 8.0;
        let track_h = 24.0;
        let frac = slot.params[p].clamp(0.0, 1.0);
        fill_round(cr, track_x, track_y, track_w, track_h, track_h / 2.0, draw::track());
        fill_round(cr, track_x, track_y, track_w * frac, track_h, track_h / 2.0, draw::effect_color(slot.kind));
        panel.fader_hit(&format!("fxp:{bus}:{index}:{p}"), track_x, track_y, track_w, track_h, true);
        let shown = format!("{:.0}%", frac * 100.0);
        draw::label(cr, fonts, false, 12.0, TEXT, track_x + track_w + 8.0, py + 24.0, &shown);
        py += 48.0;
    }
}

/// Bipolar pitch, −12 at the left, 0 in the middle, +12 at the right. Snap locks whole semitones.
fn pitcher(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, bus: usize, index: usize, slot: &EffectSlot, x: f64, y: f64, w: f64) {
    let snap = slot.pitch_snap;
    let btn_w = 76.0;
    let btn_h = 28.0;
    draw::label_fit(cr, fonts, false, 12.0, MUTED, x, y + 18.0, (w - btn_w - 72.0).max(40.0), t.param_semitones);
    let shown = crate::config::format_pitch(slot.params[0], snap);
    draw::label(cr, fonts, true, 13.0, TEXT, x + w - btn_w - 56.0, y + 18.0, &shown);
    let kind = if snap { ButtonKind::Accent } else { ButtonKind::Plain };
    panel.button(cr, fonts, x + w - btn_w, y, btn_w, btn_h, t.pitch_snap, kind);
    panel.hot(&format!("fxsnap:{bus}:{index}"), x + w - btn_w, y, btn_w, btn_h);

    let track_y = y + 40.0;
    let track_h = 24.0;
    let mark = 28.0;
    let gap = 8.0;
    draw::label(cr, fonts, false, 12.0, MUTED, x, track_y + 16.0, "-12");
    let track_x = x + mark + gap;
    let track_w = (w - 2.0 * (mark + gap)).max(48.0);
    draw::label(cr, fonts, false, 12.0, MUTED, track_x + track_w + gap, track_y + 16.0, "+12");
    let frac = slot.params[0].clamp(0.0, 1.0);
    fill_round(cr, track_x, track_y, track_w, track_h, track_h / 2.0, draw::track());
    let center = track_x + track_w * 0.5;
    let thumb = track_x + track_w * frac;
    let span_x = thumb.min(center);
    let span_w = (thumb - center).abs();
    if span_w > 1.0 {
        let radius = (track_h / 2.0).min(span_w / 2.0);
        fill_round(cr, span_x, track_y, span_w, track_h, radius, draw::effect_color(slot.kind));
    }
    fill_round(cr, center - 1.0, track_y + 4.0, 2.0, track_h - 8.0, 1.0, MUTED);
    draw::fill_circle(cr, thumb, track_y + track_h / 2.0, 9.0, draw::effect_color(slot.kind));
    panel.fader_hit(&format!("fxp:{bus}:{index}:0"), track_x, track_y, track_w, track_h, true);
}

fn grip(cr: &Context, x: f64, y: f64) {
    for row in 0..3 {
        for col in 0..2 {
            draw::fill_circle(cr, x + col as f64 * 6.0, y + 6.0 + row as f64 * 7.0, 1.6, MUTED);
        }
    }
}

fn return_row(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, bus: usize, x: f64, y: f64, w: f64) {
    let fx = if bus == 0 { panel.config.fx_a.clone() } else { panel.config.fx_b.clone() };
    let who = if bus == 0 { "fx1" } else { "fx2" };
    let sides = crate::config::side_count(panel.config.outputs);
    let mut bx = x;
    for index in 0..sides {
        let on = crate::config::route_on(fx.routes, true, index);
        let fill = if on { draw::TEXT } else { INSET };
        let ink = if on { draw::BG } else { MUTED };
        fill_round(cr, bx, y, 36.0, 28.0, 8.0, fill);
        draw::label_mid(cr, fonts, true, 11.0, ink, bx, y + 14.0, &format!("A{}", index + 1));
        panel.hot(&format!("route:{who}:a:{index}"), bx, y, 36.0, 28.0);
        bx += 40.0;
    }
    bx = x;
    for index in 0..sides {
        let on = crate::config::route_on(fx.routes, false, index);
        let fill = if on { draw::accent() } else { INSET };
        let ink = if on { draw::on_accent() } else { MUTED };
        fill_round(cr, bx, y + 32.0, 36.0, 28.0, 8.0, fill);
        draw::label_mid(cr, fonts, true, 11.0, ink, bx, y + 46.0, &format!("B{}", index + 1));
        panel.hot(&format!("route:{who}:b:{index}"), bx, y + 32.0, 36.0, 28.0);
        bx += 40.0;
    }
    let fader = draw::paint_side_fader(cr, fonts, x + w - 150.0, y + 20.0, 140.0, 36.0, fx.gain_db, 0.0, false, fx.muted, draw::FaderGlyph::Speaker, false);
    panel.fader_hit(&format!("fxret:{bus}"), fader.capsule_x, fader.capsule_y, fader.capsule_w, fader.capsule_h, true);
    let _ = t;
}

pub(crate) fn bus_mut(config: &mut crate::config::Config, bus: usize) -> &mut FxBus {
    if bus == 0 { &mut config.fx_a } else { &mut config.fx_b }
}
