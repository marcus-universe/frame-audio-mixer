<p align="center">
  <img src="assets/logo/logo_banner_blue.svg" alt="Frame Audio Mixer" width="640">
</p>
<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/github/license/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="License"></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/version-1.0.0-33C4FF?style=flat-square" alt="Version 1.0.0"></a>
  <a href="https://github.com/marcus-universe/frame-audio-mixer/stargazers"><img src="https://img.shields.io/github/stars/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="GitHub stars"></a>
</p>

[English](README.md) · [日本語](README.ja.md) · [简体中文](README.sc.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · **Italiano**

# Frame Audio Mixer

Un pannello della dashboard di SteamVR per Steam Frame. Mescola l'audio delle app con il microfono del visore e manda quel mix al PC sul flusso wireless. Lì compare come **Steam Streaming Microphone**.

Sta dopo [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner), che continua a gestire la cancellazione dell'eco e la riduzione del rumore.

## Funzioni

- **Dispositivi audio virtuali**: fino a 32 canali con nome. Ognuno è un'uscita (`Frame Mix: <name>`) che qualsiasi app può scegliere.
- **Canali delle app**: crea un canale da un'app in esecuzione. La sua icona compare accanto al nome.
- **Mix per il microfono di streaming del PC**: il PC sente il tuo microfono più i canali.
- **Due fader per canale**: uno per il visore, uno per il PC, ognuno con il proprio mute. I fader si agganciano a 20, 50, 80 e 100 %.
- **Microfono**: scegli il microfono per il mix. Il microfono predefinito di sistema non cambia.
- **Livelli** su ogni fader. Si possono spegnere.
- **Prova mix**: registra fino a 10 secondi di ciò che sente il PC e lo riproduce.
- Sette lingue e un tema colore blu o rosa.

## Installazione

In Konsole sul Frame (**+** nella barra in basso, poi l'elenco dei programmi, poi **Konsole**), esegui:

```sh
curl -fsSL https://raw.githubusercontent.com/marcus-universe/frame-audio-mixer/main/scripts/get.sh | sh
```

- Prima attiva la modalità sviluppatore: Impostazioni di Steam, poi Sistema, poi **Abilita modalità sviluppatore**. Finché non lo fai, Konsole non è nell'elenco.
- Per avviare anche il pannello con SteamVR, termina il comando con `| sh -s -- --autostart`.
- Per aggiornare, esegui di nuovo lo stesso comando. Per disinstallare, terminalo con `| sh -s -- --uninstall`.
- Non usa `sudo` e scrive solo nella tua home. Non riavvia mai PipeWire né WirePlumber, quindi una sessione VR in corso mantiene l'audio.

## Uso

1. Apri la dashboard di SteamVR e scegli **Frame Audio Mixer**.
2. Attiva **Mix**.
3. Premi **+** per aggiungere un canale: un suggerimento, un nome tuo o un'app in esecuzione.
4. In quell'app scegli l'uscita `Frame Mix: <name>`.
5. Il fader dell'altoparlante regola quanto è forte il canale nel visore. Il fader del microfono regola quanto ne sente il PC.

Se usi gli altoparlanti del Frame, attiva la cancellazione dell'eco in Frame Mic Tuner. Altrimenti il PC può sentire l'audio dell'app due volte.

## Documentazione

- [Come funziona](docs/how-it-works.it.md): il grafo audio e come vengono instradati VRLink e le app.
- [Dettagli tecnici](docs/technical.it.md): cosa viene installato, dettagli su microfono e livelli, App e aggiornamenti, disinstallazione, compilazione.

## Riconoscimenti

Il pannello e l'overlay OpenVR sono adattati da [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner) di sasaken1102r (MIT). L'elenco delle app è un port Rust di frame-apps 0.3.1 dello stesso progetto, e il programma di aggiornamento ne segue il comportamento. Vedi [`NOTICE`](NOTICE).

- [Icone e relative licenze](docs/icons.it.md)

## Licenza

[MIT](LICENSE)
