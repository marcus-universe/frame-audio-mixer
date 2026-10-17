//! Cairo drawing. Colors match Frame Mic Tuner's panel so the two apps sit together.

use cairo::{Context, FillRule, FontFace, Format, ImageSurface};
use freetype::Library as FtLibrary;
use std::f64::consts::PI;

pub const WIDTH: i32 = 1200;
pub const HEIGHT: i32 = 788;

#[derive(Clone, Copy)]
pub struct Rgb {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

pub const BG: Rgb = Rgb { r: 0x0e as f64 / 255.0, g: 0x10 as f64 / 255.0, b: 0x15 as f64 / 255.0 };
pub const CARD: Rgb = Rgb { r: 0x1a as f64 / 255.0, g: 0x1d as f64 / 255.0, b: 0x24 as f64 / 255.0 };
pub const INSET: Rgb = Rgb { r: 0x14 as f64 / 255.0, g: 0x17 as f64 / 255.0, b: 0x1c as f64 / 255.0 };
pub const TEXT: Rgb = Rgb { r: 0xf2 as f64 / 255.0, g: 0xf3 as f64 / 255.0, b: 0xf5 as f64 / 255.0 };
pub const MUTED: Rgb = Rgb { r: 0xa3 as f64 / 255.0, g: 0xa9 as f64 / 255.0, b: 0xb4 as f64 / 255.0 };
pub const SOFT: Rgb = Rgb { r: 0xdf as f64 / 255.0, g: 0xe2 as f64 / 255.0, b: 0xe7 as f64 / 255.0 };
pub const BORDER: Rgb = Rgb { r: 0x6e as f64 / 255.0, g: 0x76 as f64 / 255.0, b: 0x81 as f64 / 255.0 };
pub const ACCENT: Rgb = Rgb { r: 0xe0 as f64 / 255.0, g: 0x7e as f64 / 255.0, b: 0xf5 as f64 / 255.0 };
pub const DANGER: Rgb = Rgb { r: 0xe5 as f64 / 255.0, g: 0x53 as f64 / 255.0, b: 0x4b as f64 / 255.0 };

pub struct Fonts {
    _ft: FtLibrary,
    _regular_face: freetype::Face,
    _bold_face: freetype::Face,
    pub regular: FontFace,
    pub bold: FontFace,
}

impl Fonts {
    pub fn load() -> Result<Self, String> {
        let ft = FtLibrary::init().map_err(|e| e.to_string())?;
        let regular_path = find_font(&[
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        ])?;
        let bold_path = find_font(&[
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Bold.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc",
            "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
        ])
        .unwrap_or_else(|_| regular_path.clone());
        let regular_face = ft.new_face(&regular_path, 0).map_err(|e| e.to_string())?;
        let bold_face = ft.new_face(&bold_path, 0).map_err(|e| e.to_string())?;
        let regular = FontFace::create_from_ft(&regular_face).map_err(|e| e.to_string())?;
        let bold = FontFace::create_from_ft(&bold_face).map_err(|e| e.to_string())?;
        Ok(Self {
            _ft: ft,
            _regular_face: regular_face,
            _bold_face: bold_face,
            regular,
            bold,
        })
    }
}

fn find_font(paths: &[&str]) -> Result<String, String> {
    paths
        .iter()
        .find(|path| std::path::Path::new(path).exists())
        .map(|path| (*path).to_string())
        .ok_or_else(|| format!("no font in {paths:?}"))
}

pub fn surface() -> Result<ImageSurface, String> {
    ImageSurface::create(Format::ARgb32, WIDTH, HEIGHT).map_err(|e| e.to_string())
}

pub fn rounded(cr: &Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0);
    let _ = cr.new_sub_path();
    let _ = cr.arc(x + w - r, y + r, r, -PI / 2.0, 0.0);
    let _ = cr.arc(x + w - r, y + h - r, r, 0.0, PI / 2.0);
    let _ = cr.arc(x + r, y + h - r, r, PI / 2.0, PI);
    let _ = cr.arc(x + r, y + r, r, PI, 3.0 * PI / 2.0);
    let _ = cr.close_path();
}

pub fn fill(cr: &Context, c: Rgb, a: f64) {
    cr.set_source_rgba(c.r, c.g, c.b, a);
    let _ = cr.fill();
}

