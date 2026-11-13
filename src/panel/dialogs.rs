//! Add-a-channel, the microphone picker, App Settings, the name field, and Projects.

use super::{source_label, Hit, HitBounds, NameEdit, Overlay, Panel, Ui, SUGGESTIONS};
use crate::draw::{self, ButtonKind, Font, Rect, CONTROL, HEADING, MUTED, R_CONTROL, TEXT};
use crate::i18n::Text;
use crate::worker::Snapshot;

/// Row in a dialog list, and the distance from one row to the next.
const ROW_H: f64 = 52.0;
const ROW_STEP: f64 = 58.0;
/// Padding between a sheet's edge and its content.
const PAD: f64 = 28.0;
/// Rows reach a little further out than the text above them.
const ROW_PAD: f64 = 24.0;
/// The two buttons at the bottom of a dialog, and the controls of App Settings.
const BUTTON_H: f64 = 48.0;
const SETTING_W: f64 = 168.0;
/// Rows of a dropdown inside a dialog.
const MENU_ROW: f64 = 40.0;
/// Gap between two project rows.
const PROJECT_GAP: f64 = 6.0;

/// A setting's name, and the text of plain rows.
const BODY: Font = Font::regular(15.0);
/// The name of a list row that also carries a second line or a button.
const ROW_NAME: Font = Font::bold(16.0);
const NOTE: Font = Font::regular(13.0);
const HINT: Font = Font::regular(13.5);
/// The text in the name field.
const FIELD: Font = Font::bold(18.0);

/// Suggested names and a custom one on the left, the running apps on the right.
pub(crate) fn add(panel: &mut Panel, ui: &mut Ui, snap: &Snapshot) {
    let t = ui.t;
    let sheet = panel.sheet_rect(PAD);
    ui.dialog(panel.window(), sheet);
    let Rect { x, y, w, h } = sheet;
    let mid = x + w / 2.0;
    hairline(ui, Rect::new(mid, y + 24.0, 1.0, h - 48.0));

    ui.label(HEADING, TEXT, x + PAD, y + 48.0, t.suggest);
    let mut row = Rect::new(x + ROW_PAD, y + 72.0, mid - x - 48.0, ROW_H);
    for name in SUGGESTIONS {
        row_button(ui, row, name, Hit::Suggest(name));
        row.y += ROW_STEP;
    }
    row_button(ui, row, t.custom, Hit::CustomName);

    ui.label_fit(HEADING, TEXT, mid + PAD, y + 48.0, w / 2.0 - 220.0, t.running);
    ui.button_hot(Rect::new(x + w - 160.0, y + 24.0, 120.0, 40.0), t.refresh, ButtonKind::Plain, Hit::RefreshApps);
    running_apps(panel, ui, snap, sheet);
}

