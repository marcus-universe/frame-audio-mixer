**English** · [日本語](technical.ja.md) · [简体中文](technical.sc.md) · [Deutsch](technical.de.md) · [Français](technical.fr.md) · [Español](technical.es.md) · [Italiano](technical.it.md)

[← README](../README.md)

# Technical details

## What gets installed

The one-line install runs [`scripts/get.sh`](../scripts/get.sh). It downloads the latest release (`frame-audio-mixer-aarch64.tar.gz`), checks it against `SHA256SUMS`, and runs its `install.sh`. While no release is published, it downloads the `main` branch and builds it instead, which needs Rust (`cargo`) on the headset. It needs `curl`, `sha256sum` and `tar`. Any options after `sh -s --` are passed to `install.sh`.

`install.sh` writes only under the home directory and does not use `sudo`:

- binary `~/.local/bin/frame-audio-mixer`
- runner `~/.local/lib/frame-audio-mixer/frame-audio-mixer-graph.sh`
- user units `frame-audio-mixer.service`, `frame-audio-mixer-graph.service` and `frame-audio-mixer-channel@.service`
- WirePlumber script `~/.local/share/wireplumber/scripts/frame-audio-mixer.lua` and `~/.config/wireplumber/wireplumber.conf.d/91-frame-audio-mixer.conf` (component `custom.frame-audio-mixer`)
- desktop entry and icons under `~/.local/share`
- settings in `~/.config/frame-audio-mixer/` and channel names in `~/.config/pipewire/frame-audio-mixer/<id>.name`

At runtime the app adds PipeWire nodes named `frame_amix_*`, the WirePlumber settings `frame-audio-mixer.enabled`, `frame-audio-mixer.vrlink-gain` and `frame-audio-mixer.mic-source`, and the dashboard overlay `marcus-universe.frame-audio-mixer`.

`install.sh` never restarts PipeWire or WirePlumber. A new or changed WirePlumber script loads the next time WirePlumber starts. Restart the headset only when you are not in a VR session: restarting PipeWire or WirePlumber while playing drops SteamVR and Steam Link audio. Channels still work before that, because the panel also pins their streams while it runs.

If an older experiment left `frame-mic-mix` units behind, `install.sh` removes them. The only `frame-mic.*` value it touches is a leftover `frame-mic.mix`. It does not overwrite Frame Mic Tuner's `frame-mic-tracker.lua` or `90-frame-mic.conf`.

From a git checkout, `./install.sh` builds with `cargo` and installs. `./install.sh --autostart` also starts the panel with SteamVR. The options you used are saved in `~/.config/frame-audio-mixer/install-args`, so an update installs the same way.

## Channels

- Up to 32 channels, `ch1` to `ch32`. Each is one systemd instance with two `pw-loopback` processes.
- A name is one line, at most 32 characters, and unique ignoring case.
- A channel made from a running app is named after the app, pins the app to it, and shows the app's icon from its `.desktop` entry. Without an icon it shows the first letter.
- When the row is wider than the panel, scroll it with the controller stick or drag the bar under the channels.
- Faders go from 0 to 150 %. A drag snaps to 20, 50, 80 and 100 % when it comes within 8 pixels. The dots on a fader mark 20, 50 and 80 %. The line marks 100 %.

## Microphone

The microphone choice is only for the mix. Nothing in this app calls `wpctl set-default`, so the system default that Frame Mic Tuner uses stays as it is.

- **System default** follows `default.audio.source`.
- **Frame built-in mic** is `alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source`, the end of Valve's chain after noise suppression, echo cancellation and the mic EQ.
- Any other listed source is stored by its `node.name`.

The choice is saved as `frame-audio-mixer.mic-source` (`""`, `builtin`, or a node name). The script resolves `builtin` to the loopback above, falls back to the system default if that node is missing, and never uses `frame_amix_source`. A changed value takes effect after WirePlumber's next start.

## Level meters

Meters run only while the dashboard is visible, Mix is on, and meters are on in App Settings (the gear next to the language menu). Only the channels you can see on screen are metered. Each meter is a `pw-record` tap with `node.virtual=true`, so it does not turn Valve's mic filters on. Channel taps are passive. The microphone tap is not. The panel redraws at about 30 fps while meters run and does not draw while the dashboard is hidden.

## Apps and updates

**Apps & updates** shows this app, Frame Mic Tuner as the related app, and the Frame apps from sasaken's catalog. **Install** opens Konsole with `curl -fsSL https://frame.sasaken1102s.net | sh`. A Frame Mic Tuner that is already running has no Install button, so this page does not replace a fork of it.

The updater is written in Rust (`ureq` with rustls, `sha2`, `flate2` and `tar`):

- A check reads the latest GitHub release, compares versions, and looks for `frame-audio-mixer-<version>.tar.gz` and `SHA256SUMS`.
- Install copies the running binary to `~/.cache/frame-audio-mixer/update/` and starts it with `systemd-run --user --unit=frame-audio-mixer-update --collect`.
- Downloads are HTTPS only, from `github.com`, `api.github.com`, `codeload.github.com` and `*.githubusercontent.com`, unless `FRAME_UPDATE_ALLOW_INSECURE=1` is set. Redirects are checked, a download stops at 256 MB, and the SHA-256 must match.
- The archive is rejected if it has an absolute path, `..`, a link or a device node.
- Cache files are in `~/.cache/frame-audio-mixer/` (`update-check.json`, `update-state.json`, `update.log`, `update.lock/`, `update/`).

## Uninstall

With the one-line command:

```sh
curl -fsSL https://raw.githubusercontent.com/marcus-universe/frame-audio-mixer/main/scripts/get.sh | sh -s -- --uninstall
```

From a checkout or release folder:

```bash
./install.sh --uninstall
./install.sh --uninstall --purge
```

Without `--purge`, settings stay in `~/.config/frame-audio-mixer`. `--purge` also deletes them and the saved `frame-audio-mixer.*` values. The WirePlumber script is removed from disk but stays loaded until WirePlumber starts again.

## Build

On the headset, `rustc` and `cargo` are in `~/.cargo/bin`. The system libraries used are cairo, freetype, Vulkan, PipeWire and libclang.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release
./target/release/frame-audio-mixer --self-test
./target/release/frame-audio-mixer --language en --dump-png /tmp/frame-audio-mixer.png
./scripts/package.sh
```

`--dump-png` writes the main view and the other views (Apps, Add, channel picker, settings, language menu, a scrolled row while recording) next to the given path.

`scripts/package.sh` writes `dist/frame-audio-mixer-<version>.tar.gz`, the same bytes as `dist/frame-audio-mixer-aarch64.tar.gz`, and `dist/SHA256SUMS` listing both. Upload all three to a GitHub release, and the one-line install and the in-app updater use it.
