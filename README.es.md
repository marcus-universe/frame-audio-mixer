# Frame Audio Mixer

Frame Audio Mixer mezcla salidas de aplicaciones con nombre con el micrófono de Steam Frame y envía esa mezcla al PC por el flujo inalámbrico. El PC muestra el flujo como **Steam Streaming Microphone**.

La aplicación va después de [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner). La cancelación de eco y la supresión de ruido siguen en Frame Mic Tuner. Este seguimiento del micrófono no se sustituye.

## Qué hace

- Añade hasta ocho salidas con nombre, por ejemplo música o un juego.
- Envía cada salida al visor y, con su propio nivel, a la mezcla del micrófono.
- Mientras Mix está activo, solo la captura de VRLink apunta a esa mezcla. Las demás aplicaciones conservan el micrófono predeterminado.
- El grafo permanece apagado hasta que se activa Mix. Entonces el PC vuelve a oír el micrófono normal.

## Instalación

Ejecútalo en el visor como el usuario normal. No usa `sudo` y solo escribe en el directorio personal.

Desde un checkout de git o un archivo de versión:

```bash
./install.sh
./install.sh --autostart
```

`--autostart` también arranca el panel con SteamVR. El grafo de mezcla sigue apagado hasta que actives Mix en el panel.

`install.sh` no reinicia PipeWire ni WirePlumber. Un script nuevo de WirePlumber se carga la próxima vez que WirePlumber arranca. Hazlo solo cuando no haya una sesión de VR. Reiniciar PipeWire o WirePlumber mientras juegas corta el audio de SteamVR y de Steam Link.

La instalación añade una aplicación aparte:

- binario `~/.local/bin/frame-audio-mixer`
- unidades de usuario `frame-audio-mixer.service`, `frame-audio-mixer-graph.service` y `frame-audio-mixer-channel@.service`
- nodos de PipeWire llamados `frame_amix_*`
- componente de WirePlumber `custom.frame-audio-mixer`
- ajustes `frame-audio-mixer.enabled` y `frame-audio-mixer.vrlink-gain`
- clave de overlay del panel `marcus-universe.frame-audio-mixer`

Si un experimento anterior dejó unidades `frame-mic-mix`, `install.sh` las quita. El único valor `frame-mic.*` que cambia es borrar un `frame-mic.mix` que hubiera quedado. No sobrescribe `frame-mic-tracker.lua` ni `90-frame-mic.conf`.

## Uso

Abre **Frame Audio Mixer** en el panel de SteamVR. Volver a lanzar la aplicación mientras está en marcha abre el mismo panel.

1. Activa Mix.
2. Añade un canal (hasta 8). El nombre es una línea, como máximo 32 caracteres, y es único sin distinguir mayúsculas.
3. En la aplicación, elige la salida `Frame Mix: <name>`.
4. **Auriculares** es el volumen de ese canal en el visor. **Al micrófono** es cuánto oye el PC. El fader del micrófono y su silencio solo afectan a esta mezcla, no al micrófono predeterminado del sistema.

**Comprobar mezcla** graba **Frame Mic + Mix** hasta 10 segundos y lo reproduce en la salida actual. El pie elige el idioma (日本語, English, 简体中文, Deutsch, Français, Español, Italiano), arranca el panel con SteamVR y sale. **Aplicaciones y actualizaciones** comprueba las versiones de esta aplicación y enlaza Frame Mic Tuner.

## Cómo funciona

El micrófono es la fuente predeterminada en ese momento. En Frame es el micrófono integrado después de los filtros de Valve, salvo que hayas elegido otro micrófono.

Con Mix activo, `frame-audio-mixer-graph.service` construye el núcleo:

- `frame_amix_from_mic` captura la fuente predeterminada.
- `frame_amix_mic_virtual_sink` es el fader del micrófono hacia `frame_amix_bus`.
- `frame_amix_source` (**Frame Mic + Mix**) es la fuente que graba VRLink.

Cada canal es su propio servicio `frame-audio-mixer-channel@chN.service`:

- `frame_amix_chN` es el sumidero que eligen las aplicaciones (`Frame Mix: <name>`).
- `frame_amix_chN_virtual_sink_playback` reproduce ese sumidero en el visor. Es el fader de auriculares.
- `frame_amix_chN_virtual_sink_to_bus` envía el mismo sumidero al bus de mezcla. Es el fader hacia el micrófono.

