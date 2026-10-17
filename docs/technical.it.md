[English](technical.md) · [日本語](technical.ja.md) · [简体中文](technical.sc.md) · [Deutsch](technical.de.md) · [Français](technical.fr.md) · [Español](technical.es.md) · **Italiano**

[← README](../README.it.md)

# Dettagli tecnici

## Cosa viene installato

L'installazione in una riga esegue [`scripts/get.sh`](../scripts/get.sh). Scarica l'ultima release (`frame-audio-mixer-aarch64.tar.gz`), la verifica con `SHA256SUMS` ed esegue il suo `install.sh`. Finché non c'è una release pubblicata, scarica invece il branch `main` e lo compila, e per questo serve Rust (`cargo`) sul visore. Richiede `curl`, `sha256sum` e `tar`. Le opzioni dopo `sh -s --` vengono passate a `install.sh`.

`install.sh` scrive solo nella home e non usa `sudo`:

- binario `~/.local/bin/frame-audio-mixer`
- runner `~/.local/lib/frame-audio-mixer/frame-audio-mixer-graph.sh`
- unità utente `frame-audio-mixer.service`, `frame-audio-mixer-graph.service` e `frame-audio-mixer-channel@.service`
- script WirePlumber `~/.local/share/wireplumber/scripts/frame-audio-mixer.lua` e `~/.config/wireplumber/wireplumber.conf.d/91-frame-audio-mixer.conf` (componente `custom.frame-audio-mixer`)
- voce desktop e icone in `~/.local/share`
- impostazioni in `~/.config/frame-audio-mixer/` e nomi dei canali in `~/.config/pipewire/frame-audio-mixer/<id>.name`

Durante l'esecuzione l'app aggiunge nodi PipeWire chiamati `frame_amix_*`, le impostazioni WirePlumber `frame-audio-mixer.enabled`, `frame-audio-mixer.vrlink-gain` e `frame-audio-mixer.mic-source`, e l'overlay della dashboard `marcus-universe.frame-audio-mixer`.

`install.sh` non riavvia mai PipeWire né WirePlumber. Uno script WirePlumber nuovo o modificato viene caricato al prossimo avvio di WirePlumber. Riavvia il visore solo quando non sei in una sessione VR: riavviare PipeWire o WirePlumber mentre giochi toglie l'audio a SteamVR e a Steam Link. I canali funzionano anche prima, perché il pannello aggancia i loro flussi finché è in esecuzione.

Se un esperimento precedente ha lasciato unità `frame-mic-mix`, `install.sh` le rimuove. L'unico valore `frame-mic.*` che tocca è un `frame-mic.mix` rimasto. Non sovrascrive `frame-mic-tracker.lua` né `90-frame-mic.conf` di Frame Mic Tuner.

Da un checkout git, `./install.sh` compila con `cargo` e installa. `./install.sh --autostart` avvia anche il pannello con SteamVR. Le opzioni usate vengono salvate in `~/.config/frame-audio-mixer/install-args`, così un aggiornamento installa allo stesso modo.

## Canali

- Fino a 32 canali, da `ch1` a `ch32`. Ognuno è un'istanza systemd con due processi `pw-loopback`.
- Un nome sta su una riga, al massimo 32 caratteri, ed è unico senza distinzione tra maiuscole e minuscole.
- Un canale creato da un'app in esecuzione prende il nome dell'app, aggancia l'app a sé e mostra l'icona dell'app dalla sua voce `.desktop`. Senza icona mostra la prima lettera.
- Quando la riga è più larga del pannello, scorrila con la levetta del controller o trascina la barra sotto i canali.
- I fader vanno da 0 a 150 %. Un trascinamento si aggancia a 20, 50, 80 e 100 % quando arriva entro 8 pixel. I punti su un fader segnano 20, 50 e 80 %. La linea segna 100 %.

## Microfono

La scelta del microfono vale solo per il mix. Niente in questa app chiama `wpctl set-default`, quindi il predefinito di sistema usato da Frame Mic Tuner resta com'è.

