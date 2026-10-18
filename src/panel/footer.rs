//! Language, the settings gear, Start with SteamVR, and a red Quit.

use super::{fill_round, stroke_round, Panel};
use crate::draw::{self, ButtonKind, Fonts, CARD, LABEL_SOFT, MUTED, R_CARD, R_CONTROL, TEXT};
use crate::i18n::{Text, LANGS};
use cairo::Context;

const CONTROL_H: f64 = 48.0;
const PAD: f64 = 16.0;

pub(crate) fn draw(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, autostart: bool) {
    let (x, y, w, h) = panel.stage().footer;
    fill_round(cr, x, y, w, h, R_CARD, CARD);
    let control_y = y + (h - CONTROL_H) / 2.0;
    let mid = control_y + CONTROL_H / 2.0;
    draw::icon(cr, draw::LANG_PATH, 16.0, x + PAD + 4.0, mid - 10.0, 20.0, LABEL_SOFT);
    let drop = (x + PAD + 36.0, control_y, language_width(cr, fonts), CONTROL_H);
    let look = panel.look(drop.0, drop.1, drop.2, drop.3);
    let fill = match look {
        draw::Look::Down => draw::CONTROL_DOWN,
        draw::Look::Hover => draw::CONTROL_HOVER,
        draw::Look::Rest => draw::CONTROL,
    };
    fill_round(cr, drop.0, drop.1, drop.2, drop.3, R_CONTROL, fill);
    if panel.language_open {
        stroke_round(cr, drop.0 + 1.0, drop.1 + 1.0, drop.2 - 2.0, drop.3 - 2.0, R_CONTROL - 1.0, draw::accent(), 2.0);
    }
    draw::label_fit(cr, fonts, true, 15.0, TEXT, drop.0 + 16.0, mid + 5.0, drop.2 - 52.0, panel.lang.endonym());
    draw::chevron(cr, drop.0 + drop.2 - 28.0, mid - 2.0, 12.0, MUTED, false, panel.language_open);
    panel.hot("lang", drop.0, drop.1, drop.2, drop.3);

    let gear = (drop.0 + drop.2 + 12.0, control_y, CONTROL_H, CONTROL_H);
    panel.button(cr, fonts, gear.0, gear.1, gear.2, gear.3, "", ButtonKind::Plain);
    draw::icon(cr, draw::SETTINGS_PATH, 24.0, gear.0 + 13.0, gear.1 + 13.0, 22.0, draw::SOFT);
    panel.hot("settings", gear.0, gear.1, gear.2, gear.3);

    let quit_label = if panel.quit_armed() { t.quit_again } else { t.footer_quit };
    let quit_w = (draw::text_width(cr, fonts, true, 15.0, quit_label) + 56.0).max(128.0);
    let quit = (x + w - PAD - quit_w, control_y, quit_w, CONTROL_H);
    let kind = if panel.quit_armed() { ButtonKind::DangerArmed } else { ButtonKind::Danger };
    panel.button(cr, fonts, quit.0, quit.1, quit.2, quit.3, quit_label, kind);
    panel.hot("quit", quit.0, quit.1, quit.2, quit.3);

    let auto_w = 160.0;
    let auto_x = quit.0 - 24.0 - auto_w;
    let label_left = gear.0 + gear.2 + 16.0;
    let label_w = (auto_x - 16.0 - label_left).max(40.0);
    let natural = draw::text_width(cr, fonts, false, 14.0, t.footer_start);
    let used = natural.min(label_w);
    draw::label_fit(cr, fonts, false, 14.0, LABEL_SOFT, auto_x - 16.0 - used, mid + 5.0, used, t.footer_start);
    super::pill_toggle(panel, cr, fonts, auto_x, control_y, auto_w, CONTROL_H, autostart, (t.on, t.off), ("auto-on", "auto-off"));
}

/// The widest endonym, and the chevron.
fn language_width(cr: &Context, fonts: &Fonts) -> f64 {
    let widest = LANGS.iter().map(|lang| draw::text_width(cr, fonts, true, 15.0, lang.endonym())).fold(0.0, f64::max);
    (16.0 + widest + 44.0).ceil()
}

pub(crate) fn language_menu(panel: &mut Panel, cr: &Context, fonts: &Fonts) {
    let (fx, fy, _, _) = panel.stage().footer;
    let x = fx + PAD + 36.0;
    let menu_w = language_width(cr, fonts);
    let row_h = 40.0;
    let h = LANGS.len() as f64 * row_h + 16.0;
    let y = fy - h - 8.0;
    fill_round(cr, x, y, menu_w, h, R_CONTROL + 4.0, draw::CONTROL);
    for (i, lang) in LANGS.iter().enumerate() {
        let row = y + 8.0 + i as f64 * row_h;
        let selected = *lang == panel.lang;
        let look = panel.look(x, row, menu_w, row_h);
        if selected {
            fill_round(cr, x + 6.0, row + 2.0, menu_w - 12.0, row_h - 4.0, 10.0, draw::accent_tint());
        } else if look != draw::Look::Rest {
            fill_round(cr, x + 6.0, row + 2.0, menu_w - 12.0, row_h - 4.0, 10.0, draw::CONTROL_HOVER);
        }
        let color = if selected { draw::accent() } else { TEXT };
        draw::label_mid(cr, fonts, selected, 15.0, color, x + 16.0, row + row_h / 2.0, lang.endonym());
        panel.hot(&format!("lang:{}", lang.code()), x, row, menu_w, row_h);
    }
}
