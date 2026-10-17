//! Language, the settings gear, Start with SteamVR, and a red Quit.

use super::{fill_round, stroke_round, Panel};
use crate::draw::{self, ButtonKind, Fonts, CARD, LABEL_SOFT, MUTED, TEXT};
use crate::i18n::{Text, LANGS};
use cairo::Context;

pub(crate) fn draw(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, autostart: bool) {
    let (x, y, w, h) = (28.0, 674.0, 1144.0, 88.0);
    fill_round(cr, x, y, w, h, 18.0, CARD);
    let control_y = y + (h - 56.0) / 2.0;
    draw::icon(cr, draw::LANG_PATH, 16.0, x + 16.0, control_y + 16.0, 22.0, LABEL_SOFT);
    let drop = (x + 48.0, control_y, 220.0, 56.0);
    fill_round(cr, drop.0, drop.1, drop.2, drop.3, 16.0, draw::CONTROL);
    stroke_round(cr, drop.0, drop.1, drop.2, drop.3, 16.0, if panel.language_open { draw::ACCENT } else { draw::BORDER }, 2.0);
    draw::label_fit(cr, fonts, true, 16.0, TEXT, drop.0 + 16.0, drop.1 + 34.0, drop.2 - 48.0, panel.lang.endonym());
    draw::chevron(cr, drop.0 + drop.2 - 28.0, drop.1 + 24.0, 12.0, MUTED, false, panel.language_open);
    panel.hot("lang", drop.0, drop.1, drop.2, drop.3);

    let gear = (drop.0 + drop.2 + 12.0, control_y, 56.0, 56.0);
    panel.button(cr, fonts, gear.0, gear.1, gear.2, gear.3, "", ButtonKind::Plain);
    draw::icon(cr, draw::SETTINGS_PATH, 24.0, gear.0 + 16.0, gear.1 + 16.0, 24.0, TEXT);
    panel.hot("settings", gear.0, gear.1, gear.2, gear.3);

    let quit_label = if panel.quit_armed() { t.quit_again } else { t.footer_quit };
    let quit_w = if panel.quit_armed() { 240.0 } else { 140.0 };
    let quit = (x + w - 16.0 - quit_w, control_y, quit_w, 56.0);
    let kind = if panel.quit_armed() { ButtonKind::DangerArmed } else { ButtonKind::Danger };
    panel.button(cr, fonts, quit.0, quit.1, quit.2, quit.3, quit_label, kind);
    panel.hot("quit", quit.0, quit.1, quit.2, quit.3);

    let auto_w = 180.0;
    let auto_x = quit.0 - 16.0 - auto_w;
    segment(panel, cr, fonts, auto_x, control_y, auto_w, autostart, t.on, t.off);
    draw::label_fit(cr, fonts, false, 15.0, LABEL_SOFT, gear.0 + gear.2 + 16.0, control_y + 34.0, (auto_x - 12.0) - (gear.0 + gear.2 + 16.0), t.footer_start);
}

fn segment(panel: &mut Panel, cr: &Context, fonts: &Fonts, x: f64, y: f64, w: f64, on: bool, yes: &str, no: &str) {
    fill_round(cr, x, y, w, 56.0, 16.0, draw::CONTROL);
    let half = w / 2.0;
    if on {
        fill_round(cr, x + 4.0, y + 4.0, half - 6.0, 48.0, 14.0, draw::ACCENT);
    } else {
        fill_round(cr, x + half + 2.0, y + 4.0, half - 6.0, 48.0, 14.0, draw::ACCENT);
    }
    let yes_c = if on { draw::ON_ACCENT } else { draw::SOFT };
    let no_c = if on { draw::SOFT } else { draw::ON_ACCENT };
    let yw = draw::text_width(cr, fonts, true, 16.0, yes);
    let nw = draw::text_width(cr, fonts, true, 16.0, no);
    draw::label(cr, fonts, true, 16.0, yes_c, x + (half - yw) / 2.0, y + 34.0, yes);
    draw::label(cr, fonts, true, 16.0, no_c, x + half + (half - nw) / 2.0, y + 34.0, no);
    panel.hot("auto-on", x, y, half, 56.0);
    panel.hot("auto-off", x + half, y, half, 56.0);
}

pub(crate) fn language_menu(panel: &mut Panel, cr: &Context, fonts: &Fonts) {
    let x = 76.0;
    let row_h = 40.0;
    let h = LANGS.len() as f64 * row_h + 12.0;
    let y = 674.0 - h - 8.0;
    fill_round(cr, x, y, 220.0, h, 12.0, draw::INSET);
    stroke_round(cr, x, y, 220.0, h, 12.0, draw::BORDER, 2.0);
    for (i, lang) in LANGS.iter().enumerate() {
        let row = y + 6.0 + i as f64 * row_h;
        if *lang == panel.lang {
            fill_round(cr, x + 6.0, row, 208.0, 36.0, 8.0, CARD);
        }
        draw::label(cr, fonts, *lang == panel.lang, 16.0, TEXT, x + 16.0, row + 24.0, lang.endonym());
        panel.hot(&format!("lang:{}", lang.code()), x, row, 220.0, row_h);
    }
}
