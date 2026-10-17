//! The mix strip: one framed microphone and a scrolling row of framed channels.

use super::{fill_round, source_label, stroke_round, Panel};
use crate::draw::{self, Fonts, BORDER, INSET, MUTED, TEXT};
use crate::i18n::Text;
use crate::worker::Snapshot;
use cairo::Context;
use std::collections::HashMap;

const MIC_W: f64 = 120.0;
const CHAN_W: f64 = 144.0;
const ADD_W: f64 = 72.0;
const GAP: f64 = 10.0;
/// Space under the strip for the scrollbar.
const BAR_ROOM: f64 = 16.0;
const BAR_H: f64 = 6.0;
const ICON_S: f64 = 20.0;

pub(crate) fn draw(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot, peaks: &HashMap<String, f32>) {
    let stage = panel.stage();
    let (sx, sy, sw, sh) = stage.picker;
    let look = panel.look(sx, sy, sw, sh);
    let fill = match look {
        draw::Look::Down => draw::CONTROL_DOWN,
        draw::Look::Hover => draw::CONTROL_HOVER,
        draw::Look::Rest => draw::CONTROL,
    };
    fill_round(cr, sx, sy, sw, sh, 16.0, fill);
    stroke_round(cr, sx, sy, sw, sh, 16.0, BORDER, 2.0);
    draw::label(cr, fonts, false, 13.0, MUTED, sx + 16.0, sy + 22.0, t.mic_picker);
    let label = source_label(&panel.config.mic_source, &snap.graph, t);
    draw::label_fit(cr, fonts, true, 16.0, TEXT, sx + 16.0, sy + 46.0, sw - 56.0, &label);
    draw::chevron(cr, sx + sw - 36.0, sy + 24.0, 16.0, MUTED, false, false);
    panel.hot("mic-open", sx, sy, sw, sh);

    let (x, y, w, h) = stage.mix;
    draw::card(cr, x, y, w, h);
    let on = panel.config.mix_on;
    segment(panel, cr, fonts, x + 22.0, y + 18.0, 180.0, on, t.on, t.off, "mix-on", "mix-off");
    let status = if snap.mix_active { t.status_on } else { t.status_off };
    draw::label_fit(cr, fonts, false, 14.0, if snap.mix_active { draw::CHIP_OK_TEXT } else { draw::SOFT }, x + 220.0, y + 44.0, w - 250.0, status);

    let hint = if panel.notice.is_empty() { t.mix_hint } else { panel.notice.as_str() };
    let hint_color = if panel.notice.is_empty() { MUTED } else { draw::DANGER_TEXT };
    let hint_y = y + h - 28.0;
    draw::label_fit(cr, fonts, false, 13.0, hint_color, x + 22.0, hint_y, w - 44.0, hint);

    let strip_top = y + 78.0;
    let strip_h = (hint_y - 18.0 - BAR_ROOM - strip_top).max(180.0);
    if panel.config.list {
        draw_list(panel, cr, fonts, t, peaks, x, strip_top, w, strip_h);
        return;
    }
    let cap_top = 40.0;
    let cap_h = (strip_h - cap_top - 64.0).max(80.0);
    let mic_x = x + 16.0;
    fill_round(cr, mic_x, strip_top, MIC_W, strip_h, 16.0, INSET);
    stroke_round(cr, mic_x, strip_top, MIC_W, strip_h, 16.0, BORDER, 2.0);
    let mic_w = draw::text_width(cr, fonts, true, 14.0, t.mic).min(MIC_W - 16.0);
    draw::label_fit(cr, fonts, true, 14.0, TEXT, mic_x + (MIC_W - mic_w) / 2.0, strip_top + 26.0, MIC_W - 16.0, t.mic);
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
    let list_w = x + w - 16.0 - list_x;
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
            fill_round(cr, cx, strip_top, CHAN_W, strip_h, 16.0, INSET);
            stroke_round(cr, cx, strip_top, CHAN_W, strip_h, 16.0, draw::accent(), 2.0);
            let armed = panel.delete_armed(&channel.id);
            if armed {
                fill_round(cr, cx + 8.0, strip_top + 8.0, CHAN_W - 16.0, 28.0, 8.0, draw::CONTROL);
                let confirm_w = draw::text_width(cr, fonts, true, 12.0, t.delete_confirm).min(CHAN_W - 24.0);
                draw::label_fit(cr, fonts, true, 12.0, draw::accent(), cx + (CHAN_W - confirm_w) / 2.0, strip_top + 26.0, CHAN_W - 24.0, t.delete_confirm);
                view.hot(panel, &format!("rm:{}", channel.id), cx + 8.0, strip_top + 8.0, CHAN_W - 16.0, 28.0);
            } else {
                // Icon and name are centered as one group and stay clear of the ×.
                let with_icon = !channel.app.is_empty();
                let icon_room = if with_icon { ICON_S + 6.0 } else { 0.0 };
                let name_room = CHAN_W - 44.0 - icon_room;
                let name_w = draw::text_width(cr, fonts, true, 15.0, &channel.name).min(name_room);
                let group = icon_room + name_w;
                let group_x = (cx + (CHAN_W - group) / 2.0).min(cx + CHAN_W - 34.0 - group);
                if with_icon {
                    super::paint_desktop_icon(panel, cr, fonts, &channel.app, &channel.binary, group_x, strip_top + 11.0, ICON_S);
                }
                draw::label_fit(cr, fonts, true, 15.0, draw::accent(), group_x + icon_room, strip_top + 26.0, name_room, &channel.name);
                view.hot(panel, &format!("name:{}", channel.id), cx + 8.0, strip_top + 8.0, CHAN_W - 44.0, 28.0);
                draw::icon(cr, draw::CLOSE_PATH, 1024.0, cx + CHAN_W - 34.0, strip_top + 4.0, 26.0, TEXT);
                view.hot(panel, &format!("rm:{}", channel.id), cx + CHAN_W - 36.0, strip_top + 2.0, 30.0, 30.0);
            }
            let peak = peaks.get(&channel.id).copied().unwrap_or(0.0);
            let col = (CHAN_W - 16.0) / 2.0;
            let left = draw::paint_fader(
                cr,
                fonts,
                cx + 8.0 + col / 2.0,
                strip_top + cap_top,
                cap_h,
                channel.phones * 100.0,
                peak,
                panel.config.meters,
                channel.phones_muted,
                draw::FaderGlyph::Speaker,
                panel.config.snap,
            );
            view.fader(panel, &format!("phones:{}", channel.id), left.capsule_x - 2.0, left.capsule_y, left.meter_right - left.capsule_x + 4.0, left.capsule_h);
            view.hot(panel, &format!("mute:phones:{}", channel.id), left.icon_x - 4.0, left.icon_y - 4.0, left.icon_s + 8.0, left.icon_s + 8.0);
            let right = draw::paint_fader(
                cr,
                fonts,
                cx + 8.0 + col + col / 2.0,
                strip_top + cap_top,
                cap_h,
                channel.to_mic * 100.0,
                peak,
                panel.config.meters,
                channel.to_mic_muted,
                draw::FaderGlyph::Mic,
                panel.config.snap,
            );
            view.fader(panel, &format!("tomic:{}", channel.id), right.capsule_x - 2.0, right.capsule_y, right.meter_right - right.capsule_x + 4.0, right.capsule_h);
            view.hot(panel, &format!("mute:tomic:{}", channel.id), right.icon_x - 4.0, right.icon_y - 4.0, right.icon_s + 8.0, right.icon_s + 8.0);
        }
        cx += CHAN_W + GAP;
    }
    let add_y = strip_top + (strip_h - ADD_W) / 2.0;
    let room = channels.len() < crate::config::MAX_CHANNELS;
    fill_round(cr, cx, add_y, ADD_W, ADD_W, 16.0, INSET);
    stroke_round(cr, cx, add_y, ADD_W, ADD_W, 16.0, if room { draw::accent() } else { BORDER }, 2.0);
    draw::icon(cr, draw::ADD_PATH, 28.0, cx + 16.0, add_y + 16.0, 40.0, if room { draw::accent() } else { MUTED });
    if room {
        view.hot(panel, "add", cx, add_y, ADD_W, ADD_W);
    }
    let _ = cr.restore();

    if max_scroll > 0.0 {
        let bar_y = strip_top + strip_h + (BAR_ROOM - BAR_H) / 2.0 + 2.0;
        let thumb_w = (list_w * list_w / content).clamp(48.0, list_w);
        let thumb_x = list_x + (list_w - thumb_w) * (panel.scroll / max_scroll);
        fill_round(cr, list_x, bar_y, list_w, BAR_H, BAR_H / 2.0, draw::CONTROL);
        let thumb_look = panel.look(list_x, bar_y - 10.0, list_w, BAR_H + 20.0);
        let lit = panel.dragging(super::HSCROLL) || thumb_look != draw::Look::Rest;
        fill_round(cr, thumb_x, bar_y, thumb_w, BAR_H, BAR_H / 2.0, if lit { draw::accent() } else { BORDER });
        panel.hbar = Some(super::HBar { x: list_x, y: bar_y, w: list_w, h: BAR_H, thumb_w, max_scroll, vertical: false });
    }
}

