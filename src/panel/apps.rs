//! Apps & updates, laid out like Frame Mic Tuner. This app is Frame Audio Mixer.
//! Frame Mic Tuner is the companion. The rest of the list is sasaken's Frame apps.

use super::{backdrop, fill_round, fmt_token, paint_app_icon, stroke_round, Panel};
use crate::apps::{AppState, Entry};
use crate::draw::{self, ButtonKind, Fonts, BORDER, CARD, CHIP_OK_FILL, CHIP_OK_TEXT, DANGER_TEXT, INSET, MUTED, SOFT, TEXT};
use crate::i18n::{Lang, Text};
use crate::updater::{UpdateState, UpdateView, VERSION};
use crate::worker::Snapshot;
use cairo::Context;

const APP: (f64, f64, f64, f64) = (28.0, 92.0, 560.0, 568.0);
const RELATED: (f64, f64, f64, f64) = (604.0, 92.0, 568.0, 568.0);

pub(crate) fn draw(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot) {
    this_app(panel, cr, fonts, t, snap);
    related(panel, cr, fonts, t, snap);
}

fn this_app(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot) {
    let (x, y, w, h) = APP;
    draw::card(cr, x, y, w, h);
    let left = x + 22.0;
    draw::label(cr, fonts, true, 22.0, TEXT, left, y + 42.0, t.this_app);
    fill_round(cr, left, y + 64.0, 64.0, 64.0, 16.0, draw::ACCENT);
    draw::icon(cr, draw::HEADPHONES_PATH, 16.0, left + 16.0, y + 80.0, 32.0, draw::ON_ACCENT);
    draw::label(cr, fonts, true, 20.0, TEXT, left + 80.0, y + 90.0, "Frame Audio Mixer");
    draw::label(cr, fonts, false, 14.0, MUTED, left + 80.0 + 196.0, y + 90.0, VERSION);
    draw::label_fit(cr, fonts, false, 13.0, MUTED, left + 80.0, y + 114.0, w - 120.0, t.this_app_desc);

    update_box(panel, cr, fonts, t, &snap.update, left, y + 152.0, w - 44.0);

    draw::label_fit(cr, fonts, true, 16.0, TEXT, left, y + 310.0, 200.0, t.apps_check);
    let on = panel.config.check_updates;
    let seg = (left + 210.0, y + 284.0, 180.0, 48.0);
    fill_round(cr, seg.0, seg.1, seg.2, seg.3, 14.0, draw::CONTROL);
    if on {
        fill_round(cr, seg.0 + 4.0, seg.1 + 4.0, 86.0, 40.0, 12.0, draw::ACCENT);
    } else {
        fill_round(cr, seg.0 + 90.0, seg.1 + 4.0, 86.0, 40.0, 12.0, draw::ACCENT);
    }
    draw::label(cr, fonts, true, 16.0, if on { draw::ON_ACCENT } else { SOFT }, seg.0 + 28.0, seg.1 + 30.0, t.on);
    draw::label(cr, fonts, true, 16.0, if on { SOFT } else { draw::ON_ACCENT }, seg.0 + 118.0, seg.1 + 30.0, t.off);
    panel.hot("upd-on", seg.0, seg.1, 90.0, 48.0);
    panel.hot("upd-off", seg.0 + 90.0, seg.1, 90.0, 48.0);
    draw::label_fit(cr, fonts, false, 13.0, MUTED, left, y + 360.0, w - 44.0, t.list_note);
    cr.set_source_rgb(draw::DIVIDER.r, draw::DIVIDER.g, draw::DIVIDER.b);
    cr.rectangle(left, y + 430.0, w - 44.0, 1.0);
    let _ = cr.fill();
    draw::label(cr, fonts, false, 13.0, MUTED, left, y + 460.0, t.help_title);
    draw::label(cr, fonts, false, 14.0, SOFT, left, y + 484.0, "github.com/marcus-universe/frame-audio-mixer");
}

