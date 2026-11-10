//! App icons. Each PNG is decoded once and kept as a cairo surface; an app
//! without an icon gets a tile with its letter.

use super::ui::Ui;
use super::Panel;
use crate::draw::{self, Font, Rect};
use cairo::ImageSurface;

/// Icon of a bundled app: the file the system has, the embedded copy, or a letter tile.
/// Returns false when it fell back to the tile.
pub(crate) fn app_icon(panel: &mut Panel, ui: &Ui, name: &str, mono: &str, icon_path: &str, x: f64, y: f64, size: f64) -> bool {
    if !icon_path.is_empty() && file_icon(panel, ui, icon_path, x, y, size) {
        return true;
    }
    if let Some((bytes, _)) = crate::apps::embedded_png(name) {
        let key = format!("embedded:{name}");
        if !panel.pngs.contains_key(&key) {
            if let Ok(surface) = ImageSurface::create_from_png(&mut std::io::Cursor::new(bytes)) {
                panel.pngs.insert(key.clone(), surface);
            }
        }
        if paint_cached(ui, panel.pngs.get(&key), x, y, size) {
            return true;
        }
    }
    let letters: String = mono.chars().take(4).collect();
    letter_tile(ui, &letters, x, y, size, 0.22, Font::bold(size * 0.28), 0.62);
    false
}

/// Running-app icon from the desktop index, or a tile with the app's first letter.
pub(crate) fn desktop_icon(panel: &mut Panel, ui: &Ui, app: &str, binary: &str, x: f64, y: f64, size: f64) {
    let path = panel.icons.as_mut().and_then(|index| index.lookup(app, binary));
    if let Some(path) = path {
        let path = path.to_string_lossy().to_string();
        if file_icon(panel, ui, &path, x, y, size) {
            return;
        }
    }
    let letter: String = app.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".into());
    letter_tile(ui, &letter, x, y, size, 0.25, Font::bold(size * 0.5), 0.68);
}

/// A rounded tile in the accent tint with centered text. `radius` and `baseline` are fractions of `size`.
fn letter_tile(ui: &Ui, text: &str, x: f64, y: f64, size: f64, radius: f64, font: Font, baseline: f64) {
    ui.fill(Rect::square(x, y, size), size * radius, draw::accent_tint());
    let width = ui.text_width(font, text);
    ui.label(font, draw::accent(), x + (size - width) / 2.0, y + size * baseline, text);
}

/// Paints the PNG at `path`, loading it on first use.
fn file_icon(panel: &mut Panel, ui: &Ui, path: &str, x: f64, y: f64, size: f64) -> bool {
    if !panel.pngs.contains_key(path) {
        let Ok(mut file) = std::fs::File::open(path) else { return false };
        let Ok(surface) = ImageSurface::create_from_png(&mut file) else { return false };
        panel.pngs.insert(path.to_string(), surface);
    }
    paint_cached(ui, panel.pngs.get(path), x, y, size)
}

fn paint_cached(ui: &Ui, surface: Option<&ImageSurface>, x: f64, y: f64, size: f64) -> bool {
    let Some(surface) = surface else { return false };
    let cr = ui.cr;
    let _ = cr.save();
    cr.translate(x, y);
    let w = surface.width() as f64;
    if w > 0.0 {
        cr.scale(size / w, size / w);
    }
    cr.set_source_surface(surface, 0.0, 0.0).ok();
    let _ = cr.paint();
    let _ = cr.restore();
    true
}
