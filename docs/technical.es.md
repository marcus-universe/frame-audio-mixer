[English](technical.md) · [日本語](technical.ja.md) · [简体中文](technical.sc.md) · [Deutsch](technical.de.md) · [Français](technical.fr.md) · **Español** · [Italiano](technical.it.md)

[← README](../README.es.md)

# Detalles técnicos

## Qué se instala

La instalación de una línea ejecuta [`scripts/get.sh`](../scripts/get.sh). Descarga la última versión publicada (`frame-audio-mixer-aarch64.tar.gz`), la comprueba con `SHA256SUMS` y ejecuta su `install.sh`. Mientras no haya ninguna versión publicada, descarga la rama `main` y la compila, lo que necesita Rust (`cargo`) en el visor. Necesita `curl`, `sha256sum` y `tar`. Las opciones que van después de `sh -s --` se pasan a `install.sh`.

`install.sh` solo escribe dentro del directorio personal y no usa `sudo`:

- binario `~/.local/bin/frame-audio-mixer`
- unidades de usuario `frame-audio-mixer.service` y `frame-audio-mixer-engine.service`
- script de WirePlumber `~/.local/share/wireplumber/scripts/frame-audio-mixer.lua` y `~/.config/wireplumber/wireplumber.conf.d/91-frame-audio-mixer.conf` (componente `custom.frame-audio-mixer`)
- entrada de escritorio e iconos en `~/.local/share`
- ajustes en `~/.config/frame-audio-mixer/` y nombres de canal en `~/.config/pipewire/frame-audio-mixer/<id>.name`

Mientras funciona, la app añade nodos de PipeWire llamados `frame_amix_*`, los ajustes de WirePlumber `frame-audio-mixer.enabled`, `frame-audio-mixer.vrlink-gain` y `frame-audio-mixer.mic-source`, y el overlay del dashboard `marcus-universe.frame-audio-mixer`.

`install.sh` nunca reinicia PipeWire ni WirePlumber. Un script de WirePlumber nuevo o modificado se carga la próxima vez que WirePlumber arranca. Reinicia el visor solo cuando no estés en una sesión de VR: reiniciar PipeWire o WirePlumber mientras juegas corta el audio de SteamVR y de Steam Link. Los canales ya funcionan antes, porque el panel también fija sus flujos mientras está en marcha.

Si un experimento anterior dejó unidades `frame-mic-mix`, `install.sh` las quita. El único valor `frame-mic.*` que toca es un `frame-mic.mix` que haya quedado. No sobrescribe `frame-mic-tracker.lua` ni `90-frame-mic.conf` de Frame Mic Tuner.

Desde un checkout de git, `./install.sh` compila con `cargo` e instala. `./install.sh --autostart` también arranca el panel con SteamVR. Las opciones que usaste se guardan en `~/.config/frame-audio-mixer/install-args`, así que una actualización instala de la misma forma.

## Canales

- Hasta 32 canales, de `ch1` a `ch32`. Añadir o quitar uno solo crea o destruye los nodos de ese canal en `frame-audio-mixer-engine.service`.
- Un nombre es una línea, de 32 caracteres como máximo, y es único sin distinguir mayúsculas.
- Un canal creado a partir de una app en ejecución lleva el nombre de la app, fija la app a él y muestra el icono de la app de su entrada `.desktop`. Sin icono, muestra la primera letra.
- Cuando la fila es más ancha que el panel, desplázala con el stick del mando o arrastra la barra bajo los canales.
- Cada tira tiene un fader, desde el silencio total hasta +12 dB, con un tope en 0 dB. Los botones de silencio y solo están debajo. Los botones de ruta están apilados junto al fader: A1 a A4 (lo que oyes), PC (el micrófono de streaming del PC), B1 a B4 (micrófonos virtuales) y luego FX1 y FX2. Con 6 u 8 salidas, los botones A y B se reparten en dos columnas. En la vista de lista quedan en una fila junto a silencio y solo.

