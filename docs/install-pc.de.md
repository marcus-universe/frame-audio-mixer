[English](install-pc.md) · [日本語](install-pc.ja.md) · [简体中文](install-pc.sc.md) · **Deutsch** · [Français](install-pc.fr.md) · [Español](install-pc.es.md) · [Italiano](install-pc.it.md)

[← README](../README.de.md)

# PC-Audiogeräte installieren

**Frame Audio Link** ist ein SteamVR-Add-on für den PC, von dem du streamst. Es fügt dem PC acht Audiogeräte hinzu:

| Am PC | Was es ist |
|---|---|
| **Steam Streaming Mic B1** … **B4** | Mikrofone. Sie führen B1–B4 des Mixers auf dem Headset. |
| **Steam Streaming Out PC1** … **PC4** | Ausgänge. Was darauf spielt, lässt sich im Mixer als Kanal PC1–PC4 hinzufügen. |

Steams eigenes **Steam Streaming Microphone** und die **Steam Streaming Speakers** bleiben, wie sie sind.

## Voraussetzungen

- Frame Audio Mixer 2.2 oder neuer auf dem Headset (siehe [Installation](../README.de.md#-installation)).
- SteamVR am PC, und kabelloses Streaming zur Steam Frame funktioniert.

## Windows

1. `frame-audio-link-windows-x86_64.zip` aus dem [neuesten Release](https://github.com/marcus-universe/frame-audio-mixer/releases/latest) laden und entpacken.
2. **install.cmd** doppelklicken.
3. Windows fragt einmal nach Administratorrechten. Dieser Schritt legt die acht Geräte an.

Oder in PowerShell:

```powershell
irm https://frame-audio-mixer.marcus-universe.com/get-windows | iex
```

Das Programm landet in `%LOCALAPPDATA%\Programs\FrameAudioLink`.

- Die Geräte sind weitere Instanzen der beiden Audiotreiber, die Steam schon installiert hat. Es wird kein neuer Treiber installiert.
- Steam installiert seine Treiber, wenn Remote Play oder kabelloses VR-Streaming zum ersten Mal benutzt wird. Meldet der Installer, dass sie fehlen: einmal streamen und erneut ausführen.
- Macht Windows eines der neuen Geräte zum Standard, stellt der Installer dein bisheriges Standardgerät wieder her.

Die Geräte bleiben installiert, auch wenn SteamVR geschlossen ist.

## Linux

Läuft auf Ubuntu 22.10 oder neuer, Fedora und Arch. PipeWire muss der Soundserver sein. Prüfen mit:

```sh
pactl info | grep "Server Name"
```

Dort sollte `PulseAudio (on PipeWire …)` stehen. Ubuntu 22.04 nutzt noch PulseAudio und wird nicht unterstützt. Unter Arch vorher `pipewire-pulse` und `wireplumber` installieren, falls sie fehlen.

Als normaler Benutzer installieren, ohne `sudo`:

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/get-linux | sh
```

Das Programm landet in `~/.local/bin/frame-audio-link`. Unter Linux sind die acht Geräte PipeWire-Knoten: Es gibt sie, solange das Add-on läuft, also solange SteamVR läuft. Am Soundsystem wird nichts installiert.

## In SteamVR einschalten

Der Installer trägt das Add-on als Startanwendung ein. Prüfen, ob es eingeschaltet ist: **SteamVR → Einstellungen → Starten/Beenden → Startanwendungen für Overlay auswählen → Frame Audio Link**.

Im Dashboard gibt es dann den Tab **Audio Link**. Er zeigt, ob das Headset gefunden wurde, und hat für jeden der acht Streams einen Schalter. Ein ausgeschalteter Stream belegt keine Bandbreite.

## Benutzen

- **Vom Headset zum PC**: Im Mixer das Mikrofon oder einen Kanal auf B1–B4 legen. Am PC in einer beliebigen App (Discord, OBS, ein Spiel) **Steam Streaming Mic B1** … **B4** als Mikrofon wählen.
- **Vom PC zum Headset**: Am PC eine App auf **Steam Streaming Out PC1** … **PC4** ausgeben lassen. Im Mixer **+** drücken und unter „Läuft“ **PC1** … **PC4** wählen.

## Prüfen

- **Windows**: Die acht Geräte stehen in der Sound-Systemsteuerung (`mmsys.cpl`). `frame-audio-link devices status` in einem Terminal listet sie ebenfalls auf.
- **Linux**: Solange das Add-on läuft, zeigen `pactl list short sources` und `pactl list short sinks` die Einträge `frame_link_b1` … und `frame_link_pc1` ….
- **Overlay zeigt „Headset wird gesucht“**: Kabelloses Streaming an der Steam Frame starten und dort Frame Audio Mixer öffnen. Das Headset antwortet nur dem PC, von dem es streamt.
- **Overlay meldet ein fehlendes Audiogerät**: Unter Windows `install.cmd` noch einmal ausführen.

Das Protokoll liegt unter Windows in `%APPDATA%\frame-audio-link\frame-audio-link.log`, unter Linux in `~/.config/frame-audio-link/frame-audio-link.log`.

## Deinstallieren

- **Windows**: **uninstall.cmd** aus dem Archiv doppelklicken. Es entfernt die acht Geräte, den SteamVR-Eintrag und das Programm.
- **Linux**: `./install-linux.sh --uninstall` aus dem Archiv ausführen.

Wie der Link funktioniert, steht unter [PC-Audio-Link](pc-link.de.md).
