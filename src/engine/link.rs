//! Which ports should be linked. Node names are the stable identity; port ids
//! come from the live graph and are matched channel to channel.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    In,
    Out,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Port {
    pub id: u32,
    pub node_id: u32,
    pub direction: Direction,
    pub channel: String,
}

/// FL goes to FL. A stereo pair that has only arrived on one side does not
/// fan out onto the other channel. A genuine mono port does.
pub fn match_ports<'a>(outs: &[&'a Port], ins: &[&'a Port]) -> Vec<(&'a Port, &'a Port)> {
    let mut pairs = Vec::new();
    for output in outs {
        let matched: Vec<_> = ins.iter().copied().filter(|input| input.channel == output.channel).collect();
        if !matched.is_empty() {
            pairs.extend(matched.into_iter().map(|input| (*output, input)));
        } else if outs.len() == 1 {
            pairs.extend(ins.iter().copied().map(|input| (*output, input)));
        }
    }
    // A Bluetooth headset in hands-free mode has one MONO input. Both sides go into it.
    if pairs.is_empty() && ins.len() == 1 && ins[0].channel == "MONO" {
        pairs.extend(outs.iter().copied().map(|output| (output, ins[0])));
    }
    if pairs.is_empty() && !outs.is_empty() && outs.len() == ins.len() {
        pairs.extend(outs.iter().copied().zip(ins.iter().copied()));
    }
    pairs
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Wanted {
    pub output_node: String,
    pub input_node: String,
}

/// The links a route set implies, as (output port id, input port id, output node id, input node id).
pub fn wanted_links(
    routes: &[Wanted],
    nodes: &HashMap<String, u32>,
    ports: &[Port],
) -> Vec<(u32, u32, u32, u32)> {
    let mut out = Vec::new();
    for route in routes {
        let (Some(&output_id), Some(&input_id)) = (nodes.get(&route.output_node), nodes.get(&route.input_node)) else {
            continue;
        };
        let outs: Vec<&Port> = ports.iter().filter(|port| port.node_id == output_id && port.direction == Direction::Out).collect();
        let ins: Vec<&Port> = ports.iter().filter(|port| port.node_id == input_id && port.direction == Direction::In).collect();
        for (output, input) in match_ports(&outs, &ins) {
            out.push((output.id, input.id, output_id, input_id));
        }
    }
    out
}

pub fn self_test() -> Result<(), String> {
    let left_out = Port { id: 1, node_id: 10, direction: Direction::Out, channel: "FL".into() };
    let right_out = Port { id: 2, node_id: 10, direction: Direction::Out, channel: "FR".into() };
    let left_in = Port { id: 3, node_id: 20, direction: Direction::In, channel: "FL".into() };
    let right_in = Port { id: 4, node_id: 20, direction: Direction::In, channel: "FR".into() };
    let outs = [&left_out, &right_out];
    let ins = [&right_in, &left_in];
    let pairs = match_ports(&outs, &ins);
    if pairs.len() != 2 || pairs.iter().any(|(o, i)| o.channel != i.channel) {
        return Err(format!("channel pair {pairs:?}"));
    }
    let half = match_ports(&[&left_out], &[&left_in, &right_in]);
    if half.len() != 1 || half[0].1.channel != "FL" {
        return Err("half-arrived stereo fanned out".into());
    }
    let mono = Port { id: 5, node_id: 10, direction: Direction::Out, channel: "MONO".into() };
    let fanned = match_ports(&[&mono], &[&left_in, &right_in]);
    if fanned.len() != 2 {
        return Err("mono did not fan out".into());
    }
    let mono_in = Port { id: 6, node_id: 20, direction: Direction::In, channel: "MONO".into() };
    let folded = match_ports(&outs, &[&mono_in]);
    if folded.len() != 2 || folded.iter().any(|(_, input)| input.id != 6) {
        return Err("stereo did not fold into a mono sink".into());
    }
    Ok(())
}
