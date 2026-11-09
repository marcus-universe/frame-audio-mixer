//! The effect picker (built-in effects and VST plugins in two sections) and the
//! VST settings sheet (folder, scan, and what the scan found).

use super::{backdrop, close_button, fill_round, sheet_surface, Panel};
use crate::config::EffectKind;
use crate::draw::{self, ButtonKind, Fonts, Look, Rgb, MUTED, R_CONTROL, TEXT};
use crate::i18n::{self, Text};
use crate::vst::{VstArch, VstInfo};
use cairo::Context;

const TILE_H: f64 = 76.0;
const TILE_GAP: f64 = 12.0;
const ROW_H: f64 = 60.0;
const ROW_GAP: f64 = 8.0;
const PAD: f64 = 28.0;

fn arch_chip(t: &Text, arch: VstArch) -> (&'static str, Rgb, Rgb) {
    match arch {
        VstArch::Native => (t.vst_native, draw::CHIP_OK_FILL, draw::CHIP_OK_TEXT),
        VstArch::Windows => (t.vst_windows, draw::accent_tint(), draw::accent()),
        VstArch::Unsupported => (t.vst_unsupported, draw::CONTROL, MUTED),
    }
}

fn chip(cr: &Context, fonts: &Fonts, right: f64, mid: f64, label: &str, fill: Rgb, ink: Rgb) -> f64 {
    let w = draw::text_width(cr, fonts, true, 12.0, label) + 24.0;
    fill_round(cr, right - w, mid - 13.0, w, 26.0, 13.0, fill);
    draw::label_mid(cr, fonts, true, 12.0, ink, right - w + 12.0, mid, label);
    w
}

fn hover_fill(panel: &Panel, x: f64, y: f64, w: f64, h: f64, rest: Rgb) -> Rgb {
    match panel.look(x, y, w, h) {
        Look::Down => draw::CONTROL_DOWN,
        Look::Hover => draw::CONTROL_HOVER,
        Look::Rest => rest,
    }
}

/// Scrolls `content` inside `(top, view_h)` and returns the offset to draw at.
fn clamp_scroll(panel: &mut Panel, content: f64, view_h: f64) -> f64 {
    panel.dialog_scroll = panel.dialog_scroll.clamp(0.0, (content - view_h).max(0.0));
    panel.dialog_scroll
}

/// A hot area cut to the visible part of a scrolled list.
fn clipped_hot(panel: &mut Panel, id: &str, x: f64, y: f64, w: f64, h: f64, top: f64, bottom: f64) {
    let (y0, y1) = (y.max(top), (y + h).min(bottom));
    if y1 > y0 {
        panel.hot(id, x, y0, w, y1 - y0);
    }
}

fn plugin_row(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, info: &VstInfo, x: f64, y: f64, w: f64, id: Option<&str>, view: (f64, f64)) {
    let usable = info.arch.runs() && id.is_some();
    let fill = if usable { hover_fill(panel, x, y, w, ROW_H, draw::CONTROL) } else { draw::mix(draw::CONTROL, draw::INSET, 0.5) };
    fill_round(cr, x, y, w, ROW_H, R_CONTROL, fill);
    let (label, chip_fill, chip_ink) = arch_chip(t, info.arch);
    let chip_w = chip(cr, fonts, x + w - 14.0, y + ROW_H / 2.0, label, chip_fill, chip_ink);
    let text_w = w - chip_w - 44.0;
    let ink = if info.arch.runs() { TEXT } else { MUTED };
    if info.vendor.is_empty() {
        draw::label_fit(cr, fonts, true, 15.0, ink, x + 16.0, y + ROW_H / 2.0 + 5.0, text_w, &info.name);
    } else {
        draw::label_fit(cr, fonts, true, 15.0, ink, x + 16.0, y + 26.0, text_w, &info.name);
        draw::label_fit(cr, fonts, false, 12.5, MUTED, x + 16.0, y + 45.0, text_w, &info.vendor);
    }
    if let (true, Some(id)) = (usable, id) {
        clipped_hot(panel, id, x, y, w, ROW_H, view.0, view.1);
    }
}