fn update_box(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, update: &UpdateView, x: f64, y: f64, w: f64) {
    fill_round(cr, x, y, w, 112.0, 16.0, INSET);
    let (line, bad) = update_line(t, update);
    draw::label_fit(cr, fonts, true, 16.0, if bad { DANGER_TEXT } else { TEXT }, x + 16.0, y + 36.0, w - 200.0, &line);
    if !update.message.is_empty() && update.state != UpdateState::Available {
        draw::label_fit(cr, fonts, false, 13.0, MUTED, x + 16.0, y + 62.0, w - 200.0, &update.message);
    }
    panel.button(cr, fonts, x + w - 168.0, y + 28.0, 148.0, 48.0, t.check_now, ButtonKind::Plain);
    panel.hot("upd-now", x + w - 168.0, y + 28.0, 148.0, 48.0);
    if update.installable && update.state == UpdateState::Available {
        let caption = fmt_token(t.update_to_fmt, "version", &update.latest);
        panel.button(cr, fonts, x + 16.0, y + 64.0, 220.0, 36.0, &caption, ButtonKind::Accent);
        panel.hot("upd-install", x + 16.0, y + 64.0, 220.0, 36.0);
    }
}

fn update_line(t: &Text, update: &UpdateView) -> (String, bool) {
    match update.state {
        UpdateState::UpToDate => (fmt_token(t.up_to_date_fmt, "version", &crate::updater::VERSION), false),
        UpdateState::Available => (fmt_token(t.update_to_fmt, "version", &update.latest), false),
        UpdateState::Installing => (format!("{} {}", t.installing, update.step), false),
        UpdateState::Installed => (fmt_token(t.up_to_date_fmt, "version", &update.version), false),
        UpdateState::CheckFailed | UpdateState::InstallFailed => (t.apps_available.to_string(), true),
        UpdateState::Unknown => (t.apps_current.to_string(), false),
    }
}

fn related(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot) {
    let (x, y, w, _h) = RELATED;
    draw::card(cr, x, y, w, _h);
    let left = x + 22.0;
    let inner = w - 44.0;
    draw::label_fit(cr, fonts, true, 22.0, TEXT, left, y + 42.0, inner, t.related_title);
    draw::label_fit(cr, fonts, false, 13.0, MUTED, left, y + 66.0, inner, t.related_hint);
    let mut row_y = y + 88.0;
    for entry in snap.apps.iter().filter(|entry| entry.related).cloned().collect::<Vec<_>>() {
        app_row(panel, cr, fonts, t, &entry, left, row_y, inner, false);
        row_y += 104.0;
    }
    let missing = snap.apps.iter().filter(|entry| entry.state == AppState::Missing).count();
    let total = snap.apps.len();
    let count = if missing == 0 {
        fmt_token(t.all_installed_fmt, "total", &total.to_string())
    } else {
        fmt_token(&fmt_token(t.missing_fmt, "missing", &missing.to_string()), "total", &total.to_string())
    };
    fill_round(cr, left, row_y, inner, 88.0, 16.0, draw::CONTROL);
    stroke_round(cr, left, row_y, inner, 88.0, 16.0, BORDER, 2.0);
    let mut icon_x = left + 16.0;
    for entry in snap.apps.iter().take(4) {
        paint_app_icon(panel, cr, fonts, &entry.name, &entry.mono, &entry.icon_path, icon_x, row_y + 26.0, 36.0);
        icon_x += 22.0;
    }
    let text_x = icon_x + 24.0;
    draw::label_fit(cr, fonts, true, 16.0, TEXT, text_x, row_y + 38.0, left + inner - 40.0 - text_x, t.all_title);
    draw::label_fit(cr, fonts, false, 13.0, MUTED, text_x, row_y + 62.0, left + inner - 40.0 - text_x, &count);
    draw::chevron(cr, left + inner - 28.0, row_y + 36.0, 14.0, TEXT, true, false);
    panel.hot("apps-open", left, row_y, inner, 88.0);
    draw::label_fit(cr, fonts, false, 13.0, MUTED, left, row_y + 112.0, inner, t.install_note);
}

