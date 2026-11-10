//! Outputs drawer: Mix Check at the top, then one container per output.
//! The PC streaming microphone comes first, then the Frame speakers, Bluetooth and
//! USB while one is connected, then the virtual microphones B1..B4.

use super::{BusId, FaderId, Hit, HitBounds, Panel, Pick, Ui};
use crate::config::{self, Bus};
use crate::draw::{self, ButtonKind, Device, Fader, Font, Look, Rect, Rgb, BUTTON, DANGER, DANGER_TEXT, HEADING, INSET, MUTED, TEXT};
use crate::graph::Graph;
use crate::i18n::Text;
use crate::worker::{CheckPhase, Snapshot};
use std::collections::HashMap;

const PAD: f64 = 16.0;
const BOX_PAD: f64 = 12.0;
const BOX_H: f64 = 128.0;
/// One output when the list button stands them up as columns.
const COL_W: f64 = 148.0;
const BOX_GAP: f64 = 8.0;
const MUTE_S: f64 = 28.0;
const MUTE_RADIUS: f64 = 8.0;
const CHECK_H: f64 = 44.0;
/// Gap between the Record row and the Play row of Mix Check.
const ROW_GAP: f64 = 8.0;
const MENU_ROW: f64 = 40.0;
/// Line height of the wrapped hints.
const HINT_LINE: f64 = 18.0;
/// Shortest scrollbar thumb.
const THUMB_MIN: f64 = 32.0;
/// Height of the sideways fader inside an output box.
const BOX_FADER_H: f64 = 36.0;
/// Capsule top inside an output column, below its icon and name.
const COL_CAP_TOP: f64 = 52.0;

/// Clocks, hints and the "M" of a mute button.
const SMALL: Font = Font::regular(13.0);
const SMALL_BOLD: Font = Font::bold(13.0);
/// Status captions and selector labels.
const STATUS: Font = Font::bold(14.0);

fn buses(graph: &Graph) -> Vec<BusId> {
    let mut list = vec![BusId::Pc];
    list.extend(present_a(graph).into_iter().map(BusId::A));
    list.extend((0..config::B_SLOTS).map(BusId::B));
    list
}

/// Listen slots to show: the Frame speakers always, Bluetooth and USB while connected.
pub(crate) fn present_a(graph: &Graph) -> Vec<usize> {
    let sinks = graph.output_sinks();
    (0..config::A_SLOTS).filter(|&index| index == 0 || sinks[index].is_some()).collect()
}

pub(crate) fn draw(panel: &mut Panel, ui: &mut Ui, snap: &Snapshot, peaks: &HashMap<String, f32>) {
    let t = ui.t;
    let card = panel.stage().drawer;
    let Rect { x, y, w, h } = card;
    if w < 80.0 || h <= 0.0 {
        return;
    }
    ui.card(card);
    let graph = &snap.graph;
    let left = x + PAD;
    let inner = w - 2.0 * PAD;
    let tools_x = x + w - PAD - super::mix::TOOLS_W;
    let title_max = (tools_x - 12.0 - left).max(24.0);
    ui.label_fit(HEADING, TEXT, left, y + 40.0, title_max, t.outputs);
    super::mix::header_tools(panel, ui, tools_x, y + 8.0, true);

    let row_y = y + 60.0;
    let [record_sel, play_sel] = check_rows(panel, ui, snap, left, row_y, inner);
    let status_y = row_y + 2.0 * CHECK_H + ROW_GAP + 22.0;
    let mut bottom = check_status(ui, snap, peaks, left, status_y, inner);
    if speakers_without_echo(snap) {
        bottom = echo_hint(ui, left, bottom + 10.0, inner);
    }

    let divider = bottom + 14.0;
    ui.cr.set_source_rgb(draw::DIVIDER.r, draw::DIVIDER.g, draw::DIVIDER.b);
    ui.cr.rectangle(left, divider, inner, 1.0);
    let _ = ui.cr.fill();

    let top = divider + 14.0;
    let end = y + h - PAD;
    let list = buses(graph);
    let view_h = (end - top).max(0.0);
    let view = Rect::new(x, top, w, view_h);
    if panel.config.list {
        output_columns(panel, ui, peaks, &list, view);
    } else if view_h > 0.0 {
        output_boxes(panel, ui, graph, peaks, &list, view, end);
    }

    // The open list is drawn last, over the containers.
    if panel.check_open {
        if let Some(rect) = record_sel {
            menu(panel, ui, graph, rect, Pick::Record);
        }
    }
    if panel.play_open {
        if let Some(rect) = play_sel {
            menu(panel, ui, graph, rect, Pick::Play);
        }
    }
}

