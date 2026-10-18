//! The mix strip: one framed microphone and a scrolling row of framed channels.

use super::{fill_round, source_label, Panel};
use crate::draw::{self, Fonts, INSET, MUTED, R_SURFACE, TEXT};
use crate::i18n::Text;
use crate::worker::Snapshot;
use cairo::Context;
use std::collections::HashMap;

const MIC_W: f64 = 120.0;
const CHAN_W: f64 = 144.0;
const ADD_W: f64 = 64.0;
const GAP: f64 = 12.0;
/// Inner padding of the mixer card.
const PAD: f64 = 20.0;
/// Header row of the mixer card: On/Off, status, and the view tools.
const TOOL_S: f64 = 40.0;
/// Space under the strip for the scrollbar.
const BAR_ROOM: f64 = 16.0;
const BAR_H: f64 = 6.0;
const ICON_S: f64 = 20.0;
/// Channel name baseline and the capsule top inside a vertical strip.
const STRIP_TITLE_Y: f64 = 30.0;
const CAP_TOP: f64 = 48.0;

pub(crate) fn draw(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot, peaks: &HashMap<String, f32>) {
    let stage = panel.stage();
    let (sx, sy, sw, sh) = stage.picker;
    let look = panel.look(sx, sy, sw, sh);
    let fill = match look {
        draw::Look::Down => draw::CONTROL,
        draw::Look::Hover => INSET,
        draw::Look::Rest => draw::CARD,
    };
    fill_round(cr, sx, sy, sw, sh, R_SURFACE, fill);
    draw::label(cr, fonts, false, 12.0, MUTED, sx + PAD, sy + 24.0, t.mic_picker);
    let label = source_label(&panel.config.mic_source, &snap.graph, t);
    draw::label_fit(cr, fonts, true, 16.0, TEXT, sx + PAD, sy + 46.0, sw - 72.0, &label);
    draw::chevron(cr, sx + sw - PAD - 14.0, sy + sh / 2.0 - 3.0, 14.0, MUTED, false, false);
    panel.hot("mic-open", sx, sy, sw, sh);

    let (x, y, w, h) = stage.mix;
    draw::card(cr, x, y, w, h);
    let on = panel.config.mix_on;
    let row_y = y + PAD;
    super::pill_toggle(panel, cr, fonts, x + PAD, row_y, 160.0, TOOL_S, on, (t.on, t.off), ("mix-on", "mix-off"));
    let tools_w = TOOL_S * 3.0 + 8.0 * 2.0;
    let tools_x = x + w - PAD - tools_w;
    let status_x = x + PAD + 160.0 + 20.0;
    let status_w = (tools_x - 16.0 - status_x - 16.0).max(40.0);
    let dot = if snap.mix_active { draw::CHIP_OK_TEXT } else { MUTED };
    draw::fill_circle(cr, status_x + 4.0, row_y + TOOL_S / 2.0, 4.0, dot);
    let status = if snap.mix_active { t.status_on } else { t.status_off };
    draw::label_fit(cr, fonts, false, 14.0, draw::SOFT, status_x + 16.0, row_y + TOOL_S / 2.0 + 5.0, status_w, status);
    header_tools(panel, cr, tools_x, row_y);

    let hint = if panel.notice.is_empty() { t.mix_hint } else { panel.notice.as_str() };
    let hint_color = if panel.notice.is_empty() { MUTED } else { draw::DANGER_TEXT };
    let hint_y = y + h - PAD - 2.0;
    draw::label_fit(cr, fonts, false, 13.0, hint_color, x + PAD, hint_y, w - 2.0 * PAD, hint);

    let strip_top = row_y + TOOL_S + 16.0;
    let strip_h = (hint_y - 22.0 - BAR_ROOM - strip_top).max(180.0);
    if panel.config.list {
        draw_list(panel, cr, fonts, t, peaks, x, strip_top, w, strip_h);
        return;
    }
    let cap_top = CAP_TOP;
    let cap_h = (strip_h - cap_top - draw::FADER_FOOT).max(80.0);
    let mic_x = x + PAD;
    fill_round(cr, mic_x, strip_top, MIC_W, strip_h, R_SURFACE, INSET);
    let mic_w = draw::text_width(cr, fonts, true, 15.0, t.mic).min(MIC_W - 16.0);
    draw::label_fit(cr, fonts, true, 15.0, TEXT, mic_x + (MIC_W - mic_w) / 2.0, strip_top + STRIP_TITLE_Y, MIC_W - 16.0, t.mic);
    let mic_peak = peaks.get("mic").copied().unwrap_or(0.0);
    let geom = draw::paint_fader(
        cr,
        fonts,
        mic_x + MIC_W / 2.0,
        strip_top + cap_top,
        cap_h,
        panel.config.mic_volume * 100.0,
        mic_peak,
        panel.config.meters,
        panel.config.mic_muted,
        draw::FaderGlyph::Mic,
        panel.config.snap,
    );
    panel.fader_hit("mic", geom.capsule_x - 4.0, geom.capsule_y, geom.meter_right - geom.capsule_x + 8.0, geom.capsule_h, false);
    panel.hot("mute:mic", geom.icon_x - 6.0, geom.icon_y - 4.0, geom.icon_s + 12.0, geom.icon_s + 8.0);

    let list_x = mic_x + MIC_W + GAP;
    let list_w = x + w - PAD - list_x;
    let content = panel.config.channels.len() as f64 * (CHAN_W + GAP) + ADD_W;
    let max_scroll = (content - list_w).max(0.0);
    panel.scroll = panel.scroll.clamp(0.0, max_scroll);
    let _ = cr.save();
    cr.rectangle(list_x, strip_top, list_w, strip_h);
    let _ = cr.clip();
    let mut cx = list_x - panel.scroll;
    let channels = panel.config.channels.clone();
    let view = Clip { left: list_x, right: list_x + list_w };
    panel.set_strip(list_x, strip_top, list_w, strip_h);
    panel.visible_channels.clear();
    for channel in &channels {
        if cx + CHAN_W >= list_x && cx <= list_x + list_w {
            panel.visible_channels.push(channel.id.clone());
            fill_round(cr, cx, strip_top, CHAN_W, strip_h, R_SURFACE, INSET);
            let armed = panel.delete_armed(&channel.id);
            if armed {
                fill_round(cr, cx + 8.0, strip_top + 8.0, CHAN_W - 16.0, 32.0, 16.0, draw::DANGER_FILL);
                let confirm_w = draw::text_width(cr, fonts, true, 13.0, t.delete_confirm).min(CHAN_W - 28.0);
                draw::label_fit(cr, fonts, true, 13.0, draw::DANGER_TEXT, cx + (CHAN_W - confirm_w) / 2.0, strip_top + 29.0, CHAN_W - 28.0, t.delete_confirm);
                view.hot(panel, &format!("rm:{}", channel.id), cx + 8.0, strip_top + 8.0, CHAN_W - 16.0, 32.0);
            } else {
                // Icon and name are centered as one group and stay clear of the ×.
                let with_icon = !channel.app.is_empty();
                let icon_room = if with_icon { ICON_S + 6.0 } else { 0.0 };
                let name_room = CHAN_W - 52.0 - icon_room;
                let name_w = draw::text_width(cr, fonts, true, 15.0, &channel.name).min(name_room);
                let group = icon_room + name_w;
                let group_x = (cx + (CHAN_W - group) / 2.0).min(cx + CHAN_W - 38.0 - group);
                if with_icon {
                    super::paint_desktop_icon(panel, cr, fonts, &channel.app, &channel.binary, group_x, strip_top + STRIP_TITLE_Y - 15.0, ICON_S);
                }
                draw::label_fit(cr, fonts, true, 15.0, draw::accent(), group_x + icon_room, strip_top + STRIP_TITLE_Y, name_room, &channel.name);
                view.hot(panel, &format!("name:{}", channel.id), cx + 8.0, strip_top + 8.0, CHAN_W - 48.0, 32.0);
                remove_glyph(panel, cr, cx + CHAN_W - 40.0, strip_top + 8.0);
                view.hot(panel, &format!("rm:{}", channel.id), cx + CHAN_W - 40.0, strip_top + 8.0, REMOVE_S, REMOVE_S);
            }
            let peak = peaks.get(&channel.id).copied().unwrap_or(0.0);
            let col = (CHAN_W - 16.0) / 2.0;
            let left = draw::paint_fader(
                cr,
                fonts,
                cx + 8.0 + col / 2.0,
                strip_top + cap_top,
                cap_h,
                channel.to_mic * 100.0,
                peak,
                panel.config.meters,
                channel.to_mic_muted,
                draw::FaderGlyph::Mic,
                panel.config.snap,
            );
            view.fader(panel, &format!("tomic:{}", channel.id), left.capsule_x - 2.0, left.capsule_y, left.meter_right - left.capsule_x + 4.0, left.capsule_h);
            view.hot(panel, &format!("mute:tomic:{}", channel.id), left.icon_x - 4.0, left.icon_y - 4.0, left.icon_s + 8.0, left.icon_s + 8.0);
            let right = draw::paint_fader(
                cr,
                fonts,
                cx + 8.0 + col + col / 2.0,
                strip_top + cap_top,
                cap_h,
                channel.phones * 100.0,
                peak,
                panel.config.meters,
                channel.phones_muted,
                draw::FaderGlyph::Speaker,
                panel.config.snap,
            );
            view.fader(panel, &format!("phones:{}", channel.id), right.capsule_x - 2.0, right.capsule_y, right.meter_right - right.capsule_x + 4.0, right.capsule_h);
            view.hot(panel, &format!("mute:phones:{}", channel.id), right.icon_x - 4.0, right.icon_y - 4.0, right.icon_s + 8.0, right.icon_s + 8.0);
        }
        cx += CHAN_W + GAP;
    }
    let add_y = strip_top + (strip_h - ADD_W) / 2.0;
    let room = channels.len() < crate::config::MAX_CHANNELS;
    add_button(panel, cr, cx, add_y, ADD_W, room);
    if room {
        view.hot(panel, "add", cx, add_y, ADD_W, ADD_W);
    }
    let _ = cr.restore();

    if max_scroll > 0.0 {
        let bar_y = strip_top + strip_h + (BAR_ROOM - BAR_H) / 2.0 + 2.0;
        let thumb_w = (list_w * list_w / content).clamp(48.0, list_w);
        let thumb_x = list_x + (list_w - thumb_w) * (panel.scroll / max_scroll);
        paint_scrollbar(cr, list_x, bar_y, list_w, BAR_H, thumb_x, bar_y, thumb_w, BAR_H);
        panel.hbar = Some(super::HBar { x: list_x, y: bar_y, w: list_w, h: BAR_H, thumb_w, max_scroll, vertical: false });
    }
}

