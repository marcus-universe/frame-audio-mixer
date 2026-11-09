[English](technical.md) · **日本語** · [简体中文](technical.sc.md) · [Deutsch](technical.de.md) · [Français](technical.fr.md) · [Español](technical.es.md) · [Italiano](technical.it.md)

[← README](../README.ja.md)

# 技術的な詳細

## インストールされるもの

1 行のインストールは [`scripts/get.sh`](../scripts/get.sh) を実行します。最新のリリース（`frame-audio-mixer-aarch64.tar.gz`）をダウンロードし、`SHA256SUMS` で確認してから、その `install.sh` を実行します。リリースがまだ公開されていないあいだは、代わりに `main` ブランチをダウンロードしてビルドします。その場合は、ヘッドセットに Rust（`cargo`）が必要です。`curl`、`sha256sum`、`tar` が必要です。`sh -s --` の後のオプションは、`install.sh` に渡されます。

`install.sh` はホームディレクトリの下にだけ書き込み、`sudo` は使いません:

- バイナリ `~/.local/bin/frame-audio-mixer`
- ユーザーユニット `frame-audio-mixer.service` と `frame-audio-mixer-engine.service`
- WirePlumber スクリプト `~/.local/share/wireplumber/scripts/frame-audio-mixer.lua` と `~/.config/wireplumber/wireplumber.conf.d/91-frame-audio-mixer.conf`（コンポーネント `custom.frame-audio-mixer`）
- `~/.local/share` の下のデスクトップエントリとアイコン
- `~/.config/frame-audio-mixer/` の設定と、`~/.config/pipewire/frame-audio-mixer/<id>.name` のチャンネル名

実行中、アプリは `frame_amix_*` という名前の PipeWire ノード、WirePlumber の設定 `frame-audio-mixer.enabled`、`frame-audio-mixer.vrlink-gain`、`frame-audio-mixer.mic-source`、ダッシュボードのオーバーレイ `marcus-universe.frame-audio-mixer` を追加します。

`install.sh` は PipeWire も WirePlumber も再起動しません。新しい、または変更された WirePlumber スクリプトは、次に WirePlumber が起動したときに読み込まれます。ヘッドセットの再起動は、VR セッション中でないときだけにしてください。プレイ中に PipeWire や WirePlumber を再起動すると、SteamVR と Steam Link の音が切れます。それまでのあいだもチャンネルは使えます。パネルが動いているあいだは、パネルもストリームを固定するためです。

以前の実験で `frame-mic-mix` のユニットが残っていれば、`install.sh` が削除します。`frame-mic.*` のうち触るのは、残っている `frame-mic.mix` だけです。Frame Mic Tuner の `frame-mic-tracker.lua` と `90-frame-mic.conf` は上書きしません。

git のチェックアウトからは、`./install.sh` が `cargo` でビルドしてインストールします。`./install.sh --autostart` は、パネルを SteamVR と一緒に起動するようにもします。使ったオプションは `~/.config/frame-audio-mixer/install-args` に保存されるので、更新も同じ方法でインストールされます。

## チャンネル

- チャンネルは最大 32 個で、`ch1` から `ch32` です。追加や削除は `frame-audio-mixer-engine.service` の中で、そのチャンネルのノードだけを作ります。
- 名前は 1 行、32 文字までで、大文字小文字を無視して一意です。
- 実行中のアプリから作ったチャンネルには、そのアプリの名前が付きます。そのアプリをチャンネルに固定し、`.desktop` エントリにあるアプリのアイコンを表示します。アイコンが無い場合は、最初の 1 文字を表示します。
- 行がパネルより広いときは、コントローラーのスティックでスクロールするか、チャンネルの下のバーをドラッグします。
- 各ストリップにはフェーダーが 1 本あり、無音から +12 dB までで、0 dB で吸着します。その下にミュートとソロがあります。ルーティングのボタンはフェーダーの横に縦に並びます。A1 から A4（聞く音）、PC（PC 配信マイク）、B1 から B4（仮想マイク）、その後に FX1 と FX2 です。出力が 6 個か 8 個のときは、A と B のボタンが 2 列に分かれます。リスト表示では、ミュートとソロの横に 1 行で並びます。

