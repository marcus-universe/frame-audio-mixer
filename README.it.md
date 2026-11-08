<p align="center">
  <img src="assets/logo/logo_banner_blue.svg" alt="Frame Audio Mixer" width="640">
</p>
<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/github/license/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="License"></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/version-2.0.0-33C4FF?style=flat-square" alt="Version 2.0.0"></a>
  <a href="https://frame-audio-mixer.marcus-universe.com/"><img src="https://img.shields.io/badge/Website-33C4FF?style=flat-square" alt="Website"></a>
  <a href="https://github.com/marcus-universe/frame-audio-mixer/stargazers"><img src="https://img.shields.io/github/stars/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="GitHub stars"></a>
</p>

[English](README.md) · [日本語](README.ja.md) · [简体中文](README.sc.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · **Italiano**

# Frame Audio Mixer

Un pannello della dashboard di SteamVR per Steam Frame. Mescola l'audio delle app con il microfono del visore e manda quel mix al PC sul flusso wireless. Lì compare come **Steam Streaming Microphone**.

Sta dopo [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner), che continua a gestire la cancellazione dell'eco e la riduzione del rumore.

## Installazione

In Konsole sul Frame (**+** nella barra in basso, poi l'elenco dei programmi, poi **Konsole**), esegui:

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/i | sh
```

- Prima attiva la modalità sviluppatore: Impostazioni di Steam, poi Sistema, poi **Abilita modalità sviluppatore**. Finché non lo fai, Konsole non è nell'elenco.
- Per avviare anche il pannello con SteamVR, sostituisci `| sh` con `| sh -s -- --autostart`.
- Per aggiornare, esegui di nuovo lo stesso comando. Per disinstallare, sostituisci `| sh` con `| sh -s -- --uninstall`.
- Non usa `sudo` e scrive solo nella tua home. Non riavvia mai PipeWire né WirePlumber, quindi una sessione VR in corso mantiene l'audio.
- Da un PC: [Installazione via SSH](docs/install-ssh.it.md).

## Funzioni

- **Orientamento**: ingressi e uscite passano ciascuno dai fader verticali alle righe.
- **Bus effetti**: FX1 e FX2, con riverbero, compressore, EQ, pitch, flanger e phaser. Ogni strip può mandare a entrambi.
- **Microfoni virtuali**: da B1 a B4, con nome, oltre al microfono di streaming del PC.
- **Altoparlanti, Bluetooth e USB**: il mix esce dagli altoparlanti del Frame, da cuffie Bluetooth o dall'audio USB finché uno è collegato.
- **Canali software**: fino a 32 canali con nome. Ognuno è un'uscita che qualsiasi app può scegliere.
- **Vista larga**: un layout 21:9. Si può trascinare la divisione tra le due sezioni. Ogni lato si ferma a un quinto della finestra.
- **Livelli, mute e aggancio**: la stessa scala su ogni fader, da +6 a −48, un pulsante mute sotto ciascuno, e fader di uscita che si agganciano ai segni.
- **Prova mix**: in cima alle Uscite. Scegli un'uscita, registrane fino a 10 secondi, poi riproduci o metti in pausa.
- Sette lingue, e un tema blu o rosa.
- Sette lingue e un tema colore blu o rosa.

<table>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-1.jpg" alt="Vista larga, tema blu" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-2.jpg" alt="Ingressi in righe, tema rosa" height="320"></td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-3.jpg" alt="Bus effetti, tema rosa" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-4.jpg" alt="Dashboard, tema blu" height="320"></td>
  </tr>
</table>

## Uso

1. Apri la dashboard di SteamVR e scegli **Frame Audio Mixer**.
2. Attiva **Mix**.
3. Premi **+** per aggiungere un canale: un suggerimento, un nome tuo o un'app in esecuzione.
4. In quell'app scegli l'uscita `Frame Mix: <name>`.
5. Il fader regola il livello della strip. A1 suona nel visore. Il pulsante PC la manda al PC. Il cassetto Uscite regola il livello di ogni uscita.

Se usi gli altoparlanti del Frame, attiva la cancellazione dell'eco in Frame Mic Tuner. Altrimenti il PC può sentire l'audio dell'app due volte.

## Documentazione

- [Come funziona](docs/how-it-works.it.md): il grafo audio e come vengono instradati VRLink e le app.
- [Dettagli tecnici](docs/technical.it.md): cosa viene installato, dettagli su microfono e livelli, App e aggiornamenti, disinstallazione, compilazione.

## Riconoscimenti

Frame Audio Mixer è ispirato a [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner) di [sasaken1102r](https://github.com/sasaken1102r). Altre app Steam Frame di sasaken@ sono su [frame.sasaken1102s.net](https://frame.sasaken1102s.net/).

Il pannello, l'overlay OpenVR e l'elenco delle app sono adattati da quel progetto (MIT). Vedi [`NOTICE`](NOTICE).

- [Icone e relative licenze](docs/icons.it.md)

## Licenza

[MIT](LICENSE)
