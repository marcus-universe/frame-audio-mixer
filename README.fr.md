<p align="center">
  <a href="https://frame-audio-mixer.marcus-universe.com/"><img src="assets/logo/logo_banner_blue.svg" alt="Frame Audio Mixer" width="640"></a>
</p>
<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/github/license/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="License"></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/version-2.0.0-33C4FF?style=flat-square" alt="Version 2.0.0"></a>
  <a href="https://frame-audio-mixer.marcus-universe.com/"><img src="https://img.shields.io/badge/Website-33C4FF?style=flat-square" alt="Website"></a>
  <a href="https://github.com/marcus-universe/frame-audio-mixer/stargazers"><img src="https://img.shields.io/github/stars/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="GitHub stars"></a>
</p>

[English](README.md) · [日本語](README.ja.md) · [简体中文](README.sc.md) · [Deutsch](README.de.md) · **Français** · [Español](README.es.md) · [Italiano](README.it.md)

# 🎚️ Frame Audio Mixer

Frame Audio Mixer mélange le son des applis avec le micro du casque et envoie ce mix au PC par le flux sans fil, dans le **Steam Streaming Microphone** que SteamVR fournit déjà.

Il se place après [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner), qui gère toujours l'annulation d'écho et la réduction de bruit.

## 📥 Installation

Dans Konsole sur le Frame (**+** dans la barre en bas, puis la liste des programmes, puis **Konsole**), lancez :

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/i | sh
```

- 🛠️ Activez d'abord le mode développeur : Paramètres Steam, puis Système, puis **Activer le mode développeur**. Konsole n'apparaît pas dans la liste avant cela.
- 💻 Depuis un PC : [Installation par SSH](docs/install-ssh.fr.md).

## ✨ Fonctionnalités

- 🔄 **Orientation** : les entrées et les sorties passent chacune des faders verticaux aux rangées.
- 🎛️ **Bus d'effets** : FX1 et FX2, avec réverbération, compresseur, EQ, pitch, flanger et phaser. Chaque tranche peut envoyer vers l'un ou l'autre.
- 🎤 **Micros virtuels** : B1 à B4, nommés, en plus du micro de streaming du PC.
- 🎧 **Haut-parleurs, Bluetooth et USB** : le mix sort sur les haut-parleurs du Frame, un casque Bluetooth ou l'audio USB tant que l'un est connecté.
- 🧩 **Canaux logiciels** : jusqu'à 32 canaux nommés. Chacun est une sortie que n'importe quelle appli peut choisir.
- 🖥️ **Vue large** : une mise en page 21:9. On peut tirer la séparation entre les deux parties. Chaque côté s'arrête à un cinquième de la fenêtre.
- 📊 **Niveaux, sourdine et accroche** : la même échelle sur chaque fader, de +6 à −48, un bouton de sourdine sous chacun, et des faders de sortie qui s'accrochent aux repères.
- ⏺️ **Contrôle du mix** : en haut des sorties. Choisir une sortie, en enregistrer jusqu'à 10 secondes, puis lire ou mettre en pause.
- 🗺️ **Flux du signal** : une carte en direct du routage complet. Chaque route est vérifiée contre les liens PipeWire réels ; un lien rompu apparaît en pointillés rouges. Glisser pour déplacer, sélectionner un nœud pour son identité PipeWire.
- 🗂️ **Projets** : enregistrez les canaux, le routage, les effets et les niveaux sous un nom, et rechargez une configuration à tout moment.
- 🌐 Sept langues, et un thème bleu ou rose.

<table>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-1.jpg" alt="Vue large, thème bleu" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-2.jpg" alt="Entrées en rangées, thème rose" height="320"></td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-3.jpg" alt="Bus d'effets, thème rose" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-4.jpg" alt="Tableau de bord, thème bleu" height="320"></td>
  </tr>
</table>

## ▶️ Utilisation

1. Ouvrez le tableau de bord SteamVR et choisissez **Frame Audio Mixer**.
2. Activez le mix. Le PC l'entend comme **Steam Streaming Microphone**.
3. Appuyez sur **+** et ajoutez un canal. Donnez-lui un nom, ou choisissez une appli déjà lancée.
4. Dans cette appli, choisissez la sortie qui porte le nom du canal.
5. **Entrées**, à gauche, est ce qui entre dans le mix. **Sorties**, à droite, est la destination : le PC, les haut-parleurs du Frame, le Bluetooth, l'USB, ou un micro virtuel.

🎧 Si vous écoutez sur les haut-parleurs du Frame, activez l'annulation d'écho dans Frame Mic Tuner. Sinon le PC peut entendre ce son deux fois.

## 📚 Documentation

- 📖 [Fonctionnement](docs/how-it-works.fr.md) : le graphe audio, et le routage de VRLink et des applis.
- 🔧 [Détails techniques](docs/technical.fr.md) : ce qui est installé, détails sur le micro et les indicateurs de niveau, applis et mises à jour, désinstallation, compilation.

## 🙏 Crédits

Frame Audio Mixer s'inspire de [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner) de [sasaken1102r](https://github.com/sasaken1102r). D'autres applis Steam Frame de sasaken@ sont sur [frame.sasaken1102s.net](https://frame.sasaken1102s.net/).

Le panneau, l'overlay OpenVR et la liste des applis sont adaptés de ce projet (MIT). Voir [`NOTICE`](NOTICE).

- 🎨 [Icônes et leurs licences](docs/icons.fr.md)

## 📄 Licence

[MIT](LICENSE)
