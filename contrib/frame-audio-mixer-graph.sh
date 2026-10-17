#!/bin/bash
# Frame Audio Mixer graph. These are clients of the PipeWire that is already
# running, so they can see the headset mic and the speakers.
#
#   frame-audio-mixer-graph.sh core
#   frame-audio-mixer-graph.sh channel ch3
#
# The mic is whatever is the current default source (the built-in mic after
# Valve's filters, or an external mic). WirePlumber pins frame_amix_from_mic
# to that source. Every capture is node.virtual, so this graph does not turn
# Valve's mic filters on by itself.
set -uo pipefail

mode="${1:-}"
id="${2:-}"
name_dir="${XDG_CONFIG_HOME:-$HOME/.config}/pipewire/frame-audio-mixer"

pids=()
cleanup() {
    local pid
    for pid in "${pids[@]}"; do
        kill "$pid" 2>/dev/null || true
    done
    wait 2>/dev/null || true
}
trap cleanup EXIT INT TERM

start() {
    pw-loopback "$@" &
    pids+=("$!")
}

if [[ "$mode" == "core" ]]; then
    # Bus, then the source VRLink records. priority.session 1 so WirePlumber
    # never picks either node as a default.
    start \
        --capture-props="{ media.class = Audio/Sink node.name = frame_amix_bus node.description = \"Frame Mix Bus\" node.virtual = true audio.position = [ FL FR ] priority.session = 1 media.name = \"Frame Mix Bus\" }" \
        --playback-props="{ media.class = Audio/Source node.name = frame_amix_source node.description = \"Frame Mic + Mix\" audio.position = [ FL FR ] priority.session = 1 media.name = \"Frame Mic + Mix\" }"
    # No target.object: the WirePlumber script pins this to the current default
    # mic. node.passive is false because the graph only exists while Mix is on,
    # and a passive capture stays silent when VRLink is the only listener.
    # node.dont-fallback keeps this playback on the bus when the bus restarts.
    start \
        --capture-props="{ node.name = frame_amix_from_mic node.passive = false node.virtual = true audio.position = [ FL FR ] stream.dont-remix = true media.name = \"Frame Mix from mic\" }" \
        --playback-props="{ node.name = frame_amix_mic_virtual_sink target.object = frame_amix_bus node.passive = true node.virtual = true node.dont-fallback = true audio.position = [ FL FR ] media.name = \"Frame Mix mic level\" }"
elif [[ "$mode" == "channel" ]]; then
    # Same rule as is_channel_id in src/config.rs (ch1 to ch32).
    if [[ ! "$id" =~ ^ch[1-9][0-9]?$ ]] || (( 10#${id#ch} > 32 )); then
        echo "frame-audio-mixer: bad channel id: $id" >&2
        exit 1
    fi
    name_file="$name_dir/$id.name"
    if [[ ! -f "$name_file" ]]; then
        echo "frame-audio-mixer: missing $name_file" >&2
        exit 1
    fi
    name="$(head -n 1 "$name_file" | tr -d '\r')"
    esc="${name//\\/\\\\}"
    esc="${esc//\"/\\\"}"
    # Apps select this sink ("Frame Mix: Music"). It is not node.virtual, so it
    # shows up in an app's output list. Frame Mic Tuner only lists alsa_output.*
    # and bluez_output.*, so it still ignores this sink. priority.session 1 so
    # it never becomes the default output.
    start \
        --capture-props="{ media.class = Audio/Sink node.name = frame_amix_${id} node.description = \"Frame Mix: ${esc}\" audio.position = [ FL FR ] priority.session = 1 media.name = \"Frame Mix ${id}\" }" \
        --playback-props="{ node.name = frame_amix_${id}_virtual_sink_playback node.passive = true node.virtual = true audio.position = [ FL FR ] media.name = \"Frame Mix ${id} headphones\" }"
    # Monitor of that sink into the mix bus. The "to mic" slider is this playback.
    start \
        --capture-props="{ node.name = frame_amix_${id}_from_monitor stream.capture.sink = true target.object = frame_amix_${id} node.passive = true node.virtual = true audio.position = [ FL FR ] media.name = \"Frame Mix ${id} monitor\" }" \
        --playback-props="{ node.name = frame_amix_${id}_virtual_sink_to_bus target.object = frame_amix_bus node.passive = true node.virtual = true node.dont-fallback = true audio.position = [ FL FR ] media.name = \"Frame Mix ${id} mic\" }"
else
    echo "usage: frame-audio-mixer-graph.sh core | channel chN" >&2
    exit 2
fi

# If either loopback exits, stop the other and let systemd restart the unit.
wait -n
exit 1
