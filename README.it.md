# Frame Audio Mixer

Frame Audio Mixer mescola uscite di applicazioni con un nome e il microfono di Steam Frame, e manda quel mix al PC sul flusso wireless. Il PC mostra il flusso come **Steam Streaming Microphone**.

L'app sta dopo [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner). La cancellazione dell'eco e la soppressione del rumore restano in Frame Mic Tuner. Questo tracker del microfono non viene sostituito.

## Cosa fa

- Aggiunge fino a otto uscite con nome, per esempio musica o un gioco.
- Manda ogni uscita al visore e, con il proprio livello, nel mix del microfono.
- Con Mix attivo, solo la cattura di VRLink punta a quel mix. Le altre app tengono il microfono predefinito.
- Il grafo resta spento finché non attivi Mix. Il PC torna allora a sentire il microfono normale.

## Installazione

Eseguila sul visore come utente normale. Non usa `sudo` e scrive solo nella home.

Da un checkout git o da un archivio di rilascio:

```bash
./install.sh
./install.sh --autostart
```

`--autostart` avvia anche il pannello insieme a SteamVR. Il grafo del mix resta spento finché non attivi Mix nel pannello.

`install.sh` non riavvia PipeWire né WirePlumber. Uno script WirePlumber nuovo viene caricato al prossimo avvio di WirePlumber. Fallo solo quando non c'è una sessione VR. Riavviare PipeWire o WirePlumber mentre giochi toglie l'audio a SteamVR e a Steam Link.

L'installazione aggiunge un'app separata:

- binario `~/.local/bin/frame-audio-mixer`
- unità utente `frame-audio-mixer.service`, `frame-audio-mixer-graph.service` e `frame-audio-mixer-channel@.service`
- nodi PipeWire chiamati `frame_amix_*`
- componente WirePlumber `custom.frame-audio-mixer`
- impostazioni `frame-audio-mixer.enabled` e `frame-audio-mixer.vrlink-gain`
- chiave overlay della dashboard `marcus-universe.frame-audio-mixer`

Se un esperimento precedente ha lasciato unità `frame-mic-mix`, `install.sh` le rimuove. L'unico valore `frame-mic.*` che modifica è la cancellazione di un `frame-mic.mix` rimasto. Non sovrascrive `frame-mic-tracker.lua` né `90-frame-mic.conf`.

## Uso

Apri **Frame Audio Mixer** nella dashboard di SteamVR. Avviare di nuovo l'app mentre è in esecuzione apre lo stesso pannello.

1. Attiva Mix.
2. Aggiungi un canale (fino a 8). Il nome sta su una riga, al massimo 32 caratteri, ed è unico senza distinzione tra maiuscole e minuscole.
3. Nell'app scegli l'uscita `Frame Mix: <name>`.
4. **Cuffie** è il volume di quel canale nel visore. **Verso il microfono** è quanto ne sente il PC. Il fader del microfono e il suo mute riguardano solo questo mix, non il microfono predefinito di sistema.

**Controllo mix** registra **Frame Mic + Mix** per al massimo 10 secondi e lo riproduce sull'uscita attuale. Il piè di pagina sceglie la lingua (日本語, English, 简体中文, Deutsch, Français, Español, Italiano), avvia il pannello con SteamVR ed esce. **App e aggiornamenti** controlla le release di questa app e collega Frame Mic Tuner.

## Come funziona

Il microfono è la sorgente predefinita in quel momento. Su Frame è il microfono interno dopo i filtri Valve, salvo che sia stato scelto un altro microfono.

Con Mix attivo, `frame-audio-mixer-graph.service` costruisce il nucleo:

- `frame_amix_from_mic` cattura la sorgente predefinita.
- `frame_amix_mic_virtual_sink` è il fader del microfono verso `frame_amix_bus`.
- `frame_amix_source` (**Frame Mic + Mix**) è la sorgente registrata da VRLink.

Ogni canale è il proprio servizio `frame-audio-mixer-channel@chN.service`:

- `frame_amix_chN` è il sink scelto dalle app (`Frame Mix: <name>`).
- `frame_amix_chN_virtual_sink_playback` riproduce quel sink nel visore. È il fader delle cuffie.
- `frame_amix_chN_virtual_sink_to_bus` manda lo stesso sink nel bus del mix. È il fader verso il microfono.

Quei nomi interni di riproduzione contengono sia `virtual` sia `sink`. Lo spatializzatore non li avvolge e non manda il mix una seconda volta agli altoparlanti. Il sink del canale non è `alsa_output` né `bluez_output`, quindi non compare nell'elenco uscite di Frame Mic Tuner.

Il componente WirePlumber `custom.frame-audio-mixer` aggancia la cattura VRLink a `frame_amix_source` tramite `object.serial` mentre `frame-audio-mixer.enabled` è vero. Un nome di nodo nei metadati non sposta il flusso. Se la sorgente manca, VRLink resta sulla sorgente predefinita. Lo script non cambia `frame-mic.echo-cancel` né `frame-mic.noise-suppression`.

Il pannello aggancia un'app allo stesso modo, con l'`object.serial` del sink. Il serial cambia al riavvio del canale e l'aggancio viene riscritto. Questa app non chiama `wpctl set-default`.

