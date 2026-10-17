//! Read a `pw-dump` snapshot. The panel never changes PipeWire itself; the
//! worker turns these facts into `wpctl` and `pw-metadata` calls.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct AudioNode {
    pub id: u32,
    pub serial: u64,
    pub name: String,
    pub description: String,
    pub media_class: String,
    pub media_name: String,
    pub app_name: String,
    pub binary: String,
    pub is_virtual: bool,
    /// Target stored on the node, before metadata overrides it.
    pub target: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppRef {
    pub name: String,
    pub binary: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Graph {
    pub nodes: Vec<AudioNode>,
    pub clients: Vec<AppRef>,
    pub default_source: String,
    pub default_sink: String,
}

pub fn parse_dump(text: &str) -> Result<Graph, String> {
    let value: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let items = value.as_array().ok_or("pw-dump is not a list")?;
    let mut graph = Graph::default();
    for item in items {
        let kind = item.get("type").and_then(Value::as_str).unwrap_or("");
        if kind.ends_with("Metadata") {
            read_defaults(item, &mut graph);
        }
        if kind.ends_with("Client") {
            if let Some(props) = item.pointer("/info/props") {
                graph.clients.push(AppRef {
                    name: prop_str(props, "application.name"),
                    binary: prop_str(props, "application.process.binary"),
                });
            }
            continue;
        }
        if !kind.ends_with("Node") {
            continue;
        }
        let Some(props) = item.pointer("/info/props") else {
            continue;
        };
        let name = prop_str(props, "node.name");
        if name.is_empty() {
            continue;
        }
        let id = item.get("id").and_then(Value::as_u64).unwrap_or(0) as u32;
        let serial = prop_u64(props, "object.serial");
        graph.nodes.push(AudioNode {
            id,
            serial,
            name,
            description: prop_str(props, "node.description"),
            media_class: prop_str(props, "media.class"),
            media_name: prop_str(props, "media.name"),
            app_name: prop_str(props, "application.name"),
            binary: prop_str(props, "application.process.binary"),
            is_virtual: prop_flag(props, "node.virtual"),
            target: {
                let target = prop_str(props, "target.object");
                if target.is_empty() {
                    prop_str(props, "node.target")
                } else {
                    target
                }
            },
        });
    }
    Ok(graph)
}

fn read_defaults(item: &Value, graph: &mut Graph) {
    let name = item.pointer("/info/props/metadata.name").and_then(Value::as_str).unwrap_or("");
    if name != "default" {
        return;
    }
    let Some(entries) = item.pointer("/info/metadata").and_then(Value::as_array) else {
        return;
    };
    for entry in entries {
        let key = entry.get("key").and_then(Value::as_str).unwrap_or("");
        let node = entry.get("value").map(metadata_node_name).unwrap_or_default();
        if key == "default.audio.source" {
            graph.default_source = node;
        } else if key == "default.audio.sink" {
            graph.default_sink = node;
        }
    }
}

/// `default.audio.*` is sometimes a JSON string, sometimes an object, sometimes a bare node name.
fn metadata_node_name(value: &Value) -> String {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                if let Some(name) = parsed.get("name").and_then(Value::as_str) {
                    if !name.is_empty() {
                        return name.to_string();
                    }
                }
                if let Some(name) = parsed.as_str() {
                    if !name.is_empty() {
                        return name.to_string();
                    }
                }
            }
            if !trimmed.is_empty() && !trimmed.starts_with('{') {
                return trimmed.to_string();
            }
            String::new()
        }
        Value::Object(_) => value.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
        _ => String::new(),
    }
}

fn prop_str(props: &Value, key: &str) -> String {
    props.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn prop_flag(props: &Value, key: &str) -> bool {
    match props.get(key) {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => value == "true",
        _ => false,
    }
}

fn prop_u64(props: &Value, key: &str) -> u64 {
    match props.get(key) {
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0),
        Some(Value::String(s)) => s.parse().unwrap_or(0),
        _ => 0,
    }
}

