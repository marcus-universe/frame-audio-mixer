//! Mix Check, laid out like Frame Mic Tuner's Voice check: record in the header,
//! a status line, then a short clip history with its own play button.

use super::Panel;
use crate::draw::{self, ButtonKind, Fonts, DANGER, DANGER_TEXT, MUTED, TEXT};
use crate::i18n::Text;
use crate::worker::{CheckPhase, MixClip, Snapshot};
use cairo::Context;
use std::collections::HashMap;

pub(crate) fn draw(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot, peaks: &HashMap<String, f32>) {
    let (x, y, w, h) = panel.stage().check;
    draw::card(cr, x, y, w, h);
    let left = x + 24.0;
    let right = x + w - 24.0;
    let recording = snap.check == CheckPhase::Recording || panel.record_pending;
    let caption = if recording { t.stop } else { t.record };
    let caption_w = draw::text_width(cr, fonts, true, 15.0, caption);
    let record_w = (caption_w + 72.0).max(128.0);
    let record = (right - record_w, y + 20.0, record_w, 48.0);
    draw::label_mid(cr, fonts, true, 20.0, TEXT, left, record.1 + record.3 / 2.0, t.mix_check);

    let kind = if recording { ButtonKind::DangerArmed } else { ButtonKind::Danger };
    panel.button(cr, fonts, record.0, record.1, record.2, record.3, "", kind);
    let color = if recording { draw::on_accent() } else { DANGER_TEXT };
    let mark_x = record.0 + (record.2 - caption_w - 26.0) / 2.0;
    let mid = record.1 + record.3 / 2.0;
    if recording {
        draw::stop_square(cr, mark_x + 7.0, mid, 13.0, color);
    } else {
        draw::record_dot(cr, mark_x + 7.0, mid, 7.0, DANGER);
    }
    draw::label_mid(cr, fonts, true, 15.0, color, mark_x + 26.0, mid, caption);
    panel.hot(if recording { "stop" } else { "rec" }, record.0, record.1, record.2, record.3);

    // Two lines under the header: the description, or while recording the timer and
    // level, or an error. The divider stays put so the clips do not jump.
    let first = y + 98.0;
    let line_h = 18.0;
    if recording {
        draw::fill_circle(cr, left + 6.0, first - 5.0, 6.0, DANGER);
        draw::label(cr, fonts, true, 15.0, DANGER_TEXT, left + 18.0, first, t.recording);
        let elapsed = snap.check_secs.min(10.0);
        let left_s = (10.0 - elapsed).max(0.0);
        let clock = format!("{elapsed:.0}s  ·  {left_s:.0}s");
        let clock_w = draw::text_width(cr, fonts, false, 14.0, &clock);
        draw::label(cr, fonts, false, 14.0, TEXT, right - clock_w, first, &clock);
        let peak = peaks.get("mic").copied().unwrap_or(0.0);
        let db = if peak <= 1e-6 { -60.0 } else { (20.0 * (peak as f64).log10()).max(-60.0) };
        let fraction = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
        let bar_y = first + 12.0;
        super::fill_round(cr, left, bar_y, right - left, 8.0, 4.0, draw::BG);
        draw::fill_meter(cr, left, bar_y, right - left, 8.0, 4.0, fraction, false);
    } else {
        let (text, bold, color) = if snap.message.is_empty() {
            (t.mix_check_body, false, MUTED)
        } else {
            (snap.message.as_str(), true, DANGER_TEXT)
        };
        let lines = draw::wrap(cr, fonts, bold, 13.0, right - left, 2, text);
        for (i, line) in lines.iter().enumerate() {
            draw::label_fit(cr, fonts, bold, 13.0, color, left, first + i as f64 * line_h, right - left, line);
        }
    }

    let divider = first + line_h + 20.0;
    cr.set_source_rgb(draw::DIVIDER.r, draw::DIVIDER.g, draw::DIVIDER.b);
    cr.rectangle(left, divider, right - left, 1.0);
    let _ = cr.fill();

    let echo = speakers_without_echo(snap);
    let banner = if echo {
        let lines = draw::wrap(cr, fonts, false, 13.0, right - left - 32.0, 4, t.echo_hint);
        let bh = lines.len() as f64 * 18.0 + 24.0;
        let by = y + h - 16.0 - bh;
        super::fill_round(cr, x + 16.0, by, w - 32.0, bh, draw::R_CONTROL, draw::DANGER_FILL);
        for (i, line) in lines.iter().enumerate() {
            draw::label_fit(cr, fonts, false, 13.0, DANGER_TEXT, left + 8.0, by + 29.0 + i as f64 * 18.0, right - left - 16.0, line);
        }
        by
    } else {
        y + h
    };
    let mut row = divider + 16.0;
    for clip in snap.clips.iter().take(5) {
        if row + 64.0 > banner - 8.0 {
            break;
        }
        clip_row(panel, cr, fonts, clip, snap, left, row, right - left);
        row += 72.0;
    }
}

fn clip_row(panel: &mut Panel, cr: &Context, fonts: &Fonts, clip: &MixClip, snap: &Snapshot, x: f64, y: f64, w: f64) {
    let playing = snap.check == CheckPhase::Playing && snap.playing_id == clip.id;
    let circle = 44.0;
    let look = panel.look(x, y + 4.0, circle, circle);
    let fill = match look {
        draw::Look::Down => draw::CONTROL_DOWN,
        draw::Look::Hover => draw::CONTROL_HOVER,
        draw::Look::Rest => draw::CONTROL,
    };
    super::fill_round(cr, x, y + 4.0, circle, circle, circle / 2.0, fill);
    if playing {
        draw::stop_square(cr, x + circle / 2.0, y + 4.0 + circle / 2.0, 12.0, TEXT);
    } else {
        draw::play_triangle(cr, x + circle / 2.0 + 2.0, y + 4.0 + circle / 2.0, 14.0, TEXT);
    }
    panel.hot(&format!("clip:{}", clip.id), x, y, circle, circle + 8.0);

    let text_x = x + circle + 16.0;
    draw::label(cr, fonts, true, 14.0, TEXT, text_x, y + 16.0, &clip.clock);
    let duration = format!("{:.1}s", clip.seconds);
    let dw = draw::text_width(cr, fonts, false, 13.0, &duration);
    draw::label(cr, fonts, false, 13.0, MUTED, x + w - dw, y + 16.0, &duration);

    let wave_x = text_x;
    let wave_w = w - (text_x - x);
    let wave_y = y + 28.0;
    let bars = clip.wave.len().max(1);
    let gap = 2.0;
    let bar_w = ((wave_w + gap) / bars as f64 - gap).clamp(2.0, 4.0);
    let played = if playing && clip.seconds > 0.0 { (snap.check_secs / clip.seconds).clamp(0.0, 1.0) } else { 0.0 };
    for (index, peak) in clip.wave.iter().enumerate() {
        let bx = wave_x + index as f64 * (bar_w + gap);
        if bx + bar_w > wave_x + wave_w {
            break;
        }
        let height = (6.0 + (*peak as f64).sqrt() * 28.0).min(36.0);
        let color = if (index as f64 + 0.5) / bars as f64 <= played { draw::accent() } else { MUTED };
        super::fill_round(cr, bx, wave_y + 36.0 - height, bar_w, height, 1.5, color);
    }
}

fn speakers_without_echo(snap: &Snapshot) -> bool {
    snap.graph.default_sink.to_lowercase().contains("speaker") && snap.echo_cancel == Some(false)
}
