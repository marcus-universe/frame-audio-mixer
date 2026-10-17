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
    let left = x + 22.0;
    let right = x + w - 22.0;
    let record = (right - 140.0, y + 18.0, 140.0, 56.0);
    draw::label_fit(cr, fonts, true, 22.0, TEXT, left, y + 54.0, record.0 - left - 12.0, t.mix_check);

    let recording = snap.check == CheckPhase::Recording || panel.record_pending;
    let kind = if recording { ButtonKind::DangerArmed } else { ButtonKind::Danger };
    let caption = if recording { t.stop } else { t.record };
    panel.button(cr, fonts, record.0, record.1, record.2, record.3, "", kind);
    let color = if recording { draw::on_accent() } else { TEXT };
    if recording {
        draw::stop_square(cr, record.0 + 28.0, record.1 + 28.0, 14.0, color);
    } else {
        draw::record_dot(cr, record.0 + 28.0, record.1 + 28.0, 8.0, DANGER);
    }
    draw::label(cr, fonts, true, 16.0, color, record.0 + 44.0, record.1 + 34.0, caption);
    panel.hot(if recording { "stop" } else { "rec" }, record.0, record.1, record.2, record.3);

    // Two lines under the header: the description, or while recording the timer and
    // level, or an error. The divider stays put so the clips do not jump.
    let first = y + 100.0;
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
        let bar_y = first + 10.0;
        super::fill_round(cr, left, bar_y, right - left, 10.0, 5.0, draw::BG);
        draw::fill_meter(cr, left, bar_y, right - left, 10.0, 5.0, fraction, false);
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

    let divider = first + line_h + 18.0;
    cr.set_source_rgb(draw::DIVIDER.r, draw::DIVIDER.g, draw::DIVIDER.b);
    cr.rectangle(left, divider, right - left, 1.0);
    let _ = cr.fill();

    let echo = speakers_without_echo(snap);
    let mut row = divider + 12.0;
    for clip in snap.clips.iter().take(if echo { 4 } else { 5 }) {
        if row + 70.0 > y + h - 8.0 {
            break;
        }
        clip_row(panel, cr, fonts, clip, snap, left, row, right - left);
        row += 74.0;
    }
    if echo {
        let lines = draw::wrap(cr, fonts, false, 12.0, right - left, 4, t.echo_hint);
        let last = y + h - 22.0;
        for (i, line) in lines.iter().enumerate() {
            let baseline = last - (lines.len() - 1 - i) as f64 * 16.0;
            draw::label_fit(cr, fonts, false, 12.0, DANGER_TEXT, left, baseline, right - left, line);
        }
    }
}

fn clip_row(panel: &mut Panel, cr: &Context, fonts: &Fonts, clip: &MixClip, snap: &Snapshot, x: f64, y: f64, w: f64) {
    let playing = snap.check == CheckPhase::Playing && snap.playing_id == clip.id;
    let circle = 44.0;
    super::fill_round(cr, x, y + 4.0, circle, circle, circle / 2.0, draw::CONTROL);
    if playing {
        draw::stop_square(cr, x + circle / 2.0, y + 4.0 + circle / 2.0, 12.0, TEXT);
    } else {
        draw::play_triangle(cr, x + circle / 2.0 + 2.0, y + 4.0 + circle / 2.0, 14.0, TEXT);
    }
    panel.hot(&format!("clip:{}", clip.id), x, y, circle, circle + 8.0);

    let text_x = x + circle + 12.0;
    draw::label(cr, fonts, true, 14.0, TEXT, text_x, y + 18.0, &clip.clock);
    let duration = format!("{:.1}s", clip.seconds);
    let dw = draw::text_width(cr, fonts, false, 13.0, &duration);
    draw::label(cr, fonts, false, 13.0, MUTED, x + w - dw, y + 18.0, &duration);

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
