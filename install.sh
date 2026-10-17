#!/usr/bin/env bash
# Install or update Frame Audio Mixer on a Steam Frame. Run it on the headset as the normal user
# (no sudo). From a release archive, or from a git checkout (this builds it):
#   ./install.sh                     build if needed, then install or update
#   ./install.sh --autostart         same, and also start the panel with SteamVR
#   ./install.sh --uninstall         remove the app and its WirePlumber script
#   ./install.sh --uninstall --purge also delete the app's settings and saved frame-audio-mixer.* values
#
# This script never restarts PipeWire or WirePlumber, and never writes outside the home directory.
# A new WirePlumber script is loaded the next time WirePlumber starts (reboot while idle).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
config_home="${XDG_CONFIG_HOME:-$HOME/.config}"
data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
bin="$HOME/.local/bin/frame-audio-mixer"
lib_dir="$HOME/.local/lib/frame-audio-mixer"
desktop="$data_home/applications/frame-audio-mixer.desktop"
icons="$data_home/icons/hicolor"
unit_dir="$config_home/systemd/user"
app_config="$config_home/frame-audio-mixer"
wp_script="$data_home/wireplumber/scripts/frame-audio-mixer.lua"
wp_conf="$config_home/wireplumber/wireplumber.conf.d/91-frame-audio-mixer.conf"
name_dir="$config_home/pipewire/frame-audio-mixer"
old_update="$data_home/frame-audio-mixer/frame-update.sh"

if [[ -f "$here/Cargo.toml" ]]; then
    from_source=1
else
    from_source=0
fi

usage() {
    sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//'
}

autostart=0
uninstall=0
purge=0
for arg in "$@"; do
    case "$arg" in
        --autostart) autostart=1 ;;
        --uninstall) uninstall=1 ;;
        --purge) purge=1 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown option: $arg" >&2; usage >&2; exit 2 ;;
    esac
done
if [[ $purge -eq 1 && $uninstall -eq 0 ]]; then
    echo "--purge only works together with --uninstall." >&2
    exit 2
fi

# The old fork's Mix units, if a previous experiment left them behind.
remove_legacy_mix() {
    systemctl --user disable --now frame-mic-mix.service >/dev/null 2>&1 || true
    systemctl --user disable --now 'frame-mic-mix-channel@*.service' >/dev/null 2>&1 || true
    local unit
    for unit in "$unit_dir"/frame-mic-mix.service "$unit_dir"/frame-mic-mix-channel@.service; do
        rm -f "$unit"
    done
    rm -f "$HOME/.local/lib/frame-mic-tuner/frame-mic-mix-run.sh"
    rm -rf "$config_home/pipewire/frame-mic-mix"
    wpctl settings --delete frame-mic.mix >/dev/null 2>&1 || true
    # A WirePlumber that was started with the old schema keeps the key until it restarts.
    wpctl settings --save frame-mic.mix false >/dev/null 2>&1 || true
    systemctl --user reset-failed frame-mic-mix.service 'frame-mic-mix-channel@*.service' >/dev/null 2>&1 || true
}

stop_mixer() {
    systemctl --user disable --now frame-audio-mixer.service >/dev/null 2>&1 || true
    systemctl --user disable --now frame-audio-mixer-graph.service >/dev/null 2>&1 || true
    systemctl --user disable --now 'frame-audio-mixer-channel@*.service' >/dev/null 2>&1 || true
    pkill -TERM -x frame-audio-mixer >/dev/null 2>&1 || true
}

if [[ $uninstall -eq 1 ]]; then
    stop_mixer
    rm -f "$bin" "$desktop" "$old_update" \
        "$unit_dir/frame-audio-mixer.service" \
        "$unit_dir/frame-audio-mixer-graph.service" \
        "$unit_dir/frame-audio-mixer-channel@.service" \
        "$lib_dir/frame-audio-mixer-graph.sh" \
        "$wp_script" "$wp_conf"
    rm -rf "$name_dir"
    rmdir "$lib_dir" "$data_home/frame-audio-mixer" >/dev/null 2>&1 || true
    for size in 48 128 256; do
        rm -f "$icons/${size}x${size}/apps/frame-audio-mixer.png"
    done
    systemctl --user daemon-reload
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$(dirname "$desktop")" || true
    fi
    if [[ $purge -eq 1 ]]; then
        rm -rf "$app_config"
        wpctl settings --delete frame-audio-mixer.enabled >/dev/null 2>&1 || true
        wpctl settings --delete frame-audio-mixer.vrlink-gain >/dev/null 2>&1 || true
        wpctl settings --delete frame-audio-mixer.mic-source >/dev/null 2>&1 || true
        echo "Removed Frame Audio Mixer, its settings and the saved frame-audio-mixer.* values."
    else
        echo "Removed Frame Audio Mixer. Settings stay in $app_config (add --purge to delete them)."
    fi
    cat <<EOF

