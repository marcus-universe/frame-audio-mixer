//! Right-hand rail: Mix Check, then the two effect buses.

use super::{fill_round, Panel};
use crate::draw::{self, Fonts, INSET, MUTED, R_CONTROL, TEXT};
use crate::i18n::Text;
use cairo::Context;

const TAB: f64 = 40.0;
const GAP: f64 = 8.0;

pub(crate) fn draw(panel: &mut Panel, cr: &Context, _fonts: &Fonts, _t: &Text) {
    let (x, y, w, h) = panel.stage().rail;
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    fill_round(cr, x, y, w, h, draw::R_SURFACE, INSET);
    let cx = x + (w - TAB) / 2.0;
    let mut ty = y + 12.0;
    tab(panel, cr, cx, ty, TAB, "rail-check", draw::EAR_PATH, 24.0, TEXT, panel.drawer_shows_check());
    ty += TAB + GAP;
    for bus in [0usize, 1] {
        let color = draw::bus_color(bus);
        let open = panel.drawer_shows_fx(bus);
        let ink = if open { draw::ink_on(color) } else { color };
        let fill = if open { color } else { INSET };
        tab_fill(panel, cr, cx, ty, TAB, &format!("rail-fx-{bus}"), draw::FX_TAG_PATH, 24.0, ink, fill, color);
        ty += TAB + GAP;
    }
}

fn tab(panel: &mut Panel, cr: &Context, x: f64, y: f64, s: f64, id: &str, path: &str, view: f64, ink: crate::draw::Rgb, open: bool) {
    let look = panel.look(x, y, s, s);
    let fill = if open {
        draw::CARD
    } else {
        match look {
            draw::Look::Down => draw::CONTROL_DOWN,
            draw::Look::Hover => draw::CONTROL_HOVER,
            draw::Look::Rest => INSET,
        }
    };
    fill_round(cr, x, y, s, s, R_CONTROL, fill);
    let glyph = 22.0;
    let color = if open { ink } else { MUTED };
    draw::icon(cr, path, view, x + (s - glyph) / 2.0, y + (s - glyph) / 2.0, glyph, color);
    panel.hot(id, x, y, s, s);
}

fn tab_fill(
    panel: &mut Panel,
    cr: &Context,
    x: f64,
    y: f64,
    s: f64,
    id: &str,
    path: &str,
    view: f64,
    ink: crate::draw::Rgb,
    fill: crate::draw::Rgb,
    rest: crate::draw::Rgb,
) {
    let look = panel.look(x, y, s, s);
    let fill = match look {
        draw::Look::Down => draw::mix(fill, TEXT, 0.12),
        draw::Look::Hover => draw::mix(fill, rest, 0.35),
        draw::Look::Rest => fill,
    };
    fill_round(cr, x, y, s, s, R_CONTROL, fill);
    let glyph = 22.0;
    draw::icon(cr, path, view, x + (s - glyph) / 2.0, y + (s - glyph) / 2.0, glyph, ink);
    panel.hot(id, x, y, s, s);
}
