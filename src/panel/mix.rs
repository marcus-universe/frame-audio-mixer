//! The mix strip: one framed microphone and a scrolling row of framed channels.

use super::{source_label, BusId, FaderId, Hit, HitBounds, Panel, Scrollbar, Source, Ui};
use crate::config::{self, Channel};
use crate::draw::{self, Device, Fader, FaderGlyph, Font, Look, Rect, INSET, MUTED, R_SURFACE, TEXT};
use crate::worker::Snapshot;
use std::collections::HashMap;

const ADD_W: f64 = 64.0;
const GAP: f64 = 12.0;
/// Inner padding of the mixer card.
const PAD: f64 = 20.0;
/// Baseline of the card title and of the notice beside it, from the card top.
const HEADER_BASELINE: f64 = 40.0;
/// Where the strips start, from the card top.
const STRIPS_TOP: f64 = 56.0;
/// Header row of the mixer card: On/Off, status, and the view tools.
const TOOL_S: f64 = 40.0;
pub(crate) const TOOLS_W: f64 = TOOL_S * 2.0 + 8.0;
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
const STACK_GAP: f64 = 8.0;
/// Padding between a vertical strip's edges and its route buttons or meter scale.
const STRIP_PAD: f64 = 10.0;
/// Fader capsule, its gap to the meter, and a stereo meter pair (see `draw::paint_fader`).
const FADER_AND_METER_W: f64 = 32.0 + 4.0 + 7.0 * 2.0 + 2.0;
/// Source picker at the top of the microphone strip.
const PICK_H: f64 = 32.0;
/// The × that removes a channel.
const REMOVE_S: f64 = 32.0;

const CHANNEL_NAME: Font = Font::bold(15.0);
/// "Press again to delete", in place of the channel name.
const CONFIRM: Font = Font::bold(13.0);
const PILL: Font = Font::bold(12.0);

pub(crate) fn draw(panel: &mut Panel, ui: &mut Ui, snap: &Snapshot, peaks: &HashMap<String, f32>) {
    let t = ui.t;
    let card = panel.stage().mix;
    let Rect { x, y, w, h } = card;
    ui.card(card);
    let tools_x = x + w - PAD - TOOLS_W;
    header_tools(panel, ui, tools_x, y + 8.0, false);
    let title_max = (tools_x - 12.0 - (x + PAD)).max(24.0);
    ui.label_fit(draw::HEADING, TEXT, x + PAD, y + HEADER_BASELINE, title_max, t.inputs);
    if !panel.notice.is_empty() {
        let title_w = ui.text_width(draw::HEADING, t.inputs).min(title_max);
        let nx = x + PAD + title_w + 12.0;
        let nw = tools_x - 12.0 - nx;
        if nw >= 48.0 {
            ui.label_fit(Font::regular(13.0), draw::DANGER_TEXT, nx, y + HEADER_BASELINE, nw, &panel.notice);
        }
    }

    let strip_top = y + STRIPS_TOP;
    let strip_h = (y + h - PAD - strip_top).max(180.0);
    // The card-wide band the strips live in.
    let area = Rect::new(x, strip_top, w, strip_h);
    let a_slots = super::outputs::present_a(&snap.graph);
    let source = source_label(&panel.config.mic_source, &snap.graph, t);
    if panel.config.input_list {
        draw_rows(panel, ui, peaks, &a_slots, &source, area);
    } else {
        draw_columns(panel, ui, peaks, &a_slots, &source, area);
    }
}

/// Measures every vertical strip shares, so the microphone and the channels line up.
struct Columns {
    top: f64,
    /// One width for the microphone and every channel, so the columns match.
    strip_w: f64,
    strip_h: f64,
    cap_h: f64,
    /// Fader center, from a strip's left edge.
    fader_inset: f64,
    stack: Stack,
}

impl Columns {
    fn strip(&self, x: f64) -> Rect {
        Rect::new(x, self.top, self.strip_w, self.strip_h)
    }

