//! `Ui`: everything one frame needs to paint and to record what can be clicked.
//!
//! A page function takes the panel's state and a `Ui`. The `Ui` carries the
//! cairo context, the fonts and the translated strings, so no call repeats them,
//! and it collects the frame's hit areas, so a control and its click target are
//! declared together.

use super::hit::{FaderId, Hit};
use crate::draw::{self, ButtonKind, Font, Fonts, Look, Rect, Rgb};
use crate::i18n::Text;
use cairo::Context;

/// A clickable area recorded during the frame. Later areas lie on top.
pub(crate) struct HotArea {
    pub hit: Hit,
    pub rect: Rect,
}

/// A fader's grab area. `horizontal` says which way the value runs.
#[derive(Clone, Copy)]
pub(crate) struct FaderArea {
    pub id: FaderId,
    pub rect: Rect,
    pub horizontal: bool,
}

/// The visible part of a scrolling list. Hit areas of a half-hidden row stop at
/// its edge, so they cannot catch a click meant for whatever lies beside the list.
#[derive(Clone, Copy)]
pub(crate) struct HitBounds {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    /// A fader that sticks out is cut to the bounds. Otherwise it is left out.
    pub cut_faders: bool,
}

impl HitBounds {
    /// Bounded left and right only, for a row of columns.
    pub fn columns(left: f64, right: f64) -> Self {
        Self { left, top: f64::NEG_INFINITY, right, bottom: f64::INFINITY, cut_faders: false }
    }

    /// Bounded top and bottom only, for a list of rows.
    pub fn rows(top: f64, bottom: f64) -> Self {
        Self { left: f64::NEG_INFINITY, top, right: f64::INFINITY, bottom, cut_faders: false }
    }

    pub fn rect(view: Rect) -> Self {
        Self { left: view.x, top: view.y, right: view.right(), bottom: view.bottom(), cut_faders: false }
    }

    pub fn cutting_faders(self) -> Self {
        Self { cut_faders: true, ..self }
    }

    /// The part of `rect` inside the bounds, if any.
    fn cut(&self, rect: Rect) -> Option<Rect> {
        let x0 = rect.x.max(self.left);
        let x1 = (rect.x + rect.w).min(self.right);
        let y0 = rect.y.max(self.top);
        let y1 = (rect.y + rect.h).min(self.bottom);
        (x1 > x0 && y1 > y0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
    }

    fn holds(&self, rect: Rect) -> bool {
        rect.x >= self.left && rect.x + rect.w <= self.right && rect.y >= self.top && rect.y + rect.h <= self.bottom
    }
}

pub(crate) struct Ui<'a> {
    pub cr: &'a Context,
    pub fonts: &'a Fonts,
    /// The strings of the current language.
    pub t: &'static Text,
    /// Where the pointer is, and whether its button is held.
    pub pointer: Option<(f64, f64)>,
    pub pressed: bool,
    /// While set, hit areas are limited to these bounds. See [`HitBounds`].
    pub bounds: Option<HitBounds>,
    pub hots: Vec<HotArea>,
    pub faders: Vec<FaderArea>,
}

/// Shapes.
impl Ui<'_> {
    /// A rounded rectangle in one opaque color.
    pub fn fill(&self, rect: Rect, radius: f64, color: Rgb) {
        draw::fill_round(self.cr, rect, radius, color);
    }

    pub fn stroke(&self, rect: Rect, radius: f64, color: Rgb, width: f64) {
        draw::stroke_round(self.cr, rect, radius, color, width);
    }

    pub fn card(&self, rect: Rect) {
        draw::card(self.cr, rect);
    }

    /// A dialog surface: one tone above the cards, no outline.
    pub fn sheet(&self, rect: Rect) {
        self.fill(rect, 24.0, draw::INSET);
    }

    pub fn circle(&self, cx: f64, cy: f64, radius: f64, color: Rgb) {
        draw::fill_circle(self.cr, cx, cy, radius, color);
    }

    /// One glyph from SVG `path` data in a `view`-sized box, `size` pixels wide at `x, y`.
    pub fn icon(&self, path: &str, view: f64, x: f64, y: f64, size: f64, color: Rgb) {
        draw::icon(self.cr, path, view, x, y, size, color);
    }

    /// Limits painting to `rect` until [`Ui::unclip`].
    pub fn clip(&self, rect: Rect) {
        let _ = self.cr.save();
        self.cr.rectangle(rect.x, rect.y, rect.w, rect.h);
        self.cr.clip();
    }

    /// Limits painting to a rounded `rect` until [`Ui::unclip`].
    pub fn clip_round(&self, rect: Rect, radius: f64) {
        let _ = self.cr.save();
        draw::rounded(self.cr, rect, radius);
        self.cr.clip();
    }

    pub fn unclip(&self) {
        let _ = self.cr.restore();
    }
}

/// Text. `baseline` is the y of the text baseline; `cy` is the vertical center of the line.
impl Ui<'_> {
    pub fn text_width(&self, font: Font, text: &str) -> f64 {
        draw::text_width(self.cr, self.fonts, font, text)
    }

    pub fn label(&self, font: Font, color: Rgb, x: f64, baseline: f64, text: &str) {
        draw::label(self.cr, self.fonts, font, color, x, baseline, text);
    }

    /// Text centered vertically on `cy`, for a single line inside a control.
    pub fn label_mid(&self, font: Font, color: Rgb, x: f64, cy: f64, text: &str) {
        draw::label_mid(self.cr, self.fonts, font, color, x, cy, text);
    }

    /// Text centered in the span `x .. x + w` and vertically on `cy`.
    pub fn label_centered(&self, font: Font, color: Rgb, x: f64, w: f64, cy: f64, text: &str) {
        let tw = self.text_width(font, text);
        self.label_mid(font, color, x + (w - tw) / 2.0, cy, text);
    }

    /// Text that must stay inside `max_w`, ending with an ellipsis when it doesn't fit.
    pub fn label_fit(&self, font: Font, color: Rgb, x: f64, baseline: f64, max_w: f64, text: &str) {
        draw::label_fit(self.cr, self.fonts, font, color, x, baseline, max_w, text);
    }

    /// The end of `text` inside `max_w`. A long name keeps the characters being typed.
    pub fn label_tail(&self, font: Font, color: Rgb, x: f64, baseline: f64, max_w: f64, text: &str) {
        draw::label_tail(self.cr, self.fonts, font, color, x, baseline, max_w, text);
    }

    pub fn wrap(&self, font: Font, max_w: f64, max_lines: usize, text: &str) -> Vec<String> {
        draw::wrap(self.cr, self.fonts, font, max_w, max_lines, text)
    }
}

