[English](install-ssh.md) · [日本語](install-ssh.ja.md) · [简体中文](install-ssh.sc.md) · [Deutsch](install-ssh.de.md) · [Français](install-ssh.fr.md) · **Español** · [Italiano](install-ssh.it.md)

[← README](../README.es.md)

# Instalar por SSH

Instala Frame Audio Mixer desde un PC. Entras en el Steam Frame y ejecutas el mismo comando que en Konsole. El PC y el visor tienen que estar en la misma red.

## Activar SSH

1. En el Frame, abre Ajustes de Steam, luego Sistema, luego **Activar modo de desarrollador**.
2. En Desarrollador, pon una contraseña. Esa contraseña es el acceso SSH. Elige una fuerte: con SSH, cualquiera de tu red que la sepa puede entrar en el visor.
3. Anota la dirección IP del visor. Aparece en los ajustes de desarrollador, o en el router.

El usuario es `steamos`.

## Conectar

Windows 10 y 11, macOS y Linux incluyen un cliente OpenSSH. En un terminal del PC:

```sh
ssh steamos@<headset-ip>
```

Sustituye `<headset-ip>` por la dirección del paso anterior. La primera vez, confirma la clave del host y luego escribe la contraseña de desarrollador.

## Instalar

En esa sesión, ejecuta:

```sh
curl -fsSL https://raw.githubusercontent.com/marcus-universe/frame-audio-mixer/main/scripts/get.sh | sh
```

- Para que el panel también arranque con SteamVR, termina el comando con `| sh -s -- --autostart`.
- Para actualizar, vuelve a ejecutar el mismo comando. Para desinstalar, termínalo con `| sh -s -- --uninstall`.
- No usa `sudo` y solo escribe en el directorio personal del visor. Nunca reinicia PipeWire ni WirePlumber, así que una sesión de VR en marcha conserva el sonido.

También puedes enviar el comando en una línea, sin quedarte en la sesión. En macOS, Linux o Windows Terminal:

```sh
ssh steamos@<headset-ip> "curl -fsSL https://raw.githubusercontent.com/marcus-universe/frame-audio-mixer/main/scripts/get.sh | sh"
```

El instalador solo se ejecuta en el Steam Frame. Una sesión SSH ya está en el visor, así que esa comprobación se cumple.

Después abre el dashboard de SteamVR y elige **Frame Audio Mixer**.
