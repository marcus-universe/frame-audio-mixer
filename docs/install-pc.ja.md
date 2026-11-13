[English](install-pc.md) · **日本語** · [简体中文](install-pc.sc.md) · [Deutsch](install-pc.de.md) · [Français](install-pc.fr.md) · [Español](install-pc.es.md) · [Italiano](install-pc.it.md)

[← README](../README.ja.md)

# PC のオーディオデバイスをインストール

**Frame Audio Link** は、ストリーミング元の PC で動く SteamVR アドオンです。PC に 8 つのオーディオデバイスを追加します。

| PC 上の名前 | 内容 |
|---|---|
| **Steam Streaming Mic B1** … **B4** | マイク。ヘッドセットのミキサーの B1–B4 が届きます。 |
| **Steam Streaming Out PC1** … **PC4** | 出力。ここで再生した音を、ミキサーでチャンネル PC1–PC4 として追加できます。 |

Steam 本来の **Steam Streaming Microphone** と **Steam Streaming Speakers** はそのまま残ります。

## 始める前に

- ヘッドセットに Frame Audio Mixer 2.2 以降（[インストール](../README.ja.md#-インストール)を参照）。
- PC に SteamVR があり、Steam Frame へのワイヤレスストリーミングが動いていること。

## Windows

1. [最新リリース](https://github.com/marcus-universe/frame-audio-mixer/releases/latest)から `frame-audio-link-windows-x86_64.zip` をダウンロードして展開します。
2. **install.cmd** をダブルクリックします。
3. Windows が一度だけ管理者権限を求めます。この手順で 8 つのデバイスが作られます。

PowerShell の場合:

```powershell
irm https://frame-audio-mixer.marcus-universe.com/get-windows | iex
```

プログラムは `%LOCALAPPDATA%\Programs\FrameAudioLink` に入ります。

- デバイスは、Steam がすでに入れた 2 つのオーディオドライバーの追加インスタンスです。新しいドライバーは入りません。
- Steam のドライバーは、Remote Play かワイヤレス VR ストリーミングを初めて使ったときに入ります。見つからないと表示されたら、一度ストリーミングしてからもう一度実行してください。
- Windows が新しいデバイスを既定にした場合、インストーラーが元の既定のデバイスに戻します。

デバイスは SteamVR を閉じていても残ります。

## Linux

Ubuntu 22.10 以降、Fedora、Arch で動きます。サウンドサーバーが PipeWire である必要があります。次で確認できます:

```sh
pactl info | grep "Server Name"
```

`PulseAudio (on PipeWire …)` と出れば大丈夫です。Ubuntu 22.04 はまだ PulseAudio なので対応していません。Arch では、`pipewire-pulse` と `wireplumber` がなければ先に入れてください。

`sudo` を使わず、普段のユーザーでインストールします:

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/get-linux | sh
```

プログラムは `~/.local/bin/frame-audio-link` に入ります。Linux では 8 つのデバイスは PipeWire のノードで、アドオンが動いている間、つまり SteamVR が動いている間だけ存在します。サウンドシステムには何も入りません。

## SteamVR でオンにする

インストーラーはアドオンをスタートアップアプリとして登録します。オンになっているか確認してください: **SteamVR → 設定 → 起動/終了 → 起動時のオーバーレイアプリを選択 → Frame Audio Link**。

ダッシュボードに **Audio Link** タブが出ます。ヘッドセットが見つかったかを表示し、8 つのストリームそれぞれにスイッチがあります。オフのストリームは帯域を使いません。

## 使い方

- **ヘッドセットから PC へ**: ミキサーでマイクかチャンネルを B1–B4 に送ります。PC では、どのアプリ（Discord、OBS、ゲーム）でも **Steam Streaming Mic B1** … **B4** をマイクに選べます。
- **PC からヘッドセットへ**: PC でアプリの出力を **Steam Streaming Out PC1** … **PC4** にします。ミキサーで **+** を押し、実行中の一覧から **PC1** … **PC4** を選びます。

## 確認

- **Windows**: 8 つのデバイスがサウンドのコントロールパネル（`mmsys.cpl`）に出ます。ターミナルで `frame-audio-link devices status` でも一覧できます。
- **Linux**: アドオンが動いている間、`pactl list short sources` と `pactl list short sinks` に `frame_link_b1` … と `frame_link_pc1` … が出ます。
- **オーバーレイが「検索中」のまま**: Steam Frame でワイヤレスストリーミングを始め、そこで Frame Audio Mixer を開いてください。ヘッドセットはストリーミング元の PC にしか応答しません。
- **オーバーレイにデバイスがないと出る**: Windows では `install.cmd` をもう一度実行してください。

ログは Windows では `%APPDATA%\frame-audio-link\frame-audio-link.log`、Linux では `~/.config/frame-audio-link/frame-audio-link.log` にあります。

## アンインストール

- **Windows**: アーカイブの **uninstall.cmd** をダブルクリックします。8 つのデバイス、SteamVR の登録、プログラムが消えます。
- **Linux**: アーカイブの `./install-linux.sh --uninstall` を実行します。

リンクのしくみは [PC audio link](pc-link.md)（英語）にあります。
