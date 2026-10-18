[English](install-ssh.md) · [日本語](install-ssh.ja.md) · [简体中文](install-ssh.sc.md) · [Deutsch](install-ssh.de.md) · [Français](install-ssh.fr.md) · [Español](install-ssh.es.md) · **Italiano**

[← README](../README.it.md)

# Installazione via SSH

Installa Frame Audio Mixer da un PC. Entri nello Steam Frame ed esegui lo stesso comando di Konsole. Il PC e il visore devono essere sulla stessa rete.

## Attivare SSH

1. Sul Frame, apri Impostazioni di Steam, poi Sistema, poi **Abilita modalità sviluppatore**.
2. Sotto Sviluppatore, imposta una password. Quella password è l'accesso SSH. Scegline una robusta: con SSH, chiunque nella tua rete che la conosca può entrare nel visore.
3. Annota l'indirizzo IP del visore. Compare nelle impostazioni sviluppatore, oppure sul router.

Il nome utente è `steamos`.

## Connettersi

Windows 10 e 11, macOS e Linux includono un client OpenSSH. In un terminale sul PC:

```sh
ssh steamos@<headset-ip>
```

Sostituisci `<headset-ip>` con l'indirizzo del passo precedente. La prima volta conferma la chiave dell'host, poi inserisci la password sviluppatore.

## Installare

In quella sessione, esegui:

```sh
curl -fsSL https://raw.githubusercontent.com/marcus-universe/frame-audio-mixer/main/scripts/get.sh | sh
```

- Per avviare anche il pannello con SteamVR, termina il comando con `| sh -s -- --autostart`.
- Per aggiornare, esegui di nuovo lo stesso comando. Per disinstallare, terminalo con `| sh -s -- --uninstall`.
- Non usa `sudo` e scrive solo nella home del visore. Non riavvia mai PipeWire né WirePlumber, quindi una sessione VR in corso mantiene l'audio.

Puoi anche mandare il comando in una riga, senza restare nella sessione. Su macOS, Linux o Windows Terminal:

```sh
ssh steamos@<headset-ip> "curl -fsSL https://raw.githubusercontent.com/marcus-universe/frame-audio-mixer/main/scripts/get.sh | sh"
```

Il programma di installazione gira solo sullo Steam Frame. Una sessione SSH è già sul visore, quindi quel controllo riesce.

Poi apri la dashboard di SteamVR e scegli **Frame Audio Mixer**.
