//! Signal Flow page: the live PipeWire routing as a lane graph.
//!
//! Four lanes run left to right: sources (the mic and app streams), the mixer
//! strips, the outputs, and the devices they play on. Edges come from the saved
//! routing; each one is checked against the engine's live links, so a route
//! whose PipeWire link is gone shows as a dashed red line instead of silently
//! looking fine. Drag pans, the stick scrolls, and selecting a node shows the
//! PipeWire node behind it.

use super::{fill_round, stroke_round, Panel};
use crate::config;
use crate::draw::{self, Device, Fonts, Look, Rgb, INSET, MUTED, TEXT};
use crate::graph::Graph;
use crate::i18n::Text;
use crate::worker::Snapshot;
use cairo::Context;
use std::collections::HashMap;

pub(crate) const NODE_W: f64 = 196.0;
pub(crate) const NODE_H: f64 = 56.0;
const LANE_GAP: f64 = 96.0;
const V_GAP: f64 = 16.0;
const LANE_TITLE_H: f64 = 34.0;
const PAD: f64 = 16.0;

/// Pan, zoom and selection of the Signal Flow page.
pub(crate) struct FlowView {
    pub pan: (f64, f64),
    /// None fits the whole graph; Some is a user zoom.
    pub zoom: Option<f64>,
    /// Effective scale of the last frame, the base for the zoom buttons.
    pub scale: f64,
    pub sel: Option<String>,
    /// Canvas rectangle of the last frame. Empty space here drag-pans.
    pub canvas: Option<(f64, f64, f64, f64)>,
}

impl FlowView {
    pub fn new() -> Self {
        Self { pan: (0.0, 0.0), zoom: None, scale: 1.0, sel: None, canvas: None }
    }

    pub fn zoom_by(&mut self, factor: f64) {
        self.zoom = Some((self.scale * factor).clamp(0.35, 2.5));
    }

