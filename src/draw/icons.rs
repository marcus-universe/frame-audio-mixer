//! Icon path data and composed device glyphs.
//!
//! Paths are copied from Iconify and stored here. Nothing is fetched at runtime.
//! Licenses are in NOTICE. The fader pair shares one view box so mute does not
//! rescale the glyph.

use cairo::Context;
use std::f64::consts::PI;

use super::palette::Rgb;
use super::shape::{fill, rounded};
use super::svg::{append_svg, icon};

/// Phosphor microphone, microphone-slash, speaker-high and speaker-slash. View box 256.
pub const FADER_ICON_VIEW: f64 = 256.0;
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
/// at-icons:magnet, view box 16, MIT, Valentin Fossati.
pub const MAGNET_PATH: &str = "M14 9A6 6 0 0 1 2 9V5h4v4a2 2 0 1 0 4 0V5h4zM5 1a1 1 0 0 1 1 1v2H2V2a1 1 0 0 1 1-1zm8 0a1 1 0 0 1 1 1v2h-4V2a1 1 0 0 1 1-1z";
/// fa6-solid:list-ul, view box 512, CC BY 4.0, Font Awesome.
pub const LIST_PATH: &str = "M64 144a48 48 0 1 0 0-96a48 48 0 1 0 0 96m128-80c-17.7 0-32 14.3-32 32s14.3 32 32 32h288c17.7 0 32-14.3 32-32s-14.3-32-32-32zm0 160c-17.7 0-32 14.3-32 32s14.3 32 32 32h288c17.7 0 32-14.3 32-32s-14.3-32-32-32zm0 160c-17.7 0-32 14.3-32 32s14.3 32 32 32h288c17.7 0 32-14.3 32-32s-14.3-32-32-32zM64 464a48 48 0 1 0 0-96a48 48 0 1 0 0 96m48-208a48 48 0 1 0-96 0a48 48 0 1 0 96 0";
/// fa-solid:columns, view box 512, CC BY 4.0, Font Awesome.
pub const COLUMNS_PATH: &str = "M464 32H48C21.49 32 0 53.49 0 80v352c0 26.51 21.49 48 48 48h416c26.51 0 48-21.49 48-48V80c0-26.51-21.49-48-48-48M224 416H64V160h160zm224 0H288V160h160z";
/// fluent:full-screen-maximize-16-filled, view box 16, MIT, Microsoft. Widens the window.
pub const MAXIMIZE_PATH: &str = "M4 3.5a.5.5 0 0 0-.5.5v1.614a.75.75 0 0 1-1.5 0V4a2 2 0 0 1 2-2h1.614a.75.75 0 0 1 0 1.5zm5.636-.75a.75.75 0 0 1 .75-.75H12a2 2 0 0 1 2 2v1.614a.75.75 0 0 1-1.5 0V4a.5.5 0 0 0-.5-.5h-1.614a.75.75 0 0 1-.75-.75M2.75 9.636a.75.75 0 0 1 .75.75V12a.5.5 0 0 0 .5.5h1.614a.75.75 0 0 1 0 1.5H4a2 2 0 0 1-2-2v-1.614a.75.75 0 0 1 .75-.75m10.5 0a.75.75 0 0 1 .75.75V12a2 2 0 0 1-2 2h-1.614a.75.75 0 1 1 0-1.5H12a.5.5 0 0 0 .5-.5v-1.614a.75.75 0 0 1 .75-.75";
/// fluent:full-screen-minimize-16-filled, view box 16, MIT, Microsoft. Back to the normal window.
pub const MINIMIZE_PATH: &str = "M11.5 4a.5.5 0 0 0 .5.5h1.75a.75.75 0 0 1 0 1.5H12a2 2 0 0 1-2-2V2.25a.75.75 0 0 1 1.5 0zm0 8a.5.5 0 0 1 .5-.5h1.75a.75.75 0 0 0 0-1.5H12a2 2 0 0 0-2 2v1.75a.75.75 0 0 0 1.5 0zM4 11.5a.5.5 0 0 1 .5.5v1.75a.75.75 0 0 0 1.5 0V12a2 2 0 0 0-2-2H2.25a.75.75 0 0 0 0 1.5zM4.5 4a.5.5 0 0 1-.5.5H2.25a.75.75 0 0 0 0 1.5H4a2 2 0 0 0 2-2V2.25a.75.75 0 0 0-1.5 0z";
/// material-symbols:settings-rounded, view box 24, Apache-2.0, Google.
pub const SETTINGS_PATH: &str = "M10.825 22q-.675 0-1.162-.45t-.588-1.1L8.85 18.8q-.325-.125-.612-.3t-.563-.375l-1.55.65q-.625.275-1.25.05t-.975-.8l-1.175-2.05q-.35-.575-.2-1.225t.675-1.075l1.325-1Q4.5 12.5 4.5 12.337v-.675q0-.162.025-.337l-1.325-1Q2.675 9.9 2.525 9.25t.2-1.225L3.9 5.975q.35-.575.975-.8t1.25.05l1.55.65q.275-.2.575-.375t.6-.3l.225-1.65q.1-.65.588-1.1T10.825 2h2.35q.675 0 1.163.45t.587 1.1l.225 1.65q.325.125.613.3t.562.375l1.55-.65q.625-.275 1.25-.05t.975.8l1.175 2.05q.35.575.2 1.225t-.675 1.075l-1.325 1q.025.175.025.338v.674q0 .163-.05.338l1.325 1q.525.425.675 1.075t-.2 1.225l-1.2 2.05q-.35.575-.975.8t-1.25-.05l-1.5-.65q-.275.2-.575.375t-.6.3l-.225 1.65q-.1.65-.587 1.1t-1.163.45zm1.225-6.5q1.45 0 2.475-1.025T15.55 12t-1.025-2.475T12.05 8.5q-1.475 0-2.488 1.025T8.55 12t1.013 2.475T12.05 15.5";
/// ep:close-bold, view box 1024, MIT, Element Plus.
pub const CLOSE_PATH: &str = "M195.2 195.2a64 64 0 0 1 90.5 0L512 421.5l226.3-226.3a64 64 0 0 1 90.5 90.5L602.5 512l226.3 226.3a64 64 0 0 1-90.5 90.5L512 602.5L285.7 828.8a64 64 0 0 1-90.5-90.5L421.5 512L195.2 285.7a64 64 0 0 1 0-90.5";
/// andrsrxn:ear, the filled silhouette of the duotone mark.
pub const EAR_PATH: &str = "M7.38 3.88L5.01 7.1v7.57l2.38 2.41a2 2 0 0 1 .5.9L8.43 20a2 2 0 0 0 1.18 1.35l1.47.59a2 2 0 0 0 1.6-.04l1.67-.79a2 2 0 0 0 1.1-1.41l.84-4.12a2 2 0 0 1 .24-.62l2.64-4.48a2 2 0 0 0 .23-1.43l-.82-3.86a2 2 0 0 0-1.05-1.37l-3.4-1.73a2 2 0 0 0-1.45-.14L8.45 3.14a2 2 0 0 0-1.07.74Z";
/// iconoir:fx-tag-solid
pub const FX_TAG_PATH: &str = "M8 2.25A6.75 6.75 0 0 0 1.25 9v6A6.75 6.75 0 0 0 8 21.75h8A6.75 6.75 0 0 0 22.75 15V9A6.75 6.75 0 0 0 16 2.25zm-2 6a.75.75 0 0 0-.75.75v6a.75.75 0 0 0 1.5 0v-2.25h2.821a.75.75 0 0 0 0-1.5H6.75v-1.5H11a.75.75 0 0 0 0-1.5zm7.576.27a.75.75 0 1 0-1.152.96l2.1 2.52l-2.1 2.52a.75.75 0 1 0 1.152.96l1.924-2.308l1.924 2.308a.75.75 0 1 0 1.152-.96l-2.1-2.52l2.1-2.52a.75.75 0 1 0-1.152-.96L15.5 10.829z";
/// Two nodes feeding one: the Signal Flow tab. Drawn for this panel, view box 24.
pub const FLOW_PATH: &str = "M5 3.8a2.2 2.2 0 1 0 0 4.4a2.2 2.2 0 1 0 0-4.4M5 15.8a2.2 2.2 0 1 0 0 4.4a2.2 2.2 0 1 0 0-4.4M19 9a3 3 0 1 0 0 6a3 3 0 1 0 0-6M7.09 6.63L7.91 5.37L16.11 10.77L15.29 12.03ZM7.09 17.37L7.91 18.63L16.11 13.23L15.29 11.97Z";
/// A folder: the Projects button. Drawn for this panel, view box 24.
pub const FOLDER_PATH: &str = "M2 6a2 2 0 0 1 2-2h5l2 2h9a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2z";

