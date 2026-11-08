<p align="center">
  <img src="assets/logo/logo_banner_blue.svg" alt="Frame Audio Mixer" width="640">
</p>
<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/github/license/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="License"></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/version-2.0.0-33C4FF?style=flat-square" alt="Version 2.0.0"></a>
  <a href="https://frame-audio-mixer.marcus-universe.com/"><img src="https://img.shields.io/badge/Website-33C4FF?style=flat-square" alt="Website"></a>
  <a href="https://github.com/marcus-universe/frame-audio-mixer/stargazers"><img src="https://img.shields.io/github/stars/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="GitHub stars"></a>
</p>

**English** · [日本語](README.ja.md) · [简体中文](README.sc.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Italiano](README.it.md)

# 🎚️ Frame Audio Mixer

Frame Audio Mixer mixes app audio with the headset microphone and sends that mix to the PC over the wireless stream, into the **Steam Streaming Microphone** that SteamVR already provides.

It sits after [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner), which still handles echo cancellation and noise suppression.

## 📥 Install

In Konsole on the Frame (**+** on the bar at the bottom, then the list of programs, then **Konsole**), run:

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/i | sh
```

- 🛠️ Turn on Developer Mode first: Steam Settings, then System, then **Enable Developer Mode**. Konsole is not in the list until you do.
- 🚀 To also start the panel with SteamVR, use `| sh -s -- --autostart` instead of `| sh`.
- 🔄 To update, run the same command again. To uninstall, use `| sh -s -- --uninstall`.
- 🏠 It does not use `sudo` and only writes to your home directory. It never restarts PipeWire or WirePlumber, so a running VR session keeps its sound.
- 💻 From a PC: [Install over SSH](docs/install-ssh.md).

## ✨ Features

- 🔄 **Orientation**: inputs and outputs each switch between upright faders and rows.
- 🎛️ **Effect buses**: FX1 and FX2, each with reverb, compressor, EQ, pitch, flanger and phaser. Every strip can send to either bus.
- 🎤 **Virtual microphones**: B1 to B4, named, on top of the PC streaming microphone.
- 🎧 **Speakers, Bluetooth and USB**: the mix plays on the Frame speakers, Bluetooth headphones, or USB audio while one is connected.
- 🧩 **Software channels**: up to 32 named channels. Each one is an output any app can select.
- 🖥️ **Wide view**: a 21:9 layout. Drag the split between the two sections. Each side stops at a fifth of the window.
- 📊 **Meters, mute and snap**: the same peak scale on every fader, from +6 down to −48, a mute button under each one, and output faders that snap to the marks.
- ⏺️ **Mix Check**: at the top of Outputs. Pick an output, record up to 10 seconds, then play or pause the take.
- 🌐 Seven languages, and a blue or pink color theme.

<table>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-1.jpg" alt="Wide view, blue theme" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-2.jpg" alt="Input rows, pink theme" height="320"></td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-3.jpg" alt="Effect bus, pink theme" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-4.jpg" alt="Dashboard, blue theme" height="320"></td>
  </tr>
</table>

## ▶️ Usage

1. Open the SteamVR dashboard and choose **Frame Audio Mixer**.
2. Turn the mix on. The PC hears it as **Steam Streaming Microphone**.
3. Press **+** and add a channel. Give it a name, or pick an app that is already running.
4. In that app, choose the output with the same name.
5. **Inputs** on the left is what goes into the mix. **Outputs** on the right is where it goes: the PC, the Frame speakers, Bluetooth, USB, or a virtual microphone.

🎧 If you listen on the Frame speakers, turn echo cancellation on in Frame Mic Tuner. Otherwise the PC can hear that sound twice.

## 📚 Documentation

- 📖 [How it works](docs/how-it-works.md): the audio graph, and how VRLink and apps are routed.
- 🔧 [Technical details](docs/technical.md): what gets installed, microphone and level meter details, Apps and updates, uninstall, build.

## 🙏 Credits

Frame Audio Mixer is inspired by [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner) by [sasaken1102r](https://github.com/sasaken1102r). More Steam Frame apps by sasaken@ are at [frame.sasaken1102s.net](https://frame.sasaken1102s.net/).

The panel, the OpenVR overlay and the Apps list are adapted from that project (MIT). See [`NOTICE`](NOTICE).

- 🎨 [Icons and their licenses](docs/icons.md)

## 📄 License

[MIT](LICENSE)
