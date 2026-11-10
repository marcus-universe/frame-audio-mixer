//! Apps & updates, laid out like Frame Mic Tuner. This app is Frame Audio Mixer.
//! Frame Mic Tuner is the companion. The rest of the list is sasaken's Frame apps.

use super::art::app_icon;
use super::{fmt_token, Hit, Panel, Ui, CONTENT_TOP, GUTTER, MARGIN};
use crate::apps::{AppState, Entry};
use crate::draw::{self, ButtonKind, Font, Rect, BUTTON, CHIP_OK_FILL, CHIP_OK_TEXT, DANGER_TEXT, HEADING, INSET, MUTED, R_SURFACE, SOFT, TEXT};
use crate::i18n::{Lang, Text};
use crate::updater::{UpdateState, UpdateView, VERSION};
use crate::worker::Snapshot;

/// Padding inside a card, and inside a dialog sheet.
const CARD_PAD: f64 = 24.0;
const SHEET_PAD: f64 = 28.0;
/// One app in a list, and the distance from one to the next.
const APP_ROW_H: f64 = 96.0;
const APP_ROW_STEP: f64 = 104.0;
/// The row that opens the list of all apps.
const ALL_ROW_H: f64 = 88.0;
const BUTTON_H: f64 = 48.0;

/// App names.
const NAME: Font = Font::bold(18.0);
/// The headline of a box or a row.
const LEAD: Font = Font::bold(16.0);
const BODY: Font = Font::regular(15.0);
const NOTE: Font = Font::regular(13.0);
const SMALL: Font = Font::regular(12.0);
const CHIP: Font = Font::bold(14.0);
const TAG: Font = Font::bold(12.0);

/// Two cards: this app with its updater, and the related apps. Side by side, or stacked in portrait.
pub(crate) fn draw(panel: &mut Panel, ui: &mut Ui, snap: &Snapshot) {
    let stage = panel.stage();
    let top = CONTENT_TOP;
    let bottom = stage.footer.y - GUTTER;
    let gap = GUTTER;
    let inner = stage.w - 2.0 * MARGIN;
    let (app, related_box) = if stage.portrait {
        let h = ((bottom - top - gap) / 2.0).max(240.0);
        (Rect::new(MARGIN, top, inner, h), Rect::new(MARGIN, top + h + gap, inner, h))
    } else {
        let left_w = ((inner - gap) / 2.0).floor();
        let right_w = inner - gap - left_w;
        let h = bottom - top;
        (Rect::new(MARGIN, top, left_w, h), Rect::new(MARGIN + left_w + gap, top, right_w, h))
    };
    this_app(panel, ui, snap, app);
    related(panel, ui, snap, related_box);
}

fn this_app(panel: &mut Panel, ui: &mut Ui, snap: &Snapshot, card: Rect) {
    let t = ui.t;
    let Rect { x, y, w, h } = card;
    ui.card(card);
    let left = x + CARD_PAD;
    let inner = w - 48.0;
    ui.label(HEADING, TEXT, left, y + 44.0, t.this_app);
    let logo = crate::brand::icon_png(panel.config.accent == "pink", 128);
    draw::paint_png_bytes(ui.cr, logo, Rect::square(left, y + 68.0, 56.0));
    let name = "Frame Audio Mixer";
    ui.label(NAME, TEXT, left + 72.0, y + 92.0, name);
    let name_w = ui.text_width(NAME, name);
    ui.label(NOTE, MUTED, left + 72.0 + name_w + 10.0, y + 92.0, VERSION);
    ui.label_fit(NOTE, MUTED, left + 72.0, y + 114.0, inner - 72.0, t.this_app_desc);

    update_box(ui, &snap.update, Rect::new(left, y + 148.0, inner, 112.0));

    let check_y = y + 284.0;
    ui.label_mid(BODY, TEXT, left, check_y + 24.0, t.apps_check);
    let toggle = Rect::new(left + inner - 168.0, check_y, 168.0, BUTTON_H);
    ui.pill_toggle(toggle, panel.config.check_updates, (t.on, t.off), (Hit::CheckUpdates(true), Hit::CheckUpdates(false)));
    ui.label_fit(NOTE, MUTED, left, check_y + 76.0, inner, t.list_note);
    hairline(ui, Rect::new(left, y + h - 84.0, inner, 1.0));
    ui.label(SMALL, MUTED, left, y + h - 52.0, t.help_title);
    ui.label(Font::regular(14.0), SOFT, left, y + h - 30.0, "github.com/marcus-universe/frame-audio-mixer");
}

