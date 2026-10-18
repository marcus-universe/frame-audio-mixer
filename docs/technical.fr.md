[English](technical.md) · [日本語](technical.ja.md) · [简体中文](technical.sc.md) · [Deutsch](technical.de.md) · **Français** · [Español](technical.es.md) · [Italiano](technical.it.md)

[← README](../README.fr.md)

# Détails techniques

## Ce qui est installé

L'installation en une ligne lance [`scripts/get.sh`](../scripts/get.sh). Il télécharge la dernière release (`frame-audio-mixer-aarch64.tar.gz`), la vérifie avec `SHA256SUMS` et lance son `install.sh`. Tant qu'aucune release n'est publiée, il télécharge la branche `main` et la compile à la place, ce qui demande Rust (`cargo`) sur le casque. Il a besoin de `curl`, `sha256sum` et `tar`. Les options placées après `sh -s --` sont transmises à `install.sh`.

`install.sh` n'écrit que dans le répertoire personnel et n'utilise pas `sudo` :

- binaire `~/.local/bin/frame-audio-mixer`
- lanceur `~/.local/lib/frame-audio-mixer/frame-audio-mixer-graph.sh`
- unités utilisateur `frame-audio-mixer.service`, `frame-audio-mixer-graph.service` et `frame-audio-mixer-channel@.service`
- script WirePlumber `~/.local/share/wireplumber/scripts/frame-audio-mixer.lua` et `~/.config/wireplumber/wireplumber.conf.d/91-frame-audio-mixer.conf` (composant `custom.frame-audio-mixer`)
- entrée de bureau et icônes dans `~/.local/share`
- réglages dans `~/.config/frame-audio-mixer/` et noms des canaux dans `~/.config/pipewire/frame-audio-mixer/<id>.name`

Pendant l'exécution, l'appli ajoute des nœuds PipeWire nommés `frame_amix_*`, les réglages WirePlumber `frame-audio-mixer.enabled`, `frame-audio-mixer.vrlink-gain` et `frame-audio-mixer.mic-source`, et l'overlay du tableau de bord `marcus-universe.frame-audio-mixer`.

`install.sh` ne redémarre jamais PipeWire ni WirePlumber. Un script WirePlumber nouveau ou modifié se charge au prochain démarrage de WirePlumber. Ne redémarrez le casque que hors session VR : redémarrer PipeWire ou WirePlumber pendant le jeu coupe le son de SteamVR et de Steam Link. Les canaux fonctionnent quand même avant cela, car le panneau épingle aussi leurs flux tant qu'il tourne.

Si une ancienne expérience a laissé des unités `frame-mic-mix`, `install.sh` les retire. La seule valeur `frame-mic.*` qu'il touche est un `frame-mic.mix` restant. Il n'écrase ni `frame-mic-tracker.lua` ni `90-frame-mic.conf` de Frame Mic Tuner.

Depuis un dépôt git, `./install.sh` compile avec `cargo` puis installe. `./install.sh --autostart` démarre aussi le panneau avec SteamVR. Les options utilisées sont enregistrées dans `~/.config/frame-audio-mixer/install-args`, donc une mise à jour s'installe de la même façon.

## Canaux

- Jusqu'à 32 canaux, de `ch1` à `ch32`. Chacun est une instance systemd avec deux processus `pw-loopback`.
- Un nom tient sur une ligne, 32 caractères au plus, et il est unique sans tenir compte de la casse.
- Un canal créé à partir d'une appli en cours porte le nom de l'appli, épingle l'appli sur ce canal et affiche son icône tirée de son entrée `.desktop`. Sans icône, il affiche la première lettre.
- Quand la rangée est plus large que le panneau, faites-la défiler avec le stick de la manette ou faites glisser la barre sous les canaux.
- Les faders vont de 0 à 150 %. Pendant un glissement, le fader s'aimante à 20, 50, 80 et 100 % quand il passe à moins de 8 pixels. Les points sur un fader marquent 20, 50 et 80 %. Le trait marque 100 %.

## Micro

Le choix du micro ne concerne que le mix. Rien dans cette appli n'appelle `wpctl set-default`, donc le réglage par défaut du système qu'utilise Frame Mic Tuner reste tel quel.