/// The scrolling list of apps that play audio. A press makes a channel for that app.
fn running_apps(panel: &mut Panel, ui: &mut Ui, snap: &Snapshot, sheet: Rect) {
    let t = ui.t;
    let Rect { x: _, y, w, h } = sheet;
    let mid = sheet.x + w / 2.0;
    let list_top = y + 80.0;
    let list_h = h - 110.0;
    let row_w = w / 2.0 - 64.0;
    let apps = snap.graph.picker_apps();
    panel.app_binaries = apps.iter().map(|app| (app.name.clone(), app.binary.clone())).collect();
    // The PC outputs that have no channel yet come first, while the link is on.
    let pc_inputs: Vec<usize> = (0..crate::config::PC_INPUTS)
        .filter(|index| panel.config.pc_link && !crate::config::pc_input_used(&panel.config.channels, *index))
        .collect();
    let content = (pc_inputs.len() + apps.len()) as f64 * ROW_STEP;
    panel.dialog_scroll = panel.dialog_scroll.clamp(0.0, (content - list_h).max(0.0));
    ui.clip(Rect::new(mid + 16.0, list_top, w / 2.0 - 40.0, list_h));
    let mut ry = list_top - panel.dialog_scroll;
    for index in pc_inputs {
        if ry + ROW_H >= list_top && ry <= list_top + list_h {
            let row = Rect::new(mid + ROW_PAD, ry, row_w, ROW_H);
            row_fill(ui, row);
            super::mix::pc_input_mark(panel, ui, index, mid + 36.0, ry + 10.0, 32.0);
            let name = crate::config::pc_input_name(index);
            let name_w = ui.text_width(BODY, &name);
            ui.label(BODY, TEXT, mid + 80.0, ry + 31.0, &name);
            ui.label_fit(NOTE, MUTED, mid + 92.0 + name_w, ry + 31.0, row_w - 108.0 - name_w, t.pc_input_hint);
            ui.hot(Hit::AddPcInput(index), row);
        }
        ry += ROW_STEP;
    }
    for app in &apps {
        if ry + ROW_H < list_top || ry > list_top + list_h {
            ry += ROW_STEP;
            continue;
        }
        let row = Rect::new(mid + ROW_PAD, ry, row_w, ROW_H);
        row_fill(ui, row);
        super::art::desktop_icon(panel, ui, &app.name, &app.binary, mid + 36.0, ry + 10.0, 32.0);
        let pinned = panel.config.pins.iter().any(|pin| pin.app == app.name);
        let badge = if pinned { ui.text_width(NOTE, t.pinned) + 12.0 } else { 0.0 };
        ui.label_fit(BODY, TEXT, mid + 80.0, ry + 31.0, row_w - 96.0 - badge, &app.name);
        if pinned {
            ui.label(NOTE, MUTED, mid + ROW_PAD + row_w - badge - 4.0, ry + 31.0, t.pinned);
        }
        ui.hot(Hit::UseApp(app.name.clone()), row);
        ry += ROW_STEP;
    }
    ui.unclip();
}

/// One choice of the microphone picker.
struct MicRow {
    /// What the config stores: empty for the system default, `builtin`, or a node name.
    source: String,
    label: String,
    hint: String,
}

pub(crate) fn mic(panel: &mut Panel, ui: &mut Ui, snap: &Snapshot) {
    let t = ui.t;
    let sheet = panel.dialog_rect(880.0, 440.0, PAD);
    ui.dialog(panel.window(), sheet);
    let Rect { x, y, w, h } = sheet;
    ui.label(HEADING, TEXT, x + PAD, y + 44.0, t.mic_picker);
    let current = source_label(&panel.config.mic_source, &snap.graph, t);
    ui.label_fit(Font::regular(14.0), MUTED, x + PAD, y + 68.0, w - 56.0, &current);

    let mut row_y = y + 96.0;
    let rows = mic_rows(t, snap);
    let list_h = h - 120.0;
    panel.dialog_scroll = panel.dialog_scroll.clamp(0.0, (rows.len() as f64 * ROW_STEP - list_h).max(0.0));
    ui.clip(Rect::new(x + 16.0, row_y, w - 32.0, list_h));
    row_y -= panel.dialog_scroll;
    for MicRow { source, label, hint } in rows {
        if row_y + ROW_H >= y + 90.0 && row_y <= y + h - 16.0 {
            let row = Rect::new(x + ROW_PAD, row_y, w - 48.0, ROW_H);
            // A config written by hand may spell the system default out.
            let selected = source == panel.config.mic_source || (source.is_empty() && panel.config.mic_source == "system");
            if selected {
                ui.fill(row, R_CONTROL, draw::accent_tint());
            } else {
                row_fill(ui, row);
            }
            ui.label_fit(ROW_NAME, if selected { draw::accent() } else { TEXT }, x + 40.0, row_y + 22.0, w - 80.0, &label);
            if !hint.is_empty() {
                ui.label_fit(Font::regular(12.0), MUTED, x + 40.0, row_y + 42.0, w - 80.0, &hint);
            }
            ui.hot(Hit::MicSource(source), row);
        }
        row_y += ROW_STEP;
    }
    ui.unclip();
}

