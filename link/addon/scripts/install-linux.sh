#!/bin/sh
# Install or update Frame Audio Link, the SteamVR add-on of Frame Audio Mixer, on a Linux PC.
# Run it as your normal user (no sudo), from a release archive or from a git checkout
# (a checkout is built first, which needs Rust and the PipeWire headers):
#   ./install-linux.sh                     install or update, and register with SteamVR
#   ./install-linux.sh --uninstall         remove the add-on and its SteamVR registration
#   ./install-linux.sh --uninstall --purge also delete its settings
#
# Nothing is written outside the home directory. The sound devices (Steam Streaming Mic B1-B4 and
# Steam Streaming Out PC1-PC4) are PipeWire nodes that exist while the add-on runs, so nothing is
# installed into the sound system. PipeWire is required; PulseAudio alone is not supported.
set -eu

here="$(cd "$(dirname "$0")" && pwd)"
config_home="${XDG_CONFIG_HOME:-$HOME/.config}"
bin_dir="$HOME/.local/bin"
bin="$bin_dir/frame-audio-link"
app_config="$config_home/frame-audio-link"

usage() {
    sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'
}

uninstall=0
purge=0
for arg in "$@"; do
    case "$arg" in
        --uninstall) uninstall=1 ;;
        --purge) purge=1 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown option: $arg" >&2; usage >&2; exit 2 ;;
    esac
done
if [ "$purge" -eq 1 ] && [ "$uninstall" -eq 0 ]; then
    echo "--purge only works together with --uninstall." >&2
    exit 2
fi
if [ "$(uname -s)" != "Linux" ]; then
    echo "This script is for Linux. On Windows, run install.ps1 from the Windows archive." >&2
    exit 1
fi

# Ends a running add-on, so the next VR session starts the new one. Only
# processes started from the installed binary are touched.
stop_running() {
    for pid in $(pgrep -x frame-audio-lin 2>/dev/null || true); do
        exe="$(readlink "/proc/$pid/exe" 2>/dev/null || true)"
        case "$exe" in
            "$bin"|"$bin (deleted)") kill -TERM "$pid" 2>/dev/null || true ;;
        esac
    done
}

if [ "$uninstall" -eq 1 ]; then
    if [ -x "$bin" ]; then
        # Removes the SteamVR registration. A failure must not keep the files
        # around. Its own closing line is replaced by the one printed below.
        "$bin" uninstall >/dev/null || echo "The SteamVR registration could not be removed; continuing." >&2
    fi
    stop_running
    rm -f "$bin"
    if [ "$purge" -eq 1 ]; then
        rm -rf "$app_config"
        echo "Frame Audio Link and its settings are removed."
    else
        echo "Frame Audio Link is removed. Its settings stay in $app_config."
    fi
    exit 0
fi

# The packages a build needs, by distribution family. Nothing is installed here.
build_packages() {
    echo "Install the build packages and run this script again:" >&2
    echo "  Ubuntu/Debian: sudo apt install build-essential pkg-config clang libclang-dev libpipewire-0.3-dev" >&2
    echo "  Fedora:        sudo dnf install gcc pkgconf-pkg-config clang pipewire-devel" >&2
    echo "  Arch:          sudo pacman -S --needed base-devel pkgconf clang libpipewire" >&2
}

# The library the add-on loads at run time, by distribution family.
runtime_packages() {
    echo "The add-on needs the PipeWire client library (libpipewire-0.3.so.0):" >&2
    echo "  Ubuntu/Debian: sudo apt install libpipewire-0.3-0" >&2
    echo "  Fedora:        sudo dnf install pipewire-libs" >&2
    echo "  Arch:          sudo pacman -S --needed libpipewire" >&2
}

if [ -x "$here/frame-audio-link" ]; then
    # A release archive: the binary sits next to this script.
    built="$here/frame-audio-link"
    machine="$(uname -m)"
    if [ "$machine" != "x86_64" ]; then
        echo "The release is built for x86_64 and this PC is $machine. Build from a git checkout instead." >&2
        exit 1
    fi
elif [ -f "$here/../Cargo.toml" ]; then
    # A git checkout: build it.
    crate="$(cd "$here/.." && pwd)"
    if [ -x "$HOME/.cargo/bin/cargo" ]; then
        PATH="$HOME/.cargo/bin:$PATH"
        export PATH
    fi
    if ! command -v cargo >/dev/null 2>&1; then
        echo "Building needs Rust. Install it from https://rustup.rs and run this script again." >&2
        exit 1
    fi
    if ! command -v pkg-config >/dev/null 2>&1 || ! pkg-config --exists libpipewire-0.3 || ! command -v clang >/dev/null 2>&1; then
        echo "Building needs a C toolchain, clang and the PipeWire development files." >&2
        build_packages
        exit 1
    fi
    echo "Building Frame Audio Link (this takes a few minutes the first time)..."
    if ! (cd "$crate" && cargo build --release); then
        echo "The build failed." >&2
        build_packages
        exit 1
    fi
    built="$crate/target/release/frame-audio-link"
else
    echo "Neither a release binary nor a source checkout was found next to $0." >&2
    exit 1
fi

# The binary must start before anything is replaced. A missing PipeWire
# library shows here as a loader error.
if ! version="$("$built" version 2>&1)"; then
    echo "The add-on does not start on this system:" >&2
    echo "  $version" >&2
    runtime_packages
    exit 1
fi

mkdir -p "$bin_dir"
stop_running
# `install` replaces the file in one step, so a running copy keeps its old one.
install -m 755 "$built" "$bin"
echo "Installed $version to $bin"

# PipeWire has to be the sound server of the desktop session. This is only a
# warning: the script may run over SSH, where the session cannot be seen.
runtime="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
if [ ! -S "$runtime/pipewire-0" ]; then
    echo ""
    echo "WARNING: PipeWire does not seem to be running in this session ($runtime/pipewire-0 is missing)." >&2
    echo "Frame Audio Link creates its sound devices in PipeWire. On a system that only runs" >&2
    echo "PulseAudio they never appear; switch the session to PipeWire (with pipewire-pulse) first." >&2
elif command -v pactl >/dev/null 2>&1 && ! pactl info 2>/dev/null | grep -qi "pipewire"; then
    echo ""
    echo "WARNING: PipeWire is running, but apps that use the PulseAudio API are not served by it" >&2
    echo "(pipewire-pulse is not the PulseAudio server). Such apps will not see the Frame devices." >&2
fi

echo ""
if "$bin" install; then
    echo ""
    echo "Next steps:"
    echo "  1. Start SteamVR. Frame Audio Link starts with it; switch it off or on under"
    echo "     SteamVR Settings > Startup / Shutdown > Choose Startup Overlay Apps."
    echo "  2. While it runs, pick Steam Streaming Mic B1-B4 as a microphone and Steam Streaming Out PC1-PC4 as an output"
    echo "     in any app. Its dashboard overlay switches each stream on or off."
    echo "  3. On the headset, Frame Audio Mixer shows PC1-PC4 in the + dialog."
else
    echo "" >&2
    echo "The add-on is installed, but it could not be registered with SteamVR." >&2
    echo "Start SteamVR once, close it, and run:  $bin install" >&2
    exit 1
fi