/// The warning that the speakers play without echo cancelling. Returns its bottom edge.
fn echo_hint(ui: &Ui, x: f64, y: f64, w: f64) -> f64 {
    let lines = ui.wrap(SMALL, w - 24.0, 4, ui.t.echo_hint);
    let h = lines.len() as f64 * HINT_LINE + 20.0;
    ui.fill(Rect::new(x, y, w, h), draw::R_CONTROL, draw::DANGER_FILL);
    for (i, line) in lines.iter().enumerate() {
        ui.label_fit(SMALL, DANGER_TEXT, x + 12.0, y + 24.0 + i as f64 * HINT_LINE, w - 24.0, line);
    }
    y + h
}

/// Outputs as boxes, one under the other, scrolling vertically inside `view`.
/// `end` is the bottom of the visible list.
fn output_boxes(panel: &mut Panel, ui: &mut Ui, graph: &Graph, peaks: &HashMap<String, f32>, list: &[BusId], view: Rect, end: f64) {
    let Rect { x, y: top, w, h: view_h } = view;
    let content = list.len() as f64 * (BOX_H + BOX_GAP) - BOX_GAP;
    let max_scroll = (content - view_h).max(0.0);
    panel.out_scroll = panel.out_scroll.min(max_scroll);
    panel.out_list = Some(view);
    ui.clip(view);
    ui.bounds = Some(HitBounds::rows(top, end));
    let mut by = top - panel.out_scroll;
    for &bus in list {
        if by + BOX_H > top && by < end {
            output_box(panel, ui, graph, peaks, bus, x + PAD, by, w - 2.0 * PAD);
        }
        by += BOX_H + BOX_GAP;
    }
    ui.bounds = None;
    ui.unclip();
    if max_scroll > 0.0 {
        let track_h = view_h;
        let thumb = draw::scroll_thumb(track_h, content, THUMB_MIN);
        let ty = top + (track_h - thumb) * (panel.out_scroll / max_scroll);
        draw::paint_scroll_thumb(ui.cr, Rect::new(x + w - 7.0, ty, 4.0, thumb));
    }
}

/// Record or Stop beside the output it records, then Play or Pause beside the
/// listen output the take plays on. Returns both selector rects.
fn check_rows(panel: &mut Panel, ui: &mut Ui, snap: &Snapshot, x: f64, y: f64, w: f64) -> [Option<Rect>; 2] {
    let t = ui.t;
    let recording = snap.check == CheckPhase::Recording || panel.record_pending;
    let button_w = [t.record, t.stop, t.play, t.pause].iter().map(|caption| ui.text_width(BUTTON, caption)).fold(0.0, f64::max) + 56.0;
    let button_w = button_w.max(112.0);
    let sel_x = x + button_w + 8.0;
    let sel_w = x + w - sel_x;

    // Record or Stop, and the output it records.
    let button = Rect::new(x, y, button_w, CHECK_H);
    let caption = if recording { t.stop } else { t.record };
    let caption_w = ui.text_width(BUTTON, caption);
    let kind = if recording { ButtonKind::DangerArmed } else { ButtonKind::Danger };
    ui.button(button, "", kind);
    let ink = if recording { draw::on_accent() } else { DANGER_TEXT };
    let mark_x = x + (button_w - caption_w - 24.0) / 2.0;
    let mid = y + CHECK_H / 2.0;
    if recording {
        draw::stop_square(ui.cr, mark_x + 7.0, mid, 13.0, ink);
    } else {
        draw::record_dot(ui.cr, mark_x + 7.0, mid, 7.0, DANGER);
    }
    ui.label_mid(BUTTON, ink, mark_x + 24.0, mid, caption);
    ui.hot(if recording { Hit::StopCheck } else { Hit::Record }, button);
    let record_bus = BusId::parse(&panel.config.check_output);
    let record_label = if record_bus == BusId::Pc { t.pc_short.to_string() } else { output_label(panel, t, record_bus) };
    let record_sel = selector(panel, ui, Rect::new(sel_x, y, sel_w, CHECK_H), record_bus, &record_label, recording, Pick::Record);

    // Play or Pause, and the listen output the take plays on.
    let y = y + CHECK_H + ROW_GAP;
    let button = Rect::new(x, y, button_w, CHECK_H);
    let mid = y + CHECK_H / 2.0;
    let playing = snap.check == CheckPhase::Playing;
    let can_play = !recording && (snap.has_take || playing);
    let fill = if can_play { ui.control_fill(button, draw::CONTROL) } else { INSET };
    ui.fill(button, CHECK_H / 2.0, fill);
    let ink = if can_play { TEXT } else { draw::mix(MUTED, INSET, 0.5) };
    let caption = if playing { t.pause } else { t.play };
    let caption_w = ui.text_width(BUTTON, caption);
    let mark_x = x + (button_w - caption_w - 24.0) / 2.0;
    if playing {
        ui.fill(Rect::new(mark_x + 1.0, mid - 7.0, 4.5, 14.0), 1.5, ink);
        ui.fill(Rect::new(mark_x + 8.5, mid - 7.0, 4.5, 14.0), 1.5, ink);
        ui.hot(Hit::PauseTake, button);
    } else {
        draw::play_triangle(ui.cr, mark_x + 8.0, mid, 14.0, ink);
        if can_play {
            ui.hot(Hit::PlayTake, button);
        }
    }
    ui.label_mid(BUTTON, ink, mark_x + 24.0, mid, caption);
    let play_bus = play_bus(panel, &snap.graph);
    let play_label = output_label(panel, t, play_bus);
    let play_sel = selector(panel, ui, Rect::new(sel_x, y, sel_w, CHECK_H), play_bus, &play_label, false, Pick::Play);
    [record_sel, play_sel]
}