Un stream que cae en un canal al que no está anclado se devuelve: a su propio canal, o fuera del mezclador para que suene en la salida normal. Los anclajes comparan el binario de la app cuando se conoce — dos apps con el mismo nombre (las apps Electron se presentan todas como «Chromium») no pueden quitarse el canal.

## Proyectos

El botón de carpeta de la cabecera guarda configuraciones de enrutado. Un proyecto almacena canales, anclajes, rutas, cadenas de efectos y niveles en un archivo JSON bajo `~/.config/frame-audio-mixer/projects/`. Cargarlo reemplaza la configuración actual; las preferencias de pantalla (idioma, tema, diseño) se mantienen.

## Salidas

El micrófono de streaming del PC siempre está ahí. Ajustes añade 2, 4, 6 u 8 salidas más: 2 son A1 y B1, 8 son A1 a A4 y B1 a B4. Un número menor conserva las rutas guardadas y no las conecta.

- PC es `frame_amix_source`, que se muestra como **Frame Mix**. VRLink lo graba, así que el PC ve Steam Streaming Microphone.
- A1 son los altavoces del visor. A2 a A4 pueden ser cualquier otra salida, como Bluetooth.
- B1 a B4 son micrófonos virtuales para las apps del visor (`frame_amix_b1` a `frame_amix_b4`). Cada uno tiene nombre.

El cajón Salidas (icono de altavoz en la barra derecha) tiene la prueba de mezcla arriba y luego un contenedor por salida: su nombre, el fader maestro debajo y el silencio a la derecha. PC va primero.

Los ajustes de la versión 2 se trasladan en la primera carga: el antiguo B1 pasa a ser PC (con su fader y su silencio), y los antiguos B2 a B4 pasan a ser B1 a B3.

## Flujo de señal

La página Flujo de señal (junto a «Apps y actualizaciones» en la cabecera) dibuja toda la mezcla como un grafo por carriles: fuentes, canales del mezclador, salidas y los dispositivos a los que llegan. Las conexiones salen del enrutado guardado, y cada una se comprueba contra los enlaces PipeWire reales del motor: una ruta cuyo enlace desapareció se dibuja como línea roja discontinua. La señal audible ensancha un enlace vivo con un brillo del color del nivel. Arrastrar desplaza, el stick hace scroll, y al seleccionar un nodo se ve el nodo PipeWire que hay detrás. El botón «Otras apps» muestra además cada app que no está en ningún canal, con el dispositivo en el que realmente suena.

## Micrófono

La elección del micrófono solo afecta a la mezcla. Nada en esta app llama a `wpctl set-default`, así que el predeterminado del sistema que usa Frame Mic Tuner se queda como está.

- **Predeterminado del sistema** sigue `default.audio.source`.
- **Micrófono del Frame** es `alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source`, el final de la cadena de Valve después de la reducción de ruido, la cancelación de eco y el EQ del micrófono.
- Cualquier otra fuente de la lista se guarda por su `node.name`.

La elección se guarda como `frame-audio-mixer.mic-source` (`""`, `builtin` o un nombre de nodo). El script resuelve `builtin` al loopback de arriba, vuelve al predeterminado del sistema si falta ese nodo y nunca usa `frame_amix_source`. Un valor nuevo tiene efecto después del próximo arranque de WirePlumber.

El motor captura esa fuente con un loopback que no es virtual, así que la reducción de ruido de Valve sigue activa mientras Mix está encendido. La tira del micrófono añade 12 dB de ganancia con el fader en 0 dB. Su ruta predeterminada es solo PC.

## Medidores de nivel

Los medidores solo funcionan mientras el dashboard está visible, la mezcla está activa y los medidores están activados en Ajustes (el engranaje junto al menú de idioma). Solo se miden los canales que ves en pantalla. Cada medidor es una toma `pw-record` con `node.virtual=true`, así que no activa los filtros de micrófono de Valve. Las tomas de los canales son pasivas. La del micrófono no. El panel se redibuja a unos 30 fps mientras funcionan los medidores y no dibuja nada mientras el dashboard está oculto.

## Efectos

