//! Outputs drawer: Mix Check at the top, then one container per output.
//! The PC streaming microphone comes first, then the Frame speakers, Bluetooth and
//! USB while one is connected, then the virtual microphones B1..B4.

use super::{fill_round, Panel};
use crate::config::{self, Bus};
use crate::draw::{self, ButtonKind, Device, Fonts, Rgb, DANGER, DANGER_TEXT, INSET, MUTED, TEXT};
use crate::graph::Graph;
use crate::i18n::Text;
use crate::worker::{CheckPhase, Snapshot};
use cairo::Context;
use std::collections::HashMap;

const PAD: f64 = 16.0;
const BOX_PAD: f64 = 12.0;
const BOX_H: f64 = 128.0;
/// One output when the list button stands them up as columns.
const COL_W: f64 = 148.0;
const BOX_GAP: f64 = 8.0;
const MUTE_S: f64 = 28.0;
const CHECK_H: f64 = 44.0;
const MENU_ROW: f64 = 40.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Pc,
    A(usize),
    B(usize),
}

impl Kind {
    fn parse(value: &str) -> Kind {
        if let Some(rest) = value.strip_prefix("a:") {
            return Kind::A(rest.parse().unwrap_or(0));
        }
        if let Some(rest) = value.strip_prefix("b:") {
            return Kind::B(rest.parse().unwrap_or(0));
        }
        Kind::Pc
    }

    fn value(self) -> String {
        match self {
            Kind::Pc => "pc".into(),
            Kind::A(index) => format!("a:{index}"),
            Kind::B(index) => format!("b:{index}"),
        }
    }

    fn tag(self) -> String {
        match self {
            Kind::Pc => String::new(),
            Kind::A(index) => format!("A{}", index + 1),
            Kind::B(index) => format!("B{}", index + 1),
        }
    }
}

fn kinds(graph: &Graph) -> Vec<Kind> {
    let mut list = vec![Kind::Pc];
    list.extend(present_a(graph).into_iter().map(Kind::A));
    list.extend((0..config::B_SLOTS).map(Kind::B));
    list
}

/// Listen slots to show: the Frame speakers always, Bluetooth and USB while connected.
pub(crate) fn present_a(graph: &Graph) -> Vec<usize> {
    let sinks = graph.output_sinks();
    (0..config::A_SLOTS).filter(|&index| index == 0 || sinks[index].is_some()).collect()
}