/// A one-pixel divider. A plain rectangle, so its edges stay sharp.
fn hairline(ui: &Ui, line: Rect) {
    ui.cr.set_source_rgb(draw::DIVIDER.r, draw::DIVIDER.g, draw::DIVIDER.b);
    ui.cr.rectangle(line.x, line.y, line.w, line.h);
    let _ = ui.cr.fill();
}

/// What the updater knows, Check now, and Install once a newer version can be installed.
fn update_box(ui: &mut Ui, update: &UpdateView, area: Rect) {
    let t = ui.t;
    let Rect { x, y, w, .. } = area;
    ui.fill(area, R_SURFACE, INSET);
    let (line, bad) = update_line(t, update);
    ui.label_fit(LEAD, if bad { DANGER_TEXT } else { TEXT }, x + 20.0, y + 40.0, w - 200.0, &line);
    if !update.message.is_empty() && update.state != UpdateState::Available {
        ui.label_fit(NOTE, MUTED, x + 20.0, y + 64.0, w - 200.0, &update.message);
    }
    let check_w = (ui.text_width(BUTTON, t.check_now) + 48.0).max(140.0);
    let check = Rect::new(x + w - 20.0 - check_w, y + 32.0, check_w, BUTTON_H);
    ui.button_hot(check, t.check_now, ButtonKind::Plain, Hit::CheckUpdatesNow);
    if update.installable && update.state == UpdateState::Available {
        let caption = fmt_token(t.update_to_fmt, "version", &update.latest);
        ui.button_hot(Rect::new(x + 20.0, y + 64.0, 220.0, 40.0), &caption, ButtonKind::Accent, Hit::InstallUpdate);
    }
}

/// The updater's state in one line, and whether it reports a failure.
fn update_line(t: &Text, update: &UpdateView) -> (String, bool) {
    match update.state {
        UpdateState::UpToDate => (fmt_token(t.up_to_date_fmt, "version", VERSION), false),
        UpdateState::Available => (fmt_token(t.update_to_fmt, "version", &update.latest), false),
        UpdateState::Installing => (format!("{} {}", t.installing, update.step), false),
        UpdateState::Installed => (fmt_token(t.up_to_date_fmt, "version", &update.version), false),
        UpdateState::CheckFailed | UpdateState::InstallFailed => (t.apps_available.to_string(), true),
        UpdateState::Unknown => (t.apps_current.to_string(), false),
    }
}

/// The companion apps, then one row that opens the list of all apps.
fn related(panel: &mut Panel, ui: &mut Ui, snap: &Snapshot, card: Rect) {
    let t = ui.t;
    ui.card(card);
    let left = card.x + CARD_PAD;
    let inner = card.w - 48.0;
    ui.label_fit(HEADING, TEXT, left, card.y + 44.0, inner, t.related_title);
    ui.label_fit(NOTE, MUTED, left, card.y + 68.0, inner, t.related_hint);
    let mut row_y = card.y + 92.0;
    for entry in snap.apps.iter().filter(|entry| entry.related) {
        app_row(panel, ui, entry, Rect::new(left, row_y, inner, APP_ROW_H), false);
        row_y += APP_ROW_STEP;
    }
    all_apps_row(panel, ui, snap, Rect::new(left, row_y, inner, ALL_ROW_H));
    ui.label_fit(NOTE, MUTED, left, row_y + 116.0, inner, t.install_note);
}

