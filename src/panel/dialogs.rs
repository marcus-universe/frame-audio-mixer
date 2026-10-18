//! Add-a-channel, the microphone picker, and App Settings.

use super::{backdrop, close_button, fill_round, fmt_token, paint_desktop_icon, pill_toggle, sheet_surface, source_label, stroke_round, Panel, SUGGESTIONS};
use crate::draw::{self, ButtonKind, Fonts, CONTROL, MUTED, R_CONTROL, TEXT};

/// Row in a dialog list.
const ROW_H: f64 = 52.0;
use crate::graph::Graph;
use crate::i18n::Text;
use crate::worker::Snapshot;
use cairo::Context;

pub(crate) fn add(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot) {
    backdrop(panel, cr);
    let (x, y, w, h) = panel.sheet(28.0);
    sheet_surface(cr, x, y, w, h);
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
        row_fill(panel, cr, mid + 24.0, ry, row_w);
        paint_desktop_icon(panel, cr, fonts, &name, &app.binary, mid + 36.0, ry + 10.0, 32.0);
        let pinned = panel.config.pins.iter().any(|pin| pin.app == name);
        let badge = if pinned { draw::text_width(cr, fonts, false, 13.0, t.pinned) + 12.0 } else { 0.0 };
        draw::label_fit(cr, fonts, false, 15.0, TEXT, mid + 80.0, ry + 31.0, row_w - 96.0 - badge, &name);
        if pinned {
            draw::label(cr, fonts, false, 13.0, MUTED, mid + 24.0 + row_w - badge - 4.0, ry + 31.0, t.pinned);
        }
        panel.hot(&format!("use:{name}"), mid + 24.0, ry, row_w, 52.0);
        ry += 58.0;
    }
    let _ = cr.restore();
}

pub(crate) fn mic(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot) {
    backdrop(panel, cr);
    let (x, y, w, h) = panel.dialog_rect(880.0, 440.0, 28.0);
    sheet_surface(cr, x, y, w, h);
    panel.hot("noop", x, y, w, h);
    draw::label(cr, fonts, true, 20.0, TEXT, x + 28.0, y + 44.0, t.mic_picker);
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
            if selected {
                fill_round(cr, x + 24.0, row, w - 48.0, ROW_H, R_CONTROL, draw::accent_tint());
            } else {
                row_fill(panel, cr, x + 24.0, row, w - 48.0);
            }
            draw::label_fit(cr, fonts, true, 16.0, if selected { draw::accent() } else { TEXT }, x + 40.0, row + 22.0, w - 80.0, &label);
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
    let (x, y, w, h) = panel.dialog_rect(640.0, 260.0, 24.0);
    sheet_surface(cr, x, y, w, h);
    panel.hot("noop", x, y, w, h);
    draw::label_mid(cr, fonts, true, 20.0, TEXT, x + 28.0, y + 44.0, t.settings);
    close_button(panel, cr, x + w - 60.0, y + 24.0);

    let seg_x = x + w - 28.0 - 168.0;
    let color_y = y + 96.0;
    draw::label_mid(cr, fonts, false, 15.0, TEXT, x + 28.0, color_y + 24.0, t.color);
    let current = if panel.config.accent == "pink" { t.color_pink } else { t.color_blue };
    let drop = (seg_x, color_y, 168.0, 48.0);
    let look = panel.look(drop.0, drop.1, drop.2, drop.3);
    fill_round(cr, drop.0, drop.1, drop.2, drop.3, R_CONTROL, if look == draw::Look::Rest { CONTROL } else { draw::CONTROL_HOVER });
    if panel.color_open {
        stroke_round(cr, drop.0 + 1.0, drop.1 + 1.0, drop.2 - 2.0, drop.3 - 2.0, R_CONTROL - 1.0, draw::accent(), 2.0);
    }
    fill_round(cr, drop.0 + 16.0, drop.1 + 18.0, 12.0, 12.0, 6.0, draw::accent());
    draw::label_fit(cr, fonts, true, 15.0, TEXT, drop.0 + 38.0, drop.1 + 29.0, drop.2 - 72.0, current);
    draw::chevron(cr, drop.0 + drop.2 - 28.0, drop.1 + 22.0, 12.0, MUTED, false, panel.color_open);
    panel.hot("color-open", drop.0, drop.1, drop.2, drop.3);

    let meters_y = color_y + 64.0;
    draw::label_mid(cr, fonts, false, 15.0, TEXT, x + 28.0, meters_y + 24.0, t.meters);
    pill_toggle(panel, cr, fonts, seg_x, meters_y, 168.0, 48.0, panel.config.meters, (t.on, t.off), ("meters-on", "meters-off"));
    if panel.color_open {
        let menu_y = drop.1 + drop.3 + 6.0;
        fill_round(cr, drop.0, menu_y, drop.2, 96.0, R_CONTROL + 4.0, CONTROL);
        for (index, (id, label)) in [("color:blue", t.color_blue), ("color:pink", t.color_pink)].into_iter().enumerate() {
            let row = menu_y + 8.0 + index as f64 * 40.0;
            let selected = (id == "color:pink") == (panel.config.accent == "pink");
            if selected {
                fill_round(cr, drop.0 + 6.0, row + 2.0, drop.2 - 12.0, 36.0, 10.0, draw::accent_tint());
            }
            draw::label_mid(cr, fonts, selected, 15.0, if selected { draw::accent() } else { TEXT }, drop.0 + 16.0, row + 20.0, label);
            panel.hot(id, drop.0, row, drop.2, 40.0);
        }
    }
}

