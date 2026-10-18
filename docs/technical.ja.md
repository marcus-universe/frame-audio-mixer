[English](technical.md) · **日本語** · [简体中文](technical.sc.md) · [Deutsch](technical.de.md) · [Français](technical.fr.md) · [Español](technical.es.md) · [Italiano](technical.it.md)

[← README](../README.ja.md)

# 技術的な詳細

## インストールされるもの

1 行のインストールは [`scripts/get.sh`](../scripts/get.sh) を実行します。最新のリリース（`frame-audio-mixer-aarch64.tar.gz`）をダウンロードし、`SHA256SUMS` で確認してから、その `install.sh` を実行します。リリースがまだ公開されていないあいだは、代わりに `main` ブランチをダウンロードしてビルドします。その場合は、ヘッドセットに Rust（`cargo`）が必要です。`curl`、`sha256sum`、`tar` が必要です。`sh -s --` の後のオプションは、`install.sh` に渡されます。

`install.sh` はホームディレクトリの下にだけ書き込み、`sudo` は使いません:

- バイナリ `~/.local/bin/frame-audio-mixer`
- ランナー `~/.local/lib/frame-audio-mixer/frame-audio-mixer-graph.sh`
- ユーザーユニット `frame-audio-mixer.service`、`frame-audio-mixer-graph.service`、`frame-audio-mixer-channel@.service`
- WirePlumber スクリプト `~/.local/share/wireplumber/scripts/frame-audio-mixer.lua` と `~/.config/wireplumber/wireplumber.conf.d/91-frame-audio-mixer.conf`（コンポーネント `custom.frame-audio-mixer`）
- `~/.local/share` の下のデスクトップエントリとアイコン
- `~/.config/frame-audio-mixer/` の設定と、`~/.config/pipewire/frame-audio-mixer/<id>.name` のチャンネル名

実行中、アプリは `frame_amix_*` という名前の PipeWire ノード、WirePlumber の設定 `frame-audio-mixer.enabled`、`frame-audio-mixer.vrlink-gain`、`frame-audio-mixer.mic-source`、ダッシュボードのオーバーレイ `marcus-universe.frame-audio-mixer` を追加します。

`install.sh` は PipeWire も WirePlumber も再起動しません。新しい、または変更された WirePlumber スクリプトは、次に WirePlumber が起動したときに読み込まれます。ヘッドセットの再起動は、VR セッション中でないときだけにしてください。プレイ中に PipeWire や WirePlumber を再起動すると、SteamVR と Steam Link の音が切れます。それまでのあいだもチャンネルは使えます。パネルが動いているあいだは、パネルもストリームを固定するためです。

以前の実験で `frame-mic-mix` のユニットが残っていれば、`install.sh` が削除します。`frame-mic.*` のうち触るのは、残っている `frame-mic.mix` だけです。Frame Mic Tuner の `frame-mic-tracker.lua` と `90-frame-mic.conf` は上書きしません。

git のチェックアウトからは、`./install.sh` が `cargo` でビルドしてインストールします。`./install.sh --autostart` は、パネルを SteamVR と一緒に起動するようにもします。使ったオプションは `~/.config/frame-audio-mixer/install-args` に保存されるので、更新も同じ方法でインストールされます。

## チャンネル

- チャンネルは最大 32 個で、`ch1` から `ch32` です。それぞれが 1 つの systemd インスタンスで、`pw-loopback` のプロセスを 2 つ持ちます。
- 名前は 1 行、32 文字までで、大文字小文字を無視して一意です。
- 実行中のアプリから作ったチャンネルには、そのアプリの名前が付きます。そのアプリをチャンネルに固定し、`.desktop` エントリにあるアプリのアイコンを表示します。アイコンが無い場合は、最初の 1 文字を表示します。
- 行がパネルより広いときは、コントローラーのスティックでスクロールするか、チャンネルの下のバーをドラッグします。
- フェーダーは 0 から 150 % です。ドラッグ中に 8 ピクセル以内に近づくと、20、50、80、100 % に吸着します。フェーダーの点は 20、50、80 % を示し、線は 100 % を示します。

## マイク

マイクの選択は、ミックスだけに使います。このアプリは `wpctl set-default` を呼ばないので、Frame Mic Tuner が使うシステムの既定はそのままです。

- **システムの既定** は `default.audio.source` に従います。
- **Frame 内蔵マイク** は `alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source` で、ノイズ抑制、エコー除去、マイク EQ の後にある Valve のチェーンの終わりです。
- 一覧にあるほかのソースは、`node.name` で保存します。

選択は `frame-audio-mixer.mic-source`（`""`、`builtin`、またはノード名）として保存されます。スクリプトは `builtin` を上のループバックに解決し、そのノードが無ければシステムの既定に戻します。`frame_amix_source` は使いません。変更した値は、次に WirePlumber が起動した後に有効になります。

## レベルメーター

メーターが動くのは、ダッシュボードが表示されていて、ミックスがオンで、アプリ設定（言語メニューの隣の歯車）でメーターがオンのときだけです。メーターを測るのは、画面に見えているチャンネルだけです。各メーターは `node.virtual=true` の `pw-record` のタップなので、Valve のマイクフィルタをオンにしません。チャンネルのタップはパッシブです。マイクのタップはパッシブではありません。メーターが動いているあいだ、パネルは約 30 fps で再描画します。ダッシュボードが隠れているあいだは描画しません。

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
curl -fsSL https://frame-audio-mixer.marcus-universe.com/frame-audio-mixer-install | sh -s -- --uninstall
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