impl Graph {
    pub fn node_by_name(&self, name: &str) -> Option<&AudioNode> {
        self.nodes.iter().find(|node| node.name == name)
    }

    /// `pw-record --target` on this headset follows an object serial, not a node name.
    pub fn link_target(&self, name: &str) -> String {
        match self.node_by_name(name) {
            Some(node) if node.serial != 0 => node.serial.to_string(),
            _ => name.to_string(),
        }
    }

    pub fn description_of(&self, name: &str) -> String {
        self.node_by_name(name)
            .map(|node| {
                if node.description.is_empty() {
                    node.name.clone()
                } else {
                    node.description.clone()
                }
            })
            .unwrap_or_default()
    }

    /// Playback streams an app picker can pin. Loopbacks and our own nodes are skipped.
    /// A spatialized stream is virtual and is still included when it belongs to an app.
    pub fn playback_apps(&self) -> Vec<AudioNode> {
        self.nodes
            .iter()
            .filter(|node| node.media_class == "Stream/Output/Audio")
            .filter(|node| !node.name.starts_with("frame_amix_"))
            .filter(|node| node.binary != "pw-loopback" && node.app_name != "loopback")
            .filter(|node| !is_system_media(&node.media_name))
            .filter(|node| is_user_app(&node.app_name, &node.binary))
            .cloned()
            .collect()
    }

    /// Apps connected to PipeWire, plus apps that are already playing.
    /// A client with no stream yet can still be pinned; the pin applies when it plays.
    pub fn picker_apps(&self) -> Vec<AppRef> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut push = |name: String, binary: String| {
            let label = if name.is_empty() { binary.clone() } else { name };
            if !is_user_app(&label, &binary) {
                return;
            }
            if seen.insert(label.to_lowercase()) {
                out.push(AppRef { name: label, binary });
            }
        };
        for node in self.playback_apps() {
            push(node.app_name, node.binary);
        }
        for client in &self.clients {
            push(client.name.clone(), client.binary.clone());
        }
        out.retain(|app| !self.is_foreign_virtual_output(&app.name));
        out
    }

    /// A virtual sink that is not one of our mixer channels. Those stay out of the Running list.
    fn is_foreign_virtual_output(&self, label: &str) -> bool {
        let label = label.trim();
        if label.is_empty() || label.starts_with("Frame Mix:") || label.starts_with("frame_amix_") {
            return !label.is_empty();
        }
        self.nodes.iter().any(|node| {
            node.media_class == "Audio/Sink"
                && node.is_virtual
                && !node.name.starts_with("frame_amix_")
                && (same_name(&node.name, label) || same_name(&node.description, label) || same_name(&node.app_name, label))
        })
    }

    /// The Frame mic after Valve's noise suppression, echo cancel and EQ.
    /// Frame Mic Tuner configures those filters. This is not the raw ALSA mic.
    pub const BUILTIN_SOURCE: &'static str = "alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source";

    /// Sources the mic picker may offer. The mixer's own nodes, the raw ALSA
    /// mic, Valve's filter nodes, monitors and virtual nodes are left out.
    /// The built-in loopback is offered separately as "builtin".
    pub fn pickable_sources(&self) -> Vec<AudioNode> {
        self.nodes.iter().filter(|node| Self::is_pickable_source(node)).cloned().collect()
    }

    pub fn is_pickable_source(node: &AudioNode) -> bool {
        if node.media_class != "Audio/Source" || node.is_virtual {
            return false;
        }
        let name = node.name.as_str();
        if name.starts_with("frame_amix_") || name == Self::BUILTIN_SOURCE {
            return false;
        }
        if name.starts_with("alsa_input.platform-sound.") {
            return false;
        }
        if name.starts_with("ns_") || name.starts_with("echo_cancel_") || name.starts_with("eq_") {
            return false;
        }
        if name.contains(".monitor") || name.contains("monitor") {
            return false;
        }
        true
    }

    pub fn is_vrlink(node: &AudioNode) -> bool {
        node.media_name == "Valve VRLink Microphone Input"
            || node.app_name == "vrlink"
            || node.binary == "vrlink"
    }
}

