//! `Rect`: the one geometry type the panel is laid out with.
//!
//! Layout code names a rectangle once and derives the next one from it
//! (`card.inset(20.0).take_top(40.0)`), so no call site passes four loose numbers.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub const fn square(x: f64, y: f64, side: f64) -> Self {
        Self { x, y, w: side, h: side }
    }

    pub fn right(self) -> f64 {
        self.x + self.w
    }

    pub fn bottom(self) -> f64 {
        self.y + self.h
    }

    pub fn mid_x(self) -> f64 {
        self.x + self.w / 2.0
    }

    pub fn mid_y(self) -> f64 {
        self.y + self.h / 2.0
    }

    /// The left and top edges are inside, the right and bottom edges are not.
    pub fn contains(self, px: f64, py: f64) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }

    /// Shrinks every side by `d`. A negative `d` grows the rectangle.
    pub fn inset(self, d: f64) -> Self {
        self.inset_xy(d, d)
    }

    pub fn inset_xy(self, dx: f64, dy: f64) -> Self {
        Self { x: self.x + dx, y: self.y + dy, w: self.w - 2.0 * dx, h: self.h - 2.0 * dy }
    }

    /// The top `h` pixels.
    pub fn take_top(self, h: f64) -> Self {
        Self { h, ..self }
    }

    /// The bottom `h` pixels.
    pub fn take_bottom(self, h: f64) -> Self {
        Self { y: self.y + self.h - h, h, ..self }
    }

    /// The left `w` pixels.
    pub fn take_left(self, w: f64) -> Self {
        Self { w, ..self }
    }
}