const ROW_H: f64 = 108.0;

/// Vertical list: the microphone stays put, and each channel is a row of sideways sliders.
fn draw_list(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, peaks: &HashMap<String, f32>, x: f64, strip_top: f64, w: f64, strip_h: f64) {
    let row_x = x + 12.0;
    let row_w = w - 24.0;
    let mic_peak = peaks.get("mic").copied().unwrap_or(0.0);
    fill_round(cr, row_x, strip_top, row_w, ROW_H, 16.0, INSET);
    stroke_round(cr, row_x, strip_top, row_w, ROW_H, 16.0, BORDER, 2.0);
    draw::label_fit(cr, fonts, true, 15.0, TEXT, row_x + 14.0, strip_top + 26.0, row_w - 28.0, t.mic);
    let mic = draw::paint_side_fader(
        cr,
        fonts,
        row_x + 12.0,
        strip_top + 40.0,
        row_w - 24.0,
        56.0,
        panel.config.mic_volume * 100.0,
        mic_peak,
        panel.config.meters,
        panel.config.mic_muted,
        draw::FaderGlyph::Mic,
        panel.config.snap,
        false,
    );
    panel.fader_hit("mic", mic.capsule_x, mic.capsule_y, mic.capsule_w, mic.capsule_h, true);
    panel.hot("mute:mic", mic.icon_x - 4.0, mic.icon_y - 4.0, mic.icon_s + 8.0, mic.icon_s + 8.0);

    let list_top = strip_top + ROW_H + 8.0;
    let list_h = (strip_top + strip_h - list_top).max(72.0);
    let channels = panel.config.channels.clone();
    let content = channels.len() as f64 * (ROW_H + GAP) + 64.0;
    let max_scroll = (content - list_h).max(0.0);
    panel.scroll = panel.scroll.clamp(0.0, max_scroll);
    panel.set_strip(row_x, list_top, row_w, list_h);
    let _ = cr.save();
    cr.rectangle(row_x, list_top, row_w, list_h);
    let _ = cr.clip();
    let view = VClip { top: list_top, bottom: list_top + list_h, left: row_x, right: row_x + row_w };
    panel.visible_channels.clear();
    let mut ry = list_top - panel.scroll;
    for channel in &channels {
        if ry + ROW_H >= list_top && ry <= list_top + list_h {
            panel.visible_channels.push(channel.id.clone());
            let peak = peaks.get(&channel.id).copied().unwrap_or(0.0);
            list_channel(panel, cr, fonts, t, channel, peak, row_x, ry, row_w, &view);
        }
        ry += ROW_H + GAP;
    }
    let room = channels.len() < crate::config::MAX_CHANNELS;
    let add_s = 56.0;
    let add_x = row_x + (row_w - add_s) / 2.0;
    fill_round(cr, add_x, ry, add_s, add_s, 16.0, INSET);
    stroke_round(cr, add_x, ry, add_s, add_s, 16.0, if room { draw::accent() } else { BORDER }, 2.0);
    draw::icon(cr, draw::ADD_PATH, 28.0, add_x + 8.0, ry + 8.0, 40.0, if room { draw::accent() } else { MUTED });
    if room {
        view.hot(panel, "add", add_x, ry, add_s, add_s);
    }
    let _ = cr.restore();

    if max_scroll > 0.0 {
        let bar_x = x + w - 14.0;
        let thumb_h = (list_h * list_h / content).clamp(48.0, list_h);
        let thumb_y = list_top + (list_h - thumb_h) * (panel.scroll / max_scroll);
        fill_round(cr, bar_x, list_top, BAR_H, list_h, BAR_H / 2.0, draw::CONTROL);
        let thumb_look = panel.look(bar_x - 10.0, list_top, BAR_H + 20.0, list_h);
        let lit = panel.dragging(super::HSCROLL) || thumb_look != draw::Look::Rest;
        fill_round(cr, bar_x, thumb_y, BAR_H, thumb_h, BAR_H / 2.0, if lit { draw::accent() } else { BORDER });
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
    fill_round(cr, x, y, w, ROW_H, 16.0, INSET);
    stroke_round(cr, x, y, w, ROW_H, 16.0, draw::accent(), 2.0);
    let armed = panel.delete_armed(&channel.id);
    if armed {
        fill_round(cr, x + 8.0, y + 8.0, w - 16.0, 28.0, 8.0, draw::CONTROL);
        draw::label_fit(cr, fonts, true, 12.0, draw::accent(), x + 16.0, y + 26.0, w - 32.0, t.delete_confirm);
        view.hot(panel, &format!("rm:{}", channel.id), x + 8.0, y + 8.0, w - 16.0, 28.0);
    } else {
        let with_icon = !channel.app.is_empty();
        let icon_room = if with_icon { ICON_S + 6.0 } else { 0.0 };
        if with_icon {
            super::paint_desktop_icon(panel, cr, fonts, &channel.app, &channel.binary, x + 12.0, y + 8.0, ICON_S);
        }
        draw::label_fit(cr, fonts, true, 15.0, draw::accent(), x + 12.0 + icon_room, y + 24.0, w - 64.0 - icon_room, &channel.name);
        view.hot(panel, &format!("name:{}", channel.id), x + 8.0, y + 4.0, w - 52.0, 32.0);
        draw::icon(cr, draw::CLOSE_PATH, 1024.0, x + w - 36.0, y + 6.0, 26.0, TEXT);
        view.hot(panel, &format!("rm:{}", channel.id), x + w - 40.0, y + 2.0, 34.0, 34.0);
    }
    let gap = 10.0;
    let slot_w = (w - 24.0 - gap) / 2.0;
    let slot_y = y + 42.0;
    let left = draw::paint_side_fader(
        cr,
        fonts,
        x + 12.0,
        slot_y,
        slot_w,
        56.0,
        channel.phones * 100.0,
        peak,
        panel.config.meters,
        channel.phones_muted,
        draw::FaderGlyph::Speaker,
        panel.config.snap,
        true,
    );
    view.fader(panel, &format!("phones:{}", channel.id), left.capsule_x, left.capsule_y, left.capsule_w, left.capsule_h);
    view.hot(panel, &format!("mute:phones:{}", channel.id), left.icon_x - 4.0, left.icon_y - 4.0, left.icon_s + 8.0, left.icon_s + 8.0);
    let right = draw::paint_side_fader(
        cr,
        fonts,
        x + 12.0 + slot_w + gap,
        slot_y,
        slot_w,
        56.0,
        channel.to_mic * 100.0,
        peak,
        panel.config.meters,
        channel.to_mic_muted,
        draw::FaderGlyph::Mic,
        panel.config.snap,
        false,
    );
    view.fader(panel, &format!("tomic:{}", channel.id), right.capsule_x, right.capsule_y, right.capsule_w, right.capsule_h);
    view.hot(panel, &format!("mute:tomic:{}", channel.id), right.icon_x - 4.0, right.icon_y - 4.0, right.icon_s + 8.0, right.icon_s + 8.0);
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

fn segment(panel: &mut Panel, cr: &Context, fonts: &Fonts, x: f64, y: f64, w: f64, on: bool, yes: &str, no: &str, yes_id: &str, no_id: &str) {
    fill_round(cr, x, y, w, 40.0, 20.0, draw::CONTROL);
    let half = w / 2.0;
    if on {
        fill_round(cr, x, y, half, 40.0, 20.0, draw::accent());
    } else {
        fill_round(cr, x + half, y, half, 40.0, 20.0, draw::accent());
    }
    let yes_c = if on { draw::on_accent() } else { draw::SOFT };
    let no_c = if on { draw::SOFT } else { draw::on_accent() };
    let yw = draw::text_width(cr, fonts, true, 16.0, yes);
    let nw = draw::text_width(cr, fonts, true, 16.0, no);
    draw::label(cr, fonts, true, 16.0, yes_c, x + (half - yw) / 2.0, y + 26.0, yes);
    draw::label(cr, fonts, true, 16.0, no_c, x + half + (half - nw) / 2.0, y + 26.0, no);
    panel.hot(yes_id, x, y, half, 40.0);
    panel.hot(no_id, x + half, y, half, 40.0);
}
