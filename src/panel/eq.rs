//! Parametric EQ sheet for an EQ slot in an FX chain. `who` is `fx:{bus}:{index}`.

use super::{backdrop, fill_round, sheet_surface, Panel};
use crate::config::{BandType, EffectKind, EffectSlot, ParamEq};
use crate::draw::{self, Fonts, MUTED, TEXT};
use crate::fx::dsp::eq_response_db;
use crate::i18n::Text;
use cairo::Context;

pub(crate) fn sheet(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, who: &str) {
    let Some(eq) = eq_mut(panel, who).map(|eq| eq.clone()) else {
        panel.overlay = super::Overlay::None;
        return;
    };
    backdrop(panel, cr);
    let (x, y, w, h) = panel.sheet(16.0);
    sheet_surface(cr, x, y, w, h);
    panel.hot("noop", x, y, w, h);
    draw::label(cr, fonts, true, 20.0, TEXT, x + 24.0, y + 36.0, t.eq_title);
    let mut sx = x + w - 24.0 - 520.0;
    for points in [3u8, 6, 8, 12] {
        let on = eq.points == points;
        let fill = if on { draw::accent() } else { draw::CONTROL };
        let ink = if on { draw::on_accent() } else { draw::SOFT };
        fill_round(cr, sx, y + 16.0, 52.0, 32.0, 10.0, fill);
        centered(cr, fonts, true, 14.0, ink, sx, 52.0, y + 32.0, &points.to_string());
        panel.hot(&format!("eq-points:{points}"), sx, y + 16.0, 52.0, 32.0);
        sx += 58.0;
    }
    text_btn(panel, cr, fonts, x + w - 250.0, y + 16.0, 70.0, t.eq_reset, "eq-reset");
    text_btn(panel, cr, fonts, x + w - 172.0, y + 16.0, 78.0, t.eq_bypass, "eq-bypass");
    text_btn(panel, cr, fonts, x + w - 86.0, y + 16.0, 62.0, t.eq_done, "eq-done");

    let gx = x + 28.0;
    let gy = y + 64.0;
    let gw = w - 56.0;
    let gh = (h - 220.0).max(180.0);
    panel.eq_graph = Some((gx, gy, gw, gh));
    fill_round(cr, gx, gy, gw, gh, 12.0, draw::BG);
    let _ = cr.save();
    draw::rounded(cr, gx, gy, gw, gh, 12.0);
    cr.clip();
    panel.spectrum.paint(cr, panel.config.eq_view, gx, gy, gw, gh);
    let _ = cr.restore();
    paint_curve(cr, &eq, gx, gy, gw, gh);
    let band = panel.eq_band.min(eq.bands.len().saturating_sub(1));
    for (index, item) in eq.bands.iter().enumerate() {
        let px = gx + freq_x(item.freq, gw);
        let py = gy + gain_y(item.gain_db, gh);
        let fill = if index == band { draw::accent() } else { TEXT };
        cr.arc(px, py, 11.0, 0.0, std::f64::consts::TAU);
        draw::fill(cr, fill, 1.0);
        let num = (index + 1).to_string();
        centered(cr, fonts, true, 11.0, draw::BG, px - 11.0, 22.0, py, &num);
        panel.hot(&format!("eqband:{index}"), px - 14.0, py - 14.0, 28.0, 28.0);
    }

    let by = gy + gh + 18.0;
    if let Some(item) = eq.bands.get(band) {
        draw::label(cr, fonts, true, 14.0, TEXT, gx, by + 16.0, &format!("{}", band + 1));
        if band == 0 || band + 1 == eq.bands.len() {
            let kind = if band == 0 { eq.low } else { eq.high };
            fill_round(cr, gx + 36.0, by, 120.0, 32.0, 8.0, draw::CONTROL);
            draw::label_fit(cr, fonts, false, 13.0, TEXT, gx + 44.0, by + 20.0, 100.0, type_name(t, kind));
            panel.hot("eq-type", gx + 36.0, by, 120.0, 32.0);
        }
        slider(panel, cr, fonts, t.eq_freq, &format_freq(item.freq), "eq-freq", gx + 170.0, by, 180.0, freq_t(item.freq));
        slider(panel, cr, fonts, t.eq_gain, &format!("{:+.1}", item.gain_db), "eq-gain", gx + 360.0, by, 160.0, gain_t(item.gain_db));
        slider(panel, cr, fonts, t.eq_q, &format!("{:.2}", item.q), "eq-q", gx + 530.0, by, 140.0, q_t(item.q));
        let on_fill = if item.on { draw::accent() } else { draw::CONTROL };
        fill_round(cr, gx + gw - 72.0, by, 64.0, 32.0, 8.0, on_fill);
        centered(cr, fonts, true, 12.0, if item.on { draw::on_accent() } else { MUTED }, gx + gw - 72.0, 64.0, by + 16.0, t.eq_band);
        panel.hot("eq-on", gx + gw - 72.0, by, 64.0, 32.0);
    }
    menus_row(panel, cr, fonts, t, who, gx, by + 46.0);
}

/// The preset and analyzer dropdowns under the band controls.
fn menus_row(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, who: &str, x: f64, y: f64) {
    const W: f64 = 260.0;
    const H: f64 = 40.0;
    let lang = panel.lang;
    if let Some((bus, index)) = fx_position(who) {
        let caption = slot_mut(panel, who).map(|slot| super::menu::preset_caption(slot, lang, t)).unwrap_or_default();
        super::menu::button(panel, cr, fonts, super::menu::Menu::Preset { bus, index }, x, y, W, H, &caption);
    }
    let caption = format!("{}: {}", t.eq_view, super::menu::view_name(t, panel.config.eq_view));
    super::menu::button(panel, cr, fonts, super::menu::Menu::EqView, x + W + 12.0, y, W, H, &caption);
}

