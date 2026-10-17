//! Cairo drawing. Colors match Frame Mic Tuner's panel so the two apps sit together.

use cairo::{Context, FillRule, FontFace, Format, ImageSurface};
use freetype::Library as FtLibrary;
use std::cell::Cell;
use std::f64::consts::PI;

pub const WIDTH: i32 = 1200;
pub const HEIGHT: i32 = 788;

/// Dashboard pixels. Extend is 21:9. With the channel list it is 16:12, as wide
/// as that 21:9 dashboard so the sliders stay full size.
pub fn panel_pixels(wide: bool, list: bool) -> (i32, i32) {
    if !wide {
        return (WIDTH, HEIGHT);
    }
    let long = (HEIGHT as f64 * 21.0 / 9.0).round() as i32;
    if !list {
        return (long, HEIGHT);
    }
    let unit = (long as f64 / 16.0).round() as i32;
    (unit * 16, unit * 12)
}

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
const PINK: Rgb = Rgb { r: 0xe0 as f64 / 255.0, g: 0x7e as f64 / 255.0, b: 0xf5 as f64 / 255.0 };
const PINK_HOVER: Rgb = Rgb { r: 0xe9 as f64 / 255.0, g: 0x9a as f64 / 255.0, b: 0xf8 as f64 / 255.0 };
const PINK_DOWN: Rgb = Rgb { r: 0xc8 as f64 / 255.0, g: 0x65 as f64 / 255.0, b: 0xde as f64 / 255.0 };
const PINK_INK: Rgb = Rgb { r: 0x1c as f64 / 255.0, g: 0x0f as f64 / 255.0, b: 0x22 as f64 / 255.0 };
/// #33C4FF, the blue in the logo.
const BLUE: Rgb = Rgb { r: 0x33 as f64 / 255.0, g: 0xc4 as f64 / 255.0, b: 0xff as f64 / 255.0 };
const BLUE_HOVER: Rgb = Rgb { r: 0x66 as f64 / 255.0, g: 0xd4 as f64 / 255.0, b: 0xff as f64 / 255.0 };
const BLUE_DOWN: Rgb = Rgb { r: 0x1a as f64 / 255.0, g: 0xa8 as f64 / 255.0, b: 0xe0 as f64 / 255.0 };
const BLUE_INK: Rgb = Rgb { r: 0x0e as f64 / 255.0, g: 0x10 as f64 / 255.0, b: 0x15 as f64 / 255.0 };

thread_local! {
    static THEME_PINK: Cell<bool> = const { Cell::new(false) };
}

/// `pink` selects the previous accent. Anything else is the blue logo color.
pub fn set_accent(name: &str) {
    THEME_PINK.with(|cell| cell.set(name == "pink"));
}

pub fn accent() -> Rgb {
    if THEME_PINK.with(|cell| cell.get()) { PINK } else { BLUE }
}

pub fn accent_hover() -> Rgb {
    if THEME_PINK.with(|cell| cell.get()) { PINK_HOVER } else { BLUE_HOVER }
}

pub fn accent_down() -> Rgb {
    if THEME_PINK.with(|cell| cell.get()) { PINK_DOWN } else { BLUE_DOWN }
}

