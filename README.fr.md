# Frame Audio Mixer

Frame Audio Mixer mélange des sorties d'applications nommées avec le microphone du Steam Frame et envoie ce mélange au PC par le flux sans fil. Le PC affiche le flux sous le nom **Steam Streaming Microphone**.

L'application se place après [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner). L'annulation d'écho et la suppression du bruit restent dans Frame Mic Tuner. Ce suivi du microphone n'est pas remplacé.

## Rôle

- Ajoute jusqu'à huit sorties nommées, par exemple la musique ou un jeu.
- Envoie chaque sortie au casque et, avec son propre niveau, dans le mélange du microphone.
- Tant que Mix est actif, seul l'enregistrement VRLink est dirigé vers ce mélange. Les autres applications gardent le microphone par défaut.
- Le graphe reste arrêté tant que Mix n'est pas activé. Le PC entend alors de nouveau le microphone habituel.

## Installation

À lancer sur le casque avec l'utilisateur normal. Pas de `sudo`, et uniquement dans le répertoire personnel.

Depuis un dépôt git ou une archive de version :

```bash
./install.sh
./install.sh --autostart
```

`--autostart` démarre aussi le panneau avec SteamVR. Le graphe de mixage reste arrêté jusqu'à l'activation de Mix dans le panneau.

`install.sh` ne redémarre ni PipeWire ni WirePlumber. Un nouveau script WirePlumber est chargé au prochain démarrage de WirePlumber. Ne le faire que hors session VR. Redémarrer PipeWire ou WirePlumber pendant le jeu coupe le son de SteamVR et de Steam Link.

L'installation ajoute une application distincte :

- binaire `~/.local/bin/frame-audio-mixer`
- unités utilisateur `frame-audio-mixer.service`, `frame-audio-mixer-graph.service` et `frame-audio-mixer-channel@.service`
- nœuds PipeWire nommés `frame_amix_*`
- composant WirePlumber `custom.frame-audio-mixer`
- réglages `frame-audio-mixer.enabled` et `frame-audio-mixer.vrlink-gain`
- clé d'overlay du tableau de bord `marcus-universe.frame-audio-mixer`

Si une ancienne expérience a laissé des unités `frame-mic-mix`, `install.sh` les retire. La seule valeur `frame-mic.*` qu'il modifie est la suppression d'un `frame-mic.mix` restant. Il n'écrase ni `frame-mic-tracker.lua` ni `90-frame-mic.conf`.

## Utilisation

Ouvrir **Frame Audio Mixer** dans le tableau de bord SteamVR. Relancer l'application pendant qu'elle tourne ouvre le même panneau.

1. Activer Mix.
2. Ajouter un canal (8 au maximum). Le nom tient sur une ligne, 32 caractères au plus, et il est unique sans tenir compte de la casse.
3. Dans l'application, choisir la sortie `Frame Mix: <name>`.
4. **Casque** est le volume de ce canal dans le casque. **Vers le micro** est la part entendue par le PC. Le fader du microphone et sa sourdine ne concernent que ce mélange, pas le microphone par défaut du système.

**Contrôle du mix** enregistre **Frame Mic + Mix** pendant 10 secondes au plus, puis le lit sur la sortie actuelle. Le pied de page choisit la langue (日本語, English, 简体中文, Deutsch, Français, Español, Italiano), démarre le panneau avec SteamVR et quitte. **Applications et mises à jour** vérifie les versions de cette application et renvoie vers Frame Mic Tuner.

## Fonctionnement

Le microphone est la source par défaut du moment. Sur le Frame, c'est le micro intégré après les filtres Valve, sauf si un autre microphone est choisi.

Quand Mix est actif, `frame-audio-mixer-graph.service` construit le cœur :

- `frame_amix_from_mic` capture la source par défaut.
- `frame_amix_mic_virtual_sink` est le fader du microphone vers `frame_amix_bus`.
- `frame_amix_source` (**Frame Mic + Mix**) est la source enregistrée par VRLink.

Chaque canal est son propre service `frame-audio-mixer-channel@chN.service` :

- `frame_amix_chN` est le sink choisi par les applications (`Frame Mix: <name>`).
- `frame_amix_chN_virtual_sink_playback` joue ce sink dans le casque. C'est le fader casque.
- `frame_amix_chN_virtual_sink_to_bus` envoie le même sink dans le bus de mixage. C'est le fader vers le micro.

