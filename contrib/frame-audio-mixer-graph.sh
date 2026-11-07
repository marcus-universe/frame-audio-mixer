#!/bin/bash
# Frame Audio Mixer graph. These are clients of the PipeWire that is already
# running, so they can see the headset mic and the speakers.
#
#   frame-audio-mixer-graph.sh core
#   frame-audio-mixer-graph.sh channel ch3
#   frame-audio-mixer-graph.sh fx a-pc
#
# The mic is whatever is the current default source (the built-in mic after
# Valve's filters, or an external mic). WirePlumber pins frame_amix_from_mic
# to that source. Every capture is node.virtual, so this graph does not turn
# Valve's mic filters on by itself.
set -uo pipefail

mode="${1:-}"
id="${2:-}"
name_dir="${XDG_CONFIG_HOME:-$HOME/.config}/pipewire/frame-audio-mixer"
# The headset graph is 48 kHz and the speaker period is 480 samples (10 ms).
# A different period, including 512, is a second clock: the ALSA device then
# counts xruns and a 512-frame effect block drops one 480-frame hole about
# every 160 ms. Matching 480 lets a new channel join without retiming the
# graph. No force-quantum: that would retime every other stream.
lock="audio.rate = 48000 node.latency = 480/48000 stream.dont-remix = true"

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
    # The bus is a sink. One pw-loopback cannot be both that sink and Frame Mix:
    # the playback end stays silent while the sink inputs are live. The playback
    # side is only discarded. Its name contains virtual and sink, so the
    # spatializer does not play the mix on the speakers. priority.session 1 so
    # WirePlumber never picks the bus as a default.
    start \
        --capture-props="{ media.class = Audio/Sink node.name = frame_amix_bus node.description = \"Frame Mix Bus\" node.virtual = true audio.position = [ FL FR ] priority.session = 1 media.name = \"Frame Mix Bus\" $lock }" \
        --playback-props="{ node.name = frame_amix_bus_virtual_sink_discard node.passive = true node.virtual = true node.dont-fallback = true audio.position = [ FL FR ] media.name = \"Frame Mix Bus discard\" $lock }"
    # Frame Mix records the bus monitor, the same way a channel monitor records
    # its sink. Not passive, so the copy runs while VRLink or Mix Check listens.
    start \
        --capture-props="{ node.name = frame_amix_from_bus stream.capture.sink = true target.object = frame_amix_bus node.passive = false node.virtual = true node.dont-fallback = true audio.position = [ FL FR ] media.name = \"Frame Mix from bus\" $lock }" \
        --playback-props="{ media.class = Audio/Source node.name = frame_amix_source node.description = \"Frame Mix\" node.nick = \"Frame Mix\" device.description = \"Frame Mix\" audio.position = [ FL FR ] priority.session = 1 media.name = \"Frame Mix\" $lock }"
    # No target.object: the WirePlumber script pins this to the current default
    # mic. node.passive is false because the graph only exists while Mix is on,
    # and a passive capture stays silent when VRLink is the only listener.
    # node.dont-fallback keeps this playback on the bus when the bus restarts.
    start \
        --capture-props="{ node.name = frame_amix_from_mic node.passive = false node.virtual = true audio.position = [ FL FR ] media.name = \"Frame Mix from mic\" $lock }" \
        --playback-props="{ node.name = frame_amix_mic_virtual_sink target.object = frame_amix_bus node.passive = true node.virtual = true node.dont-fallback = true audio.position = [ FL FR ] media.name = \"Frame Mix mic level\" $lock }"
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
    # Apps select this sink by the channel name ("DJ"). No "Frame Mix:" prefix:
    # PortAudio and Mixxx split device names on a colon and drop the rest.
    # It is not node.virtual, so it shows up in an app's output list. Frame Mic
    # Tuner only lists alsa_output.* and bluez_output.*, so it still ignores
    # this sink. priority.session 1 so it never becomes the default output.
    start \
        --capture-props="{ media.class = Audio/Sink node.name = frame_amix_${id} node.description = \"${esc}\" node.nick = \"${esc}\" device.description = \"${esc}\" audio.position = [ FL FR ] priority.session = 1 media.name = \"${esc}\" $lock }" \
        --playback-props="{ node.name = frame_amix_${id}_virtual_sink_playback node.passive = true node.virtual = true audio.position = [ FL FR ] media.name = \"Frame Mix ${id} headphones\" $lock }"
    # Monitor of that sink into the mix bus. The "to mic" slider is this playback.
    # Not passive, for the same reason as the mic capture: with both ends passive
    # the bus only ever carried the microphone.
    start \
        --capture-props="{ node.name = frame_amix_${id}_from_monitor stream.capture.sink = true target.object = frame_amix_${id} node.passive = false node.virtual = true audio.position = [ FL FR ] media.name = \"Frame Mix ${id} monitor\" $lock }" \
        --playback-props="{ node.name = frame_amix_${id}_virtual_sink_to_bus target.object = frame_amix_bus node.passive = true node.virtual = true node.dont-fallback = true audio.position = [ FL FR ] media.name = \"Frame Mix ${id} mic\" $lock }"
elif [[ "$mode" == "fx" ]]; then
    # Mix sink for one effect-bus instance: a-pc, a-phones, b-pc, b-phones.
    if [[ "$id" != "a-pc" && "$id" != "a-phones" && "$id" != "b-pc" && "$id" != "b-phones" ]]; then
        echo "frame-audio-mixer: bad fx id: $id" >&2
        exit 1
    fi
    node="frame_amix_fx_${id//-/_}"
    # The playback is a dry copy of whatever was sent here. It has no target,
    # so it follows the speakers. node.mute in these props does not stick;
    # the panel mutes this node. dont-fallback keeps a later target from
    # dropping the stream onto the speakers.
    start \
        --capture-props="{ media.class = Audio/Sink node.name = ${node} node.description = \"Frame Mix FX ${id}\" node.virtual = true audio.position = [ FL FR ] priority.session = 1 media.name = \"Frame Mix FX ${id}\" $lock }" \
        --playback-props="{ node.name = ${node}_virtual_sink_discard node.passive = true node.virtual = true node.dont-fallback = true audio.position = [ FL FR ] media.name = \"Frame Mix FX ${id} discard\" $lock }"
else
    echo "usage: frame-audio-mixer-graph.sh core | channel chN | fx a-pc" >&2
    exit 2
fi

# If either loopback exits, stop the other and let systemd restart the unit.
wait -n
exit 1
