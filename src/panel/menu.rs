//! Dropdowns for effect presets and the EQ analyzer. The open list draws above
//! everything else, and any press outside its rows closes it.

use super::{Hit, Panel, Ui};
use crate::config::{EffectSlot, EqView};
use crate::draw::{self, Font, Rect, BUTTON, MUTED, R_CONTROL, TEXT};
use crate::fx::presets;
use crate::i18n::Text;

const ROW_H: f64 = 40.0;
const MENU_PAD: f64 = 8.0;
const MIN_W: f64 = 220.0;
/// The current value on a dropdown button.
const VALUE: Font = Font::bold(13.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Menu {
    Preset { bus: usize, index: usize },
    EqView,
}

struct Row {
    label: String,
    hit: Hit,
    selected: bool,
}

/// A control showing its current value, with a chevron. `rect` anchors the list.
pub(crate) fn button(panel: &mut Panel, ui: &mut Ui, menu: Menu, rect: Rect, caption: &str) {
    let open = panel.menu == Some(menu);
    ui.fill(rect, R_CONTROL.min(rect.h / 2.0), ui.control_fill(rect, draw::CONTROL));
    ui.label_fit(VALUE, TEXT, rect.x + 14.0, rect.y + rect.h / 2.0 + 5.0, rect.w - 44.0, caption);
    draw::chevron(ui.cr, rect.x + rect.w - 26.0, rect.y + rect.h / 2.0 - 2.0, 12.0, MUTED, false, open);
    if open {
        panel.menu_anchor = rect;
    }
    ui.hot(Hit::OpenMenu(menu), rect);
}

/// "Preset: Church", or "Preset: Custom" once a value has been moved.
pub(crate) fn preset_caption(slot: &EffectSlot, lang: crate::i18n::Lang, t: &Text) -> String {
    let name = presets::active(slot).map_or(t.preset_custom, |preset| preset.name(lang));
    format!("{}: {name}", t.preset)
}

pub(crate) fn view_name(t: &Text, view: EqView) -> &'static str {
    match view {
        EqView::Off => t.view_off,
        EqView::Line => t.view_line,
        EqView::Area => t.view_area,
        EqView::Bars => t.view_bars,
        EqView::Heat => t.view_heat,
    }
}

fn rows(panel: &mut Panel, t: &Text, menu: Menu) -> Vec<Row> {
    match menu {
        Menu::EqView => EqView::ALL
            .iter()
            .enumerate()
            .map(|(row, view)| Row { label: view_name(t, *view).to_string(), hit: Hit::PickEqView(row), selected: *view == panel.config.eq_view })
            .collect(),
        Menu::Preset { bus, index } => {
            let lang = panel.lang;
            let Some(slot) = super::fx::bus_mut(&mut panel.config, bus).chain.get(index) else { return Vec::new() };
            let active = presets::active(slot);
            presets::of(slot.kind)
                .iter()
                .enumerate()
                .map(|(row, preset)| Row {
                    label: preset.name(lang).to_string(),
                    hit: Hit::PickPreset { bus, index, row },
                    selected: active.is_some_and(|found| std::ptr::eq(found, preset)),
                })
                .collect()
        }
    }
}

/// Below the button when it fits, otherwise above, otherwise wherever the panel has room.
fn list_top(anchor_y: f64, anchor_h: f64, list_h: f64, panel_h: f64) -> f64 {
    let below = anchor_y + anchor_h + 4.0;
    if below + list_h <= panel_h - MENU_PAD {
        return below;
    }
    let above = anchor_y - 4.0 - list_h;
    if above >= MENU_PAD {
        return above;
    }
    (panel_h - list_h - MENU_PAD).max(MENU_PAD)
}

/// The open dropdown's list, on top of everything.
pub(crate) fn draw(panel: &mut Panel, ui: &mut Ui) {
    let Some(menu) = panel.menu else { return };
    let rows = rows(panel, ui.t, menu);
    let window = panel.window();
    ui.hot(Hit::MenuClose, window);
    let anchor = panel.menu_anchor;
    let w = anchor.w.max(MIN_W);
    let x = anchor.x.min(window.w - w - MENU_PAD).max(MENU_PAD);
    let h = rows.len() as f64 * ROW_H + 2.0 * MENU_PAD;
    let list = Rect::new(x, list_top(anchor.y, anchor.h, h, window.h), w, h);
    // A dark rim separates the list from the controls under it.
    ui.fill(list.inset(-1.0), R_CONTROL + 5.0, draw::BG);
    ui.fill(list, R_CONTROL + 4.0, draw::CONTROL);
    for (i, row) in rows.into_iter().enumerate() {
        let area = Rect::new(list.x, list.y + MENU_PAD + i as f64 * ROW_H, list.w, ROW_H);
        let highlight = area.inset_xy(6.0, 2.0);
        if row.selected {
            ui.fill(highlight, 10.0, draw::accent_tint());
        } else if ui.look(area) != draw::Look::Rest {
            ui.fill(highlight, 10.0, draw::CONTROL_HOVER);
        }
        let ink = if row.selected { draw::accent() } else { TEXT };
        ui.label_fit(Font::weight(row.selected, BUTTON.size), ink, area.x + 16.0, area.y + ROW_H / 2.0 + 5.0, area.w - 32.0, &row.label);
        ui.hot(row.hit, area);
    }
}

pub(crate) fn self_test() -> Result<(), String> {
    if list_top(100.0, 40.0, 200.0, 788.0) != 144.0 {
        return Err("menu below".into());
    }
    if list_top(600.0, 40.0, 200.0, 788.0) != 396.0 {
        return Err("menu above".into());
    }
    if list_top(100.0, 40.0, 780.0, 788.0) < MENU_PAD - 1.0 {
        return Err("menu off panel".into());
    }
    Ok(())
}