fn same_name(left: &str, right: &str) -> bool {
    !left.is_empty() && left.eq_ignore_ascii_case(right)
}

fn is_system_media(name: &str) -> bool {
    name.contains("ALSA internal") || name.contains("Filter Chain") || name.contains("Echo-Cancel")
}

fn is_user_app(name: &str, binary: &str) -> bool {
    if name.is_empty() && binary.is_empty() {
        return false;
    }
    let name = name.to_lowercase();
    let binary = binary.to_lowercase();
    const SKIP: &[&str] = &[
        "pipewire",
        "wireplumber",
        "wireplumber [export]",
        "xdg-desktop-portal",
        "pw-dump",
        "pw-record",
        "pw-play",
        "pw-loopback",
        "pw-cli",
        "wpctl",
        "frame-audio-mixer",
        "systemd",
    ];
    if SKIP.iter().any(|item| name == *item || binary == *item) {
        return false;
    }
    if name.starts_with("wireplumber") || binary == "wireplumber" {
        return false;
    }
    true
}

pub fn self_test() -> Result<(), String> {
    let graph = parse_dump(FIXTURE)?;
    if graph.default_source != "alsa_input.mic" {
        return Err(format!("default source {}", graph.default_source));
    }
    let source = graph.node_by_name("frame_amix_source").ok_or("missing source")?;
    if source.serial != 42 || source.id != 7 {
        return Err("source ids".into());
    }
    let apps = graph.playback_apps();
    if apps.len() != 2 || apps[0].app_name != "Zen" || apps[1].app_name != "Moonlight" {
        return Err(format!("apps {}", apps.len()));
    }
    let picker: Vec<_> = graph.picker_apps().into_iter().map(|app| app.name).collect();
    if picker != vec!["Zen".to_string(), "Moonlight".to_string(), "Steam".to_string()] {
        return Err(format!("picker {picker:?}"));
    }
    if graph.nodes.iter().any(|node| node.name.starts_with("frame_amix_") && node.is_virtual && Graph::is_vrlink(node))
    {
        return Err("virtual node counted as vrlink".into());
    }
    let extra = parse_dump(SOURCE_FIXTURE)?;
    let names: Vec<_> = extra.pickable_sources().into_iter().map(|node| node.name).collect();
    if names != vec!["bluez_input.headset".to_string()] {
        return Err(format!("pickable {names:?}"));
    }
    if graph.link_target("frame_amix_source") != "42" {
        return Err("serial target".into());
    }
    let object_form = r#"[{"id":1,"type":"PipeWire:Interface:Metadata","info":{"props":{"metadata.name":"default"},"metadata":[
        {"key":"default.audio.source","value":{"name":"alsa_input.object"}},
        {"key":"default.audio.sink","value":"alsa_output.bare"}
    ]}}]"#;
    let object = parse_dump(object_form)?;
    if object.default_source != "alsa_input.object" || object.default_sink != "alsa_output.bare" {
        return Err(format!("metadata {} {}", object.default_source, object.default_sink));
    }
    Ok(())
}