/// One Mix Check dropdown button. Returns its rect, or None when there is no room.
fn selector(panel: &Panel, ui: &mut Ui, rect: Rect, bus: BusId, label: &str, busy: bool, pick: Pick) -> Option<Rect> {
    let Rect { x, y, w, h } = rect;
    if w < 80.0 {
        return None;
    }
    let open = match pick {
        Pick::Record => panel.check_open,
        Pick::Play => panel.play_open,
    };
    let fill = if busy {
        INSET
    } else if open {
        draw::CONTROL_DOWN
    } else {
        ui.control_fill(rect, draw::CONTROL)
    };
    ui.fill(rect, draw::R_CONTROL, fill);
    let mid = y + h / 2.0;
    let ink = if busy { MUTED } else { TEXT };
    let text_x = choice_mark(ui, bus, x + 12.0, mid, ink);
    ui.label_fit(STATUS, ink, text_x, mid + 5.0, x + w - 30.0 - text_x, label);
    draw::chevron(ui.cr, x + w - 24.0, mid - 2.0, 12.0, MUTED, false, open);
    if !busy {
        ui.hot(Hit::PickMenu(pick), rect);
    }
    Some(rect)
}

/// The listen output a take plays on: the chosen one while it is connected,
/// else the one that is the default output, else the Frame speakers.
fn play_bus(panel: &Panel, graph: &Graph) -> BusId {
    let present = present_a(graph);
    let chosen = panel.config.check_play.strip_prefix("a:").and_then(|index| index.parse::<usize>().ok());
    if let Some(index) = chosen.filter(|index| present.contains(index)) {
        return BusId::A(index);
    }
    let sinks = graph.output_sinks();
    present
        .into_iter()
        .find(|&index| sinks[index].as_deref() == Some(graph.default_sink.as_str()))
        .map(BusId::A)
        .unwrap_or(BusId::A(0))
}

/// What a listen slot plays on.
pub(crate) fn a_device(index: usize) -> Device {
    match index {
        0 => Device::Headset,
        1 => Device::Bluetooth,
        2 => Device::Usb,
        _ => Device::Other,
    }
}

/// PC icon, device icon, or the B tag in front of an output name. Returns where the name starts.
fn choice_mark(ui: &Ui, bus: BusId, x: f64, mid: f64, ink: Rgb) -> f64 {
    match bus {
        BusId::Pc => {
            draw::pc_icon(ui.cr, x, mid - 10.0, 20.0, draw::accent());
            x + 28.0
        }
        BusId::A(index) => {
            if !draw::device_icon(ui.cr, a_device(index), x, mid - 10.0, 20.0, ink) {
                ui.label_mid(SMALL_BOLD, ink, x, mid, &bus.tag());
            }
            x + 30.0
        }
        BusId::B(_) => {
            ui.label_mid(SMALL_BOLD, draw::accent(), x, mid, &bus.tag());
            x + 30.0
        }
    }
}

