//! Fonts and text layout: labels, baseline helpers, fitting and wrapping.

use cairo::{Context, FontFace};
use freetype::Library as FtLibrary;

use super::palette::Rgb;

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
        // "Bold" in the panel is Medium. Titles and labels stay firm without the weight of Bold.
        let bold_path = find_font(&[
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Medium.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Medium.ttc",
            "/usr/share/fonts/noto/NotoSans-Medium.ttf",
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

pub fn set_font(cr: &Context, fonts: &Fonts, bold: bool, size: f64) {
    cr.set_font_face(if bold { &fonts.bold } else { &fonts.regular });
    cr.set_font_size(size);
}

pub fn text_width(cr: &Context, fonts: &Fonts, bold: bool, size: f64, text: &str) -> f64 {
    set_font(cr, fonts, bold, size);
    cr.text_extents(text).map(|e| e.x_advance()).unwrap_or(0.0)
}

pub fn label(cr: &Context, fonts: &Fonts, bold: bool, size: f64, c: Rgb, x: f64, baseline: f64, text: &str) {
    set_font(cr, fonts, bold, size);
    cr.set_source_rgb(c.r, c.g, c.b);
    cr.move_to(x, baseline);
    let _ = cr.show_text(text);
}

/// Text centered on `cy`, for a single line inside a control.
pub fn label_mid(cr: &Context, fonts: &Fonts, bold: bool, size: f64, c: Rgb, x: f64, cy: f64, text: &str) {
    label(cr, fonts, bold, size, c, x, cy + size * 0.36, text);
}

/// `y` is the vertical center of the text, so a number sits on a meter tick.
pub(super) fn label_at(cr: &Context, fonts: &Fonts, size: f64, c: Rgb, x: f64, y: f64, text: &str) {
    set_font(cr, fonts, false, size);
    let baseline = cr.text_extents(text).ok().map(|ext| y - ext.y_bearing() - ext.height() / 2.0).unwrap_or(y + size * 0.3);
    label(cr, fonts, false, size, c, x, baseline, text);
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

/// Draw the end of `text` inside `max_w`. A long name keeps the characters being typed.
pub fn label_tail(cr: &Context, fonts: &Fonts, bold: bool, size: f64, c: Rgb, x: f64, baseline: f64, max_w: f64, text: &str) {
    let shown = fit_tail(cr, fonts, bold, size, max_w, text);
    label(cr, fonts, bold, size, c, x, baseline, &shown);
}

/// The end of `text` that fits in `max_w`, with a leading ellipsis when the start is cut.
pub fn fit_tail(cr: &Context, fonts: &Fonts, bold: bool, size: f64, max_w: f64, text: &str) -> String {
    if text.is_empty() || max_w <= 0.0 || text_width(cr, fonts, bold, size, text) <= max_w {
        return text.to_string();
    }
    let mut kept: Vec<char> = text.chars().collect();
    while kept.len() > 1 {
        let shown: String = std::iter::once('…').chain(kept.iter().copied()).collect();
        if text_width(cr, fonts, bold, size, &shown) <= max_w {
            break;
        }
        kept.remove(0);
    }
    std::iter::once('…').chain(kept.iter().copied()).collect()
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