ピン留めされていないチャンネルに乗ったストリームは戻されます: 自分のチャンネルへ、またはミキサーの外へ移され、通常の出力で再生されます。ピンはバイナリが分かる場合はバイナリで照合するので、同じ表示名を共有するアプリ（Electron アプリはすべて “Chromium” と名乗ります）が互いのチャンネルを奨うことはありません。

## プロジェクト

ヘッダーのフォルダーボタンに保存済みのルーティングセットアップがあります。プロジェクトはチャンネル、ピン、ルート、エフェクトチェーン、レベルを 1 つの JSON ファイルとして `~/.config/frame-audio-mixer/projects/` に保存します。読み込むと現在のセットアップを置き換えますが、言語・テーマ・レイアウトなどの表示設定はそのままです。

## 出力

PC 配信マイクは常にあります。アプリ設定で、出力を 2、4、6、8 個追加します。2 は A1 と B1、8 は A1 から A4 と B1 から B4 です。数を減らしても、保存されたルートは残りますが、接続はされません。

- PC は `frame_amix_source` で、**Frame Mix** と表示されます。VRLink がこれを録音するので、PC には Steam Streaming Microphone として見えます。
- A1 はヘッドセットのスピーカーです。A2 から A4 には、Bluetooth などほかの任意のシンクを使えます。
- B1 から B4 は、ヘッドセットのアプリ向けの仮想マイク（`frame_amix_b1` から `frame_amix_b4`）です。それぞれに名前があります。

Outputs ドロワー（右のレールのスピーカーアイコン）には、一番上にミックス確認があり、その下に出力ごとのコンテナが並びます。各コンテナには名前、その下にマスターフェーダー、右側にミュートがあります。PC が最初です。

バージョン 2 の設定は、最初の読み込みで移行されます。古い B1 は PC になり（フェーダーとミュートも引き継ぎます）、古い B2 から B4 は B1 から B3 になります。

## 信号フロー

信号フローのページ（ヘッダーの「アプリと更新」の隣）は、ミックス全体をレーン状のグラフとして描きます: ソース、ミキサーのチャンネル、出力、そしてその先のデバイス。線は保存されたルーティングから作られ、それぞれがエンジンの実際の PipeWire リンクと照合されます — リンクが消えたルートは赤い破線になります。音が流れているリンクは、レベル色の光で太く見えます。ドラッグで移動、スティックでスクロール、ノードを選ぶと背後の PipeWire ノードが表示されます。「その他のアプリ」トグルを有効にすると、どのチャンネルにも属さないアプリも、実際に再生しているデバイスと一緒に表示されます。

## マイク

マイクの選択は、ミックスだけに使います。このアプリは `wpctl set-default` を呼ばないので、Frame Mic Tuner が使うシステムの既定はそのままです。

- **システムの既定** は `default.audio.source` に従います。
- **Frame 内蔵マイク** は `alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source` で、ノイズ抑制、エコー除去、マイク EQ の後にある Valve のチェーンの終わりです。
- 一覧にあるほかのソースは、`node.name` で保存します。

選択は `frame-audio-mixer.mic-source`（`""`、`builtin`、またはノード名）として保存されます。スクリプトは `builtin` を上のループバックに解決し、そのノードが無ければシステムの既定に戻します。`frame_amix_source` は使いません。変更した値は、次に WirePlumber が起動した後に有効になります。

エンジンは、仮想ではないループバックでそのソースをキャプチャするので、Mix がオンのあいだ Valve のノイズ抑制は動いたままです。マイクストリップは、フェーダーが 0 dB のときに 12 dB のトリムを加えます。既定の行き先は PC だけです。

## レベルメーター