    /// Fader with its meter, the route buttons left of it, and Mute below.
    /// The fader is grabbed `reach` pixels beyond the capsule and the meter.
    fn controls(&self, ui: &mut Ui, strip_x: f64, source: Source, fader: &Fader, routes: Routes, a_slots: &[usize], reach: f64) {
        let fader_cx = strip_x + self.fader_inset;
        let cap_top = self.top + CAP_TOP;
        let geom = draw::paint_fader(ui.cr, ui.fonts, fader_cx, cap_top, self.cap_h, fader);
        let capsule = geom.capsule;
        let grab = Rect::new(capsule.x - reach, capsule.y, geom.meter_right - capsule.x + 2.0 * reach, capsule.h);
        ui.fader(fader_of(source), grab, false);
        self.stack.draw(ui, fader_cx - 16.0 - STACK_GAP - self.stack.w, cap_top, source, routes, a_slots);
        mute_button(ui, fader_cx, cap_top + self.cap_h, self.strip_w - 16.0, source, fader.muted);
    }
}

/// Vertical strips: the microphone stays put, the channels scroll sideways beside it.
fn draw_columns(panel: &mut Panel, ui: &mut Ui, peaks: &HashMap<String, f32>, a_slots: &[usize], source: &str, area: Rect) {
    let Rect { x, y: strip_top, w, h: strip_h } = area;
    let cap_h = (strip_h - CAP_TOP - draw::FADER_FOOT).max(80.0);
    let stack = Stack::fit(cell_count(a_slots), cap_h);
    let strip_w = strip_width(ui, &stack);
    let fader_inset = STRIP_PAD + stack.w + STACK_GAP + 16.0;
    let cols = Columns { top: strip_top, strip_w, strip_h, cap_h, fader_inset, stack };

    let mic = cols.strip(x + PAD);
    ui.fill(mic, R_SURFACE, INSET);
    source_picker(ui, mic.inset(8.0).take_top(PICK_H), source);
    let mic_levels = [peaks.get("mic").copied().unwrap_or(0.0)];
    let config = &panel.config;
    let fader = Fader { db: config.mic_gain_db, levels: &mic_levels, meters: config.meters, muted: config.mic_muted, glyph: FaderGlyph::Mic, snaps: true };
    let routes = Routes { bits: config.mic_routes, to_pc: config.mic_to_pc, fx1: config.mic_send_fx1, fx2: config.mic_send_fx2 };
    cols.controls(ui, mic.x, Source::Mic, &fader, routes, a_slots, 4.0);

    let list_x = mic.x + strip_w + GAP;
    let list_w = x + w - PAD - list_x;
    let content = panel.config.channels.len() as f64 * (strip_w + GAP) + ADD_W;
    let max_scroll = (content - list_w).max(0.0);
    panel.scroll = panel.scroll.clamp(0.0, max_scroll);
    if list_w <= 1.0 {
        return;
    }
    let list = Rect::new(list_x, strip_top, list_w, strip_h);
    ui.clip(list);
    // Hit areas of a half-hidden channel stop at the list's edge, so they cannot
    // catch a click meant for the drawer or the microphone strip.
    ui.bounds = Some(HitBounds::columns(list_x, list_x + list_w).cutting_faders());
    panel.strip = Some(list);
    panel.visible_channels.clear();
    let channels = panel.config.channels.clone();
    let mut cx = list_x - panel.scroll;
    for (index, channel) in channels.iter().enumerate() {
        if cx + strip_w >= list_x && cx <= list_x + list_w {
            panel.visible_channels.push(channel.id.clone());
            let levels = stereo_levels(peaks, &channel.id);
            channel_strip(panel, ui, &cols, index, channel, cx, &levels, a_slots);
        }
        cx += strip_w + GAP;
    }
    let add = Rect::square(cx, strip_top + (strip_h - ADD_W) / 2.0, ADD_W);
    let room = channels.len() < config::MAX_CHANNELS;
    add_button(ui, add, room);
    if room {
        ui.hot(Hit::AddChannel, add);
    }
    ui.bounds = None;
    ui.unclip();

    if max_scroll > 0.0 {
        let thumb_w = draw::scroll_thumb(list_w, content, 32.0);
        let thumb_x = list_x + (list_w - thumb_w) * (panel.scroll / max_scroll);
        let bar_y = strip_top + strip_h - 8.0;
        draw::paint_scroll_thumb(ui.cr, Rect::new(thumb_x, bar_y, thumb_w, 4.0));
        panel.scrollbar = Some(Scrollbar { rect: Rect::new(list_x, bar_y - 8.0, list_w, 16.0), thumb_len: thumb_w, max_scroll, vertical: false });
    }
}