pub fn fill_keep(cr: &Context, c: Rgb, a: f64) {
    cr.set_source_rgba(c.r, c.g, c.b, a);
    let _ = cr.fill_preserve();
}

pub fn card(cr: &Context, x: f64, y: f64, w: f64, h: f64) {
    rounded(cr, x, y, w, h, 18.0);
    fill(cr, CARD, 1.0);
}

pub fn set_font(cr: &Context, fonts: &Fonts, bold: bool, size: f64) {
    let _ = cr.set_font_face(if bold { &fonts.bold } else { &fonts.regular });
    cr.set_font_size(size);
}

pub fn text_width(cr: &Context, fonts: &Fonts, bold: bool, size: f64, text: &str) -> f64 {
    set_font(cr, fonts, bold, size);
    cr.text_extents(text).map(|e| e.x_advance()).unwrap_or(0.0)
}

pub fn label(cr: &Context, fonts: &Fonts, bold: bool, size: f64, c: Rgb, x: f64, baseline: f64, text: &str) {
    set_font(cr, fonts, bold, size);
    cr.set_source_rgb(c.r, c.g, c.b);
    let _ = cr.move_to(x, baseline);
    let _ = cr.show_text(text);
}

/// Draw text that must stay inside `max_w`, ending with an ellipsis when it doesn't fit.
pub fn label_fit(
    cr: &Context,
    fonts: &Fonts,
    bold: bool,
    size: f64,
    c: Rgb,
    x: f64,
    baseline: f64,
    max_w: f64,
    text: &str,
) {
    if text_width(cr, fonts, bold, size, text) <= max_w {
        label(cr, fonts, bold, size, c, x, baseline, text);
        return;
    }
    let mut kept: String = text.chars().collect();
    while !kept.is_empty() && text_width(cr, fonts, bold, size, &format!("{kept}…")) > max_w {
        kept.pop();
    }
    label(cr, fonts, bold, size, c, x, baseline, &format!("{kept}…"));
}

pub fn button(cr: &Context, fonts: &Fonts, x: f64, y: f64, w: f64, h: f64, caption: &str, accent: bool) {
    rounded(cr, x, y, w, h, 12.0);
    if accent {
        fill(cr, ACCENT, 1.0);
        let tw = text_width(cr, fonts, true, 16.0, caption);
        label(cr, fonts, true, 16.0, BG, x + (w - tw) / 2.0, y + h / 2.0 + 6.0, caption);
    } else {
        fill(cr, INSET, 1.0);
        cr.set_source_rgb(BORDER.r, BORDER.g, BORDER.b);
        cr.set_line_width(1.0);
        rounded(cr, x, y, w, h, 12.0);
        let _ = cr.stroke();
        let tw = text_width(cr, fonts, true, 16.0, caption);
        label(cr, fonts, true, 16.0, SOFT, x + (w - tw) / 2.0, y + h / 2.0 + 6.0, caption);
    }
}

pub fn icon(cr: &Context, path: &str, view: f64, x: f64, y: f64, size: f64, c: Rgb) {
    let _ = cr.save();
    cr.translate(x, y);
    cr.scale(size / view, size / view);
    cr.set_fill_rule(FillRule::EvenOdd);
    append_svg(cr, path);
    fill(cr, c, 1.0);
    let _ = cr.restore();
}

pub fn surface_to_rgba(surface: &mut ImageSurface) -> Vec<u8> {
    let width = surface.width() as usize;
    let height = surface.height() as usize;
    let stride = surface.stride() as usize;
    // data() needs the only cairo reference, so the drawing context must already be dropped.
    let data = surface.data().expect("surface data");
    let mut out = vec![0u8; width * height * 4];
    for y in 0..height {
        for x in 0..width {
            let i = y * stride + x * 4;
            let b = data[i] as u32;
            let g = data[i + 1] as u32;
            let r = data[i + 2] as u32;
            let a = data[i + 3] as u32;
            let un = |c: u32| -> u8 {
                if a == 0 {
                    0
                } else {
                    ((c * 255 + a / 2) / a).min(255) as u8
                }
            };
            let o = (y * width + x) * 4;
            out[o] = un(r);
            out[o + 1] = un(g);
            out[o + 2] = un(b);
            out[o + 3] = a as u8;
        }
    }
    out
}

