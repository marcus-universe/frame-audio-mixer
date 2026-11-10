//! Language, the settings gear, Start with SteamVR, and a red Quit.

use super::{Hit, Panel, Ui};
use crate::draw::{self, ButtonKind, Font, Look, Rect, BUTTON, CARD, LABEL_SOFT, MUTED, R_CARD, R_CONTROL, TEXT};
use crate::i18n::LANGS;

const CONTROL_H: f64 = 48.0;
const PAD: f64 = 16.0;
/// The language dropdown starts right of its glyph.
const LANGUAGE_X: f64 = PAD + 36.0;
const MENU_ROW: f64 = 40.0;
const AUTOSTART_W: f64 = 160.0;
/// "Start with SteamVR", left of its toggle.
const HINT: Font = Font::regular(14.0);

pub(crate) fn draw(panel: &mut Panel, ui: &mut Ui, autostart: bool) {
    let t = ui.t;
    let footer = panel.stage().footer;
    ui.fill(footer, R_CARD, CARD);
    let top = footer.y + (footer.h - CONTROL_H) / 2.0;
    let mid = top + CONTROL_H / 2.0;

    ui.icon(draw::LANG_PATH, 16.0, footer.x + PAD + 4.0, mid - 10.0, 20.0, LABEL_SOFT);
    let language = Rect::new(footer.x + LANGUAGE_X, top, language_width(ui), CONTROL_H);
    ui.fill(language, R_CONTROL, ui.control_fill(language, draw::CONTROL));
    if panel.language_open {
        ui.stroke(language.inset(1.0), R_CONTROL - 1.0, draw::accent(), 2.0);
    }
    ui.label_fit(BUTTON, TEXT, language.x + 16.0, mid + 5.0, language.w - 52.0, panel.lang.endonym());
    draw::chevron(ui.cr, language.x + language.w - 28.0, mid - 2.0, 12.0, MUTED, false, panel.language_open);
    ui.hot(Hit::LanguageMenu, language);

    let gear = Rect::square(language.x + language.w + 12.0, top, CONTROL_H);
    ui.button(gear, "", ButtonKind::Plain);
    ui.icon(draw::SETTINGS_PATH, 24.0, gear.x + 13.0, gear.y + 13.0, 22.0, draw::SOFT);
    ui.hot(Hit::Settings, gear);

    // Quit turns solid red while it waits for the second press.
    let armed = panel.quit_armed();
    let quit_label = if armed { t.quit_again } else { t.footer_quit };
    let quit_w = (ui.text_width(BUTTON, quit_label) + 56.0).max(128.0);
    let quit = Rect::new(footer.x + footer.w - PAD - quit_w, top, quit_w, CONTROL_H);
    ui.button_hot(quit, quit_label, if armed { ButtonKind::DangerArmed } else { ButtonKind::Danger }, Hit::Quit);

    let auto = Rect::new(quit.x - 24.0 - AUTOSTART_W, top, AUTOSTART_W, CONTROL_H);
    let label_left = gear.x + gear.w + 16.0;
    let label_w = (auto.x - 16.0 - label_left).max(40.0);
    let used = ui.text_width(HINT, t.footer_start).min(label_w);
    ui.label_fit(HINT, LABEL_SOFT, auto.x - 16.0 - used, mid + 5.0, used, t.footer_start);
    ui.pill_toggle(auto, autostart, (t.on, t.off), (Hit::Autostart(true), Hit::Autostart(false)));
}

/// The widest endonym, and the chevron.
fn language_width(ui: &Ui) -> f64 {
    let widest = LANGS.iter().map(|lang| ui.text_width(BUTTON, lang.endonym())).fold(0.0, f64::max);
    (16.0 + widest + 44.0).ceil()
}

/// The list of languages, opening upward from the dropdown.
pub(crate) fn language_menu(panel: &mut Panel, ui: &mut Ui) {
    let footer = panel.stage().footer;
    let w = language_width(ui);
    let h = LANGS.len() as f64 * MENU_ROW + 16.0;
    let menu = Rect::new(footer.x + LANGUAGE_X, footer.y - h - 8.0, w, h);
    ui.fill(menu, R_CONTROL + 4.0, draw::CONTROL);
    for (i, lang) in LANGS.iter().enumerate() {
        let row = Rect::new(menu.x, menu.y + 8.0 + i as f64 * MENU_ROW, menu.w, MENU_ROW);
        let selected = *lang == panel.lang;
        if selected {
            ui.fill(row.inset_xy(6.0, 2.0), 10.0, draw::accent_tint());
        } else if ui.look(row) != Look::Rest {
            ui.fill(row.inset_xy(6.0, 2.0), 10.0, draw::CONTROL_HOVER);
        }
        let color = if selected { draw::accent() } else { TEXT };
        ui.label_mid(Font::weight(selected, BUTTON.size), color, row.x + 16.0, row.y + MENU_ROW / 2.0, lang.endonym());
        ui.hot(Hit::Language(*lang), row);
    }
}