Esos nombres internos de reproducción contienen `virtual` y `sink`. El espacializador no los envuelve y no manda la mezcla una segunda vez a los altavoces. El sumidero del canal no es `alsa_output` ni `bluez_output`, así que no aparece en la lista de salidas de Frame Mic Tuner.

El componente de WirePlumber `custom.frame-audio-mixer` fija la captura de VRLink en `frame_amix_source` mediante `object.serial` mientras `frame-audio-mixer.enabled` es verdadero. Un nombre de nodo en los metadatos no mueve el flujo. Si falta la fuente, VRLink sigue con la fuente predeterminada. El script no cambia `frame-mic.echo-cancel` ni `frame-mic.noise-suppression`.

El panel fija una aplicación del mismo modo, con el `object.serial` del sumidero. El serial cambia al reiniciar el canal, y la fijación se vuelve a escribir. Esta aplicación no llama a `wpctl set-default`.

`frame-audio-mixer.vrlink-gain` vale `1.0` de forma predeterminada. Con ese valor la aplicación no cambia el volumen de VRLink. Cualquier otra ganancia se guarda y se restaura al apagar Mix. Apagar Mix detiene las unidades del grafo y los nodos `frame_amix_*` desaparecen.

## Cancelación de eco

Con los altavoces de Frame, activa la cancelación de eco en Frame Mic Tuner. Si no, el micrófono oye los altavoces, y los altavoces ya reproducen la mezcla, así que el PC puede oír el audio de la aplicación dos veces. Los auriculares, o un micrófono que no oye los altavoces, no tienen ese camino acústico. Un micrófono externo sigue siendo la fuente predeterminada si lo seleccionas. Esta aplicación sigue esa fuente y no sustituye los interruptores de Frame Mic Tuner.

## Desinstalación

```bash
./install.sh --uninstall
./install.sh --uninstall --purge
```

Sin `--purge`, los ajustes quedan en `~/.config/frame-audio-mixer`. `--purge` también borra los valores guardados `frame-audio-mixer.*`. El script de WirePlumber se quita del disco y sigue cargado hasta que WirePlumber vuelva a arrancar. Hazlo solo cuando no haya una sesión de VR.

## Compilación