pub const MIC_PATH: &str = "M8.4 16.8a4.805 4.805 0 0 0 4.8-4.8V4.8a4.8 4.8 0 1 0-9.6 0V12a4.805 4.805 0 0 0 4.8 4.8 M16.8 12V9.6a1.2 1.2 0 1 0-2.4 0V12a6 6 0 1 1-12 0V9.6a1.2 1.2 0 1 0-2.4 0V12a8.406 8.406 0 0 0 7.154 8.298l.046.006V21.6H3.6a1.2 1.2 0 1 0 0 2.4h9.6a1.2 1.2 0 1 0 0-2.4H9.6v-1.296c4.09-.609 7.193-4.093 7.2-8.303z";
pub const MIC_OFF_PATH: &str = "M400 240v-31.55c0-8.61-6.62-16-15.23-16.43A16 16 0 0 0 368 208v32a111.7 111.7 0 0 1-2.68 24.38a2 2 0 0 0 .53 1.84l22.59 22.59a2 2 0 0 0 3.29-.72A143.3 143.3 0 0 0 400 240M256 352a112.36 112.36 0 0 1-112-112v-31.55c0-8.61-6.62-16-15.23-16.43A16 16 0 0 0 112 208v32c0 74 56.1 135.12 128 143.11V432h-47.55c-8.61 0-16 6.62-16.43 15.23A16 16 0 0 0 192 464h127.55c8.61 0 16-6.62 16.43-15.23A16 16 0 0 0 320 432h-48v-48.89a143.1 143.1 0 0 0 52-16.22a4 4 0 0 0 .91-6.35l-18.4-18.39a3 3 0 0 0-3.41-.58A111 111 0 0 1 256 352m1.14-304a79.66 79.66 0 0 0-68.47 36.57a4 4 0 0 0 .54 5L332.59 233a2 2 0 0 0 3.41-1.42V128.91C336 85 301 48.6 257.14 48M179.41 215a2 2 0 0 0-3.41 1.42V239a80.9 80.9 0 0 0 23.45 56.9a78.55 78.55 0 0 0 77.8 21.19a2 2 0 0 0 .86-3.35Z";
pub const SPEAKER_PATH: &str = "M26 49.451c1.581 0 2.721-1.163 2.721-2.721V9.637c0-1.558-1.14-2.86-2.767-2.86c-1.14 0-1.907.511-3.14 1.674l-10.256 9.698a.9.9 0 0 1-.604.209H5.046C1.768 18.358 0 20.148 0 23.637v9.023c0 3.489 1.767 5.28 5.047 5.28h6.907c.232 0 .441.07.604.209l10.256 9.79c1.116 1.047 2.046 1.512 3.186 1.512m22.489-4.744c.906.604 2.023.372 2.674-.558c3.07-4.28 4.837-9.977 4.837-15.93c0-5.977-1.744-11.675-4.837-15.954c-.674-.907-1.768-1.14-2.674-.535c-.884.605-1.024 1.744-.326 2.744c2.535 3.721 4.093 8.605 4.093 13.744c0 5.14-1.512 10.07-4.117 13.745c-.65 1-.534 2.14.35 2.744m-9.28-6.28c.791.559 1.93.373 2.605-.534c1.814-2.442 2.907-6.024 2.907-9.675c0-3.65-1.116-7.209-2.907-9.697c-.674-.907-1.79-1.093-2.605-.535c-1.023.674-1.14 1.86-.395 2.883c1.349 1.814 2.163 4.582 2.163 7.35s-.86 5.534-2.186 7.371c-.698 1-.582 2.14.418 2.838";
pub const SPEAKER_OFF_PATH: &str = "M39.18 33.402V8.207c0-1.57-1.149-2.883-2.79-2.883c-1.148 0-1.921.516-3.163 1.688l-9.61 9.093c-.14.141-.281.235-.445.282l-.938.07Zm7.593 16.57c.727.704 1.875.704 2.555 0c.703-.726.727-1.85 0-2.554L9.203 7.293a1.84 1.84 0 0 0-2.578 0c-.68.68-.68 1.875 0 2.555Zm-10.336-1.64c1.383 0 2.344-.82 2.649-2.203l-27.68-27.633c-.75.89-1.172 2.18-1.172 3.82v9.094c0 3.516 1.781 5.32 5.086 5.32h6.961c.235 0 .446.07.586.211l10.36 9.868c1.125 1.054 2.062 1.523 3.21 1.523";
pub const ADD_PATH: &str = "M3 6.75A3.75 3.75 0 0 1 6.75 3h14.5A3.75 3.75 0 0 1 25 6.75v14.5A3.75 3.75 0 0 1 21.25 25H6.75A3.75 3.75 0 0 1 3 21.25zm11 .75a.75.75 0 0 0-.75.75v5h-5a.75.75 0 0 0 0 1.5h5v5a.75.75 0 0 0 1.5 0v-5h5a.75.75 0 0 0 0-1.5h-5v-5A.75.75 0 0 0 14 7.5";
pub const LANG_PATH: &str = "M11 5a.75.75 0 0 1 .688.452l3.25 7.5a.75.75 0 1 1-1.376.596L12.89 12H9.109l-.67 1.548a.75.75 0 1 1-1.377-.596l3.25-7.5A.75.75 0 0 1 11 5m-1.24 5.5h2.48L11 7.636zM5 1a.75.75 0 0 1 .75.75v1.261a25 25 0 0 1 2.598.211a.75.75 0 1 1-.2 1.487q-.33-.045-.662-.08A13 13 0 0 1 5.92 8.058q.356.456.752.873a.75.75 0 0 1-1.086 1.035A13 13 0 0 1 5 9.307a13 13 0 0 1-2.841 2.546a.75.75 0 0 1-.827-1.251A11.6 11.6 0 0 0 4.08 8.057a13 13 0 0 1-.554-.938a.75.75 0 1 1 1.323-.707l.15.271c.388-.68.708-1.405.952-2.164a24 24 0 0 0-4.1.19a.75.75 0 0 1-.2-1.487q1.28-.171 2.598-.211V1.75A.75.75 0 0 1 5 1";
pub const HEADPHONES_PATH: &str = "M8 1a7 7 0 0 1 7 7v5a2 2 0 0 1-2 2h-2a1 1 0 0 1-1-1v-4a1 1 0 0 1 1-1h2V8A5 5 0 0 0 3 8v1h2a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1H3a2 2 0 0 1-2-2V8a7 7 0 0 1 7-7";
pub const RECORD_PATH: &str = "M2 12C2 6.477 6.477 2 12 2s10 4.477 10 10s-4.477 10-10 10S2 17.523 2 12m10 6a6 6 0 1 0 0-12a6 6 0 0 0 0 12";
pub const STOP_PATH: &str = "M4.75 3A1.75 1.75 0 0 0 3 4.75v14.5c0 .966.784 1.75 1.75 1.75h14.5A1.75 1.75 0 0 0 21 19.25V4.75A1.75 1.75 0 0 0 19.25 3z";
pub const PLAY_PATH: &str = "M5 5.274c0-1.707 1.826-2.792 3.325-1.977l12.362 6.727c1.566.852 1.566 3.1 0 3.952L8.325 20.702C6.826 21.518 5 20.432 5 18.726z";

