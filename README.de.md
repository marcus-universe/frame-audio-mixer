<p align="center">
  <img src="assets/logo/logo_banner_blue.svg" alt="Frame Audio Mixer" width="640">
</p>
<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/github/license/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="License"></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/version-0.1.0-33C4FF?style=flat-square" alt="Version 0.1.0"></a>
  <a href="https://github.com/marcus-universe/frame-audio-mixer/stargazers"><img src="https://img.shields.io/github/stars/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="GitHub stars"></a>
</p>

[English](README.md) · [日本語](README.ja.md) · [简体中文](README.sc.md) · **Deutsch** · [Français](README.fr.md) · [Español](README.es.md) · [Italiano](README.it.md)

# Frame Audio Mixer

Ein SteamVR-Dashboard-Panel für die Steam Frame. Es mischt den Ton von Apps mit dem Mikrofon des Headsets und schickt diese Mischung über den drahtlosen Stream an den PC. Dort erscheint sie als **Steam Streaming Microphone**.

Es sitzt hinter [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner). Echounterdrückung und Rauschunterdrückung bleiben bei Frame Mic Tuner.

## Funktionen

- **Virtuelle Audiogeräte**: bis zu 32 benannte Kanäle. Jeder Kanal ist ein Ausgang (`Frame Mix: <name>`), den jede App wählen kann.
- **App-Kanäle**: einen Kanal aus einer laufenden App anlegen. Ihr Icon steht neben dem Namen.
- **Mischung für das Streaming-Mikrofon am PC**: der PC hört das Mikrofon und die Kanäle.
- **Zwei Fader pro Kanal**: einer für das Headset, einer für den PC, jeder mit eigener Stummschaltung. Die Fader rasten bei 20, 50, 80 und 100 % ein.
- **Mikrofon**: das Mikrofon für die Mischung wählen. Das Standardmikrofon des Systems ändert sich nicht.
- **Pegelanzeigen** an jedem Fader. Sie lassen sich ausschalten.
- **Mix-Check**: bis zu 10 Sekunden von dem aufnehmen, was der PC hört, und wieder abspielen.
- Sieben Sprachen und ein blaues oder rosa Farbschema.

## Installation

In Konsole auf der Frame (**+** in der Leiste unten, dann die Liste der Programme, dann **Konsole**) ausführen:

```sh
curl -fsSL https://raw.githubusercontent.com/marcus-universe/frame-audio-mixer/main/scripts/get.sh | sh
```

- Vorher den Entwicklermodus einschalten: Steam-Einstellungen, dann System, dann **Entwicklermodus aktivieren**. Erst danach steht Konsole in der Liste.
- Um das Panel auch mit SteamVR zu starten, den Befehl mit `| sh -s -- --autostart` beenden.
- Zum Aktualisieren denselben Befehl noch einmal ausführen. Zum Deinstallieren den Befehl mit `| sh -s -- --uninstall` beenden.
- Der Befehl nutzt kein `sudo` und schreibt nur ins Home-Verzeichnis. Er startet PipeWire und WirePlumber nie neu, eine laufende VR-Sitzung behält also ihren Ton.

## Bedienung

1. Das SteamVR-Dashboard öffnen und **Frame Audio Mixer** wählen.
2. **Mix** einschalten.
3. Mit **+** einen Kanal hinzufügen: einen Vorschlag, einen eigenen Namen oder eine laufende App.
4. In dieser App den Ausgang `Frame Mix: <name>` wählen.
5. Der Lautsprecher-Fader stellt ein, wie laut der Kanal im Headset ist. Der Mikrofon-Fader stellt ein, wie viel davon der PC hört.

Bei den Lautsprechern der Frame die Echounterdrückung in Frame Mic Tuner einschalten. Sonst kann der PC den App-Ton doppelt hören.

## Dokumentation

- [So funktioniert es](docs/how-it-works.de.md): der Audio-Graph und wie VRLink und Apps geroutet werden.
- [Technische Details](docs/technical.de.md): was installiert wird, Details zu Mikrofon und Pegelanzeige, Apps und Updates, Deinstallation, Bauen.

## Danksagung

Das Panel und das OpenVR-Overlay sind aus [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner) von sasaken1102r (MIT) übernommen und angepasst. Die App-Liste ist eine Rust-Portierung von frame-apps 0.3.1 aus demselben Projekt, und der Updater folgt dessen Verhalten. Siehe [`NOTICE`](NOTICE).

- [Icons und ihre Lizenzen](docs/icons.de.md)

## Lizenz

[MIT](LICENSE)