/// bi:headset-vr, view box 16, MIT, The Bootstrap Authors.
pub const HEADSET_VR_PATH: &str = "M8 1.248c1.857 0 3.526.641 4.65 1.794a5 5 0 0 1 2.518 1.09C13.907 1.482 11.295 0 8 0C4.75 0 2.12 1.48.844 4.122a5 5 0 0 1 2.289-1.047C4.236 1.872 5.974 1.248 8 1.248M12 12a4 4 0 0 1-2.786-1.13l-.002-.002a1.6 1.6 0 0 0-.276-.167A2.2 2.2 0 0 0 8 10.5c-.414 0-.729.103-.935.201a1.6 1.6 0 0 0-.277.167l-.002.002A4 4 0 1 1 4 4h8a4 4 0 0 1 0 8";
/// ic:round-bluetooth-searching, view box 24, Apache-2.0, Google.
pub const BLUETOOTH_PATH: &str = "m15.98 10.28l-1.38 1.38c-.2.2-.2.51 0 .71l1.38 1.38c.28.28.75.15.85-.23c.11-.5.17-1 .17-1.52a6 6 0 0 0-.18-1.48a.5.5 0 0 0-.84-.24m4.12-2.5a.863.863 0 0 0-1.4-.24c-.26.26-.31.64-.17.98c.46 1.07.72 2.24.72 3.47c0 1.24-.26 2.42-.73 3.49a.84.84 0 0 0 .16.94a.84.84 0 0 0 1.35-.23c.63-1.3.98-2.76.98-4.3a10 10 0 0 0-.91-4.11M11.41 12L15 8.42c.39-.39.39-1.02 0-1.42l-4.29-4.29c-.63-.63-1.71-.19-1.71.7v6.18L5.11 5.7A.996.996 0 1 0 3.7 7.11L8.59 12L3.7 16.89a.996.996 0 1 0 1.41 1.41L9 14.41v6.18c0 .89 1.08 1.34 1.71.71L15 17c.39-.39.39-1.02 0-1.42zM11 5.83l1.88 1.88L11 9.59zm0 12.34v-3.76l1.88 1.88z";

