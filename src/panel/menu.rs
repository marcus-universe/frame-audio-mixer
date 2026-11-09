//! Dropdowns for effect presets and the EQ analyzer. The open list draws above
//! everything else, and any press outside its rows closes it.

use super::{fill_round, Action, Panel};
use crate::config::{EffectSlot, EqView};
use crate::draw::{self, Fonts, Look, MUTED, R_CONTROL, TEXT};
use crate::fx::presets;
use crate::i18n::Text;
use cairo::Context;

const ROW_H: f64 = 40.0;
const MENU_PAD: f64 = 8.0;
const MIN_W: f64 = 220.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Menu {
    Preset { bus: usize, index: usize },
    EqView,
}

impl Menu {
    fn open_id(self) -> String {
        match self {
            Menu::Preset { bus, index } => format!("menu:preset:{bus}:{index}"),
            Menu::EqView => "menu:eqview".into(),
        }
    }
}

struct Row {
    label: String,
    id: String,
    selected: bool,
}

/// A control showing its current value, with a chevron. `x, y, w, h` anchor the list.
pub(crate) fn button(panel: &mut Panel, cr: &Context, fonts: &Fonts, menu: Menu, x: f64, y: f64, w: f64, h: f64, caption: &str) {
    let fill = match panel.look(x, y, w, h) {
        Look::Down => draw::CONTROL_DOWN,
        Look::Hover => draw::CONTROL_HOVER,
        Look::Rest => draw::CONTROL,
    };
    fill_round(cr, x, y, w, h, R_CONTROL.min(h / 2.0), fill);
    draw::label_fit(cr, fonts, true, 13.0, TEXT, x + 14.0, y + h / 2.0 + 5.0, w - 44.0, caption);
    draw::chevron(cr, x + w - 26.0, y + h / 2.0 - 2.0, 12.0, MUTED, false, panel.menu == Some(menu));
    if panel.menu == Some(menu) {
        panel.menu_anchor = (x, y, w, h);
    }
    panel.hot(&menu.open_id(), x, y, w, h);
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

fn view_key(view: EqView) -> usize {
    EqView::ALL.iter().position(|item| *item == view).unwrap_or(0)
}

fn rows(panel: &mut Panel, t: &Text, menu: Menu) -> Vec<Row> {
    match menu {
        Menu::EqView => EqView::ALL
            .iter()
            .map(|view| Row {
                label: view_name(t, *view).to_string(),
                id: format!("pick:eqview:{}", view_key(*view)),
                selected: *view == panel.config.eq_view,
            })
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
                    id: format!("pick:preset:{bus}:{index}:{row}"),
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

pub(crate) fn draw(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text) {
    let Some(menu) = panel.menu else { return };
    let rows = rows(panel, t, menu);
    let (pw, ph) = panel.size_f();
    panel.hot("menu-close", 0.0, 0.0, pw, ph);
    let (ax, ay, aw, ah) = panel.menu_anchor;
    let w = aw.max(MIN_W);
    let x = ax.min(pw - w - MENU_PAD).max(MENU_PAD);
    let h = rows.len() as f64 * ROW_H + 2.0 * MENU_PAD;
    let y = list_top(ay, ah, h, ph);
    fill_round(cr, x - 1.0, y - 1.0, w + 2.0, h + 2.0, R_CONTROL + 5.0, draw::BG);
    fill_round(cr, x, y, w, h, R_CONTROL + 4.0, draw::CONTROL);
    for (i, row) in rows.iter().enumerate() {
        let ry = y + MENU_PAD + i as f64 * ROW_H;
        if row.selected {
            fill_round(cr, x + 6.0, ry + 2.0, w - 12.0, ROW_H - 4.0, 10.0, draw::accent_tint());
        } else if panel.look(x, ry, w, ROW_H) != Look::Rest {
            fill_round(cr, x + 6.0, ry + 2.0, w - 12.0, ROW_H - 4.0, 10.0, draw::CONTROL_HOVER);
        }
        let ink = if row.selected { draw::accent() } else { TEXT };
        draw::label_fit(cr, fonts, row.selected, 15.0, ink, x + 16.0, ry + ROW_H / 2.0 + 5.0, w - 32.0, &row.label);
        panel.hot(&row.id, x, ry, w, ROW_H);
    }
}

impl Panel {
    /// Handles every menu id. None means `id` is not one of them.
    pub(super) fn handle_menu(&mut self, id: &str) -> Option<Action> {
        if id == "menu-close" {
            self.menu = None;
            return Some(Action::None);
        }
        if id == "menu:eqview" {
            self.menu = Some(Menu::EqView);
            return Some(Action::None);
        }
        if let Some(rest) = id.strip_prefix("menu:preset:") {
            let (bus, index) = rest.split_once(':')?;
            self.menu = Some(Menu::Preset { bus: bus.parse().ok()?, index: index.parse().ok()? });
            return Some(Action::None);
        }
        if let Some(rest) = id.strip_prefix("pick:eqview:") {
            self.menu = None;
            self.config.eq_view = *EqView::ALL.get(rest.parse::<usize>().ok()?)?;
            self.spectrum.clear();
            return Some(Action::Save);
        }
        if let Some(rest) = id.strip_prefix("pick:preset:") {
            self.menu = None;
            let mut parts = rest.split(':').map(str::parse::<usize>);
            let (bus, index, row) = (parts.next()?.ok()?, parts.next()?.ok()?, parts.next()?.ok()?);
            let slot = super::fx::bus_mut(&mut self.config, bus).chain.get_mut(index)?;
            let preset = presets::of(slot.kind).get(row)?;
            preset.apply(slot);
            return Some(Action::Save);
        }
        None
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
