[English](install-ssh.md) · [日本語](install-ssh.ja.md) · [简体中文](install-ssh.sc.md) · **Deutsch** · [Français](install-ssh.fr.md) · [Español](install-ssh.es.md) · [Italiano](install-ssh.it.md)

[← README](../README.de.md)

# Installation über SSH

Frame Audio Mixer von einem PC aus installieren. Du meldest dich an der Steam Frame an und führst denselben Befehl aus wie in Konsole. PC und Headset müssen im selben Netzwerk sein.

## SSH einschalten

1. Auf der Frame die Steam-Einstellungen öffnen, dann System, dann **Entwicklermodus aktivieren**.
2. Unter Entwickler ein Passwort setzen. Das ist das SSH-Login. Wähle ein starkes Passwort: Mit SSH kann sich jeder im Netzwerk anmelden, der es kennt.
3. Die IP-Adresse des Headsets notieren. Sie steht in den Entwicklereinstellungen oder am Router.

Der Benutzername ist `steamos`.

## Verbinden

Windows 10 und 11, macOS und Linux haben einen OpenSSH-Client. In einem Terminal auf dem PC:

```sh
ssh steamos@<headset-ip>
```

`<headset-ip>` durch die Adresse von oben ersetzen. Beim ersten Mal den Host-Schlüssel bestätigen und danach das Entwicklerpasswort eingeben.

## Installieren

In dieser Sitzung ausführen:

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/i -o frame-audio-mixer-install && sh frame-audio-mixer-install
```

- Um das Panel auch mit SteamVR zu starten, `--autostart` an das Ende setzen.
- Zum Aktualisieren denselben Befehl noch einmal ausführen. Zum Deinstallieren `--uninstall` an das Ende setzen.
- Der Befehl nutzt kein `sudo` und schreibt nur ins Home-Verzeichnis auf dem Headset. Er startet PipeWire und WirePlumber nie neu, eine laufende VR-Sitzung behält also ihren Ton.

Den Befehl kannst du auch in einer Zeile schicken, ohne in der Sitzung zu bleiben. Unter macOS, Linux oder Windows Terminal:

```sh
ssh steamos@<headset-ip> "curl -fsSL https://frame-audio-mixer.marcus-universe.com/i -o frame-audio-mixer-install && sh frame-audio-mixer-install"
```

Das Installationsprogramm läuft nur auf der Steam Frame. Eine SSH-Sitzung ist schon auf dem Headset, die Prüfung ist damit erfüllt.

Danach das SteamVR-Dashboard öffnen und **Frame Audio Mixer** wählen.
