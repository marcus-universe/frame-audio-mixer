//! The mix strip: one framed microphone and a scrolling row of framed channels.

use super::{fill_round, source_label, Panel};
use crate::draw::{self, Device, Fonts, INSET, MUTED, R_SURFACE, TEXT};
use crate::i18n::Text;
use crate::worker::Snapshot;
use cairo::Context;
use std::collections::HashMap;

const MIC_W: f64 = 188.0;
const CHAN_W: f64 = 210.0;
const ADD_W: f64 = 64.0;
const GAP: f64 = 12.0;
/// Inner padding of the mixer card.
const PAD: f64 = 20.0;
/// Header row of the mixer card: On/Off, status, and the view tools.
const TOOL_S: f64 = 40.0;
pub(crate) const TOOLS_W: f64 = TOOL_S * 3.0 + 8.0 * 2.0;
const ICON_S: f64 = 20.0;
/// Channel name baseline and the capsule top inside a vertical strip.
const STRIP_TITLE_Y: f64 = 30.0;
const CAP_TOP: f64 = 48.0;
/// Route buttons stacked left of a vertical fader. They shrink to `ROUTE_H_MIN`
/// rather than wrap into a second column.
const ROUTE_W: f64 = 44.0;
const ROUTE_H: f64 = 26.0;
const ROUTE_H_MIN: f64 = 18.0;
const ROUTE_GAP: f64 = 3.0;
/// Space between the A, PC, B and FX groups.
const GROUP_GAP: f64 = 8.0;
const STACK_GAP: f64 = 14.0;
/// Capsule, meter and meter scale, measured from the capsule's left edge.
/// Source picker at the top of the microphone strip.
const PICK_H: f64 = 32.0;