/// Pointer state and hit areas.
impl Ui<'_> {
    /// How a control at `rect` should look under the pointer.
    pub fn look(&self, rect: Rect) -> Look {
        let inside = self.pointer.is_some_and(|(px, py)| rect.contains(px, py));
        if inside && self.pressed {
            Look::Down
        } else if inside {
            Look::Hover
        } else {
            Look::Rest
        }
    }

    /// The fill of a flat control: `rest`, or the shared hover and pressed tones.
    pub fn control_fill(&self, rect: Rect, rest: Rgb) -> Rgb {
        match self.look(rect) {
            Look::Down => draw::CONTROL_DOWN,
            Look::Hover => draw::CONTROL_HOVER,
            Look::Rest => rest,
        }
    }

    /// Makes `rect` clickable. Inside [`Ui::bounds`] only its visible part is.
    pub fn hot(&mut self, hit: Hit, rect: Rect) {
        let rect = match &self.bounds {
            Some(bounds) => match bounds.cut(rect) {
                Some(visible) => visible,
                None => return,
            },
            None => rect,
        };
        self.hots.push(HotArea { hit, rect });
    }

    /// Makes `rect` a fader's grab area. `horizontal` says which way the value runs.
    pub fn fader(&mut self, id: FaderId, rect: Rect, horizontal: bool) {
        let rect = match &self.bounds {
            Some(bounds) if bounds.cut_faders => match bounds.cut(rect) {
                Some(visible) => visible,
                None => return,
            },
            Some(bounds) if !bounds.holds(rect) => return,
            _ => rect,
        };
        self.faders.push(FaderArea { id, rect, horizontal });
    }

    /// The topmost hit area under a point.
    pub fn hit_at(hots: &[HotArea], x: f64, y: f64) -> Option<&Hit> {
        hots.iter().rev().find(|hot| hot.rect.contains(x, y)).map(|hot| &hot.hit)
    }
}

/// Controls made of the pieces above.
impl Ui<'_> {
    /// A pill button that reacts to the pointer. It is not clickable by itself.
    pub fn button(&self, rect: Rect, caption: &str, kind: ButtonKind) {
        draw::button(self.cr, self.fonts, rect, caption, kind, self.look(rect));
    }

    /// A pill button and its click target.
    pub fn button_hot(&mut self, rect: Rect, caption: &str, kind: ButtonKind, hit: Hit) {
        self.button(rect, caption, kind);
        self.hot(hit, rect);
    }

    /// On/Off as a pill. The chosen half is filled with the accent.
    /// `hits` are what the first and the second half do.
    pub fn pill_toggle(&mut self, rect: Rect, on: bool, labels: (&str, &str), hits: (Hit, Hit)) {
        let Rect { x, y, w, h } = rect;
        self.fill(rect, h / 2.0, draw::CONTROL);
        let half = w / 2.0;
        let pad = 4.0;
        let sel_x = if on { x + pad } else { x + half };
        self.fill(Rect::new(sel_x, y + pad, half - pad, h - 2.0 * pad), (h - 2.0 * pad) / 2.0, draw::accent());
        for (index, (text, hit)) in [(labels.0, hits.0), (labels.1, hits.1)].into_iter().enumerate() {
            let picked = (index == 0) == on;
            let half_x = x + half * index as f64;
            let color = if picked { draw::on_accent() } else { draw::SOFT };
            let tw = self.text_width(draw::BUTTON, text);
            self.label_mid(draw::BUTTON, color, half_x + half / 2.0 - tw / 2.0, y + h / 2.0, text);
            self.hot(hit, Rect::new(half_x, y, half, h));
        }
    }

    /// Close button in a dialog's top-right corner, with a hover layer.
    pub fn close_button(&mut self, x: f64, y: f64) {
        let rect = Rect::square(x, y, 40.0);
        let look = self.look(rect);
        if look != Look::Rest {
            self.fill(rect, rect.w / 2.0, if look == Look::Down { draw::CONTROL_DOWN } else { draw::CONTROL_HOVER });
        }
        self.icon(draw::CLOSE_PATH, 1024.0, x + 11.0, y + 11.0, 18.0, draw::SOFT);
        self.hot(Hit::Dismiss, rect);
    }

    /// Dims the panel behind a dialog. A click on it dismisses the dialog.
    pub fn backdrop(&mut self, panel: Rect) {
        self.fill(panel, 28.0, draw::BG);
        self.cr.set_source_rgba(0.0, 0.0, 0.0, 0.45);
        let _ = self.cr.paint();
        self.hot(Hit::Dismiss, panel);
    }

    /// A dialog: the dimmed backdrop, then the sheet, which swallows clicks on itself.
    pub fn dialog(&mut self, panel: Rect, sheet: Rect) {
        self.backdrop(panel);
        self.sheet(sheet);
        self.hot(Hit::Noop, sheet);
    }
}
