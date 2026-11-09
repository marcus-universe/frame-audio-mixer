//! A tiny SVG path-data parser. It renders the icon paths embedded in
//! [`super::icons`] straight onto a cairo context — nothing is fetched or
//! rasterized ahead of time.

use cairo::{Context, FillRule, Format, ImageSurface};
use std::f64::consts::PI;

use super::palette::{Rgb, TEXT};
use super::shape::{fill, surface_to_rgba};

/// One glyph from `path` data in a `view`-sized box, filled into a `size` square.
pub fn icon(cr: &Context, path: &str, view: f64, x: f64, y: f64, size: f64, c: Rgb) {
    let _ = cr.save();
    let scale = size / view;
    cr.translate(x + (size - view * scale) / 2.0, y);
    cr.scale(scale, scale);
    cr.set_fill_rule(FillRule::EvenOdd);
    append_svg(cr, path);
    fill(cr, c, 1.0);
    let _ = cr.restore();
}

/// Opaque pixels of one glyph drawn at 64 px. Used to catch a path the parser drops.
pub(super) fn icon_coverage(path: &str, view: f64) -> Result<usize, String> {
    let size = 64;
    let surface = ImageSurface::create(Format::ARgb32, size, size).map_err(|e| e.to_string())?;
    {
        let cr = Context::new(&surface).map_err(|e| e.to_string())?;
        icon(&cr, path, view, 0.0, 0.0, size as f64, TEXT);
    }
    let mut surface = surface;
    let rgba = surface_to_rgba(&mut surface);
    Ok(rgba.chunks(4).filter(|px| px[3] > 16).count())
}

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
        cr.line_to(x2, y2);
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
        cr.curve_to(p1x, p1y, p2x, p2y, ex, ey);
    }
    *cx = x2;
    *cy = y2;
}

fn quad_curve(cr: &Context, x0: f64, y0: f64, cpx: f64, cpy: f64, x: f64, y: f64) {
    let c1x = x0 + (2.0 / 3.0) * (cpx - x0);
    let c1y = y0 + (2.0 / 3.0) * (cpy - y0);
    let c2x = x + (2.0 / 3.0) * (cpx - x);
    let c2y = y + (2.0 / 3.0) * (cpy - y);
    cr.curve_to(c1x, c1y, c2x, c2y, x, y);
}

pub(super) fn append_svg(cr: &Context, d: &str) {
    let bytes = d.as_bytes();
    let mut i = 0;
    let mut cx = 0.0;
    let mut cy = 0.0;
    let mut sx = 0.0;
    let mut sy = 0.0;
    let mut command = 0u8;
    let mut qcx = 0.0;
    let mut qcy = 0.0;
    let mut quad = false;
    let mut ccx = 0.0;
    let mut ccy = 0.0;
    let mut cubic = false;
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
                cr.move_to(x, y);
                cx = x;
                cy = y;
                sx = x;
                sy = y;
                quad = false;
                cubic = false;
            }
            b'L' | b'l' => {
                let (Some(mut x), Some(mut y)) = (read_number(bytes, &mut i), read_number(bytes, &mut i)) else { return };
                if command == b'l' {
                    x += cx;
                    y += cy;
                }
                cr.line_to(x, y);
                cx = x;
                cy = y;
                quad = false;
                cubic = false;
            }
            b'H' | b'h' => {
                let Some(mut x) = read_number(bytes, &mut i) else { return };
                if command == b'h' {
                    x += cx;
                }
                cr.line_to(x, cy);
                cx = x;
                quad = false;
                cubic = false;
            }
            b'V' | b'v' => {
                let Some(mut y) = read_number(bytes, &mut i) else { return };
                if command == b'v' {
                    y += cy;
                }
                cr.line_to(cx, y);
                cy = y;
                quad = false;
                cubic = false;
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
                cr.curve_to(n[0], n[1], n[2], n[3], n[4], n[5]);
                ccx = n[2];
                ccy = n[3];
                cx = n[4];
                cy = n[5];
                quad = false;
                cubic = true;
            }
            b'S' | b's' => {
                let nums = [(); 4].map(|_| read_number(bytes, &mut i));
                if nums.iter().any(|n| n.is_none()) {
                    return;
                }
                let mut n = nums.map(|v| v.unwrap());
                if command == b's' {
                    n[0] += cx;
                    n[1] += cy;
                    n[2] += cx;
                    n[3] += cy;
                }
                let (c1x, c1y) = if cubic { (2.0 * cx - ccx, 2.0 * cy - ccy) } else { (cx, cy) };
                cr.curve_to(c1x, c1y, n[0], n[1], n[2], n[3]);
                ccx = n[0];
                ccy = n[1];
                cx = n[2];
                cy = n[3];
                quad = false;
                cubic = true;
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
                quad_curve(cr, cx, cy, n[0], n[1], n[2], n[3]);
                qcx = n[0];
                qcy = n[1];
                cx = n[2];
                cy = n[3];
                quad = true;
                cubic = false;
            }
            b'T' | b't' => {
                let (Some(mut x), Some(mut y)) = (read_number(bytes, &mut i), read_number(bytes, &mut i)) else { return };
                if command == b't' {
                    x += cx;
                    y += cy;
                }
                let (cpx, cpy) = if quad { (2.0 * cx - qcx, 2.0 * cy - qcy) } else { (cx, cy) };
                quad_curve(cr, cx, cy, cpx, cpy, x, y);
                qcx = cpx;
                qcy = cpy;
                cx = x;
                cy = y;
                quad = true;
                cubic = false;
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
                quad = false;
                cubic = false;
            }
            b'Z' | b'z' => {
                cr.close_path();
                cx = sx;
                cy = sy;
                quad = false;
                cubic = false;
            }
            _ => return,
        }
    }
}
