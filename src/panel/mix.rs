//! The mix strip: one framed microphone and a scrolling row of framed channels.

use super::{fill_round, source_label, stroke_round, Panel};
use crate::draw::{self, Fonts, ACCENT, BORDER, INSET, MUTED, TEXT};
use crate::i18n::Text;
use crate::worker::Snapshot;
use cairo::Context;
use std::collections::HashMap;

const MIC_W: f64 = 120.0;
const CHAN_W: f64 = 144.0;
const ADD_W: f64 = 72.0;
const GAP: f64 = 10.0;

pub(crate) fn draw(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot, peaks: &HashMap<String, f32>) {
    let (sx, sy, sw, sh) = (28.0, 92.0, 747.0, 60.0);
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

    let (x, y, w, h) = (28.0, 168.0, 747.0, 492.0);
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
    let strip_h = (hint_y - 18.0 - strip_top).max(180.0);
    let cap_top = 40.0;
    let cap_h = (strip_h - cap_top - 64.0).max(80.0);
    let mic_x = x + 16.0;
    fill_round(cr, mic_x, strip_top, MIC_W, strip_h, 16.0, INSET);
    stroke_round(cr, mic_x, strip_top, MIC_W, strip_h, 16.0, BORDER, 2.0);
    draw::label_fit(cr, fonts, true, 14.0, TEXT, mic_x + 10.0, strip_top + 26.0, MIC_W - 20.0, t.mic);
    let mic_peak = peaks.get("mic").copied().unwrap_or(0.0);
    let geom = draw::paint_fader(
        cr,
        fonts,
        mic_x + 52.0,
        strip_top + cap_top,
        cap_h,
        panel.config.mic_volume * 100.0,
        mic_peak,
        panel.config.meters,
        panel.config.mic_muted,
        draw::FaderGlyph::Mic,
    );
    panel.fader_hit("mic", geom.capsule_x - 4.0, geom.capsule_y, geom.meter_right - geom.capsule_x + 8.0, geom.capsule_h);
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
    for channel in &channels {
        if cx + CHAN_W >= list_x && cx <= list_x + list_w {
            fill_round(cr, cx, strip_top, CHAN_W, strip_h, 16.0, INSET);
            stroke_round(cr, cx, strip_top, CHAN_W, strip_h, 16.0, ACCENT, 2.0);
            let armed = panel.delete_armed(&channel.id);
            if armed {
                fill_round(cr, cx + 8.0, strip_top + 8.0, CHAN_W - 16.0, 28.0, 8.0, draw::CONTROL);
                draw::label_fit(cr, fonts, true, 12.0, ACCENT, cx + 12.0, strip_top + 26.0, CHAN_W - 24.0, t.delete_confirm);
                panel.hot(&format!("rm:{}", channel.id), cx + 8.0, strip_top + 8.0, CHAN_W - 16.0, 28.0);
            } else {
                draw::label_fit(cr, fonts, true, 15.0, ACCENT, cx + 10.0, strip_top + 26.0, CHAN_W - 48.0, &channel.name);
                panel.hot(&format!("name:{}", channel.id), cx + 8.0, strip_top + 8.0, CHAN_W - 44.0, 28.0);
                draw::label(cr, fonts, true, 16.0, TEXT, cx + CHAN_W - 24.0, strip_top + 26.0, "×");
                panel.hot(&format!("rm:{}", channel.id), cx + CHAN_W - 32.0, strip_top + 6.0, 26.0, 26.0);
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
            );
            panel.fader_hit(&format!("phones:{}", channel.id), left.capsule_x - 2.0, left.capsule_y, left.meter_right - left.capsule_x + 4.0, left.capsule_h);
            panel.hot(&format!("mute:phones:{}", channel.id), left.icon_x - 4.0, left.icon_y - 4.0, left.icon_s + 8.0, left.icon_s + 8.0);
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
            );
            panel.fader_hit(&format!("tomic:{}", channel.id), right.capsule_x - 2.0, right.capsule_y, right.meter_right - right.capsule_x + 4.0, right.capsule_h);
            panel.hot(&format!("mute:tomic:{}", channel.id), right.icon_x - 4.0, right.icon_y - 4.0, right.icon_s + 8.0, right.icon_s + 8.0);
        }
        cx += CHAN_W + GAP;
    }
    let add_y = strip_top + (strip_h - ADD_W) / 2.0;
    let room = channels.len() < crate::config::MAX_CHANNELS;
    fill_round(cr, cx, add_y, ADD_W, ADD_W, 16.0, INSET);
    stroke_round(cr, cx, add_y, ADD_W, ADD_W, 16.0, if room { ACCENT } else { BORDER }, 2.0);
    draw::icon(cr, draw::ADD_PATH, 28.0, cx + 16.0, add_y + 16.0, 40.0, if room { ACCENT } else { MUTED });
    if room {
        panel.hot("add", cx, add_y, ADD_W, ADD_W);
    }
    let _ = cr.restore();
}

fn segment(panel: &mut Panel, cr: &Context, fonts: &Fonts, x: f64, y: f64, w: f64, on: bool, yes: &str, no: &str, yes_id: &str, no_id: &str) {
    fill_round(cr, x, y, w, 40.0, 20.0, draw::CONTROL);
    let half = w / 2.0;
    if on {
        fill_round(cr, x, y, half, 40.0, 20.0, draw::ACCENT);
    } else {
        fill_round(cr, x + half, y, half, 40.0, 20.0, draw::ACCENT);
    }
    let yes_c = if on { draw::ON_ACCENT } else { draw::SOFT };
    let no_c = if on { draw::SOFT } else { draw::ON_ACCENT };
    let yw = draw::text_width(cr, fonts, true, 16.0, yes);
    let nw = draw::text_width(cr, fonts, true, 16.0, no);
    draw::label(cr, fonts, true, 16.0, yes_c, x + (half - yw) / 2.0, y + 26.0, yes);
    draw::label(cr, fonts, true, 16.0, no_c, x + half + (half - nw) / 2.0, y + 26.0, no);
    panel.hot(yes_id, x, y, half, 40.0);
    panel.hot(no_id, x + half, y, half, 40.0);
}