const ROW_H: f64 = 80.0;
const LIST_GAP: f64 = 8.0;
/// Title baseline inside a list row.
const TITLE_Y: f64 = 26.0;
/// Sideways fader under the title. The track is 24px inside this box.
const SLOT_Y: f64 = 32.0;
const SLOT_H: f64 = 44.0;
const ROW_PAD: f64 = 16.0;

/// Vertical list. The microphone is the first row and scrolls with the channels.
fn draw_list(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, peaks: &HashMap<String, f32>, x: f64, strip_top: f64, w: f64, strip_h: f64) {
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
        let mic_peak = peaks.get("mic").copied().unwrap_or(0.0);
        list_mic(panel, cr, fonts, t, mic_peak, row_x, ry, row_w, &view);
    }
    ry += ROW_H + LIST_GAP;
    for channel in &channels {
        if ry + ROW_H >= list_top && ry <= list_top + list_h {
            panel.visible_channels.push(channel.id.clone());
            let peak = peaks.get(&channel.id).copied().unwrap_or(0.0);
            list_channel(panel, cr, fonts, t, channel, peak, row_x, ry, row_w, &view);
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
        let bar_x = x + w - (PAD + BAR_H) / 2.0;
        let thumb_h = (list_h * list_h / content).clamp(48.0, list_h);
        let thumb_y = list_top + (list_h - thumb_h) * (panel.scroll / max_scroll);
        paint_scrollbar(cr, bar_x, list_top, BAR_H, list_h, bar_x, thumb_y, BAR_H, thumb_h);
        panel.hbar = Some(super::HBar {
            x: bar_x,
            y: list_top,
            w: BAR_H,
            h: list_h,
            thumb_w: thumb_h,
            max_scroll,
            vertical: true,
        });
    }
}

