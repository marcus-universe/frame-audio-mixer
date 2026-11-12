<p align="center">
  <a href="https://frame-audio-mixer.marcus-universe.com/"><img src="assets/logo/logo_banner_blue.svg" alt="Frame Audio Mixer" width="640"></a>
</p>
<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/github/license/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="License"></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/version-2.0.0-33C4FF?style=flat-square" alt="Version 2.0.0"></a>
  <a href="https://frame-audio-mixer.marcus-universe.com/"><img src="https://img.shields.io/badge/Website-33C4FF?style=flat-square" alt="Website"></a>
  <a href="https://github.com/marcus-universe/frame-audio-mixer/stargazers"><img src="https://img.shields.io/github/stars/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="GitHub stars"></a>
</p>

[English](README.md) · [日本語](README.ja.md) · [简体中文](README.sc.md) · **Deutsch** · [Français](README.fr.md) · [Español](README.es.md) · [Italiano](README.it.md)

# 🎚️ Frame Audio Mixer

Frame Audio Mixer mischt den Ton von Apps mit dem Mikrofon des Headsets und schickt diese Mischung über den drahtlosen Stream an den PC, in das **Steam Streaming Microphone**, das SteamVR schon bereitstellt.

Es sitzt hinter [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner). Echounterdrückung und Rauschunterdrückung bleiben bei Frame Mic Tuner.

## 📥 Installation

In Konsole auf der Frame (**+** in der Leiste unten, dann die Liste der Programme, dann **Konsole**) ausführen:

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/i | sh
```

- 🛠️ Vorher den Entwicklermodus einschalten: Steam-Einstellungen, dann System, dann **Entwicklermodus aktivieren**. Erst danach steht Konsole in der Liste.
- 💻 Vom PC aus: [Installation über SSH](docs/install-ssh.de.md).

## ✨ Funktionen

- 🔄 **Ausrichtung**: Eingänge und Ausgänge wechseln jeweils zwischen stehenden Fadern und Zeilen.
- 🎛️ **Effektbusse**: FX1 und FX2, mit Hall, Kompressor, EQ, Pitch, Flanger und Phaser. Jeder Streifen kann beide einschalten; das Ergebnis ersetzt den Klang des Streifens auf dessen eigenen Ausgängen.
- 🎤 **Virtuelle Mikrofone**: B1 bis B4, benannt, zusätzlich zum Streaming-Mikrofon am PC.
- 🎧 **Lautsprecher, Bluetooth und USB**: die Mischung läuft über die Frame-Lautsprecher, Bluetooth-Kopfhörer oder USB-Audio, solange eines verbunden ist.
- 🧩 **Softwarekanäle**: bis zu 32 benannte Kanäle. Jeder ist ein Ausgang, den jede App wählen kann.
- 🖥️ **Breite Ansicht**: ein 21:9-Layout. Die Trennlinie zwischen den beiden Bereichen lässt sich ziehen. Jede Seite endet bei einem Fünftel des Fensters.
- 📊 **Pegel, Stumm und Einrasten**: dieselbe Skala an jedem Fader, von +6 bis −48, ein Stumm-Knopf unter jedem Fader, und Ausgangsfader, die an den Marken einrasten.
- ⏺️ **Mix-Check**: oben bei den Ausgängen. Einen Ausgang wählen, bis zu 10 Sekunden aufnehmen, dann abspielen oder pausieren.
- 🗺️ **Signalfluss**: eine Live-Karte des gesamten Routings. Jede Route wird gegen die echten PipeWire-Links geprüft; ein fehlender Link erscheint als rot gestrichelte Linie. Ziehen zum Verschieben, Knoten wählen für die PipeWire-Identität.
- 🗂️ **Projekte**: Kanäle, Routing, Effekte und Pegel unter einem Namen speichern und jederzeit wieder laden.
- 🌐 Sieben Sprachen und ein blaues oder rosa Farbschema.

<table>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-1.jpg" alt="Breite Ansicht, blaues Thema" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-2.jpg" alt="Eingänge als Zeilen, rosa Thema" height="320"></td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-3.jpg" alt="Effektbus, rosa Thema" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-4.jpg" alt="Dashboard, blaues Thema" height="320"></td>
  </tr>
</table>

## ▶️ Bedienung

1. Das SteamVR-Dashboard öffnen und **Frame Audio Mixer** wählen.
2. Die Mischung einschalten. Der PC hört sie als **Steam Streaming Microphone**.
3. Mit **+** einen Kanal hinzufügen. Einen Namen vergeben oder eine laufende App wählen.
4. In dieser App den Ausgang mit dem Namen des Kanals wählen.
5. **Eingänge** links ist das, was in die Mischung geht. **Ausgänge** rechts ist das Ziel: der PC, die Frame-Lautsprecher, Bluetooth, USB oder ein virtuelles Mikrofon.

🎧 Bei den Lautsprechern der Frame die Echounterdrückung in Frame Mic Tuner einschalten. Sonst kann der PC den Ton doppelt hören.

## 📚 Dokumentation

- 📖 [So funktioniert es](docs/how-it-works.de.md): der Audio-Graph und wie VRLink und Apps geroutet werden.
- 🔧 [Technische Details](docs/technical.de.md): was installiert wird, Details zu Mikrofon und Pegelanzeige, Apps und Updates, Deinstallation, Bauen.

## 🙏 Danksagung

Frame Audio Mixer ist inspiriert von [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner) von [sasaken1102r](https://github.com/sasaken1102r). Weitere Steam-Frame-Apps von sasaken@ gibt es auf [frame.sasaken1102s.net](https://frame.sasaken1102s.net/).

Das Panel, das OpenVR-Overlay und die App-Liste sind aus diesem Projekt (MIT) übernommen und angepasst. Siehe [`NOTICE`](NOTICE).

- 🎨 [Icons und ihre Lizenzen](docs/icons.de.md)

## 📄 Lizenz

[MIT](LICENSE)