pub(crate) fn draw(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot, peaks: &HashMap<String, f32>) {
    let stage = panel.stage();
    let (x, y, w, h) = stage.mix;
    draw::card(cr, x, y, w, h);
    let tools_x = x + w - PAD - TOOLS_W;
    header_tools(panel, cr, tools_x, y + 8.0, false);
    let title_max = (tools_x - 12.0 - (x + PAD)).max(24.0);
    draw::label_fit(cr, fonts, true, 20.0, TEXT, x + PAD, y + 40.0, title_max, t.inputs);
    if !panel.notice.is_empty() {
        let title_w = draw::text_width(cr, fonts, true, 20.0, t.inputs).min(title_max);
        let nx = x + PAD + title_w + 12.0;
        let nw = tools_x - 12.0 - nx;
        if nw >= 48.0 {
            draw::label_fit(cr, fonts, false, 13.0, draw::DANGER_TEXT, nx, y + 40.0, nw, &panel.notice);
        }
    }

    let strip_top = y + 56.0;
    let strip_h = (y + h - PAD - strip_top).max(180.0);
    let a_slots = super::outputs::present_a(&snap.graph);
    let source = source_label(&panel.config.mic_source, &snap.graph, t);
    if panel.config.input_list {
        draw_list(panel, cr, fonts, t, peaks, &a_slots, &source, x, strip_top, w, strip_h);
        return;
    }
    let cap_top = CAP_TOP;
    let cap_h = (strip_h - cap_top - draw::FADER_FOOT).max(80.0);
    let stack = Stack::fit(cell_count(&a_slots), cap_h);
    // One width for the microphone and every channel, so the columns match.
    let strip_w = CHAN_W.max(MIC_W);
    let mic_x = x + PAD;
    fill_round(cr, mic_x, strip_top, strip_w, strip_h, R_SURFACE, INSET);
    source_picker(panel, cr, fonts, mic_x + 8.0, strip_top + 8.0, strip_w - 16.0, &source, None);
    let mic_levels = [peaks.get("mic").copied().unwrap_or(0.0)];
    let fader_cx = mic_x + strip_w / 2.0;
    let geom = draw::paint_fader(
        cr,
        fonts,
        fader_cx,
        strip_top + cap_top,
        cap_h,
        panel.config.mic_gain_db,
        &mic_levels,
        panel.config.meters,
        panel.config.mic_muted,
        draw::FaderGlyph::Mic,
        true,
    );
    panel.fader_hit("mic", geom.capsule_x - 4.0, geom.capsule_y, geom.meter_right - geom.capsule_x + 8.0, geom.capsule_h, false);
    let mic_flow = Routes { bits: panel.config.mic_routes, to_pc: panel.config.mic_to_pc, fx1: panel.config.mic_send_fx1, fx2: panel.config.mic_send_fx2 };
    stack.draw(panel, cr, fonts, fader_cx - 16.0 - STACK_GAP - stack.w, strip_top + cap_top, "mic", mic_flow, &a_slots, None);
    mute_button(panel, cr, fonts, t, fader_cx, strip_top + cap_top + cap_h, strip_w - 16.0, "mic", panel.config.mic_muted, None);

    let list_x = mic_x + strip_w + GAP;
    let list_w = x + w - PAD - list_x;
    let content = panel.config.channels.len() as f64 * (strip_w + GAP) + ADD_W;
    let max_scroll = (content - list_w).max(0.0);
    panel.scroll = panel.scroll.clamp(0.0, max_scroll);
    if list_w > 1.0 {
    let _ = cr.save();
    cr.rectangle(list_x, strip_top, list_w, strip_h);
    let _ = cr.clip();
    let mut cx = list_x - panel.scroll;
    let channels = panel.config.channels.clone();
    let view = Clip { left: list_x, right: list_x + list_w };
    panel.set_strip(list_x, strip_top, list_w, strip_h);
    panel.visible_channels.clear();
    for channel in &channels {
        if cx + strip_w >= list_x && cx <= list_x + list_w {
            panel.visible_channels.push(channel.id.clone());
            fill_round(cr, cx, strip_top, strip_w, strip_h, R_SURFACE, INSET);
            let armed = panel.delete_armed(&channel.id);
            if armed {
                fill_round(cr, cx + 8.0, strip_top + 8.0, strip_w - 16.0, 32.0, 16.0, draw::DANGER_FILL);
                let confirm_w = draw::text_width(cr, fonts, true, 13.0, t.delete_confirm).min(strip_w - 28.0);
                draw::label_fit(cr, fonts, true, 13.0, draw::DANGER_TEXT, cx + (strip_w - confirm_w) / 2.0, strip_top + 29.0, strip_w - 28.0, t.delete_confirm);
                view.hot(panel, &format!("rm:{}", channel.id), cx + 8.0, strip_top + 8.0, strip_w - 16.0, 32.0);
            } else {
                // Icon and name are centered as one group and stay clear of the ×.
                let with_icon = !channel.app.is_empty();
                let icon_room = if with_icon { ICON_S + 6.0 } else { 0.0 };
                let name_room = strip_w - 52.0 - icon_room;
                let name_w = draw::text_width(cr, fonts, true, 15.0, &channel.name).min(name_room);
                let group = icon_room + name_w;
                let title_x = (cx + (strip_w - group) / 2.0).min(cx + strip_w - 38.0 - group);
                if with_icon {
                    super::paint_desktop_icon(panel, cr, fonts, &channel.app, &channel.binary, title_x, strip_top + STRIP_TITLE_Y - 15.0, ICON_S);
                }
                draw::label_fit(cr, fonts, true, 15.0, draw::accent(), title_x + icon_room, strip_top + STRIP_TITLE_Y, name_room, &channel.name);
                view.hot(panel, &format!("name:{}", channel.id), cx + 8.0, strip_top + 8.0, strip_w - 48.0, 32.0);
                remove_glyph(panel, cr, cx + strip_w - 40.0, strip_top + 8.0);
                view.hot(panel, &format!("rm:{}", channel.id), cx + strip_w - 40.0, strip_top + 8.0, REMOVE_S, REMOVE_S);
            }
            let levels = stereo_levels(peaks, &channel.id);
            let fader_cx = cx + strip_w / 2.0;
            let geom = draw::paint_fader(
                cr,
                fonts,
                fader_cx,
                strip_top + cap_top,
                cap_h,
                channel.gain_db,
                &levels,
                panel.config.meters,
                channel.muted,
                draw::FaderGlyph::Speaker,
                true,
            );
            view.fader(panel, &channel.id, geom.capsule_x - 2.0, geom.capsule_y, geom.meter_right - geom.capsule_x + 4.0, geom.capsule_h);
            let flow = Routes { bits: channel.routes, to_pc: channel.to_pc, fx1: channel.send_fx1, fx2: channel.send_fx2 };
            stack.draw(panel, cr, fonts, fader_cx - 16.0 - STACK_GAP - stack.w, strip_top + cap_top, &channel.id, flow, &a_slots, Some(&view));
            mute_button(panel, cr, fonts, t, fader_cx, strip_top + cap_top + cap_h, strip_w - 16.0, &channel.id, channel.muted, Some(&view));
        }
        cx += strip_w + GAP;
    }
    let add_y = strip_top + (strip_h - ADD_W) / 2.0;
    let room = channels.len() < crate::config::MAX_CHANNELS;
    add_button(panel, cr, cx, add_y, ADD_W, room);
    if room {
        view.hot(panel, "add", cx, add_y, ADD_W, ADD_W);
    }
    let _ = cr.restore();

    if max_scroll > 0.0 {
        let thumb_w = draw::scroll_thumb(list_w, content, 32.0);
        let thumb_x = list_x + (list_w - thumb_w) * (panel.scroll / max_scroll);
        let bar_y = strip_top + strip_h - 8.0;
        draw::paint_scroll_thumb(cr, thumb_x, bar_y, thumb_w, 4.0);
        panel.hbar = Some(super::HBar { x: list_x, y: bar_y - 8.0, w: list_w, h: 16.0, thumb_w, max_scroll, vertical: false });
    }
    }
}