/// One channel as a vertical strip with its left edge at `x`.
fn channel_strip(panel: &mut Panel, ui: &mut Ui, cols: &Columns, index: usize, channel: &Channel, x: f64, levels: &[f32], a_slots: &[usize]) {
    let t = ui.t;
    let strip = cols.strip(x);
    let (top, strip_w) = (strip.y, strip.w);
    ui.fill(strip, R_SURFACE, INSET);
    if panel.channel_armed(&channel.id) {
        let confirm = strip.inset(8.0).take_top(32.0);
        ui.fill(confirm, 16.0, draw::DANGER_FILL);
        let confirm_w = ui.text_width(CONFIRM, t.delete_confirm).min(strip_w - 28.0);
        ui.label_fit(CONFIRM, draw::DANGER_TEXT, x + (strip_w - confirm_w) / 2.0, top + 29.0, strip_w - 28.0, t.delete_confirm);
        ui.hot(Hit::RemoveChannel(index), confirm);
    } else {
        // Icon and name are centered as one group and stay clear of the ×.
        let with_icon = !channel.app.is_empty();
        let icon_room = if with_icon { ICON_S + 6.0 } else { 0.0 };
        let name_room = strip_w - 52.0 - icon_room;
        let name_w = ui.text_width(CHANNEL_NAME, &channel.name).min(name_room);
        let group = icon_room + name_w;
        let title_x = (x + (strip_w - group) / 2.0).min(x + strip_w - 38.0 - group);
        if with_icon {
            super::art::desktop_icon(panel, ui, &channel.app, &channel.binary, title_x, top + STRIP_TITLE_Y - 15.0, ICON_S);
        }
        ui.label_fit(CHANNEL_NAME, draw::accent(), title_x + icon_room, top + STRIP_TITLE_Y, name_room, &channel.name);
        ui.hot(Hit::RenameChannel(index), Rect::new(x + 8.0, top + 8.0, strip_w - 48.0, 32.0));
        let remove = Rect::square(x + strip_w - 40.0, top + 8.0, REMOVE_S);
        remove_glyph(ui, remove);
        ui.hot(Hit::RemoveChannel(index), remove);
    }
    let fader = Fader { db: channel.gain_db, levels, meters: panel.config.meters, muted: channel.muted, glyph: FaderGlyph::Speaker, snaps: true };
    cols.controls(ui, x, Source::Channel(index), &fader, Routes::of(channel), a_slots, 2.0);
}

const ROW_H: f64 = 108.0;
const LIST_GAP: f64 = 8.0;
/// Title baseline inside a list row.
const TITLE_Y: f64 = 22.0;
const SLOT_H: f64 = 40.0;
const ROW_PAD: f64 = 16.0;