- **Predefinito di sistema** segue `default.audio.source`.
- **Microfono del Frame** è `alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source`, la fine della catena Valve dopo riduzione del rumore, cancellazione dell'eco ed EQ del microfono.
- Ogni altra sorgente in elenco viene salvata con il suo `node.name`.

La scelta viene salvata come `frame-audio-mixer.mic-source` (`""`, `builtin` o un nome di nodo). Lo script risolve `builtin` nel loopback qui sopra, torna al predefinito di sistema se quel nodo manca e non usa mai `frame_amix_source`. Un valore modificato ha effetto dopo il prossimo avvio di WirePlumber.

## Livelli

I livelli girano solo se la dashboard è visibile, Mix è attivo e i livelli sono attivi nelle impostazioni dell'app (l'ingranaggio accanto al menu della lingua). Vengono misurati solo i canali visibili sullo schermo. Ogni livello è una presa `pw-record` con `node.virtual=true`, quindi non accende i filtri Valve del microfono. Le prese dei canali sono passive. Quella del microfono no. Il pannello si ridisegna a circa 30 fps finché i livelli girano e non disegna mentre la dashboard è nascosta.

## App e aggiornamenti

**App e aggiornamenti** mostra questa app, Frame Mic Tuner come app correlata e le app Frame del catalogo di sasaken. **Installa** apre Konsole con `curl -fsSL https://frame.sasaken1102s.net | sh`. Un Frame Mic Tuner già in esecuzione non ha il pulsante Installa, quindi questa pagina non sostituisce un suo fork.

Il programma di aggiornamento è scritto in Rust (`ureq` con rustls, `sha2`, `flate2` e `tar`):

- Un controllo legge l'ultima release GitHub, confronta le versioni e cerca `frame-audio-mixer-<version>.tar.gz` e `SHA256SUMS`.
- L'installazione copia il binario in esecuzione in `~/.cache/frame-audio-mixer/update/` e lo avvia con `systemd-run --user --unit=frame-audio-mixer-update --collect`.
- I download sono solo HTTPS, da `github.com`, `api.github.com`, `codeload.github.com` e `*.githubusercontent.com`, salvo che sia impostato `FRAME_UPDATE_ALLOW_INSECURE=1`. I reindirizzamenti vengono controllati, un download si ferma a 256 MB e lo SHA-256 deve coincidere.
- L'archivio viene rifiutato se contiene un percorso assoluto, `..`, un link o un nodo di dispositivo.
- I file di cache sono in `~/.cache/frame-audio-mixer/` (`update-check.json`, `update-state.json`, `update.log`, `update.lock/`, `update/`).

## Disinstallazione

Con il comando in una riga:

```sh
curl -fsSL https://raw.githubusercontent.com/marcus-universe/frame-audio-mixer/main/scripts/get.sh | sh -s -- --uninstall
```

Da un checkout o da una cartella di release:

```bash
./install.sh --uninstall
./install.sh --uninstall --purge
```

Senza `--purge`, le impostazioni restano in `~/.config/frame-audio-mixer`. `--purge` cancella anche quelle e i valori salvati `frame-audio-mixer.*`. Lo script WirePlumber viene tolto dal disco ma resta caricato fino al prossimo avvio di WirePlumber.

## Compilazione

Sul visore, `rustc` e `cargo` sono in `~/.cargo/bin`. Le librerie di sistema usate sono cairo, freetype, Vulkan, PipeWire e libclang.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release
./target/release/frame-audio-mixer --self-test
./target/release/frame-audio-mixer --language en --dump-png /tmp/frame-audio-mixer.png
./scripts/package.sh
```

`--dump-png` scrive la vista principale e le altre viste (App, Aggiungi, scelta del canale, impostazioni, menu della lingua, una riga scorsa durante la registrazione) accanto al percorso indicato.

`scripts/package.sh` scrive `dist/frame-audio-mixer-<version>.tar.gz`, con gli stessi byte di `dist/frame-audio-mixer-aarch64.tar.gz`, e `dist/SHA256SUMS` che li elenca entrambi. Carica tutti e tre in una release GitHub, e l'installazione in una riga e il programma di aggiornamento nell'app la useranno.