/// Built-in effects as tiles, then every plugin the last scan found.
pub(crate) fn picker(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, bus: usize) {
    backdrop(panel, cr);
    let (x, y, w, h) = panel.dialog_rect(860.0, 700.0, 24.0);
    sheet_surface(cr, x, y, w, h);
    panel.hot("noop", x, y, w, h);
    draw::label_mid(cr, fonts, true, 20.0, TEXT, x + PAD, y + 44.0, t.add_effect);
    close_button(panel, cr, x + w - 60.0, y + 24.0);

    let inner = w - 2.0 * PAD;
    let columns = if inner > 620.0 { 4 } else { 3 };
    let tile_w = (inner - (columns - 1) as f64 * TILE_GAP) / columns as f64;
    let builtin_rows = EffectKind::ALL.len().div_ceil(columns);
    let plugins = panel.vst_plugins.clone();
    let list_top = y + 84.0;
    let list_bottom = y + h - 24.0;
    let builtin_h = 34.0 + builtin_rows as f64 * (TILE_H + TILE_GAP);
    let vst_h = 34.0 + if plugins.is_empty() { 120.0 } else { plugins.len() as f64 * (ROW_H + ROW_GAP) };
    let scroll = clamp_scroll(panel, builtin_h + 20.0 + vst_h, list_bottom - list_top);
    let _ = cr.save();
    cr.rectangle(x, list_top, w, list_bottom - list_top);
    cr.clip();
    let view = (list_top, list_bottom);

    let mut cy = list_top - scroll;
    draw::label(cr, fonts, true, 15.0, MUTED, x + PAD, cy + 18.0, t.builtin);
    cy += 34.0;
    for (index, kind) in EffectKind::ALL.into_iter().enumerate() {
        let tx = x + PAD + (index % columns) as f64 * (tile_w + TILE_GAP);
        let ty = cy + (index / columns) as f64 * (TILE_H + TILE_GAP);
        fill_round(cr, tx, ty, tile_w, TILE_H, R_CONTROL, hover_fill(panel, tx, ty, tile_w, TILE_H, draw::CONTROL));
        fill_round(cr, tx + 14.0, ty + 18.0, 6.0, TILE_H - 36.0, 3.0, draw::effect_color(kind));
        draw::label_fit(cr, fonts, true, 15.0, TEXT, tx + 32.0, ty + 33.0, tile_w - 44.0, i18n::effect_name(t, kind));
        let count = crate::fx::presets::of(kind).len().to_string();
        let presets = super::fmt_token(t.presets_fmt, "n", &count);
        draw::label_fit(cr, fonts, false, 12.5, MUTED, tx + 32.0, ty + 54.0, tile_w - 44.0, &presets);
        clipped_hot(panel, &format!("fxadd:{bus}:{}", kind.key()), tx, ty, tile_w, TILE_H, view.0, view.1);
    }
    cy += builtin_rows as f64 * (TILE_H + TILE_GAP) + 20.0;

    draw::label(cr, fonts, true, 15.0, MUTED, x + PAD, cy + 18.0, t.vst_section);
    let gear_w = (draw::text_width(cr, fonts, true, 14.0, t.vst_setup) + 36.0).min(inner / 2.0);
    let (gx, gy) = (x + w - PAD - gear_w, cy - 4.0);
    panel.button(cr, fonts, gx, gy, gear_w, 32.0, t.vst_setup, ButtonKind::Plain);
    clipped_hot(panel, "vst-open", gx, gy, gear_w, 32.0, view.0, view.1);
    cy += 34.0;
    if plugins.is_empty() {
        fill_round(cr, x + PAD, cy, inner, 104.0, R_CONTROL, draw::mix(draw::CONTROL, draw::INSET, 0.5));
        let lines = draw::wrap(cr, fonts, false, 14.0, inner - 40.0, 3, t.vst_empty_picker);
        for (index, line) in lines.iter().enumerate() {
            draw::label(cr, fonts, false, 14.0, MUTED, x + PAD + 20.0, cy + 34.0 + index as f64 * 20.0, line);
        }
    } else {
        for (index, info) in plugins.iter().enumerate() {
            let id = format!("fxaddvst:{bus}:{index}");
            plugin_row(panel, cr, fonts, t, info, x + PAD, cy, inner, Some(&id), view);
            cy += ROW_H + ROW_GAP;
        }
    }
    let _ = cr.restore();
}

