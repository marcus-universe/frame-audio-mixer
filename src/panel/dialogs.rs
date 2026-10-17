//! Add-a-channel, the microphone picker, and App Settings.

use super::{backdrop, fill_round, paint_desktop_icon, source_label, stroke_round, Panel, SUGGESTIONS};
use crate::draw::{self, ButtonKind, Fonts, BORDER, CARD, INSET, MUTED, TEXT};
use crate::graph::Graph;
use crate::i18n::Text;
use crate::worker::Snapshot;
use cairo::Context;

pub(crate) fn add(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot) {
    backdrop(panel, cr);
    let (x, y, w, h) = (48.0, 36.0, 1104.0, 710.0);
    fill_round(cr, x, y, w, h, 18.0, CARD);
    stroke_round(cr, x, y, w, h, 18.0, BORDER, 2.0);
    let mid = x + w / 2.0;
    cr.set_source_rgb(draw::DIVIDER.r, draw::DIVIDER.g, draw::DIVIDER.b);
    cr.rectangle(mid, y + 24.0, 1.0, h - 48.0);
    let _ = cr.fill();
    draw::label(cr, fonts, true, 20.0, TEXT, x + 28.0, y + 48.0, t.suggest);
    let mut row = y + 72.0;
    for name in SUGGESTIONS {
        row_button(panel, cr, fonts, x + 24.0, row, mid - x - 48.0, name, &format!("suggest:{name}"));
        row += 58.0;
    }
    row_button(panel, cr, fonts, x + 24.0, row, mid - x - 48.0, t.custom, "custom");

    draw::label(cr, fonts, true, 20.0, TEXT, mid + 28.0, y + 48.0, t.running);
    panel.button(cr, fonts, x + w - 160.0, y + 24.0, 120.0, 40.0, t.refresh, ButtonKind::Plain);
    panel.hot("refresh", x + w - 160.0, y + 24.0, 120.0, 40.0);
    let list_top = y + 80.0;
    let list_h = h - 110.0;
    let apps = snap.graph.playback_apps();
    let content = apps.len() as f64 * 58.0;
    panel.dialog_scroll = panel.dialog_scroll.clamp(0.0, (content - list_h).max(0.0));
    let _ = cr.save();
    cr.rectangle(mid + 16.0, list_top, w / 2.0 - 40.0, list_h);
    let _ = cr.clip();
    let mut ry = list_top - panel.dialog_scroll;
    if apps.is_empty() {
        draw::label(cr, fonts, false, 14.0, MUTED, mid + 28.0, list_top + 28.0, t.pick_channel);
    }
    for node in &apps {
        let name = if node.app_name.is_empty() { node.binary.clone() } else { node.app_name.clone() };
        if name.is_empty() || ry + 52.0 < list_top || ry > list_top + list_h {
            ry += 58.0;
            continue;
        }
        fill_round(cr, mid + 24.0, ry, w / 2.0 - 64.0, 52.0, 14.0, INSET);
        paint_desktop_icon(panel, cr, fonts, &name, &node.binary, mid + 36.0, ry + 10.0, 32.0);
        draw::label_fit(cr, fonts, true, 16.0, TEXT, mid + 80.0, ry + 32.0, w / 2.0 - 160.0, &name);
        panel.hot(&format!("use:{name}"), mid + 24.0, ry, w / 2.0 - 64.0, 52.0);
        ry += 58.0;
    }
    let _ = cr.restore();
    // Swallow clicks on the card so they don't dismiss it.
    panel.hot("noop", x, y, w, h);
}