const ROW_H: f64 = 128.0;
const LIST_GAP: f64 = 8.0;
/// Title baseline inside a list row.
const TITLE_Y: f64 = 22.0;
const SLOT_H: f64 = 40.0;
const ROW_PAD: f64 = 16.0;

/// Vertical list. The microphone is the first row and scrolls with the channels.
fn draw_list(
    panel: &mut Panel,
    cr: &Context,
    fonts: &Fonts,
    t: &Text,
    peaks: &HashMap<String, f32>,
    a_slots: &[usize],
    source: &str,
    x: f64,
    strip_top: f64,
    w: f64,
    strip_h: f64,
) {
    let row_x = x + PAD;
    let row_w = w - 2.0 * PAD;
    let list_top = strip_top;
    let list_h = strip_h.max(72.0);
    let channels = panel.config.channels.clone();
    let content = (channels.len() as f64 + 1.0) * (ROW_H + LIST_GAP) + 56.0;
    let max_scroll = (content - list_h).max(0.0);
    panel.scroll = panel.scroll.clamp(0.0, max_scroll);
    panel.set_strip(row_x, list_top, row_w, list_h);
    let _ = cr.save();
    cr.rectangle(row_x, list_top, row_w, list_h);
    let _ = cr.clip();
    let view = VClip { top: list_top, bottom: list_top + list_h, left: row_x, right: row_x + row_w };
    panel.visible_channels.clear();
    let mut ry = list_top - panel.scroll;
    if ry + ROW_H >= list_top && ry <= list_top + list_h {
        let mic_levels = [peaks.get("mic").copied().unwrap_or(0.0)];
        list_mic(panel, cr, fonts, t, &mic_levels, a_slots, source, row_x, ry, row_w, &view);
    }
    ry += ROW_H + LIST_GAP;
    for channel in &channels {
        if ry + ROW_H >= list_top && ry <= list_top + list_h {
            panel.visible_channels.push(channel.id.clone());
            let levels = stereo_levels(peaks, &channel.id);
            list_channel(panel, cr, fonts, t, channel, &levels, a_slots, row_x, ry, row_w, &view);
        }
        ry += ROW_H + LIST_GAP;
    }
    let room = channels.len() < crate::config::MAX_CHANNELS;
    let add_s = 56.0;
    let add_x = row_x + (row_w - add_s) / 2.0;
    add_button(panel, cr, add_x, ry, add_s, room);
    if room {
        view.hot(panel, "add", add_x, ry, add_s, add_s);
    }
    let _ = cr.restore();

    if max_scroll > 0.0 {
        let bar_x = x + w - 11.0;
        let thumb_h = draw::scroll_thumb(list_h, content, 32.0);
        let thumb_y = list_top + (list_h - thumb_h) * (panel.scroll / max_scroll);
        draw::paint_scroll_thumb(cr, bar_x, thumb_y, 4.0, thumb_h);
        panel.hbar = Some(super::HBar {
            x: bar_x - 8.0,
            y: list_top,
            w: 16.0,
            h: list_h,
            thumb_w: thumb_h,
            max_scroll,
            vertical: true,
        });
    }
}