/// The system default, the built-in microphone, then every source that can be picked.
fn mic_rows(t: &Text, snap: &Snapshot) -> Vec<MicRow> {
    let mut rows = vec![
        MicRow { source: String::new(), label: t.mic_system.to_string(), hint: String::new() },
        MicRow { source: "builtin".into(), label: t.mic_builtin.to_string(), hint: t.mic_after.to_string() },
    ];
    for node in snap.graph.pickable_sources() {
        let label = if node.description.is_empty() { node.name.clone() } else { node.description.clone() };
        rows.push(MicRow { source: node.name.clone(), label, hint: node.name });
    }
    rows
}

/// Accent color, meters, and the way to the VST plugins.
pub(crate) fn settings(panel: &mut Panel, ui: &mut Ui) {
    let t = ui.t;
    let sheet = panel.dialog_rect(640.0, if panel.color_open { 472.0 } else { 364.0 }, 24.0);
    ui.dialog(panel.window(), sheet);
    let Rect { x, y, w, .. } = sheet;
    ui.label_mid(HEADING, TEXT, x + PAD, y + 44.0, t.settings);
    ui.close_button(x + w - 60.0, y + 24.0);

    // Every control sits in one column on the right, its name on the left.
    let control_x = x + w - PAD - SETTING_W;
    let color_y = y + 96.0;
    ui.label_mid(BODY, TEXT, x + PAD, color_y + 24.0, t.color);
    let drop = Rect::new(control_x, color_y, SETTING_W, BUTTON_H);
    accent_dropdown(panel, ui, drop);

    // The open color list pushes the rows below it down.
    let menu_h = if panel.color_open { 108.0 } else { 0.0 };
    let meters_y = color_y + 64.0 + menu_h;
    ui.label_mid(BODY, TEXT, x + PAD, meters_y + 24.0, t.meters);
    let meters = Rect::new(control_x, meters_y, SETTING_W, BUTTON_H);
    ui.pill_toggle(meters, panel.config.meters, (t.on, t.off), (Hit::Meters(true), Hit::Meters(false)));

    // The PC link: its switch, and under its name who is at the other end.
    let link_y = meters_y + 64.0;
    ui.label_mid(BODY, TEXT, x + PAD, link_y + 15.0, t.pc_link);
    let status = if !panel.config.pc_link {
        String::new()
    } else if panel.link.connected() {
        super::fmt_token(t.pc_link_on, "name", &panel.link.peer)
    } else {
        t.pc_link_wait.to_string()
    };
    ui.label_fit(NOTE, MUTED, x + PAD, link_y + 40.0, control_x - x - PAD - 16.0, &status);
    let link = Rect::new(control_x, link_y, SETTING_W, BUTTON_H);
    ui.pill_toggle(link, panel.config.pc_link, (t.on, t.off), (Hit::PcLink(true), Hit::PcLink(false)));

    let vst_y = link_y + 64.0;
    ui.label_mid(BODY, TEXT, x + PAD, vst_y + 24.0, t.vst_plugins);
    ui.button_hot(Rect::new(control_x, vst_y, SETTING_W, BUTTON_H), t.vst_manage, ButtonKind::Plain, Hit::VstSettings);

    if panel.color_open {
        accent_menu(panel, ui, drop);
    }
}

/// The closed dropdown: a dot in the accent color and its name.
fn accent_dropdown(panel: &Panel, ui: &mut Ui, drop: Rect) {
    let t = ui.t;
    let current = if panel.config.accent == "pink" { t.color_pink } else { t.color_blue };
    let hovered = ui.look(drop) != draw::Look::Rest;
    ui.fill(drop, R_CONTROL, if hovered { draw::CONTROL_HOVER } else { CONTROL });
    if panel.color_open {
        ui.stroke(drop.inset(1.0), R_CONTROL - 1.0, draw::accent(), 2.0);
    }
    ui.fill(Rect::square(drop.x + 16.0, drop.y + 18.0, 12.0), 6.0, draw::accent());
    ui.label_fit(draw::BUTTON, TEXT, drop.x + 38.0, drop.y + 29.0, drop.w - 72.0, current);
    draw::chevron(ui.cr, drop.x + drop.w - 28.0, drop.y + 22.0, 12.0, MUTED, false, panel.color_open);
    ui.hot(Hit::AccentMenu, drop);
}