/// Timer and level while recording, progress while a take plays, otherwise the hint.
/// `y` is the baseline of the first line. Returns the bottom edge.
fn check_status(ui: &Ui, snap: &Snapshot, peaks: &HashMap<String, f32>, x: f64, y: f64, w: f64) -> f64 {
    let t = ui.t;
    match snap.check {
        CheckPhase::Recording => {
            ui.circle(x + 6.0, y - 5.0, 6.0, DANGER);
            ui.label(STATUS, DANGER_TEXT, x + 18.0, y, t.recording);
            let elapsed = snap.check_secs.min(10.0);
            let clock = format!("{elapsed:.1}s / 10s");
            let cw = ui.text_width(SMALL, &clock);
            ui.label(SMALL, TEXT, x + w - cw, y, &clock);
            let peak = peaks.get("check").copied().unwrap_or(0.0);
            draw::paint_led_bar(ui.cr, Rect::new(x, y + 12.0, w, 8.0), peak as f64);
            y + 20.0
        }
        CheckPhase::Playing | CheckPhase::Paused => {
            let caption = if snap.check == CheckPhase::Playing { t.play } else { t.pause };
            ui.label(STATUS, TEXT, x, y, caption);
            let total = snap.take_secs.max(0.1);
            let clock = format!("{:.1}s / {:.1}s", snap.check_secs.min(total), total);
            let cw = ui.text_width(SMALL, &clock);
            ui.label(SMALL, TEXT, x + w - cw, y, &clock);
            let done = (snap.check_secs / total).clamp(0.0, 1.0);
            ui.fill(Rect::new(x, y + 12.0, w, 6.0), 3.0, INSET);
            if done > 0.0 {
                ui.fill(Rect::new(x, y + 12.0, (w * done).max(6.0), 6.0), 3.0, draw::accent());
            }
            y + 18.0
        }
        _ => {
            let (text, font, color) = if !snap.message.is_empty() {
                (snap.message.as_str(), SMALL_BOLD, DANGER_TEXT)
            } else {
                (t.mix_check_body, SMALL, MUTED)
            };
            let take = if snap.has_take && snap.message.is_empty() { format!("{:.1}s", snap.take_secs) } else { String::new() };
            let take_w = if take.is_empty() { 0.0 } else { ui.text_width(SMALL_BOLD, &take) + 12.0 };
            let lines = ui.wrap(font, w - take_w, 2, text);
            for (i, line) in lines.iter().enumerate() {
                ui.label_fit(font, color, x, y + i as f64 * HINT_LINE, w - take_w, line);
            }
            if !take.is_empty() {
                ui.label(SMALL_BOLD, TEXT, x + w - take_w + 12.0, y, &take);
            }
            y + (lines.len().max(1) as f64 - 1.0) * HINT_LINE + 4.0
        }
    }
}

/// Outputs stood up: one column per output, scrolling sideways inside `view`. The fader
/// and its mute share the column's center, the same way the input strips do.
fn output_columns(panel: &mut Panel, ui: &mut Ui, peaks: &HashMap<String, f32>, list: &[BusId], view: Rect) {
    let Rect { x, y, w, h } = view;
    if h < 80.0 {
        return;
    }
    let view_w = (w - 2.0 * PAD).max(0.0);
    let content = list.len() as f64 * (COL_W + BOX_GAP) - BOX_GAP;
    let max_scroll = (content - view_w).max(0.0);
    panel.out_scroll = panel.out_scroll.min(max_scroll);
    panel.out_list = Some(view);
    ui.clip(Rect::new(x, y, w, h - 8.0));
    ui.bounds = Some(HitBounds::columns(x, x + w));
    let mut cx = x + PAD - panel.out_scroll;
    for &bus in list {
        if cx + COL_W > x && cx < x + w {
            output_column(panel, ui, peaks, bus, Rect::new(cx, y, COL_W, h - 10.0));
        }
        cx += COL_W + BOX_GAP;
    }
    ui.bounds = None;
    ui.unclip();
    if max_scroll > 0.0 {
        let thumb = draw::scroll_thumb(view_w, content, THUMB_MIN);
        let tx = x + PAD + (view_w - thumb) * (panel.out_scroll / max_scroll);
        draw::paint_scroll_thumb(ui.cr, Rect::new(tx, y + h - 6.0, thumb, 4.0));
    }
}

/// The container of one output. The PC microphone is tinted and outlined with the accent.
fn container(ui: &Ui, rect: Rect, pc: bool) {
    let fill = if pc { draw::mix(draw::accent(), INSET, 0.12) } else { INSET };
    ui.fill(rect, draw::R_CONTROL, fill);
    if pc {
        ui.stroke(rect.inset(0.5), draw::R_CONTROL, draw::mix(draw::accent(), INSET, 0.45), 1.0);
    }
}

