# Frame Audio Mixer

Frame Audio Mixer mischt benannte App-Ausgänge mit dem Mikrofon der Steam Frame und schickt diese Mischung über den drahtlosen Stream an den PC. Der PC zeigt den Stream als **Steam Streaming Microphone**.

Die App sitzt hinter [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner). Echo-Unterdrückung und Rauschunterdrückung bleiben bei Frame Mic Tuner. Dieser Tracker wird nicht ersetzt.

## Was sie tut

- Legt bis zu acht benannte Ausgänge an, zum Beispiel Musik oder ein Spiel.
- Spielt jeden Ausgang auf dem Headset und, mit eigenem Pegel, in die Mikrofonmischung.
- Richtet nur die VRLink-Aufnahme auf diese Mischung, solange Mix an ist. Andere Apps behalten das normale Standardmikrofon.
- Lässt den Graphen aus, bis Mix eingeschaltet wird. Danach hört der PC wieder das normale Mikrofon.

## Installation

Auf dem Headset als normaler Benutzer ausführen. Ohne `sudo`, und nur im Home-Verzeichnis.

Aus einem Git-Checkout oder einem Release-Archiv:

```bash
./install.sh
./install.sh --autostart
```

`--autostart` startet das Panel zusammen mit SteamVR. Der Mix-Graph bleibt aus, bis Mix im Panel eingeschaltet wird.

`install.sh` startet weder PipeWire noch WirePlumber neu. Ein neues WirePlumber-Skript wird beim nächsten Start von WirePlumber geladen. Das nur tun, wenn keine VR-Sitzung läuft. Ein Neustart von PipeWire oder WirePlumber während des Spielens nimmt SteamVR und Steam Link den Ton.

Installiert wird eine eigene App:

- Programm `~/.local/bin/frame-audio-mixer`
- Benutzer-Units `frame-audio-mixer.service`, `frame-audio-mixer-graph.service` und `frame-audio-mixer-channel@.service`
- PipeWire-Knoten mit dem Präfix `frame_amix_*`
- WirePlumber-Komponente `custom.frame-audio-mixer`
- Einstellungen `frame-audio-mixer.enabled` und `frame-audio-mixer.vrlink-gain`
- Dashboard-Overlay-Schlüssel `marcus-universe.frame-audio-mixer`

Falls ein älterer Versuch `frame-mic-mix`-Units hinterlassen hat, entfernt `install.sh` sie. Der einzige `frame-mic.*`-Wert, den es ändert, ist ein übrig gebliebenes `frame-mic.mix`. `frame-mic-tracker.lua` und `90-frame-mic.conf` werden nicht überschrieben.

## Bedienung

Im SteamVR-Dashboard **Frame Audio Mixer** öffnen. Ein zweiter Start, während die App läuft, öffnet dasselbe Panel.

1. Mix einschalten.
2. Einen Kanal hinzufügen (bis zu 8). Der Name ist eine Zeile, höchstens 32 Zeichen, und ohne Groß-/Kleinschreibung eindeutig.
3. In der App den Ausgang `Frame Mix: <name>` wählen.
4. **Kopfhörer** ist die Lautstärke dieses Kanals im Headset. **Zum Mikrofon** ist der Anteil, den der PC hört. Fader und Stummschaltung des Mikrofons gelten nur für diese Mischung, nicht für das System-Standardmikrofon.

**Mix-Prüfung** nimmt **Frame Mic + Mix** bis zu 10 Sekunden auf und spielt das auf dem aktuellen Ausgang ab. Die Fußzeile wählt die Sprache (日本語, English, 简体中文, Deutsch, Français, Español, Italiano), startet das Panel mit SteamVR und beendet die App. **Apps & Updates** prüft die Releases dieser App und verlinkt Frame Mic Tuner.

## So funktioniert es

Das Mikrofon ist die aktuelle Standardquelle. Auf der Frame ist das das eingebaute Mikrofon nach den Valve-Filtern, solange kein anderes Mikrofon gewählt ist.

Bei eingeschaltetem Mix baut `frame-audio-mixer-graph.service` den Kern:

- `frame_amix_from_mic` nimmt die Standardquelle auf.
- `frame_amix_mic_virtual_sink` ist der Mikrofon-Fader in den `frame_amix_bus`.
- `frame_amix_source` (**Frame Mic + Mix**) ist die Quelle, die VRLink aufnimmt.

Jeder Kanal ist ein eigener Dienst `frame-audio-mixer-channel@chN.service`:

- `frame_amix_chN` ist die Senke, die Apps wählen (`Frame Mix: <name>`).
- `frame_amix_chN_virtual_sink_playback` spielt diese Senke auf dem Headset. Das ist der Kopfhörer-Fader.
- `frame_amix_chN_virtual_sink_to_bus` schickt dieselbe Senke in den Mix-Bus. Das ist der Fader zum Mikrofon.

Diese internen Wiedergabenamen enthalten `virtual` und `sink`. Der Spatializer packt sie deshalb nicht ein und schickt den Mix nicht ein zweites Mal auf die Lautsprecher. Die Kanalsenke ist weder `alsa_output` noch `bluez_output` und erscheint nicht in der Ausgangsliste von Frame Mic Tuner.

Die WirePlumber-Komponente `custom.frame-audio-mixer` pinnt die VRLink-Aufnahme per `object.serial` auf `frame_amix_source`, solange `frame-audio-mixer.enabled` wahr ist. Ein Knotenname in den Metadaten verschiebt den Stream nicht. Fehlt die Quelle, bleibt VRLink auf der normalen Standardquelle. Das Skript ändert `frame-mic.echo-cancel` und `frame-mic.noise-suppression` nicht.