/// Vertical list. The microphone is the first row and scrolls with the channels.
fn draw_rows(panel: &mut Panel, ui: &mut Ui, peaks: &HashMap<String, f32>, a_slots: &[usize], source: &str, area: Rect) {
    let row_x = area.x + PAD;
    let row_w = area.w - 2.0 * PAD;
    let list_top = area.y;
    let list_h = area.h.max(72.0);
    let list = Rect::new(row_x, list_top, row_w, list_h);
    let channels = panel.config.channels.clone();
    let content = (channels.len() as f64 + 1.0) * (ROW_H + LIST_GAP) + 56.0;
    let max_scroll = (content - list_h).max(0.0);
    panel.scroll = panel.scroll.clamp(0.0, max_scroll);
    panel.strip = Some(list);
    ui.clip(list);
    ui.bounds = Some(HitBounds::rect(list));
    panel.visible_channels.clear();
    let visible = |ry: f64| ry + ROW_H >= list_top && ry <= list_top + list_h;
    let mut ry = list_top - panel.scroll;
    if visible(ry) {
        let mic_levels = [peaks.get("mic").copied().unwrap_or(0.0)];
        mic_row(panel, ui, &mic_levels, a_slots, source, Rect::new(row_x, ry, row_w, ROW_H));
    }
    ry += ROW_H + LIST_GAP;
    for (index, channel) in channels.iter().enumerate() {
        if visible(ry) {
            panel.visible_channels.push(channel.id.clone());
            let levels = stereo_levels(peaks, &channel.id);
            channel_row(panel, ui, index, channel, &levels, a_slots, Rect::new(row_x, ry, row_w, ROW_H));
        }
        ry += ROW_H + LIST_GAP;
    }
    let room = channels.len() < config::MAX_CHANNELS;
    let add_s = 56.0;
    let add = Rect::square(row_x + (row_w - add_s) / 2.0, ry, add_s);
    add_button(ui, add, room);
    if room {
        ui.hot(Hit::AddChannel, add);
    }
    ui.bounds = None;
    ui.unclip();

    if max_scroll > 0.0 {
        let bar_x = area.x + area.w - 11.0;
        let thumb_h = draw::scroll_thumb(list_h, content, 32.0);
        let thumb_y = list_top + (list_h - thumb_h) * (panel.scroll / max_scroll);
        draw::paint_scroll_thumb(ui.cr, Rect::new(bar_x, thumb_y, 4.0, thumb_h));
        panel.scrollbar = Some(Scrollbar { rect: Rect::new(bar_x - 8.0, list_top, 16.0, list_h), thumb_len: thumb_h, max_scroll, vertical: true });
    }
}

fn mic_row(panel: &Panel, ui: &mut Ui, levels: &[f32], a_slots: &[usize], source: &str, row: Rect) {
    ui.fill(row, R_SURFACE, INSET);
    source_picker(ui, Rect::new(row.x + 8.0, row.y + 4.0, (row.w - 16.0).min(320.0), PICK_H), source);
    let config = &panel.config;
    let fader = Fader { db: config.mic_gain_db, levels, meters: config.meters, muted: config.mic_muted, glyph: FaderGlyph::Mic, snaps: true };
    let routes = Routes { bits: config.mic_routes, to_pc: config.mic_to_pc, fx1: config.mic_send_fx1, fx2: config.mic_send_fx2 };
    row_controls(ui, row, Source::Mic, &fader, routes, a_slots);
}

fn channel_row(panel: &mut Panel, ui: &mut Ui, index: usize, channel: &Channel, levels: &[f32], a_slots: &[usize], row: Rect) {
    let t = ui.t;
    let Rect { x, y, w, .. } = row;
    ui.fill(row, R_SURFACE, INSET);
    if panel.channel_armed(&channel.id) {
        let confirm = Rect::new(x + 8.0, y + 6.0, w - 56.0, 28.0);
        ui.fill(confirm, 14.0, draw::DANGER_FILL);
        ui.label_fit(CONFIRM, draw::DANGER_TEXT, x + ROW_PAD, y + 25.0, w - 80.0, t.delete_confirm);
        ui.hot(Hit::RemoveChannel(index), confirm);
    } else {
        let with_icon = !channel.app.is_empty();
        let icon_room = if with_icon { ICON_S + 8.0 } else { 0.0 };
        if with_icon {
            super::art::desktop_icon(panel, ui, &channel.app, &channel.binary, x + ROW_PAD, y + TITLE_Y - 15.0, ICON_S);
        }
        ui.label_fit(CHANNEL_NAME, draw::accent(), x + ROW_PAD + icon_room, y + TITLE_Y, 180.0, &channel.name);
        ui.hot(Hit::RenameChannel(index), Rect::new(x + 8.0, y + 4.0, 220.0, 30.0));
    }
    let remove = Rect::square(x + w - 40.0, y + 4.0, REMOVE_S);
    remove_glyph(ui, remove);
    ui.hot(Hit::RemoveChannel(index), remove);
    let fader = Fader { db: channel.gain_db, levels, meters: panel.config.meters, muted: channel.muted, glyph: FaderGlyph::Speaker, snaps: true };
    row_controls(ui, row, Source::Channel(index), &fader, Routes::of(channel), a_slots);
}

