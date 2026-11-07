[English](how-it-works.md) · [日本語](how-it-works.ja.md) · [简体中文](how-it-works.sc.md) · [Deutsch](how-it-works.de.md) · **Français** · [Español](how-it-works.es.md) · [Italiano](how-it-works.it.md)

[← README](../README.fr.md)

# Fonctionnement

```mermaid
flowchart LR
  appA[Appli] --> chSink["frame_amix_chN"]
  micSrc["Micro après les filtres Valve"] --> micStrip["Tranche micro"]
  chSink --> chStrip["Tranche"]
  chStrip --> busA["A1 à A4"]
  chStrip --> busB["B1 à B4"]
  micStrip --> busB
  busA --> speakers["Haut-parleurs ou autre sortie"]
  busB --> frameMix["frame_amix_source"]
  frameMix --> vrlink["VRLink"]
```

## Le micro

Le mix part du micro choisi dans le panneau. **Défaut du système** suit la source par défaut. **Micro du Frame** est le micro intégré après les filtres de Valve. Frame Mic Tuner les contrôle. Cette appli ne les change pas et ne change pas le réglage par défaut du système.

## Le moteur

`frame-audio-mixer-engine.service` possède le graphe. Le mix continue quand le panneau se ferme.

- Chaque canal est un sink `frame_amix_chN`. Les applis le voient sous le nom du canal (`DJ`).
- La tranche applique les inserts, le fader, la sourdine, le solo et le niveau, puis envoie vers les bus A et B allumés.
- A1, ce sont les haut-parleurs du casque. A2 à A4, c'est la sortie choisie.
- B1 est `frame_amix_source` (**Frame Mix**). B2 à B4 sont d'autres micros virtuels.
- La tranche micro va par défaut seulement vers B1. La capture n'est pas virtuelle, donc la réduction de bruit reste active tant que Mix est allumé.

Ajouter ou retirer un canal ne crée ou ne détruit que ses nœuds. PipeWire et WirePlumber ne redémarrent pas.

Les noms internes contiennent `virtual` et `sink`, donc l'audio spatial ne les enveloppe pas.

## Routage vers le PC

Tant que Mix est actif (`frame-audio-mixer.enabled` est vrai), le composant WirePlumber `custom.frame-audio-mixer` dirige le flux de capture VRLink vers `frame_amix_source`. Il écrit l'`object.serial` de cette source dans les métadonnées par défaut, car un nom de nœud à cet endroit ne déplace pas le flux. Si la source manque, VRLink garde le micro par défaut habituel. Les autres applis qui enregistrent le micro ne sont pas déplacées.

`frame-audio-mixer.vrlink-gain` vaut `1.0` par défaut, ce qui laisse le volume de VRLink tel quel. Toute autre valeur s'applique tant que Mix est actif, et le volume précédent revient quand Mix s'arrête.

## Routage des applis vers les canaux

Une appli qui choisit `Frame Mix: <name>` passe quand même par l'audio spatial, et un nom de nœud comme cible est ignoré. Frame Audio Mixer épingle donc le flux au canal avec l'`object.serial` de la sortie. Le script WirePlumber et le panneau le font tous les deux. Le serial change quand un canal redémarre, et l'épingle est réécrite quand la sortie revient. Quand une sortie est retirée, l'épingle est effacée et l'appli revient à la sortie habituelle.

Un canal créé à partir d'une appli en cours se souvient aussi de cette appli. Tant que Mix est actif, ses flux sont épinglés au canal, même si l'appli n'a pas de réglage de sortie.

Rien dans cette appli n'appelle `wpctl set-default`.

## Annulation d'écho

Avec les haut-parleurs du Frame, le micro peut entendre les haut-parleurs. Ceux-ci jouent déjà les canaux, donc sans annulation d'écho le PC entend le son de l'appli deux fois : une fois par le mix et une fois par l'air. Activez l'annulation d'écho dans Frame Mic Tuner quand vous utilisez les haut-parleurs. Les écouteurs n'ont pas ce problème.
