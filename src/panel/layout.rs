//! Where the cards sit for the current window: the mixer, the drawer beside it,
//! the rail, the footer, and the dialogs on top.

use super::Panel;
use crate::draw::{self, Rect};

/// Window margin, the gap between cards, and the header and footer bands.
pub(crate) const MARGIN: f64 = 24.0;
pub(crate) const GUTTER: f64 = 16.0;
pub(crate) const HEADER_H: f64 = 48.0;
pub(crate) const CONTENT_TOP: f64 = MARGIN + HEADER_H + GUTTER;
const FOOTER_H: f64 = 80.0;
const RAIL_W: f64 = 48.0;
/// Width of the drawer in the regular window.
const DRAWER_W: f64 = 384.0;

/// Where the mixer, the drawer, the rail and the footer sit for the current window.
pub(crate) struct Stage {
    /// Window width.
    pub w: f64,
    pub portrait: bool,
    pub footer: Rect,
    pub mix: Rect,
    /// The drawer beside the mixer: Outputs with Mix Check, or an effect bus.
    pub drawer: Rect,
    pub rail: Rect,
}

/// The mixer and the drawer share `span`. Each one keeps at least 20% of the window.
fn split_limits(w: f64) -> (f64, f64) {
    let span = (w - 2.0 * MARGIN - RAIL_W - 2.0 * GUTTER).max(0.0);
    let min_w = (0.20 * w).min(span * 0.5);
    (min_w, span)
}

impl Panel {
    pub fn pixel_size(&self) -> (i32, i32) {
        draw::panel_pixels(self.config.wide, false)
    }

    pub(crate) fn size_f(&self) -> (f64, f64) {
        let (w, h) = self.pixel_size();
        (w as f64, h as f64)
    }

    /// The whole window.
    pub(crate) fn window(&self) -> Rect {
        let (w, h) = self.size_f();
        Rect::new(0.0, 0.0, w, h)
    }

    /// A centered dialog of at most `max_w` by `max_h`, keeping `margin` clear of the window edges.
    pub(crate) fn dialog_rect(&self, max_w: f64, max_h: f64, margin: f64) -> Rect {
        let (pw, ph) = self.size_f();
        let w = max_w.min(pw - margin * 2.0).max(280.0);
        let h = max_h.min(ph - margin * 2.0).max(200.0);
        Rect::new((pw - w) / 2.0, (ph - h) / 2.0, w, h)
    }

    /// A dialog that fills the window up to `margin`.
    pub(crate) fn sheet_rect(&self, margin: f64) -> Rect {
        let (pw, ph) = self.size_f();
        Rect::new(margin, margin, (pw - margin * 2.0).max(200.0), (ph - margin * 2.0).max(200.0))
    }

    pub(crate) fn stage(&self) -> Stage {
        let (w, h) = self.size_f();
        let footer = Rect::new(MARGIN, h - MARGIN - FOOTER_H, w - 2.0 * MARGIN, FOOTER_H);
        let bottom = footer.y - GUTTER;
        let content_h = bottom - CONTENT_TOP;
        let portrait = h > w && !self.config.list;
        let rail = Rect::new(w - MARGIN - RAIL_W, CONTENT_TOP, RAIL_W, content_h);
        let ease = 1.0 - (1.0 - self.drawer_progress).powi(2);
        let drawer_w = self.drawer_span() * ease;
        let drawer = Rect::new(rail.x - GUTTER - drawer_w, CONTENT_TOP, drawer_w, content_h);
        let mix_right = if drawer_w > 8.0 { drawer.x - GUTTER } else { rail.x - GUTTER };
        let mix = Rect::new(MARGIN, CONTENT_TOP, (mix_right - MARGIN).max(0.0), content_h);
        Stage { w, portrait, footer, mix, drawer, rail }
    }

    /// Open width of the drawer. A saved split follows the pointer; otherwise the
    /// 21:9 view gives Outputs a third of the screen and the regular view keeps 384.
    /// Either card stops at 20% of the window.
    fn drawer_span(&self) -> f64 {
        let (w, _) = self.size_f();
        let (min_w, span) = split_limits(w);
        let raw = if (0.0..1.0).contains(&self.config.split) && self.config.split > 0.0 {
            self.config.split * span
        } else if w <= draw::WIDTH as f64 + 1.0 {
            DRAWER_W
        } else {
            w / 3.0
        };
        raw.clamp(min_w, (span - min_w).max(min_w))
    }

    /// The pointer sits in the gutter. The drawer takes what remains up to the rail.
    pub(super) fn set_split(&mut self, x: f64) {
        let (w, _) = self.size_f();
        let (min_w, span) = split_limits(w);
        if span <= 1.0 {
            return;
        }
        let rail_x = w - MARGIN - RAIL_W;
        let right = rail_x - GUTTER - (x + GUTTER / 2.0);
        let right = right.clamp(min_w, (span - min_w).max(min_w));
        self.config.split = right / span;
    }
}
