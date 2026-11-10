//! Parametric EQ sheet for an EQ slot in an FX chain.

use super::menu::{self, Menu};
use super::{EqSlider, FaderId, FxSlot, Hit, Overlay, Panel, Ui};
use crate::config::{BandType, EffectKind, EffectSlot, ParamEq};
use crate::draw::{self, Font, Rect, HEADING, MUTED, TEXT};
use crate::fx::dsp::eq_response_db;
use crate::i18n::Text;
use cairo::Context;

/// Buttons of the title row and the band row.
const BUTTON_H: f64 = 32.0;
const HANDLE_R: f64 = 11.0;
/// The band handles are grabbed a little outside their circle.
const HANDLE_HIT: f64 = 28.0;
const SMALL: Font = Font::regular(12.0);
const SMALL_BOLD: Font = Font::bold(12.0);

pub(crate) fn sheet(panel: &mut Panel, ui: &mut Ui, slot: FxSlot) {
    let t = ui.t;
    let Some(eq) = eq_mut(panel, slot).map(|eq| eq.clone()) else {
        panel.overlay = Overlay::None;
        return;
    };
    let sheet = panel.sheet_rect(16.0);
    ui.dialog(panel.window(), sheet);
    ui.label(HEADING, TEXT, sheet.x + 24.0, sheet.y + 36.0, t.eq_title);
    title_buttons(ui, sheet, &eq);

    let graph = Rect::new(sheet.x + 28.0, sheet.y + 64.0, sheet.w - 56.0, (sheet.h - 220.0).max(180.0));
    panel.eq_graph = Some(graph);
    ui.fill(graph, 12.0, draw::BG);
    ui.clip_round(graph, 12.0);
    panel.spectrum.paint(ui.cr, panel.config.eq_view, graph);
    ui.unclip();
    paint_curve(ui.cr, &eq, graph);
    let band = panel.eq_band.min(eq.bands.len().saturating_sub(1));
    for (index, item) in eq.bands.iter().enumerate() {
        let px = graph.x + freq_x(item.freq, graph.w);
        let py = graph.y + gain_y(item.gain_db, graph.h);
        ui.circle(px, py, HANDLE_R, if index == band { draw::accent() } else { TEXT });
        ui.label_centered(Font::bold(11.0), draw::BG, px - HANDLE_R, 2.0 * HANDLE_R, py, &(index + 1).to_string());
        ui.hot(Hit::EqBand(index), Rect::square(px - HANDLE_HIT / 2.0, py - HANDLE_HIT / 2.0, HANDLE_HIT));
    }

    let controls_y = graph.y + graph.h + 18.0;
    band_controls(ui, &eq, band, graph, controls_y);
    menus_row(panel, ui, slot, graph.x, controls_y + 46.0);
}

/// Number of bands, then Reset, Bypass and Done, right-aligned in the title row.
fn title_buttons(ui: &mut Ui, sheet: Rect, eq: &ParamEq) {
    let t = ui.t;
    let y = sheet.y + 16.0;
    let right = sheet.x + sheet.w;
    let mut x = right - 24.0 - 520.0;
    for points in [3u8, 6, 8, 12] {
        let on = eq.points == points;
        let button = Rect::new(x, y, 52.0, BUTTON_H);
        ui.fill(button, 10.0, if on { draw::accent() } else { draw::CONTROL });
        let ink = if on { draw::on_accent() } else { draw::SOFT };
        ui.label_centered(Font::bold(14.0), ink, x, 52.0, y + 16.0, &points.to_string());
        ui.hot(Hit::EqPoints(points), button);
        x += 58.0;
    }
    text_button(ui, Rect::new(right - 250.0, y, 70.0, BUTTON_H), t.eq_reset, Hit::EqReset);
    text_button(ui, Rect::new(right - 172.0, y, 78.0, BUTTON_H), t.eq_bypass, Hit::EqBypass);
    text_button(ui, Rect::new(right - 86.0, y, 62.0, BUTTON_H), t.eq_done, Hit::EqDone);
}

