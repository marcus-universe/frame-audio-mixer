[English](install-pc.md) · [日本語](install-pc.ja.md) · [简体中文](install-pc.sc.md) · [Deutsch](install-pc.de.md) · [Français](install-pc.fr.md) · [Español](install-pc.es.md) · **Italiano**

[← README](../README.it.md)

# Installare i dispositivi audio del PC

**Frame Audio Link** è un componente aggiuntivo di SteamVR per il PC da cui fai streaming. Aggiunge otto dispositivi audio al PC:

| Sul PC | Cos'è |
|---|---|
| **Steam Streaming Mic B1** … **B4** | Microfoni. Portano B1–B4 del mixer sul visore. |
| **Steam Streaming Out PC1** … **PC4** | Uscite. Ciò che vi suona si può aggiungere nel mixer come canale PC1–PC4. |

Lo **Steam Streaming Microphone** e gli **Steam Streaming Speakers** di Steam restano come sono.

## Prima di iniziare

- Frame Audio Mixer 2.2 o successivo sul visore (vedi [Installazione](../README.it.md#-installazione)).
- SteamVR sul PC, e lo streaming wireless verso lo Steam Frame funzionante.

## Windows

1. Scarica `frame-audio-link-windows-x86_64.zip` dall'[ultima versione](https://github.com/marcus-universe/frame-audio-mixer/releases/latest) ed estrailo.
2. Fai doppio clic su **install.cmd**.
3. Windows chiede una volta i permessi di amministratore. Questo passo crea gli otto dispositivi.

Oppure in PowerShell:

```powershell
irm https://frame-audio-mixer.marcus-universe.com/get-windows | iex
```

Il programma va in `%LOCALAPPDATA%\Programs\FrameAudioLink`.

- I dispositivi sono altre istanze dei due driver audio che Steam ha già installato. Non viene installato nessun driver nuovo.
- Steam installa i suoi driver la prima volta che si usa Remote Play o lo streaming VR wireless. Se l'installatore dice che mancano, fai streaming una volta e rieseguilo.
- Se Windows rende predefinito uno dei nuovi dispositivi, l'installatore rimette quello che avevi.

I dispositivi restano installati, anche quando SteamVR è chiuso.

## Linux

Funziona su Ubuntu 22.10 o successivo, Fedora e Arch. PipeWire deve essere il server audio. Controlla con:

```sh
pactl info | grep "Server Name"
```

Deve dire `PulseAudio (on PipeWire …)`. Ubuntu 22.04 usa ancora PulseAudio e non è supportato. Su Arch, installa prima `pipewire-pulse` e `wireplumber` se mancano.

Installa come utente normale, senza `sudo`:

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/get-linux | sh
```

Il programma va in `~/.local/bin/frame-audio-link`. Su Linux gli otto dispositivi sono nodi PipeWire: esistono finché il componente è in esecuzione, cioè finché SteamVR è in esecuzione. Nel sistema audio non viene installato nulla.

## Attivarlo in SteamVR

L'installatore registra il componente come app di avvio. Controlla che sia attivo: **SteamVR → Impostazioni → Avvio / Arresto → Scegli le app overlay all'avvio → Frame Audio Link**.

La dashboard ha allora una scheda **Audio Link**. Mostra se il visore è stato trovato e ha un interruttore per ognuno degli otto flussi. Un flusso spento non usa banda.

## Uso

- **Dal visore al PC**: nel mixer, manda il microfono o un canale su B1–B4. Sul PC, scegli **Steam Streaming Mic B1** … **B4** come microfono in qualsiasi app (Discord, OBS, un gioco).
- **Dal PC al visore**: sul PC, fai suonare un'app su **Steam Streaming Out PC1** … **PC4**. Nel mixer, premi **+** e scegli **PC1** … **PC4** sotto «In esecuzione».

## Controllare

- **Windows**: gli otto dispositivi sono elencati nel pannello Audio (`mmsys.cpl`). Anche `frame-audio-link devices status` in un terminale li elenca.
- **Linux**: mentre il componente è in esecuzione, `pactl list short sources` e `pactl list short sinks` mostrano `frame_link_b1` … e `frame_link_pc1` ….
- **L'overlay dice «Ricerca»**: avvia lo streaming wireless sullo Steam Frame e apri lì Frame Audio Mixer. Il visore risponde solo al PC da cui fa streaming.
- **L'overlay dice che manca un dispositivo**: su Windows, esegui di nuovo `install.cmd`.

Il registro è in `%APPDATA%\frame-audio-link\frame-audio-link.log` su Windows e in `~/.config/frame-audio-link/frame-audio-link.log` su Linux.

## Disinstallare

- **Windows**: fai doppio clic su **uninstall.cmd** dell'archivio. Rimuove gli otto dispositivi, la voce di SteamVR e il programma.
- **Linux**: esegui `./install-linux.sh --uninstall` dall'archivio.

Come funziona il collegamento è spiegato in [PC audio link](pc-link.md) (in inglese).