pub fn on_accent() -> Rgb {
    if THEME_PINK.with(|cell| cell.get()) { PINK_INK } else { BLUE_INK }
}
pub const DANGER: Rgb = Rgb { r: 0xe5 as f64 / 255.0, g: 0x53 as f64 / 255.0, b: 0x4b as f64 / 255.0 };
pub const DANGER_FILL: Rgb = Rgb { r: 0x2a as f64 / 255.0, g: 0x15 as f64 / 255.0, b: 0x18 as f64 / 255.0 };
pub const DANGER_HOVER: Rgb = Rgb { r: 0x3a as f64 / 255.0, g: 0x1c as f64 / 255.0, b: 0x1f as f64 / 255.0 };
pub const DANGER_DOWN: Rgb = Rgb { r: 0x4a as f64 / 255.0, g: 0x20 as f64 / 255.0, b: 0x24 as f64 / 255.0 };
pub const DANGER_TEXT: Rgb = Rgb { r: 0xff as f64 / 255.0, g: 0x8a as f64 / 255.0, b: 0x80 as f64 / 255.0 };
pub const CONTROL: Rgb = Rgb { r: 0x20 as f64 / 255.0, g: 0x24 as f64 / 255.0, b: 0x2c as f64 / 255.0 };
pub const CONTROL_HOVER: Rgb = Rgb { r: 0x2a as f64 / 255.0, g: 0x2f as f64 / 255.0, b: 0x38 as f64 / 255.0 };
pub const CONTROL_DOWN: Rgb = Rgb { r: 0x34 as f64 / 255.0, g: 0x39 as f64 / 255.0, b: 0x44 as f64 / 255.0 };
pub const LABEL_SOFT: Rgb = Rgb { r: 0xc9 as f64 / 255.0, g: 0xcd as f64 / 255.0, b: 0xd4 as f64 / 255.0 };
pub const DIVIDER: Rgb = Rgb { r: 0x3a as f64 / 255.0, g: 0x3f as f64 / 255.0, b: 0x4a as f64 / 255.0 };
pub const CHIP_OK_FILL: Rgb = Rgb { r: 0x17 as f64 / 255.0, g: 0x34 as f64 / 255.0, b: 0x24 as f64 / 255.0 };
pub const CHIP_OK_TEXT: Rgb = Rgb { r: 0x4a as f64 / 255.0, g: 0xde as f64 / 255.0, b: 0x80 as f64 / 255.0 };

/// How a button is painted. Matches Frame Mic Tuner's frame-ui button kinds.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Plain,
    Accent,
    /// Red border. Used for Quit and Record.
    Danger,
    /// Red fill. Used while Quit is armed, or while a recording can be stopped.
    DangerArmed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Look {
    Rest,
    Hover,
    Down,
}

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

pub fn surface_wh(width: i32, height: i32) -> Result<ImageSurface, String> {
    ImageSurface::create(Format::ARgb32, width, height).map_err(|e| e.to_string())
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

/// Break `text` into lines no wider than `max_w`. Text without spaces (Japanese,
/// Chinese) breaks between characters. Past `max_lines` the last line ends in an ellipsis.
pub fn wrap(cr: &Context, fonts: &Fonts, bold: bool, size: f64, max_w: f64, max_lines: usize, text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut pieces: Vec<String> = Vec::new();
    let mut word = String::new();
    for c in text.chars() {
        if is_cjk(c) {
            if !word.is_empty() {
                pieces.push(std::mem::take(&mut word));
            }
            pieces.push(c.to_string());
        } else {
            word.push(c);
            if c == ' ' {
                pieces.push(std::mem::take(&mut word));
            }
        }
    }
    if !word.is_empty() {
        pieces.push(word);
    }
    for piece in pieces {
        let candidate = format!("{line}{piece}");
        if line.is_empty() || text_width(cr, fonts, bold, size, candidate.trim_end()) <= max_w {
            line = candidate;
        } else {
            lines.push(line.trim_end().to_string());
            line = piece.trim_start().to_string();
        }
    }
    if !line.trim().is_empty() {
        lines.push(line.trim_end().to_string());
    }
    if lines.len() > max_lines.max(1) {
        let rest = lines.split_off(max_lines.max(1) - 1).join(" ");
        lines.push(rest);
    }
    lines
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x3000..=0x30FF | 0x3400..=0x9FFF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF)
}

pub fn stroke(cr: &Context, c: Rgb, width: f64) {
    cr.set_source_rgb(c.r, c.g, c.b);
    cr.set_line_width(width);
    let _ = cr.stroke();
}

