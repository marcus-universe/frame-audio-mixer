//! The effect picker (built-in effects and VST plugins in two sections) and the
//! VST settings sheet (folder, scan, and what the scan found).

use super::{Hit, HitBounds, Panel, Ui};
use crate::config::EffectKind;
use crate::draw::{self, ButtonKind, Font, Rect, Rgb, BUTTON, HEADING, MUTED, R_CONTROL, TEXT};
use crate::i18n::{self, Text};
use crate::vst::{VstArch, VstInfo};

const TILE_H: f64 = 76.0;
const TILE_GAP: f64 = 12.0;
const ROW_H: f64 = 60.0;
const ROW_GAP: f64 = 8.0;
const PAD: f64 = 28.0;
/// A section title and the space under it.
const SECTION_H: f64 = 34.0;
const SECTION: Font = Font::bold(15.0);
/// The second line of a tile or a plugin row.
const DETAIL: Font = Font::regular(12.5);
/// The tone of a row that cannot be pressed.
fn disabled_fill() -> Rgb {
    draw::mix(draw::CONTROL, draw::INSET, 0.5)
}

fn arch_chip(t: &Text, arch: VstArch) -> (&'static str, Rgb, Rgb) {
    match arch {
        VstArch::Native => (t.vst_native, draw::CHIP_OK_FILL, draw::CHIP_OK_TEXT),
        VstArch::Windows => (t.vst_windows, draw::accent_tint(), draw::accent()),
        VstArch::Unsupported => (t.vst_unsupported, draw::CONTROL, MUTED),
    }
}

/// A small pill ending at `right`, centered on `mid`. Returns its width.
fn chip(ui: &Ui, right: f64, mid: f64, label: &str, fill: Rgb, ink: Rgb) -> f64 {
    let font = Font::bold(12.0);
    let w = ui.text_width(font, label) + 24.0;
    ui.fill(Rect::new(right - w, mid - 13.0, w, 26.0), 13.0, fill);
    ui.label_mid(font, ink, right - w + 12.0, mid, label);
    w
}

/// Keeps the dialog's scroll inside `content` for a list `view_h` tall, and returns it.
fn clamp_scroll(panel: &mut Panel, content: f64, view_h: f64) -> f64 {
    panel.dialog_scroll = panel.dialog_scroll.clamp(0.0, (content - view_h).max(0.0));
    panel.dialog_scroll
}

/// One scanned plugin: name, vendor, and where it can run. `hit` makes a usable one pressable.
fn plugin_row(ui: &mut Ui, info: &VstInfo, row: Rect, hit: Option<Hit>) {
    let Rect { x, y, w, .. } = row;
    let usable = info.arch.runs() && hit.is_some();
    ui.fill(row, R_CONTROL, if usable { ui.control_fill(row, draw::CONTROL) } else { disabled_fill() });
    let (label, chip_fill, chip_ink) = arch_chip(ui.t, info.arch);
    let chip_w = chip(ui, x + w - 14.0, y + ROW_H / 2.0, label, chip_fill, chip_ink);
    let text_w = w - chip_w - 44.0;
    let ink = if info.arch.runs() { TEXT } else { MUTED };
    if info.vendor.is_empty() {
        ui.label_fit(BUTTON, ink, x + 16.0, y + ROW_H / 2.0 + 5.0, text_w, &info.name);
    } else {
        ui.label_fit(BUTTON, ink, x + 16.0, y + 26.0, text_w, &info.name);
        ui.label_fit(DETAIL, MUTED, x + 16.0, y + 45.0, text_w, &info.vendor);
    }
    if let (true, Some(hit)) = (usable, hit) {
        ui.hot(hit, row);
    }
}

