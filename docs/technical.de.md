[English](technical.md) · [日本語](technical.ja.md) · [简体中文](technical.sc.md) · **Deutsch** · [Français](technical.fr.md) · [Español](technical.es.md) · [Italiano](technical.it.md)

[← README](../README.de.md)

# Technische Details

## Was installiert wird

Die Installation mit einer Zeile führt [`scripts/get.sh`](../scripts/get.sh) aus. Das Skript lädt das neueste Release (`frame-audio-mixer-aarch64.tar.gz`), prüft es gegen `SHA256SUMS` und führt dessen `install.sh` aus. Solange kein Release veröffentlicht ist, lädt es stattdessen den Branch `main` und baut ihn. Dafür braucht es Rust (`cargo`) auf dem Headset. Es braucht `curl`, `sha256sum` und `tar`. Optionen nach `sh -s --` werden an `install.sh` weitergegeben.

`install.sh` schreibt nur ins Home-Verzeichnis und nutzt kein `sudo`:

- Programm `~/.local/bin/frame-audio-mixer`
- Runner `~/.local/lib/frame-audio-mixer/frame-audio-mixer-graph.sh`
- Benutzer-Units `frame-audio-mixer.service`, `frame-audio-mixer-graph.service` und `frame-audio-mixer-channel@.service`
- WirePlumber-Skript `~/.local/share/wireplumber/scripts/frame-audio-mixer.lua` und `~/.config/wireplumber/wireplumber.conf.d/91-frame-audio-mixer.conf` (Komponente `custom.frame-audio-mixer`)
- Desktop-Eintrag und Icons unter `~/.local/share`
- Einstellungen in `~/.config/frame-audio-mixer/` und Kanalnamen in `~/.config/pipewire/frame-audio-mixer/<id>.name`

Zur Laufzeit legt die App PipeWire-Knoten mit dem Namen `frame_amix_*` an, außerdem die WirePlumber-Einstellungen `frame-audio-mixer.enabled`, `frame-audio-mixer.vrlink-gain` und `frame-audio-mixer.mic-source` sowie das Dashboard-Overlay `marcus-universe.frame-audio-mixer`.

`install.sh` startet PipeWire und WirePlumber nie neu. Ein neues oder geändertes WirePlumber-Skript wird beim nächsten Start von WirePlumber geladen. Das Headset nur neu starten, wenn keine VR-Sitzung läuft: Ein Neustart von PipeWire oder WirePlumber während des Spielens nimmt SteamVR und Steam Link den Ton. Die Kanäle funktionieren schon vorher, weil auch das Panel ihre Streams pinnt, solange es läuft.

Falls ein älterer Versuch `frame-mic-mix`-Units hinterlassen hat, entfernt `install.sh` sie. Der einzige `frame-mic.*`-Wert, den es ändert, ist ein übrig gebliebenes `frame-mic.mix`. `frame-mic-tracker.lua` und `90-frame-mic.conf` von Frame Mic Tuner werden nicht überschrieben.

Aus einem Git-Checkout baut `./install.sh` mit `cargo` und installiert. `./install.sh --autostart` startet das Panel zusätzlich mit SteamVR. Die verwendeten Optionen werden in `~/.config/frame-audio-mixer/install-args` gespeichert, damit ein Update auf dieselbe Weise installiert.

## Kanäle

- Bis zu 32 Kanäle, `ch1` bis `ch32`. Jeder ist eine systemd-Instanz mit zwei `pw-loopback`-Prozessen.
- Ein Name ist eine Zeile, höchstens 32 Zeichen, und ohne Groß-/Kleinschreibung eindeutig.
- Ein Kanal aus einer laufenden App bekommt den Namen der App, pinnt die App auf sich und zeigt das Icon der App aus ihrem `.desktop`-Eintrag. Ohne Icon zeigt er den ersten Buchstaben.
- Ist die Reihe breiter als das Panel, mit dem Stick des Controllers scrollen oder die Leiste unter den Kanälen ziehen.
- Die Fader gehen von 0 bis 150 %. Beim Ziehen rastet ein Fader bei 20, 50, 80 und 100 % ein, wenn er auf 8 Pixel herankommt. Die Punkte auf einem Fader markieren 20, 50 und 80 %. Die Linie markiert 100 %.

## Mikrofon

Die Mikrofonwahl gilt nur für die Mischung. Nichts in dieser App ruft `wpctl set-default` auf. Der Systemstandard, den Frame Mic Tuner nutzt, bleibt also, wie er ist.

