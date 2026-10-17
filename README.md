# Frame Audio Mixer

Frame Audio Mixer mixes named app outputs with the Steam Frame microphone and sends that mix to the PC on the wireless stream. The PC shows the stream as **Steam Streaming Microphone**.

It runs after [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner). Frame Mic Tuner still owns echo cancellation and noise suppression. This app does not replace that microphone tracker.

## What it does

- Adds up to eight named outputs, such as Music or a game.
- Sends each output to the headset and, with its own level, into the microphone mix.
- While Mix is on, points only the VRLink capture at that mix. Other apps keep the normal default microphone.
- Leaves the graph off until you turn Mix on. The PC then hears the normal microphone again.

## Install

Run this on the headset as the normal user. It does not use `sudo`, and it only writes under the home directory.

From a git checkout or a release archive:

```bash
./install.sh
./install.sh --autostart
```

`--autostart` also starts the panel with SteamVR. The mix graph stays off until you turn Mix on.

`install.sh` does not restart PipeWire or WirePlumber. A new WirePlumber script is loaded the next time WirePlumber starts. Do that only when you are not in a VR session. Restarting PipeWire or WirePlumber while playing drops SteamVR and Steam Link audio.

The install adds a separate app:

- binary `~/.local/bin/frame-audio-mixer`
- user units `frame-audio-mixer.service`, `frame-audio-mixer-graph.service`, and `frame-audio-mixer-channel@.service`
- PipeWire nodes named `frame_amix_*`
- WirePlumber component `custom.frame-audio-mixer`
- settings `frame-audio-mixer.enabled` and `frame-audio-mixer.vrlink-gain`
- dashboard overlay key `marcus-universe.frame-audio-mixer`

If an older experiment left `frame-mic-mix` units behind, `install.sh` removes them. The only `frame-mic.*` value it changes is deleting a leftover `frame-mic.mix`. It does not overwrite `frame-mic-tracker.lua` or `90-frame-mic.conf`.

## Usage

Open the SteamVR dashboard, then **Frame Audio Mixer**. Launching the app again while it is running opens the same panel.

1. Turn Mix on.
2. Add a channel (up to 8). The name is one line, at most 32 characters, and unique ignoring case.
3. In the app, choose the output `Frame Mix: <name>`.
4. **Headphones** is how loud that channel is in the headset. **To mic** is how much of it the PC hears. The microphone fader and its mute affect only this mix, not the system default microphone.

**Mix Check** records **Frame Mic + Mix** for up to 10 seconds, then plays it on the current output. The footer chooses the language (日本語, English, 简体中文, Deutsch, Français, Español, Italiano), starts the panel with SteamVR, and quits. **Apps & updates** checks this app's releases and links to Frame Mic Tuner.

## How it works

The microphone is whatever is the current default source. On the Frame that is the built-in mic after Valve's filters, unless you have selected another microphone.

While Mix is on, `frame-audio-mixer-graph.service` builds the core:

- `frame_amix_from_mic` captures the default source.
- `frame_amix_mic_virtual_sink` is the microphone fader into `frame_amix_bus`.
- `frame_amix_source` (**Frame Mic + Mix**) is the source VRLink records.

Each channel is its own `frame-audio-mixer-channel@chN.service`:

- `frame_amix_chN` is the sink apps select (`Frame Mix: <name>`).
- `frame_amix_chN_virtual_sink_playback` plays that sink to the headset. This is the headphones fader.
- `frame_amix_chN_virtual_sink_to_bus` sends the same sink into the mix bus. This is the to-mic fader.

Those internal playback names contain both `virtual` and `sink`, so the spatializer does not wrap them and send the mix to the speakers a second time. The channel sink itself is not `alsa_output` or `bluez_output`, so it stays out of Frame Mic Tuner's output list.

WirePlumber component `custom.frame-audio-mixer` pins the VRLink capture to `frame_amix_source` by `object.serial` while `frame-audio-mixer.enabled` is true. A node name in the metadata does not move the stream. If the source is missing, VRLink keeps the normal default source. The script does not change `frame-mic.echo-cancel` or `frame-mic.noise-suppression`.

