[English](install-ssh.md) · [日本語](install-ssh.ja.md) · [简体中文](install-ssh.sc.md) · [Deutsch](install-ssh.de.md) · **Français** · [Español](install-ssh.es.md) · [Italiano](install-ssh.it.md)

[← README](../README.fr.md)

# Installation par SSH

Installez Frame Audio Mixer depuis un PC. Vous vous connectez au Steam Frame et lancez la même commande que dans Konsole. Le PC et le casque doivent être sur le même réseau.

## Activer SSH

1. Sur le Frame, ouvrez les paramètres Steam, puis Système, puis **Activer le mode développeur**.
2. Sous Développeur, définissez un mot de passe. C'est l'identifiant SSH. Choisissez-en un robuste : avec SSH, toute personne de votre réseau qui le connaît peut se connecter au casque.
3. Notez l'adresse IP du casque. Elle apparaît dans les réglages développeur, ou sur votre routeur.

Le nom d'utilisateur est `steamos`.

## Se connecter

Windows 10 et 11, macOS et Linux incluent un client OpenSSH. Dans un terminal sur le PC :

```sh
ssh steamos@<headset-ip>
```

Remplacez `<headset-ip>` par l'adresse notée plus haut. La première fois, confirmez la clé de l'hôte, puis saisissez le mot de passe développeur.

## Installer

Dans cette session, lancez :

```sh
curl -fsSL https://raw.githubusercontent.com/marcus-universe/frame-audio-mixer/main/scripts/get.sh | sh
```

- Pour démarrer aussi le panneau avec SteamVR, terminez la commande par `| sh -s -- --autostart`.
- Pour mettre à jour, relancez la même commande. Pour désinstaller, terminez-la par `| sh -s -- --uninstall`.
- L'installation n'utilise pas `sudo` et n'écrit que dans le répertoire personnel du casque. Elle ne redémarre jamais PipeWire ni WirePlumber, donc une session VR en cours garde son son.

Vous pouvez aussi envoyer la commande en une ligne, sans rester dans la session. Sous macOS, Linux ou Windows Terminal :

```sh
ssh steamos@<headset-ip> "curl -fsSL https://raw.githubusercontent.com/marcus-universe/frame-audio-mixer/main/scripts/get.sh | sh"
```

Le programme d'installation ne s'exécute que sur le Steam Frame. Une session SSH est déjà sur le casque, donc cette vérification réussit.

Ouvrez ensuite le tableau de bord SteamVR et choisissez **Frame Audio Mixer**.