Das Panel pinnt eine App auf dieselbe Weise, mit der `object.serial` der Senke. Nach einem Neustart des Kanals ändert sich die Serial, und der Pin wird neu geschrieben. Diese App ruft `wpctl set-default` nicht auf.

`frame-audio-mixer.vrlink-gain` ist standardmäßig `1.0`. Dabei lässt die App die Lautstärke von VRLink unverändert. Jeder andere Gain wird gespeichert und beim Ausschalten von Mix wiederhergestellt. Mix aus schaltet die Graph-Units ab, und die Knoten `frame_amix_*` verschwinden.

## Echo-Unterdrückung

Bei den Lautsprechern der Frame die Echo-Unterdrückung in Frame Mic Tuner einschalten. Sonst hört das Mikrofon die Lautsprecher, und die Lautsprecher spielen die Mischung schon. Der PC kann den App-Ton dann doppelt hören. Kopfhörer oder ein Mikrofon, das die Lautsprecher nicht hört, haben diesen akustischen Weg nicht. Ein externes Mikrofon bleibt die Standardquelle, wenn es ausgewählt ist. Diese App folgt dieser Quelle und ersetzt die Schalter von Frame Mic Tuner nicht.

## Deinstallation

```bash
./install.sh --uninstall
./install.sh --uninstall --purge
```

Ohne `--purge` bleiben die Einstellungen in `~/.config/frame-audio-mixer`. `--purge` löscht auch die gespeicherten Werte `frame-audio-mixer.*`. Das WirePlumber-Skript wird von der Platte entfernt und bleibt geladen, bis WirePlumber neu startet. Das nur ohne VR-Sitzung tun.

## Bauen

Auf dem Headset liegen `rustc` und `cargo` in `~/.cargo/bin`. Die verwendeten Systempakete sind cairo, freetype, Vulkan, PipeWire und libclang.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
python3 scripts/make-icons.py
cargo build --release
./target/release/frame-audio-mixer --self-test
./target/release/frame-audio-mixer --language de --dump-png /tmp/frame-audio-mixer.png
./scripts/package.sh
```

`scripts/package.sh` schreibt `dist/frame-audio-mixer-<version>.tar.gz` und `dist/SHA256SUMS`.

Auf diesem Headset am 2026-10-17 lief `cargo build --release` durch, und `--self-test` gab `self-test ok` aus. `--dump-png` für `ja`, `en`, `sc`, `de`, `fr`, `es` und `it` schrieb Bilder mit 1200×788. In jedem Bild lag das rechteste Pixel der Fußzeile bei x=1152. Keine Sprache lief über das 1200px breite Panel hinaus.

## Danksagung

Panel, OpenVR-Overlay und Updater sind eine Umsetzung von Frame Mic Tuner von sasaken1102r, MIT-Lizenz: https://github.com/sasaken1102r/frame-mic-tuner

`vendor/frame-updater/frame-update.sh` ist der Updater dieses Projekts.

## Icons

`scripts/make-icons.py` zeichnet das Dashboard-Icon. Die Zeichen im Panel sind MIT-Icons von Iconify:

- https://icon-sets.iconify.design/heroicons/language-16-solid/
- https://icon-sets.iconify.design/fontisto/mic/
- https://icon-sets.iconify.design/famicons/mic-off/
- https://icon-sets.iconify.design/f7/speaker-2-fill/
- https://icon-sets.iconify.design/f7/speaker-slash-fill/
- https://icon-sets.iconify.design/fluent/add-square-28-filled/
- https://icon-sets.iconify.design/at-icons/headphones/ (@icons von Valentin Fossati)
- https://icon-sets.iconify.design/fluent/record-24-filled/
- https://icon-sets.iconify.design/fluent/stop-24-filled/
- https://icon-sets.iconify.design/fluent/play-24-filled/

## Prüfung auf dem Headset (2026-10-17)

Gemessen, während `steamvr.service` und `gamescope-session.service` active waren. WirePlumber wurde nicht neu gestartet. Das neue Skript ist installiert und noch nicht geladen.

- `~/.local/bin/frame-audio-mixer` ist installiert. `frame-audio-mixer.service` ist enabled und inactive. `frame-audio-mixer-graph.service` ist disabled und inactive. `frame-mic-tuner.service` ist enabled und active.
- `frame-mic-tracker.lua` und `90-frame-mic.conf` sind noch da und erwähnen `frame_amix` nicht. `91-frame-audio-mixer.conf` und `frame-audio-mixer.lua` sind installiert. Es gibt keine Unit `frame-mic-mix.service`.
- `frame-mic.echo-cancel` ist false (gespeichert false). `frame-mic.noise-suppression` ist true (gespeichert true). `frame-mic.mix` steht noch in der Liste, Wert false (gespeichert false): das laufende WirePlumber hat den Schlüssel behalten. `frame-audio-mixer.enabled` und `frame-audio-mixer.vrlink-gain` sind in den laufenden Einstellungen noch nicht vorhanden.
- `pw-dump` hatte 19 Knoten und keine `frame_amix_*`, `frame_mix_*` oder `frame_micmix*`. Die Aufnahme-Streams waren `eq_capture`, `echo_cancel_capture`, `ns_capture` und die ALSA-Mikrofon-Loopback. Ein VRLink-Aufnahmestream war nicht da, seine Lautstärke wurde nicht gemessen. Die Standardquelle war `alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source`. Die Standardsenke war `alsa_loopback_device.stereo.alsa_output.platform-sound.HiFi__Speaker__sink`.
- Hörprüfungen (Mikrofon, doppeltes Echo, externes Mikrofon) wurden nicht gemacht. Dafür muss jemand am PC sein, und der Mix-Graph wurde in dieser VR-Sitzung nicht gestartet.
