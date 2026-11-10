//! Cairo drawing. Colors match Frame Mic Tuner's panel so the two apps sit together.
//!
//! The module is split by concern:
//! - [`geom`]: `Rect`, the rectangle every layout is expressed in.
//! - [`palette`]: design tokens — colors, radii, the accent theme.
//! - [`text`]: fonts, text styles and text layout (labels, fitting, wrapping).
//! - [`shape`]: path and surface primitives (rounded rects, fills, PNG paint).
//! - [`widgets`]: the panel's controls — buttons, faders, LED meters, scrollbars.
//! - [`svg`]: the tiny SVG path parser that renders the embedded icon data.
//! - [`icons`]: the icon path data and the composed device glyphs.
//!
//! Everything is re-exported here, so callers keep writing `draw::X`.

mod geom;
mod icons;
mod palette;
mod shape;
mod svg;
mod text;
mod widgets;

pub use geom::Rect;
pub use icons::*;
pub use palette::*;
pub use shape::*;
pub use svg::icon;
pub use text::*;
pub use widgets::*;

use cairo::{Context, Format, ImageSurface};

pub const WIDTH: i32 = 1200;
pub const HEIGHT: i32 = 788;

/// Dashboard pixels. Extend is 21:9. With the channel list it is 9:10,
/// about as wide as the normal panel so Mix Check fits beside the sliders.
pub fn panel_pixels(wide: bool, list: bool) -> (i32, i32) {
    if !wide {
        return (WIDTH, HEIGHT);
    }
    if !list {
        let long = (HEIGHT as f64 * 21.0 / 9.0).round() as i32;
        return (long, HEIGHT);
    }
    let unit = (WIDTH as f64 / 9.0).round().max(1.0) as i32;
    (unit * 9, unit * 10)
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
        (MAGNET_PATH, 16.0),
        (LIST_PATH, 512.0),
        (COLUMNS_PATH, 512.0),
        (MAXIMIZE_PATH, 16.0),
        (MINIMIZE_PATH, 16.0),
        (SETTINGS_PATH, 24.0),
        (CLOSE_PATH, 1024.0),
        (EAR_PATH, 24.0),
        (FX_TAG_PATH, 24.0),
        (HEADSET_VR_PATH, 16.0),
        (BLUETOOTH_PATH, 24.0),
        (FLOW_PATH, 24.0),
        (FOLDER_PATH, 24.0),
    ] {
        icon(&cr, path, view, 0.0, 0.0, 16.0, TEXT);
    }
    for (name, path, view) in [
        ("headset", HEADSET_VR_PATH, 16.0),
        ("bluetooth", BLUETOOTH_PATH, 24.0),
        ("flow", FLOW_PATH, 24.0),
    ] {
        let ink = svg::icon_coverage(path, view)?;
        if ink < 300 {
            return Err(format!("{name} icon coverage {ink}"));
        }
    }
    if Device::of_sink("bluez_output.AA_BB.1") != Device::Bluetooth
        || Device::of_sink("alsa_output.usb-Generic_USB_Audio-00.analog-stereo") != Device::Usb
        || Device::of_sink("alsa_output.platform-sound.HiFi__Speaker__sink") != Device::Headset
    {
        return Err("device kinds".into());
    }
    let pc_ink = {
        let surface = ImageSurface::create(Format::ARgb32, 64, 64).map_err(|e| e.to_string())?;
        {
            let pc = Context::new(&surface).map_err(|e| e.to_string())?;
            pc_icon(&pc, 0.0, 0.0, 64.0, TEXT);
        }
        let mut surface = surface;
        surface_to_rgba(&mut surface).chunks(4).filter(|px| px[3] > 160).count()
    };
    if pc_ink < 300 {
        return Err(format!("pc icon coverage {pc_ink}"));
    }
    // fa6-solid:list-ul uses smooth curves. A dropped `s` leaves only the first bullet.
    let list_ink = svg::icon_coverage(LIST_PATH, 512.0)?;
    if list_ink < 400 {
        return Err(format!("list icon coverage {list_ink}"));
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
    if list_w * 10 != list_h * 9 || list_h <= list_w || list_w + 8 < WIDTH {
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
    let quiet = meter_color(0.2);
    let clip = meter_color(1.2);
    if (quiet.g - METER_GREEN_LO.g).abs() > 0.2 || (clip.r - METER_RED.r).abs() > 0.01 {
        return Err("meter colors".into());
    }
    if (meter_color(0.9).r - METER_YELLOW.r).abs() > 0.01 {
        return Err("meter yellow".into());
    }
    for mark in [12.0, 6.0, 3.0, 0.0, -6.0, -12.0, -24.0, -48.0] {
        if (widgets::db_pos(mark) - crate::config::db_to_t(mark)).abs() > 1e-9 {
            return Err(format!("meter mark {mark}"));
        }
    }
    record_dot(&cr, 8.0, 8.0, 3.0, DANGER);
    stop_square(&cr, 8.0, 8.0, 6.0, TEXT);
    play_triangle(&cr, 8.0, 8.0, 8.0, TEXT);
    if let Ok(fonts) = Fonts::load() {
        let fader = Fader { db: 100.0, levels: &[0.2, 0.1], meters: true, muted: false, glyph: FaderGlyph::Mic, snaps: true };
        let geom = paint_fader(&cr, &fonts, 8.0, 0.0, 40.0, &fader);
        if geom.capsule.w != 32.0 {
            return Err("fader width".into());
        }
    }
    Ok(())
}