/// The two accent colors, listed under the dropdown.
fn accent_menu(panel: &Panel, ui: &mut Ui, drop: Rect) {
    let t = ui.t;
    let menu_y = drop.y + drop.h + 6.0;
    ui.fill(Rect::new(drop.x, menu_y, drop.w, 96.0), R_CONTROL + 4.0, CONTROL);
    for (index, (accent, label)) in [("blue", t.color_blue), ("pink", t.color_pink)].into_iter().enumerate() {
        let row = Rect::new(drop.x, menu_y + 8.0 + index as f64 * MENU_ROW, drop.w, MENU_ROW);
        let selected = (accent == "pink") == (panel.config.accent == "pink");
        if selected {
            ui.fill(row.inset_xy(6.0, 2.0), 10.0, draw::accent_tint());
        }
        ui.label_mid(Font::weight(selected, 15.0), if selected { draw::accent() } else { TEXT }, drop.x + 16.0, row.y + 20.0, label);
        ui.hot(Hit::Accent(accent), row);
    }
}

/// Tonal row with a hover layer.
fn row_fill(ui: &Ui, row: Rect) {
    ui.fill(row, R_CONTROL, ui.control_fill(row, CONTROL));
}

fn row_button(ui: &mut Ui, row: Rect, label: &str, hit: Hit) {
    row_fill(ui, row);
    ui.label_mid(BODY, TEXT, row.x + 16.0, row.y + ROW_H / 2.0, label);
    ui.hot(hit, row);
}

/// A one-pixel divider. A plain rectangle, so its edges stay sharp.
fn hairline(ui: &Ui, line: Rect) {
    ui.cr.set_source_rgb(draw::DIVIDER.r, draw::DIVIDER.g, draw::DIVIDER.b);
    ui.cr.rectangle(line.x, line.y, line.w, line.h);
    let _ = ui.cr.fill();
}

/// Channel name. The field shows what the headset keyboard types. Nothing is added until Add or Save.
pub(crate) fn name(panel: &mut Panel, ui: &mut Ui) {
    let t = ui.t;
    let sheet = panel.dialog_rect(680.0, 300.0, PAD);
    ui.dialog(panel.window(), sheet);
    let Rect { x, y, w, h } = sheet;
    let title = if panel.naming_project() {
        t.project_name
    } else if panel.naming_folder() {
        t.vst_folder
    } else {
        t.channel_name
    };
    ui.label_mid(HEADING, TEXT, x + PAD, y + 44.0, title);
    ui.close_button(x + w - 60.0, y + 24.0);

    name_field(panel, ui, Rect::new(x + PAD, y + 84.0, w - 56.0, 56.0), title);
    if panel.name_rejected {
        ui.label_fit(NOTE, draw::DANGER_TEXT, x + PAD, y + 164.0, w - 56.0, t.name_rejected);
    }

    // An existing thing is saved; only a new channel is added.
    let saving = panel.naming_project() || panel.naming_folder() || matches!(panel.overlay, Overlay::Name(NameEdit::Rename(_)) | Overlay::Name(NameEdit::Bus(_)));
    let buttons_y = y + h - 76.0;
    ui.button_hot(Rect::new(x + PAD, buttons_y, 180.0, BUTTON_H), t.cancel, ButtonKind::Plain, Hit::Dismiss);
    let ok = if saving { t.save } else { t.add };
    ui.button_hot(Rect::new(x + w - 208.0, buttons_y, 180.0, BUTTON_H), ok, ButtonKind::Accent, Hit::NameOk);
}

