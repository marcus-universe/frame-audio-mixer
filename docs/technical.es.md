[English](technical.md) · [日本語](technical.ja.md) · [简体中文](technical.sc.md) · [Deutsch](technical.de.md) · [Français](technical.fr.md) · **Español** · [Italiano](technical.it.md)

[← README](../README.es.md)

# Detalles técnicos

## Qué se instala

La instalación de una línea ejecuta [`scripts/get.sh`](../scripts/get.sh). Descarga la última versión publicada (`frame-audio-mixer-aarch64.tar.gz`), la comprueba con `SHA256SUMS` y ejecuta su `install.sh`. Mientras no haya ninguna versión publicada, descarga la rama `main` y la compila, lo que necesita Rust (`cargo`) en el visor. Necesita `curl`, `sha256sum` y `tar`. Las opciones que van después de `sh -s --` se pasan a `install.sh`.

`install.sh` solo escribe dentro del directorio personal y no usa `sudo`:

- binario `~/.local/bin/frame-audio-mixer`
- lanzador `~/.local/lib/frame-audio-mixer/frame-audio-mixer-graph.sh`
- unidades de usuario `frame-audio-mixer.service`, `frame-audio-mixer-graph.service` y `frame-audio-mixer-channel@.service`
- script de WirePlumber `~/.local/share/wireplumber/scripts/frame-audio-mixer.lua` y `~/.config/wireplumber/wireplumber.conf.d/91-frame-audio-mixer.conf` (componente `custom.frame-audio-mixer`)
- entrada de escritorio e iconos en `~/.local/share`
- ajustes en `~/.config/frame-audio-mixer/` y nombres de canal en `~/.config/pipewire/frame-audio-mixer/<id>.name`

Mientras funciona, la app añade nodos de PipeWire llamados `frame_amix_*`, los ajustes de WirePlumber `frame-audio-mixer.enabled`, `frame-audio-mixer.vrlink-gain` y `frame-audio-mixer.mic-source`, y el overlay del dashboard `marcus-universe.frame-audio-mixer`.

`install.sh` nunca reinicia PipeWire ni WirePlumber. Un script de WirePlumber nuevo o modificado se carga la próxima vez que WirePlumber arranca. Reinicia el visor solo cuando no estés en una sesión de VR: reiniciar PipeWire o WirePlumber mientras juegas corta el audio de SteamVR y de Steam Link. Los canales ya funcionan antes, porque el panel también fija sus flujos mientras está en marcha.

Si un experimento anterior dejó unidades `frame-mic-mix`, `install.sh` las quita. El único valor `frame-mic.*` que toca es un `frame-mic.mix` que haya quedado. No sobrescribe `frame-mic-tracker.lua` ni `90-frame-mic.conf` de Frame Mic Tuner.

Desde un checkout de git, `./install.sh` compila con `cargo` e instala. `./install.sh --autostart` también arranca el panel con SteamVR. Las opciones que usaste se guardan en `~/.config/frame-audio-mixer/install-args`, así que una actualización instala de la misma forma.

## Canales

- Hasta 32 canales, de `ch1` a `ch32`. Cada uno es una instancia de systemd con dos procesos `pw-loopback`.
- Un nombre es una línea, de 32 caracteres como máximo, y es único sin distinguir mayúsculas.
- Un canal creado a partir de una app en ejecución lleva el nombre de la app, fija la app a él y muestra el icono de la app de su entrada `.desktop`. Sin icono, muestra la primera letra.
- Cuando la fila es más ancha que el panel, desplázala con el stick del mando o arrastra la barra bajo los canales.
- Los faders van de 0 a 150 %. Al arrastrar, se ajustan a 20, 50, 80 y 100 % cuando quedan a menos de 8 píxeles. Los puntos de un fader marcan 20, 50 y 80 %. La línea marca 100 %.

## Micrófono

La elección del micrófono solo afecta a la mezcla. Nada en esta app llama a `wpctl set-default`, así que el predeterminado del sistema que usa Frame Mic Tuner se queda como está.

- **Predeterminado del sistema** sigue `default.audio.source`.
- **Micrófono del Frame** es `alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source`, el final de la cadena de Valve después de la reducción de ruido, la cancelación de eco y el EQ del micrófono.
- Cualquier otra fuente de la lista se guarda por su `node.name`.

La elección se guarda como `frame-audio-mixer.mic-source` (`""`, `builtin` o un nombre de nodo). El script resuelve `builtin` al loopback de arriba, vuelve al predeterminado del sistema si falta ese nodo y nunca usa `frame_amix_source`. Un valor nuevo tiene efecto después del próximo arranque de WirePlumber.

## Medidores de nivel

Los medidores solo funcionan mientras el dashboard está visible, la mezcla está activa y los medidores están activados en Ajustes (el engranaje junto al menú de idioma). Solo se miden los canales que ves en pantalla. Cada medidor es una toma `pw-record` con `node.virtual=true`, así que no activa los filtros de micrófono de Valve. Las tomas de los canales son pasivas. La del micrófono no. El panel se redibuja a unos 30 fps mientras funcionan los medidores y no dibuja nada mientras el dashboard está oculto.

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
curl -fsSL https://frame-audio-mixer.marcus-universe.com/frame-audio-mixer-install | sh -s -- --uninstall
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