/// The first four app icons, overlapping, and how many apps are still missing.
fn all_apps_row(panel: &mut Panel, ui: &mut Ui, snap: &Snapshot, row: Rect) {
    let t = ui.t;
    let missing = snap.apps.iter().filter(|entry| entry.state == AppState::Missing).count();
    let total = snap.apps.len();
    let count = if missing == 0 {
        fmt_token(t.all_installed_fmt, "total", &total.to_string())
    } else {
        fmt_token(&fmt_token(t.missing_fmt, "missing", &missing.to_string()), "total", &total.to_string())
    };
    ui.fill(row, R_SURFACE, ui.control_fill(row, draw::CONTROL));
    let mut icon_x = row.x + 16.0;
    for entry in snap.apps.iter().take(4) {
        app_icon(panel, ui, &entry.name, &entry.mono, &entry.icon_path, icon_x, row.y + 26.0, 36.0);
        icon_x += 22.0;
    }
    let text_x = icon_x + 24.0;
    let text_w = row.x + row.w - 40.0 - text_x;
    ui.label_fit(LEAD, TEXT, text_x, row.y + 38.0, text_w, t.all_title);
    ui.label_fit(NOTE, MUTED, text_x, row.y + 62.0, text_w, &count);
    draw::chevron(ui.cr, row.x + row.w - 28.0, row.y + 37.0, 14.0, SOFT, true, false);
    ui.hot(Hit::AppsList, row);
}

/// One app: icon, name and description, and on the right Install or a state chip.
/// `in_list` is the row inside the list dialog, which sits on a lighter surface.
fn app_row(panel: &mut Panel, ui: &mut Ui, entry: &Entry, row: Rect, in_list: bool) {
    let t = ui.t;
    let Rect { x, y, w, .. } = row;
    ui.fill(row, R_SURFACE, if in_list { draw::CONTROL } else { INSET });
    app_icon(panel, ui, &entry.name, &entry.mono, &entry.icon_path, x + 16.0, y + 22.0, 52.0);
    // Whatever sits on the right takes room from the text.
    let mut right = x + w - 16.0;
    if entry.state == AppState::Missing && !entry.busy {
        let install = Rect::new(right - 120.0, y + 24.0, 120.0, BUTTON_H);
        ui.button_hot(install, t.install, ButtonKind::Accent, Hit::AppInstall(entry.key.clone()));
        right -= 136.0;
    } else if entry.state != AppState::Unknown || entry.busy {
        let chip_w = state_chip(ui, entry, right, y, in_list);
        right -= chip_w + 12.0;
    }
    let text_x = x + 84.0;
    let text_w = (right - text_x).max(40.0);
    ui.label_fit(NAME, TEXT, text_x, y + 40.0, text_w, &entry.name);
    if in_list && entry.related {
        let tag_w = ui.text_width(TAG, t.related_tag) + 16.0;
        let name_w = ui.text_width(NAME, &entry.name);
        if name_w + 12.0 + tag_w < text_w {
            ui.fill(Rect::new(text_x + name_w + 10.0, y + 22.0, tag_w, 22.0), 11.0, draw::CONTROL);
            ui.label(TAG, draw::accent(), text_x + name_w + 18.0, y + 38.0, t.related_tag);
        }
    }
    let desc = if panel.lang == Lang::Ja { entry.desc_ja.as_str() } else { entry.desc_en.as_str() };
    let sub = if !in_list && entry.name == "frame-mic-tuner" { t.reason_tuner } else { desc };
    ui.label_fit(NOTE, MUTED, text_x, y + 66.0, text_w, sub);
}

/// Busy, Running (green, with a dot) or Installed, ending at `right`. Returns the chip's width.
fn state_chip(ui: &Ui, entry: &Entry, right: f64, row_y: f64, in_list: bool) -> f64 {
    let t = ui.t;
    let label = if entry.busy {
        t.chip_busy
    } else if entry.state == AppState::Running {
        t.chip_running
    } else {
        t.chip_installed
    };
    let ok = !entry.busy && entry.state == AppState::Running;
    let chip_w = 18.0 + ui.text_width(CHIP, label) + 28.0;
    let rest = if in_list { INSET } else { draw::CONTROL };
    ui.fill(Rect::new(right - chip_w, row_y + 28.0, chip_w, 40.0), 20.0, if ok { CHIP_OK_FILL } else { rest });
    if ok {
        ui.circle(right - chip_w + 16.0, row_y + 48.0, 5.0, CHIP_OK_TEXT);
    }
    ui.label(CHIP, if ok { CHIP_OK_TEXT } else { SOFT }, right - chip_w + if ok { 28.0 } else { 14.0 }, row_y + 52.0, label);
    chip_w
}