/// Built-in effects as tiles, then every plugin the last scan found.
pub(crate) fn picker(panel: &mut Panel, ui: &mut Ui, bus: usize) {
    let t = ui.t;
    let sheet = panel.dialog_rect(860.0, 700.0, 24.0);
    ui.dialog(panel.window(), sheet);
    ui.label_mid(HEADING, TEXT, sheet.x + PAD, sheet.y + 44.0, t.add_effect);
    ui.close_button(sheet.x + sheet.w - 60.0, sheet.y + 24.0);

    let left = sheet.x + PAD;
    let inner = sheet.w - 2.0 * PAD;
    let columns = if inner > 620.0 { 4 } else { 3 };
    let tile_w = (inner - (columns - 1) as f64 * TILE_GAP) / columns as f64;
    let builtin_rows = EffectKind::ALL.len().div_ceil(columns);
    let plugins = panel.vst_plugins.clone();
    let list_top = sheet.y + 84.0;
    let list_bottom = sheet.y + sheet.h - 24.0;
    let builtin_h = SECTION_H + builtin_rows as f64 * (TILE_H + TILE_GAP);
    let vst_h = SECTION_H + if plugins.is_empty() { 120.0 } else { plugins.len() as f64 * (ROW_H + ROW_GAP) };
    let scroll = clamp_scroll(panel, builtin_h + 20.0 + vst_h, list_bottom - list_top);
    ui.clip(Rect::new(sheet.x, list_top, sheet.w, list_bottom - list_top));
    ui.bounds = Some(HitBounds::rows(list_top, list_bottom));

    let mut cy = list_top - scroll;
    ui.label(SECTION, MUTED, left, cy + 18.0, t.builtin);
    cy += SECTION_H;
    for (index, kind) in EffectKind::ALL.into_iter().enumerate() {
        let tile = Rect::new(
            left + (index % columns) as f64 * (tile_w + TILE_GAP),
            cy + (index / columns) as f64 * (TILE_H + TILE_GAP),
            tile_w,
            TILE_H,
        );
        ui.fill(tile, R_CONTROL, ui.control_fill(tile, draw::CONTROL));
        ui.fill(Rect::new(tile.x + 14.0, tile.y + 18.0, 6.0, TILE_H - 36.0), 3.0, draw::effect_color(kind));
        ui.label_fit(BUTTON, TEXT, tile.x + 32.0, tile.y + 33.0, tile_w - 44.0, i18n::effect_name(t, kind));
        let count = crate::fx::presets::of(kind).len().to_string();
        let presets = super::fmt_token(t.presets_fmt, "n", &count);
        ui.label_fit(DETAIL, MUTED, tile.x + 32.0, tile.y + 54.0, tile_w - 44.0, &presets);
        ui.hot(Hit::FxAdd { bus, kind }, tile);
    }
    cy += builtin_rows as f64 * (TILE_H + TILE_GAP) + 20.0;

    ui.label(SECTION, MUTED, left, cy + 18.0, t.vst_section);
    let setup_w = (ui.text_width(Font::bold(14.0), t.vst_setup) + 36.0).min(inner / 2.0);
    let setup = Rect::new(sheet.x + sheet.w - PAD - setup_w, cy - 4.0, setup_w, 32.0);
    ui.button_hot(setup, t.vst_setup, ButtonKind::Plain, Hit::VstSettings);
    cy += SECTION_H;
    if plugins.is_empty() {
        ui.fill(Rect::new(left, cy, inner, 104.0), R_CONTROL, disabled_fill());
        let hint = Font::regular(14.0);
        for (index, line) in ui.wrap(hint, inner - 40.0, 3, t.vst_empty_picker).iter().enumerate() {
            ui.label(hint, MUTED, left + 20.0, cy + 34.0 + index as f64 * 20.0, line);
        }
    } else {
        for (plugin, info) in plugins.iter().enumerate() {
            plugin_row(ui, info, Rect::new(left, cy, inner, ROW_H), Some(Hit::FxAddVst { bus, plugin }));
            cy += ROW_H + ROW_GAP;
        }
    }
    ui.bounds = None;
    ui.unclip();
}

/// Folder, Scan, and the scan's result with what can run where.
pub(crate) fn settings(panel: &mut Panel, ui: &mut Ui) {
    let t = ui.t;
    let sheet = panel.dialog_rect(780.0, 700.0, 24.0);
    ui.dialog(panel.window(), sheet);
    ui.label_mid(HEADING, TEXT, sheet.x + PAD, sheet.y + 44.0, t.vst_plugins);
    ui.close_button(sheet.x + sheet.w - 60.0, sheet.y + 24.0);
    let left = sheet.x + PAD;
    let inner = sheet.w - 2.0 * PAD;
    let mut cy = sheet.y + 80.0;
    let hint = Font::regular(13.5);
    for line in ui.wrap(hint, inner, 3, t.vst_hint) {
        ui.label(hint, MUTED, left, cy, &line);
        cy += 19.0;
    }
    cy += 14.0;

    // The folder field opens the keyboard; Scan sits beside it.
    let scan_w = (ui.text_width(BUTTON, t.vst_scan) + 56.0).max(120.0);
    let field = Rect::new(left, cy, inner - scan_w - 12.0, 52.0);
    ui.fill(field, R_CONTROL, ui.control_fill(field, draw::CONTROL));
    ui.icon(draw::FOLDER_PATH, 24.0, field.x + 16.0, field.y + 15.0, 22.0, draw::SOFT);
    ui.label(Font::regular(12.0), MUTED, field.x + 52.0, field.y + 20.0, t.vst_folder);
    ui.label_tail(BUTTON, TEXT, field.x + 52.0, field.y + 40.0, field.w - 68.0, &panel.config.vst_folder);
    ui.hot(Hit::VstFolder, field);
    let scan = Rect::new(field.x + field.w + 12.0, cy + 2.0, scan_w, 48.0);
    ui.button_hot(scan, t.vst_scan, ButtonKind::Accent, Hit::VstScan);
    cy += 72.0;

    let plugins = panel.vst_plugins.clone();
    let usable = plugins.iter().filter(|info| info.arch.runs()).count();
    let count = super::fmt_token(&super::fmt_token(t.vst_found_fmt, "found", &plugins.len().to_string()), "usable", &usable.to_string());
    ui.label(Font::bold(14.0), MUTED, left, cy, if plugins.is_empty() { t.vst_none } else { &count });
    cy += 16.0;

    let (top, bottom) = (cy, sheet.y + sheet.h - 24.0);
    let scroll = clamp_scroll(panel, plugins.len() as f64 * (ROW_H + ROW_GAP), bottom - top);
    ui.clip(Rect::new(sheet.x, top, sheet.w, bottom - top));
    for (index, info) in plugins.iter().enumerate() {
        let y = top + index as f64 * (ROW_H + ROW_GAP) - scroll;
        if y + ROW_H < top || y > bottom {
            continue;
        }
        plugin_row(ui, info, Rect::new(left, y, inner, ROW_H), None);
    }
    ui.unclip();
}