fn list_mic(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, levels: &[f32], a_slots: &[usize], source: &str, x: f64, y: f64, w: f64, view: &VClip) {
    fill_round(cr, x, y, w, ROW_H, R_SURFACE, INSET);
    source_picker(panel, cr, fonts, x + 8.0, y + 4.0, (w - 16.0).min(320.0), source, Some(view));
    let mic = draw::paint_side_fader(
        cr,
        fonts,
        x + ROW_PAD - 4.0,
        y + 36.0,
        w - 2.0 * ROW_PAD + 4.0,
        SLOT_H,
        panel.config.mic_gain_db,
        levels,
        panel.config.meters,
        panel.config.mic_muted,
        draw::FaderGlyph::Mic,
        true,
    );
    view.fader(panel, "mic", mic.capsule_x, mic.capsule_y, mic.capsule_w, mic.capsule_h);
    let used = list_mute(panel, cr, fonts, t, x + ROW_PAD, y + ROW_H - 36.0, "mic", panel.config.mic_muted, view);
    let flow = Routes { bits: panel.config.mic_routes, to_pc: panel.config.mic_to_pc, fx1: panel.config.mic_send_fx1, fx2: panel.config.mic_send_fx2 };
    route_row(panel, cr, fonts, x + ROW_PAD + used + 16.0, y + ROW_H - 32.0, x + w - ROW_PAD, "mic", flow, a_slots, view);
}

fn list_channel(
    panel: &mut Panel,
    cr: &Context,
    fonts: &Fonts,
    t: &Text,
    channel: &crate::config::Channel,
    levels: &[f32],
    a_slots: &[usize],
    x: f64,
    y: f64,
    w: f64,
    view: &VClip,
) {
    fill_round(cr, x, y, w, ROW_H, R_SURFACE, INSET);
    let armed = panel.delete_armed(&channel.id);
    if armed {
        fill_round(cr, x + 8.0, y + 6.0, w - 56.0, 28.0, 14.0, draw::DANGER_FILL);
        draw::label_fit(cr, fonts, true, 13.0, draw::DANGER_TEXT, x + ROW_PAD, y + 25.0, w - 80.0, t.delete_confirm);
        view.hot(panel, &format!("rm:{}", channel.id), x + 8.0, y + 6.0, w - 56.0, 28.0);
    } else {
        let with_icon = !channel.app.is_empty();
        let icon_room = if with_icon { ICON_S + 8.0 } else { 0.0 };
        if with_icon {
            super::paint_desktop_icon(panel, cr, fonts, &channel.app, &channel.binary, x + ROW_PAD, y + TITLE_Y - 15.0, ICON_S);
        }
        draw::label_fit(cr, fonts, true, 15.0, draw::accent(), x + ROW_PAD + icon_room, y + TITLE_Y, 180.0, &channel.name);
        view.hot(panel, &format!("name:{}", channel.id), x + 8.0, y + 4.0, 220.0, 30.0);
    }
    remove_glyph(panel, cr, x + w - 40.0, y + 4.0);
    view.hot(panel, &format!("rm:{}", channel.id), x + w - 40.0, y + 4.0, REMOVE_S, REMOVE_S);
    let fader = draw::paint_side_fader(
        cr,
        fonts,
        x + ROW_PAD - 4.0,
        y + 36.0,
        w - 2.0 * ROW_PAD + 4.0,
        SLOT_H,
        channel.gain_db,
        levels,
        panel.config.meters,
        channel.muted,
        draw::FaderGlyph::Speaker,
        true,
    );
    view.fader(panel, &channel.id, fader.capsule_x, fader.capsule_y, fader.capsule_w, fader.capsule_h);
    let used = list_mute(panel, cr, fonts, t, x + ROW_PAD, y + ROW_H - 36.0, &channel.id, channel.muted, view);
    let flow = Routes { bits: channel.routes, to_pc: channel.to_pc, fx1: channel.send_fx1, fx2: channel.send_fx2 };
    route_row(panel, cr, fonts, x + ROW_PAD + used + 16.0, y + ROW_H - 32.0, x + w - ROW_PAD, &channel.id, flow, a_slots, view);
}

