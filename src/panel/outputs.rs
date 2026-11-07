//! Outputs drawer: one master per A and B bus.

use super::{fill_round, Panel};
use crate::config::{self, Bus};
use crate::draw::{self, Fonts, INSET, MUTED, TEXT};
use crate::graph::Graph;
use crate::i18n::Text;
use cairo::Context;

pub(crate) fn draw(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, graph: &Graph) {
    let (x, y, w, h) = panel.stage().check;
    if w < 80.0 || h <= 0.0 {
        return;
    }
    draw::card(cr, x, y, w, h);
    panel.sink_names = graph
        .nodes
        .iter()
        .filter(|node| node.media_class == "Audio/Sink" && (node.name.starts_with("alsa_output") || node.name.starts_with("bluez_output")))
        .map(|node| node.name.clone())
        .collect();
    draw::label(cr, fonts, true, 20.0, TEXT, x + 20.0, y + 40.0, t.outputs);
    let sides = config::side_count(panel.config.outputs);
    let mut row = y + 64.0;
    for index in 0..sides {
        if row + 56.0 > y + h - 12.0 {
            break;
        }
        bus_row(panel, cr, fonts, t, graph, true, index, &panel.config.a[index].clone(), x + 16.0, row, w - 32.0);
        row += 58.0;
    }
    for index in 0..sides {
        if row + 56.0 > y + h - 12.0 {
            break;
        }
        bus_row(panel, cr, fonts, t, graph, false, index, &panel.config.b[index].clone(), x + 16.0, row, w - 32.0);
        row += 58.0;
    }
}

fn bus_row(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, graph: &Graph, a_side: bool, index: usize, bus: &Bus, x: f64, y: f64, w: f64) {
    let label = if a_side && index == 0 {
        t.speakers.to_string()
    } else if !a_side && index == 0 {
        t.frame_mix.to_string()
    } else if a_side {
        if bus.target.is_empty() { t.choose_device.to_string() } else { device_label(graph, &bus.target) }
    } else if bus.name.is_empty() {
        format!("B{}", index + 1)
    } else {
        bus.name.clone()
    };
    let tag = format!("{}{}", if a_side { "A" } else { "B" }, index + 1);
    let ink = if a_side { TEXT } else { draw::accent() };
    draw::label(cr, fonts, true, 13.0, ink, x, y + 18.0, &tag);
    draw::label_fit(cr, fonts, false, 14.0, TEXT, x + 36.0, y + 18.0, w - 150.0, &label);
    if a_side && index > 0 {
        panel.hot(&format!("adev:{index}"), x + 36.0, y, w - 150.0, 28.0);
    }
    if !a_side && index > 0 {
        panel.hot(&format!("bname:{index}"), x + 36.0, y, w - 150.0, 28.0);
    }
    let fader = draw::paint_side_fader(
        cr,
        fonts,
        x + w - 210.0,
        y + 8.0,
        150.0,
        36.0,
        bus.gain_db,
        0.0,
        false,
        bus.muted,
        draw::FaderGlyph::Speaker,
        false,
    );
    let side = if a_side { "a" } else { "b" };
    panel.fader_hit(&format!("bus:{side}:{index}"), fader.capsule_x, fader.capsule_y, fader.capsule_w, fader.capsule_h, true);
    let mute_fill = if bus.muted { draw::DANGER } else { INSET };
    let mute_ink = if bus.muted { draw::BG } else { MUTED };
    fill_round(cr, x + w - 52.0, y + 10.0, 44.0, 28.0, 8.0, mute_fill);
    draw::label_mid(cr, fonts, true, 12.0, mute_ink, x + w - 52.0, y + 24.0, "M");
    panel.hot(&format!("bmute:{side}:{index}"), x + w - 52.0, y + 10.0, 44.0, 28.0);
}

fn device_label(graph: &Graph, name: &str) -> String {
    graph
        .nodes
        .iter()
        .find(|node| node.name == name)
        .map(|node| if node.description.is_empty() { node.name.clone() } else { node.description.clone() })
        .unwrap_or_else(|| name.to_string())
}