`frame-audio-mixer.vrlink-gain` è `1.0` per impostazione predefinita. Con quel valore l'app non cambia il volume di VRLink. Ogni altro guadagno viene salvato e ripristinato quando Mix si spegne. Spegnere Mix ferma le unità del grafo e i nodi `frame_amix_*` spariscono.

## Cancellazione dell'eco

Con gli altoparlanti di Frame, attiva la cancellazione dell'eco in Frame Mic Tuner. Altrimenti il microfono sente gli altoparlanti, che stanno già riproducendo il mix, e il PC può sentire l'audio dell'app due volte. Le cuffie, o un microfono che non sente gli altoparlanti, non hanno quel percorso acustico. Un microfono esterno resta la sorgente predefinita se lo selezioni. Questa app segue quella sorgente e non sostituisce gli interruttori di Frame Mic Tuner.

## Disinstallazione

```bash
./install.sh --uninstall
./install.sh --uninstall --purge
```

Senza `--purge`, le impostazioni restano in `~/.config/frame-audio-mixer`. `--purge` cancella anche i valori salvati `frame-audio-mixer.*`. Lo script di WirePlumber viene tolto dal disco e resta caricato fino al prossimo avvio di WirePlumber. Fallo solo quando non c'è una sessione VR.

## Compilazione

Sul visore, `rustc` e `cargo` sono in `~/.cargo/bin`. I pacchetti di sistema usati sono cairo, freetype, Vulkan, PipeWire e libclang.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
python3 scripts/make-icons.py
cargo build --release
./target/release/frame-audio-mixer --self-test
./target/release/frame-audio-mixer --language it --dump-png /tmp/frame-audio-mixer.png
./scripts/package.sh
```

`scripts/package.sh` scrive `dist/frame-audio-mixer-<version>.tar.gz` e `dist/SHA256SUMS`.

Su questo visore il 2026-10-17, `cargo build --release` è terminato e `--self-test` ha stampato `self-test ok`. `--dump-png` per `ja`, `en`, `sc`, `de`, `fr`, `es` e `it` ha scritto immagini 1200×788. In ogni immagine il pixel più a destra del piè di pagina era x=1152. Nessuna lingua superava il pannello largo 1200 px.

## Riconoscimenti

Il pannello, l'overlay OpenVR e il programma di aggiornamento sono adattati da Frame Mic Tuner di sasaken1102r, licenza MIT: https://github.com/sasaken1102r/frame-mic-tuner

`vendor/frame-updater/frame-update.sh` è il programma di aggiornamento di quel progetto.

## Icone

`scripts/make-icons.py` disegna l'icona della dashboard. I glifi del pannello sono icone MIT di Iconify:

- https://icon-sets.iconify.design/heroicons/language-16-solid/
- https://icon-sets.iconify.design/fontisto/mic/
- https://icon-sets.iconify.design/famicons/mic-off/
- https://icon-sets.iconify.design/f7/speaker-2-fill/
- https://icon-sets.iconify.design/f7/speaker-slash-fill/
- https://icon-sets.iconify.design/fluent/add-square-28-filled/
- https://icon-sets.iconify.design/at-icons/headphones/ (@icons di Valentin Fossati)
- https://icon-sets.iconify.design/fluent/record-24-filled/
- https://icon-sets.iconify.design/fluent/stop-24-filled/
- https://icon-sets.iconify.design/fluent/play-24-filled/

## Controllo sul visore (2026-10-17)

Misurato mentre `steamvr.service` e `gamescope-session.service` erano active. WirePlumber non è stato riavviato. Il nuovo script è installato e non è ancora caricato.

- `~/.local/bin/frame-audio-mixer` è installato. `frame-audio-mixer.service` è enabled e inactive. `frame-audio-mixer-graph.service` è disabled e inactive. `frame-mic-tuner.service` è enabled e active.
- `frame-mic-tracker.lua` e `90-frame-mic.conf` sono ancora presenti e non citano `frame_amix`. `91-frame-audio-mixer.conf` e `frame-audio-mixer.lua` sono installati. Non c'è l'unità `frame-mic-mix.service`.
- `frame-mic.echo-cancel` è false (salvato false). `frame-mic.noise-suppression` è true (salvato true). `frame-mic.mix` è ancora in elenco, valore false (salvato false): il WirePlumber in esecuzione ha tenuto la chiave. `frame-audio-mixer.enabled` e `frame-audio-mixer.vrlink-gain` non sono ancora nelle impostazioni in esecuzione.
- `pw-dump` aveva 19 nodi e nessun `frame_amix_*`, `frame_mix_*` o `frame_micmix*`. I flussi di cattura erano `eq_capture`, `echo_cancel_capture`, `ns_capture` e il loopback ALSA del microfono. Non c'era un flusso di cattura VRLink, quindi il suo volume non è stato misurato. La sorgente predefinita era `alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source`. Il sink predefinito era `alsa_loopback_device.stereo.alsa_output.platform-sound.HiFi__Speaker__sink`.
- I controlli udibili (microfono, eco doppio, microfono esterno) non sono stati fatti. Serve qualcuno al PC, e il grafo del mix non è stato avviato durante questa sessione VR.