/// andrsrxn:device-pc-speaker, view box 24, MIT, Andrés Raxón. A stroked duotone,
/// so it is drawn by `pc_icon` rather than `icon`.
const PC_SCREEN_FILL: &str = "M1.66 10.14c0-1.89 0-2.83.58-3.42c.59-.58 1.53-.58 3.42-.58H8c1.89 0 2.83 0 3.42.58c.58.59.58 1.53.58 3.42v2.25c0 1.88 0 2.83-.58 3.41c-.59.59-1.53.59-3.42.59H5.66c-1.89 0-2.83 0-3.42-.59c-.58-.58-.58-1.53-.58-3.41z";
const PC_SCREEN_STROKE: &str = "M12 6.14H5.66c-1.89 0-2.83 0-3.42.58c-.58.59-.58 1.53-.58 3.42v2.25c0 1.88 0 2.83.58 3.41c.59.59 1.53.59 3.42.59h6.1m-5.99 3.07h4.31m-2.15 0v-2.98";

/// The PC streaming output. `size` is the square the 24-unit view box fills.
pub fn pc_icon(cr: &Context, x: f64, y: f64, size: f64, c: Rgb) {
    let _ = cr.save();
    cr.translate(x, y);
    cr.scale(size / 24.0, size / 24.0);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);
    append_svg(cr, PC_SCREEN_FILL);
    fill(cr, c, 0.2);
    cr.arc(17.17, 14.12, 2.34, 0.0, 2.0 * PI);
    fill(cr, c, 0.2);
    cr.set_source_rgb(c.r, c.g, c.b);
    cr.set_line_width(1.5);
    append_svg(cr, PC_SCREEN_STROKE);
    let _ = cr.stroke();
    rounded(cr, 12.0, 4.54, 10.34, 14.92, 2.0);
    let _ = cr.stroke();
    cr.new_sub_path();
    cr.arc(17.17, 14.12, 2.17, 0.0, 2.0 * PI);
    let _ = cr.stroke();
    cr.arc(17.17, 8.1, 0.46, 0.0, 2.0 * PI);
    let _ = cr.fill_preserve();
    let _ = cr.stroke();
    let _ = cr.restore();
}

