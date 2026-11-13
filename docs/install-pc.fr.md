[English](install-pc.md) · [日本語](install-pc.ja.md) · [简体中文](install-pc.sc.md) · [Deutsch](install-pc.de.md) · **Français** · [Español](install-pc.es.md) · [Italiano](install-pc.it.md)

[← README](../README.fr.md)

# Installer les périphériques audio du PC

**Frame Audio Link** est un module SteamVR pour le PC depuis lequel vous streamez. Il ajoute huit périphériques audio au PC :

| Sur le PC | Ce que c'est |
|---|---|
| **Steam Streaming Mic B1** … **B4** | Des micros. Ils transportent B1–B4 du mixeur du casque. |
| **Steam Streaming Out PC1** … **PC4** | Des sorties. Ce qui y joue peut être ajouté dans le mixeur comme canal PC1–PC4. |

Le **Steam Streaming Microphone** et les **Steam Streaming Speakers** de Steam restent tels quels.

## Avant de commencer

- Frame Audio Mixer 2.2 ou plus récent sur le casque (voir [Installation](../README.fr.md#-installation)).
- SteamVR sur le PC, et le streaming sans fil vers le Steam Frame qui fonctionne.

## Windows

1. Téléchargez `frame-audio-link-windows-x86_64.zip` depuis la [dernière version](https://github.com/marcus-universe/frame-audio-mixer/releases/latest) et décompressez-le.
2. Double-cliquez sur **install.cmd**.
3. Windows demande une fois les droits d'administrateur. Cette étape crée les huit périphériques.

Ou dans PowerShell :

```powershell
irm https://frame-audio-mixer.marcus-universe.com/get-windows | iex
```

Le programme va dans `%LOCALAPPDATA%\Programs\FrameAudioLink`.

- Les périphériques sont d'autres instances des deux pilotes audio que Steam a déjà installés. Aucun nouveau pilote n'est installé.
- Steam installe ses pilotes la première fois que Remote Play ou le streaming VR sans fil est utilisé. Si l'installateur dit qu'ils manquent, streamez une fois et relancez-le.
- Si Windows fait d'un nouveau périphérique le périphérique par défaut, l'installateur remet votre ancien périphérique par défaut.

Les périphériques restent installés, même quand SteamVR est fermé.

## Linux

Fonctionne sous Ubuntu 22.10 ou plus récent, Fedora et Arch. PipeWire doit être le serveur son. Vérifiez avec :

```sh
pactl info | grep "Server Name"
```

Vous devez voir `PulseAudio (on PipeWire …)`. Ubuntu 22.04 utilise encore PulseAudio et n'est pas pris en charge. Sous Arch, installez d'abord `pipewire-pulse` et `wireplumber` s'ils manquent.

Installez en tant qu'utilisateur normal, sans `sudo` :

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/get-linux | sh
```

Le programme va dans `~/.local/bin/frame-audio-link`. Sous Linux, les huit périphériques sont des nœuds PipeWire : ils existent tant que le module tourne, donc tant que SteamVR tourne. Rien n'est installé dans le système son.

## L'activer dans SteamVR

L'installateur enregistre le module comme application de démarrage. Vérifiez qu'il est activé : **SteamVR → Paramètres → Démarrage / Arrêt → Choisir les applications d'overlay au démarrage → Frame Audio Link**.

Le tableau de bord a alors un onglet **Audio Link**. Il indique si le casque est trouvé et a un interrupteur pour chacun des huit flux. Un flux coupé n'utilise pas de bande passante.

## Utilisation

- **Du casque vers le PC** : dans le mixeur, envoyez le micro ou un canal vers B1–B4. Sur le PC, choisissez **Steam Streaming Mic B1** … **B4** comme micro dans n'importe quelle appli (Discord, OBS, un jeu).
- **Du PC vers le casque** : sur le PC, faites jouer une appli sur **Steam Streaming Out PC1** … **PC4**. Dans le mixeur, appuyez sur **+** et choisissez **PC1** … **PC4** sous « En cours ».

## Vérifier

- **Windows** : les huit périphériques apparaissent dans le panneau Son (`mmsys.cpl`). `frame-audio-link devices status` dans un terminal les liste aussi.
- **Linux** : tant que le module tourne, `pactl list short sources` et `pactl list short sinks` affichent `frame_link_b1` … et `frame_link_pc1` ….
- **L'overlay indique « Recherche »** : lancez le streaming sans fil sur le Steam Frame et ouvrez-y Frame Audio Mixer. Le casque ne répond qu'au PC depuis lequel il streame.
- **L'overlay indique qu'un périphérique n'est pas installé** : sous Windows, relancez `install.cmd`.

Le journal se trouve dans `%APPDATA%\frame-audio-link\frame-audio-link.log` sous Windows et dans `~/.config/frame-audio-link/frame-audio-link.log` sous Linux.

## Désinstaller

- **Windows** : double-cliquez sur **uninstall.cmd** de l'archive. Il supprime les huit périphériques, l'entrée SteamVR et le programme.
- **Linux** : lancez `./install-linux.sh --uninstall` depuis l'archive.

Le fonctionnement du lien est décrit dans [PC audio link](pc-link.md) (en anglais).