メーターが動くのは、ダッシュボードが表示されていて、ミックスがオンで、アプリ設定（言語メニューの隣の歯車）でメーターがオンのときだけです。メーターを測るのは、画面に見えているチャンネルだけです。各メーターは `node.virtual=true` の `pw-record` のタップなので、Valve のマイクフィルタをオンにしません。チャンネルのタップはパッシブです。マイクのタップはパッシブではありません。メーターが動いているあいだ、パネルは約 30 fps で再描画します。ダッシュボードが隠れているあいだは描画しません。

## エフェクト

FX1 と FX2 は共有のセンドで、それぞれ最大 3 つのエフェクトのチェーンを持ちます。EQ はその 1 つで、3、6、8、12 ポイントのパラメトリックイコライザーです。スロットの編集で開きます。外側のポイントは、ベル、シェルフ、ハイパス（低い側のポイント）、ローパス（高い側のポイント）にできます。センドはドライの経路をそのまま残します。各リターンには、専用のフェーダーと A、PC、B のルートがあります。処理はエンジンの中で、float32、48 kHz、480 フレームで行います。センドを切り替えても、PipeWire や WirePlumber は再起動しません。

## アプリと更新

**アプリと更新** には、このアプリ、関連アプリとしての Frame Mic Tuner、sasaken のカタログにある Frame アプリが表示されます。**インストール** は Konsole を開き、`curl -fsSL https://frame.sasaken1102s.net | sh` を実行します。すでに動いている Frame Mic Tuner にはインストールボタンが無いので、このページがそのフォークを置き換えることはありません。

アップデータは Rust で書かれています（`ureq` と rustls、`sha2`、`flate2`、`tar`）:

- 確認では GitHub の最新リリースを読み、バージョンを比べ、`frame-audio-mixer-<version>.tar.gz` と `SHA256SUMS` を探します。
- インストールでは、実行中のバイナリを `~/.cache/frame-audio-mixer/update/` にコピーし、`systemd-run --user --unit=frame-audio-mixer-update --collect` で起動します。
- ダウンロードは HTTPS だけで、`github.com`、`api.github.com`、`codeload.github.com`、`*.githubusercontent.com` からに限ります。ただし、`FRAME_UPDATE_ALLOW_INSECURE=1` が設定されている場合は除きます。リダイレクトも確認し、ダウンロードは 256 MB で止まります。SHA-256 は一致する必要があります。
- 絶対パス、`..`、リンク、デバイスノードを含むアーカイブは拒否します。
- キャッシュファイルは `~/.cache/frame-audio-mixer/` にあります（`update-check.json`、`update-state.json`、`update.log`、`update.lock/`、`update/`）。

## アンインストール

1 行のコマンドでは:

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/i | sh -s -- --uninstall
```

チェックアウトまたはリリースのフォルダからは:

```bash
./install.sh --uninstall
./install.sh --uninstall --purge
```

`--purge` を付けない場合、設定は `~/.config/frame-audio-mixer` に残ります。`--purge` は、設定と保存された `frame-audio-mixer.*` の値も削除します。WirePlumber スクリプトはディスクから削除されますが、次に WirePlumber が起動するまで読み込まれたままです。

## ビルド

ヘッドセットでは、`rustc` と `cargo` は `~/.cargo/bin` にあります。使うシステムライブラリは cairo、freetype、Vulkan、PipeWire、libclang です。

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release
./target/release/frame-audio-mixer --self-test
./target/release/frame-audio-mixer --language en --dump-png /tmp/frame-audio-mixer.png
./scripts/package.sh
```

`--dump-png` は、メイン画面とほかの画面（アプリ、追加、チャンネル選択、設定、言語メニュー、録音中にスクロールした行）を、指定したパスの隣に書き出します。

`scripts/package.sh` は、`dist/frame-audio-mixer-<version>.tar.gz`、それと同じ内容の `dist/frame-audio-mixer-aarch64.tar.gz`、両方を載せた `dist/SHA256SUMS` を書き出します。3 つとも GitHub のリリースにアップロードすると、1 行のインストールとアプリ内のアップデータがそれを使います。
