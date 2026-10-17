//! Add-a-channel, the microphone picker, and App Settings.

use super::{backdrop, fill_round, fmt_token, paint_desktop_icon, source_label, stroke_round, Panel, SUGGESTIONS};
use crate::draw::{self, ButtonKind, Fonts, BORDER, CARD, INSET, MUTED, TEXT};
use crate::graph::Graph;
use crate::i18n::Text;
use crate::worker::Snapshot;
use cairo::Context;

pub(crate) fn add(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot) {
    backdrop(panel, cr);
    let (x, y, w, h) = panel.sheet(28.0);
    fill_round(cr, x, y, w, h, 18.0, CARD);
    stroke_round(cr, x, y, w, h, 18.0, BORDER, 2.0);
    panel.hot("noop", x, y, w, h);
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

    let choosing = panel.pin_app.clone();
    let heading = if choosing.is_some() { t.picker } else { t.running };
    let heading_w = if choosing.is_some() { w / 2.0 - 56.0 } else { w / 2.0 - 220.0 };
    draw::label_fit(cr, fonts, true, 20.0, TEXT, mid + 28.0, y + 48.0, heading_w, heading);
    if choosing.is_none() {
        panel.button(cr, fonts, x + w - 160.0, y + 24.0, 120.0, 40.0, t.refresh, ButtonKind::Plain);
        panel.hot("refresh", x + w - 160.0, y + 24.0, 120.0, 40.0);
    }
    let list_top = y + 80.0;
    let list_h = h - 110.0;
    let row_w = w / 2.0 - 64.0;
    if let Some(app) = choosing {
        draw::label_fit(cr, fonts, false, 13.0, MUTED, mid + 28.0, y + 70.0, row_w, &app);
        let channels = panel.config.channels.clone();
        let content = channels.len() as f64 * 58.0 + 2.0 * 58.0;
        panel.dialog_scroll = panel.dialog_scroll.clamp(0.0, (content - list_h).max(0.0));
        let _ = cr.save();
        cr.rectangle(mid + 16.0, list_top, w / 2.0 - 40.0, list_h);
        let _ = cr.clip();
        let mut ry = list_top - panel.dialog_scroll;
        row_button(panel, cr, fonts, mid + 24.0, ry, row_w, &format!("‹  {}", t.back_mixer), "pin-back");
        ry += 58.0;
        let binary = panel.app_binaries.iter().find(|(name, _)| *name == app).map(|(_, b)| b.clone()).unwrap_or_default();
        let new_label = fmt_token(t.new_channel_fmt, "name", &app);
        icon_row(panel, cr, fonts, mid + 24.0, ry, row_w, (&app, &binary), &new_label, "pin-new", true);
        ry += 58.0;
        for channel in &channels {
            if ry + 52.0 >= list_top && ry <= list_top + list_h {
                let id = format!("pin:{}", channel.id);
                if channel.app.is_empty() {
                    row_button(panel, cr, fonts, mid + 24.0, ry, row_w, &channel.name, &id);
                } else {
                    icon_row(panel, cr, fonts, mid + 24.0, ry, row_w, (&channel.app, &channel.binary), &channel.name, &id, false);
                }
            }
            ry += 58.0;
        }
        let _ = cr.restore();
        return;
    }
    let apps = snap.graph.picker_apps();
    panel.app_binaries = apps.iter().map(|app| (app.name.clone(), app.binary.clone())).collect();
    let content = apps.len() as f64 * 58.0;
    panel.dialog_scroll = panel.dialog_scroll.clamp(0.0, (content - list_h).max(0.0));
    let _ = cr.save();
    cr.rectangle(mid + 16.0, list_top, w / 2.0 - 40.0, list_h);
    let _ = cr.clip();
    let mut ry = list_top - panel.dialog_scroll;
    for app in &apps {
        let name = app.name.clone();
        if ry + 52.0 < list_top || ry > list_top + list_h {
            ry += 58.0;
            continue;
        }
        fill_round(cr, mid + 24.0, ry, row_w, 52.0, 14.0, INSET);
        paint_desktop_icon(panel, cr, fonts, &name, &app.binary, mid + 36.0, ry + 10.0, 32.0);
        let pinned = panel.config.pins.iter().any(|pin| pin.app == name);
        let badge = if pinned { draw::text_width(cr, fonts, false, 13.0, t.pinned) + 12.0 } else { 0.0 };
        draw::label_fit(cr, fonts, true, 16.0, TEXT, mid + 80.0, ry + 32.0, row_w - 96.0 - badge, &name);
        if pinned {
            draw::label(cr, fonts, false, 13.0, MUTED, mid + 24.0 + row_w - badge, ry + 32.0, t.pinned);
        }
        panel.hot(&format!("use:{name}"), mid + 24.0, ry, row_w, 52.0);
        ry += 58.0;
    }
    let _ = cr.restore();
}

