//! The panel's controls: buttons, scrollbars, LED meters and the two fader styles.

use cairo::Context;
use std::f64::consts::PI;

use super::icons::{FADER_ICON_VIEW, MIC_OFF_PATH, MIC_PATH, SPEAKER_OFF_PATH, SPEAKER_PATH};
use super::palette::*;
use super::shape::{fill, rounded};
use super::svg::icon;
use super::text::{label, label_at, label_mid, text_width, Fonts};

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

/// Thumb length along a scrollbar. The minimum never exceeds the track, so a short section cannot panic.
pub fn scroll_thumb(track: f64, content: f64, min_px: f64) -> f64 {
    if !track.is_finite() || track <= 1.0 {
        return 0.0;
    }
    if !content.is_finite() || content <= track {
        return track;
    }
    let min_px = min_px.min(track).max(0.0);
    (track * track / content).clamp(min_px, track)
}

/// The thin scrollbar both sections use. The handle is the accent color.
pub fn paint_scroll_thumb(cr: &Context, x: f64, y: f64, w: f64, h: f64) {
    if w < 1.0 || h < 1.0 {
        return;
    }
    rounded(cr, x, y, w, h, (w.min(h) / 2.0).max(1.0));
    fill(cr, accent(), 1.0);
}

/// Flat pill buttons: tonal for Plain and Danger, filled for Accent and DangerArmed.
/// `look` is Rest unless the pointer is over the button.
pub fn button(cr: &Context, fonts: &Fonts, x: f64, y: f64, w: f64, h: f64, caption: &str, kind: ButtonKind, look: Look) {
    let (fill_c, _border, text_c) = match kind {
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
            (fill_c, DANGER, DANGER_TEXT)
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
    rounded(cr, x, y, w, h, h / 2.0);
    fill(cr, fill_c, 1.0);
    if !caption.is_empty() {
        let tw = text_width(cr, fonts, true, 15.0, caption);
        label_mid(cr, fonts, true, 15.0, text_c, x + (w - tw) / 2.0, y + h / 2.0, caption);
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

const LED_H: f64 = 3.0;
const LED_GAP: f64 = 1.0;
const METER_TICKS: [(f64, &str); 6] = [(6.0, "+6"), (0.0, "0"), (-6.0, "−6"), (-12.0, "−12"), (-24.0, "−24"), (-48.0, "−48")];

fn signal_db(amplitude: f64) -> f64 {
    if amplitude <= 1e-8 { -120.0 } else { 20.0 * amplitude.max(0.0).log10() }
}

/// Same travel as the fader, so a mark and the number of that level share one position.
pub(super) fn db_pos(db: f64) -> f64 {
    crate::config::db_to_t(db)
}

fn led_db(t: f64) -> f64 {
    let db = crate::config::t_to_db(t);
    if db.is_finite() { db } else { crate::config::FADER_DB_MIN }
}

/// Green below −6 dB, yellow through 0 dB, red above.
fn led_color(db: f64) -> Rgb {
    if db > 0.0 {
        METER_RED
    } else if db >= -6.0 {
        METER_YELLOW
    } else {
        let t = ((db - crate::config::FADER_DB_MIN) / (-6.0 - crate::config::FADER_DB_MIN)).clamp(0.0, 1.0);
        mix(METER_GREEN_LO, METER_GREEN_HI, t)
    }
}

fn led_lit(amplitude: f64, db: f64) -> bool {
    signal_db(amplitude) + 0.4 >= db
}

/// Segmented peak column. `amplitude` is linear gain, 1.0 at 0 dB.
/// The scale is −48 dB to +6 dB. Green below −6 dB, yellow to 0 dB, red above.
pub fn paint_led_column(cr: &Context, fonts: &Fonts, x: f64, y: f64, w: f64, h: f64, amplitude: f64, labels: bool) -> f64 {
    if w <= 0.0 || h <= 0.0 {
        return x;
    }
    rounded(cr, x, y, w, h, 2.0);
    fill(cr, BG, 1.0);
    let step = LED_H + LED_GAP;
    let n = ((h + LED_GAP) / step).floor().max(1.0) as usize;
    for i in 0..n {
        let t = (i as f64 + 0.5) / n as f64;
        let db = led_db(t);
        let ly = y + h - (i as f64 + 1.0) * step + LED_GAP;
        let color = if led_lit(amplitude, db) { led_color(db) } else { mix(TEXT, BG, 0.12) };
        rounded(cr, x + 1.0, ly, w - 2.0, LED_H, 0.8);
        fill(cr, color, 1.0);
    }
    if !labels {
        return x + w;
    }
    let label_x = x + w + 4.0;
    let mut right = label_x;
    for (db, text) in METER_TICKS {
        let ty = y + h - db_pos(db) * h;
        cr.set_source_rgba(MUTED.r, MUTED.g, MUTED.b, 0.7);
        cr.set_line_width(1.0);
        cr.move_to(x + w, ty);
        cr.line_to(x + w + 3.0, ty);
        let _ = cr.stroke();
        let tw = text_width(cr, fonts, false, 10.0, text);
        label_at(cr, fonts, 10.0, MUTED, label_x, ty, text);
        right = right.max(label_x + tw);
    }
    right
}

/// One column per channel: a single 12 px column, or a 7 px pair for stereo.
/// Returns the right edge of the scale labels.
pub fn paint_led_columns(cr: &Context, fonts: &Fonts, x: f64, y: f64, h: f64, levels: &[f32]) -> f64 {
    let (w, gap) = if levels.len() > 1 { (STEREO_LED_W, 2.0) } else { (MONO_LED_W, 0.0) };
    let mut right = x;
    for (index, level) in levels.iter().enumerate() {
        let last = index + 1 == levels.len();
        right = paint_led_column(cr, fonts, x + index as f64 * (w + gap), y, w, h, *level as f64, last);
    }
    right
}

const MONO_LED_W: f64 = 12.0;
const STEREO_LED_W: f64 = 7.0;

/// Short LED strip along a side fader. The fader draws 0 and −12 on its own marks.
pub fn paint_led_strip(cr: &Context, x: f64, y: f64, w: f64, h: f64, amplitude: f64) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    rounded(cr, x, y, w, h, h / 2.0);
    fill(cr, BG, 1.0);
    let step = LED_H + LED_GAP;
    let n = ((w + LED_GAP) / step).floor().max(1.0) as usize;
    for i in 0..n {
        let t = (i as f64 + 0.5) / n as f64;
        let db = led_db(t);
        let lx = x + i as f64 * step;
        let color = if led_lit(amplitude, db) { led_color(db) } else { mix(TEXT, BG, 0.12) };
        rounded(cr, lx, y + 1.0, LED_H, h - 2.0, 0.8);
        fill(cr, color, 1.0);
    }
}

/// Horizontal LED meter for Mix Check.
pub fn paint_led_bar(cr: &Context, x: f64, y: f64, w: f64, h: f64, amplitude: f64) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    rounded(cr, x, y, w, h, h / 2.0);
    fill(cr, BG, 1.0);
    let step = 4.0;
    let n = ((w + 1.0) / step).floor().max(1.0) as usize;
    for i in 0..n {
        let t = (i as f64 + 0.5) / n as f64;
        let db = led_db(t);
        let lx = x + i as f64 * step;
        let color = if led_lit(amplitude, db) { led_color(db) } else { mix(TEXT, BG, 0.12) };
        rounded(cr, lx + 0.5, y + 1.0, 3.0, h - 2.0, 1.0);
        fill(cr, color, 1.0);
    }
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
}

/// Below a vertical fader: percent baseline, path icon, then Mute/Solo in mix.rs.
const FADER_PCT_Y: f64 = 16.0;
const FADER_ICON_Y: f64 = 36.0;
pub const FADER_MUTE_Y: f64 = 48.0;
pub const FADER_MUTE_H: f64 = 28.0;
/// Room a vertical fader needs under its capsule, including Mute and Solo.
pub const FADER_FOOT: f64 = 80.0;

/// One mix fader scaled from silence to +12 dB. `db` is the strip gain.
/// `levels` are post-fader linear peaks, one per meter column (one, or left and right).
pub fn paint_fader(
    cr: &Context,
    fonts: &Fonts,
    cx: f64,
    top: f64,
    cap_h: f64,
    db: f64,
    levels: &[f32],
    meters: bool,
    muted: bool,
    glyph: FaderGlyph,
    snaps: bool,
) -> FaderGeom {
    let cap_w = 32.0;
    let x = cx - cap_w / 2.0;
    let radius = cap_w / 2.0;
    let fraction = crate::config::db_to_t(db);
    let _ = cr.save();
    rounded(cr, x, top, cap_w, cap_h, radius);
    cr.clip();
    rounded(cr, x, top, cap_w, cap_h, 0.0);
    fill(cr, track(), 1.0);
    let fill_top = top + cap_h - cap_h * fraction;
    if top + cap_h - fill_top > 0.5 {
        rounded(cr, x, fill_top, cap_w, top + cap_h - fill_top, 0.0);
        fill(cr, accent(), 1.0);
    }
    let _ = cr.restore();
    if snaps {
        for mark in [6.0, -6.0, -12.0, -24.0, -48.0] {
            let dot_y = top + cap_h - cap_h * crate::config::db_to_t(mark);
            let filled = db.is_finite() && db + 0.05 >= mark;
            let (c, a) = if filled { (on_accent(), 0.6) } else { (TEXT, 0.45) };
            cr.arc(cx, dot_y, 2.5, 0.0, 2.0 * PI);
            fill(cr, c, a);
        }
    }
    let notch_y = top + cap_h - cap_h * crate::config::db_to_t(0.0);
    let notch_c = if db.is_finite() && db >= -0.05 { on_accent() } else { TEXT };
    cr.set_source_rgba(notch_c.r, notch_c.g, notch_c.b, 0.85);
    cr.set_line_width(2.0);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.move_to(x + 7.0, notch_y);
    cr.line_to(x + cap_w - 7.0, notch_y);
    let _ = cr.stroke();

    let meter_right = if meters {
        let silent = muted || !db.is_finite();
        let shown: Vec<f32> = levels.iter().map(|level| if silent { 0.0 } else { *level }).collect();
        paint_led_columns(cr, fonts, x + cap_w + 4.0, top, cap_h, &shown)
    } else {
        x + cap_w
    };

    let pct = crate::config::format_db(db);
    let pct_color = if db.abs() < 0.05 { accent() } else { TEXT };
    let tw = text_width(cr, fonts, true, 13.0, &pct);
    label(cr, fonts, true, 13.0, pct_color, cx - tw / 2.0, top + cap_h + FADER_PCT_Y, &pct);

    let icon_s = 22.0;
    let icon_x = cx - icon_s / 2.0;
    let icon_y = top + cap_h + FADER_ICON_Y - icon_s / 2.0;
    paint_fader_glyph(cr, glyph, muted, icon_x, icon_y, icon_s);
    FaderGeom { capsule_x: x, capsule_y: top, capsule_w: cap_w, capsule_h: cap_h, meter_right }
}

/// A sideways fader. The 0 dB mark stays a line.
pub fn paint_side_fader(
    cr: &Context,
    fonts: &Fonts,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    db: f64,
    levels: &[f32],
    meters: bool,
    muted: bool,
    glyph: FaderGlyph,
    snaps: bool,
) -> FaderGeom {
    let track_h = 24.0;
    let icon_box = 28.0;
    let icon_s = 22.0;
    let pct_w = 64.0;
    let track_w = (w - icon_box - pct_w - 12.0).max(36.0);
    let track_x = x + icon_box + 8.0;
    let track_y = y + (h - track_h) / 2.0;
    let radius = track_h / 2.0;
    let fraction = crate::config::db_to_t(db);
    let _ = cr.save();
    rounded(cr, track_x, track_y, track_w, track_h, radius);
    cr.clip();
    rounded(cr, track_x, track_y, track_w, track_h, 0.0);
    fill(cr, track(), 1.0);
    if track_w * fraction > 0.5 {
        rounded(cr, track_x, track_y, track_w * fraction, track_h, 0.0);
        fill(cr, accent(), 1.0);
    }
    let _ = cr.restore();
    if snaps {
        for mark in [6.0, -6.0, -12.0, -24.0, -48.0] {
            let dot_x = track_x + track_w * crate::config::db_to_t(mark);
            let filled = db.is_finite() && db + 0.05 >= mark;
            let (c, a) = if filled { (on_accent(), 0.6) } else { (TEXT, 0.45) };
            cr.arc(dot_x, track_y + track_h / 2.0, 2.5, 0.0, 2.0 * PI);
            fill(cr, c, a);
        }
    }
    let notch_x = track_x + track_w * crate::config::db_to_t(0.0);
    let notch_c = if db.is_finite() && db >= -0.05 { on_accent() } else { TEXT };
    cr.set_source_rgba(notch_c.r, notch_c.g, notch_c.b, 0.85);
    cr.set_line_width(2.0);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.move_to(notch_x, track_y + 6.0);
    cr.line_to(notch_x, track_y + track_h - 6.0);
    let _ = cr.stroke();

    if meters && !levels.is_empty() {
        let silent = muted || !db.is_finite();
        let my = (track_y - 10.0).max(y);
        let strip_h = if levels.len() > 1 { 4.0 } else { 5.0 };
        for (index, level) in levels.iter().enumerate() {
            let amplitude = if silent { 0.0 } else { *level as f64 };
            // The full track width, so a tick lands on the fader mark of the same level.
            paint_led_strip(cr, track_x, my + index as f64 * (strip_h + 1.0), track_w, strip_h, amplitude);
        }
    }
    // The same marks as the vertical meter: +6, 0, −6, −12, −24, −48.
    for (db, text) in METER_TICKS {
        let tx = track_x + track_w * crate::config::db_to_t(db);
        let tw = text_width(cr, fonts, false, 9.0, text);
        label_at(cr, fonts, 9.0, MUTED, tx - tw / 2.0, track_y - 14.0, text);
    }

    let pct = crate::config::format_db(db);
    let pct_color = if db.abs() < 0.05 { accent() } else { TEXT };
    let pw = text_width(cr, fonts, true, 13.0, &pct);
    label_mid(cr, fonts, true, 13.0, pct_color, x + w - pw, track_y + track_h / 2.0, &pct);

    let icon_x = x + (icon_box - icon_s) / 2.0;
    let icon_y = y + (h - icon_s) / 2.0;
    paint_fader_glyph(cr, glyph, muted, icon_x, icon_y, icon_s);
    FaderGeom {
        capsule_x: track_x,
        capsule_y: track_y,
        capsule_w: track_w,
        capsule_h: track_h,
        meter_right: track_x + track_w,
    }
}