- **Défaut du système** suit `default.audio.source`.
- **Micro intégré du Frame** est `alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source`, la fin de la chaîne de Valve après la réduction de bruit, l'annulation d'écho et l'égaliseur du micro.
- Toute autre source de la liste est enregistrée par son `node.name`.

Le choix est enregistré dans `frame-audio-mixer.mic-source` (`""`, `builtin` ou un nom de nœud). Le script fait correspondre `builtin` à la boucle ci-dessus, revient au défaut du système si ce nœud manque, et n'utilise jamais `frame_amix_source`. Une valeur modifiée prend effet au prochain démarrage de WirePlumber.

## Indicateurs de niveau

Les indicateurs ne tournent que si le tableau de bord est visible, si Mix est actif et si les niveaux sont activés dans Réglages (l'engrenage à côté du menu des langues). Seuls les canaux visibles à l'écran sont mesurés. Chaque indicateur est une prise `pw-record` avec `node.virtual=true`, donc il n'active pas les filtres micro de Valve. Les prises des canaux sont passives. Celle du micro ne l'est pas. Le panneau se redessine à environ 30 images/s tant que les indicateurs tournent, et ne dessine rien quand le tableau de bord est caché.

## Applis et mises à jour

**Applis et mises à jour** affiche cette appli, Frame Mic Tuner comme appli liée, et les applis Frame du catalogue de sasaken. **Installer** ouvre Konsole avec `curl -fsSL https://frame.sasaken1102s.net | sh`. Un Frame Mic Tuner déjà lancé n'a pas de bouton Installer, donc cette page ne remplace pas un fork de celui-ci.

Le programme de mise à jour est écrit en Rust (`ureq` avec rustls, `sha2`, `flate2` et `tar`) :

- Une vérification lit la dernière release GitHub, compare les versions et cherche `frame-audio-mixer-<version>.tar.gz` et `SHA256SUMS`.
- L'installation copie le binaire en cours d'exécution dans `~/.cache/frame-audio-mixer/update/` et le lance avec `systemd-run --user --unit=frame-audio-mixer-update --collect`.
- Les téléchargements se font uniquement en HTTPS, depuis `github.com`, `api.github.com`, `codeload.github.com` et `*.githubusercontent.com`, sauf si `FRAME_UPDATE_ALLOW_INSECURE=1` est défini. Les redirections sont vérifiées, un téléchargement s'arrête à 256 Mo, et le SHA-256 doit correspondre.
- L'archive est refusée si elle contient un chemin absolu, `..`, un lien ou un fichier de périphérique.
- Les fichiers de cache se trouvent dans `~/.cache/frame-audio-mixer/` (`update-check.json`, `update-state.json`, `update.log`, `update.lock/`, `update/`).

## Désinstallation

Avec la commande en une ligne :

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/frame-audio-mixer-install | sh -s -- --uninstall
```

Depuis un dépôt ou un dossier de release :

```bash
./install.sh --uninstall
./install.sh --uninstall --purge
```

Sans `--purge`, les réglages restent dans `~/.config/frame-audio-mixer`. `--purge` les efface aussi, ainsi que les valeurs `frame-audio-mixer.*` enregistrées. Le script WirePlumber est retiré du disque, mais reste chargé jusqu'au prochain démarrage de WirePlumber.

## Compilation

Sur le casque, `rustc` et `cargo` sont dans `~/.cargo/bin`. Les bibliothèques système utilisées sont cairo, freetype, Vulkan, PipeWire et libclang.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release
./target/release/frame-audio-mixer --self-test
./target/release/frame-audio-mixer --language en --dump-png /tmp/frame-audio-mixer.png
./scripts/package.sh
```

`--dump-png` écrit la vue principale et les autres vues (Applis, Ajouter, sélecteur de canal, réglages, menu des langues, une rangée défilée pendant un enregistrement) à côté du chemin donné.

`scripts/package.sh` écrit `dist/frame-audio-mixer-<version>.tar.gz`, identique octet pour octet à `dist/frame-audio-mixer-aarch64.tar.gz`, et `dist/SHA256SUMS` qui liste les deux. Envoyez les trois fichiers dans une release GitHub : l'installation en une ligne et le programme de mise à jour intégré l'utiliseront.