/// Left and right peaks of an output, as the engine publishes them.
fn bus_levels(peaks: &HashMap<String, f32>, bus: BusId) -> [f32; 2] {
    let id = bus.value();
    let side = |suffix: &str| peaks.get(&format!("out:{id}:{suffix}")).copied().unwrap_or(0.0);
    [side("l"), side("r")]
}

/// The square "M" button of an output. Red while the output is muted.
/// `cy` is the vertical center of the letter.
fn mute_button(ui: &mut Ui, rect: Rect, cy: f64, bus: BusId, muted: bool) {
    let fill = if muted { DANGER } else { ui.control_fill(rect, draw::CONTROL) };
    let ink = if muted { draw::BG } else { draw::SOFT };
    ui.fill(rect, MUTE_RADIUS, fill);
    ui.label_centered(SMALL_BOLD, ink, rect.x, rect.w, cy, "M");
    ui.hot(Hit::BusMute(bus), rect);
}

fn output_column(panel: &mut Panel, ui: &mut Ui, peaks: &HashMap<String, f32>, id: BusId, column: Rect) {
    let Rect { x, y, w, h } = column;
    let pc = id == BusId::Pc;
    container(ui, column, pc);
    let cx = x + w / 2.0;
    let _ = choice_mark(ui, id, cx - 10.0, y + 18.0, TEXT);
    let label = output_label(panel, ui.t, id);
    let name_font = Font::weight(!pc, 13.0);
    let room = w - 16.0;
    let tw = ui.text_width(name_font, &label).min(room);
    ui.label_fit(name_font, TEXT, cx - tw / 2.0, y + 42.0, room, &label);
    if let BusId::B(index) = id {
        ui.hot(Hit::BusRename(index), Rect::new(x + 8.0, y + 28.0, w - 16.0, 20.0));
    }
    let bus = bus_of(panel, id);
    let cap_h = (h - COL_CAP_TOP - draw::FADER_FOOT).max(48.0);
    let levels = bus_levels(peaks, id);
    let fader = Fader {
        db: bus.gain_db,
        levels: &levels,
        meters: panel.config.meters,
        muted: bus.muted,
        glyph: draw::FaderGlyph::Speaker,
        snaps: panel.config.snap,
    };
    let geom = draw::paint_fader(ui.cr, ui.fonts, cx, y + COL_CAP_TOP, cap_h, &fader);
    let grab = Rect::new(geom.capsule.x - 4.0, geom.capsule.y, geom.meter_right - geom.capsule.x + 8.0, geom.capsule.h);
    ui.fader(FaderId::Bus(id), grab, false);
    let mute = Rect::square(cx - MUTE_S / 2.0, y + COL_CAP_TOP + cap_h + draw::FADER_MUTE_Y, MUTE_S);
    mute_button(ui, mute, mute.y + MUTE_S / 2.0, id, bus.muted);
}

fn output_box(panel: &mut Panel, ui: &mut Ui, graph: &Graph, peaks: &HashMap<String, f32>, id: BusId, x: f64, y: f64, w: f64) {
    let t = ui.t;
    let pc = id == BusId::Pc;
    container(ui, Rect::new(x, y, w, BOX_H), pc);
    let name_mid = y + BOX_PAD + MUTE_S / 2.0;
    let text_x = choice_mark(ui, id, x + BOX_PAD, name_mid, TEXT);
    let label = output_label(panel, t, id);
    let mute = Rect::square(x + w - BOX_PAD - MUTE_S, y + BOX_PAD, MUTE_S);
    let name_w = mute.x - 8.0 - text_x;
    ui.label_fit(Font::weight(!pc, 15.0), TEXT, text_x, name_mid + 5.0, name_w, &label);
    let detail = if pc { t.frame_mix.to_string() } else { output_detail(graph, id) };
    if !detail.is_empty() {
        ui.label_fit(Font::regular(12.0), MUTED, text_x, name_mid + 21.0, name_w, &detail);
    }
    if let BusId::B(index) = id {
        ui.hot(Hit::BusRename(index), Rect::new(text_x, y + BOX_PAD - 4.0, name_w, MUTE_S + 8.0));
    }

    let bus = bus_of(panel, id);
    mute_button(ui, mute, name_mid, id, bus.muted);

    let levels = bus_levels(peaks, id);
    let fader = Fader {
        db: bus.gain_db,
        levels: &levels,
        meters: panel.config.meters,
        muted: bus.muted,
        glyph: draw::FaderGlyph::Speaker,
        snaps: panel.config.snap,
    };
    let area = Rect::new(x + BOX_PAD - 4.0, y + BOX_H - BOX_PAD - BOX_FADER_H, w - 2.0 * BOX_PAD + 4.0, BOX_FADER_H);
    let geom = draw::paint_side_fader(ui.cr, ui.fonts, area, &fader);
    ui.fader(FaderId::Bus(id), geom.capsule, true);
}