FX1 y FX2 son inserciones, y cada uno tiene una cadena de hasta tres efectos. EQ es uno de ellos: un ecualizador paramétrico de 3, 6, 8 o 12 puntos, que se abre con Editar en su ranura. Los puntos extremos pueden ser campana, shelf, o paso alto (punto bajo) o paso bajo (punto alto). El pitcher ajusta el tono y los formantes por separado, ±12 semitonos cada uno, y además el brillo. Con los formantes intactos, una voz desplazada suena como otra persona y no como una grabación acelerada; si los formantes siguen al tono, sale el clásico efecto ardilla. Sus presets incluyen hombre a mujer, hombre a chica, mujer a hombre y chica a hombre. Una tira que activa un bus procesa su propia copia de esa cadena, FX1 antes que FX2, antes de su fader. El resultado sustituye a la señal de la tira en las rutas de esa tira; no hay ningún nodo de efectos en el grafo ni se mezcla una copia en seco. Cada bus tiene un fader de nivel, aplicado después de su cadena. Un plugin VST tiene un solo host y un solo flujo, así que procesa la primera tira que usa su bus (el micrófono y después los canales en orden) y queda en bypass en las demás. El procesamiento es float32 a 48 kHz y 480 frames dentro del motor. Activar o desactivar un efecto no reinicia PipeWire ni WirePlumber.

## Apps y actualizaciones

**Apps y actualizaciones** muestra esta app, Frame Mic Tuner como app relacionada y las apps de Frame del catálogo de sasaken. **Instalar** abre Konsole con `curl -fsSL https://frame.sasaken1102s.net | sh`. Un Frame Mic Tuner que ya está en marcha no tiene botón Instalar, así que esta página no reemplaza un fork suyo.

El actualizador está escrito en Rust (`ureq` con rustls, `sha2`, `flate2` y `tar`):

- Una comprobación lee la última versión publicada en GitHub, compara versiones y busca `frame-audio-mixer-<version>.tar.gz` y `SHA256SUMS`.
- La instalación copia el binario en marcha a `~/.cache/frame-audio-mixer/update/` y lo arranca con `systemd-run --user --unit=frame-audio-mixer-update --collect`.
- Las descargas son solo por HTTPS, desde `github.com`, `api.github.com`, `codeload.github.com` y `*.githubusercontent.com`, salvo que esté definido `FRAME_UPDATE_ALLOW_INSECURE=1`. Se comprueban las redirecciones, una descarga se detiene en 256 MB y el SHA-256 debe coincidir.
- El archivo se rechaza si tiene una ruta absoluta, `..`, un enlace o un nodo de dispositivo.
- Los archivos de caché están en `~/.cache/frame-audio-mixer/` (`update-check.json`, `update-state.json`, `update.log`, `update.lock/`, `update/`).

## Desinstalación

Con el comando de una línea:

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/i | sh -s -- --uninstall
```

Desde un checkout o una carpeta de versión:

```bash
./install.sh --uninstall
./install.sh --uninstall --purge
```

Sin `--purge`, los ajustes quedan en `~/.config/frame-audio-mixer`. `--purge` también los borra, junto con los valores guardados `frame-audio-mixer.*`. El script de WirePlumber se quita del disco, pero sigue cargado hasta que WirePlumber vuelva a arrancar.

## Compilación

En el visor, `rustc` y `cargo` están en `~/.cargo/bin`. Las bibliotecas del sistema que se usan son cairo, freetype, Vulkan, PipeWire y libclang.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release
./target/release/frame-audio-mixer --self-test
./target/release/frame-audio-mixer --language en --dump-png /tmp/frame-audio-mixer.png
./scripts/package.sh
```

`--dump-png` escribe la vista principal y las demás vistas (Apps, Añadir, selector de canal, ajustes, menú de idioma, una fila desplazada mientras graba) junto a la ruta indicada.

`scripts/package.sh` escribe `dist/frame-audio-mixer-<version>.tar.gz`, con los mismos bytes que `dist/frame-audio-mixer-aarch64.tar.gz`, y `dist/SHA256SUMS` con los dos. Sube los tres a una versión publicada de GitHub, y la instalación de una línea y el actualizador de la app la usarán.
