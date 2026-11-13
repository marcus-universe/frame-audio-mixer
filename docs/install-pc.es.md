[English](install-pc.md) · [日本語](install-pc.ja.md) · [简体中文](install-pc.sc.md) · [Deutsch](install-pc.de.md) · [Français](install-pc.fr.md) · **Español** · [Italiano](install-pc.it.md)

[← README](../README.es.md)

# Instalar los dispositivos de audio del PC

**Frame Audio Link** es un complemento de SteamVR para el PC desde el que haces streaming. Añade ocho dispositivos de audio al PC:

| En el PC | Qué es |
|---|---|
| **Steam Streaming Mic B1** … **B4** | Micrófonos. Llevan B1–B4 del mezclador del visor. |
| **Steam Streaming Out PC1** … **PC4** | Salidas. Lo que suena en ellas se puede añadir en el mezclador como canal PC1–PC4. |

El **Steam Streaming Microphone** y los **Steam Streaming Speakers** de Steam siguen como están.

## Antes de empezar

- Frame Audio Mixer 2.2 o posterior en el visor (ver [Instalación](../README.es.md#-instalación)).
- SteamVR en el PC, y el streaming inalámbrico al Steam Frame funcionando.

## Windows

1. Descarga `frame-audio-link-windows-x86_64.zip` de la [última versión](https://github.com/marcus-universe/frame-audio-mixer/releases/latest) y descomprímelo.
2. Haz doble clic en **install.cmd**.
3. Windows pide una vez permisos de administrador. Ese paso crea los ocho dispositivos.

O en PowerShell:

```powershell
irm https://frame-audio-mixer.marcus-universe.com/get-windows | iex
```

El programa se instala en `%LOCALAPPDATA%\Programs\FrameAudioLink`.

- Los dispositivos son más instancias de los dos controladores de audio que Steam ya instaló. No se instala ningún controlador nuevo.
- Steam instala sus controladores la primera vez que se usa Remote Play o el streaming de VR inalámbrico. Si el instalador dice que faltan, haz streaming una vez y vuelve a ejecutarlo.
- Si Windows convierte uno de los nuevos dispositivos en el predeterminado, el instalador vuelve a poner el que tenías.

Los dispositivos quedan instalados, también con SteamVR cerrado.

## Linux

Funciona en Ubuntu 22.10 o posterior, Fedora y Arch. PipeWire debe ser el servidor de sonido. Compruébalo con:

```sh
pactl info | grep "Server Name"
```

Debería decir `PulseAudio (on PipeWire …)`. Ubuntu 22.04 todavía usa PulseAudio y no es compatible. En Arch, instala antes `pipewire-pulse` y `wireplumber` si faltan.

Instala como usuario normal, sin `sudo`:

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/get-linux | sh
```

El programa se instala en `~/.local/bin/frame-audio-link`. En Linux los ocho dispositivos son nodos de PipeWire: existen mientras el complemento está en marcha, es decir, mientras SteamVR está en marcha. No se instala nada en el sistema de sonido.

## Activarlo en SteamVR

El instalador registra el complemento como aplicación de inicio. Comprueba que está activado: **SteamVR → Ajustes → Inicio / Apagado → Elegir aplicaciones de superposición al inicio → Frame Audio Link**.

El panel tiene entonces una pestaña **Audio Link**. Muestra si se encontró el visor y tiene un interruptor para cada uno de los ocho flujos. Un flujo apagado no usa ancho de banda.

## Uso

- **Del visor al PC**: en el mezclador, envía el micrófono o un canal a B1–B4. En el PC, elige **Steam Streaming Mic B1** … **B4** como micrófono en cualquier app (Discord, OBS, un juego).
- **Del PC al visor**: en el PC, haz que una app suene en **Steam Streaming Out PC1** … **PC4**. En el mezclador, pulsa **+** y elige **PC1** … **PC4** en «En ejecución».

## Comprobar

- **Windows**: los ocho dispositivos aparecen en el panel de Sonido (`mmsys.cpl`). `frame-audio-link devices status` en una terminal también los lista.
- **Linux**: mientras el complemento está en marcha, `pactl list short sources` y `pactl list short sinks` muestran `frame_link_b1` … y `frame_link_pc1` ….
- **La superposición dice «Buscando»**: inicia el streaming inalámbrico en el Steam Frame y abre allí Frame Audio Mixer. El visor solo responde al PC desde el que hace streaming.
- **La superposición dice que falta un dispositivo**: en Windows, ejecuta de nuevo `install.cmd`.

El registro está en `%APPDATA%\frame-audio-link\frame-audio-link.log` en Windows y en `~/.config/frame-audio-link/frame-audio-link.log` en Linux.

## Desinstalar

- **Windows**: haz doble clic en **uninstall.cmd** del archivo. Quita los ocho dispositivos, la entrada de SteamVR y el programa.
- **Linux**: ejecuta `./install-linux.sh --uninstall` desde el archivo.

Cómo funciona el enlace está en [PC audio link](pc-link.md) (en inglés).