pub(crate) fn draw(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot, peaks: &HashMap<String, f32>) {
    let (x, y, w, h) = panel.stage().check;
    if w < 80.0 || h <= 0.0 {
        return;
    }
    draw::card(cr, x, y, w, h);
    let graph = &snap.graph;
    let left = x + PAD;
    let inner = w - 2.0 * PAD;
    let tools_x = x + w - PAD - super::mix::TOOLS_W;
    let title_max = (tools_x - 12.0 - left).max(24.0);
    draw::label_fit(cr, fonts, true, 20.0, TEXT, left, y + 40.0, title_max, t.outputs);
    super::mix::header_tools(panel, cr, tools_x, y + 8.0, true);

    let row_y = y + 60.0;
    let [record_sel, play_sel] = check_rows(panel, cr, fonts, t, snap, left, row_y, inner);
    let status_y = row_y + 2.0 * CHECK_H + ROW_GAP + 22.0;
    let mut bottom = check_status(cr, fonts, t, snap, peaks, left, status_y, inner);
    if speakers_without_echo(snap) {
        let lines = draw::wrap(cr, fonts, false, 13.0, inner - 24.0, 4, t.echo_hint);
        let bh = lines.len() as f64 * 18.0 + 20.0;
        let by = bottom + 10.0;
        fill_round(cr, left, by, inner, bh, draw::R_CONTROL, draw::DANGER_FILL);
        for (i, line) in lines.iter().enumerate() {
            draw::label_fit(cr, fonts, false, 13.0, DANGER_TEXT, left + 12.0, by + 24.0 + i as f64 * 18.0, inner - 24.0, line);
        }
        bottom = by + bh;
    }

    let divider = bottom + 14.0;
    cr.set_source_rgb(draw::DIVIDER.r, draw::DIVIDER.g, draw::DIVIDER.b);
    cr.rectangle(left, divider, inner, 1.0);
    let _ = cr.fill();

    let top = divider + 14.0;
    let end = y + h - PAD;
    let list = kinds(graph);
    let view_h = (end - top).max(0.0);
    if panel.config.list {
        output_columns(panel, cr, fonts, t, graph, peaks, &list, x, top, w, view_h);
    } else if view_h > 0.0 {
        let content = list.len() as f64 * (BOX_H + BOX_GAP) - BOX_GAP;
        let max_scroll = (content - view_h).max(0.0);
        panel.out_scroll = panel.out_scroll.min(max_scroll);
        panel.out_list = Some((x, top, w, view_h));
        let _ = cr.save();
        cr.rectangle(x, top, w, view_h);
        cr.clip();
        let view = View { top, bottom: end };
        let mut by = top - panel.out_scroll;
        for kind in list {
            if by + BOX_H > top && by < end {
                output_box(panel, cr, fonts, t, graph, peaks, kind, left, by, inner, &view);
            }
            by += BOX_H + BOX_GAP;
        }
        let _ = cr.restore();
        if max_scroll > 0.0 {
            let track_h = view_h;
            let thumb = draw::scroll_thumb(track_h, content, 32.0);
            let ty = top + (track_h - thumb) * (panel.out_scroll / max_scroll);
            draw::paint_scroll_thumb(cr, x + w - 7.0, ty, 4.0, thumb);
        }
    }

    if panel.check_open {
        if let Some(rect) = record_sel {
            menu(panel, cr, fonts, t, graph, rect, Pick::Record);
        }
    }
    if panel.play_open {
        if let Some(rect) = play_sel {
            menu(panel, cr, fonts, t, graph, rect, Pick::Play);
        }
    }
}

struct View {
    top: f64,
    bottom: f64,
}

impl View {
    fn hot(&self, panel: &mut Panel, id: &str, x: f64, y: f64, w: f64, h: f64) {
        let y0 = y.max(self.top);
        let y1 = (y + h).min(self.bottom);
        if y1 > y0 {
            panel.hot(id, x, y0, w, y1 - y0);
        }
    }

    fn fader(&self, panel: &mut Panel, id: &str, x: f64, y: f64, w: f64, h: f64) {
        if y >= self.top && y + h <= self.bottom {
            panel.fader_hit(id, x, y, w, h, true);
        }
    }
}

type Rect = (f64, f64, f64, f64);

/// The two selectors of Mix Check.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pick {
    /// The output a take records.
    Record,
    /// The listen output a take plays on.
    Play,
}

