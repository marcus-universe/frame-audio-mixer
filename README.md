<p align="center">
  <img src="assets/logo/logo_banner_blue.svg" alt="Frame Audio Mixer" width="640">
</p>
<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/github/license/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="License"></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/version-1.1.0-33C4FF?style=flat-square" alt="Version 1.1.0"></a>
  <a href="https://frame-audio-mixer.marcus-universe.com/"><img src="https://img.shields.io/badge/Website-33C4FF?style=flat-square" alt="Website"></a>
  <a href="https://github.com/marcus-universe/frame-audio-mixer/stargazers"><img src="https://img.shields.io/github/stars/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="GitHub stars"></a>
</p>

**English** · [日本語](README.ja.md) · [简体中文](README.sc.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Italiano](README.it.md)

# Frame Audio Mixer

A SteamVR dashboard panel for the Steam Frame. It mixes app audio with the headset microphone and sends that mix to the PC over the wireless stream, where it shows up as **Steam Streaming Microphone**.

It sits after [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner), which still handles echo cancellation and noise suppression.

## Install

In Konsole on the Frame (**+** on the bar at the bottom, then the list of programs, then **Konsole**), run:

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/i -o frame-audio-mixer-install && sh frame-audio-mixer-install
```

- Turn on Developer Mode first: Steam Settings, then System, then **Enable Developer Mode**. Konsole is not in the list until you do.
- To also start the panel with SteamVR, add `--autostart` at the end.
- To update, run the same command again. To uninstall, add `--uninstall` at the end.
- It does not use `sudo` and only writes to your home directory. It never restarts PipeWire or WirePlumber, so a running VR session keeps its sound.
- From a PC: [Install over SSH](docs/install-ssh.md).

## Features

- **Virtual audio devices**: up to 32 named channels. Each one is an output (`Frame Mix: <name>`) that any app can select.
- **App channels**: make a channel from a running app. Its icon is shown next to the name.
- **PC streaming microphone mix**: the PC hears your microphone plus the channels.
- **Two faders per channel**: one for the headset, one for the PC, each with its own mute. Faders snap to 20, 50, 80 and 100 %.
- **Microphone**: choose the microphone for the mix. The system default microphone does not change.
- **Level meters** on every fader. They can be turned off.
- **Mix Check**: record up to 10 seconds of what the PC hears and play it back.
- Seven languages, and a blue or pink color theme.

<table>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-1.png" alt="Column view, blue theme" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-2.png" alt="List view, blue theme" height="320"></td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-3.png" alt="Column view, pink theme" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-4.png" alt="List view, pink theme" height="320"></td>
  </tr>
</table>

## Usage

1. Open the SteamVR dashboard and choose **Frame Audio Mixer**.
2. Turn **Mix** on.
3. Press **+** to add a channel: a suggestion, your own name, or a running app.
4. In that app, choose the output `Frame Mix: <name>`.
5. The speaker fader sets how loud the channel is in your headset. The microphone fader sets how much of it the PC hears.

If you use the Frame's speakers, turn echo cancellation on in Frame Mic Tuner. Otherwise the PC can hear the app audio twice.

## Documentation

- [How it works](docs/how-it-works.md): the audio graph, and how VRLink and apps are routed.
- [Technical details](docs/technical.md): what gets installed, microphone and level meter details, Apps and updates, uninstall, build.

## Credits

Frame Audio Mixer is inspired by [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner) by [sasaken1102r](https://github.com/sasaken1102r). More Steam Frame apps by sasaken@ are at [frame.sasaken1102s.net](https://frame.sasaken1102s.net/).

The panel, the OpenVR overlay and the Apps list are adapted from that project (MIT). See [`NOTICE`](NOTICE).

- [Icons and their licenses](docs/icons.md)

## License

[MIT](LICENSE)