pub(crate) fn mic(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot) {
    backdrop(panel, cr);
    let (x, y, w, h) = (160.0, 140.0, 880.0, 440.0);
    fill_round(cr, x, y, w, h, 18.0, CARD);
    stroke_round(cr, x, y, w, h, 18.0, BORDER, 2.0);
    draw::label(cr, fonts, true, 22.0, TEXT, x + 28.0, y + 44.0, t.mic_picker);
    let current = source_label(&panel.config.mic_source, &snap.graph, t);
    draw::label_fit(cr, fonts, false, 14.0, MUTED, x + 28.0, y + 68.0, w - 56.0, &current);
    let mut row = y + 96.0;
    let rows = mic_rows(panel, t, snap);
    let list_h = h - 120.0;
    panel.dialog_scroll = panel.dialog_scroll.clamp(0.0, (rows.len() as f64 * 58.0 - list_h).max(0.0));
    let _ = cr.save();
    cr.rectangle(x + 16.0, row, w - 32.0, list_h);
    let _ = cr.clip();
    row -= panel.dialog_scroll;
    for (id, label, hint) in rows {
        if row + 52.0 >= y + 90.0 && row <= y + h - 16.0 {
            let selected = (id == "mic:system" && panel.config.mic_source.is_empty())
                || (id == "mic:builtin" && panel.config.mic_source == "builtin")
                || id.strip_prefix("mic:") == Some(panel.config.mic_source.as_str());
            fill_round(cr, x + 24.0, row, w - 48.0, 52.0, 14.0, if selected { draw::CONTROL } else { INSET });
            if selected {
                stroke_round(cr, x + 24.0, row, w - 48.0, 52.0, 14.0, draw::ACCENT, 2.0);
            }
            draw::label_fit(cr, fonts, true, 16.0, TEXT, x + 40.0, row + 22.0, w - 80.0, &label);
            if !hint.is_empty() {
                draw::label_fit(cr, fonts, false, 12.0, MUTED, x + 40.0, row + 42.0, w - 80.0, &hint);
            }
            panel.hot(&id, x + 24.0, row, w - 48.0, 52.0);
        }
        row += 58.0;
    }
    let _ = cr.restore();
    panel.hot("noop", x, y, w, h);
}

fn mic_rows(panel: &Panel, t: &Text, snap: &Snapshot) -> Vec<(String, String, String)> {
    let _ = panel;
    let mut rows = vec![
        ("mic:system".into(), t.mic_system.to_string(), String::new()),
        ("mic:builtin".into(), t.mic_builtin.to_string(), t.mic_after.to_string()),
    ];
    for node in snap.graph.pickable_sources() {
        let label = if node.description.is_empty() { node.name.clone() } else { node.description.clone() };
        rows.push((format!("mic:{}", node.name), label, node.name));
    }
    rows
}

pub(crate) fn settings(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text) {
    backdrop(panel, cr);
    let (x, y, w, h) = (300.0, 220.0, 600.0, 240.0);
    fill_round(cr, x, y, w, h, 18.0, CARD);
    stroke_round(cr, x, y, w, h, 18.0, BORDER, 2.0);
    draw::label(cr, fonts, true, 22.0, TEXT, x + 28.0, y + 48.0, t.settings);
    draw::label(cr, fonts, true, 16.0, TEXT, x + 28.0, y + 110.0, t.meters);
    let on = panel.config.meters;
    let seg_x = x + w - 220.0;
    fill_round(cr, seg_x, y + 84.0, 180.0, 48.0, 14.0, draw::CONTROL);
    if on {
        fill_round(cr, seg_x + 4.0, y + 88.0, 86.0, 40.0, 12.0, draw::ACCENT);
    } else {
        fill_round(cr, seg_x + 90.0, y + 88.0, 86.0, 40.0, 12.0, draw::ACCENT);
    }
    draw::label(cr, fonts, true, 16.0, if on { draw::ON_ACCENT } else { draw::SOFT }, seg_x + 28.0, y + 114.0, t.on);
    draw::label(cr, fonts, true, 16.0, if on { draw::SOFT } else { draw::ON_ACCENT }, seg_x + 118.0, y + 114.0, t.off);
    panel.hot("meters-on", seg_x, y + 84.0, 90.0, 48.0);
    panel.hot("meters-off", seg_x + 90.0, y + 84.0, 90.0, 48.0);
    draw::label_fit(cr, fonts, false, 13.0, MUTED, x + 28.0, y + 170.0, w - 56.0, t.mix_hint);
    panel.hot("noop", x, y, w, h);
}

fn row_button(panel: &mut Panel, cr: &Context, fonts: &Fonts, x: f64, y: f64, w: f64, label: &str, id: &str) {
    fill_round(cr, x, y, w, 52.0, 14.0, INSET);
    draw::label_fit(cr, fonts, true, 16.0, TEXT, x + 16.0, y + 32.0, w - 32.0, label);
    panel.hot(id, x, y, w, 52.0);
}

/// Keep the builtin constant referenced so a rename fails here too.
#[allow(dead_code)]
fn _builtin() -> &'static str {
    Graph::BUILTIN_SOURCE
}
