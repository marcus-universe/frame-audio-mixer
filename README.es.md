<p align="center">
  <img src="assets/logo/logo_banner_blue.svg" alt="Frame Audio Mixer" width="640">
</p>
<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/github/license/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="License"></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/version-2.0.0-33C4FF?style=flat-square" alt="Version 2.0.0"></a>
  <a href="https://frame-audio-mixer.marcus-universe.com/"><img src="https://img.shields.io/badge/Website-33C4FF?style=flat-square" alt="Website"></a>
  <a href="https://github.com/marcus-universe/frame-audio-mixer/stargazers"><img src="https://img.shields.io/github/stars/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="GitHub stars"></a>
</p>

[English](README.md) · [日本語](README.ja.md) · [简体中文](README.sc.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · **Español** · [Italiano](README.it.md)

# 🎚️ Frame Audio Mixer

Frame Audio Mixer mezcla el audio de las apps con el micrófono del visor y envía esa mezcla al PC por el flujo inalámbrico, al **Steam Streaming Microphone** que SteamVR ya ofrece.

Va después de [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner), que sigue encargándose de la cancelación de eco y la reducción de ruido.

## 📥 Instalación

En Konsole en el Frame (**+** en la barra de abajo, luego la lista de programas, luego **Konsole**), ejecuta:

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/i | sh
```

- 🛠️ Primero activa el modo de desarrollador: Ajustes de Steam, luego Sistema, luego **Activar modo de desarrollador**. Konsole no aparece en la lista hasta que lo hagas.
- 🚀 Para que el panel también arranque con SteamVR, cambia `| sh` por `| sh -s -- --autostart`.
- 🔄 Para actualizar, vuelve a ejecutar el mismo comando. Para desinstalar, cambia `| sh` por `| sh -s -- --uninstall`.
- 🏠 No usa `sudo` y solo escribe en tu directorio personal. Nunca reinicia PipeWire ni WirePlumber, así que una sesión de VR en marcha conserva el sonido.
- 💻 Desde un PC: [Instalar por SSH](docs/install-ssh.es.md).

## ✨ Funciones

- 🔄 **Orientación**: las entradas y las salidas cambian cada una entre faders verticales y filas.
- 🎛️ **Buses de efectos**: FX1 y FX2, con reverberación, compresor, EQ, pitch, flanger y phaser. Cada canal puede enviar a cualquiera de los dos.
- 🎤 **Micrófonos virtuales**: B1 a B4, con nombre, además del micrófono de streaming del PC.
- 🎧 **Altavoces, Bluetooth y USB**: la mezcla suena en los altavoces del Frame, en auriculares Bluetooth o en audio USB mientras haya uno conectado.
- 🧩 **Canales de software**: hasta 32 canales con nombre. Cada uno es una salida que cualquier app puede elegir.
- 🖥️ **Vista ancha**: un diseño 21:9. Se puede arrastrar la división entre las dos zonas. Cada lado se detiene en un quinto de la ventana.
- 📊 **Medidores, silencio y ajuste**: la misma escala en cada fader, de +6 a −48, un botón de silencio debajo de cada uno, y faders de salida que se ajustan a las marcas.
- ⏺️ **Prueba de mezcla**: arriba en Salidas. Elige una salida, graba hasta 10 segundos y reproduce o pausa la toma.
- 🌐 Siete idiomas, y un tema azul o rosa.

<table>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-1.jpg" alt="Vista ancha, tema azul" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-2.jpg" alt="Entradas en filas, tema rosa" height="320"></td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-3.jpg" alt="Bus de efectos, tema rosa" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-4.jpg" alt="Panel, tema azul" height="320"></td>
  </tr>
</table>

## ▶️ Uso

1. Abre el dashboard de SteamVR y elige **Frame Audio Mixer**.
2. Activa la mezcla. El PC la oye como **Steam Streaming Microphone**.
3. Pulsa **+** y añade un canal. Ponle un nombre, o elige una app que ya esté en marcha.
4. En esa app, elige la salida con el nombre del canal.
5. **Entradas**, a la izquierda, es lo que entra en la mezcla. **Salidas**, a la derecha, es el destino: el PC, los altavoces del Frame, Bluetooth, USB o un micrófono virtual.

🎧 Si escuchas por los altavoces del Frame, activa la cancelación de eco en Frame Mic Tuner. Si no, el PC puede oír ese sonido dos veces.

## 📚 Documentación

- 📖 [Cómo funciona](docs/how-it-works.es.md): el grafo de audio y cómo se enrutan VRLink y las apps.
- 🔧 [Detalles técnicos](docs/technical.es.md): qué se instala, detalles del micrófono y de los medidores, Apps y actualizaciones, desinstalación, compilación.

## 🙏 Créditos

Frame Audio Mixer se inspira en [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner) de [sasaken1102r](https://github.com/sasaken1102r). Más apps de Steam Frame de sasaken@ están en [frame.sasaken1102s.net](https://frame.sasaken1102s.net/).

El panel, el overlay de OpenVR y la lista de apps están adaptados de ese proyecto (MIT). Consulta [`NOTICE`](NOTICE).

- 🎨 [Iconos y sus licencias](docs/icons.es.md)

## 📄 Licencia

[MIT](LICENSE)
