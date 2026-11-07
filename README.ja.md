<p align="center">
  <img src="assets/logo/logo_banner_blue.svg" alt="Frame Audio Mixer" width="640">
</p>
<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/github/license/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="License"></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/version-2.0.0-33C4FF?style=flat-square" alt="Version 2.0.0"></a>
  <a href="https://frame-audio-mixer.marcus-universe.com/"><img src="https://img.shields.io/badge/Website-33C4FF?style=flat-square" alt="Website"></a>
  <a href="https://github.com/marcus-universe/frame-audio-mixer/stargazers"><img src="https://img.shields.io/github/stars/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="GitHub stars"></a>
</p>

[English](README.md) · **日本語** · [简体中文](README.sc.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Italiano](README.it.md)

# Frame Audio Mixer

Steam Frame 用の SteamVR ダッシュボードパネルです。アプリの音をヘッドセットのマイクと混ぜ、そのミックスをワイヤレス配信で PC に送ります。PC では **Steam Streaming Microphone** として表示されます。

このアプリは [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner) の後段で動きます。エコー除去とノイズ抑制は、引き続き Frame Mic Tuner が担当します。

## インストール

Frame の Konsole（下のバーの **+**、プログラムの一覧、**Konsole** の順に選びます）で、次を実行します:

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/i | sh
```

- 先に開発者モードをオンにしてください。Steam の設定、システム、**開発者モードを有効化** の順に選びます。オンにするまで、Konsole は一覧に出ません。
- パネルを SteamVR と一緒に起動するには、`| sh` の代わりに `| sh -s -- --autostart` を使います。
- 更新するには、同じコマンドをもう一度実行します。アンインストールするには、`| sh` の代わりに `| sh -s -- --uninstall` を使います。
- `sudo` は使わず、書き込むのはホームディレクトリだけです。PipeWire や WirePlumber を再起動しないので、実行中の VR セッションの音は途切れません。
- PC から入れる場合: [SSH でインストール](docs/install-ssh.ja.md)。

## できること

- **仮想オーディオデバイス**: 名前付きのチャンネルを最大 32 個作れます。それぞれが出力（`Frame Mix: <name>`）になり、どのアプリからも選べます。
- **アプリのチャンネル**: 実行中のアプリからチャンネルを作れます。名前の横にそのアプリのアイコンが表示されます。
- **PC 配信マイクのミックス**: PC にはマイクとチャンネルの音が届きます。
- **PC 出力**: PC に Steam Streaming Microphone として届く `Frame Mix` は、PC アイコンの付いた独立した出力です。PC に何が送られるかがすぐ分かります。
- **ルーティング**: チャンネルごとに 1 本のフェーダー（dB）。ミュートとソロ付き。ルーティングのボタンはフェーダーの横に縦に並びます。A は聞く側（A1 はヘッドセット）、PC は配信マイク、B は追加の仮想マイク、FX1 と FX2 はエフェクトのセンドです。設定で A と B の出力数を 2、4、6、8 から選びます。
- **エフェクトとパラメトリック EQ**: FX1 と FX2 には、それぞれ最大 3 つのエフェクトを入れられます。EQ は 3、6、8、12 ポイントで、応答カーブを表示します。外側のポイントは、ベル、シェルフ、ハイパス、ローパスにできます。
- **マイク**: ミックスに使うマイクを選べます。システムの既定マイクは変わりません。
- **レベルメーター**をすべてのフェーダーに表示します。オフにもできます。
- **ミックス確認**: Outputs ドロワーの一番上にあります。出力を選んで最大 10 秒録音し、その録音を再生または一時停止します。右のレールは Outputs（スピーカーアイコン）、FX1、FX2 です。
- 7 つの言語と、青またはピンクのカラーテーマに対応しています。

<table>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-1.png" alt="カラム表示、青のテーマ" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-2.png" alt="リスト表示、青のテーマ" height="320"></td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-3.png" alt="カラム表示、ピンクのテーマ" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-4.png" alt="リスト表示、ピンクのテーマ" height="320"></td>
  </tr>
</table>

## 使い方

1. SteamVR のダッシュボードを開き、**Frame Audio Mixer** を選びます。
2. **ミックス** をオンにします。
3. **+** を押してチャンネルを追加します。候補、自分で付けた名前、実行中のアプリから選べます。
4. そのアプリで、出力 `Frame Mix: <name>` を選びます。
5. フェーダーはストリップの音量です。A1 はヘッドセットで再生します。PC ボタンで PC に送ります。各出力の音量は Outputs ドロワーで設定します。

Frame のスピーカーを使う場合は、Frame Mic Tuner でエコー除去をオンにしてください。オフのままだと、PC にアプリの音が二重に届くことがあります。

## ドキュメント

- [しくみ](docs/how-it-works.ja.md): オーディオグラフと、VRLink とアプリの音の経路。
- [技術的な詳細](docs/technical.ja.md): インストールされるもの、マイクとレベルメーターの詳細、アプリと更新、アンインストール、ビルド。

## クレジット

Frame Audio Mixer は、[sasaken1102r](https://github.com/sasaken1102r) の [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner) に着想を得ています。ささけん＠の Steam Frame アプリは [frame.sasaken1102s.net](https://frame.sasaken1102s.net/) にまとまっています。

パネル、OpenVR オーバーレイ、アプリ一覧は、そのプロジェクト（MIT）を元にしています。[`NOTICE`](NOTICE) を見てください。

- [アイコンとそのライセンス](docs/icons.ja.md)

## ライセンス

[MIT](LICENSE)
