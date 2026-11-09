//! Design tokens: the tonal surfaces, text colors, accent theme and radii.
//! Flat tonal surfaces — each nested layer is one step lighter, so nothing
//! needs an outline.

use std::cell::Cell;

#[derive(Clone, Copy)]
pub struct Rgb {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

/// `Rgb` from the usual 8-bit hex components.
const fn rgb8(r: u8, g: u8, b: u8) -> Rgb {
    Rgb { r: r as f64 / 255.0, g: g as f64 / 255.0, b: b as f64 / 255.0 }
}

pub const BG: Rgb = rgb8(0x0e, 0x10, 0x15);
pub const CARD: Rgb = rgb8(0x17, 0x1a, 0x21);
pub const INSET: Rgb = rgb8(0x1f, 0x23, 0x2b);
/// Card, strip, control, button.
pub const R_CARD: f64 = 20.0;
pub const R_SURFACE: f64 = 16.0;
pub const R_CONTROL: f64 = 12.0;
pub const TEXT: Rgb = rgb8(0xf2, 0xf3, 0xf5);
pub const MUTED: Rgb = rgb8(0xa3, 0xa9, 0xb4);
pub const SOFT: Rgb = rgb8(0xdf, 0xe2, 0xe7);
pub const BORDER: Rgb = rgb8(0x6e, 0x76, 0x81);
const PINK: Rgb = rgb8(0xe0, 0x7e, 0xf5);
const PINK_HOVER: Rgb = rgb8(0xe9, 0x9a, 0xf8);
const PINK_DOWN: Rgb = rgb8(0xc8, 0x65, 0xde);
const PINK_INK: Rgb = rgb8(0x1c, 0x0f, 0x22);
/// #33C4FF, the blue in the logo.
const BLUE: Rgb = rgb8(0x33, 0xc4, 0xff);
const BLUE_HOVER: Rgb = rgb8(0x66, 0xd4, 0xff);
const BLUE_DOWN: Rgb = rgb8(0x1a, 0xa8, 0xe0);
const BLUE_INK: Rgb = rgb8(0x0e, 0x10, 0x15);

pub const DANGER: Rgb = rgb8(0xe5, 0x53, 0x4b);
pub const FX1: Rgb = rgb8(0x7f, 0xd8, 0xa8);
pub const FX2: Rgb = rgb8(0xf0, 0xb4, 0x5a);
pub const DANGER_FILL: Rgb = rgb8(0x3a, 0x1d, 0x21);
pub const DANGER_HOVER: Rgb = rgb8(0x48, 0x23, 0x28);
pub const DANGER_DOWN: Rgb = rgb8(0x56, 0x29, 0x2e);
pub const DANGER_TEXT: Rgb = rgb8(0xff, 0x8a, 0x80);
pub const METER_GREEN_LO: Rgb = rgb8(0x1e, 0x6b, 0x45);
pub const METER_GREEN_HI: Rgb = rgb8(0x3d, 0xdc, 0x97);
pub const METER_YELLOW: Rgb = rgb8(0xf0, 0xc1, 0x4a);
pub const METER_RED: Rgb = rgb8(0xff, 0x4b, 0x5c);
pub const FX_REVERB: Rgb = rgb8(0x3b, 0x6f, 0xe0);
pub const FX_COMP: Rgb = rgb8(0xff, 0x4b, 0x5c);
pub const FX_EQ: Rgb = rgb8(0x2e, 0xc4, 0xb6);
pub const FX_DELAY: Rgb = rgb8(0x3d, 0xdc, 0x97);
pub const FX_PHASER: Rgb = rgb8(0xa7, 0x8b, 0xfa);
pub const FX_PITCHER: Rgb = rgb8(0xf5, 0xa5, 0x24);
pub const FX_FLANGER: Rgb = rgb8(0xe0, 0x7e, 0xf5);
pub const FX_VST: Rgb = rgb8(0xc8, 0xcd, 0xd6);
pub const CONTROL: Rgb = rgb8(0x28, 0x2d, 0x36);
pub const CONTROL_HOVER: Rgb = rgb8(0x31, 0x37, 0x42);
pub const CONTROL_DOWN: Rgb = rgb8(0x3b, 0x41, 0x4d);
pub const LABEL_SOFT: Rgb = rgb8(0xc9, 0xcd, 0xd4);
pub const DIVIDER: Rgb = rgb8(0x2c, 0x31, 0x3a);
pub const CHIP_OK_FILL: Rgb = rgb8(0x17, 0x34, 0x24);
pub const CHIP_OK_TEXT: Rgb = rgb8(0x4a, 0xde, 0x80);

thread_local! {
    static THEME_PINK: Cell<bool> = const { Cell::new(false) };
}

/// `pink` selects the previous accent. Anything else is the blue logo color.
pub fn set_accent(name: &str) {
    THEME_PINK.with(|cell| cell.set(name == "pink"));
}

pub fn accent() -> Rgb {
    if THEME_PINK.with(Cell::get) { PINK } else { BLUE }
}

pub fn accent_hover() -> Rgb {
    if THEME_PINK.with(Cell::get) { PINK_HOVER } else { BLUE_HOVER }
}

pub fn accent_down() -> Rgb {
    if THEME_PINK.with(Cell::get) { PINK_DOWN } else { BLUE_DOWN }
}

pub fn on_accent() -> Rgb {
    if THEME_PINK.with(Cell::get) { PINK_INK } else { BLUE_INK }
}

/// `a` laid over `b` at `amount`, as one opaque color.
pub fn mix(a: Rgb, b: Rgb, amount: f64) -> Rgb {
    Rgb { r: b.r + (a.r - b.r) * amount, g: b.g + (a.g - b.g) * amount, b: b.b + (a.b - b.b) * amount }
}

/// The unfilled part of a fader. Tinted with the accent so it reads as part of the control.
pub fn track() -> Rgb {
    mix(accent(), INSET, 0.2)
}

/// Selected rows and toggles that are on but not primary.
pub fn accent_tint() -> Rgb {
    mix(accent(), INSET, 0.16)
}

pub fn ink_on(fill: Rgb) -> Rgb {
    if fill.r > fill.b { PINK_INK } else { BLUE_INK }
}

pub fn effect_color(kind: crate::config::EffectKind) -> Rgb {
    match kind {
        crate::config::EffectKind::Reverb => FX_REVERB,
        crate::config::EffectKind::Comp => FX_COMP,
        crate::config::EffectKind::Eq => FX_EQ,
        crate::config::EffectKind::Pitcher => FX_PITCHER,
        crate::config::EffectKind::Flanger => FX_FLANGER,
        crate::config::EffectKind::Phaser => FX_PHASER,
        crate::config::EffectKind::Delay => FX_DELAY,
        crate::config::EffectKind::Vst => FX_VST,
    }
}

/// Post-fader amplitude 0..1.5 (1.0 is 0 dB / 100%).
pub fn meter_color(amplitude: f64) -> Rgb {
    if amplitude >= 1.0 {
        METER_RED
    } else if amplitude >= 0.8 {
        METER_YELLOW
    } else {
        mix(METER_GREEN_HI, METER_GREEN_LO, (amplitude / 0.8).clamp(0.0, 1.0))
    }
}

/// Color of a meter reading. `fraction` is 0 at silence and 1 at 0 dB.
pub fn level_color(fraction: f64) -> Rgb {
    meter_color(fraction.clamp(0.0, 1.0) * 1.5)
}