/// Left and right peaks of a channel, as the engine publishes them.
fn stereo_levels(peaks: &HashMap<String, f32>, id: &str) -> [f32; 2] {
    let side = |suffix: &str| peaks.get(&format!("{id}:{suffix}")).copied().unwrap_or(0.0);
    [side("l"), side("r")]
}

/// The microphone source as a dropdown at the top of the mic strip.
fn source_picker(panel: &mut Panel, cr: &Context, fonts: &Fonts, x: f64, y: f64, w: f64, label: &str, view: Option<&VClip>) {
    let look = panel.look(x, y, w, PICK_H);
    let fill = match look {
        draw::Look::Down => draw::CONTROL_DOWN,
        draw::Look::Hover => draw::CONTROL_HOVER,
        draw::Look::Rest => draw::CONTROL,
    };
    fill_round(cr, x, y, w, PICK_H, draw::R_CONTROL, fill);
    let glyph = 16.0;
    draw::icon(cr, draw::MIC_PATH, 256.0, x + 10.0, y + (PICK_H - glyph) / 2.0, glyph, draw::accent());
    draw::label_fit(cr, fonts, true, 14.0, TEXT, x + 32.0, y + PICK_H / 2.0 + 5.0, w - 32.0 - 28.0, label);
    draw::chevron(cr, x + w - 22.0, y + PICK_H / 2.0 - 3.0, 12.0, MUTED, false, false);
    match view {
        Some(view) => view.hot(panel, "mic-open", x, y, w, PICK_H),
        None => panel.hot("mic-open", x, y, w, PICK_H),
    }
}


/// The visible part of the channel row. Hit areas of a half-hidden channel stop at its
/// edge, so they cannot catch a click meant for Mix Check or the microphone strip.
struct Clip {
    left: f64,
    right: f64,
}

impl Clip {
    fn cut(&self, x: f64, w: f64) -> Option<(f64, f64)> {
        let start = x.max(self.left);
        let end = (x + w).min(self.right);
        (end > start).then_some((start, end - start))
    }

    fn hot(&self, panel: &mut Panel, id: &str, x: f64, y: f64, w: f64, h: f64) {
        if let Some((x, w)) = self.cut(x, w) {
            panel.hot(id, x, y, w, h);
        }
    }

    fn fader(&self, panel: &mut Panel, id: &str, x: f64, y: f64, w: f64, h: f64) {
        if let Some((x, w)) = self.cut(x, w) {
            panel.fader_hit(id, x, y, w, h, false);
        }
    }
}

struct VClip {
    top: f64,
    bottom: f64,
    left: f64,
    right: f64,
}

impl VClip {
    fn hot(&self, panel: &mut Panel, id: &str, x: f64, y: f64, w: f64, h: f64) {
        let y0 = y.max(self.top);
        let y1 = (y + h).min(self.bottom);
        let x0 = x.max(self.left);
        let x1 = (x + w).min(self.right);
        if y1 > y0 && x1 > x0 {
            panel.hot(id, x0, y0, x1 - x0, y1 - y0);
        }
    }

    fn fader(&self, panel: &mut Panel, id: &str, x: f64, y: f64, w: f64, h: f64) {
        if y >= self.top && y + h <= self.bottom && x >= self.left && x + w <= self.right {
            panel.fader_hit(id, x, y, w, h, true);
        }
    }
}

const REMOVE_S: f64 = 32.0;