The panel pins an app to a channel the same way, with the sink's `object.serial`. The serial changes when the channel restarts, so the pin is written again. Nothing in this app calls `wpctl set-default`.

`frame-audio-mixer.vrlink-gain` defaults to `1.0`. At that value the app does not change VRLink's volume. Any other gain is saved and restored when Mix turns off. Turning Mix off stops the graph units, so the `frame_amix_*` nodes disappear.

## Microphone

The mixer's microphone choice is only for the mix. Nothing in this app calls `wpctl set-default`, so the system default that Frame Mic Tuner uses stays as it is.

- **System default** follows `default.audio.source`.
- **Frame built-in mic** is `alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source`. That is the end of Valve's chain, after noise suppression, echo cancellation and the mic EQ that Frame Mic Tuner controls.
- Any other listed source is stored as that node's `node.name`.

The value is the WirePlumber string `frame-audio-mixer.mic-source` (`""`, `builtin`, or a node name). The script resolves `builtin` to the loopback above and falls back to the system default if that node is missing. It never targets `frame_amix_source`. WirePlumber reads a new setting the next time it starts, and only while the headset is idle. Until then the choice is saved and the mix still follows the system default.

## Level meters

Meters run only while the dashboard is visible, Mix is on, and level meters are on. The gear next to the language menu opens App Settings, where meters can be turned off. Each meter is a `pw-record` tap with `node.virtual=true`, so it does not turn Valve's mic filters on. Channel taps are passive and listen to the channel sink. The microphone tap is not passive. The panel redraws at 15 fps while those taps are running, and not at all while the dashboard is hidden.

## Apps and updates

Apps & updates shows this app, Frame Mic Tuner as the related app (echo cancel and noise suppression before the mix), and the Frame apps from sasaken's catalog. Install opens Konsole with `curl -fsSL https://frame.sasaken1102s.net | sh`. A Frame Mic Tuner that is already running has no Install button, so this page does not replace the fork on this headset.

The updater is Rust: `ureq` with rustls, `sha2`, `flate2` and `tar`. `frame-update.sh` is not installed. A check reads the latest GitHub release, compares versions, and looks for `frame-audio-mixer-<version>.tar.gz` and `SHA256SUMS`. Install copies the running binary into `~/.cache/frame-audio-mixer/update/` and starts it with `systemd-run --user --unit=frame-audio-mixer-update --collect`. Downloads are HTTPS only, from `github.com`, `api.github.com`, `codeload.github.com` and `*.githubusercontent.com`, unless `FRAME_UPDATE_ALLOW_INSECURE=1` is set. Redirects are checked, the download stops at 256 MB, the SHA-256 must match, and the tar is rejected if it has an absolute path, `..`, a link or a device node. Cache files stay under `~/.cache/frame-audio-mixer/` (`update-check.json`, `update-state.json`, `update.log`, `update.lock/`, `update/`).

`scripts/get.sh` is the first install. It needs `curl`, `sha256sum` and `tar`, downloads `frame-audio-mixer-aarch64.tar.gz` (the same bytes as the versioned archive) and checks `SHA256SUMS`. Until a release is published it prints that there is no release yet.

## Echo cancellation

Use the Frame speakers with echo cancellation on in Frame Mic Tuner. Otherwise the microphone hears the speakers, and the speakers are already playing the mix, so the PC can hear the app audio twice. Headphones, or a microphone that does not hear the speakers, do not have that acoustic path. An external microphone is still the default source if you select it; this app follows that source and does not replace Frame Mic Tuner's switches.

## Uninstall

```bash
./install.sh --uninstall
./install.sh --uninstall --purge
```

Without `--purge`, settings stay in `~/.config/frame-audio-mixer`. `--purge` also deletes the saved `frame-audio-mixer.*` values. The WirePlumber script is removed from disk and stays loaded until WirePlumber starts again. Do that only when you are not in a VR session.

## Build