fn list_mic(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, peak: f32, x: f64, y: f64, w: f64, view: &VClip) {
    fill_round(cr, x, y, w, ROW_H, R_SURFACE, INSET);
    draw::label_fit(cr, fonts, true, 15.0, TEXT, x + ROW_PAD, y + TITLE_Y, w - 2.0 * ROW_PAD, t.mic);
    let mic = draw::paint_side_fader(
        cr,
        fonts,
        x + ROW_PAD - 4.0,
        y + SLOT_Y,
        w - 2.0 * ROW_PAD + 4.0,
        SLOT_H,
        panel.config.mic_volume * 100.0,
        peak,
        panel.config.meters,
        panel.config.mic_muted,
        draw::FaderGlyph::Mic,
        panel.config.snap,
    );
    view.fader(panel, "mic", mic.capsule_x, mic.capsule_y, mic.capsule_w, mic.capsule_h);
    view.hot(panel, "mute:mic", mic.icon_x - 4.0, mic.icon_y - 4.0, mic.icon_s + 8.0, mic.icon_s + 8.0);
}

fn list_channel(
    panel: &mut Panel,
    cr: &Context,
    fonts: &Fonts,
    t: &Text,
    channel: &crate::config::Channel,
    peak: f32,
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
        draw::label_fit(cr, fonts, true, 15.0, draw::accent(), x + ROW_PAD + icon_room, y + TITLE_Y, w - 72.0 - icon_room, &channel.name);
        view.hot(panel, &format!("name:{}", channel.id), x + 8.0, y + 4.0, w - 56.0, 30.0);
    }
    remove_glyph(panel, cr, x + w - 40.0, y + 4.0);
    view.hot(panel, &format!("rm:{}", channel.id), x + w - 40.0, y + 4.0, REMOVE_S, REMOVE_S);
    let gap = 16.0;
    let slot_w = (w - 2.0 * ROW_PAD + 4.0 - gap) / 2.0;
    let slot_y = y + SLOT_Y;
    let left = draw::paint_side_fader(
        cr,
        fonts,
        x + ROW_PAD - 4.0,
        slot_y,
        slot_w,
        SLOT_H,
        channel.to_mic * 100.0,
        peak,
        panel.config.meters,
        channel.to_mic_muted,
        draw::FaderGlyph::Mic,
        panel.config.snap,
    );
    view.fader(panel, &format!("tomic:{}", channel.id), left.capsule_x, left.capsule_y, left.capsule_w, left.capsule_h);
    view.hot(panel, &format!("mute:tomic:{}", channel.id), left.icon_x - 4.0, left.icon_y - 4.0, left.icon_s + 8.0, left.icon_s + 8.0);
    let right = draw::paint_side_fader(
        cr,
        fonts,
        x + ROW_PAD - 4.0 + slot_w + gap,
        slot_y,
        slot_w,
        SLOT_H,
        channel.phones * 100.0,
        peak,
        panel.config.meters,
        channel.phones_muted,
        draw::FaderGlyph::Speaker,
        panel.config.snap,
    );
    view.fader(panel, &format!("phones:{}", channel.id), right.capsule_x, right.capsule_y, right.capsule_w, right.capsule_h);
    view.hot(panel, &format!("mute:phones:{}", channel.id), right.icon_x - 4.0, right.icon_y - 4.0, right.icon_s + 8.0, right.icon_s + 8.0);
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