fn app_row(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, entry: &Entry, x: f64, y: f64, w: f64, in_list: bool) {
    fill_round(cr, x, y, w, 96.0, 16.0, INSET);
    paint_app_icon(panel, cr, fonts, &entry.name, &entry.mono, &entry.icon_path, x + 16.0, y + 22.0, 52.0);
    let mut right = x + w - 16.0;
    if entry.state == AppState::Missing && !entry.busy {
        panel.button(cr, fonts, right - 120.0, y + 24.0, 120.0, 48.0, t.install, ButtonKind::Accent);
        panel.hot(&format!("app:{}", entry.key), right - 120.0, y + 24.0, 120.0, 48.0);
        right -= 136.0;
    } else if entry.state != AppState::Unknown || entry.busy {
        let label = if entry.busy {
            t.chip_busy
        } else if entry.state == AppState::Running {
            t.chip_running
        } else {
            t.chip_installed
        };
        let ok = !entry.busy && entry.state == AppState::Running;
        let chip_w = 18.0 + draw::text_width(cr, fonts, true, 14.0, label) + 28.0;
        fill_round(cr, right - chip_w, y + 28.0, chip_w, 40.0, 20.0, if ok { CHIP_OK_FILL } else { draw::CONTROL });
        if ok {
            draw::fill_circle(cr, right - chip_w + 16.0, y + 48.0, 5.0, CHIP_OK_TEXT);
        }
        draw::label(cr, fonts, true, 14.0, if ok { CHIP_OK_TEXT } else { SOFT }, right - chip_w + if ok { 28.0 } else { 14.0 }, y + 52.0, label);
        right -= chip_w + 12.0;
    }
    let text_x = x + 84.0;
    let text_w = (right - text_x).max(40.0);
    draw::label_fit(cr, fonts, true, 18.0, TEXT, text_x, y + 40.0, text_w, &entry.name);
    if in_list && entry.related {
        let tag_w = draw::text_width(cr, fonts, true, 12.0, t.related_tag) + 16.0;
        let name_w = draw::text_width(cr, fonts, true, 18.0, &entry.name);
        if name_w + 12.0 + tag_w < text_w {
            fill_round(cr, text_x + name_w + 10.0, y + 22.0, tag_w, 22.0, 11.0, draw::CONTROL);
            draw::label(cr, fonts, true, 12.0, draw::ACCENT, text_x + name_w + 18.0, y + 38.0, t.related_tag);
        }
    }
    let desc = if panel.lang == Lang::Ja { entry.desc_ja.as_str() } else { entry.desc_en.as_str() };
    let sub = if !in_list && entry.name == "frame-mic-tuner" { t.reason_tuner } else { desc };
    draw::label_fit(cr, fonts, false, 13.0, MUTED, text_x, y + 66.0, text_w, sub);
}

