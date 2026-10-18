[English](install-ssh.md) · **日本語** · [简体中文](install-ssh.sc.md) · [Deutsch](install-ssh.de.md) · [Français](install-ssh.fr.md) · [Español](install-ssh.es.md) · [Italiano](install-ssh.it.md)

[← README](../README.ja.md)

# SSH でインストール

PC から Frame Audio Mixer をインストールします。Steam Frame にログインして、Konsole と同じコマンドを実行します。PC とヘッドセットは同じネットワークにある必要があります。

## SSH を有効にする

1. Frame で Steam の設定、システム、**開発者モードを有効化** の順に開きます。
2. 開発者の項目でパスワードを設定します。これが SSH のログインパスワードです。強いパスワードにしてください。SSH を有効にすると、同じネットワークでパスワードを知っている人は誰でもヘッドセットにログインできます。
3. ヘッドセットの IP アドレスを控えます。開発者設定か、ルーターに表示されます。

ユーザー名は `steamos` です。

## 接続する

Windows 10 と 11、macOS、Linux には OpenSSH クライアントが入っています。PC のターミナルで次を実行します。

```sh
ssh steamos@<headset-ip>
```

`<headset-ip>` を上で控えたアドレスに置き換えます。初回はホスト鍵を確認し、そのあと開発者パスワードを入力します。

## インストールする

そのセッションで次を実行します。

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/i -o frame-audio-mixer-install && sh frame-audio-mixer-install
```

- パネルを SteamVR と一緒に起動するには、最後に `--autostart` を付けます。
- 更新するには、同じコマンドをもう一度実行します。アンインストールするには、最後に `--uninstall` を付けます。
- `sudo` は使わず、書き込むのはヘッドセットのホームディレクトリだけです。PipeWire や WirePlumber を再起動しないので、実行中の VR セッションの音は途切れません。

対話セッションに残らず、1 行で送ることもできます。macOS、Linux、または Windows Terminal で次を実行します。

```sh
ssh steamos@<headset-ip> "curl -fsSL https://frame-audio-mixer.marcus-universe.com/i -o frame-audio-mixer-install && sh frame-audio-mixer-install"
```

インストーラーは Steam Frame 上でだけ動きます。SSH セッションはすでにヘッドセット上なので、この確認は通ります。

そのあと SteamVR のダッシュボードを開き、**Frame Audio Mixer** を選びます。