/// The × that removes a channel. Quiet until the pointer is on it.
fn remove_glyph(panel: &Panel, cr: &Context, x: f64, y: f64) {
    let look = panel.look(x, y, REMOVE_S, REMOVE_S);
    if look != draw::Look::Rest {
        let fill = if look == draw::Look::Down { draw::CONTROL_DOWN } else { draw::CONTROL_HOVER };
        fill_round(cr, x, y, REMOVE_S, REMOVE_S, REMOVE_S / 2.0, fill);
    }
    let s = 18.0;
    draw::icon(cr, draw::CLOSE_PATH, 1024.0, x + (REMOVE_S - s) / 2.0, y + (REMOVE_S - s) / 2.0, s, if look == draw::Look::Rest { MUTED } else { TEXT });
}

/// The add-channel tile: the accent square glyph on a tonal tile.
fn add_button(panel: &Panel, cr: &Context, x: f64, y: f64, s: f64, room: bool) {
    let look = if room { panel.look(x, y, s, s) } else { draw::Look::Rest };
    let fill = match look {
        draw::Look::Down => draw::CONTROL_DOWN,
        draw::Look::Hover => draw::CONTROL_HOVER,
        draw::Look::Rest => INSET,
    };
    fill_round(cr, x, y, s, s, R_SURFACE, fill);
    let glyph = (s * 0.5).round();
    draw::icon(cr, draw::ADD_PATH, 28.0, x + (s - glyph) / 2.0, y + (s - glyph) / 2.0, glyph, if room { draw::accent() } else { MUTED });
}

/// Snap, the wide window, and this side's orientation. `outputs` picks which orientation the last button owns.
pub(crate) fn header_tools(panel: &mut Panel, cr: &Context, x: f64, y: f64, outputs: bool) {
    let step = TOOL_S + 8.0;
    let rows = if outputs { panel.config.list } else { panel.config.input_list };
    let snap_id = if panel.config.snap { "snap-off" } else { "snap-on" };
    tool_button(panel, cr, x, y, draw::MAGNET_PATH, 16.0, panel.config.snap, snap_id);
    let (extend_path, extend_view) = if rows {
        (draw::RESIZE_ROW_PATH, 24.0)
    } else {
        (draw::RESIZE_COLUMN_PATH, 24.0)
    };
    let wide_id = if panel.config.wide { "wide-off" } else { "wide-on" };
    tool_button(panel, cr, x + step, y, extend_path, extend_view, panel.config.wide, wide_id);
    let (view_path, view_on, view_id) = if rows {
        (draw::LIST_PATH, true, if outputs { "list-off" } else { "in-list-off" })
    } else {
        (draw::COLUMNS_PATH, false, if outputs { "list-on" } else { "in-list-on" })
    };
    tool_button(panel, cr, x + step * 2.0, y, view_path, 512.0, view_on, view_id);
}

/// Where one strip goes: A bits, the PC, B bits and the two sends.
#[derive(Clone, Copy)]
struct Routes {
    bits: u8,
    to_pc: bool,
    fx1: bool,
    fx2: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cell {
    A,
    Pc,
    B,
    Fx1,
    Fx2,
}

/// One route button: what it is, its caption and hit id, whether it is on, and
/// for a listen output the device it plays on.
struct RouteCell {
    kind: Cell,
    caption: String,
    hit: String,
    on: bool,
    device: Device,
}

/// Buttons beside one fader: the listen outputs that are present, the PC, B1..B4, FX1 and FX2.
fn cell_count(a_slots: &[usize]) -> usize {
    a_slots.len() + 1 + crate::config::B_SLOTS + 2
}

fn route_cells(id: &str, routes: Routes, a_slots: &[usize]) -> Vec<RouteCell> {
    let mut cells = vec![RouteCell { kind: Cell::Pc, caption: String::new(), hit: format!("route:{id}:pc"), on: routes.to_pc, device: Device::Other }];
    for &index in a_slots {
        cells.push(RouteCell {
            kind: Cell::A,
            caption: String::new(),
            hit: format!("route:{id}:a:{index}"),
            on: crate::config::route_on(routes.bits, true, index),
            device: super::outputs::a_device(index),
        });
    }
    for index in 0..crate::config::B_SLOTS {
        cells.push(RouteCell {
            kind: Cell::B,
            caption: format!("B{}", index + 1),
            hit: format!("route:{id}:b:{index}"),
            on: crate::config::route_on(routes.bits, false, index),
            device: Device::Other,
        });
    }
    cells.push(RouteCell { kind: Cell::Fx1, caption: "FX1".into(), hit: format!("send:{id}:1"), on: routes.fx1, device: Device::Other });
    cells.push(RouteCell { kind: Cell::Fx2, caption: "FX2".into(), hit: format!("send:{id}:2"), on: routes.fx2, device: Device::Other });
    cells
}

/// The route buttons beside a vertical fader, always one column. With many outputs
/// the buttons get shorter instead of wrapping.
struct Stack {
    w: f64,
    h: f64,
    gap: f64,
    group_gap: f64,
}

impl Stack {
    fn fit(cells: usize, room: f64) -> Self {
        let n = cells as f64;
        let full = n * ROUTE_H + (n - 1.0) * ROUTE_GAP + 3.0 * GROUP_GAP;
        let scale = (room / full).clamp(ROUTE_H_MIN / ROUTE_H, 1.0);
        Self { w: ROUTE_W, h: ROUTE_H * scale, gap: ROUTE_GAP * scale, group_gap: GROUP_GAP * scale }
    }