pub(crate) fn list(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot) {
    backdrop(panel, cr);
    let (x, y, w, h) = (56.0, 56.0, 1088.0, 676.0);
    fill_round(cr, x, y, w, h, 18.0, CARD);
    stroke_round(cr, x, y, w, h, 18.0, BORDER, 2.0);
    draw::label(cr, fonts, true, 22.0, TEXT, x + 28.0, y + 40.0, t.all_title);
    draw::label_fit(cr, fonts, false, 13.0, MUTED, x + 28.0, y + 64.0, w - 80.0, t.list_sub);
    draw::label(cr, fonts, true, 20.0, TEXT, x + w - 48.0, y + 44.0, "×");
    panel.hot("dismiss", x + w - 56.0, y + 16.0, 40.0, 40.0);
    let list_top = y + 88.0;
    let list_h = 400.0;
    let content = snap.apps.len() as f64 * 104.0;
    panel.dialog_scroll = panel.dialog_scroll.clamp(0.0, (content - list_h).max(0.0));
    let _ = cr.save();
    cr.rectangle(x + 16.0, list_top, w - 32.0, list_h);
    let _ = cr.clip();
    let mut row = list_top - panel.dialog_scroll;
    let apps = snap.apps.clone();
    for entry in &apps {
        if row + 96.0 >= list_top && row <= list_top + list_h {
            app_row(panel, cr, fonts, t, entry, x + 24.0, row, w - 48.0, true);
        }
        row += 104.0;
    }
    let _ = cr.restore();
    draw::label_fit(cr, fonts, false, 12.0, MUTED, x + 28.0, y + h - 78.0, w - 280.0, t.list_note);
    let label = if snap.menu_busy { t.chip_busy } else { t.open_installer };
    panel.button(cr, fonts, x + w - 250.0, y + h - 80.0, 214.0, 52.0, label, ButtonKind::Plain);
    if !snap.menu_busy {
        panel.hot("app:", x + w - 250.0, y + h - 80.0, 214.0, 52.0);
    }
    panel.hot("noop", x, y, w, h);
}

pub(crate) fn confirm(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot, key: &str) {
    backdrop(panel, cr);
    let (x, y, w, h) = (260.0, 150.0, 680.0, 476.0);
    fill_round(cr, x, y, w, h, 18.0, CARD);
    stroke_round(cr, x, y, w, h, 18.0, BORDER, 2.0);
    let entry = snap.apps.iter().find(|entry| entry.key == key);
    let title = if let Some(entry) = entry {
        fmt_token(t.confirm_install_fmt, "name", &entry.name)
    } else {
        t.confirm_menu.to_string()
    };
    if let Some(entry) = entry {
        paint_app_icon(panel, cr, fonts, &entry.name, &entry.mono, &entry.icon_path, x + 28.0, y + 24.0, 44.0);
        draw::label_fit(cr, fonts, true, 20.0, TEXT, x + 84.0, y + 52.0, w - 120.0, &title);
    } else {
        draw::label_fit(cr, fonts, true, 20.0, TEXT, x + 28.0, y + 52.0, w - 56.0, &title);
    }
    draw::label_fit(cr, fonts, false, 15.0, SOFT, x + 28.0, y + 100.0, w - 56.0, t.confirm_lead);
    let command = if key.is_empty() {
        "curl -fsSL https://frame.sasaken1102s.net | sh".to_string()
    } else {
        format!("curl -fsSL https://frame.sasaken1102s.net | sh -s -- install {key}")
    };
    fill_round(cr, x + 28.0, y + 118.0, w - 56.0, 64.0, 12.0, draw::BG);
    draw::label_fit(cr, fonts, false, 13.0, TEXT, x + 40.0, y + 154.0, w - 80.0, &command);
    let notes = [if key.is_empty() { t.confirm_menu_body } else { t.confirm_progress }, t.confirm_no_sudo, t.confirm_close];
    let mut ny = y + 200.0;
    for note in notes {
        draw::label_fit(cr, fonts, false, 13.0, MUTED, x + 28.0, ny, w - 56.0, note);
        ny += 28.0;
    }
    panel.button(cr, fonts, x + 28.0, y + h - 76.0, 180.0, 48.0, t.cancel, ButtonKind::Plain);
    panel.hot("confirm-no", x + 28.0, y + h - 76.0, 180.0, 48.0);
    panel.button(cr, fonts, x + w - 230.0, y + h - 76.0, 200.0, 48.0, t.confirm_launch, ButtonKind::Accent);
    panel.hot(&format!("confirm-yes:{key}"), x + w - 230.0, y + h - 76.0, 200.0, 48.0);
    panel.hot("noop", x, y, w, h);
}