/// The sideways fader of a list row, then Mute and the route buttons under it.
fn row_controls(ui: &mut Ui, row: Rect, source: Source, fader: &Fader, routes: Routes, a_slots: &[usize]) {
    let Rect { x, y, w, .. } = row;
    let slot = Rect::new(x + ROW_PAD - 4.0, y + 36.0, w - 2.0 * ROW_PAD + 4.0, SLOT_H);
    let geom = draw::paint_side_fader(ui.cr, ui.fonts, slot, fader);
    ui.fader(fader_of(source), geom.capsule, true);
    let used = row_mute(ui, x + ROW_PAD, y + ROW_H - 36.0, source, fader.muted);
    route_row(ui, x + ROW_PAD + used + 16.0, y + ROW_H - 32.0, x + w - ROW_PAD, source, routes, a_slots);
}

/// The fader that sets the level of `source`.
fn fader_of(source: Source) -> FaderId {
    match source {
        Source::Mic => FaderId::Mic,
        Source::Channel(index) => FaderId::Channel(index),
    }
}

/// Left and right peaks of a channel, as the engine publishes them.
fn stereo_levels(peaks: &HashMap<String, f32>, id: &str) -> [f32; 2] {
    let side = |suffix: &str| peaks.get(&format!("{id}:{suffix}")).copied().unwrap_or(0.0);
    [side("l"), side("r")]
}

/// The microphone source as a dropdown at the top of the mic strip.
fn source_picker(ui: &mut Ui, rect: Rect, label: &str) {
    let Rect { x, y, w, .. } = rect;
    ui.fill(rect, draw::R_CONTROL, ui.control_fill(rect, draw::CONTROL));
    let glyph = 16.0;
    ui.icon(draw::MIC_PATH, 256.0, x + 10.0, y + (PICK_H - glyph) / 2.0, glyph, draw::accent());
    ui.label_fit(Font::bold(14.0), TEXT, x + 32.0, y + PICK_H / 2.0 + 5.0, w - 32.0 - 28.0, label);
    draw::chevron(ui.cr, x + w - 22.0, y + PICK_H / 2.0 - 3.0, 12.0, MUTED, false, false);
    ui.hot(Hit::MicPicker, rect);
}

/// The × that removes a channel. Quiet until the pointer is on it.
fn remove_glyph(ui: &Ui, rect: Rect) {
    let look = ui.look(rect);
    if look != Look::Rest {
        let fill = if look == Look::Down { draw::CONTROL_DOWN } else { draw::CONTROL_HOVER };
        ui.fill(rect, REMOVE_S / 2.0, fill);
    }
    let s = 18.0;
    let ink = if look == Look::Rest { MUTED } else { TEXT };
    ui.icon(draw::CLOSE_PATH, 1024.0, rect.x + (REMOVE_S - s) / 2.0, rect.y + (REMOVE_S - s) / 2.0, s, ink);
}

/// The add-channel tile: the accent square glyph on a tonal tile.
/// Without `room` for another channel it is dimmed and ignores the pointer.
fn add_button(ui: &Ui, tile: Rect, room: bool) {
    let s = tile.w;
    let fill = if room { ui.control_fill(tile, INSET) } else { INSET };
    ui.fill(tile, R_SURFACE, fill);
    let glyph = (s * 0.5).round();
    ui.icon(draw::ADD_PATH, 28.0, tile.x + (s - glyph) / 2.0, tile.y + (s - glyph) / 2.0, glyph, if room { draw::accent() } else { MUTED });
}

/// As wide as a strip's content: route buttons, fader, meter and its scale,
/// with `STRIP_PAD` on both sides. Anything wider only adds empty margins.
fn strip_width(ui: &Ui, stack: &Stack) -> f64 {
    let scale_w = ["+6", "−12", "−24", "−48"].iter().map(|mark| ui.text_width(Font::regular(10.0), mark)).fold(0.0, f64::max);
    (2.0 * STRIP_PAD + stack.w + STACK_GAP + FADER_AND_METER_W + 4.0 + scale_w).ceil()
}

