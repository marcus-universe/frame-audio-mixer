//! Path and surface primitives: rounded rectangles, fills, small glyphs drawn
//! by hand, and the ARGB-to-RGBA copy the overlay needs.

use cairo::{Context, Format, ImageSurface};
use std::f64::consts::PI;

use super::palette::{Rgb, CARD, R_CARD};

pub fn surface_wh(width: i32, height: i32) -> Result<ImageSurface, String> {
    ImageSurface::create(Format::ARgb32, width, height).map_err(|e| e.to_string())
}

pub fn rounded(cr: &Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -PI / 2.0, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, PI / 2.0);
    cr.arc(x + r, y + h - r, r, PI / 2.0, PI);
    cr.arc(x + r, y + r, r, PI, 3.0 * PI / 2.0);
    cr.close_path();
}

pub fn fill(cr: &Context, c: Rgb, a: f64) {
    cr.set_source_rgba(c.r, c.g, c.b, a);
    let _ = cr.fill();
}

pub fn stroke(cr: &Context, c: Rgb, width: f64) {
    cr.set_source_rgb(c.r, c.g, c.b);
    cr.set_line_width(width);
    let _ = cr.stroke();
}

pub fn card(cr: &Context, x: f64, y: f64, w: f64, h: f64) {
    rounded(cr, x, y, w, h, R_CARD);
    fill(cr, CARD, 1.0);
}

pub fn fill_circle(cr: &Context, cx: f64, cy: f64, r: f64, c: Rgb) {
    cr.arc(cx, cy, r, 0.0, 2.0 * PI);
    fill(cr, c, 1.0);
}

/// Record dot, drawn rather than taken from an icon set.
pub fn record_dot(cr: &Context, cx: f64, cy: f64, r: f64, c: Rgb) {
    fill_circle(cr, cx, cy, r, c);
}

pub fn stop_square(cr: &Context, cx: f64, cy: f64, s: f64, c: Rgb) {
    rounded(cr, cx - s / 2.0, cy - s / 2.0, s, s, s * 0.15);
    fill(cr, c, 1.0);
}

pub fn play_triangle(cr: &Context, cx: f64, cy: f64, s: f64, c: Rgb) {
    cr.move_to(cx - s * 0.32, cy - s * 0.42);
    cr.line_to(cx + s * 0.44, cy);
    cr.line_to(cx - s * 0.32, cy + s * 0.42);
    cr.close_path();
    fill(cr, c, 1.0);
}

/// A chevron. `up` points up; otherwise it points down. `right` draws ">".
pub fn chevron(cr: &Context, x: f64, y: f64, size: f64, c: Rgb, right: bool, up: bool) {
    let _ = cr.save();
    cr.set_source_rgb(c.r, c.g, c.b);
    cr.set_line_width(2.0);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);
    if right {
        cr.move_to(x, y);
        cr.line_to(x + size * 0.45, y + size * 0.5);
        cr.line_to(x, y + size);
    } else if up {
        cr.move_to(x, y + size * 0.35);
        cr.line_to(x + size * 0.5, y);
        cr.line_to(x + size, y + size * 0.35);
    } else {
        cr.move_to(x, y);
        cr.line_to(x + size * 0.5, y + size * 0.35);
        cr.line_to(x + size, y);
    }
    let _ = cr.stroke();
    let _ = cr.restore();
}

pub fn paint_png_bytes(cr: &Context, bytes: &[u8], x: f64, y: f64, w: f64, h: f64) {
    let mut cursor = std::io::Cursor::new(bytes);
    let Ok(image) = ImageSurface::create_from_png(&mut cursor) else { return };
    let iw = image.width() as f64;
    let ih = image.height() as f64;
    if iw <= 0.0 || ih <= 0.0 {
        return;
    }
    let _ = cr.save();
    cr.translate(x, y);
    cr.scale(w / iw, h / ih);
    let _ = cr.set_source_surface(&image, 0.0, 0.0);
    let _ = cr.paint();
    let _ = cr.restore();
}

pub fn surface_to_rgba(surface: &mut ImageSurface) -> Vec<u8> {
    let width = surface.width() as usize;
    let height = surface.height() as usize;
    let stride = surface.stride() as usize;
    // data() needs the only cairo reference, so the drawing context must already be dropped.
    let data = surface.data().expect("surface data");
    let mut out = vec![0u8; width * height * 4];
    if width == 0 || height == 0 {
        return out;
    }
    let need = (height - 1) * stride + width * 4;
    assert!(data.len() >= need, "cairo surface shorter than its size");
    for y in 0..height {
        let row = y * stride;
        let dst = y * width * 4;
        for x in 0..width {
            let i = row + x * 4;
            let b = data[i] as u32;
            let g = data[i + 1] as u32;
            let r = data[i + 2] as u32;
            let a = data[i + 3];
            let o = dst + x * 4;
            if a == 0 {
                out[o] = 0;
                out[o + 1] = 0;
                out[o + 2] = 0;
                out[o + 3] = 0;
            } else {
                let a32 = a as u32;
                out[o] = ((r * 255 + a32 / 2) / a32).min(255) as u8;
                out[o + 1] = ((g * 255 + a32 / 2) / a32).min(255) as u8;
                out[o + 2] = ((b * 255 + a32 / 2) / a32).min(255) as u8;
                out[o + 3] = a;
            }
        }
    }
    out
}
