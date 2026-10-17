//! Mix Check. Record sits in the title row, in red, the way Voice check does.

use super::Panel;
use crate::draw::{self, ButtonKind, Fonts, DANGER, DANGER_TEXT, MUTED, ON_ACCENT, TEXT};
use crate::i18n::Text;
use crate::worker::{CheckPhase, Snapshot};
use cairo::Context;
use std::collections::HashMap;

pub(crate) fn draw(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot, peaks: &HashMap<String, f32>) {
    let (x, y, w, h) = (791.0, 92.0, 381.0, 568.0);
    draw::card(cr, x, y, w, h);
    let left = x + 22.0;
    let right = x + w - 22.0;
    let record = (right - 140.0, y + 18.0, 140.0, 56.0);
    draw::label_fit(cr, fonts, true, 22.0, TEXT, left, y + 42.0, record.0 - left - 12.0, t.mix_check);
    draw::label_fit(cr, fonts, false, 13.0, MUTED, left, y + 64.0, record.0 - left - 12.0, t.mix_check_body);

    let recording = snap.check == CheckPhase::Recording;
    let kind = if recording { ButtonKind::DangerArmed } else { ButtonKind::Danger };
    let caption = if recording { t.stop } else { t.record };
    if panel.config.mix_on || recording {
        panel.button(cr, fonts, record.0, record.1, record.2, record.3, "", kind);
        let color = if recording { ON_ACCENT } else { TEXT };
        if recording {
            draw::stop_square(cr, record.0 + 28.0, record.1 + 28.0, 14.0, color);
        } else {
            draw::record_dot(cr, record.0 + 28.0, record.1 + 28.0, 8.0, DANGER);
        }
        draw::label(cr, fonts, true, 16.0, color, record.0 + 44.0, record.1 + 34.0, caption);
        panel.hot(if recording { "stop" } else { "rec" }, record.0, record.1, record.2, record.3);
    } else {
        panel.button(cr, fonts, record.0, record.1, record.2, record.3, t.record, ButtonKind::Plain);
        draw::label_fit(cr, fonts, false, 13.0, MUTED, left, y + 96.0, w - 44.0, t.mix_check_need);
    }

    let line_y = y + 108.0;
    if recording {
        draw::fill_circle(cr, left + 6.0, line_y, 6.0, DANGER);
        draw::label(cr, fonts, true, 15.0, DANGER_TEXT, left + 18.0, line_y + 5.0, t.recording);
        let elapsed = snap.check_secs;
        let left_s = (10.0 - elapsed).max(0.0);
        let clock = format!("{elapsed:.0}s  ·  {left_s:.0}s");
        draw::label(cr, fonts, false, 14.0, TEXT, left + 130.0, line_y + 5.0, &clock);
        let peak = peaks.get("mic").copied().unwrap_or(0.0);
        let db = if peak <= 1e-6 { -60.0 } else { (20.0 * (peak as f64).log10()).max(-60.0) };
        let fraction = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
        let bar_y = line_y + 28.0;
        super::fill_round(cr, left, bar_y, right - left, 14.0, 7.0, draw::BG);
        if fraction > 0.0 {
            super::fill_round(cr, left, bar_y, (right - left) * fraction, 14.0, 7.0, draw::ACCENT);
        }
        super::stroke_round(cr, left, bar_y, right - left, 14.0, 7.0, draw::BORDER, 1.5);
    } else if !snap.message.is_empty() {
        draw::label_fit(cr, fonts, true, 14.0, DANGER_TEXT, left, line_y + 5.0, right - left, &snap.message);
    } else if snap.check == CheckPhase::Ready || snap.check == CheckPhase::Playing {
        draw::label(cr, fonts, false, 14.0, MUTED, left, line_y + 5.0, t.ready);
    }

    let play_y = line_y + 64.0;
    let can_play = snap.check == CheckPhase::Ready || snap.check == CheckPhase::Playing;
    panel.button(cr, fonts, left, play_y, 140.0, 48.0, "", if can_play { ButtonKind::Plain } else { ButtonKind::Plain });
    draw::play_triangle(cr, left + 28.0, play_y + 24.0, 16.0, if can_play { TEXT } else { MUTED });
    draw::label(cr, fonts, true, 16.0, if can_play { TEXT } else { MUTED }, left + 44.0, play_y + 30.0, t.play);
    if can_play {
        panel.hot("play", left, play_y, 140.0, 48.0);
    }
    if speakers_without_echo(snap) {
        draw::label_fit(cr, fonts, false, 13.0, DANGER_TEXT, left, play_y + 72.0, right - left, t.echo_hint);
    }
}

fn speakers_without_echo(snap: &Snapshot) -> bool {
    snap.graph.default_sink.to_lowercase().contains("speaker") && snap.echo_cancel == Some(false)
}
