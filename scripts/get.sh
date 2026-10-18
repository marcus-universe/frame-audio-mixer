#!/bin/sh
# Install or update Frame Audio Mixer on a Steam Frame with one command:
#   curl -fsSL https://frame-audio-mixer.marcus-universe.com/i -o frame-audio-mixer-install && sh frame-audio-mixer-install
# Options go to install.sh, for example:
#   curl -fsSL https://frame-audio-mixer.marcus-universe.com/i -o frame-audio-mixer-install && sh frame-audio-mixer-install --autostart
#   curl -fsSL https://frame-audio-mixer.marcus-universe.com/i -o frame-audio-mixer-install && sh frame-audio-mixer-install --uninstall
# It uses the latest release and checks SHA256SUMS. Without a release it builds
# the main branch, which needs Rust (cargo) on the headset. Needs curl,
# sha256sum and tar. Nothing is written outside the home directory.
set -eu

if [ "$(uname -m)" != "aarch64" ]; then
    echo "This is for the Steam Frame (aarch64)." >&2
    exit 1
fi

repo="marcus-universe/frame-audio-mixer"
releases="https://github.com/$repo/releases/latest/download"
archive="frame-audio-mixer-aarch64.tar.gz"
work="${TMPDIR:-/tmp}/frame-audio-mixer-get"
rm -rf "$work"
mkdir -p "$work"
cd "$work"

if curl -fsSL -o "$archive" "$releases/$archive" && curl -fsSL -o SHA256SUMS "$releases/SHA256SUMS"; then
    if ! grep -F "  $archive" SHA256SUMS | sha256sum -c -; then
        echo "Checksum failed." >&2
        exit 1
    fi
    tar -xzf "$archive"
    chmod 755 install.sh frame-audio-mixer
    exec ./install.sh "$@"
fi

uninstall=0
for arg in "$@"; do
    case "$arg" in
        --uninstall) uninstall=1 ;;
    esac
done
if [ "$uninstall" -eq 0 ]; then
    echo "No release is published yet. Building the main branch instead."
fi
if [ "$uninstall" -eq 0 ] && ! command -v cargo >/dev/null 2>&1 && [ ! -x "$HOME/.cargo/bin/cargo" ]; then
    echo "Building needs Rust. Install it from https://rustup.rs and run this command again." >&2
    exit 1
fi
curl -fsSL -o source.tar.gz "https://codeload.github.com/$repo/tar.gz/refs/heads/main"
tar -xzf source.tar.gz
cd frame-audio-mixer-main
chmod 755 install.sh
exec ./install.sh "$@"
