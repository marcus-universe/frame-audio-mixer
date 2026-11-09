[English](technical.md) · [日本語](technical.ja.md) · [简体中文](technical.sc.md) · [Deutsch](technical.de.md) · **Français** · [Español](technical.es.md) · [Italiano](technical.it.md)

[← README](../README.fr.md)

# Détails techniques

## Ce qui est installé

L'installation en une ligne lance [`scripts/get.sh`](../scripts/get.sh). Il télécharge la dernière release (`frame-audio-mixer-aarch64.tar.gz`), la vérifie avec `SHA256SUMS` et lance son `install.sh`. Tant qu'aucune release n'est publiée, il télécharge la branche `main` et la compile à la place, ce qui demande Rust (`cargo`) sur le casque. Il a besoin de `curl`, `sha256sum` et `tar`. Les options placées après `sh -s --` sont transmises à `install.sh`.

`install.sh` n'écrit que dans le répertoire personnel et n'utilise pas `sudo` :

- binaire `~/.local/bin/frame-audio-mixer`
- unités utilisateur `frame-audio-mixer.service` et `frame-audio-mixer-engine.service`
- script WirePlumber `~/.local/share/wireplumber/scripts/frame-audio-mixer.lua` et `~/.config/wireplumber/wireplumber.conf.d/91-frame-audio-mixer.conf` (composant `custom.frame-audio-mixer`)
- entrée de bureau et icônes dans `~/.local/share`
- réglages dans `~/.config/frame-audio-mixer/` et noms des canaux dans `~/.config/pipewire/frame-audio-mixer/<id>.name`

Pendant l'exécution, l'appli ajoute des nœuds PipeWire nommés `frame_amix_*`, les réglages WirePlumber `frame-audio-mixer.enabled`, `frame-audio-mixer.vrlink-gain` et `frame-audio-mixer.mic-source`, et l'overlay du tableau de bord `marcus-universe.frame-audio-mixer`.

`install.sh` ne redémarre jamais PipeWire ni WirePlumber. Un script WirePlumber nouveau ou modifié se charge au prochain démarrage de WirePlumber. Ne redémarrez le casque que hors session VR : redémarrer PipeWire ou WirePlumber pendant le jeu coupe le son de SteamVR et de Steam Link. Les canaux fonctionnent quand même avant cela, car le panneau épingle aussi leurs flux tant qu'il tourne.

Si une ancienne expérience a laissé des unités `frame-mic-mix`, `install.sh` les retire. La seule valeur `frame-mic.*` qu'il touche est un `frame-mic.mix` restant. Il n'écrase ni `frame-mic-tracker.lua` ni `90-frame-mic.conf` de Frame Mic Tuner.

Depuis un dépôt git, `./install.sh` compile avec `cargo` puis installe. `./install.sh --autostart` démarre aussi le panneau avec SteamVR. Les options utilisées sont enregistrées dans `~/.config/frame-audio-mixer/install-args`, donc une mise à jour s'installe de la même façon.

## Canaux

- Jusqu'à 32 canaux, de `ch1` à `ch32`. Ajouter ou retirer un canal ne crée ou ne détruit que les nœuds de ce canal dans `frame-audio-mixer-engine.service`.
- Un nom tient sur une ligne, 32 caractères au plus, et il est unique sans tenir compte de la casse.
- Un canal créé à partir d'une appli en cours porte le nom de l'appli, épingle l'appli sur ce canal et affiche son icône tirée de son entrée `.desktop`. Sans icône, il affiche la première lettre.
- Quand la rangée est plus large que le panneau, faites-la défiler avec le stick de la manette ou faites glisser la barre sous les canaux.
- Chaque tranche a un fader, du silence à +12 dB, avec un cran à 0 dB. Sourdine et Solo sont dessous. Les boutons de routage sont empilés à côté du fader : A1 à A4 (ce que vous entendez), PC (le micro de streaming du PC), B1 à B4 (micros virtuels), puis FX1 et FX2. Avec 6 ou 8 sorties, les boutons A et B se répartissent sur deux colonnes. Dans la vue en liste, ils sont sur une seule ligne à côté de Sourdine et Solo.

Un flux qui atterrit sur un canal auquel il n'est pas épinglé est renvoyé : vers son propre canal, ou hors du mixeur pour jouer sur la sortie normale. Les épingles comparent le binaire de l'appli quand il est connu — deux applis partageant le même nom (les applis Electron se présentent toutes comme « Chromium ») ne peuvent pas se voler le canal.

## Projets