- **Systemstandard** folgt `default.audio.source`.
- **Frame-Mikrofon** ist `alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source`, das Ende der Valve-Kette nach Rauschunterdrückung, Echounterdrückung und Mikrofon-EQ.
- Jede andere aufgeführte Quelle wird über ihren `node.name` gespeichert.

Die Wahl wird als `frame-audio-mixer.mic-source` gespeichert (`""`, `builtin` oder ein Knotenname). Das Skript löst `builtin` zur oben genannten Loopback auf, fällt auf den Systemstandard zurück, wenn dieser Knoten fehlt, und nutzt nie `frame_amix_source`. Ein geänderter Wert wirkt erst nach dem nächsten Start von WirePlumber.

## Pegelanzeigen

Die Pegel laufen nur, solange das Dashboard sichtbar ist, Mix an ist und die Pegelanzeige in den App-Einstellungen (das Zahnrad neben dem Sprachmenü) an ist. Gemessen werden nur die Kanäle, die auf dem Bildschirm zu sehen sind. Jede Pegelanzeige ist ein `pw-record`-Tap mit `node.virtual=true`. Deshalb schaltet sie die Mikrofonfilter von Valve nicht ein. Kanal-Taps sind passiv. Der Mikrofon-Tap ist es nicht. Solange die Pegel laufen, zeichnet das Panel mit etwa 30 Bildern pro Sekunde neu. Bei ausgeblendetem Dashboard zeichnet es nicht.

## Apps und Updates

**Apps und Updates** zeigt diese App, Frame Mic Tuner als verwandte App und die Frame-Apps aus sasakens Katalog. **Installieren** öffnet Konsole mit `curl -fsSL https://frame.sasaken1102s.net | sh`. Ein Frame Mic Tuner, der schon läuft, hat keinen Installieren-Knopf. Diese Seite ersetzt also keinen Fork davon.

Der Updater ist in Rust geschrieben (`ureq` mit rustls, `sha2`, `flate2` und `tar`):

- Eine Prüfung liest das neueste GitHub-Release, vergleicht die Versionen und sucht nach `frame-audio-mixer-<version>.tar.gz` und `SHA256SUMS`.
- Die Installation kopiert die laufende Binary nach `~/.cache/frame-audio-mixer/update/` und startet sie mit `systemd-run --user --unit=frame-audio-mixer-update --collect`.
- Downloads nur per HTTPS, von `github.com`, `api.github.com`, `codeload.github.com` und `*.githubusercontent.com`, außer `FRAME_UPDATE_ALLOW_INSECURE=1` ist gesetzt. Weiterleitungen werden geprüft, ein Download bricht bei 256 MB ab, und die SHA-256 muss stimmen.
- Das Archiv wird abgelehnt, wenn es einen absoluten Pfad, `..`, einen Link oder eine Gerätedatei enthält.
- Die Cache-Dateien liegen in `~/.cache/frame-audio-mixer/` (`update-check.json`, `update-state.json`, `update.log`, `update.lock/`, `update/`).

## Deinstallation

Mit dem Befehl in einer Zeile:

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/i -o frame-audio-mixer-install && sh frame-audio-mixer-install --uninstall
```

Aus einem Checkout oder einem Release-Ordner:

```bash
./install.sh --uninstall
./install.sh --uninstall --purge
```

Ohne `--purge` bleiben die Einstellungen in `~/.config/frame-audio-mixer`. `--purge` löscht auch sie und die gespeicherten Werte `frame-audio-mixer.*`. Das WirePlumber-Skript wird von der Platte entfernt, bleibt aber geladen, bis WirePlumber neu startet.

## Bauen

Auf dem Headset liegen `rustc` und `cargo` in `~/.cargo/bin`. Die verwendeten Systembibliotheken sind cairo, freetype, Vulkan, PipeWire und libclang.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release
./target/release/frame-audio-mixer --self-test
./target/release/frame-audio-mixer --language en --dump-png /tmp/frame-audio-mixer.png
./scripts/package.sh
```

`--dump-png` schreibt die Hauptansicht und die anderen Ansichten (Apps, Hinzufügen, Kanalauswahl, Einstellungen, Sprachmenü, eine gescrollte Reihe während der Aufnahme) neben den angegebenen Pfad.

`scripts/package.sh` schreibt `dist/frame-audio-mixer-<version>.tar.gz`, mit denselben Bytes wie `dist/frame-audio-mixer-aarch64.tar.gz`, und `dist/SHA256SUMS`, das beide auflistet. Alle drei in ein GitHub-Release hochladen, dann nutzen die Installation mit einer Zeile und der Updater in der App dieses Release.