On the headset, `rustc` and `cargo` are in `~/.cargo/bin`. The system packages used here are cairo, freetype, Vulkan, PipeWire, and libclang.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
python3 scripts/make-icons.py
cargo build --release
./target/release/frame-audio-mixer --self-test
./target/release/frame-audio-mixer --language en --dump-png /tmp/frame-audio-mixer.png
./scripts/package.sh
```

`scripts/package.sh` writes `dist/frame-audio-mixer-<version>.tar.gz`, the same bytes as `dist/frame-audio-mixer-aarch64.tar.gz`, and `dist/SHA256SUMS` listing both.

On this headset on 2026-10-17, `cargo build --release` finished and `--self-test` printed `self-test ok`. `--dump-png` for `ja`, `en`, `sc`, `de`, `fr`, `es`, and `it` wrote 1200×788 images. In each image the rightmost footer pixel was x=1152, and no language footer crossed the 1200px panel.

## Credit

The panel and the OpenVR overlay are adapted from Frame Mic Tuner by sasaken1102r, which is MIT licensed: https://github.com/sasaken1102r/frame-mic-tuner

The Apps list is a Rust port of frame-apps 0.3.1 from that project. Those app icons are MIT, like the code they came with. The updater behavior follows that project's updater, rewritten in Rust. See `NOTICE`.

## Icons

`scripts/make-icons.py` draws the dashboard icon. Record, Stop and Play are drawn shapes. The other panel glyphs are embedded paths, not fetched at runtime:

| Icon | Set | Author | License |
| --- | --- | --- | --- |
| [fontisto:mic](https://icon-sets.iconify.design/fontisto/mic/) | fontisto | Kenan Gündoğan | MIT |
| [famicons:mic-off](https://icon-sets.iconify.design/famicons/mic-off/) | famicons | Family | MIT |
| [f7:speaker-2-fill](https://icon-sets.iconify.design/f7/speaker-2-fill/) | Framework7 | Vladimir Kharlampidi | MIT |
| [f7:speaker-slash-fill](https://icon-sets.iconify.design/f7/speaker-slash-fill/) | Framework7 | Vladimir Kharlampidi | MIT |
| [fluent:add-square-28-filled](https://icon-sets.iconify.design/fluent/add-square-28-filled/) | Fluent | Microsoft | MIT |
| [heroicons:language-16-solid](https://icon-sets.iconify.design/heroicons/language-16-solid/) | Heroicons | Refactoring UI Inc | MIT |
| [at-icons:headphones](https://icon-sets.iconify.design/at-icons/headphones/) | @icons | Valentin Fossati | MIT |
| [material-symbols:settings-rounded](https://icon-sets.iconify.design/material-symbols/settings-rounded/) | Material Symbols | Google | Apache-2.0 |

## Headset check (2026-10-17)

Measured while `steamvr.service` and `gamescope-session.service` were active. WirePlumber was not restarted. The new script is installed and is not loaded yet.

- `~/.local/bin/frame-audio-mixer` is installed. `frame-audio-mixer.service` is enabled and inactive. `frame-audio-mixer-graph.service` is disabled and inactive. `frame-mic-tuner.service` is enabled and active.
- `frame-mic-tracker.lua` and `90-frame-mic.conf` are still present and do not mention `frame_amix`. `91-frame-audio-mixer.conf` and `frame-audio-mixer.lua` are installed. There is no `frame-mic-mix.service` unit.
- `frame-mic.echo-cancel` is false (saved false). `frame-mic.noise-suppression` is true (saved true). `frame-mic.mix` is still listed, value false (saved false): the running WirePlumber kept the key. `frame-audio-mixer.enabled` and `frame-audio-mixer.vrlink-gain` are not in the running settings yet.
- `pw-dump` had 19 nodes and no `frame_amix_*`, `frame_mix_*`, or `frame_micmix*` nodes. The capture streams were `eq_capture`, `echo_cancel_capture`, `ns_capture`, and the ALSA mic loopback. No VRLink capture stream was present, so its volume was not measured. The default source was `alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source`. The default sink was `alsa_loopback_device.stereo.alsa_output.platform-sound.HiFi__Speaker__sink`.
- Audible checks (the microphone, a doubled echo, an external mic) were not run. They need someone at the PC, and the mix graph was not started during this VR session.