/// The selected band: its number, its filter type at either end, three sliders, and On.
fn band_controls(ui: &mut Ui, eq: &ParamEq, band: usize, graph: Rect, y: f64) {
    let t = ui.t;
    let Some(item) = eq.bands.get(band) else { return };
    let x = graph.x;
    ui.label(Font::bold(14.0), TEXT, x, y + 16.0, &format!("{}", band + 1));
    if band == 0 || band + 1 == eq.bands.len() {
        let kind = if band == 0 { eq.low } else { eq.high };
        let filter = Rect::new(x + 36.0, y, 120.0, BUTTON_H);
        ui.fill(filter, 8.0, draw::CONTROL);
        ui.label_fit(Font::regular(13.0), TEXT, x + 44.0, y + 20.0, 100.0, type_name(t, kind));
        ui.hot(Hit::EqType, filter);
    }
    slider(ui, t.eq_freq, &format_freq(item.freq), EqSlider::Freq, x + 170.0, y, 180.0, freq_t(item.freq));
    slider(ui, t.eq_gain, &format!("{:+.1}", item.gain_db), EqSlider::Gain, x + 360.0, y, 160.0, gain_t(item.gain_db));
    slider(ui, t.eq_q, &format!("{:.2}", item.q), EqSlider::Q, x + 530.0, y, 140.0, q_t(item.q));
    let on = Rect::new(x + graph.w - 72.0, y, 64.0, BUTTON_H);
    ui.fill(on, 8.0, if item.on { draw::accent() } else { draw::CONTROL });
    ui.label_centered(SMALL_BOLD, if item.on { draw::on_accent() } else { MUTED }, on.x, on.w, y + 16.0, t.eq_band);
    ui.hot(Hit::EqBandOn, on);
}

/// The preset and analyzer dropdowns under the band controls.
fn menus_row(panel: &mut Panel, ui: &mut Ui, slot: FxSlot, x: f64, y: f64) {
    const W: f64 = 260.0;
    const H: f64 = 40.0;
    let t = ui.t;
    let lang = panel.lang;
    let caption = slot_mut(panel, slot).map(|effect| menu::preset_caption(effect, lang, t)).unwrap_or_default();
    menu::button(panel, ui, Menu::Preset { bus: slot.bus, index: slot.index }, Rect::new(x, y, W, H), &caption);
    let caption = format!("{}: {}", t.eq_view, menu::view_name(t, panel.config.eq_view));
    menu::button(panel, ui, Menu::EqView, Rect::new(x + W + 12.0, y, W, H), &caption);
}

fn text_button(ui: &mut Ui, rect: Rect, label: &str, hit: Hit) {
    ui.fill(rect, 8.0, draw::CONTROL);
    ui.label_centered(Font::regular(13.0), TEXT, rect.x, rect.w, rect.y + 16.0, label);
    ui.hot(hit, rect);
}

/// A thin slider with its caption and value above it. `t` is the position, 0 to 1.
fn slider(ui: &mut Ui, caption: &str, value: &str, id: EqSlider, x: f64, y: f64, w: f64, t: f64) {
    ui.label(SMALL, MUTED, x, y + 12.0, caption);
    ui.label(SMALL_BOLD, TEXT, x + 72.0, y + 12.0, value);
    let track = Rect::new(x, y + 20.0, w, 8.0);
    ui.fill(track, 4.0, draw::CONTROL);
    ui.fill(track.take_left((w * t).max(4.0)), 4.0, draw::accent());
    // The grab area is taller than the track.
    ui.fader(FaderId::Eq(id), Rect::new(x, track.y - 8.0, w, 24.0), true);
}

/// The dB grid and the response curve.
fn paint_curve(cr: &Context, eq: &ParamEq, graph: Rect) {
    let Rect { x, y, w, h } = graph;
    cr.set_source_rgba(draw::MUTED.r, draw::MUTED.g, draw::MUTED.b, 0.35);
    cr.set_line_width(1.0);
    for db in [-18.0, -12.0, -6.0, 0.0, 6.0, 12.0, 18.0] {
        let py = y + gain_y(db, h);
        cr.move_to(x, py);
        cr.line_to(x + w, py);
    }
    let _ = cr.stroke();
    let n = 240;
    for i in 0..n {
        let freq = 20.0 * (1000.0_f64).powf(i as f64 / (n - 1) as f64);
        let db = if eq.bypass { 0.0 } else { eq_response_db(eq, freq) };
        let px = x + freq_x(freq, w);
        let py = y + gain_y(db, h);
        if i == 0 { cr.move_to(px, py); } else { cr.line_to(px, py); }
    }
    // A dark edge keeps the curve readable on the brightest part of the heat map.
    cr.set_source_rgba(draw::BG.r, draw::BG.g, draw::BG.b, 0.65);
    cr.set_line_width(6.0);
    let _ = cr.stroke_preserve();
    let accent = draw::accent();
    cr.set_source_rgba(accent.r, accent.g, accent.b, 0.95);
    cr.set_line_width(3.0);
    let _ = cr.stroke();
}

fn slot_mut(panel: &mut Panel, slot: FxSlot) -> Option<&mut EffectSlot> {
    super::fx::bus_mut(&mut panel.config, slot.bus).chain.get_mut(slot.index)
}

/// The EQ of that slot. A slot without band data yet gets six flat bands.
fn eq_mut(panel: &mut Panel, slot: FxSlot) -> Option<&mut ParamEq> {
    let effect = slot_mut(panel, slot)?;
    if effect.kind != EffectKind::Eq {
        return None;
    }
    Some(effect.eq.get_or_insert_with(|| ParamEq::new(6)))
}

