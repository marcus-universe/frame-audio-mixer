**English** · [日本語](technical.ja.md) · [简体中文](technical.sc.md) · [Deutsch](technical.de.md) · [Français](technical.fr.md) · [Español](technical.es.md) · [Italiano](technical.it.md)

[← README](../README.md)

# Technical details

## What gets installed

The one-line install runs [`scripts/get.sh`](../scripts/get.sh). It downloads the latest release (`frame-audio-mixer-aarch64.tar.gz`), checks it against `SHA256SUMS`, and runs its `install.sh`. While no release is published, it downloads the `main` branch and builds it instead, which needs Rust (`cargo`) on the headset. It needs `curl`, `sha256sum` and `tar`. Any options after `sh -s --` are passed to `install.sh`.

`install.sh` writes only under the home directory and does not use `sudo`:

- binary `~/.local/bin/frame-audio-mixer`
- user units `frame-audio-mixer.service` and `frame-audio-mixer-engine.service`
- WirePlumber script `~/.local/share/wireplumber/scripts/frame-audio-mixer.lua` and `~/.config/wireplumber/wireplumber.conf.d/91-frame-audio-mixer.conf` (component `custom.frame-audio-mixer`)
- desktop entry and icons under `~/.local/share`
- settings in `~/.config/frame-audio-mixer/`

The engine unit replaces the older graph, channel and effect-bus units. `install.sh` stops those units and deletes them. It never restarts PipeWire or WirePlumber. A new WirePlumber script loads the next time WirePlumber starts. Restart the headset only when you are not in a VR session.

At runtime the engine adds PipeWire nodes named `frame_amix_*`. The panel talks to it on a Unix socket at `$XDG_RUNTIME_DIR/frame-audio-mixer/engine.sock`. The WirePlumber settings are `frame-audio-mixer.enabled` and `frame-audio-mixer.vrlink-gain`. The overlay key is `marcus-universe.frame-audio-mixer`.

If an older experiment left `frame-mic-mix` units behind, `install.sh` removes them. The only `frame-mic.*` value it touches is a leftover `frame-mic.mix`. It does not overwrite Frame Mic Tuner's `frame-mic-tracker.lua` or `90-frame-mic.conf`.

From a git checkout, `./install.sh` builds with `cargo` and installs. `./install.sh --autostart` also starts the panel with SteamVR. The options you used are saved in `~/.config/frame-audio-mixer/install-args`, so an update installs the same way.

## Channels

- Up to 32 channels, `ch1` to `ch32`. Adding or removing one creates or destroys only that channel's nodes inside `frame-audio-mixer-engine.service`. There is no per-channel process.
- A name is one line, at most 32 characters, and unique ignoring case. Apps see the channel as an output with that name (`DJ`).
- A channel made from a running app is named after the app, pins the app to it, and shows the app's icon from its `.desktop` entry. Without an icon it shows the first letter.
- When the row is wider than the panel, scroll it with the controller stick or drag the bar under the channels.
- Each strip has one fader, from silence to +12 dB, with a detent at 0 dB. Mute and Solo sit under it. The route buttons are stacked next to the fader: A1–A4 (what you hear), PC (the PC streaming microphone), B1–B4 (virtual microphones), then FX1 and FX2. With 6 or 8 outputs the A and B buttons split into two columns. In the list view they sit in one row next to Mute and Solo.

A stream that lands on a channel it is not pinned to is moved back: onto its own channel, or off the mixer so it plays on the normal output. Pins match the app's binary when one is stored, so two apps that share a display name (Electron apps all report “Chromium”) cannot take each other's channel.

## Projects

The folder button in the header holds saved routing setups. A project stores the channels, pins, routes, effect chains and levels as one JSON file under `~/.config/frame-audio-mixer/projects/`. Loading a project replaces the current setup; screen preferences such as language, theme and layout stay as they are.

## Outputs

The PC streaming microphone is always there. App Settings adds 2, 4, 6 or 8 more outputs: 2 is A1 and B1, 8 is A1–A4 and B1–B4. A lower count keeps the stored routes and does not link them.

- PC is `frame_amix_source`, shown as **Frame Mix**. VRLink records it, so the PC sees Steam Streaming Microphone.
- A1 is the headset speakers. A2–A4 can be any other sink, such as Bluetooth.
- B1–B4 are virtual microphones for apps on the headset (`frame_amix_b1` to `frame_amix_b4`). Each has a name.

The Outputs drawer (speaker icon on the right rail) holds Mix Check at the top, then one container per output: its name, the master fader under it, and Mute at the right. PC comes first.

Settings from version 2 move over on first load: the old B1 becomes PC (with its fader and Mute), and the old B2–B4 become B1–B3.

## Signal flow

The Signal flow page (next to Apps & updates in the header) draws the whole mix as a lane graph: sources, mixer strips, outputs, and the devices they reach. Edges come from the saved routing, and each one is checked against the engine's live PipeWire links — a route whose link is gone draws as a dashed red line. Audible signal widens a live edge with a level-tinted glow. Drag pans, the stick scrolls, and selecting a node shows the PipeWire node behind it. The Other apps toggle also shows every app that is not on any channel, with the real device it plays on.

## Microphone

The microphone choice is only for the mix. Nothing in this app calls `wpctl set-default`, so the system default that Frame Mic Tuner uses stays as it is.

- **System default** follows `default.audio.source`.
- **Frame built-in mic** is `alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source`, the end of Valve's chain after noise suppression, echo cancellation and the mic EQ.
- Any other listed source is stored by its `node.name`.

The engine captures that source with a loopback that is not virtual, so Valve's noise suppression stays on while Mix is on. The mic strip adds 12 dB of trim at the 0 dB fader. Its default route is PC only.

## Level meters

Meters are peaks kept in the engine and read over the socket. The panel asks for them at about 30 fps only while the dashboard is visible. App Settings can turn them off. There is no `pw-record` process per meter.

Mute and Solo sit under every fader. Solo is one group: a soloed strip is the only one that passes, and the saved mute flags stay as they are.

## Effects

FX1 and FX2 are inserts, and each holds a chain of up to three effects. EQ is one of them: a parametric equalizer with 3, 6, 8 or 12 points, opened with Edit on its slot. The outer points can be a bell, a shelf, or a high-pass (low point) or low-pass (high point). The pitcher sets pitch and formants apart, each ±12 semitones, plus a brightness tilt. With the formants kept, a shifted voice sounds like another speaker rather than a sped-up one; with the formants following the pitch, it is the classic chipmunk. Its presets include male to female, male to girl, female to male and girl to male. A strip that turns a bus on runs its own copy of that chain, FX1 before FX2, ahead of its fader. The result replaces the strip's signal on the routes of that strip; no effect node exists in the graph and no dry copy is mixed in. Each bus has one level fader, applied after its chain. A VST plugin has one host and one stream, so it processes the first strip that uses its bus (the microphone, then the channels in order) and is bypassed on the others. Processing is float32 at 48 kHz and 480 frames inside the engine. Toggling an effect does not restart PipeWire or WirePlumber.

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
curl -fsSL https://frame-audio-mixer.marcus-universe.com/i | sh -s -- --uninstall
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

`--dump-png` writes the main view and the other views (Apps, Add, settings, language menu, a scrolled row while recording, an effect bus) next to the given path.

`scripts/package.sh` writes `dist/frame-audio-mixer-<version>.tar.gz`, the same bytes as `dist/frame-audio-mixer-aarch64.tar.gz`, and `dist/SHA256SUMS` listing both. Upload all three to a GitHub release, and the one-line install and the in-app updater use it.