/// Record or Stop beside the output it records, then Play or Pause beside the
/// listen output the take plays on. Returns both selector rects.
fn check_rows(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot, x: f64, y: f64, w: f64) -> [Option<Rect>; 2] {
    let recording = snap.check == CheckPhase::Recording || panel.record_pending;
    let button_w = [t.record, t.stop, t.play, t.pause]
        .iter()
        .map(|caption| draw::text_width(cr, fonts, true, 15.0, caption))
        .fold(0.0, f64::max)
        + 56.0;
    let button_w = button_w.max(112.0);
    let sel_x = x + button_w + 8.0;
    let sel_w = x + w - sel_x;

    let caption = if recording { t.stop } else { t.record };
    let caption_w = draw::text_width(cr, fonts, true, 15.0, caption);
    let kind = if recording { ButtonKind::DangerArmed } else { ButtonKind::Danger };
    panel.button(cr, fonts, x, y, button_w, CHECK_H, "", kind);
    let ink = if recording { draw::on_accent() } else { DANGER_TEXT };
    let mark_x = x + (button_w - caption_w - 24.0) / 2.0;
    let mid = y + CHECK_H / 2.0;
    if recording {
        draw::stop_square(cr, mark_x + 7.0, mid, 13.0, ink);
    } else {
        draw::record_dot(cr, mark_x + 7.0, mid, 7.0, DANGER);
    }
    draw::label_mid(cr, fonts, true, 15.0, ink, mark_x + 24.0, mid, caption);
    panel.hot(if recording { "stop" } else { "rec" }, x, y, button_w, CHECK_H);
    let record_kind = Kind::parse(&panel.config.check_output);
    let record_label = if record_kind == Kind::Pc { t.pc_short.to_string() } else { output_label(panel, t, record_kind) };
    let record_sel = selector(panel, cr, fonts, (sel_x, y, sel_w, CHECK_H), record_kind, &record_label, recording, Pick::Record);

    let y = y + CHECK_H + ROW_GAP;
    let mid = y + CHECK_H / 2.0;
    let playing = snap.check == CheckPhase::Playing;
    let can_play = !recording && (snap.has_take || playing);
    let fill = if !can_play {
        INSET
    } else {
        match panel.look(x, y, button_w, CHECK_H) {
            draw::Look::Down => draw::CONTROL_DOWN,
            draw::Look::Hover => draw::CONTROL_HOVER,
            draw::Look::Rest => draw::CONTROL,
        }
    };
    fill_round(cr, x, y, button_w, CHECK_H, CHECK_H / 2.0, fill);
    let ink = if can_play { TEXT } else { draw::mix(MUTED, INSET, 0.5) };
    let caption = if playing { t.pause } else { t.play };
    let caption_w = draw::text_width(cr, fonts, true, 15.0, caption);
    let mark_x = x + (button_w - caption_w - 24.0) / 2.0;
    if playing {
        fill_round(cr, mark_x + 1.0, mid - 7.0, 4.5, 14.0, 1.5, ink);
        fill_round(cr, mark_x + 8.5, mid - 7.0, 4.5, 14.0, 1.5, ink);
        panel.hot("check-pause", x, y, button_w, CHECK_H);
    } else {
        draw::play_triangle(cr, mark_x + 8.0, mid, 14.0, ink);
        if can_play {
            panel.hot("check-play", x, y, button_w, CHECK_H);
        }
    }
    draw::label_mid(cr, fonts, true, 15.0, ink, mark_x + 24.0, mid, caption);
    let play_kind = play_kind(panel, &snap.graph);
    let play_label = output_label(panel, t, play_kind);
    let play_sel = selector(panel, cr, fonts, (sel_x, y, sel_w, CHECK_H), play_kind, &play_label, false, Pick::Play);
    [record_sel, play_sel]
}

const ROW_GAP: f64 = 8.0;

/// One Mix Check dropdown button. Returns its rect, or None when there is no room.
fn selector(panel: &mut Panel, cr: &Context, fonts: &Fonts, (x, y, w, h): Rect, kind: Kind, label: &str, busy: bool, pick: Pick) -> Option<Rect> {
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
        match panel.look(x, y, w, h) {
            draw::Look::Down => draw::CONTROL_DOWN,
            draw::Look::Hover => draw::CONTROL_HOVER,
            draw::Look::Rest => draw::CONTROL,
        }
    };
    fill_round(cr, x, y, w, h, draw::R_CONTROL, fill);
    let mid = y + h / 2.0;
    let ink = if busy { MUTED } else { TEXT };
    let text_x = choice_mark(cr, fonts, kind, x + 12.0, mid, ink);
    draw::label_fit(cr, fonts, true, 14.0, ink, text_x, mid + 5.0, x + w - 30.0 - text_x, label);
    draw::chevron(cr, x + w - 24.0, mid - 2.0, 12.0, MUTED, false, open);
    if !busy {
        let id = match pick {
            Pick::Record => "check-out-open",
            Pick::Play => "play-to-open",
        };
        panel.hot(id, x, y, w, h);
    }
    Some((x, y, w, h))
}