/// The text field: the draft, or `placeholder` while it is empty, and a caret
/// while the headset keyboard is open. A press on it opens the keyboard.
fn name_field(panel: &Panel, ui: &mut Ui, field: Rect, placeholder: &str) {
    ui.fill(field, R_CONTROL, CONTROL);
    ui.stroke(field.inset(1.0), R_CONTROL - 1.0, draw::accent(), 2.0);
    let draft = panel.name_draft.as_str();
    let text_x = field.x + 16.0;
    let baseline = field.y + 36.0;
    let room = field.w - 36.0;
    if draft.is_empty() {
        ui.label_fit(Font::regular(18.0), MUTED, text_x, baseline, room, placeholder);
    } else {
        ui.label_tail(FIELD, TEXT, text_x, baseline, room, draft);
    }
    let shown = if draft.is_empty() { String::new() } else { draw::fit_tail(ui.cr, ui.fonts, FIELD, room, draft) };
    let caret_x = text_x + ui.text_width(FIELD, &shown).min(room);
    if panel.name_keyboard && caret_x < field.x + field.w - 12.0 {
        let cr = ui.cr;
        cr.set_source_rgb(draw::accent().r, draw::accent().g, draw::accent().b);
        cr.set_line_width(2.0);
        cr.move_to(caret_x + 2.0, field.y + 16.0);
        cr.line_to(caret_x + 2.0, field.y + 40.0);
        let _ = cr.stroke();
    }
    ui.hot(Hit::NameField, field);
}

/// Saved routing setups. A row loads its project; the cross deletes after a
/// second press, like removing a channel.
pub(crate) fn projects(panel: &mut Panel, ui: &mut Ui) {
    let t = ui.t;
    let sheet = panel.dialog_rect(680.0, 580.0, PAD);
    ui.dialog(panel.window(), sheet);
    let Rect { x, y, w, h } = sheet;
    ui.label_mid(HEADING, TEXT, x + PAD, y + 44.0, t.projects);
    ui.close_button(x + w - 60.0, y + 24.0);
    let inner = w - 56.0;
    let mut row = y + 82.0;
    for line in ui.wrap(HINT, inner, 3, t.projects_hint) {
        ui.label(HINT, MUTED, x + PAD, row, &line);
        row += 19.0;
    }
    row += 12.0;
    let save_w = (ui.text_width(draw::BUTTON, t.project_save) + 56.0).min(inner);
    ui.button_hot(Rect::new(x + PAD, row, save_w, BUTTON_H), t.project_save, ButtonKind::Accent, Hit::ProjectSave);
    row += 64.0;
    hairline(ui, Rect::new(x + PAD, row, inner, 1.0));
    row += 13.0;

    let list = Rect::new(x + PAD, row, inner, (y + h - 24.0 - row).max(0.0));
    if panel.projects.is_empty() {
        let mut ty = list.y + 32.0;
        for line in ui.wrap(BODY, inner, 3, t.projects_empty) {
            ui.label(BODY, MUTED, list.x, ty, &line);
            ty += 21.0;
        }
        return;
    }
    project_rows(panel, ui, list);
}

fn project_rows(panel: &mut Panel, ui: &mut Ui, list: Rect) {
    let step = ROW_H + PROJECT_GAP;
    let content = panel.projects.len() as f64 * step - PROJECT_GAP;
    panel.dialog_scroll = panel.dialog_scroll.min((content - list.h).max(0.0));
    let scroll = panel.dialog_scroll;
    ui.clip(list);
    // Hit areas stop at the list too, so a row half out of view cannot be hit there.
    ui.bounds = Some(HitBounds::rows(list.y, list.y + list.h));
    for (index, project) in panel.projects.iter().enumerate() {
        let row = Rect::new(list.x, list.y + index as f64 * step - scroll, list.w, ROW_H);
        if row.y + ROW_H < list.y || row.y > list.y + list.h {
            continue;
        }
        ui.fill(row, R_CONTROL, ui.control_fill(row, CONTROL));
        ui.label_fit(ROW_NAME, TEXT, list.x + 16.0, row.y + ROW_H / 2.0 + 6.0, list.w - 76.0, &project.name);
        let armed = panel.project_armed(index);
        let delete = Rect::square(list.x + list.w - 46.0, row.y + (ROW_H - 36.0) / 2.0, 36.0);
        if armed {
            ui.fill(delete, delete.w / 2.0, draw::DANGER);
        }
        let ink = if armed { draw::on_accent() } else { draw::DANGER_TEXT };
        ui.icon(draw::CLOSE_PATH, 1024.0, delete.x + 10.0, delete.y + 10.0, 16.0, ink);
        ui.hot(Hit::ProjectLoad(index), row);
        ui.hot(Hit::ProjectRemove(index), delete);
    }
    ui.bounds = None;
    ui.unclip();
}