/// Fill and border follow Frame Mic Tuner. `look` is Rest unless the pointer is over the button.
pub fn button(cr: &Context, fonts: &Fonts, x: f64, y: f64, w: f64, h: f64, caption: &str, kind: ButtonKind, look: Look) {
    let (fill_c, border, text_c) = match kind {
        ButtonKind::Accent => {
            let fill_c = match look {
                Look::Down => accent_down(),
                Look::Hover => accent_hover(),
                Look::Rest => accent(),
            };
            (fill_c, fill_c, on_accent())
        }
        ButtonKind::Danger => {
            let fill_c = match look {
                Look::Down => DANGER_DOWN,
                Look::Hover => DANGER_HOVER,
                Look::Rest => DANGER_FILL,
            };
            (fill_c, DANGER, TEXT)
        }
        ButtonKind::DangerArmed => (DANGER, DANGER, on_accent()),
        ButtonKind::Plain => {
            let fill_c = match look {
                Look::Down => CONTROL_DOWN,
                Look::Hover => CONTROL_HOVER,
                Look::Rest => CONTROL,
            };
            (fill_c, BORDER, TEXT)
        }
    };
    rounded(cr, x, y, w, h, h.min(32.0) / 2.0);
    fill(cr, fill_c, 1.0);
    rounded(cr, x, y, w, h, h.min(32.0) / 2.0);
    stroke(cr, border, 2.0);
    if !caption.is_empty() {
        let tw = text_width(cr, fonts, true, 16.0, caption);
        label(cr, fonts, true, 16.0, text_c, x + (w - tw) / 2.0, y + h / 2.0 + 6.0, caption);
    }
}