pub(crate) fn set_points(panel: &mut Panel, slot: FxSlot, points: u8) {
    if let Some(eq) = eq_mut(panel, slot) {
        eq.set_points(points);
    }
}

pub(crate) fn reset(panel: &mut Panel, slot: FxSlot) {
    if let Some(eq) = eq_mut(panel, slot) {
        let points = eq.points;
        *eq = ParamEq::new(points);
    }
}

pub(crate) fn toggle_bypass(panel: &mut Panel, slot: FxSlot) {
    if let Some(eq) = eq_mut(panel, slot) {
        eq.bypass = !eq.bypass;
    }
}

pub(crate) fn toggle_band(panel: &mut Panel, slot: FxSlot) {
    let index = panel.eq_band;
    if let Some(eq) = eq_mut(panel, slot) {
        if let Some(band) = eq.bands.get_mut(index) {
            band.on = !band.on;
        }
    }
}

pub(crate) fn cycle_type(panel: &mut Panel, slot: FxSlot) {
    let index = panel.eq_band;
    if let Some(eq) = eq_mut(panel, slot) {
        let last = eq.bands.len().saturating_sub(1);
        if index == 0 {
            eq.low = match eq.low {
                BandType::Bell => BandType::Shelf,
                BandType::Shelf => BandType::HighPass,
                _ => BandType::Bell,
            };
        } else if index == last {
            eq.high = match eq.high {
                BandType::Bell => BandType::Shelf,
                BandType::Shelf => BandType::LowPass,
                _ => BandType::Bell,
            };
        }
    }
}

pub(crate) fn drag_handle(panel: &mut Panel, slot: FxSlot, index: usize, x: f64, y: f64) {
    let Some(graph) = panel.eq_graph else { return };
    let freq = x_freq(((x - graph.x) / graph.w).clamp(0.0, 1.0));
    let gain = y_gain(((y - graph.y) / graph.h).clamp(0.0, 1.0));
    panel.eq_band = index;
    if let Some(eq) = eq_mut(panel, slot) {
        if let Some(band) = eq.bands.get_mut(index) {
            band.freq = freq.clamp(20.0, 20_000.0);
            band.gain_db = gain.clamp(-18.0, 18.0);
        }
    }
}

pub(crate) fn set_slider(panel: &mut Panel, slot: FxSlot, which: EqSlider, t: f64) {
    let index = panel.eq_band;
    if let Some(eq) = eq_mut(panel, slot) {
        if let Some(band) = eq.bands.get_mut(index) {
            match which {
                EqSlider::Freq => band.freq = x_freq(t),
                EqSlider::Gain => band.gain_db = (t * 36.0 - 18.0).clamp(-18.0, 18.0),
                EqSlider::Q => band.q = (0.1 * 100f64.powf(t)).clamp(0.1, 10.0),
            }
        }
    }
}

pub(crate) fn nudge_q(panel: &mut Panel, slot: FxSlot, delta: f64) {
    let index = panel.eq_band;
    if let Some(eq) = eq_mut(panel, slot) {
        if let Some(band) = eq.bands.get_mut(index) {
            band.q = (band.q * if delta > 0.0 { 0.9 } else { 1.1 }).clamp(0.1, 10.0);
        }
    }
}

fn freq_x(freq: f64, w: f64) -> f64 {
    let t = (freq.max(20.0) / 20.0).ln() / 1000f64.ln();
    t.clamp(0.0, 1.0) * w
}

fn x_freq(t: f64) -> f64 {
    20.0 * 1000f64.powf(t.clamp(0.0, 1.0))
}

fn gain_y(db: f64, h: f64) -> f64 {
    let t = (18.0 - db.clamp(-18.0, 18.0)) / 36.0;
    t * h
}

fn y_gain(t: f64) -> f64 {
    18.0 - t.clamp(0.0, 1.0) * 36.0
}

fn freq_t(freq: f64) -> f64 {
    ((freq.max(20.0) / 20.0).ln() / 1000f64.ln()).clamp(0.0, 1.0)
}

fn gain_t(db: f64) -> f64 {
    ((db.clamp(-18.0, 18.0) + 18.0) / 36.0).clamp(0.0, 1.0)
}

fn q_t(q: f64) -> f64 {
    (q.max(0.1) / 0.1).ln() / 100f64.ln()
}

fn format_freq(freq: f64) -> String {
    if freq >= 1000.0 { format!("{:.2} kHz", freq / 1000.0) } else { format!("{:.0} Hz", freq) }
}

fn type_name(t: &Text, kind: BandType) -> &str {
    match kind {
        BandType::Bell => t.filter_bell,
        BandType::Shelf => t.filter_shelf,
        BandType::HighPass => t.filter_highpass,
        BandType::LowPass => t.filter_lowpass,
    }
}
