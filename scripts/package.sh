#!/usr/bin/env bash
# Build a release archive on the headset (aarch64).
#   ./scripts/package.sh
# Writes dist/frame-audio-mixer-<version>.tar.gz and dist/SHA256SUMS.
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
cd "$here"
export PATH="$HOME/.cargo/bin:$PATH"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
cargo build --release
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/contrib/wireplumber" "$stage/contrib/icons" "$stage/scripts"
cp target/release/frame-audio-mixer "$stage/frame-audio-mixer"
cp install.sh LICENSE NOTICE "$stage/"
cp contrib/frame-audio-mixer-graph.sh contrib/*.service contrib/*.desktop "$stage/contrib/"
cp contrib/wireplumber/* "$stage/contrib/wireplumber/"
cp contrib/icons/*.png "$stage/contrib/icons/"
cp scripts/get.sh "$stage/scripts/"
cp README*.md "$stage/" 2>/dev/null || true
chmod 755 "$stage/install.sh" "$stage/frame-audio-mixer" "$stage/contrib/frame-audio-mixer-graph.sh" "$stage/scripts/get.sh"
mkdir -p dist
archive="dist/frame-audio-mixer-${version}.tar.gz"
stable="dist/frame-audio-mixer-aarch64.tar.gz"
tar -czf "$archive" -C "$stage" .
cp "$archive" "$stable"
(
    cd dist
    sha256sum "frame-audio-mixer-${version}.tar.gz" "frame-audio-mixer-aarch64.tar.gz" > SHA256SUMS
)
echo "Wrote $archive and $stable"