/// The listen output a take plays on: the chosen one while it is connected,
/// else the one that is the default output, else the Frame speakers.
fn play_kind(panel: &Panel, graph: &Graph) -> Kind {
    let present = present_a(graph);
    let chosen = panel.config.check_play.strip_prefix("a:").and_then(|index| index.parse::<usize>().ok());
    if let Some(index) = chosen.filter(|index| present.contains(index)) {
        return Kind::A(index);
    }
    let sinks = graph.output_sinks();
    present
        .into_iter()
        .find(|&index| sinks[index].as_deref() == Some(graph.default_sink.as_str()))
        .map(Kind::A)
        .unwrap_or(Kind::A(0))
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
fn choice_mark(cr: &Context, fonts: &Fonts, kind: Kind, x: f64, mid: f64, ink: Rgb) -> f64 {
    match kind {
        Kind::Pc => {
            draw::pc_icon(cr, x, mid - 10.0, 20.0, draw::accent());
            x + 28.0
        }
        Kind::A(index) => {
            if !draw::device_icon(cr, a_device(index), x, mid - 10.0, 20.0, ink) {
                draw::label_mid(cr, fonts, true, 13.0, ink, x, mid, &kind.tag());
            }
            x + 30.0
        }
        Kind::B(_) => {
            draw::label_mid(cr, fonts, true, 13.0, draw::accent(), x, mid, &kind.tag());
            x + 30.0
        }
    }
}

/// Timer and level while recording, progress while a take plays, otherwise the hint.
fn check_status(cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot, peaks: &HashMap<String, f32>, x: f64, y: f64, w: f64) -> f64 {
    match snap.check {
        CheckPhase::Recording => {
            draw::fill_circle(cr, x + 6.0, y - 5.0, 6.0, DANGER);
            draw::label(cr, fonts, true, 14.0, DANGER_TEXT, x + 18.0, y, t.recording);
            let elapsed = snap.check_secs.min(10.0);
            let clock = format!("{elapsed:.1}s / 10s");
            let cw = draw::text_width(cr, fonts, false, 13.0, &clock);
            draw::label(cr, fonts, false, 13.0, TEXT, x + w - cw, y, &clock);
            let peak = peaks.get("check").copied().unwrap_or(0.0);
            draw::paint_led_bar(cr, x, y + 12.0, w, 8.0, peak as f64);
            y + 20.0
        }
        CheckPhase::Playing | CheckPhase::Paused => {
            let caption = if snap.check == CheckPhase::Playing { t.play } else { t.pause };
            draw::label(cr, fonts, true, 14.0, TEXT, x, y, caption);
            let total = snap.take_secs.max(0.1);
            let clock = format!("{:.1}s / {:.1}s", snap.check_secs.min(total), total);
            let cw = draw::text_width(cr, fonts, false, 13.0, &clock);
            draw::label(cr, fonts, false, 13.0, TEXT, x + w - cw, y, &clock);
            let done = (snap.check_secs / total).clamp(0.0, 1.0);
            fill_round(cr, x, y + 12.0, w, 6.0, 3.0, INSET);
            if done > 0.0 {
                fill_round(cr, x, y + 12.0, (w * done).max(6.0), 6.0, 3.0, draw::accent());
            }
            y + 18.0
        }
        _ => {
            let (text, bold, color) = if !snap.message.is_empty() {
                (snap.message.as_str(), true, DANGER_TEXT)
            } else {
                (t.mix_check_body, false, MUTED)
            };
            let take = if snap.has_take && snap.message.is_empty() { format!("{:.1}s", snap.take_secs) } else { String::new() };
            let take_w = if take.is_empty() { 0.0 } else { draw::text_width(cr, fonts, true, 13.0, &take) + 12.0 };
            let lines = draw::wrap(cr, fonts, bold, 13.0, w - take_w, 2, text);
            for (i, line) in lines.iter().enumerate() {
                draw::label_fit(cr, fonts, bold, 13.0, color, x, y + i as f64 * 18.0, w - take_w, line);
            }
            if !take.is_empty() {
                draw::label(cr, fonts, true, 13.0, TEXT, x + w - take_w + 12.0, y, &take);
            }
            y + (lines.len().max(1) as f64 - 1.0) * 18.0 + 4.0
        }
    }
}

/// Outputs stood up: one column per output, scrolling sideways. The fader and its
/// mute share the column's center, the same way the input strips do.
fn output_columns(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, graph: &Graph, peaks: &HashMap<String, f32>, list: &[Kind], x: f64, y: f64, w: f64, h: f64) {
    if h < 80.0 {
        return;
    }
    let view_w = (w - 2.0 * PAD).max(0.0);
    let content = list.len() as f64 * (COL_W + BOX_GAP) - BOX_GAP;
    let max_scroll = (content - view_w).max(0.0);
    panel.out_scroll = panel.out_scroll.min(max_scroll);
    panel.out_list = Some((x, y, w, h));
    let _ = cr.save();
    cr.rectangle(x, y, w, h - 8.0);
    cr.clip();
    let clip = ColClip { left: x, right: x + w };
    let mut cx = x + PAD - panel.out_scroll;
    for kind in list {
        if cx + COL_W > x && cx < x + w {
            output_column(panel, cr, fonts, t, graph, peaks, *kind, cx, y, COL_W, h - 10.0, &clip);
        }
        cx += COL_W + BOX_GAP;
    }
    let _ = cr.restore();
    if max_scroll > 0.0 {
        let thumb = draw::scroll_thumb(view_w, content, 32.0);
        let tx = x + PAD + (view_w - thumb) * (panel.out_scroll / max_scroll);
        draw::paint_scroll_thumb(cr, tx, y + h - 6.0, thumb, 4.0);
    }
}

struct ColClip {
    left: f64,
    right: f64,
}

impl ColClip {
    fn hot(&self, panel: &mut Panel, id: &str, x: f64, y: f64, w: f64, h: f64) {
        let x0 = x.max(self.left);
        let x1 = (x + w).min(self.right);
        if x1 > x0 {
            panel.hot(id, x0, y, x1 - x0, h);
        }
    }

    fn fader(&self, panel: &mut Panel, id: &str, x: f64, y: f64, w: f64, h: f64) {
        if x >= self.left && x + w <= self.right {
            panel.fader_hit(id, x, y, w, h, false);
        }
    }
}

fn output_column(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, graph: &Graph, peaks: &HashMap<String, f32>, kind: Kind, x: f64, y: f64, w: f64, h: f64, clip: &ColClip) {
    let pc = kind == Kind::Pc;
    let fill = if pc { draw::mix(draw::accent(), INSET, 0.12) } else { INSET };
    fill_round(cr, x, y, w, h, draw::R_CONTROL, fill);
    if pc {
        draw::rounded(cr, x + 0.5, y + 0.5, w - 1.0, h - 1.0, draw::R_CONTROL);
        draw::stroke(cr, draw::mix(draw::accent(), INSET, 0.45), 1.0);
    }
    let cx = x + w / 2.0;
    let _ = choice_mark(cr, fonts, kind, cx - 10.0, y + 18.0, TEXT);
    let label = output_label(panel, t, kind);
    let room = w - 16.0;
    let tw = draw::text_width(cr, fonts, !pc, 13.0, &label).min(room);
    draw::label_fit(cr, fonts, !pc, 13.0, TEXT, cx - tw / 2.0, y + 42.0, room, &label);
    let id = match kind {
        Kind::Pc => "pc".to_string(),
        Kind::A(index) => format!("a:{index}"),
        Kind::B(index) => format!("b:{index}"),
    };
    if let Kind::B(index) = kind {
        clip.hot(panel, &format!("bname:{index}"), x + 8.0, y + 28.0, w - 16.0, 20.0);
    }
    let bus = bus_of(panel, kind);
    let cap_top = 52.0;
    let cap_h = (h - cap_top - draw::FADER_FOOT).max(48.0);
    let side = |suffix: &str| peaks.get(&format!("out:{id}:{suffix}")).copied().unwrap_or(0.0);
    let geom = draw::paint_fader(
        cr,
        fonts,
        cx,
        y + cap_top,
        cap_h,
        bus.gain_db,
        &[side("l"), side("r")],
        panel.config.meters,
        bus.muted,
        draw::FaderGlyph::Speaker,
        panel.config.snap,
    );
    clip.fader(panel, &format!("bus:{id}"), geom.capsule_x - 4.0, geom.capsule_y, geom.meter_right - geom.capsule_x + 8.0, geom.capsule_h);
    let mute_s = MUTE_S;
    let mute_x = cx - mute_s / 2.0;
    let mute_y = y + cap_top + cap_h + draw::FADER_MUTE_Y;
    let mute_fill = if bus.muted {
        DANGER
    } else {
        match panel.look(mute_x, mute_y, mute_s, mute_s) {
            draw::Look::Down => draw::CONTROL_DOWN,
            draw::Look::Hover => draw::CONTROL_HOVER,
            draw::Look::Rest => draw::CONTROL,
        }
    };
    let mute_ink = if bus.muted { draw::BG } else { draw::SOFT };
    fill_round(cr, mute_x, mute_y, mute_s, mute_s, 8.0, mute_fill);
    let mw = draw::text_width(cr, fonts, true, 13.0, "M");
    draw::label_mid(cr, fonts, true, 13.0, mute_ink, mute_x + (mute_s - mw) / 2.0, mute_y + mute_s / 2.0, "M");
    clip.hot(panel, &format!("bmute:{id}"), mute_x, mute_y, mute_s, mute_s);
    let _ = graph;
}

fn output_box(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, graph: &Graph, peaks: &HashMap<String, f32>, kind: Kind, x: f64, y: f64, w: f64, view: &View) {
    let pc = kind == Kind::Pc;
    let fill = if pc { draw::mix(draw::accent(), INSET, 0.12) } else { INSET };
    fill_round(cr, x, y, w, BOX_H, draw::R_CONTROL, fill);
    if pc {
        draw::rounded(cr, x + 0.5, y + 0.5, w - 1.0, BOX_H - 1.0, draw::R_CONTROL);
        draw::stroke(cr, draw::mix(draw::accent(), INSET, 0.45), 1.0);
    }
    let name_mid = y + BOX_PAD + MUTE_S / 2.0;
    let text_x = choice_mark(cr, fonts, kind, x + BOX_PAD, name_mid, TEXT);
    let label = output_label(panel, t, kind);
    let mute_x = x + w - BOX_PAD - MUTE_S;
    let name_w = mute_x - 8.0 - text_x;
    draw::label_fit(cr, fonts, !pc, 15.0, TEXT, text_x, name_mid + 5.0, name_w, &label);
    let detail = if pc { t.frame_mix.to_string() } else { output_detail(graph, kind) };
    if !detail.is_empty() {
        draw::label_fit(cr, fonts, false, 12.0, MUTED, text_x, name_mid + 21.0, name_w, &detail);
    }
    if let Kind::B(index) = kind {
        view.hot(panel, &format!("bname:{index}"), text_x, y + BOX_PAD - 4.0, name_w, MUTE_S + 8.0);
    }

    let bus = bus_of(panel, kind);
    let id = match kind {
        Kind::Pc => "pc".to_string(),
        Kind::A(index) => format!("a:{index}"),
        Kind::B(index) => format!("b:{index}"),
    };
    let mute_fill = if bus.muted {
        DANGER
    } else {
        match panel.look(mute_x, y + BOX_PAD, MUTE_S, MUTE_S) {
            draw::Look::Down => draw::CONTROL_DOWN,
            draw::Look::Hover => draw::CONTROL_HOVER,
            draw::Look::Rest => draw::CONTROL,
        }
    };
    let mute_ink = if bus.muted { draw::BG } else { draw::SOFT };
    fill_round(cr, mute_x, y + BOX_PAD, MUTE_S, MUTE_S, 8.0, mute_fill);
    let mw = draw::text_width(cr, fonts, true, 13.0, "M");
    draw::label_mid(cr, fonts, true, 13.0, mute_ink, mute_x + (MUTE_S - mw) / 2.0, name_mid, "M");
    view.hot(panel, &format!("bmute:{id}"), mute_x, y + BOX_PAD, MUTE_S, MUTE_S);

    let side = |suffix: &str| peaks.get(&format!("out:{id}:{suffix}")).copied().unwrap_or(0.0);
    let levels = [side("l"), side("r")];
    let fader = draw::paint_side_fader(
        cr,
        fonts,
        x + BOX_PAD - 4.0,
        y + BOX_H - BOX_PAD - 36.0,
        w - 2.0 * BOX_PAD + 4.0,
        36.0,
        bus.gain_db,
        &levels,
        panel.config.meters,
        bus.muted,
        draw::FaderGlyph::Speaker,
        panel.config.snap,
    );
    view.fader(panel, &format!("bus:{id}"), fader.capsule_x, fader.capsule_y, fader.capsule_w, fader.capsule_h);
}

fn bus_of(panel: &Panel, kind: Kind) -> Bus {
    match kind {
        Kind::Pc => panel.config.pc.clone(),
        Kind::A(index) => panel.config.a[index.min(3)].clone(),
        Kind::B(index) => panel.config.b[index.min(3)].clone(),
    }
}

fn output_label(panel: &Panel, t: &Text, kind: Kind) -> String {
    match kind {
        Kind::Pc => t.pc_stream.to_string(),
        Kind::A(index) => match a_device(index) {
            Device::Bluetooth => t.bluetooth.to_string(),
            Device::Usb => t.usb.to_string(),
            _ => t.speakers.to_string(),
        },
        Kind::B(index) => {
            let name = &panel.config.b[index.min(3)].name;
            if name.is_empty() { format!("{} {}", t.virtual_mic, index + 1) } else { name.clone() }
        }
    }
}

/// The connected device's own name under the Bluetooth or USB title.
fn output_detail(graph: &Graph, kind: Kind) -> String {
    match kind {
        Kind::A(index) if index > 0 => graph.output_sinks()[index.min(config::A_SLOTS - 1)]
            .as_deref()
            .map(|name| device_label(graph, name))
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// The choices under a selector, drawn over the containers.
fn menu(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, graph: &Graph, (x, y, w, h): Rect, pick: Pick) {
    let (list, current, prefix) = match pick {
        Pick::Record => (kinds(graph), Kind::parse(&panel.config.check_output), "check-out"),
        Pick::Play => (present_a(graph).into_iter().map(Kind::A).collect(), play_kind(panel, graph), "play-to"),
    };
    let (_, ph) = panel.size_f();
    let top = y + h + 6.0;
    let rows = list.len() as f64;
    let menu_h = (rows * MENU_ROW + 12.0).min(ph - top - 16.0);
    fill_round(cr, x - 2.0, top - 2.0, w + 4.0, menu_h + 4.0, draw::R_CONTROL + 2.0, draw::BG);
    fill_round(cr, x, top, w, menu_h, draw::R_CONTROL, draw::CARD);
    draw::rounded(cr, x + 0.5, top + 0.5, w - 1.0, menu_h - 1.0, draw::R_CONTROL);
    draw::stroke(cr, draw::DIVIDER, 1.0);
    panel.hot(&format!("{prefix}-menu"), x, top, w, menu_h);
    let mut ry = top + 6.0;
    for kind in list {
        if ry + MENU_ROW > top + menu_h {
            break;
        }
        let on = kind == current;
        let look = panel.look(x + 6.0, ry, w - 12.0, MENU_ROW);
        let fill = if on {
            Some(draw::accent_tint())
        } else {
            match look {
                draw::Look::Down => Some(draw::CONTROL_DOWN),
                draw::Look::Hover => Some(draw::CONTROL_HOVER),
                draw::Look::Rest => None,
            }
        };
        if let Some(fill) = fill {
            fill_round(cr, x + 6.0, ry, w - 12.0, MENU_ROW, 8.0, fill);
        }
        let mid = ry + MENU_ROW / 2.0;
        let text_x = choice_mark(cr, fonts, kind, x + 16.0, mid, TEXT);
        let label = output_label(panel, t, kind);
        draw::label_fit(cr, fonts, on, 14.0, if on { draw::accent() } else { TEXT }, text_x, mid + 5.0, x + w - 16.0 - text_x, &label);
        panel.hot(&format!("{prefix}:{}", kind.value()), x + 6.0, ry, w - 12.0, MENU_ROW);
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
