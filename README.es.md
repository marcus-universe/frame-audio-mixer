<p align="center">
  <img src="assets/logo/logo_banner_blue.svg" alt="Frame Audio Mixer" width="640">
</p>
<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/github/license/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="License"></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/version-1.1.0-33C4FF?style=flat-square" alt="Version 1.1.0"></a>
  <a href="https://frame-audio-mixer.marcus-universe.com/"><img src="https://img.shields.io/badge/Website-33C4FF?style=flat-square" alt="Website"></a>
  <a href="https://github.com/marcus-universe/frame-audio-mixer/stargazers"><img src="https://img.shields.io/github/stars/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="GitHub stars"></a>
</p>

[English](README.md) · [日本語](README.ja.md) · [简体中文](README.sc.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · **Español** · [Italiano](README.it.md)

# Frame Audio Mixer

Un panel del dashboard de SteamVR para Steam Frame. Mezcla el audio de las apps con el micrófono del visor y envía esa mezcla al PC por el flujo inalámbrico, donde aparece como **Steam Streaming Microphone**.

Va después de [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner), que sigue encargándose de la cancelación de eco y la reducción de ruido.

## Instalación

En Konsole en el Frame (**+** en la barra de abajo, luego la lista de programas, luego **Konsole**), ejecuta:

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/i -o frame-audio-mixer-install && sh frame-audio-mixer-install
```

- Primero activa el modo de desarrollador: Ajustes de Steam, luego Sistema, luego **Activar modo de desarrollador**. Konsole no aparece en la lista hasta que lo hagas.
- Para que el panel también arranque con SteamVR, añade `--autostart` al final.
- Para actualizar, vuelve a ejecutar el mismo comando. Para desinstalar, añade `--uninstall` al final.
- No usa `sudo` y solo escribe en tu directorio personal. Nunca reinicia PipeWire ni WirePlumber, así que una sesión de VR en marcha conserva el sonido.
- Desde un PC: [Instalar por SSH](docs/install-ssh.es.md).

## Funciones

- **Dispositivos de audio virtuales**: hasta 32 canales con nombre. Cada uno es una salida (`Frame Mix: <name>`) que cualquier app puede elegir.
- **Canales de apps**: crea un canal a partir de una app en ejecución. Su icono aparece junto al nombre.
- **Mezcla del micrófono de streaming del PC**: el PC oye tu micrófono más los canales.
- **Dos faders por canal**: uno para el visor y otro para el PC, cada uno con su propio silencio. Los faders se ajustan a 20, 50, 80 y 100 %.
- **Micrófono**: elige el micrófono de la mezcla. El micrófono predeterminado del sistema no cambia.
- **Medidores** en cada fader. Se pueden desactivar.
- **Prueba de mezcla**: graba hasta 10 segundos de lo que oye el PC y reprodúcelo.
- Siete idiomas y un tema de color azul o rosa.

<table>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-1.png" alt="Vista de columnas, tema azul" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-2.png" alt="Vista de lista, tema azul" height="320"></td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-3.png" alt="Vista de columnas, tema rosa" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-4.png" alt="Vista de lista, tema rosa" height="320"></td>
  </tr>
</table>

## Uso

1. Abre el dashboard de SteamVR y elige **Frame Audio Mixer**.
2. Activa la **mezcla**.
3. Pulsa **+** para añadir un canal: una sugerencia, un nombre propio o una app en ejecución.
4. En esa app, elige la salida `Frame Mix: <name>`.
5. El fader del altavoz marca el volumen del canal en tu visor. El fader del micrófono marca cuánto oye el PC de él.

Si usas los altavoces del Frame, activa la cancelación de eco en Frame Mic Tuner. Si no, el PC puede oír el audio de la app dos veces.

## Documentación

- [Cómo funciona](docs/how-it-works.es.md): el grafo de audio y cómo se enrutan VRLink y las apps.
- [Detalles técnicos](docs/technical.es.md): qué se instala, detalles del micrófono y de los medidores, Apps y actualizaciones, desinstalación, compilación.

## Créditos

Frame Audio Mixer se inspira en [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner) de [sasaken1102r](https://github.com/sasaken1102r). Más apps de Steam Frame de sasaken@ están en [frame.sasaken1102s.net](https://frame.sasaken1102s.net/).

El panel, el overlay de OpenVR y la lista de apps están adaptados de ese proyecto (MIT). Consulta [`NOTICE`](NOTICE).

- [Iconos y sus licencias](docs/icons.es.md)

## Licencia

[MIT](LICENSE)