/// Tonal row with a hover layer.
fn row_fill(panel: &Panel, cr: &Context, x: f64, y: f64, w: f64) {
    let look = panel.look(x, y, w, ROW_H);
    let fill = match look {
        draw::Look::Down => draw::CONTROL_DOWN,
        draw::Look::Hover => draw::CONTROL_HOVER,
        draw::Look::Rest => CONTROL,
    };
    fill_round(cr, x, y, w, ROW_H, R_CONTROL, fill);
}

fn row_button(panel: &mut Panel, cr: &Context, fonts: &Fonts, x: f64, y: f64, w: f64, label: &str, id: &str) {
    row_fill(panel, cr, x, y, w);
    draw::label_mid(cr, fonts, false, 15.0, TEXT, x + 16.0, y + ROW_H / 2.0, label);
    panel.hot(id, x, y, w, ROW_H);
}

/// A row with the app's icon. `accent` marks the row that creates a new channel.
fn icon_row(panel: &mut Panel, cr: &Context, fonts: &Fonts, x: f64, y: f64, w: f64, app: (&str, &str), label: &str, id: &str, accent: bool) {
    if accent {
        fill_round(cr, x, y, w, ROW_H, R_CONTROL, draw::accent_tint());
    } else {
        row_fill(panel, cr, x, y, w);
    }
    paint_desktop_icon(panel, cr, fonts, app.0, app.1, x + 12.0, y + 10.0, 32.0);
    draw::label_fit(cr, fonts, accent, 15.0, if accent { draw::accent() } else { TEXT }, x + 56.0, y + 31.0, w - 72.0, label);
    panel.hot(id, x, y, w, 52.0);
}

/// Keep the builtin constant referenced so a rename fails here too.
#[allow(dead_code)]
fn _builtin() -> &'static str {
    Graph::BUILTIN_SOURCE
}

/// Channel name. The field shows what the headset keyboard types. Nothing is added until Add or Save.
pub(crate) fn name(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text) {
    backdrop(panel, cr);
    let (x, y, w, h) = panel.dialog_rect(680.0, 300.0, 28.0);
    sheet_surface(cr, x, y, w, h);
    panel.hot("noop", x, y, w, h);
    draw::label_mid(cr, fonts, true, 20.0, TEXT, x + 28.0, y + 44.0, t.channel_name);
    close_button(panel, cr, x + w - 60.0, y + 24.0);

    let field = (x + 28.0, y + 84.0, w - 56.0, 56.0);
    fill_round(cr, field.0, field.1, field.2, field.3, R_CONTROL, CONTROL);
    stroke_round(cr, field.0 + 1.0, field.1 + 1.0, field.2 - 2.0, field.3 - 2.0, R_CONTROL - 1.0, draw::accent(), 2.0);
    let draft = panel.name_draft.clone();
    let text_x = field.0 + 16.0;
    let baseline = field.1 + 36.0;
    let room = field.2 - 36.0;
    if draft.is_empty() {
        draw::label_fit(cr, fonts, false, 18.0, MUTED, text_x, baseline, room, t.channel_name);
    } else {
        draw::label_tail(cr, fonts, true, 18.0, TEXT, text_x, baseline, room, &draft);
    }
    let shown = if draft.is_empty() { String::new() } else { draw::fit_tail(cr, fonts, true, 18.0, room, &draft) };
    let caret_x = text_x + draw::text_width(cr, fonts, true, 18.0, &shown).min(room);
    if panel.name_keyboard && caret_x < field.0 + field.2 - 12.0 {
        cr.set_source_rgb(draw::accent().r, draw::accent().g, draw::accent().b);
        cr.set_line_width(2.0);
        cr.move_to(caret_x + 2.0, field.1 + 16.0);
        cr.line_to(caret_x + 2.0, field.1 + 40.0);
        let _ = cr.stroke();
    }
    panel.hot("name-field", field.0, field.1, field.2, field.3);
    if panel.name_rejected {
        draw::label_fit(cr, fonts, false, 13.0, draw::DANGER_TEXT, x + 28.0, y + 164.0, w - 56.0, t.name_rejected);
    }

    let renaming = matches!(panel.overlay, super::Overlay::Name(super::NameEdit::Rename(_)));
    let by = y + h - 76.0;
    panel.button(cr, fonts, x + 28.0, by, 180.0, 48.0, t.cancel, ButtonKind::Plain);
    panel.hot("dismiss", x + 28.0, by, 180.0, 48.0);
    let ok = if renaming { t.save } else { t.add };
    panel.button(cr, fonts, x + w - 208.0, by, 180.0, 48.0, ok, ButtonKind::Accent);
    panel.hot("name-ok", x + w - 208.0, by, 180.0, 48.0);
}