/// Snap and this side's orientation. `outputs` picks which orientation the second button owns.
/// The wide window is a whole-app setting, so its button sits in the app header.
pub(crate) fn header_tools(panel: &mut Panel, ui: &mut Ui, x: f64, y: f64, outputs: bool) {
    let step = TOOL_S + 8.0;
    let snap = panel.config.snap;
    tool_button(ui, Rect::square(x, y, TOOL_S), draw::MAGNET_PATH, 16.0, snap, Hit::Snap(!snap));
    // The glyph shows the current orientation; a press switches to the other one.
    let rows = if outputs { panel.config.list } else { panel.config.input_list };
    let view_path = if rows { draw::LIST_PATH } else { draw::COLUMNS_PATH };
    let switch = if outputs { Hit::OutputColumns(!rows) } else { Hit::InputRows(!rows) };
    tool_button(ui, Rect::square(x + step, y, TOOL_S), view_path, 512.0, rows, switch);
}

/// Where one strip goes: A bits, the PC, B bits and the two sends.
#[derive(Clone, Copy)]
struct Routes {
    bits: u8,
    to_pc: bool,
    fx1: bool,
    fx2: bool,
}

impl Routes {
    fn of(channel: &Channel) -> Self {
        Self { bits: channel.routes, to_pc: channel.to_pc, fx1: channel.send_fx1, fx2: channel.send_fx2 }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cell {
    A,
    Pc,
    B,
    Fx1,
    Fx2,
}

/// One route button: what it is, its caption, what a press does, whether it is on,
/// and for a listen output the device it plays on.
struct RouteCell {
    kind: Cell,
    caption: String,
    hit: Hit,
    on: bool,
    device: Device,
}

/// Buttons beside one fader: the listen outputs that are present, the PC, B1..B4, FX1 and FX2.
fn cell_count(a_slots: &[usize]) -> usize {
    a_slots.len() + 1 + config::B_SLOTS + 2
}

fn route_cells(from: Source, routes: Routes, a_slots: &[usize]) -> Vec<RouteCell> {
    let mut cells = Vec::with_capacity(cell_count(a_slots));
    cells.push(RouteCell { kind: Cell::Pc, caption: String::new(), hit: Hit::Route { from, to: BusId::Pc }, on: routes.to_pc, device: Device::Other });
    for &index in a_slots {
        cells.push(RouteCell {
            kind: Cell::A,
            caption: String::new(),
            hit: Hit::Route { from, to: BusId::A(index) },
            on: config::route_on(routes.bits, true, index),
            device: super::outputs::a_device(index),
        });
    }
    for index in 0..config::B_SLOTS {
        cells.push(RouteCell {
            kind: Cell::B,
            caption: format!("B{}", index + 1),
            hit: Hit::Route { from, to: BusId::B(index) },
            on: config::route_on(routes.bits, false, index),
            device: Device::Other,
        });
    }
    cells.push(RouteCell { kind: Cell::Fx1, caption: "FX1".into(), hit: Hit::Insert { from, fx: 0 }, on: routes.fx1, device: Device::Other });
    cells.push(RouteCell { kind: Cell::Fx2, caption: "FX2".into(), hit: Hit::Insert { from, fx: 1 }, on: routes.fx2, device: Device::Other });
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

    fn draw(&self, ui: &mut Ui, x: f64, y: f64, from: Source, routes: Routes, a_slots: &[usize]) {
        let mut cy = y;
        let mut last = None;
        for cell in route_cells(from, routes, a_slots) {
            if last.is_some_and(|kind| group(kind) != group(cell.kind)) {
                cy += self.group_gap;
            }
            last = Some(cell.kind);
            route_cell(ui, Rect::new(x, cy, self.w, self.h), cell);
            cy += self.h + self.gap;
        }
    }
}

/// FX1 and FX2 sit together. Every other kind is its own group.
fn group(cell: Cell) -> Cell {
    if cell == Cell::Fx2 { Cell::Fx1 } else { cell }
}

/// The same buttons in one row, for list view. They narrow to fit before `right`.
fn route_row(ui: &mut Ui, x: f64, y: f64, right: f64, from: Source, routes: Routes, a_slots: &[usize]) {
    let cells = route_cells(from, routes, a_slots);
    let gaps = 3.0 * GROUP_GAP + (cells.len() - 1) as f64 * ROUTE_GAP;
    let bw = ((right - x - gaps) / cells.len() as f64).clamp(14.0, 40.0);
    let h = 24.0;
    let mut cx = x;
    let mut last = None;
    for cell in cells {
        if last.is_some_and(|kind| group(kind) != group(cell.kind)) {
            cx += GROUP_GAP;
        }
        last = Some(cell.kind);
        route_cell(ui, Rect::new(cx, y, bw, h), cell);
        cx += bw + ROUTE_GAP;
    }
}

fn route_cell(ui: &mut Ui, rect: Rect, cell: RouteCell) {
    let Rect { x, y, w, h } = rect;
    let kind = cell.kind;
    let (fill, ink) = if cell.on {
        match kind {
            Cell::A => (TEXT, draw::BG),
            Cell::Pc => (draw::accent(), draw::on_accent()),
            Cell::B => (draw::mix(draw::accent(), draw::CONTROL, 0.45), TEXT),
            Cell::Fx1 => (draw::FX1, draw::BG),
            Cell::Fx2 => (draw::FX2, draw::BG),
        }
    } else {
        let ink = match kind {
            Cell::Pc => draw::accent(),
            Cell::Fx1 => draw::FX1,
            Cell::Fx2 => draw::FX2,
            _ => MUTED,
        };
        (ui.control_fill(rect, draw::CONTROL), ink)
    };
    ui.fill(rect, 8.0_f64.min(h / 2.0), fill);
    let s = (h - 6.0).min(20.0);
    let (gx, gy) = (x + (w - s) / 2.0, y + (h - s) / 2.0);
    let drawn = match kind {
        Cell::Pc => {
            draw::pc_icon(ui.cr, gx, gy, s, ink);
            true
        }
        Cell::A => draw::device_icon(ui.cr, cell.device, gx, gy, s, ink),
        _ => false,
    };
    if !drawn {
        let size = if h < 22.0 { 11.0 } else { 12.0 };
        ui.label_centered(Font::bold(size), ink, x, w, y + h / 2.0, &cell.caption);
    }
    ui.hot(cell.hit, rect);
}

/// Mute under a vertical fader, centered on the strip. `foot` is the capsule's bottom.
fn mute_button(ui: &mut Ui, cx: f64, foot: f64, max_w: f64, source: Source, muted: bool) {
    let btn_w = max_w.min(96.0);
    let rect = Rect::new(cx - btn_w / 2.0, foot + draw::FADER_MUTE_Y, btn_w, draw::FADER_MUTE_H);
    let caption = ui.t.mute;
    pill(ui, caption, Hit::Mute(source), rect, muted);
}

/// Mute of a list row. Returns the width it takes.
fn row_mute(ui: &mut Ui, x: f64, y: f64, source: Source, muted: bool) -> f64 {
    let mute_w = 72.0;
    let caption = ui.t.mute;
    pill(ui, caption, Hit::Mute(source), Rect::new(x, y + 4.0, mute_w, 24.0), muted);
    mute_w
}

/// A small toggle. Red while `on`.
fn pill(ui: &mut Ui, caption: &str, hit: Hit, rect: Rect, on: bool) {
    let fill = if on { draw::DANGER } else { draw::CONTROL };
    let ink = if on { draw::BG } else { draw::SOFT };
    ui.fill(rect, 12.0, fill);
    ui.label_centered(PILL, ink, rect.x, rect.w, rect.y + rect.h / 2.0, caption);
    ui.hot(hit, rect);
}

/// A square icon button of the card header. Filled with the accent while `on`.
fn tool_button(ui: &mut Ui, rect: Rect, path: &str, view: f64, on: bool, hit: Hit) {
    let s = TOOL_S;
    let fill = if on {
        match ui.look(rect) {
            Look::Down => draw::accent_down(),
            Look::Hover => draw::accent_hover(),
            Look::Rest => draw::accent(),
        }
    } else {
        ui.control_fill(rect, draw::CONTROL)
    };
    ui.fill(rect, draw::R_CONTROL, fill);
    let glyph = 20.0;
    ui.icon(path, view, rect.x + (s - glyph) / 2.0, rect.y + (s - glyph) / 2.0, glyph, if on { draw::on_accent() } else { draw::SOFT });
    ui.hot(hit, rect);
}