Le bouton dossier de l'en-tête contient les configurations enregistrées. Un projet stocke les canaux, épingles, routes, chaînes d'effets et niveaux dans un fichier JSON sous `~/.config/frame-audio-mixer/projects/`. Le charger remplace la configuration actuelle ; les préférences d'affichage (langue, thème, mise en page) restent telles quelles.

## Sorties

Le micro de streaming du PC est toujours là. Les Réglages ajoutent 2, 4, 6 ou 8 sorties de plus : 2, c'est A1 et B1 ; 8, c'est A1 à A4 et B1 à B4. Un nombre plus petit garde les routes enregistrées sans les connecter.

- PC est `frame_amix_source`, affiché sous le nom **Frame Mix**. VRLink l'enregistre, donc le PC voit Steam Streaming Microphone.
- A1, ce sont les haut-parleurs du casque. A2 à A4 peuvent être n'importe quelle autre sortie, comme du Bluetooth.
- B1 à B4 sont des micros virtuels pour les applis du casque (`frame_amix_b1` à `frame_amix_b4`). Chacun a un nom.

Le tiroir Sorties (icône haut-parleur sur la barre de droite) contient le contrôle du mix en haut, puis un bloc par sortie : son nom, le fader master dessous et la sourdine à droite. PC vient en premier.

Les réglages de la version 2 sont repris au premier chargement : l'ancien B1 devient PC (avec son fader et sa sourdine), et les anciens B2 à B4 deviennent B1 à B3.

## Flux du signal

La page Flux du signal (à côté d'« Applis et mises à jour » dans l'en-tête) dessine tout le mix comme un graphe en colonnes : sources, tranches du mixeur, sorties, et les appareils atteints. Les liaisons viennent du routage enregistré, et chacune est vérifiée contre les vrais liens PipeWire du moteur — une route dont le lien a disparu s'affiche en pointillés rouges. Un signal audible élargit la liaison d'un halo coloré selon le niveau. Glisser déplace, le stick fait défiler, et sélectionner un nœud montre le nœud PipeWire derrière. Le bouton « Autres applis » montre aussi chaque appli qui n'est sur aucun canal, avec l'appareil sur lequel elle joue réellement.

## Micro

Le choix du micro ne concerne que le mix. Rien dans cette appli n'appelle `wpctl set-default`, donc le réglage par défaut du système qu'utilise Frame Mic Tuner reste tel quel.

- **Défaut du système** suit `default.audio.source`.
- **Micro intégré du Frame** est `alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source`, la fin de la chaîne de Valve après la réduction de bruit, l'annulation d'écho et l'égaliseur du micro.
- Toute autre source de la liste est enregistrée par son `node.name`.

Le choix est enregistré dans `frame-audio-mixer.mic-source` (`""`, `builtin` ou un nom de nœud). Le script fait correspondre `builtin` à la boucle ci-dessus, revient au défaut du système si ce nœud manque, et n'utilise jamais `frame_amix_source`. Une valeur modifiée prend effet au prochain démarrage de WirePlumber.

Le moteur capture cette source avec une boucle qui n'est pas virtuelle, donc la réduction de bruit de Valve reste active tant que Mix est actif. La tranche micro ajoute 12 dB de gain quand le fader est à 0 dB. Sa route par défaut est PC seulement.

## Indicateurs de niveau

Les indicateurs ne tournent que si le tableau de bord est visible, si Mix est actif et si les niveaux sont activés dans Réglages (l'engrenage à côté du menu des langues). Seuls les canaux visibles à l'écran sont mesurés. Chaque indicateur est une prise `pw-record` avec `node.virtual=true`, donc il n'active pas les filtres micro de Valve. Les prises des canaux sont passives. Celle du micro ne l'est pas. Le panneau se redessine à environ 30 images/s tant que les indicateurs tournent, et ne dessine rien quand le tableau de bord est caché.

## Effets

FX1 et FX2 sont des envois partagés, et chacun contient une chaîne de trois effets au plus. EQ en fait partie : un égaliseur paramétrique à 3, 6, 8 ou 12 points, qui s'ouvre avec Modifier sur son emplacement. Les points extrêmes peuvent être une cloche, un shelf, ou un passe-haut (point bas) ou un passe-bas (point haut). Un envoi laisse le chemin direct intact. Chaque retour a son propre fader et ses routes A, PC et B. Le traitement se fait en float32 à 48 kHz et 480 trames dans le moteur. Activer ou couper un envoi ne redémarre ni PipeWire ni WirePlumber.

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
curl -fsSL https://frame-audio-mixer.marcus-universe.com/i | sh -s -- --uninstall
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