const FIXTURE: &str = r#"[
  {"id": 1, "type": "PipeWire:Interface:Metadata", "info": {"props": {"metadata.name": "default"}, "metadata": [
    {"key": "default.audio.source", "value": "{\"name\":\"alsa_input.mic\"}"},
    {"key": "default.audio.sink", "value": "{\"name\":\"alsa_output.platform-sound.HiFi__Speaker__sink\"}"}
  ]}},
  {"id": 7, "type": "PipeWire:Interface:Node", "info": {"props": {
    "node.name": "frame_amix_source", "object.serial": 42, "node.description": "Frame Mic + Mix",
    "media.class": "Audio/Source", "node.virtual": "true"
  }}},
  {"id": 8, "type": "PipeWire:Interface:Node", "info": {"props": {
    "node.name": "frame_amix_from_mic", "object.serial": 43, "media.class": "Stream/Input/Audio", "node.virtual": "true"
  }}},
  {"id": 9, "type": "PipeWire:Interface:Node", "info": {"props": {
    "node.name": "zen-playback", "object.serial": 90, "media.class": "Stream/Output/Audio",
    "application.name": "Zen", "media.name": "Playback", "target.object": "frame_amix_ch1"
  }}},
  {"id": 10, "type": "PipeWire:Interface:Node", "info": {"props": {
    "node.name": "frame_amix_ch1_virtual_sink_playback", "object.serial": 91,
    "media.class": "Stream/Output/Audio", "node.virtual": "true", "application.name": "loopback"
  }}},
  {"id": 11, "type": "PipeWire:Interface:Node", "info": {"props": {
    "node.name": "moonlight-playback", "object.serial": 92, "media.class": "Stream/Output/Audio",
    "application.name": "Moonlight", "node.virtual": true, "media.name": "Playback"
  }}},
  {"id": 12, "type": "PipeWire:Interface:Node", "info": {"props": {
    "node.name": "filter-chain-playback", "media.class": "Stream/Output/Audio",
    "media.name": "Filter Chain Sink", "node.virtual": true
  }}},
  {"id": 20, "type": "PipeWire:Interface:Client", "info": {"props": {
    "application.name": "Steam", "application.process.binary": "steam"
  }}},
  {"id": 21, "type": "PipeWire:Interface:Client", "info": {"props": {
    "application.name": "WirePlumber", "application.process.binary": "wireplumber"
  }}},
  {"id": 22, "type": "PipeWire:Interface:Client", "info": {"props": {
    "application.name": "Zen", "application.process.binary": "zen"
  }}},
  {"id": 30, "type": "PipeWire:Interface:Node", "info": {"props": {
    "node.name": "easyeffects_sink", "node.description": "Easy Effects",
    "media.class": "Audio/Sink", "node.virtual": true
  }}},
  {"id": 31, "type": "PipeWire:Interface:Client", "info": {"props": {
    "application.name": "Easy Effects", "application.process.binary": "easyeffects"
  }}}
]"#;

const SOURCE_FIXTURE: &str = r#"[
  {"id": 1, "type": "PipeWire:Interface:Node", "info": {"props": {
    "node.name": "alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source",
    "media.class": "Audio/Source", "node.description": "Built-in Audio"
  }}},
  {"id": 2, "type": "PipeWire:Interface:Node", "info": {"props": {
    "node.name": "alsa_input.platform-sound.HiFi__Mic__source",
    "media.class": "Audio/Source", "node.description": "Raw"
  }}},
  {"id": 3, "type": "PipeWire:Interface:Node", "info": {"props": {
    "node.name": "ns_source", "media.class": "Audio/Source", "node.virtual": "true"
  }}},
  {"id": 4, "type": "PipeWire:Interface:Node", "info": {"props": {
    "node.name": "echo_cancel_source", "media.class": "Audio/Source"
  }}},
  {"id": 5, "type": "PipeWire:Interface:Node", "info": {"props": {
    "node.name": "eq_source", "media.class": "Audio/Source"
  }}},
  {"id": 6, "type": "PipeWire:Interface:Node", "info": {"props": {
    "node.name": "frame_amix_source", "media.class": "Audio/Source"
  }}},
  {"id": 7, "type": "PipeWire:Interface:Node", "info": {"props": {
    "node.name": "bluez_input.headset.monitor", "media.class": "Audio/Source"
  }}},
  {"id": 8, "type": "PipeWire:Interface:Node", "info": {"props": {
    "node.name": "bluez_input.headset", "media.class": "Audio/Source", "node.description": "Headset"
  }}}
]"#;
