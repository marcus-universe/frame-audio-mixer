#!/bin/sh
# Bootstrap Frame Audio Mixer on a Steam Frame. Needs curl, sha256sum and tar.
# Updates after this use the in-app updater.
#   curl -fsSL https://raw.githubusercontent.com/marcus-universe/frame-audio-mixer/main/scripts/get.sh | sh
set -eu

if [ "$(uname -m)" != "aarch64" ]; then
    echo "This is for the Steam Frame (aarch64)." >&2
    exit 1
fi

repo="https://github.com/marcus-universe/frame-audio-mixer/releases/latest/download"
archive="frame-audio-mixer-aarch64.tar.gz"
work="${TMPDIR:-/tmp}/frame-audio-mixer-get"
rm -rf "$work"
mkdir -p "$work"
cd "$work"

if ! curl -fsSL -o "$archive" "$repo/$archive"; then
    echo "No release yet. Frame Audio Mixer has not published v0.1.0." >&2
    exit 1
fi
if ! curl -fsSL -o SHA256SUMS "$repo/SHA256SUMS"; then
    echo "No release yet. The checksum file is missing." >&2
    exit 1
fi

if ! grep -F "  $archive" SHA256SUMS | sha256sum -c -; then
    echo "Checksum failed." >&2
    exit 1
fi

tar -xzf "$archive"
chmod 755 install.sh frame-audio-mixer
exec ./install.sh "$@"