    pub fn fit(&mut self) {
        self.zoom = None;
        self.pan = (0.0, 0.0);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NodeIcon {
    Mic,
    App,
    Channel,
    Fx(usize),
    Pc,
    Bus,
    Device(Device),
}

struct Node {
    /// Selection key and hot id suffix.
    key: String,
    label: String,
    sub: String,
    lane: usize,
    icon: NodeIcon,
    /// Post-fader peak, when this node has a meter.
    level: Option<f64>,
    /// The PipeWire node behind this tile, for the detail bar.
    pw_name: String,
    /// A placeholder (the PC while not streaming) draws dimmed.
    present: bool,
    muted: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EdgeState {
    /// The PipeWire link exists.
    Live,
    /// The route is configured but its link is gone.
    Missing,
    /// Nothing is expected to flow (the PC while not streaming).
    Idle,
}

struct Edge {
    from: usize,
    to: usize,
    state: EdgeState,
    /// Peak of the feeding node, for the glow on a live edge.
    level: f64,
    muted: bool,
}

struct Model {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

fn peak(peaks: &HashMap<String, f32>, keys: &[&str]) -> f64 {
    keys.iter().filter_map(|key| peaks.get(*key)).fold(0.0f32, |a, b| a.max(*b)) as f64
}

/// Strip DSP node, the output side of every route the engine makes.
fn dsp_name(id: &str) -> String {
    format!("frame_amix_{id}_virtual_sink_dsp")
}

/// The lane graph for the current config and live graph.
fn model(panel: &Panel, t: &Text, snap: &Snapshot, peaks: &HashMap<String, f32>) -> Model {
    let graph = &snap.graph;
    let config = &panel.config;
    let mut nodes: Vec<Node> = Vec::new();
    let mut edges: Vec<Edge> = Vec::new();

    // Lane 1 first: the mixer strips give the sources something to point at.
    let mic_strip = nodes.len();
    nodes.push(Node {
        key: "mic".into(),
        label: t.mic.to_string(),
        sub: super::source_label(&config.mic_source, graph, t),
        lane: 1,
        icon: NodeIcon::Mic,
        level: Some(peak(peaks, &["mic"])),
        pw_name: "frame_amix_mic_virtual_sink".into(),
        present: graph.node_by_name("frame_amix_mic_virtual_sink").is_some(),
        muted: config.mic_muted,
    });
    let mut strip_of: HashMap<String, usize> = HashMap::new();
    for channel in &config.channels {
        let sink = format!("frame_amix_{}", channel.id);
        strip_of.insert(channel.id.clone(), nodes.len());
        nodes.push(Node {
            key: channel.id.clone(),
            label: channel.name.clone(),
            sub: String::new(),
            lane: 1,
            icon: if channel.app.is_empty() { NodeIcon::Channel } else { NodeIcon::App },
            level: Some(peak(peaks, &[&format!("{}:l", channel.id), &format!("{}:r", channel.id)])),
            present: graph.node_by_name(&sink).is_some(),
            pw_name: sink,
            muted: channel.muted,
        });
    }
    let mut fx_nodes = [0usize; 2];
    for (index, fx) in [&config.fx_a, &config.fx_b].into_iter().enumerate() {
        let names: Vec<&str> = fx.chain.iter().map(|slot| crate::i18n::effect_name(t, slot.kind)).collect();
        let name = format!("frame_amix_fx{}_virtual_sink", index + 1);
        fx_nodes[index] = nodes.len();
        nodes.push(Node {
            key: format!("fx{}", index + 1),
            label: format!("FX{}", index + 1),
            sub: names.join(" · "),
            lane: 1,
            icon: NodeIcon::Fx(index),
            level: None,
            present: graph.node_by_name(&name).is_some(),
            pw_name: name,
            muted: fx.muted,
        });
    }

    // Lane 0: the microphone source, and every app stream feeding a channel.
    let mic_source = super::resolved_source(&config.mic_source, graph);
    let mic_src_node = nodes.len();
    {
        let (label, present) = match &mic_source {
            Some(name) => {
                let desc = graph.description_of(name);
                (if desc.is_empty() { name.clone() } else { desc }, true)
            }
            None => (t.mic_system.to_string(), false),
        };
        nodes.push(Node {
            key: "mic-src".into(),
            label,
            sub: t.mic.to_string(),
            lane: 0,
            icon: NodeIcon::Mic,
            level: None,
            pw_name: mic_source.clone().unwrap_or_default(),
            present,
            muted: false,
        });
    }
    let mic_live = mic_source
        .as_deref()
        .is_some_and(|source| graph.linked(source, "frame_amix_mic_from_mic"));
    edges.push(Edge {
        from: mic_src_node,
        to: mic_strip,
        state: if mic_live { EdgeState::Live } else { EdgeState::Missing },
        level: peak(peaks, &["mic"]),
        muted: config.mic_muted,
    });
    for stream in graph.playback_apps() {
        let Some((channel, strip)) = config
            .channels
            .iter()
            .find(|channel| graph.linked_into(stream.id, &format!("frame_amix_{}", channel.id)))
            .and_then(|channel| strip_of.get(&channel.id).map(|index| (channel, *index)))
        else {
            continue;
        };
        let label = if stream.app_name.is_empty() { stream.binary.clone() } else { stream.app_name.clone() };
        let from = nodes.len();
        nodes.push(Node {
            key: format!("app:{}", stream.id),
            label,
            sub: stream.media_name.clone(),
            lane: 0,
            icon: NodeIcon::App,
            level: None,
            pw_name: stream.name.clone(),
            present: true,
            muted: false,
        });
        edges.push(Edge {
            from,
            to: strip,
            state: EdgeState::Live,
            level: peak(peaks, &[&format!("{}:l", channel.id), &format!("{}:r", channel.id)]),
            muted: channel.muted,
        });
    }

    // Lane 2: the PC send, the listen outputs that exist, and the virtual mics.
    let sinks = graph.output_sinks();
    let pc_out = nodes.len();
    nodes.push(Node {
        key: "out:pc".into(),
        label: t.pc_stream.to_string(),
        sub: "Frame Mix".into(),
        lane: 2,
        icon: NodeIcon::Pc,
        level: Some(peak(peaks, &["out:pc:l", "out:pc:r"])),
        pw_name: "frame_amix_pc_virtual_sink".into(),
        present: graph.node_by_name("frame_amix_pc_virtual_sink").is_some(),
        muted: config.pc.muted,
    });
    let mut a_outs: [Option<usize>; config::A_SLOTS] = [None; config::A_SLOTS];
    for (index, sink) in sinks.iter().enumerate() {
        if index > 0 && sink.is_none() {
            continue;
        }
        let label = match index {
            0 => t.speakers.to_string(),
            1 => t.bluetooth.to_string(),
            _ => t.usb.to_string(),
        };
        let name = format!("frame_amix_a{}_virtual_sink", index + 1);
        a_outs[index] = Some(nodes.len());
        nodes.push(Node {
            key: format!("out:a:{index}"),
            label: format!("A{} · {label}", index + 1),
            sub: String::new(),
            lane: 2,
            icon: NodeIcon::Bus,
            level: Some(peak(peaks, &[&format!("out:a:{index}:l"), &format!("out:a:{index}:r")])),
            present: graph.node_by_name(&name).is_some(),
            pw_name: name,
            muted: config.a[index].muted,
        });
    }
    let mut b_outs = [0usize; config::B_SLOTS];
    for (index, slot) in b_outs.iter_mut().enumerate() {
        let custom = &config.b[index].name;
        let label = if custom.is_empty() { format!("B{}", index + 1) } else { custom.clone() };
        let name = format!("frame_amix_b{}_virtual_sink", index + 1);
        *slot = nodes.len();
        nodes.push(Node {
            key: format!("out:b:{index}"),
            label,
            sub: t.virtual_mic.to_string(),
            lane: 2,
            icon: NodeIcon::Bus,
            level: Some(peak(peaks, &[&format!("out:b:{index}:l"), &format!("out:b:{index}:r")])),
            present: graph.node_by_name(&name).is_some(),
            pw_name: name,
            muted: config.b[index].muted,
        });
    }

    // Edges out of every strip, checked against the engine's live links.
    let mut strip_routes = |from: usize, id: &str, bits: u8, to_pc: bool, fx1: bool, fx2: bool, muted: bool, level: f64| {
        let dsp = dsp_name(id);
        let mut push = |to: usize, sink: &str| {
            let live = graph.linked(&dsp, sink);
            edges.push(Edge { from, to, state: if live { EdgeState::Live } else { EdgeState::Missing }, level, muted });
        };
        if to_pc {
            push(pc_out, "frame_amix_pc_virtual_sink");
        }
        for (index, out) in a_outs.iter().enumerate() {
            if let Some(out) = out {
                if config::route_on(bits, true, index) {
                    push(*out, &format!("frame_amix_a{}_virtual_sink", index + 1));
                }
            }
        }
        for (index, out) in b_outs.iter().enumerate() {
            if config::route_on(bits, false, index) {
                push(*out, &format!("frame_amix_b{}_virtual_sink", index + 1));
            }
        }
        if fx1 {
            push(fx_nodes[0], "frame_amix_fx1_virtual_sink");
        }
        if fx2 {
            push(fx_nodes[1], "frame_amix_fx2_virtual_sink");
        }
    };
    let mic_level = peak(peaks, &["mic"]);
    strip_routes(mic_strip, "mic", config.mic_routes, config.mic_to_pc, config.mic_send_fx1, config.mic_send_fx2, config.mic_muted, mic_level);
    for channel in &config.channels {
        if let Some(&from) = strip_of.get(&channel.id) {
            let level = peak(peaks, &[&format!("{}:l", channel.id), &format!("{}:r", channel.id)]);
            strip_routes(from, &channel.id, channel.routes, channel.to_pc, channel.send_fx1, channel.send_fx2, channel.muted, level);
        }
    }
    for (index, fx) in [&config.fx_a, &config.fx_b].into_iter().enumerate() {
        strip_routes(fx_nodes[index], &format!("fx{}", index + 1), fx.routes, fx.to_pc, false, false, fx.muted, 0.0);
    }

    // Lane 3: the real devices, and whoever records the PC stream and the virtual mics.
    let mut device_of: HashMap<String, usize> = HashMap::new();
    for (index, sink) in sinks.iter().enumerate() {
        let (Some(out), Some(sink)) = (a_outs[index], sink.as_deref()) else { continue };
        let from_level = nodes[out].level.unwrap_or(0.0);
        let play = format!("frame_amix_a{}_virtual_sink_play", index + 1);
        let live = graph.linked(&play, sink);
        let desc = graph.description_of(sink);
        let to = nodes.len();
        device_of.insert(sink.to_string(), to);
        nodes.push(Node {
            key: format!("dev:a:{index}"),
            label: if desc.is_empty() { sink.to_string() } else { desc },
            sub: String::new(),
            lane: 3,
            icon: NodeIcon::Device(Device::of_sink(sink)),
            level: None,
            pw_name: sink.to_string(),
            present: true,
            muted: false,
        });
        edges.push(Edge {
            from: out,
            to,
            state: if live { EdgeState::Live } else { EdgeState::Missing },
            level: from_level,
            muted: nodes[out].muted,
        });
    }
    let vrlink = graph.nodes.iter().find(|node| node.media_class == "Stream/Input/Audio" && Graph::is_vrlink(node));
    {
        let to = nodes.len();
        let (state, sub, pw_name, present) = match (vrlink, graph.node_by_name("frame_amix_source")) {
            (Some(stream), Some(source)) => {
                let live = graph.edges.contains(&(source.id, stream.id));
                (
                    if live { EdgeState::Live } else { EdgeState::Missing },
                    "VRLink".to_string(),
                    stream.name.clone(),
                    true,
                )
            }
            _ => (EdgeState::Idle, t.flow_idle.to_string(), String::new(), false),
        };
        nodes.push(Node {
            key: "dev:pc".into(),
            label: t.pc_short.to_string(),
            sub,
            lane: 3,
            icon: NodeIcon::Pc,
            level: None,
            pw_name,
            present,
            muted: false,
        });
        let level = nodes[pc_out].level.unwrap_or(0.0);
        edges.push(Edge { from: pc_out, to, state, level, muted: config.pc.muted });
    }
    for (index, out) in b_outs.iter().enumerate() {
        let source = format!("frame_amix_b{}", index + 1);
        let Some(source_node) = graph.node_by_name(&source) else { continue };
        for stream in graph.nodes.iter().filter(|node| {
            node.media_class == "Stream/Input/Audio"
                && !node.name.starts_with("frame_amix_")
                && graph.edges.contains(&(source_node.id, node.id))
        }) {
            let label = if stream.app_name.is_empty() { stream.binary.clone() } else { stream.app_name.clone() };
            let to = nodes.len();
            nodes.push(Node {
                key: format!("dev:b:{}", stream.id),
                label,
                sub: t.virtual_mic.to_string(),
                lane: 3,
                icon: NodeIcon::App,
                level: None,
                pw_name: stream.name.clone(),
                present: true,
                muted: false,
            });
            edges.push(Edge {
                from: *out,
                to,
                state: EdgeState::Live,
                level: nodes[*out].level.unwrap_or(0.0),
                muted: config.b[index].muted,
            });
        }
    }

    // Behind the "Other apps" toggle: everything playing outside the mixer, on
    // its way to the normal output.
    if config.flow_apps {
        for stream in graph.playback_apps() {
            let on_channel = config
                .channels
                .iter()
                .any(|channel| graph.linked_into(stream.id, &format!("frame_amix_{}", channel.id)));
            if on_channel {
                continue;
            }
            let label = if stream.app_name.is_empty() { stream.binary.clone() } else { stream.app_name.clone() };
            let from = nodes.len();
            nodes.push(Node {
                key: format!("oapp:{}", stream.id),
                label,
                sub: stream.media_name.clone(),
                lane: 0,
                icon: NodeIcon::App,
                level: None,
                pw_name: stream.name.clone(),
                present: true,
                muted: false,
            });
            // The real sink this stream plays on; a corked stream points at the default.
            let playing_on = graph
                .edges
                .iter()
                .filter(|(out, _)| *out == stream.id)
                .find_map(|(_, input)| {
                    graph
                        .nodes
                        .iter()
                        .find(|node| node.id == *input && node.media_class == "Audio/Sink" && !node.name.starts_with("frame_amix_"))
                });
            let (sink_name, state) = match playing_on {
                Some(node) => (node.name.clone(), EdgeState::Live),
                None => (graph.default_sink.clone(), EdgeState::Idle),
            };
            if sink_name.is_empty() {
                continue;
            }
            let to = *device_of.entry(sink_name.clone()).or_insert_with(|| {
                let desc = graph.description_of(&sink_name);
                let index = nodes.len();
                nodes.push(Node {
                    key: format!("dev:x:{sink_name}"),
                    label: if desc.is_empty() { sink_name.clone() } else { desc },
                    sub: String::new(),
                    lane: 3,
                    icon: NodeIcon::Device(Device::of_sink(&sink_name)),
                    level: None,
                    pw_name: sink_name.clone(),
                    present: true,
                    muted: false,
                });
                index
            });
            edges.push(Edge { from, to, state, level: 0.0, muted: false });
        }
    }

    Model { nodes, edges }
}

/// Node positions in content space, lanes centered on the tallest one.
fn layout(nodes: &[Node]) -> Vec<(f64, f64)> {
    let mut counts = [0usize; 4];
    for node in nodes {
        counts[node.lane.min(3)] += 1;
    }
    let tallest = counts.iter().copied().max().unwrap_or(0) as f64;
    let full_h = tallest * (NODE_H + V_GAP) - V_GAP;
    let mut next = [0usize; 4];
    nodes
        .iter()
        .map(|node| {
            let lane = node.lane.min(3);
            let row = next[lane];
            next[lane] += 1;
            let lane_h = counts[lane] as f64 * (NODE_H + V_GAP) - V_GAP;
            let x = lane as f64 * (NODE_W + LANE_GAP);
            let y = LANE_TITLE_H + (full_h - lane_h) / 2.0 + row as f64 * (NODE_H + V_GAP);
            (x, y)
        })
        .collect()
}

pub(crate) fn draw(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, snap: &Snapshot, peaks: &HashMap<String, f32>) {
    let stage = panel.stage();
    let (x, y) = (stage.mix.0, stage.mix.1);
    let w = stage.w - 2.0 * super::MARGIN;
    let h = stage.mix.3;
    draw::card(cr, x, y, w, h);
    draw::label_fit(cr, fonts, true, 20.0, TEXT, x + PAD, y + 40.0, w * 0.35, t.flow);
    let title_w = draw::text_width(cr, fonts, true, 20.0, t.flow).min(w * 0.35);
    let on = panel.config.flow_apps;
    toggle_chip(panel, cr, fonts, x + PAD + title_w + 24.0, y + 14.0, t.flow_all, on);
    legend(cr, fonts, t, x + w - PAD, y + 18.0);

    let canvas = (x + PAD, y + 56.0, w - 2.0 * PAD, h - 56.0 - PAD);
    panel.flow.canvas = Some(canvas);
    if !snap.mix_active {
        let lines = draw::wrap(cr, fonts, false, 16.0, canvas.2 * 0.7, 3, t.flow_empty);
        let mut ty = canvas.1 + canvas.3 / 2.0 - lines.len() as f64 * 11.0;
        for line in &lines {
            let tw = draw::text_width(cr, fonts, false, 16.0, line);
            draw::label_mid(cr, fonts, false, 16.0, MUTED, canvas.0 + (canvas.2 - tw) / 2.0, ty, line);
            ty += 22.0;
        }
        return;
    }

    let model = model(panel, t, snap, peaks);
    let spots = layout(&model.nodes);
    let content_w = 4.0 * NODE_W + 3.0 * LANE_GAP;
    let content_h = spots
        .iter()
        .map(|(_, sy)| sy + NODE_H)
        .fold(LANE_TITLE_H, f64::max);
    let fit = ((canvas.2 - 8.0) / content_w).min((canvas.3 - 8.0) / content_h).clamp(0.2, 1.2);
    let scale = panel.flow.zoom.unwrap_or(fit);
    panel.flow.scale = scale;
    // Keep some of the graph on screen whatever was dragged.
    let max_x = (content_w * scale - 40.0).max(0.0);
    let max_y = (content_h * scale - 40.0).max(0.0);
    panel.flow.pan.0 = panel.flow.pan.0.clamp(-max_x, max_x);
    panel.flow.pan.1 = panel.flow.pan.1.clamp(-max_y, max_y);
    let origin_x = canvas.0 + (canvas.2 - content_w * scale) / 2.0 - panel.flow.pan.0;
    let origin_y = canvas.1 + (canvas.3 - content_h * scale) / 2.0 - panel.flow.pan.1;
    let place = |index: usize| {
        let (nx, ny) = spots[index];
        (origin_x + nx * scale, origin_y + ny * scale)
    };

    let _ = cr.save();
    cr.rectangle(canvas.0, canvas.1, canvas.2, canvas.3);
    cr.clip();

    // Lane titles pan with the graph, so a zoomed view still says where it is.
    for (lane, title) in [t.flow_sources, t.flow_mixer, t.flow_outputs, t.flow_devices].into_iter().enumerate() {
        let lx = origin_x + lane as f64 * (NODE_W + LANE_GAP) * scale;
        draw::label(cr, fonts, true, 14.0 * scale.clamp(0.75, 1.2), MUTED, lx, origin_y + 14.0 * scale, title);
    }

    let hover = panel
        .pointer
        .filter(|(px, py)| *px >= canvas.0 && *px < canvas.0 + canvas.2 && *py >= canvas.1 && *py < canvas.1 + canvas.3)
        .and_then(|(px, py)| {
            model.nodes.iter().enumerate().find_map(|(index, _)| {
                let (sx, sy) = place(index);
                let (sw, sh) = (NODE_W * scale, NODE_H * scale);
                (px >= sx && px < sx + sw && py >= sy && py < sy + sh).then_some(index)
            })
        });
    let selected = panel.flow.sel.as_deref().and_then(|sel| model.nodes.iter().position(|node| node.key == sel));

    for edge in &model.edges {
        let lit = hover.or(selected).is_some_and(|index| edge.from == index || edge.to == index);
        paint_edge(cr, edge, place(edge.from), place(edge.to), scale, lit);
    }
    for (index, node) in model.nodes.iter().enumerate() {
        let (sx, sy) = place(index);
        paint_node(panel, cr, fonts, node, sx, sy, scale, hover == Some(index), selected == Some(index), canvas);
    }
    let _ = cr.restore();

    detail_bar(panel, cr, fonts, t, &model, selected, canvas);
    zoom_controls(panel, cr, fonts, t, canvas);
}

/// Compact on/off chip: the accent tint and a lit dot while on.
fn toggle_chip(panel: &mut Panel, cr: &Context, fonts: &Fonts, x: f64, y: f64, label: &str, on: bool) {
    let h = 34.0;
    let text_w = draw::text_width(cr, fonts, true, 14.0, label);
    let w = text_w + 46.0;
    let look = panel.look(x, y, w, h);
    let fill = if on {
        draw::accent_tint()
    } else {
        match look {
            Look::Down => draw::CONTROL_DOWN,
            Look::Hover => draw::CONTROL_HOVER,
            Look::Rest => draw::CONTROL,
        }
    };
    fill_round(cr, x, y, w, h, h / 2.0, fill);
    if on && look != Look::Rest {
        stroke_round(cr, x + 1.0, y + 1.0, w - 2.0, h - 2.0, h / 2.0 - 1.0, draw::accent(), 1.5);
    }
    draw::fill_circle(cr, x + 17.0, y + h / 2.0, 4.0, if on { draw::accent() } else { MUTED });
    draw::label_mid(cr, fonts, true, 14.0, if on { TEXT } else { draw::SOFT }, x + 30.0, y + h / 2.0, label);
    panel.hot("flow-apps", x, y, w, h);
}

fn legend(cr: &Context, fonts: &Fonts, t: &Text, right: f64, y: f64) {
    let mut x = right;
    for (label, color, dashed) in [(t.flow_missing, draw::DANGER, true), (t.flow_live, draw::accent(), false)] {
        let tw = draw::text_width(cr, fonts, false, 13.0, label);
        x -= tw;
        draw::label_mid(cr, fonts, false, 13.0, MUTED, x, y + 11.0, label);
        x -= 30.0;
        let _ = cr.save();
        if dashed {
            cr.set_dash(&[4.0, 3.0], 0.0);
        }
        cr.set_line_width(2.5);
        cr.set_source_rgb(color.r, color.g, color.b);
        cr.move_to(x + 2.0, y + 11.0);
        cr.line_to(x + 24.0, y + 11.0);
        let _ = cr.stroke();
        let _ = cr.restore();
        x -= 24.0;
    }
}

fn paint_edge(cr: &Context, edge: &Edge, from: (f64, f64), to: (f64, f64), scale: f64, lit: bool) {
    let x1 = from.0 + NODE_W * scale;
    let y1 = from.1 + NODE_H * scale / 2.0;
    let x2 = to.0;
    let y2 = to.1 + NODE_H * scale / 2.0;
    let reach = ((x2 - x1) * 0.5).max(24.0 * scale);
    let trace = |cr: &Context| {
        cr.move_to(x1, y1);
        cr.curve_to(x1 + reach, y1, x2 - reach, y2, x2, y2);
    };
    let _ = cr.save();
    let alpha = if edge.muted { 0.3 } else if lit { 1.0 } else { 0.75 };
    match edge.state {
        EdgeState::Live => {
            // Audible signal widens the line with a level-tinted glow.
            if edge.level > 0.02 && !edge.muted {
                trace(cr);
                let glow = draw::level_color(edge.level);
                cr.set_source_rgba(glow.r, glow.g, glow.b, 0.35);
                cr.set_line_width((5.0 + 3.0 * edge.level.min(1.0)) * scale.max(0.6));
                let _ = cr.stroke();
            }
            trace(cr);
            let c = draw::accent();
            cr.set_source_rgba(c.r, c.g, c.b, alpha);
            cr.set_line_width(if lit { 3.0 } else { 2.0 });
            let _ = cr.stroke();
        }
        EdgeState::Missing => {
            trace(cr);
            cr.set_dash(&[6.0, 4.0], 0.0);
            cr.set_source_rgba(draw::DANGER.r, draw::DANGER.g, draw::DANGER.b, alpha);
            cr.set_line_width(if lit { 3.0 } else { 2.0 });
            let _ = cr.stroke();
        }
        EdgeState::Idle => {
            trace(cr);
            cr.set_dash(&[2.0, 4.0], 0.0);
            cr.set_source_rgba(MUTED.r, MUTED.g, MUTED.b, 0.5);
            cr.set_line_width(2.0);
            let _ = cr.stroke();
        }
    }
    let _ = cr.restore();
}

fn paint_node(
    panel: &mut Panel,
    cr: &Context,
    fonts: &Fonts,
    node: &Node,
    x: f64,
    y: f64,
    scale: f64,
    hovered: bool,
    selected: bool,
    canvas: (f64, f64, f64, f64),
) {
    let w = NODE_W * scale;
    let h = NODE_H * scale;
    let r = draw::R_CONTROL * scale;
    let fill = if hovered { draw::CONTROL_HOVER } else { INSET };
    let alpha = if node.present { 1.0 } else { 0.55 };
    draw::rounded(cr, x, y, w, h, r);
    draw::fill(cr, fill, alpha);
    if selected {
        stroke_round(cr, x + 1.0, y + 1.0, w - 2.0, h - 2.0, r - 1.0, draw::accent(), 2.0);
    }
    let pad = 10.0 * scale;
    let icon_s = 22.0 * scale;
    let icon_y = y + (h - icon_s) / 2.0;
    paint_icon(cr, node.icon, x + pad, icon_y, icon_s, if node.present { draw::SOFT } else { MUTED });
    let text_x = x + pad + icon_s + 8.0 * scale;
    let meter_w = if node.level.is_some() { 14.0 * scale } else { 0.0 };
    let text_w = (x + w - 8.0 * scale - meter_w - text_x).max(10.0);
    let label_c = if node.muted { MUTED } else { TEXT };
    if node.sub.is_empty() {
        draw::label_fit(cr, fonts, true, 15.0 * scale, label_c, text_x, y + h / 2.0 + 5.0 * scale, text_w, &node.label);
    } else {
        draw::label_fit(cr, fonts, true, 14.0 * scale, label_c, text_x, y + h / 2.0 - 3.0 * scale, text_w, &node.label);
        draw::label_fit(cr, fonts, false, 11.5 * scale, MUTED, text_x, y + h / 2.0 + 13.0 * scale, text_w, &node.sub);
    }
    if let Some(level) = node.level {
        let shown = if node.muted { 0.0 } else { level };
        let dot = 4.5 * scale;
        let cx = x + w - 8.0 * scale - dot;
        let color = if shown > 0.015 { draw::level_color(shown) } else { draw::mix(TEXT, INSET, 0.18) };
        draw::fill_circle(cr, cx, y + h / 2.0, dot, color);
    }
    // Hot area clipped to the canvas, so a tile half out of view cannot be
    // clicked where it is not drawn.
    let hx = x.max(canvas.0);
    let hy = y.max(canvas.1);
    let hw = (x + w).min(canvas.0 + canvas.2) - hx;
    let hh = (y + h).min(canvas.1 + canvas.3) - hy;
    if hw > 0.0 && hh > 0.0 {
        panel.hot(&format!("fnode:{}", node.key), hx, hy, hw, hh);
    }
}

fn paint_icon(cr: &Context, icon: NodeIcon, x: f64, y: f64, s: f64, c: Rgb) {
    match icon {
        NodeIcon::Mic => draw::icon(cr, draw::MIC_PATH, draw::FADER_ICON_VIEW, x, y, s, c),
        NodeIcon::App | NodeIcon::Channel => draw::icon(cr, draw::SPEAKER_PATH, draw::FADER_ICON_VIEW, x, y, s, c),
        NodeIcon::Fx(index) => {
            let color = if index == 0 { draw::FX1 } else { draw::FX2 };
            draw::icon(cr, draw::FX_TAG_PATH, 24.0, x, y, s, color);
        }
        NodeIcon::Pc => draw::pc_icon(cr, x, y, s, c),
        NodeIcon::Bus => draw::icon(cr, draw::HEADPHONES_PATH, 16.0, x, y, s, c),
        NodeIcon::Device(device) => {
            if !draw::device_icon(cr, device, x, y, s, c) {
                draw::icon(cr, draw::SPEAKER_PATH, draw::FADER_ICON_VIEW, x, y, s, c);
            }
        }
    }
}

/// The selected node's PipeWire identity, or the pan hint while nothing is selected.
fn detail_bar(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, model: &Model, selected: Option<usize>, canvas: (f64, f64, f64, f64)) {
    let bar_h = 40.0;
    let (cx, cy, cw, ch) = canvas;
    let by = cy + ch - bar_h;
    match selected.map(|index| &model.nodes[index]) {
        Some(node) => {
            fill_round(cr, cx, by, cw, bar_h, draw::R_CONTROL, draw::accent_tint());
            let mid = by + bar_h / 2.0;
            let mut left = cx + 14.0;
            draw::label_mid(cr, fonts, true, 14.0, TEXT, left, mid, &node.label);
            left += draw::text_width(cr, fonts, true, 14.0, &node.label) + 16.0;
            if !node.pw_name.is_empty() {
                draw::label_fit(cr, fonts, false, 12.5, MUTED, left, mid + 4.5, cx + cw - 46.0 - left, &node.pw_name);
            }
            super::close_button(panel, cr, cx + cw - 42.0, by);
            // The close button's own hot already dismisses; it doubles as deselect.
        }
        None => {
            draw::label_mid(cr, fonts, false, 12.5, MUTED, cx + 2.0, by + bar_h - 8.0, t.flow_hint);
        }
    }
}

fn zoom_controls(panel: &mut Panel, cr: &Context, fonts: &Fonts, t: &Text, canvas: (f64, f64, f64, f64)) {
    let s = 40.0;
    let (cx, cy, cw, _) = canvas;
    let fit_w = (draw::text_width(cr, fonts, true, 14.0, t.flow_fit) + 28.0).max(s);
    let mut bx = cx + cw - s;
    let by = cy + 8.0;
    for (id, glyph) in [("flow-zin", "+"), ("flow-zout", "−")] {
        let look = panel.look(bx, by, s, s);
        let fill = match look {
            Look::Down => draw::CONTROL_DOWN,
            Look::Hover => draw::CONTROL_HOVER,
            Look::Rest => draw::CONTROL,
        };
        fill_round(cr, bx, by, s, s, s / 2.0, fill);
        let tw = draw::text_width(cr, fonts, true, 20.0, glyph);
        draw::label_mid(cr, fonts, true, 20.0, TEXT, bx + (s - tw) / 2.0, by + s / 2.0, glyph);
        panel.hot(id, bx, by, s, s);
        bx -= s + 8.0;
    }
    bx -= fit_w - s;
    panel.button(cr, fonts, bx, by, fit_w, s, t.flow_fit, draw::ButtonKind::Plain);
    panel.hot("flow-fit", bx, by, fit_w, s);
}

/// Pure layout checks for the self-test: lanes keep their order and nothing overlaps.
pub(crate) fn self_test() -> Result<(), String> {
    let nodes: Vec<Node> = (0..6)
        .map(|index| Node {
            key: format!("n{index}"),
            label: String::new(),
            sub: String::new(),
            lane: index % 3,
            icon: NodeIcon::Channel,
            level: None,
            pw_name: String::new(),
            present: true,
            muted: false,
        })
        .collect();
    let spots = layout(&nodes);
    if spots.len() != 6 {
        return Err("flow layout count".into());
    }
    for (index, (x, _)) in spots.iter().enumerate() {
        let lane = index % 3;
        if (x - lane as f64 * (NODE_W + LANE_GAP)).abs() > 0.01 {
            return Err(format!("flow lane x {index}"));
        }
    }
    let (first, second) = (spots[0], spots[3]);
    if second.1 - first.1 < NODE_H {
        return Err("flow rows overlap".into());
    }
    let mut view = FlowView::new();
    view.scale = 1.0;
    view.zoom_by(1.25);
    if view.zoom != Some(1.25) {
        return Err("flow zoom".into());
    }
    view.fit();
    if view.zoom.is_some() || view.pan != (0.0, 0.0) {
        return Err("flow fit".into());
    }
    Ok(())
}
