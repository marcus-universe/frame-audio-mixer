#!/bin/sh
# Build the Linux release archive of Frame Audio Link. Run it on Linux, in a checkout:
#   ./scripts/package-linux.sh
# Writes dist/frame-audio-link-<version>-linux-<arch>.tar.gz, the same bytes as
# dist/frame-audio-link-linux-<arch>.tar.gz (the name get-linux.sh downloads), and their
# lines in dist/SHA256SUMS. Lines of other archives in that file are kept.
#
# The binary runs on distributions whose glibc is at least as new as the one it was built
# against, so build on the oldest release you want to support.
set -eu

here="$(cd "$(dirname "$0")/.." && pwd)"
cd "$here"
if [ -x "$HOME/.cargo/bin/cargo" ]; then
    PATH="$HOME/.cargo/bin:$PATH"
    export PATH
fi
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1 | tr -d '\r')"
arch="$(uname -m)"
cargo build --release

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
cp target/release/frame-audio-link "$stage/frame-audio-link"
# Debug symbols triple the download and are of no use to a user.
if command -v strip >/dev/null 2>&1; then
    strip --strip-unneeded "$stage/frame-audio-link"
fi
# Stripped of carriage returns, in case the checkout came through Windows.
tr -d '\r' < scripts/install-linux.sh > "$stage/install-linux.sh"
chmod 755 "$stage/frame-audio-link" "$stage/install-linux.sh"
for notice in ../../LICENSE ../../NOTICE; do
    if [ -f "$notice" ]; then
        cp "$notice" "$stage/"
        chmod 644 "$stage/$(basename "$notice")"
    fi
done
cat > "$stage/README.txt" <<EOF
Frame Audio Link $version for Linux ($arch)

The SteamVR add-on of Frame Audio Mixer. While SteamVR runs it gives this PC four
more streaming microphones (Steam Streaming Mic B1-B4, the B outputs of the mixer on the Steam
Frame) and four outputs (Steam Streaming Out PC1-PC4) that the mixer can take in as channels.

Install or update, as your normal user:
    ./install-linux.sh
Remove:
    ./install-linux.sh --uninstall

Needs PipeWire as the session's sound server (with pipewire-pulse for apps that
use the PulseAudio API) and SteamVR. Nothing is written outside your home folder.

https://github.com/marcus-universe/frame-audio-mixer
EOF

mkdir -p dist
# The archive records the folder's own mode. mktemp's private one would be
# applied to wherever the archive is unpacked.
chmod 755 "$stage"
versioned="frame-audio-link-${version}-linux-${arch}.tar.gz"
stable="frame-audio-link-linux-${arch}.tar.gz"
tar -czf "dist/$versioned" -C "$stage" .
cp "dist/$versioned" "dist/$stable"
(
    cd dist
    # Replace this archive's old lines and keep everyone else's.
    if [ -f SHA256SUMS ]; then
        grep -v -F -e "  $versioned" -e "  $stable" SHA256SUMS > SHA256SUMS.new || true
    else
        : > SHA256SUMS.new
    fi
    sha256sum "$versioned" "$stable" >> SHA256SUMS.new
    mv SHA256SUMS.new SHA256SUMS
)
echo "Wrote dist/$versioned and dist/$stable"
