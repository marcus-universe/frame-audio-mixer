<p align="center">
  <a href="https://frame-audio-mixer.marcus-universe.com/"><img src="assets/logo/logo_banner_blue.svg" alt="Frame Audio Mixer" width="640"></a>
</p>
<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/github/license/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="License"></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/version-2.0.0-33C4FF?style=flat-square" alt="Version 2.0.0"></a>
  <a href="https://frame-audio-mixer.marcus-universe.com/"><img src="https://img.shields.io/badge/Website-33C4FF?style=flat-square" alt="Website"></a>
  <a href="https://github.com/marcus-universe/frame-audio-mixer/stargazers"><img src="https://img.shields.io/github/stars/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="GitHub stars"></a>
</p>

[English](README.md) · **日本語** · [简体中文](README.sc.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Italiano](README.it.md)

# 🎚️ Frame Audio Mixer

Frame Audio Mixer を使うと、Steam Frame の中で複数のオーディオチャンネルとミックスを作れます。アプリの音とヘッドセットのマイクを混ぜ、それぞれのミックスを必要な場所へ送ります。ヘッドセット内の Frame スピーカーやヘッドホンへ、またはワイヤレス配信で PC へ。PC では、SteamVR が用意している **Steam Streaming Microphone** として届きます。

このアプリは [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner) の後段で動きます。エコー除去とノイズ抑制は、引き続き Frame Mic Tuner が担当します。

## 📥 インストール

Frame の Konsole（下のバーの **+**、プログラムの一覧、**Konsole** の順に選びます）で、次を実行します:

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/i | sh
```

- 🛠️ 先に開発者モードをオンにしてください。Steam の設定、システム、**開発者モードを有効化** の順に選びます。オンにするまで、Konsole は一覧に出ません。
- 💻 PC から入れる場合: [SSH でインストール](docs/install-ssh.ja.md)。
- 🖥️ PC のオーディオデバイス（Steam Streaming Mic B1–B4 と Out PC1–PC4、Windows と Linux）: [PC にインストール](docs/install-pc.ja.md)。

## ✨ できること

- 🧩 **アプリのチャンネル**: 最大 32 個のアプリを入力として追加できます。起動中のアプリを選ぶか、名前付きの仮想出力を作ります。仮想出力は Steam Frame 上のどのアプリからも選べ、その音はミキサーに専用のチャンネルとして届きます。
- 🎤 **仮想マイク**: B1 から B4。名前を付けられ、PC の配信マイクに加わります。
- 🔗 **PCオーディオリンク**: SteamVR 用アドオン Frame Audio Link を使うと、B1 から B4 が PC 上でも 4 つのマイクになり、PC の 4 つの出力（PC1 から PC4）をチャンネルとして追加できます。ストリームごとにオフにして帯域を節約できます。
- 🎧 **スピーカー、Bluetooth、USB**: 接続中は Frame のスピーカー、Bluetooth ヘッドホン、USB オーディオで聞けます。
- 🎛️ **エフェクトバス**: FX1 と FX2。リバーブ、コンプ、EQ、ピッチ、フランジャー、フェイザー。各ストリップでどちらもオンにでき、エフェクトの結果がそのストリップの出力先でそのまま元の音に置き換わります。
- 🗂️ **プロジェクト**: 現在のチャンネル、ルーティング、エフェクト、レベルを名前を付けて保存し、いつでも読み込めます。
- 🗺️ **信号フロー**: ミックス全体のルーティングをライブ表示。各ルートは実際の PipeWire リンクと照合され、切れたリンクは赤い破線で表示されます。ドラッグで移動、ノードを選ぶと PipeWire 上の名前が見えます。
- 📊 **メーター、ミュート、スナップ**: すべてのフェーダーが +6 から −48 の同じ目盛り。各フェーダーの下にミュート。出力フェーダーは目盛りに吸着します。
- ⏺️ **ミックス確認**: 出力の一番上。出力を選んで最大 10 秒録音し、再生または一時停止します。
- 🔄 **向き**: 入力と出力は、それぞれ縦のフェーダーと横の列を切り替えられます。
- 🌐 7 つの言語と、青またはピンクのテーマ。

<table>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-1.jpg" alt="PC1 チャンネルのあるミキサー、青のテーマ" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-2.jpg" alt="ミキサー、ピンクのテーマ" height="320"></td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-3.jpg" alt="信号フロー、青のテーマ" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-4.jpg" alt="SteamVR で Mixxx の下に置いたミキサー" height="320"></td>
  </tr>
</table>

## ▶️ 使い方

1. SteamVR のダッシュボードを開き、**Frame Audio Mixer** を選びます。
2. ミックスをオンにします。PC では **Steam Streaming Microphone** として聞こえます。
3. **+** でチャンネルを追加します。名前を付けるか、実行中のアプリを選びます。
4. そのアプリで、チャンネルと同じ名前の出力を選びます。
5. 左の**入力**がミックスに入る音です。右の**出力**が行き先です。PC、Frame のスピーカー、Bluetooth、USB、仮想マイクです。

🎧 Frame のスピーカーで聞くときは、Frame Mic Tuner でエコー除去をオンにしてください。オフだと、その音が PC に二重に届くことがあります。

## 📚 ドキュメント

- 📖 [しくみ](docs/how-it-works.ja.md): オーディオグラフと、VRLink とアプリの音の経路。
- 🔧 [技術的な詳細](docs/technical.ja.md): インストールされるもの、マイクとレベルメーターの詳細、アプリと更新、アンインストール、ビルド。
- 🔗 [PCオーディオリンク](docs/pc-link.md)（英語）: PC 用の SteamVR アドオン、インストールと使い方、リンクの動作。

## 🙏 クレジット

Frame Audio Mixer は、[sasaken1102r](https://github.com/sasaken1102r) の [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner) に着想を得ています。ささけん＠の Steam Frame アプリは [frame.sasaken1102s.net](https://frame.sasaken1102s.net/) にまとまっています。

パネル、OpenVR オーバーレイ、アプリ一覧は、そのプロジェクト（MIT）を元にしています。[`NOTICE`](NOTICE) を見てください。

- 🎨 [アイコンとそのライセンス](docs/icons.ja.md)

## 📄 ライセンス

[MIT](LICENSE)