/// Effect bus and chain position from a sheet id, `fx:{bus}:{index}`.
pub(crate) fn fx_position(who: &str) -> Option<(usize, usize)> {
    let (bus, index) = who.strip_prefix("fx:")?.split_once(':')?;
    Some((bus.parse().ok()?, index.parse().ok()?))
}

fn text_btn(panel: &mut Panel, cr: &Context, fonts: &Fonts, x: f64, y: f64, w: f64, label: &str, id: &str) {
    fill_round(cr, x, y, w, 32.0, 8.0, draw::CONTROL);
    centered(cr, fonts, false, 13.0, TEXT, x, w, y + 16.0, label);
    panel.hot(id, x, y, w, 32.0);
}

fn centered(cr: &Context, fonts: &Fonts, bold: bool, size: f64, ink: draw::Rgb, x: f64, w: f64, mid: f64, text: &str) {
    let tw = draw::text_width(cr, fonts, bold, size, text);
    draw::label_mid(cr, fonts, bold, size, ink, x + (w - tw) / 2.0, mid, text);
}

fn slider(panel: &mut Panel, cr: &Context, fonts: &Fonts, caption: &str, value: &str, id: &str, x: f64, y: f64, w: f64, t: f64) {
    draw::label(cr, fonts, false, 12.0, MUTED, x, y + 12.0, caption);
    draw::label(cr, fonts, true, 12.0, TEXT, x + 72.0, y + 12.0, value);
    let tx = x;
    let ty = y + 20.0;
    fill_round(cr, tx, ty, w, 8.0, 4.0, draw::CONTROL);
    fill_round(cr, tx, ty, (w * t).max(4.0), 8.0, 4.0, draw::accent());
    panel.fader_hit(id, tx, ty - 8.0, w, 24.0, true);
}

fn paint_curve(cr: &Context, eq: &ParamEq, x: f64, y: f64, w: f64, h: f64) {
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

fn slot_mut<'a>(panel: &'a mut Panel, who: &str) -> Option<&'a mut EffectSlot> {
    let mut parts = who.strip_prefix("fx:")?.split(':');
    let bus: usize = parts.next()?.parse().ok()?;
    let index: usize = parts.next()?.parse().ok()?;
    super::fx::bus_mut(&mut panel.config, bus).chain.get_mut(index)
}

/// The EQ of that slot. A slot without band data yet gets six flat bands.
fn eq_mut<'a>(panel: &'a mut Panel, who: &str) -> Option<&'a mut ParamEq> {
    let slot = slot_mut(panel, who)?;
    if slot.kind != EffectKind::Eq {
        return None;
    }
    Some(slot.eq.get_or_insert_with(|| ParamEq::new(6)))
}

pub(crate) fn set_points(panel: &mut Panel, who: &str, points: u8) {
    if let Some(eq) = eq_mut(panel, who) {
        eq.set_points(points);
    }
}

pub(crate) fn reset(panel: &mut Panel, who: &str) {
    if let Some(eq) = eq_mut(panel, who) {
        let points = eq.points;
        *eq = ParamEq::new(points);
    }
}

pub(crate) fn toggle_bypass(panel: &mut Panel, who: &str) {
    if let Some(eq) = eq_mut(panel, who) {
        eq.bypass = !eq.bypass;
    }
}

pub(crate) fn toggle_band(panel: &mut Panel, who: &str) {
    let index = panel.eq_band;
    if let Some(eq) = eq_mut(panel, who) {
        if let Some(band) = eq.bands.get_mut(index) {
            band.on = !band.on;
        }
    }
}

pub(crate) fn cycle_type(panel: &mut Panel, who: &str) {
    let index = panel.eq_band;
    if let Some(eq) = eq_mut(panel, who) {
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

pub(crate) fn drag_handle(panel: &mut Panel, who: &str, index: usize, x: f64, y: f64) {
    let Some((gx, gy, gw, gh)) = panel.eq_graph else { return };
    let freq = x_freq(((x - gx) / gw).clamp(0.0, 1.0));
    let gain = y_gain(((y - gy) / gh).clamp(0.0, 1.0));
    panel.eq_band = index;
    if let Some(eq) = eq_mut(panel, who) {
        if let Some(band) = eq.bands.get_mut(index) {
            band.freq = freq.clamp(20.0, 20_000.0);
            band.gain_db = gain.clamp(-18.0, 18.0);
        }
    }
}

pub(crate) fn set_slider(panel: &mut Panel, who: &str, which: &str, t: f64) {
    let index = panel.eq_band;
    if let Some(eq) = eq_mut(panel, who) {
        if let Some(band) = eq.bands.get_mut(index) {
            match which {
                "freq" => band.freq = x_freq(t),
                "gain" => band.gain_db = (t * 36.0 - 18.0).clamp(-18.0, 18.0),
                "q" => band.q = (0.1 * 100f64.powf(t)).clamp(0.1, 10.0),
                _ => {}
            }
        }
    }
}

pub(crate) fn nudge_q(panel: &mut Panel, who: &str, delta: f64) {
    let index = panel.eq_band;
    if let Some(eq) = eq_mut(panel, who) {
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