/// Folder, Scan, and the scan's result with what can run where.
pub(crate) fn settings(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text) {
    backdrop(panel, cr);
    let (x, y, w, h) = panel.dialog_rect(780.0, 700.0, 24.0);
    sheet_surface(cr, x, y, w, h);
    panel.hot("noop", x, y, w, h);
    draw::label_mid(cr, fonts, true, 20.0, TEXT, x + PAD, y + 44.0, t.vst_plugins);
    close_button(panel, cr, x + w - 60.0, y + 24.0);
    let inner = w - 2.0 * PAD;
    let mut cy = y + 80.0;
    for line in draw::wrap(cr, fonts, false, 13.5, inner, 3, t.vst_hint) {
        draw::label(cr, fonts, false, 13.5, MUTED, x + PAD, cy, &line);
        cy += 19.0;
    }
    cy += 14.0;

    let scan_w = (draw::text_width(cr, fonts, true, 15.0, t.vst_scan) + 56.0).max(120.0);
    let field = (x + PAD, cy, inner - scan_w - 12.0, 52.0);
    fill_round(cr, field.0, field.1, field.2, field.3, R_CONTROL, hover_fill(panel, field.0, field.1, field.2, field.3, draw::CONTROL));
    draw::icon(cr, draw::FOLDER_PATH, 24.0, field.0 + 16.0, field.1 + 15.0, 22.0, draw::SOFT);
    draw::label(cr, fonts, false, 12.0, MUTED, field.0 + 52.0, field.1 + 20.0, t.vst_folder);
    let folder = panel.config.vst_folder.clone();
    draw::label_tail(cr, fonts, true, 15.0, TEXT, field.0 + 52.0, field.1 + 40.0, field.2 - 68.0, &folder);
    panel.hot("vst-folder", field.0, field.1, field.2, field.3);
    panel.button(cr, fonts, field.0 + field.2 + 12.0, cy + 2.0, scan_w, 48.0, t.vst_scan, ButtonKind::Accent);
    panel.hot("vst-scan", field.0 + field.2 + 12.0, cy + 2.0, scan_w, 48.0);
    cy += 72.0;

    let plugins = panel.vst_plugins.clone();
    let usable = plugins.iter().filter(|info| info.arch.runs()).count();
    let count = super::fmt_token(&super::fmt_token(t.vst_found_fmt, "found", &plugins.len().to_string()), "usable", &usable.to_string());
    draw::label(cr, fonts, true, 14.0, MUTED, x + PAD, cy, if plugins.is_empty() { t.vst_none } else { &count });
    cy += 16.0;

    let (top, bottom) = (cy, y + h - 24.0);
    let scroll = clamp_scroll(panel, plugins.len() as f64 * (ROW_H + ROW_GAP), bottom - top);
    let _ = cr.save();
    cr.rectangle(x, top, w, bottom - top);
    cr.clip();
    for (index, info) in plugins.iter().enumerate() {
        let ry = top + index as f64 * (ROW_H + ROW_GAP) - scroll;
        if ry + ROW_H < top || ry > bottom {
            continue;
        }
        plugin_row(panel, cr, fonts, t, info, x + PAD, ry, inner, None, (top, bottom));
    }
    let _ = cr.restore();
}