/// andrsrxn:usb-port, view box 24, MIT, Andrés Raxón. A stroked duotone like `pc_icon`.
const USB_RING_STROKE: &str = "M13.5 20.8a1.54 1.54 0 1 1-3.08 0a1.54 1.54 0 0 1 3.08 0M8.84 9.53a1.48 1.48 0 1 1-2.96 0a1.48 1.48 0 0 1 2.96 0";
const USB_TREE_STROKE: &str = "M11.96 17.19L9.4 15.75c-1-.56-1.49-.84-1.76-1.3c-.28-.47-.28-1.05-.28-2.19v-1.24m4.6 3.4l3.14-1.38c.87-.39 1.3-.58 1.6-.91a2 2 0 0 0 .3-.47c.2-.4.2-.88.2-1.84m-5.24 9.44V1.99";
const USB_TIP_STROKE: &str = "m9.38 3.93l1.16-1.16c.67-.67 1-1 1.42-1s.75.33 1.41 1l1.17 1.16";

// 6.28 is an icon coordinate from the source SVG, not a rounded TAU.
#[expect(clippy::approx_constant)]
pub fn usb_icon(cr: &Context, x: f64, y: f64, size: f64, c: Rgb) {
    let _ = cr.save();
    cr.translate(x, y);
    cr.scale(size / 24.0, size / 24.0);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);
    for (rx, ry, rs) in [(15.72, 6.28, 2.96), (10.53, 19.37, 2.86), (5.85, 7.99, 3.09)] {
        rounded(cr, rx, ry, rs, rs, 1.0);
        fill(cr, c, 0.2);
    }
    cr.set_source_rgb(c.r, c.g, c.b);
    cr.set_line_width(1.5);
    for path in [USB_RING_STROKE, USB_TREE_STROKE, USB_TIP_STROKE] {
        append_svg(cr, path);
        let _ = cr.stroke();
    }
    rounded(cr, 15.72, 6.28, 2.8, 2.8, 0.5);
    let _ = cr.stroke();
    let _ = cr.restore();
}

/// What an A output plays on. Decides its icon and title.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Device {
    Headset,
    Bluetooth,
    Usb,
    Other,
}

impl Device {
    pub fn of_sink(name: &str) -> Device {
        let lower = name.to_ascii_lowercase();
        if lower.starts_with("bluez_output") {
            Device::Bluetooth
        } else if lower.contains("usb") {
            Device::Usb
        } else if lower.contains("platform-sound") || lower.contains("speaker") {
            Device::Headset
        } else {
            Device::Other
        }
    }
}

/// The headset, Bluetooth or USB glyph in a `size` square. `Other` draws nothing and returns false.
pub fn device_icon(cr: &Context, device: Device, x: f64, y: f64, size: f64, c: Rgb) -> bool {
    match device {
        Device::Headset => icon(cr, HEADSET_VR_PATH, 16.0, x, y, size, c),
        Device::Bluetooth => icon(cr, BLUETOOTH_PATH, 24.0, x, y, size, c),
        Device::Usb => usb_icon(cr, x, y, size, c),
        Device::Other => return false,
    }
    true
}
