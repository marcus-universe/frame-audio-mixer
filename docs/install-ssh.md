**English** · [日本語](install-ssh.ja.md) · [简体中文](install-ssh.sc.md) · [Deutsch](install-ssh.de.md) · [Français](install-ssh.fr.md) · [Español](install-ssh.es.md) · [Italiano](install-ssh.it.md)

[← README](../README.md)

# Install over SSH

Install Frame Audio Mixer from a PC. You log in to the Steam Frame and run the same command as in Konsole. The PC and the headset need to be on the same network.

## Turn on SSH

1. On the Frame, open Steam Settings, then System, then **Enable Developer Mode**.
2. Under Developer, set a password. That password is the SSH login. Choose a strong one: with SSH on, anyone on your network who knows it can log in to the headset.
3. Note the headset's IP address. It is shown in the developer settings, or on your router.

The login name is `steamos`.

## Connect

Windows 10 and 11, macOS and Linux include an OpenSSH client. In a terminal on the PC:

```sh
ssh steamos@<headset-ip>
```

Replace `<headset-ip>` with the address from the step above. The first time, confirm the host key, then enter the developer password.

## Install

In that session, run:

```sh
curl -fsSL https://raw.githubusercontent.com/marcus-universe/frame-audio-mixer/main/scripts/get.sh | sh
```

- To also start the panel with SteamVR, end the command with `| sh -s -- --autostart`.
- To update, run the same command again. To uninstall, end it with `| sh -s -- --uninstall`.
- It does not use `sudo` and only writes to the home directory on the headset. It never restarts PipeWire or WirePlumber, so a running VR session keeps its sound.

You can also send the command in one line and skip the interactive session. On macOS, Linux, or Windows Terminal:

```sh
ssh steamos@<headset-ip> "curl -fsSL https://raw.githubusercontent.com/marcus-universe/frame-audio-mixer/main/scripts/get.sh | sh"
```

The installer only runs on the Steam Frame. An SSH session is already on the headset, so that check passes.

Then open the SteamVR dashboard and choose **Frame Audio Mixer**.
