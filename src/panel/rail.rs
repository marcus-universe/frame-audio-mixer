//! Right-hand rail: Outputs (with Mix Check), FX1, FX2.

use super::{Drawer, Hit, Panel, Ui};
use crate::draw::{self, Look, Rect, Rgb, INSET, MUTED, R_CONTROL, TEXT};

const TAB: f64 = 40.0;
const GAP: f64 = 8.0;
const GLYPH: f64 = 22.0;

/// One tab: its glyph, its colors, and the drawer it opens.
struct Tab {
    drawer: Drawer,
    path: &'static str,
    view: f64,
    ink: Rgb,
    fill: Rgb,
    /// The tone the fill moves toward under the pointer.
    hover: Rgb,
}

pub(crate) fn draw(panel: &mut Panel, ui: &mut Ui) {
    let rail = panel.stage().rail;
    if rail.w <= 0.0 || rail.h <= 0.0 {
        return;
    }
    ui.fill(rail, draw::R_SURFACE, INSET);
    let open = panel.drawer_shows_outputs();
    let (ink, fill) = if open { (TEXT, draw::CARD) } else { (MUTED, INSET) };
    let outputs = Tab { drawer: Drawer::Outputs, path: draw::SPEAKER_PATH, view: 256.0, ink, fill, hover: draw::CONTROL_HOVER };
    // An effect tab takes the color of its bus, and is filled with it while open.
    let fx_tab = |bus: usize, drawer: Drawer, color: Rgb| {
        let open = panel.drawer_shows_fx(bus);
        let ink = if open { draw::ink_on(color) } else { color };
        let fill = if open { color } else { INSET };
        Tab { drawer, path: draw::FX_TAG_PATH, view: 24.0, ink, fill, hover: color }
    };
    let tabs = [outputs, fx_tab(0, Drawer::FxA, draw::FX1), fx_tab(1, Drawer::FxB, draw::FX2)];
    let x = rail.x + (rail.w - TAB) / 2.0;
    let mut y = rail.y + 12.0;
    for tab in tabs {
        paint_tab(ui, Rect::square(x, y, TAB), &tab);
        y += TAB + GAP;
    }
}

fn paint_tab(ui: &mut Ui, rect: Rect, tab: &Tab) {
    let fill = match ui.look(rect) {
        Look::Down => draw::mix(tab.fill, TEXT, 0.12),
        Look::Hover => draw::mix(tab.fill, tab.hover, 0.35),
        Look::Rest => tab.fill,
    };
    ui.fill(rect, R_CONTROL, fill);
    ui.icon(tab.path, tab.view, rect.x + (rect.w - GLYPH) / 2.0, rect.y + (rect.h - GLYPH) / 2.0, GLYPH, tab.ink);
    ui.hot(Hit::Drawer(tab.drawer), rect);
}
