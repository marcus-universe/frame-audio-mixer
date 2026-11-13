**English** · [日本語](install-pc.ja.md) · [简体中文](install-pc.sc.md) · [Deutsch](install-pc.de.md) · [Français](install-pc.fr.md) · [Español](install-pc.es.md) · [Italiano](install-pc.it.md)

[← README](../README.md)

# Install the PC audio devices

**Frame Audio Link** is a SteamVR add-on for the PC you stream from. It adds eight sound devices to the PC:

| On the PC | What it is |
|---|---|
| **Steam Streaming Mic B1** … **B4** | Microphones. They carry B1–B4 of the mixer on the headset. |
| **Steam Streaming Out PC1** … **PC4** | Outputs. What plays on them can be added in the mixer as channels PC1–PC4. |

Steam's own **Steam Streaming Microphone** and **Steam Streaming Speakers** stay as they are.

## Before you start

- Frame Audio Mixer 2.2 or newer on the headset (see [Install](../README.md#-install)).
- SteamVR on the PC, and wireless streaming to the Steam Frame working.

## Windows

1. Download `frame-audio-link-windows-x86_64.zip` from the [latest release](https://github.com/marcus-universe/frame-audio-mixer/releases/latest) and unpack it.
2. Double-click **install.cmd**.
3. Windows asks once for administrator permission. That step creates the eight devices.

Or in PowerShell:

```powershell
irm https://frame-audio-mixer.marcus-universe.com/get-windows | iex
```

The program goes to `%LOCALAPPDATA%\Programs\FrameAudioLink`.

- The devices are more instances of the two audio drivers Steam already installed. Nothing new is installed as a driver.
- Steam installs its drivers the first time Remote Play or wireless VR streaming is used. If the installer says they are missing, stream once and run it again.
- If Windows makes one of the new devices the default, the installer puts your previous default back.

The devices stay installed, also while SteamVR is closed.

## Linux

Works on Ubuntu 22.10 or newer, Fedora and Arch. PipeWire must be the sound server. Check with:

```sh
pactl info | grep "Server Name"
```

It should say `PulseAudio (on PipeWire …)`. Ubuntu 22.04 still uses PulseAudio and is not supported. On Arch, install `pipewire-pulse` and `wireplumber` first if they are missing.

Install as your normal user, without `sudo`:

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/get-linux | sh
```

The program goes to `~/.local/bin/frame-audio-link`. On Linux the eight devices are PipeWire nodes: they exist while the add-on runs, which is while SteamVR runs. Nothing is installed into the sound system.

## Turn it on in SteamVR

The installer registers the add-on as a startup app. Check that it is switched on: **SteamVR → Settings → Startup / Shutdown → Choose Startup Overlay Apps → Frame Audio Link**.

The dashboard then has an **Audio Link** tab. It shows whether the headset is found, and has a switch for each of the eight streams. A stream that is off uses no network bandwidth.

## Use it

- **Headset to PC**: in the mixer, route the microphone or a channel to B1–B4. On the PC, choose **Steam Streaming Mic B1** … **B4** as the microphone in any app (Discord, OBS, a game).
- **PC to headset**: on the PC, let an app play on **Steam Streaming Out PC1** … **PC4**. In the mixer, press **+** and pick **PC1** … **PC4** under Running.

## Check

- **Windows**: the eight devices are listed in the Sound control panel (`mmsys.cpl`). `frame-audio-link devices status` in a terminal lists them too.
- **Linux**: while the add-on runs, `pactl list short sources` and `pactl list short sinks` show `frame_link_b1` … and `frame_link_pc1` ….
- **Overlay says "Searching"**: start wireless streaming on the Steam Frame and open Frame Audio Mixer there. The headset only answers the PC it is streaming from.
- **Overlay says a device is not installed**: on Windows, run `install.cmd` again.

The log is in `%APPDATA%\frame-audio-link\frame-audio-link.log` on Windows and in `~/.config/frame-audio-link/frame-audio-link.log` on Linux.

## Uninstall

- **Windows**: double-click **uninstall.cmd** from the archive. It removes the eight devices, the SteamVR entry and the program.
- **Linux**: run `./install-linux.sh --uninstall` from the archive.

How the link works is in [PC audio link](pc-link.md).