fn bus_of(panel: &Panel, id: BusId) -> Bus {
    match id {
        BusId::Pc => panel.config.pc.clone(),
        BusId::A(index) => panel.config.a[index.min(3)].clone(),
        BusId::B(index) => panel.config.b[index.min(3)].clone(),
    }
}

fn output_label(panel: &Panel, t: &Text, id: BusId) -> String {
    match id {
        BusId::Pc => t.pc_stream.to_string(),
        BusId::A(index) => match a_device(index) {
            Device::Bluetooth => t.bluetooth.to_string(),
            Device::Usb => t.usb.to_string(),
            _ => t.speakers.to_string(),
        },
        BusId::B(index) => {
            let name = &panel.config.b[index.min(3)].name;
            if name.is_empty() { format!("{} {}", t.virtual_mic, index + 1) } else { name.clone() }
        }
    }
}

/// The connected device's own name under the Bluetooth or USB title.
fn output_detail(graph: &Graph, id: BusId) -> String {
    match id {
        BusId::A(index) if index > 0 => graph.output_sinks()[index.min(config::A_SLOTS - 1)]
            .as_deref()
            .map(|name| device_label(graph, name))
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// The choices under the selector at `anchor`, drawn over the containers.
fn menu(panel: &mut Panel, ui: &mut Ui, graph: &Graph, anchor: Rect, pick: Pick) {
    let Rect { x, y, w, h } = anchor;
    let (list, current) = match pick {
        Pick::Record => (buses(graph), BusId::parse(&panel.config.check_output)),
        Pick::Play => (present_a(graph).into_iter().map(BusId::A).collect(), play_bus(panel, graph)),
    };
    let (_, ph) = panel.size_f();
    let top = y + h + 6.0;
    let rows = list.len() as f64;
    let menu_h = (rows * MENU_ROW + 12.0).min(ph - top - 16.0);
    let surface = Rect::new(x, top, w, menu_h);
    ui.fill(surface.inset(-2.0), draw::R_CONTROL + 2.0, draw::BG);
    ui.fill(surface, draw::R_CONTROL, draw::CARD);
    ui.stroke(surface.inset(0.5), draw::R_CONTROL, draw::DIVIDER, 1.0);
    ui.hot(Hit::PickMenuSurface(pick), surface);
    let mut ry = top + 6.0;
    for bus in list {
        if ry + MENU_ROW > top + menu_h {
            break;
        }
        let row = Rect::new(x + 6.0, ry, w - 12.0, MENU_ROW);
        let on = bus == current;
        let fill = if on {
            Some(draw::accent_tint())
        } else {
            match ui.look(row) {
                Look::Down => Some(draw::CONTROL_DOWN),
                Look::Hover => Some(draw::CONTROL_HOVER),
                Look::Rest => None,
            }
        };
        if let Some(fill) = fill {
            ui.fill(row, 8.0, fill);
        }
        let mid = ry + MENU_ROW / 2.0;
        let text_x = choice_mark(ui, bus, x + 16.0, mid, TEXT);
        let label = output_label(panel, ui.t, bus);
        ui.label_fit(Font::weight(on, 14.0), if on { draw::accent() } else { TEXT }, text_x, mid + 5.0, x + w - 16.0 - text_x, &label);
        ui.hot(Hit::PickBus(pick, bus), row);
        ry += MENU_ROW;
    }
}

fn device_label(graph: &Graph, name: &str) -> String {
    graph
        .nodes
        .iter()
        .find(|node| node.name == name)
        .map(|node| if node.description.is_empty() { node.name.clone() } else { node.description.clone() })
        .unwrap_or_else(|| name.to_string())
}

fn speakers_without_echo(snap: &Snapshot) -> bool {
    snap.graph.default_sink.to_lowercase().contains("speaker") && snap.echo_cancel == Some(false)
}