The WirePlumber script is removed but stays loaded until WirePlumber starts again.
Restart the headset when you are not in a VR session. Don't restart PipeWire or
WirePlumber while playing: SteamVR and Steam Link lose their sound.
EOF
    exit 0
fi

if [[ "$(uname -m)" != "aarch64" ]]; then
    echo "This is for the Steam Frame (aarch64), but this machine is $(uname -m)." >&2
    exit 1
fi
tools="wpctl systemctl"
if [[ $from_source -eq 1 ]]; then
    tools="$tools cargo rustc"
fi
for tool in $tools; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        if [[ "$tool" == "cargo" || "$tool" == "rustc" ]] && [[ -x "$HOME/.cargo/bin/$tool" ]]; then
            continue
        fi
        echo "Missing tool: $tool" >&2
        if [[ "$tool" == "cargo" || "$tool" == "rustc" ]]; then
            echo "Building from source needs Rust (https://rustup.rs). A release archive does not." >&2
        fi
        exit 1
    fi
done
export PATH="$HOME/.cargo/bin:$PATH"

remove_legacy_mix

cd "$here"
if [[ $from_source -eq 1 ]]; then
    cargo build --release
    built_bin="target/release/frame-audio-mixer"
else
    built_bin="$here/frame-audio-mixer"
fi

install -Dm755 "$built_bin" "$bin"
rm -f "$old_update"
install -Dm755 contrib/frame-audio-mixer-graph.sh "$lib_dir/frame-audio-mixer-graph.sh"
for size in 48 128 256; do
    icon_src="assets/icons/blue-$size.png"
    if [[ ! -f "$icon_src" ]]; then
        icon_src="contrib/icons/frame-audio-mixer-$size.png"
    fi
    install -Dm644 "$icon_src" "$icons/${size}x${size}/apps/frame-audio-mixer.png"
done
mkdir -p "$(dirname "$desktop")" "$name_dir"
sed "s|@BINARY@|$bin|" contrib/frame-audio-mixer.desktop > "$desktop.tmp"
chmod 644 "$desktop.tmp"
mv "$desktop.tmp" "$desktop"
install -Dm644 contrib/frame-audio-mixer.service "$unit_dir/frame-audio-mixer.service"
install -Dm644 contrib/frame-audio-mixer-graph.service "$unit_dir/frame-audio-mixer-graph.service"
install -Dm644 contrib/frame-audio-mixer-channel@.service "$unit_dir/frame-audio-mixer-channel@.service"
systemctl --user daemon-reload
if command -v desktop-file-validate >/dev/null 2>&1; then
    desktop-file-validate "$desktop" || true
fi
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$(dirname "$desktop")" || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1 && [[ -f "$icons/index.theme" ]]; then
    gtk-update-icon-cache -q "$icons" || true
fi

wp_changed=0
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

strip_spdx() {
    sed '1{/SPDX-License-Identifier/d}' "$1"
}

install_if_changed() {
    local source="$1" target="$2"
    if [[ -f "$target" ]] && cmp -s <(strip_spdx "$source") <(strip_spdx "$target"); then
        echo "WirePlumber: $target is already up to date."
        return
    fi
    install -Dm644 "$source" "$target"
    echo "WirePlumber: installed $target"
    wp_changed=1
}

cp contrib/wireplumber/frame-audio-mixer.lua "$tmp"
install_if_changed "$tmp" "$wp_script"
sed "s|@SCRIPT_PATH@|$wp_script|" contrib/wireplumber/91-frame-audio-mixer.conf > "$tmp"
install_if_changed "$tmp" "$wp_conf"

if [[ $autostart -eq 1 ]]; then
    systemctl --user enable frame-audio-mixer.service
fi
autostart_state="$(systemctl --user is-enabled frame-audio-mixer.service 2>/dev/null || true)"

mkdir -p "$app_config"
: > "$app_config/install-args"
if [[ $autostart -eq 1 || "$autostart_state" == "enabled" ]]; then
    echo --autostart >> "$app_config/install-args"
fi

cat <<EOF

Frame Audio Mixer is installed:
  $bin
  $desktop
  $unit_dir/frame-audio-mixer.service (start with SteamVR: ${autostart_state:-unknown})
Open it from the SteamVR dashboard: + > Frame Audio Mixer, then the Mix icon.
The mix graph stays off until you turn Mix on in the panel.
EOF
if pgrep -x frame-audio-mixer >/dev/null 2>&1; then
    echo "A running Frame Audio Mixer keeps the old version until you quit it or restart SteamVR."
fi
if [[ $wp_changed -eq 1 ]]; then
    cat <<EOF

The WirePlumber script was installed or changed. It loads the next time WirePlumber
starts. Restart the headset when you are not in a VR session.
Don't restart PipeWire or WirePlumber while playing: SteamVR and Steam Link lose their sound.
EOF
fi
