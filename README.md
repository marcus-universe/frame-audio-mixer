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

`scripts/package.sh` writes `dist/frame-audio-mixer-<version>.tar.gz` and `dist/SHA256SUMS`.

On this headset on 2026-10-17, `cargo build --release` finished and `--self-test` printed `self-test ok`. `--dump-png` for `ja`, `en`, `sc`, `de`, `fr`, `es`, and `it` wrote 1200×788 images. In each image the rightmost footer pixel was x=1152, and no language footer crossed the 1200px panel.

## Credit

The panel, the OpenVR overlay, and the updater are adapted from Frame Mic Tuner by sasaken1102r, which is MIT licensed: https://github.com/sasaken1102r/frame-mic-tuner

`vendor/frame-updater/frame-update.sh` is that project's updater.

## Icons

`scripts/make-icons.py` draws the dashboard icon. The panel glyphs are MIT-licensed icons from Iconify:

- https://icon-sets.iconify.design/heroicons/language-16-solid/
- https://icon-sets.iconify.design/fontisto/mic/
- https://icon-sets.iconify.design/famicons/mic-off/
- https://icon-sets.iconify.design/f7/speaker-2-fill/
- https://icon-sets.iconify.design/f7/speaker-slash-fill/
- https://icon-sets.iconify.design/fluent/add-square-28-filled/
- https://icon-sets.iconify.design/at-icons/headphones/ (@icons by Valentin Fossati)
- https://icon-sets.iconify.design/fluent/record-24-filled/
- https://icon-sets.iconify.design/fluent/stop-24-filled/
- https://icon-sets.iconify.design/fluent/play-24-filled/
