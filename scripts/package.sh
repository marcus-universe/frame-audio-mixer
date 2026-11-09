#!/usr/bin/env bash
# Build a release archive on the headset (aarch64).
#   ./scripts/package.sh
# Writes dist/frame-audio-mixer-<version>.tar.gz and dist/SHA256SUMS.
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
cd "$here"
export PATH="$HOME/.cargo/bin:$PATH"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1 | tr -d '\r')"
cargo build --release
cargo build --release --manifest-path vst-host/Cargo.toml
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/contrib/wireplumber" "$stage/assets/icons" "$stage/scripts"
cp target/release/frame-audio-mixer "$stage/frame-audio-mixer"
cp vst-host/target/release/frame-vst-host "$stage/frame-vst-host"
if [[ -f vst-host/prebuilt/frame-vst-host.exe ]]; then
    cp vst-host/prebuilt/frame-vst-host.exe "$stage/frame-vst-host.exe"
else
    echo "warning: vst-host/prebuilt/frame-vst-host.exe is missing; run scripts/build-vst-host-windows.ps1 on Windows" >&2
fi
cp install.sh LICENSE NOTICE "$stage/"
cp contrib/*.service contrib/*.desktop "$stage/contrib/"
cp contrib/wireplumber/* "$stage/contrib/wireplumber/"
cp assets/icons/blue-*.png assets/icons/pink-*.png "$stage/assets/icons/"
cp scripts/get.sh "$stage/scripts/"
cp README*.md "$stage/" 2>/dev/null || true
if [[ -d docs ]]; then
    cp -r docs "$stage/"
fi
chmod 755 "$stage/install.sh" "$stage/frame-audio-mixer" "$stage/scripts/get.sh"
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