pub(crate) fn mic(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot) {
    backdrop(panel, cr);
    let (x, y, w, h) = panel.dialog_rect(880.0, 440.0, 28.0);
    fill_round(cr, x, y, w, h, 18.0, CARD);
    stroke_round(cr, x, y, w, h, 18.0, BORDER, 2.0);
    panel.hot("noop", x, y, w, h);
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
                stroke_round(cr, x + 24.0, row, w - 48.0, 52.0, 14.0, draw::accent(), 2.0);
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
    let (x, y, w, h) = panel.dialog_rect(680.0, 548.0, 24.0);
    fill_round(cr, x, y, w, h, 18.0, CARD);
    stroke_round(cr, x, y, w, h, 18.0, BORDER, 2.0);
    panel.hot("noop", x, y, w, h);
    draw::label_fit(cr, fonts, true, 22.0, TEXT, x + 28.0, y + 48.0, w - 96.0, t.settings);
    let close = (x + w - 56.0, y + 16.0, 40.0, 40.0);
    let close_look = panel.look(close.0, close.1, close.2, close.3);
    if close_look != draw::Look::Rest {
        fill_round(cr, close.0, close.1, close.2, close.3, 12.0, if close_look == draw::Look::Down { draw::CONTROL_DOWN } else { draw::CONTROL_HOVER });
    }
    draw::icon(cr, draw::CLOSE_PATH, 1024.0, close.0 + 6.0, close.1 + 6.0, 28.0, TEXT);
    panel.hot("dismiss", close.0, close.1, close.2, close.3);

    let seg_x = x + w - 208.0;
    toggle(panel, cr, fonts, x, y + 84.0, seg_x, t.meters, panel.config.meters, "meters-on", "meters-off", t);
    toggle(panel, cr, fonts, x, y + 156.0, seg_x, t.snap, panel.config.snap, "snap-on", "snap-off", t);
    toggle(panel, cr, fonts, x, y + 228.0, seg_x, t.list_sliders, panel.config.list, "list-on", "list-off", t);
    toggle(panel, cr, fonts, x, y + 300.0, seg_x, t.wide, panel.config.wide, "wide-on", "wide-off", t);

    draw::label_fit(cr, fonts, true, 16.0, TEXT, x + 28.0, y + 404.0, seg_x - x - 40.0, t.color);
    let current = if panel.config.accent == "pink" { t.color_pink } else { t.color_blue };
    let drop = (seg_x, y + 376.0, 180.0, 48.0);
    fill_round(cr, drop.0, drop.1, drop.2, drop.3, 14.0, draw::CONTROL);
    stroke_round(cr, drop.0, drop.1, drop.2, drop.3, 14.0, if panel.color_open { draw::accent() } else { BORDER }, 2.0);
    draw::label_fit(cr, fonts, true, 16.0, TEXT, drop.0 + 16.0, drop.1 + 30.0, drop.2 - 44.0, current);
    draw::chevron(cr, drop.0 + drop.2 - 28.0, drop.1 + 18.0, 12.0, MUTED, false, panel.color_open);
    panel.hot("color-open", drop.0, drop.1, drop.2, drop.3);
    if panel.color_open {
        let menu_y = drop.1 + drop.3 + 6.0;
        fill_round(cr, drop.0, menu_y, drop.2, 92.0, 12.0, draw::INSET);
        stroke_round(cr, drop.0, menu_y, drop.2, 92.0, 12.0, BORDER, 2.0);
        for (index, (id, label)) in [("color:blue", t.color_blue), ("color:pink", t.color_pink)].into_iter().enumerate() {
            let row = menu_y + 6.0 + index as f64 * 40.0;
            let selected = (id == "color:pink") == (panel.config.accent == "pink");
            if selected {
                fill_round(cr, drop.0 + 6.0, row, drop.2 - 12.0, 36.0, 8.0, draw::CONTROL);
            }
            draw::label(cr, fonts, selected, 16.0, TEXT, drop.0 + 16.0, row + 24.0, label);
            panel.hot(id, drop.0, row, drop.2, 40.0);
        }
    }
}

fn toggle(panel: &mut Panel, cr: &Context, fonts: &Fonts, x: f64, y: f64, seg_x: f64, label: &str, on: bool, on_id: &str, off_id: &str, t: &Text) {
    draw::label_fit(cr, fonts, true, 16.0, TEXT, x + 28.0, y + 30.0, seg_x - x - 40.0, label);
    fill_round(cr, seg_x, y, 180.0, 48.0, 14.0, draw::CONTROL);
    if on {
        fill_round(cr, seg_x + 4.0, y + 4.0, 86.0, 40.0, 12.0, draw::accent());
    } else {
        fill_round(cr, seg_x + 90.0, y + 4.0, 86.0, 40.0, 12.0, draw::accent());
    }
    draw::label(cr, fonts, true, 16.0, if on { draw::on_accent() } else { draw::SOFT }, seg_x + 28.0, y + 30.0, t.on);
    draw::label(cr, fonts, true, 16.0, if on { draw::SOFT } else { draw::on_accent() }, seg_x + 118.0, y + 30.0, t.off);
    panel.hot(on_id, seg_x, y, 90.0, 48.0);
    panel.hot(off_id, seg_x + 90.0, y, 90.0, 48.0);
}

fn row_button(panel: &mut Panel, cr: &Context, fonts: &Fonts, x: f64, y: f64, w: f64, label: &str, id: &str) {
    fill_round(cr, x, y, w, 52.0, 14.0, INSET);
    draw::label_fit(cr, fonts, true, 16.0, TEXT, x + 16.0, y + 32.0, w - 32.0, label);
    panel.hot(id, x, y, w, 52.0);
}

/// A row with the app's icon. `accent` outlines the row that creates a new channel.
fn icon_row(panel: &mut Panel, cr: &Context, fonts: &Fonts, x: f64, y: f64, w: f64, app: (&str, &str), label: &str, id: &str, accent: bool) {
    fill_round(cr, x, y, w, 52.0, 14.0, INSET);
    if accent {
        stroke_round(cr, x, y, w, 52.0, 14.0, draw::accent(), 2.0);
    }
    paint_desktop_icon(panel, cr, fonts, app.0, app.1, x + 12.0, y + 10.0, 32.0);
    draw::label_fit(cr, fonts, true, 16.0, if accent { draw::accent() } else { TEXT }, x + 56.0, y + 32.0, w - 72.0, label);
    panel.hot(id, x, y, w, 52.0);
}

/// Keep the builtin constant referenced so a rename fails here too.
#[allow(dead_code)]
fn _builtin() -> &'static str {
    Graph::BUILTIN_SOURCE
}
