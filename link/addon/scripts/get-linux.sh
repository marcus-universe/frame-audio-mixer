#!/bin/sh
# Install or update Frame Audio Link (the SteamVR add-on of Frame Audio Mixer) on a
# Linux PC with one command:
#   curl -fsSL https://frame-audio-mixer.marcus-universe.com/get-linux | sh
# Options go to install-linux.sh, for example:
#   curl -fsSL https://frame-audio-mixer.marcus-universe.com/get-linux | sh -s -- --uninstall
# The website serves a copy of this file at /get-linux (site/get-linux). Keep both the same.
# The release also carries it as get-linux.sh.
# It uses the latest release and checks its SHA-256. Without a release for this PC it
# builds the main branch, which needs Rust (cargo), clang and the PipeWire headers.
# Needs curl, sha256sum and tar. Nothing is written outside the home directory.
# The body is a function so a piped download is read in full before it runs.
set -eu

main() {
    if [ "$(uname -s)" != "Linux" ]; then
        echo "This is for Linux. On Windows, use the Windows archive and its install.ps1." >&2
        exit 1
    fi

    repo="marcus-universe/frame-audio-mixer"
    # FRAME_AUDIO_LINK_RELEASES points somewhere else, to try an archive
    # before it is published.
    releases="${FRAME_AUDIO_LINK_RELEASES:-https://github.com/$repo/releases/latest/download}"
    archive="frame-audio-link-linux-$(uname -m).tar.gz"
    work="${TMPDIR:-/tmp}/frame-audio-link-get"
    rm -rf "$work"
    mkdir -p "$work"
    cd "$work"

    if curl -fsSL -o "$archive" "$releases/$archive" && curl -fsSL -o SHA256SUMS "$releases/SHA256SUMS"; then
        if ! grep -F "  $archive" SHA256SUMS | sha256sum -c -; then
            echo "Checksum failed." >&2
            exit 1
        fi
        tar -xzf "$archive"
        chmod 755 install-linux.sh frame-audio-link
        # install-linux.sh must not read this pipe, or it consumes the rest of the script.
        exec ./install-linux.sh "$@" </dev/null
    fi

    uninstall=0
    purge=0
    for arg in "$@"; do
        case "$arg" in
            --uninstall) uninstall=1 ;;
            --purge) purge=1 ;;
        esac
    done
    if [ "$uninstall" -eq 1 ]; then
        # Nothing has to be downloaded to remove the add-on.
        settings="${XDG_CONFIG_HOME:-$HOME/.config}/frame-audio-link"
        if [ -x "$HOME/.local/bin/frame-audio-link" ]; then
            "$HOME/.local/bin/frame-audio-link" uninstall || true
        fi
        rm -f "$HOME/.local/bin/frame-audio-link"
        if [ "$purge" -eq 1 ]; then
            rm -rf "$settings"
            echo "Frame Audio Link and its settings are removed."
        else
            echo "Frame Audio Link is removed. Its settings stay in $settings."
        fi
        exit 0
    fi
    echo "No release for this PC is published yet. Building the main branch instead."
    if ! command -v cargo >/dev/null 2>&1 && [ ! -x "$HOME/.cargo/bin/cargo" ]; then
        echo "Building needs Rust. Install it from https://rustup.rs and run this command again." >&2
        exit 1
    fi
    curl -fsSL -o source.tar.gz "https://codeload.github.com/$repo/tar.gz/refs/heads/main"
    tar -xzf source.tar.gz
    if [ ! -f frame-audio-mixer-main/link/addon/scripts/install-linux.sh ]; then
        echo "The main branch does not contain the Linux add-on yet." >&2
        exit 1
    fi
    cd frame-audio-mixer-main/link/addon/scripts
    chmod 755 install-linux.sh
    exec ./install-linux.sh "$@" </dev/null
}

main "$@"