/// Colors of one button kind at rest. The self-test checks these against frame-ui.
pub fn button_colors(kind: ButtonKind) -> (Rgb, Rgb, Rgb) {
    match kind {
        ButtonKind::Plain => (CONTROL, BORDER, TEXT),
        ButtonKind::Accent => (accent(), accent(), on_accent()),
        ButtonKind::Danger => (DANGER_FILL, DANGER, TEXT),
        ButtonKind::DangerArmed => (DANGER, DANGER, on_accent()),
    }
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

pub fn fill_circle(cr: &Context, cx: f64, cy: f64, r: f64, c: Rgb) {
    let _ = cr.arc(cx, cy, r, 0.0, 2.0 * PI);
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
    let _ = cr.close_path();
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

/// Color of a meter reading. `fraction` is 0 at the quiet end (blue) and 1 at the loud end (pink).
pub fn level_color(fraction: f64) -> Rgb {
    let f = fraction.clamp(0.0, 1.0);
    Rgb {
        r: BLUE.r + (PINK.r - BLUE.r) * f,
        g: BLUE.g + (PINK.g - BLUE.g) * f,
        b: BLUE.b + (PINK.b - BLUE.b) * f,
    }
}

/// Brand gradient, blue at the start of the span and pink at the end.
/// `vertical` grows up from the bottom. Otherwise the fill grows to the right.
pub fn fill_meter(cr: &Context, x: f64, y: f64, full_w: f64, h: f64, radius: f64, fraction: f64, vertical: bool) {
    let fraction = fraction.clamp(0.0, 1.0);
    if fraction <= 0.004 || full_w <= 0.0 || h <= 0.0 {
        return;
    }
    let _ = cr.save();
    rounded(cr, x, y, full_w, h, radius);
    let _ = cr.clip();
    let grad = if vertical {
        cairo::LinearGradient::new(0.0, y + h, 0.0, y)
    } else {
        cairo::LinearGradient::new(x, 0.0, x + full_w, 0.0)
    };
    grad.add_color_stop_rgb(0.0, BLUE.r, BLUE.g, BLUE.b);
    grad.add_color_stop_rgb(1.0, PINK.r, PINK.g, PINK.b);
    let _ = cr.set_source(&grad);
    if vertical {
        let lit = y + h - h * fraction;
        cr.rectangle(x, lit, full_w, y + h - lit);
    } else {
        cr.rectangle(x, y, full_w * fraction, h);
    }
    let _ = cr.fill();
    let _ = cr.restore();
}

/// Path and view box. Mute and live glyphs share one view box, so the symbol
/// stays the same size and stays centered when the slash is drawn.
pub fn fader_icon(glyph: FaderGlyph, muted: bool) -> (&'static str, f64) {
    match (glyph, muted) {
        (FaderGlyph::Mic, true) => (MIC_OFF_PATH, FADER_ICON_VIEW),
        (FaderGlyph::Mic, false) => (MIC_PATH, FADER_ICON_VIEW),
        (FaderGlyph::Speaker, true) => (SPEAKER_OFF_PATH, FADER_ICON_VIEW),
        (FaderGlyph::Speaker, false) => (SPEAKER_PATH, FADER_ICON_VIEW),
    }
}

fn paint_fader_glyph(cr: &Context, glyph: FaderGlyph, muted: bool, x: f64, y: f64, size: f64) {
    let (path, view) = fader_icon(glyph, muted);
    icon(cr, path, view, x, y, size, TEXT);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FaderGlyph {
    Mic,
    Speaker,
}

pub struct FaderGeom {
    pub capsule_x: f64,
    pub capsule_y: f64,
    pub capsule_w: f64,
    pub capsule_h: f64,
    pub meter_right: f64,
    pub icon_x: f64,
    pub icon_y: f64,
    pub icon_s: f64,
}

/// One mix fader: a 0–150% capsule, an optional meter, the percent, and a mute icon.
/// `cx` is the horizontal center of the capsule. `peak` is a linear 0–1 level.
pub fn paint_fader(
    cr: &Context,
    fonts: &Fonts,
    cx: f64,
    top: f64,
    cap_h: f64,
    percent: f64,
    peak: f32,
    meters: bool,
    muted: bool,
    glyph: FaderGlyph,
    snaps: bool,
) -> FaderGeom {
    let cap_w = 32.0;
    let x = cx - cap_w / 2.0;
    let radius = cap_w / 2.0;
    let fraction = (percent / 150.0).clamp(0.0, 1.0);
    let _ = cr.save();
    rounded(cr, x, top, cap_w, cap_h, radius);
    let _ = cr.clip();
    rounded(cr, x, top, cap_w, cap_h, 0.0);
    fill(cr, BG, 1.0);
    let fill_top = top + cap_h - cap_h * fraction;
    if top + cap_h - fill_top > 0.5 {
        rounded(cr, x, fill_top, cap_w, top + cap_h - fill_top, 0.0);
        fill(cr, accent(), 1.0);
    }
    let _ = cr.restore();
    rounded(cr, x, top, cap_w, cap_h, radius);
    stroke(cr, BORDER, 1.5);
    // Snap points below 100% are dots, so they never read as the 100% line.
    if snaps {
        for point in crate::config::FADER_SNAPS {
            let mark = point * 100.0;
            if (mark - 100.0).abs() < 0.5 {
                continue;
            }
            let dot_y = top + cap_h - cap_h * (mark / 150.0);
            let filled = mark <= percent + 0.01;
            let (c, a) = if filled { (on_accent(), 0.6) } else { (TEXT, 0.45) };
            let _ = cr.arc(cx, dot_y, 2.5, 0.0, 2.0 * PI);
            fill(cr, c, a);
        }
    }
    let notch_y = top + cap_h - cap_h * (100.0 / 150.0);
    cr.set_source_rgb(TEXT.r, TEXT.g, TEXT.b);
    cr.set_line_width(2.0);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.move_to(x + 5.0, notch_y);
    cr.line_to(x + cap_w - 5.0, notch_y);
    let _ = cr.stroke();

    let post = if muted { 0.0 } else { peak * (percent / 100.0) as f32 };
    let db = if post <= 1e-6 { -120.0 } else { 20.0 * (post as f64).log10() };
    let level = ((db + 48.0) / 48.0).clamp(0.0, 1.0);
    let meter_right = if meters {
        let mx = x + cap_w + 4.0;
        let mw = 6.0;
        rounded(cr, mx, top, mw, cap_h, 3.0);
        fill(cr, BG, 1.0);
        fill_meter(cr, mx, top, mw, cap_h, 3.0, level, true);
        mx + mw
    } else {
        x + cap_w
    };

    let pct = format!("{:.0}%", percent);
    let pct_color = if (percent - 100.0).abs() < 0.5 { accent() } else { TEXT };
    let tw = text_width(cr, fonts, true, 13.0, &pct);
    label(cr, fonts, true, 13.0, pct_color, cx - tw / 2.0, top + cap_h + 13.0, &pct);

    let icon_s = 22.0;
    let icon_x = cx - icon_s / 2.0;
    let icon_y = top + cap_h + 30.0 - icon_s / 2.0;
    paint_fader_glyph(cr, glyph, muted, icon_x, icon_y, icon_s);
    if meters {
        if db > -47.5 {
            let shown = if db > -0.5 { 0.0 } else { db };
            let text = format!("{shown:.0}");
            let dw = text_width(cr, fonts, true, 11.0, &text);
            label(cr, fonts, true, 11.0, level_color(level), cx - dw / 2.0, top + cap_h + 52.0, &text);
        }
    }
    FaderGeom {
        capsule_x: x,
        capsule_y: top,
        capsule_w: cap_w,
        capsule_h: cap_h,
        meter_right,
        icon_x,
        icon_y,
        icon_s,
    }
}

/// A sideways fader: the mute icon, then the track growing to the right, then the percent.
/// The 100% mark stays a line.
pub fn paint_side_fader(
    cr: &Context,
    fonts: &Fonts,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    percent: f64,
    peak: f32,
    meters: bool,
    muted: bool,
    glyph: FaderGlyph,
    snaps: bool,
) -> FaderGeom {
    let track_h = 26.0;
    let icon_box = 28.0;
    let icon_s = 22.0;
    let pct_w = 46.0;
    let track_w = (w - icon_box - pct_w - 12.0).max(36.0);
    let track_x = x + icon_box + 6.0;
    let track_y = y + (h - track_h) / 2.0;
    let radius = track_h / 2.0;
    let fraction = (percent / 150.0).clamp(0.0, 1.0);
    let _ = cr.save();
    rounded(cr, track_x, track_y, track_w, track_h, radius);
    let _ = cr.clip();
    rounded(cr, track_x, track_y, track_w, track_h, 0.0);
    fill(cr, BG, 1.0);
    if track_w * fraction > 0.5 {
        rounded(cr, track_x, track_y, track_w * fraction, track_h, 0.0);
        fill(cr, accent(), 1.0);
    }
    let _ = cr.restore();
    rounded(cr, track_x, track_y, track_w, track_h, radius);
    stroke(cr, BORDER, 1.5);
    if snaps {
        for point in crate::config::FADER_SNAPS {
            let mark = point * 100.0;
            if (mark - 100.0).abs() < 0.5 {
                continue;
            }
            let dot_x = track_x + track_w * (mark / 150.0);
            let filled = mark <= percent + 0.01;
            let (c, a) = if filled { (on_accent(), 0.6) } else { (TEXT, 0.45) };
            let _ = cr.arc(dot_x, track_y + track_h / 2.0, 2.5, 0.0, 2.0 * PI);
            fill(cr, c, a);
        }
    }
    let notch_x = track_x + track_w * (100.0 / 150.0);
    cr.set_source_rgb(TEXT.r, TEXT.g, TEXT.b);
    cr.set_line_width(2.0);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.move_to(notch_x, track_y + 4.0);
    cr.line_to(notch_x, track_y + track_h - 4.0);
    let _ = cr.stroke();

    let post = if muted { 0.0 } else { peak * (percent / 100.0) as f32 };
    let db = if post <= 1e-6 { -120.0 } else { 20.0 * (post as f64).log10() };
    let level = ((db + 48.0) / 48.0).clamp(0.0, 1.0);
    if meters {
        let my = (track_y - 8.0).max(y);
        rounded(cr, track_x, my, track_w, 5.0, 2.5);
        fill(cr, BG, 1.0);
        fill_meter(cr, track_x, my, track_w, 5.0, 2.5, level, false);
    }

    let pct = format!("{:.0}%", percent);
    let pct_color = if (percent - 100.0).abs() < 0.5 { accent() } else { TEXT };
    label(cr, fonts, true, 13.0, pct_color, track_x + track_w + 6.0, track_y + track_h / 2.0 + 4.0, &pct);

    let icon_x = x + (icon_box - icon_s) / 2.0;
    let icon_y = y + (h - icon_s) / 2.0;
    paint_fader_glyph(cr, glyph, muted, icon_x, icon_y, icon_s);
    FaderGeom {
        capsule_x: track_x,
        capsule_y: track_y,
        capsule_w: track_w,
        capsule_h: track_h,
        meter_right: track_x + track_w,
        icon_x,
        icon_y,
        icon_s,
    }
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

// Icon paths are copied from Iconify and stored here. Nothing is fetched at runtime.
// Licenses are in NOTICE. The fader pair shares one view box so mute does not rescale the glyph.
/// Phosphor microphone, microphone-slash, speaker-high and speaker-slash. View box 256.
const FADER_ICON_VIEW: f64 = 256.0;
/// ph:microphone-fill, view box 256, MIT.
pub const MIC_PATH: &str = "M80 128V64a48 48 0 0 1 96 0v64a48 48 0 0 1-96 0m128 0a8 8 0 0 0-16 0a64 64 0 0 1-128 0a8 8 0 0 0-16 0a80.11 80.11 0 0 0 72 79.6V240a8 8 0 0 0 16 0v-32.4a80.11 80.11 0 0 0 72-79.6";
/// ph:microphone-slash-fill, view box 256, MIT.
pub const MIC_OFF_PATH: &str = "M213.38 229.92a8 8 0 0 1-11.3-.54l-30.92-34A78.8 78.8 0 0 1 136 207.59V240a8 8 0 0 1-16 0v-32.4A80.11 80.11 0 0 1 48 128a8 8 0 0 1 16 0a64.07 64.07 0 0 0 64 64a63.4 63.4 0 0 0 32.21-8.68l-11.1-12.2A48 48 0 0 1 80 128V95.09L42.08 53.38a8 8 0 0 1 11.84-10.76l160 176a8 8 0 0 1-.54 11.3m-24.19-63.13a7.9 7.9 0 0 0 3.51.82a8 8 0 0 0 7.19-4.49A79.16 79.16 0 0 0 208 128a8 8 0 0 0-16 0a63.3 63.3 0 0 1-6.48 28.09a8 8 0 0 0 3.67 10.7m-27.33-29.22a8 8 0 0 0 13.88-4.57a50 50 0 0 0 .26-5V64a48 48 0 0 0-92-19.13a8 8 0 0 0 1.41 8.57Z";
/// ph:speaker-high-fill, view box 256, MIT.
pub const SPEAKER_PATH: &str = "M160 32.25v191.44a8.29 8.29 0 0 1-3.91 7.18a8 8 0 0 1-9-.56l-65.57-51a4 4 0 0 1-1.52-3.15V79.84a4 4 0 0 1 1.55-3.15l65.57-51a8 8 0 0 1 10 .16a8.27 8.27 0 0 1 2.88 6.4M60 80H32a16 16 0 0 0-16 16v64a16 16 0 0 0 16 16h28a4 4 0 0 0 4-4V84a4 4 0 0 0-4-4m126.77 20.84a8 8 0 0 0-.72 11.3a24 24 0 0 1 0 31.72a8 8 0 1 0 12 10.58a40 40 0 0 0 0-52.88a8 8 0 0 0-11.31-.72Zm40.89-26.17a8 8 0 1 0-11.92 10.66a64 64 0 0 1 0 85.34a8 8 0 1 0 11.92 10.66a80 80 0 0 0 0-106.66";
/// ph:speaker-slash-fill, view box 256, MIT.
pub const SPEAKER_OFF_PATH: &str = "M213.92 210.62a8 8 0 1 1-11.84 10.76L160 175.09v48.6a8.29 8.29 0 0 1-3.91 7.18a8 8 0 0 1-9-.56l-65.55-51a4 4 0 0 1-1.54-3.13V87.09L42.08 45.38a8 8 0 1 1 11.84-10.76Zm-27.21-55.46a8 8 0 0 0 11.29-.7a40 40 0 0 0 0-52.88a8 8 0 1 0-12 10.57a24 24 0 0 1 0 31.72a8 8 0 0 0 .71 11.29m40.92-80.49a8 8 0 1 0-11.92 10.66a64 64 0 0 1 0 85.34a8 8 0 1 0 11.92 10.66a80 80 0 0 0 0-106.66M153 119.87a4 4 0 0 0 7-2.7V32.25a8.27 8.27 0 0 0-2.88-6.4a8 8 0 0 0-10-.16l-43.29 33.64a4 4 0 0 0-.5 5.85ZM60 80H32a16 16 0 0 0-16 16v64a16 16 0 0 0 16 16h28a4 4 0 0 0 4-4V84a4 4 0 0 0-4-4";
/// fluent:add-square-28-filled, view box 28, MIT.
pub const ADD_PATH: &str = "M3 6.75A3.75 3.75 0 0 1 6.75 3h14.5A3.75 3.75 0 0 1 25 6.75v14.5A3.75 3.75 0 0 1 21.25 25H6.75A3.75 3.75 0 0 1 3 21.25zm11 .75a.75.75 0 0 0-.75.75v5h-5a.75.75 0 0 0 0 1.5h5v5a.75.75 0 0 0 1.5 0v-5h5a.75.75 0 0 0 0-1.5h-5v-5A.75.75 0 0 0 14 7.5";
/// heroicons:language-16-solid, view box 16, MIT.
pub const LANG_PATH: &str = "M11 5a.75.75 0 0 1 .688.452l3.25 7.5a.75.75 0 1 1-1.376.596L12.89 12H9.109l-.67 1.548a.75.75 0 1 1-1.377-.596l3.25-7.5A.75.75 0 0 1 11 5m-1.24 5.5h2.48L11 7.636zM5 1a.75.75 0 0 1 .75.75v1.261a25 25 0 0 1 2.598.211a.75.75 0 1 1-.2 1.487q-.33-.045-.662-.08A13 13 0 0 1 5.92 8.058q.356.456.752.873a.75.75 0 0 1-1.086 1.035A13 13 0 0 1 5 9.307a13 13 0 0 1-2.841 2.546a.75.75 0 0 1-.827-1.251A11.6 11.6 0 0 0 4.08 8.057a13 13 0 0 1-.554-.938a.75.75 0 1 1 1.323-.707l.15.271c.388-.68.708-1.405.952-2.164a24 24 0 0 0-4.1.19a.75.75 0 0 1-.2-1.487q1.28-.171 2.598-.211V1.75A.75.75 0 0 1 5 1";
/// at-icons:headphones, view box 16, MIT, Valentin Fossati.
pub const HEADPHONES_PATH: &str = "M8 1a7 7 0 0 1 7 7v5a2 2 0 0 1-2 2h-2a1 1 0 0 1-1-1v-4a1 1 0 0 1 1-1h2V8A5 5 0 0 0 3 8v1h2a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1H3a2 2 0 0 1-2-2V8a7 7 0 0 1 7-7";
/// material-symbols:settings-rounded, view box 24, Apache-2.0, Google.
pub const SETTINGS_PATH: &str = "M10.825 22q-.675 0-1.162-.45t-.588-1.1L8.85 18.8q-.325-.125-.612-.3t-.563-.375l-1.55.65q-.625.275-1.25.05t-.975-.8l-1.175-2.05q-.35-.575-.2-1.225t.675-1.075l1.325-1Q4.5 12.5 4.5 12.337v-.675q0-.162.025-.337l-1.325-1Q2.675 9.9 2.525 9.25t.2-1.225L3.9 5.975q.35-.575.975-.8t1.25.05l1.55.65q.275-.2.575-.375t.6-.3l.225-1.65q.1-.65.588-1.1T10.825 2h2.35q.675 0 1.163.45t.587 1.1l.225 1.65q.325.125.613.3t.562.375l1.55-.65q.625-.275 1.25-.05t.975.8l1.175 2.05q.35.575.2 1.225t-.675 1.075l-1.325 1q.025.175.025.338v.674q0 .163-.05.338l1.325 1q.525.425.675 1.075t-.2 1.225l-1.2 2.05q-.35.575-.975.8t-1.25-.05l-1.5-.65q-.275.2-.575.375t-.6.3l-.225 1.65q-.1.65-.587 1.1t-1.163.45zm1.225-6.5q1.45 0 2.475-1.025T15.55 12t-1.025-2.475T12.05 8.5q-1.475 0-2.488 1.025T8.55 12t1.013 2.475T12.05 15.5";
/// ep:close-bold, view box 1024, MIT, Element Plus.
pub const CLOSE_PATH: &str = "M195.2 195.2a64 64 0 0 1 90.5 0L512 421.5l226.3-226.3a64 64 0 0 1 90.5 90.5L602.5 512l226.3 226.3a64 64 0 0 1-90.5 90.5L512 602.5L285.7 828.8a64 64 0 0 1-90.5-90.5L421.5 512L195.2 285.7a64 64 0 0 1 0-90.5";

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

fn quad_curve(cr: &Context, x0: f64, y0: f64, cpx: f64, cpy: f64, x: f64, y: f64) {
    let c1x = x0 + (2.0 / 3.0) * (cpx - x0);
    let c1y = y0 + (2.0 / 3.0) * (cpy - y0);
    let c2x = x + (2.0 / 3.0) * (cpx - x);
    let c2y = y + (2.0 / 3.0) * (cpy - y);
    let _ = cr.curve_to(c1x, c1y, c2x, c2y, x, y);
}

fn append_svg(cr: &Context, d: &str) {
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
                quad = false;
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
                quad = false;
            }
            b'H' | b'h' => {
                let Some(mut x) = read_number(bytes, &mut i) else { return };
                if command == b'h' {
                    x += cx;
                }
                let _ = cr.line_to(x, cy);
                cx = x;
                quad = false;
            }
            b'V' | b'v' => {
                let Some(mut y) = read_number(bytes, &mut i) else { return };
                if command == b'v' {
                    y += cy;
                }
                let _ = cr.line_to(cx, y);
                cy = y;
                quad = false;
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
                quad = false;
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
            }
            b'Z' | b'z' => {
                let _ = cr.close_path();
                cx = sx;
                cy = sy;
                quad = false;
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
        (MIC_PATH, FADER_ICON_VIEW),
        (MIC_OFF_PATH, FADER_ICON_VIEW),
        (SPEAKER_PATH, FADER_ICON_VIEW),
        (SPEAKER_OFF_PATH, FADER_ICON_VIEW),
        (ADD_PATH, 28.0),
        (LANG_PATH, 16.0),
        (HEADPHONES_PATH, 16.0),
        (SETTINGS_PATH, 24.0),
        (CLOSE_PATH, 1024.0),
    ] {
        icon(&cr, path, view, 0.0, 0.0, 16.0, TEXT);
    }
    let (fill_c, border, _) = button_colors(ButtonKind::Danger);
    if (fill_c.r - DANGER_FILL.r).abs() > 0.01 || (border.r - DANGER.r).abs() > 0.01 {
        return Err("danger button colors".into());
    }
    let (armed, _, _) = button_colors(ButtonKind::DangerArmed);
    if (armed.r - DANGER.r).abs() > 0.01 {
        return Err("armed danger fill".into());
    }
    if fader_icon(FaderGlyph::Speaker, true).0 != SPEAKER_OFF_PATH || fader_icon(FaderGlyph::Mic, true).0 != MIC_OFF_PATH {
        return Err("muted icons".into());
    }
    if fader_icon(FaderGlyph::Speaker, false).0 != SPEAKER_PATH || fader_icon(FaderGlyph::Mic, false).0 != MIC_PATH {
        return Err("live icons".into());
    }
    let (list_w, list_h) = panel_pixels(true, true);
    if list_w * 12 != list_h * 16 || list_w <= WIDTH {
        return Err(format!("list extend {list_w}x{list_h}"));
    }
    let (wide_w, wide_h) = panel_pixels(true, false);
    if wide_h != HEIGHT || wide_w <= WIDTH {
        return Err(format!("extend 21:9 {wide_w}x{wide_h}"));
    }
    if panel_pixels(false, true) != (WIDTH, HEIGHT) || panel_pixels(false, false) != (WIDTH, HEIGHT) {
        return Err("normal dashboard".into());
    }
    let views = [
        fader_icon(FaderGlyph::Mic, false).1,
        fader_icon(FaderGlyph::Mic, true).1,
        fader_icon(FaderGlyph::Speaker, false).1,
        fader_icon(FaderGlyph::Speaker, true).1,
    ];
    if views.iter().any(|view| (*view - FADER_ICON_VIEW).abs() > 0.01) {
        return Err("icon view".into());
    }
    let quiet = level_color(0.0);
    let loud = level_color(1.0);
    if (quiet.b - BLUE.b).abs() > 0.01 || (loud.r - PINK.r).abs() > 0.01 {
        return Err("meter gradient".into());
    }
    record_dot(&cr, 8.0, 8.0, 3.0, DANGER);
    stop_square(&cr, 8.0, 8.0, 6.0, TEXT);
    play_triangle(&cr, 8.0, 8.0, 8.0, TEXT);
    if let Ok(fonts) = Fonts::load() {
        let geom = paint_fader(&cr, &fonts, 8.0, 0.0, 40.0, 100.0, 0.2, true, false, FaderGlyph::Mic, true);
        if geom.capsule_w != 32.0 {
            return Err("fader width".into());
        }
    }
    Ok(())
}