Ces noms de lecture internes contiennent `virtual` et `sink`. Le spatialiseur ne les enveloppe donc pas et n'envoie pas le mix une seconde fois vers les haut-parleurs. Le sink du canal n'est ni `alsa_output` ni `bluez_output`, donc il n'apparaît pas dans la liste des sorties de Frame Mic Tuner.

Le composant WirePlumber `custom.frame-audio-mixer` épingle la capture VRLink sur `frame_amix_source` par `object.serial` tant que `frame-audio-mixer.enabled` est vrai. Un nom de nœud dans les métadonnées ne déplace pas le flux. Si la source manque, VRLink garde la source par défaut. Le script ne change ni `frame-mic.echo-cancel` ni `frame-mic.noise-suppression`.

Le panneau épingle une application de la même façon, avec l'`object.serial` du sink. Le serial change au redémarrage du canal, et l'épingle est réécrite. Cette application n'appelle pas `wpctl set-default`.

`frame-audio-mixer.vrlink-gain` vaut `1.0` par défaut. À cette valeur, l'application ne change pas le volume de VRLink. Tout autre gain est enregistré et restauré quand Mix s'arrête. Arrêter Mix stoppe les unités du graphe, et les nœuds `frame_amix_*` disparaissent.

## Annulation d'écho

Avec les haut-parleurs du Frame, activer l'annulation d'écho dans Frame Mic Tuner. Sinon le microphone entend les haut-parleurs, qui jouent déjà le mix, et le PC peut entendre le son de l'application deux fois. Un casque, ou un microphone qui n'entend pas les haut-parleurs, n'a pas ce chemin acoustique. Un microphone externe reste la source par défaut s'il est choisi. Cette application suit cette source et ne remplace pas les interrupteurs de Frame Mic Tuner.

## Désinstallation

```bash
./install.sh --uninstall
./install.sh --uninstall --purge
```

Sans `--purge`, les réglages restent dans `~/.config/frame-audio-mixer`. `--purge` efface aussi les valeurs `frame-audio-mixer.*` enregistrées. Le script WirePlumber est retiré du disque et reste chargé jusqu'au prochain démarrage de WirePlumber. Ne le faire que hors session VR.

## Compilation

Sur le casque, `rustc` et `cargo` sont dans `~/.cargo/bin`. Les paquets système utilisés sont cairo, freetype, Vulkan, PipeWire et libclang.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
python3 scripts/make-icons.py
cargo build --release
./target/release/frame-audio-mixer --self-test
./target/release/frame-audio-mixer --language fr --dump-png /tmp/frame-audio-mixer.png
./scripts/package.sh
```

`scripts/package.sh` écrit `dist/frame-audio-mixer-<version>.tar.gz` et `dist/SHA256SUMS`.

Sur ce casque le 2026-10-17, `cargo build --release` s'est terminé et `--self-test` a affiché `self-test ok`. `--dump-png` pour `ja`, `en`, `sc`, `de`, `fr`, `es` et `it` a écrit des images 1200×788. Dans chaque image, le pixel le plus à droite du pied de page était x=1152. Aucune langue ne dépassait le panneau de 1200 px.

## Crédit

Le panneau, l'overlay OpenVR et le programme de mise à jour sont adaptés de Frame Mic Tuner par sasaken1102r, sous licence MIT : https://github.com/sasaken1102r/frame-mic-tuner

`vendor/frame-updater/frame-update.sh` est le programme de mise à jour de ce projet.

## Icônes

`scripts/make-icons.py` dessine l'icône du tableau de bord. Les glyphes du panneau sont des icônes MIT d'Iconify :

- https://icon-sets.iconify.design/heroicons/language-16-solid/
- https://icon-sets.iconify.design/fontisto/mic/
- https://icon-sets.iconify.design/famicons/mic-off/
- https://icon-sets.iconify.design/f7/speaker-2-fill/
- https://icon-sets.iconify.design/f7/speaker-slash-fill/
- https://icon-sets.iconify.design/fluent/add-square-28-filled/
- https://icon-sets.iconify.design/at-icons/headphones/ (@icons par Valentin Fossati)
- https://icon-sets.iconify.design/fluent/record-24-filled/
- https://icon-sets.iconify.design/fluent/stop-24-filled/
- https://icon-sets.iconify.design/fluent/play-24-filled/