/// Every app in a scrolling list, and the button that opens the installer for all of them.
pub(crate) fn list(panel: &mut Panel, ui: &mut Ui, snap: &Snapshot) {
    let t = ui.t;
    let sheet = panel.sheet_rect(40.0);
    ui.dialog(panel.window(), sheet);
    let Rect { x, y, w, h } = sheet;
    ui.label(HEADING, TEXT, x + SHEET_PAD, y + 44.0, t.all_title);
    ui.label_fit(NOTE, MUTED, x + SHEET_PAD, y + 68.0, w - 100.0, t.list_sub);
    ui.close_button(x + w - 60.0, y + 20.0);
    let list_top = y + 88.0;
    let list_h = (h - 188.0).max(160.0);
    let content = snap.apps.len() as f64 * APP_ROW_STEP;
    panel.dialog_scroll = panel.dialog_scroll.clamp(0.0, (content - list_h).max(0.0));
    ui.clip(Rect::new(x + 16.0, list_top, w - 32.0, list_h));
    let mut row_y = list_top - panel.dialog_scroll;
    for entry in &snap.apps {
        if row_y + APP_ROW_H >= list_top && row_y <= list_top + list_h {
            app_row(panel, ui, entry, Rect::new(x + 24.0, row_y, w - 48.0, APP_ROW_H), true);
        }
        row_y += APP_ROW_STEP;
    }
    ui.unclip();
    ui.label_fit(SMALL, MUTED, x + SHEET_PAD, y + h - 78.0, w - 280.0, t.list_note);
    let label = if snap.menu_busy { t.chip_busy } else { t.open_installer };
    let installer = Rect::new(x + w - 250.0, y + h - 80.0, 214.0, 52.0);
    ui.button(installer, label, ButtonKind::Plain);
    if !snap.menu_busy {
        ui.hot(Hit::AppInstall(String::new()), installer);
    }
}

/// Shows the command the installer will run for `key` (every app when it is empty) and asks first.
pub(crate) fn confirm(panel: &mut Panel, ui: &mut Ui, snap: &Snapshot, key: &str) {
    let t = ui.t;
    let sheet = panel.dialog_rect(680.0, 476.0, 24.0);
    ui.dialog(panel.window(), sheet);
    let Rect { x, y, w, h } = sheet;
    let inner = w - 56.0;
    match snap.apps.iter().find(|entry| entry.key == key) {
        Some(entry) => {
            let title = fmt_token(t.confirm_install_fmt, "name", &entry.name);
            app_icon(panel, ui, &entry.name, &entry.mono, &entry.icon_path, x + SHEET_PAD, y + 24.0, 44.0);
            ui.label_fit(HEADING, TEXT, x + 84.0, y + 52.0, w - 120.0, &title);
        }
        None => ui.label_fit(HEADING, TEXT, x + SHEET_PAD, y + 52.0, inner, t.confirm_menu),
    }
    ui.label_fit(BODY, SOFT, x + SHEET_PAD, y + 100.0, inner, t.confirm_lead);
    let command = if key.is_empty() {
        "curl -fsSL https://frame.sasaken1102s.net | sh".to_string()
    } else {
        format!("curl -fsSL https://frame.sasaken1102s.net | sh -s -- install {key}")
    };
    ui.fill(Rect::new(x + SHEET_PAD, y + 118.0, inner, 64.0), 12.0, draw::BG);
    ui.label_fit(NOTE, TEXT, x + 40.0, y + 154.0, w - 80.0, &command);
    let notes = [if key.is_empty() { t.confirm_menu_body } else { t.confirm_progress }, t.confirm_no_sudo, t.confirm_close];
    let mut note_y = y + 200.0;
    for note in notes {
        ui.label_fit(NOTE, MUTED, x + SHEET_PAD, note_y, inner, note);
        note_y += 28.0;
    }
    let buttons_y = y + h - 76.0;
    ui.button_hot(Rect::new(x + SHEET_PAD, buttons_y, 180.0, BUTTON_H), t.cancel, ButtonKind::Plain, Hit::ConfirmNo);
    ui.button_hot(Rect::new(x + w - 230.0, buttons_y, 200.0, BUTTON_H), t.confirm_launch, ButtonKind::Accent, Hit::ConfirmYes(key.to_string()));
}