fn read_number(bytes: &[u8], i: &mut usize) -> Option<f64> {
    while *i < bytes.len() && (bytes[*i] == b' ' || bytes[*i] == b',') {
        *i += 1;
    }
    if *i >= bytes.len() {
        return None;
    }
    let start = *i;
    if bytes[*i] == b'+' || bytes[*i] == b'-' {
        *i += 1;
    }
    let mut saw = false;
    while *i < bytes.len() && bytes[*i].is_ascii_digit() {
        saw = true;
        *i += 1;
    }
    if *i < bytes.len() && bytes[*i] == b'.' {
        *i += 1;
        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            saw = true;
            *i += 1;
        }
    }
    if *i < bytes.len() && (bytes[*i] == b'e' || bytes[*i] == b'E') {
        let e = *i;
        *i += 1;
        if *i < bytes.len() && (bytes[*i] == b'+' || bytes[*i] == b'-') {
            *i += 1;
        }
        let exp = *i;
        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            *i += 1;
        }
        if *i == exp {
            *i = e;
        }
    }
    if !saw {
        return None;
    }
    std::str::from_utf8(&bytes[start..*i]).ok()?.parse().ok()
}

fn svg_arc(cr: &Context, cx: &mut f64, cy: &mut f64, mut rx: f64, mut ry: f64, phi_deg: f64, large: bool, sweep: bool, x2: f64, y2: f64) {
    if rx == 0.0 || ry == 0.0 || (*cx == x2 && *cy == y2) {
        let _ = cr.line_to(x2, y2);
        *cx = x2;
        *cy = y2;
        return;
    }
    let phi = phi_deg * PI / 180.0;
    let (sin_phi, cos_phi) = (phi.sin(), phi.cos());
    let dx = (*cx - x2) / 2.0;
    let dy = (*cy - y2) / 2.0;
    let x1p = cos_phi * dx + sin_phi * dy;
    let y1p = -sin_phi * dx + cos_phi * dy;
    rx = rx.abs();
    ry = ry.abs();
    let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
    if lambda > 1.0 {
        let scale = lambda.sqrt();
        rx *= scale;
        ry *= scale;
    }
    let rx2 = rx * rx;
    let ry2 = ry * ry;
    let num = rx2 * ry2 - rx2 * y1p * y1p - ry2 * x1p * x1p;
    let den = rx2 * y1p * y1p + ry2 * x1p * x1p;
    let mut coef = if den == 0.0 { 0.0 } else { (num / den).max(0.0).sqrt() };
    if large == sweep {
        coef = -coef;
    }
    let cxp = coef * rx * y1p / ry;
    let cyp = coef * -ry * x1p / rx;
    let center_x = cos_phi * cxp - sin_phi * cyp + (*cx + x2) / 2.0;
    let center_y = sin_phi * cxp + cos_phi * cyp + (*cy + y2) / 2.0;
    let angle_between = |ux: f64, uy: f64, vx: f64, vy: f64| {
        let dot = ux * vx + uy * vy;
        let len = ((ux * ux + uy * uy) * (vx * vx + vy * vy)).sqrt();
        let mut ang = if len == 0.0 { 0.0 } else { (dot / len).clamp(-1.0, 1.0).acos() };
        if ux * vy - uy * vx < 0.0 {
            ang = -ang;
        }
        ang
    };
    let theta1 = angle_between(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry);
    let mut dtheta = angle_between((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry);
    if !sweep && dtheta > 0.0 {
        dtheta -= 2.0 * PI;
    }
    if sweep && dtheta < 0.0 {
        dtheta += 2.0 * PI;
    }
    let segments = ((dtheta.abs() / (PI / 2.0)).ceil() as i32).max(1);
    let delta = dtheta / segments as f64;
    let t = (delta / 4.0).tan() * 4.0 / 3.0;
    let mut theta = theta1;
    let map_point = |ux: f64, uy: f64| {
        (
            center_x + rx * ux * cos_phi - ry * uy * sin_phi,
            center_y + rx * ux * sin_phi + ry * uy * cos_phi,
        )
    };
    for _ in 0..segments {
        let (cos1, sin1) = (theta.cos(), theta.sin());
        theta += delta;
        let (cos2, sin2) = (theta.cos(), theta.sin());
        let (p1x, p1y) = map_point(cos1 - t * sin1, sin1 + t * cos1);
        let (p2x, p2y) = map_point(cos2 + t * sin2, sin2 - t * cos2);
        let (ex, ey) = map_point(cos2, sin2);
        let _ = cr.curve_to(p1x, p1y, p2x, p2y, ex, ey);
    }
    *cx = x2;
    *cy = y2;
}

fn append_svg(cr: &Context, d: &str) {
    let bytes = d.as_bytes();
    let mut i = 0;
    let mut cx = 0.0;
    let mut cy = 0.0;
    let mut sx = 0.0;
    let mut sy = 0.0;
    let mut command = 0u8;
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b',') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        if bytes[i].is_ascii_alphabetic() {
            command = bytes[i];
            i += 1;
        } else if command == b'M' {
            command = b'L';
        } else if command == b'm' {
            command = b'l';
        } else if command == 0 {
            break;
        }
        match command {
            b'M' | b'm' => {
                let (Some(mut x), Some(mut y)) = (read_number(bytes, &mut i), read_number(bytes, &mut i)) else { return };
                if command == b'm' {
                    x += cx;
                    y += cy;
                }
                let _ = cr.move_to(x, y);
                cx = x;
                cy = y;
                sx = x;
                sy = y;
            }
            b'L' | b'l' => {
                let (Some(mut x), Some(mut y)) = (read_number(bytes, &mut i), read_number(bytes, &mut i)) else { return };
                if command == b'l' {
                    x += cx;
                    y += cy;
                }
                let _ = cr.line_to(x, y);
                cx = x;
                cy = y;
            }
            b'H' | b'h' => {
                let Some(mut x) = read_number(bytes, &mut i) else { return };
                if command == b'h' {
                    x += cx;
                }
                let _ = cr.line_to(x, cy);
                cx = x;
            }
            b'V' | b'v' => {
                let Some(mut y) = read_number(bytes, &mut i) else { return };
                if command == b'v' {
                    y += cy;
                }
                let _ = cr.line_to(cx, y);
                cy = y;
            }
            b'C' | b'c' => {
                let nums = [(); 6].map(|_| read_number(bytes, &mut i));
                if nums.iter().any(|n| n.is_none()) {
                    return;
                }
                let mut n = nums.map(|v| v.unwrap());
                if command == b'c' {
                    n[0] += cx;
                    n[1] += cy;
                    n[2] += cx;
                    n[3] += cy;
                    n[4] += cx;
                    n[5] += cy;
                }
                let _ = cr.curve_to(n[0], n[1], n[2], n[3], n[4], n[5]);
                cx = n[4];
                cy = n[5];
            }
            b'Q' | b'q' => {
                let nums = [(); 4].map(|_| read_number(bytes, &mut i));
                if nums.iter().any(|n| n.is_none()) {
                    return;
                }
                let mut n = nums.map(|v| v.unwrap());
                if command == b'q' {
                    n[0] += cx;
                    n[1] += cy;
                    n[2] += cx;
                    n[3] += cy;
                }
                let c1x = cx + (2.0 / 3.0) * (n[0] - cx);
                let c1y = cy + (2.0 / 3.0) * (n[1] - cy);
                let c2x = n[2] + (2.0 / 3.0) * (n[0] - n[2]);
                let c2y = n[3] + (2.0 / 3.0) * (n[1] - n[3]);
                let _ = cr.curve_to(c1x, c1y, c2x, c2y, n[2], n[3]);
                cx = n[2];
                cy = n[3];
            }
            b'A' | b'a' => {
                let nums = [(); 7].map(|_| read_number(bytes, &mut i));
                if nums.iter().any(|n| n.is_none()) {
                    return;
                }
                let n = nums.map(|v| v.unwrap());
                let mut x = n[5];
                let mut y = n[6];
                if command == b'a' {
                    x += cx;
                    y += cy;
                }
                svg_arc(cr, &mut cx, &mut cy, n[0], n[1], n[2], n[3] != 0.0, n[4] != 0.0, x, y);
            }
            b'Z' | b'z' => {
                let _ = cr.close_path();
                cx = sx;
                cy = sy;
            }
            _ => return,
        }
    }
}

/// Parse every icon path once so a bad path fails the self-test, not the panel.
pub fn self_test_paths() -> Result<(), String> {
    let surface = ImageSurface::create(Format::ARgb32, 16, 16).map_err(|e| e.to_string())?;
    let cr = Context::new(&surface).map_err(|e| e.to_string())?;
    for (path, view) in [
        (MIC_PATH, 24.0),
        (MIC_OFF_PATH, 512.0),
        (SPEAKER_PATH, 56.0),
        (SPEAKER_OFF_PATH, 56.0),
        (ADD_PATH, 28.0),
        (LANG_PATH, 16.0),
        (HEADPHONES_PATH, 16.0),
        (RECORD_PATH, 24.0),
        (STOP_PATH, 24.0),
        (PLAY_PATH, 24.0),
    ] {
        icon(&cr, path, view, 0.0, 0.0, 16.0, TEXT);
    }
    Ok(())
}