/// Track in the theme color, thumb in the same color at full strength.
fn paint_scrollbar(cr: &Context, x: f64, y: f64, w: f64, h: f64, tx: f64, ty: f64, tw: f64, th: f64) {
    let radius = (w.min(h) / 2.0).max(1.0);
    draw::rounded(cr, x, y, w, h, radius);
    draw::fill(cr, draw::accent(), 0.22);
    draw::rounded(cr, tx, ty, tw, th, (tw.min(th) / 2.0).max(1.0));
    draw::fill(cr, draw::accent(), 1.0);
}

/// Snap, window extend, and the list/column switch. Extend's glyph follows the current layout.
fn header_tools(panel: &mut Panel, cr: &Context, x: f64, y: f64) {
    let step = TOOL_S + 8.0;
    let snap_id = if panel.config.snap { "snap-off" } else { "snap-on" };
    tool_button(panel, cr, x, y, draw::MAGNET_PATH, 16.0, panel.config.snap, snap_id);
    let (extend_path, extend_view) = if panel.config.list {
        (draw::RESIZE_ROW_PATH, 24.0)
    } else {
        (draw::RESIZE_COLUMN_PATH, 24.0)
    };
    let wide_id = if panel.config.wide { "wide-off" } else { "wide-on" };
    tool_button(panel, cr, x + step, y, extend_path, extend_view, panel.config.wide, wide_id);
    let (view_path, view_on, view_id) = if panel.config.list {
        (draw::LIST_PATH, true, "list-off")
    } else {
        (draw::COLUMNS_PATH, false, "list-on")
    };
    tool_button(panel, cr, x + step * 2.0, y, view_path, 512.0, view_on, view_id);
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