    fn draw(&self, panel: &mut Panel, cr: &Context, fonts: &Fonts, x: f64, y: f64, id: &str, routes: Routes, a_slots: &[usize], clip: Option<&Clip>) {
        let mut cy = y;
        let mut last = None;
        for cell in route_cells(id, routes, a_slots) {
            if last.is_some_and(|kind| group(kind) != group(cell.kind)) {
                cy += self.group_gap;
            }
            last = Some(cell.kind);
            route_cell(panel, cr, fonts, x, cy, self.w, self.h, &cell, clip, None);
            cy += self.h + self.gap;
        }
    }
}

/// FX1 and FX2 sit together. Every other kind is its own group.
fn group(cell: Cell) -> Cell {
    if cell == Cell::Fx2 { Cell::Fx1 } else { cell }
}

/// The same buttons in one row, for list view. They narrow to fit before `right`.
fn route_row(panel: &mut Panel, cr: &Context, fonts: &Fonts, x: f64, y: f64, right: f64, id: &str, routes: Routes, a_slots: &[usize], view: &VClip) {
    let cells = route_cells(id, routes, a_slots);
    let gaps = 3.0 * GROUP_GAP + (cells.len() - 1) as f64 * ROUTE_GAP;
    let bw = ((right - x - gaps) / cells.len() as f64).clamp(14.0, 40.0);
    let h = 24.0;
    let mut cx = x;
    let mut last = None;
    for cell in &cells {
        if last.is_some_and(|kind| group(kind) != group(cell.kind)) {
            cx += GROUP_GAP;
        }
        last = Some(cell.kind);
        route_cell(panel, cr, fonts, cx, y, bw, h, cell, None, Some(view));
        cx += bw + ROUTE_GAP;
    }
}

fn route_cell(panel: &mut Panel, cr: &Context, fonts: &Fonts, x: f64, y: f64, w: f64, h: f64, cell: &RouteCell, clip: Option<&Clip>, vclip: Option<&VClip>) {
    let (kind, on, id) = (cell.kind, cell.on, cell.hit.as_str());
    let (fill, ink) = if on {
        match kind {
            Cell::A => (TEXT, draw::BG),
            Cell::Pc => (draw::accent(), draw::on_accent()),
            Cell::B => (draw::mix(draw::accent(), draw::CONTROL, 0.45), TEXT),
            Cell::Fx1 => (draw::FX1, draw::BG),
            Cell::Fx2 => (draw::FX2, draw::BG),
        }
    } else {
        let rest = match panel.look(x, y, w, h) {
            draw::Look::Down => draw::CONTROL_DOWN,
            draw::Look::Hover => draw::CONTROL_HOVER,
            draw::Look::Rest => draw::CONTROL,
        };
        let ink = match kind {
            Cell::Pc => draw::accent(),
            Cell::Fx1 => draw::FX1,
            Cell::Fx2 => draw::FX2,
            _ => MUTED,
        };
        (rest, ink)
    };
    fill_round(cr, x, y, w, h, 8.0_f64.min(h / 2.0), fill);
    let s = (h - 6.0).min(20.0);
    let (gx, gy) = (x + (w - s) / 2.0, y + (h - s) / 2.0);
    let drawn = match kind {
        Cell::Pc => {
            draw::pc_icon(cr, gx, gy, s, ink);
            true
        }
        Cell::A => draw::device_icon(cr, cell.device, gx, gy, s, ink),
        _ => false,
    };
    if !drawn {
        let size = if h < 22.0 { 11.0 } else { 12.0 };
        let tw = draw::text_width(cr, fonts, true, size, &cell.caption);
        draw::label_mid(cr, fonts, true, size, ink, x + (w - tw) / 2.0, y + h / 2.0, &cell.caption);
    }
    if let Some(clip) = clip {
        clip.hot(panel, id, x, y, w, h);
    } else if let Some(view) = vclip {
        view.hot(panel, id, x, y, w, h);
    } else {
        panel.hot(id, x, y, w, h);
    }
}

/// Mute under a vertical fader, centered on the strip.
fn mute_button(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, cx: f64, foot: f64, max_w: f64, id: &str, muted: bool, clip: Option<&Clip>) {
    let btn_w = max_w.min(96.0);
    pill(panel, cr, fonts, t.mute, &format!("mute:{id}"), cx - btn_w / 2.0, foot + draw::FADER_MUTE_Y, btn_w, draw::FADER_MUTE_H, muted, clip);
}

/// Mute of a list row. Returns the width it takes.
fn list_mute(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, x: f64, y: f64, id: &str, muted: bool, view: &VClip) -> f64 {
    let mute_w = 72.0;
    pill_clip(panel, cr, fonts, t.mute, &format!("mute:{id}"), x, y + 4.0, mute_w, 24.0, muted, view);
    mute_w
}

fn pill(panel: &mut Panel, cr: &Context, fonts: &Fonts, caption: &str, id: &str, x: f64, y: f64, w: f64, h: f64, on: bool, clip: Option<&Clip>) {
    let fill = if on { draw::DANGER } else { draw::CONTROL };
    let ink = if on { draw::BG } else { draw::SOFT };
    fill_round(cr, x, y, w, h, 12.0, fill);
    let tw = draw::text_width(cr, fonts, true, 12.0, caption);
    draw::label_mid(cr, fonts, true, 12.0, ink, x + (w - tw) / 2.0, y + h / 2.0, caption);
    if let Some(clip) = clip {
        clip.hot(panel, id, x, y, w, h);
    } else {
        panel.hot(id, x, y, w, h);
    }
}

fn pill_clip(panel: &mut Panel, cr: &Context, fonts: &Fonts, caption: &str, id: &str, x: f64, y: f64, w: f64, h: f64, on: bool, view: &VClip) {
    let fill = if on { draw::DANGER } else { draw::CONTROL };
    let ink = if on { draw::BG } else { draw::SOFT };
    fill_round(cr, x, y, w, h, 12.0, fill);
    let tw = draw::text_width(cr, fonts, true, 12.0, caption);
    draw::label_mid(cr, fonts, true, 12.0, ink, x + (w - tw) / 2.0, y + h / 2.0, caption);
    view.hot(panel, id, x, y, w, h);
}

fn tool_button(panel: &mut Panel, cr: &Context, x: f64, y: f64, path: &str, view: f64, on: bool, id: &str) {
    let s = TOOL_S;
    let look = panel.look(x, y, s, s);
    let fill = if on {
        match look {
            draw::Look::Down => draw::accent_down(),
            draw::Look::Hover => draw::accent_hover(),
            draw::Look::Rest => draw::accent(),
        }
    } else {
        match look {
            draw::Look::Down => draw::CONTROL_DOWN,
            draw::Look::Hover => draw::CONTROL_HOVER,
            draw::Look::Rest => draw::CONTROL,
        }
    };
    fill_round(cr, x, y, s, s, draw::R_CONTROL, fill);
    let glyph = 20.0;
    draw::icon(cr, path, view, x + (s - glyph) / 2.0, y + (s - glyph) / 2.0, glyph, if on { draw::on_accent() } else { draw::SOFT });
    panel.hot(id, x, y, s, s);
}
