<p align="center">
  <img src="assets/logo/logo_banner_blue.svg" alt="Frame Audio Mixer" width="640">
</p>
<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/github/license/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="License"></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/version-1.0.0-33C4FF?style=flat-square" alt="Version 1.0.0"></a>
  <a href="https://github.com/marcus-universe/frame-audio-mixer/stargazers"><img src="https://img.shields.io/github/stars/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="GitHub stars"></a>
</p>

[English](README.md) · **日本語** · [简体中文](README.sc.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Italiano](README.it.md)

# Frame Audio Mixer

Steam Frame 用の SteamVR ダッシュボードパネルです。アプリの音をヘッドセットのマイクと混ぜ、そのミックスをワイヤレス配信で PC に送ります。PC では **Steam Streaming Microphone** として表示されます。

このアプリは [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner) の後段で動きます。エコー除去とノイズ抑制は、引き続き Frame Mic Tuner が担当します。

## できること

- **仮想オーディオデバイス**: 名前付きのチャンネルを最大 32 個作れます。それぞれが出力（`Frame Mix: <name>`）になり、どのアプリからも選べます。
- **アプリのチャンネル**: 実行中のアプリからチャンネルを作れます。名前の横にそのアプリのアイコンが表示されます。
- **PC 配信マイクのミックス**: PC にはマイクとチャンネルの音が届きます。
- **チャンネルごとに 2 本のフェーダー**: ヘッドセット用と PC 用があり、それぞれにミュートがあります。フェーダーは 20、50、80、100 % に吸着します。
- **マイク**: ミックスに使うマイクを選べます。システムの既定マイクは変わりません。
- **レベルメーター**をすべてのフェーダーに表示します。オフにもできます。
- **ミックス確認**: PC に届く音を最大 10 秒録音して再生します。
- 7 つの言語と、青またはピンクのカラーテーマに対応しています。

## インストール

Frame の Konsole（下のバーの **+**、プログラムの一覧、**Konsole** の順に選びます）で、次を実行します:

```sh
curl -fsSL https://raw.githubusercontent.com/marcus-universe/frame-audio-mixer/main/scripts/get.sh | sh
```

- 先に開発者モードをオンにしてください。Steam の設定、システム、**開発者モードを有効化** の順に選びます。オンにするまで、Konsole は一覧に出ません。
- パネルを SteamVR と一緒に起動するには、コマンドの最後を `| sh -s -- --autostart` にします。
- 更新するには、同じコマンドをもう一度実行します。アンインストールするには、最後を `| sh -s -- --uninstall` にします。
- `sudo` は使わず、書き込むのはホームディレクトリだけです。PipeWire や WirePlumber を再起動しないので、実行中の VR セッションの音は途切れません。

## 使い方

1. SteamVR のダッシュボードを開き、**Frame Audio Mixer** を選びます。
2. **ミックス** をオンにします。
3. **+** を押してチャンネルを追加します。候補、自分で付けた名前、実行中のアプリから選べます。
4. そのアプリで、出力 `Frame Mix: <name>` を選びます。
5. スピーカーのフェーダーは、そのチャンネルをヘッドセットでどのくらいの音量で聞くかを決めます。マイクのフェーダーは、PC にどれだけ届くかを決めます。

Frame のスピーカーを使う場合は、Frame Mic Tuner でエコー除去をオンにしてください。オフのままだと、PC にアプリの音が二重に届くことがあります。

## ドキュメント

- [しくみ](docs/how-it-works.ja.md): オーディオグラフと、VRLink とアプリの音の経路。
- [技術的な詳細](docs/technical.ja.md): インストールされるもの、マイクとレベルメーターの詳細、アプリと更新、アンインストール、ビルド。

## クレジット

パネルと OpenVR オーバーレイは、sasaken1102r による [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner)（MIT）を元にしています。アプリ一覧は同じプロジェクトの frame-apps 0.3.1 を Rust に移植したもので、アップデータもその動きに合わせています。[`NOTICE`](NOTICE) を見てください。

- [アイコンとそのライセンス](docs/icons.ja.md)

## ライセンス

[MIT](LICENSE)