En el visor, `rustc` y `cargo` están en `~/.cargo/bin`. Los paquetes del sistema que se usan son cairo, freetype, Vulkan, PipeWire y libclang.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
python3 scripts/make-icons.py
cargo build --release
./target/release/frame-audio-mixer --self-test
./target/release/frame-audio-mixer --language es --dump-png /tmp/frame-audio-mixer.png
./scripts/package.sh
```

`scripts/package.sh` escribe `dist/frame-audio-mixer-<version>.tar.gz` y `dist/SHA256SUMS`.

En este visor el 2026-10-17, `cargo build --release` terminó y `--self-test` imprimió `self-test ok`. `--dump-png` para `ja`, `en`, `sc`, `de`, `fr`, `es` e `it` escribió imágenes de 1200×788. En cada imagen el píxel más a la derecha del pie estaba en x=1152. Ningún idioma se salía del panel de 1200 px.

## Micrófono, medidores, apps

La elección del micrófono solo afecta a la mezcla. La app no llama a `wpctl set-default`. «Predeterminado del sistema» sigue la fuente del sistema. «Micrófono del Frame» es `alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source`, el final de la cadena de Valve después de Frame Mic Tuner. `frame-audio-mixer.mic-source` se lee la próxima vez que WirePlumber arranca, solo con el visor en reposo.

Los medidores solo funcionan si el panel está visible, la mezcla está activa y los medidores están activos en Ajustes (el engranaje junto al idioma). Cada toma es un `pw-record` con `node.virtual=true`. Los canales son pasivos; el micrófono no. El panel se redibuja a 15 fps mientras están activos.

Apps y actualizaciones muestra esta app, Frame Mic Tuner como app relacionada y las apps de Frame de sasaken. Instalar abre Konsole. Un Frame Mic Tuner que ya está en ejecución no tiene botón Instalar.

El actualizador es Rust (`ureq` con rustls, `sha2`, `flate2`, `tar`). Ya no se instala `frame-update.sh`. La comprobación lee la última release de GitHub. La instalación copia el binario y lo arranca con `systemd-run --user --unit=frame-audio-mixer-update`. Descargas solo por HTTPS de GitHub, máximo 256 MB, el SHA-256 debe coincidir, y el tar se rechaza si tiene una ruta absoluta, `..`, un enlace o un dispositivo. `scripts/get.sh` necesita `curl`, `sha256sum` y `tar`. Hasta que haya una release, dice que todavía no hay. `scripts/package.sh` también escribe `dist/frame-audio-mixer-aarch64.tar.gz` con los mismos bytes.

## Crédito

El panel y el overlay de OpenVR están adaptados de Frame Mic Tuner de sasaken1102r, con licencia MIT: https://github.com/sasaken1102r/frame-mic-tuner

La lista de apps es un port a Rust de frame-apps 0.3.1. Esos iconos son MIT. El actualizador sigue el comportamiento de ese proyecto, reescrito en Rust. Véase `NOTICE`.

## Iconos

`scripts/make-icons.py` dibuja el icono del panel. Grabar, Parar y Reproducir son formas dibujadas. Los demás glifos son trazados incrustados:

| Icono | Conjunto | Autor | Licencia |
| --- | --- | --- | --- |
| [fontisto:mic](https://icon-sets.iconify.design/fontisto/mic/) | fontisto | Kenan Gündoğan | MIT |
| [famicons:mic-off](https://icon-sets.iconify.design/famicons/mic-off/) | famicons | Family | MIT |
| [f7:speaker-2-fill](https://icon-sets.iconify.design/f7/speaker-2-fill/) | Framework7 | Vladimir Kharlampidi | MIT |
| [f7:speaker-slash-fill](https://icon-sets.iconify.design/f7/speaker-slash-fill/) | Framework7 | Vladimir Kharlampidi | MIT |
| [fluent:add-square-28-filled](https://icon-sets.iconify.design/fluent/add-square-28-filled/) | Fluent | Microsoft | MIT |
| [heroicons:language-16-solid](https://icon-sets.iconify.design/heroicons/language-16-solid/) | Heroicons | Refactoring UI Inc | MIT |
| [at-icons:headphones](https://icon-sets.iconify.design/at-icons/headphones/) | @icons | Valentin Fossati | MIT |
| [material-symbols:settings-rounded](https://icon-sets.iconify.design/material-symbols/settings-rounded/) | Material Symbols | Google | Apache-2.0 |

## Comprobación en el visor (2026-10-17)

Medido mientras `steamvr.service` y `gamescope-session.service` estaban active. WirePlumber no se reinició. El script nuevo está instalado y todavía no está cargado.

- `~/.local/bin/frame-audio-mixer` está instalado. `frame-audio-mixer.service` está enabled e inactive. `frame-audio-mixer-graph.service` está disabled e inactive. `frame-mic-tuner.service` está enabled y active.
- `frame-mic-tracker.lua` y `90-frame-mic.conf` siguen presentes y no mencionan `frame_amix`. `91-frame-audio-mixer.conf` y `frame-audio-mixer.lua` están instalados. No hay una unidad `frame-mic-mix.service`.
- `frame-mic.echo-cancel` es false (guardado false). `frame-mic.noise-suppression` es true (guardado true). `frame-mic.mix` sigue en la lista, valor false (guardado false): el WirePlumber en marcha conservó la clave. `frame-audio-mixer.enabled` y `frame-audio-mixer.vrlink-gain` aún no están en los ajustes en marcha.
- `pw-dump` tenía 19 nodos y ningún `frame_amix_*`, `frame_mix_*` ni `frame_micmix*`. Los flujos de captura eran `eq_capture`, `echo_cancel_capture`, `ns_capture` y el loopback ALSA del micrófono. No había un flujo de captura VRLink, así que no se midió su volumen. La fuente predeterminada era `alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source`. El sumidero predeterminado era `alsa_loopback_device.stereo.alsa_output.platform-sound.HiFi__Speaker__sink`.
- No se hicieron pruebas audibles (micrófono, eco duplicado, micrófono externo). Hace falta alguien en el PC, y el grafo de mezcla no se arrancó durante esta sesión de VR.
