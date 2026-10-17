# Frame Audio Mixer

Frame Audio Mixer は、名前を付けたアプリの出力を Steam Frame のマイクと混ぜ、そのミックスをワイヤレス接続で PC に送ります。PC では **Steam Streaming Microphone** として聞こえます。

このアプリは [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner) の後段で動きます。エコー除去とノイズ抑制は Frame Mic Tuner のままです。マイク用のトラッカーは置き換えません。

## できること

- Music やゲームなど、最大 8 個の名前付き出力を追加します。
- それぞれの出力をヘッドセットへ送り、別の音量でマイクのミックスにも入れます。
- ミックスがオンのあいだ、VRLink の録音だけをそのミックスに向けます。ほかのアプリは通常の既定マイクのままです。
- ミックスをオンにするまでグラフは動きません。オフにすると、PC には通常のマイクが届きます。

## インストール

ヘッドセット上で、通常のユーザーとして実行します。`sudo` は使わず、書き込みはホームディレクトリだけです。

git のチェックアウトからも、リリースペッケージからも同じです。

```bash
./install.sh
./install.sh --autostart
```

`--autostart` はパネルを SteamVR と一緒に起動します。ミックスのグラフは、パネルでオンにするまで止まります。

`install.sh` は PipeWire も WirePlumber も再起動しません。新しい WirePlumber スクリプトは、次に WirePlumber が起動したときに読み込まれます。VR セッション中には再起動しないでください。再生中に PipeWire や WirePlumber を再起動すると、SteamVR と Steam Link の音が切れます。

入るものは別のアプリです。

- バイナリ `~/.local/bin/frame-audio-mixer`
- ユーザーユニット `frame-audio-mixer.service`、`frame-audio-mixer-graph.service`、`frame-audio-mixer-channel@.service`
- `frame_amix_*` という名前の PipeWire ノード
- WirePlumber コンポーネント `custom.frame-audio-mixer`
- 設定 `frame-audio-mixer.enabled` と `frame-audio-mixer.vrlink-gain`
- ダッシュボードのオーバーレイキー `marcus-universe.frame-audio-mixer`

以前の実験で `frame-mic-mix` のユニットが残っていれば、`install.sh` が削除します。`frame-mic.*` のうち触るのは、残っていた `frame-mic.mix` の削除だけです。`frame-mic-tracker.lua` と `90-frame-mic.conf` は上書きしません。

## 使い方

SteamVR のダッシュボードから **Frame Audio Mixer** を開きます。起動中にもう一度起動すると、同じパネルが開きます。

1. ミックスをオンにします。
2. チャンネルを追加します（最大 8）。名前は 1 行、32 文字までで、大文字小文字を無視して一意です。
3. アプリ側で出力 `Frame Mix: <name>` を選びます。
4. **ヘッドホン** はそのチャンネルのヘッドセット側の音量です。**マイクへ** は PC にどれだけ送るかです。マイクのフェーダーとミュートは、このミックスだけを変え、システムの既定マイクは変えません。

**ミックス確認** は **Frame Mic + Mix** を最大 10 秒録音し、今の出力で再生します。フッターで言語（日本語、English、简体中文、Deutsch、Français、Español、Italiano）を選び、SteamVR と一緒に起動するか、終了します。**アプリと更新** ではこのアプリのリリースを確認し、Frame Mic Tuner へのリンクもあります。

## しくみ

マイクは、そのときの既定のソースです。Frame では、別のマイクを選んでいなければ、Valve のフィルタを通った内蔵マイクです。

ミックスがオンのとき、`frame-audio-mixer-graph.service` が中核を作ります。

- `frame_amix_from_mic` が既定ソースを録音します。
- `frame_amix_mic_virtual_sink` がマイクのフェーダーで、`frame_amix_bus` に入ります。
- `frame_amix_source`（**Frame Mic + Mix**）が、VRLink の録音元です。

チャンネルはそれぞれ `frame-audio-mixer-channel@chN.service` です。

- `frame_amix_chN` はアプリが選ぶシンクです（`Frame Mix: <name>`）。
- `frame_amix_chN_virtual_sink_playback` はそのシンクをヘッドセットで再生します。ヘッドホンのフェーダーです。
- `frame_amix_chN_virtual_sink_to_bus` は同じシンクをミックスバスへ送ります。マイクへのフェーダーです。

内部の再生ノード名には `virtual` と `sink` の両方が入っているので、スペーシャライザが包み込んでミックスをスピーカーへ二重に出しません。チャンネルのシンクは `alsa_output` でも `bluez_output` でもないので、Frame Mic Tuner の出力一覧には出ません。

WirePlumber コンポーネント `custom.frame-audio-mixer` は、`frame-audio-mixer.enabled` が true のあいだ、VRLink の録音を `object.serial` で `frame_amix_source` に固定します。メタデータにノード名を書いてもストリームは動きません。ソースが無いときは、VRLink は通常の既定ソースのままです。このスクリプトは `frame-mic.echo-cancel` と `frame-mic.noise-suppression` を変えません。

パネルも同じ方法で、シンクの `object.serial` を使ってアプリをチャンネルに固定します。チャンネルを再起動すると serial が変わるので、そのときピンを書き直します。このアプリは `wpctl set-default` を呼びません。

`frame-audio-mixer.vrlink-gain` の既定値は `1.0` です。その値では VRLink の音量を変えません。別のゲインは保存され、ミックスをオフにしたときに戻します。ミックスをオフにするとグラフのユニットが止まり、`frame_amix_*` のノードは消えます。

## エコー除去

Frame のスピーカーを使うときは、Frame Mic Tuner でエコー除去をオンにしてください。オフのままだと、マイクがスピーカーを拾い、スピーカーはすでにミックスを再生しているので、PC にアプリの音が二重に届くことがあります。ヘッドホン、またはスピーカーを拾わないマイクなら、その音響経路はありません。外部マイクを選んだ場合も、既定ソースはそちらです。このアプリはそのソースに従い、Frame Mic Tuner のスイッチは置き換えません。

## アンインストール

```bash
./install.sh --uninstall
./install.sh --uninstall --purge
```

`--purge` を付けない場合、設定は `~/.config/frame-audio-mixer` に残ります。`--purge` は保存された `frame-audio-mixer.*` の値も消します。WirePlumber のスクリプトはディスクからは消えますが、次に WirePlumber が起動するまで読み込まれたままです。VR セッション中には再起動しないでください。

## ビルド

ヘッドセットでは `rustc` と `cargo` は `~/.cargo/bin` にあります。ここで使うシステムパッケージは cairo、freetype、Vulkan、PipeWire、libclang です。

```bash
export PATH="$HOME/.cargo/bin:$PATH"
python3 scripts/make-icons.py
cargo build --release
./target/release/frame-audio-mixer --self-test
./target/release/frame-audio-mixer --language ja --dump-png /tmp/frame-audio-mixer.png
./scripts/package.sh
```

`scripts/package.sh` は `dist/frame-audio-mixer-<version>.tar.gz` と `dist/SHA256SUMS` を書き出します。

2026-10-17 のこのヘッドセットでは、`cargo build --release` が完了し、`--self-test` は `self-test ok` と表示しました。`ja`、`en`、`sc`、`de`、`fr`、`es`、`it` の `--dump-png` は 1200×788 の画像でした。どの画像もフッターの右端の画素は x=1152 で、言語のフッターが 1200px のパネルを超えることはありませんでした。

## マイク、メーター、アプリ

マイクの選択はミックスだけに効きます。`wpctl set-default` は呼びません。「システムの既定」はシステムのソースに従います。「Frame 内蔵マイク」は `alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source` で、Frame Mic Tuner の後にある Valve の鎖の終わりです。`frame-audio-mixer.mic-source` は、待機中に WirePlumber が再起動してから読み込まれます。

メーターは、パネルが見えていて、ミックスがオンで、言語の隣の歯車（アプリ設定）でメーターがオンのときだけ動きます。各タップは `node.virtual=true` の `pw-record` です。チャンネルは passive、マイクは passive ではありません。表示中は 15 fps で描きます。

アプリと更新には、このアプリ、関連アプリとしての Frame Mic Tuner、sasaken の Frame アプリが出ます。インストールは Konsole を開きます。すでに動いている Frame Mic Tuner にはインストールボタンがありません。

アップデータは Rust です（`ureq` と rustls、`sha2`、`flate2`、`tar`）。`frame-update.sh` は入れません。確認は GitHub の最新リリースを読みます。インストールはバイナリをコピーし、`systemd-run --user --unit=frame-audio-mixer-update` で起動します。ダウンロードは GitHub の HTTPS だけ、256 MB まで、SHA-256 が一致し、tar に絶対パス、`..`、リンク、デバイスがあってはなりません。`scripts/get.sh` は `curl`、`sha256sum`、`tar` が必要です。リリースが出るまでは「まだリリースがない」と表示します。`scripts/package.sh` は同じ内容の `dist/frame-audio-mixer-aarch64.tar.gz` も書きます。

## クレジット

パネルと OpenVR オーバーレイは、sasaken1102r による Frame Mic Tuner（MIT）を移植したものです。https://github.com/sasaken1102r/frame-mic-tuner

アプリ一覧は frame-apps 0.3.1 の Rust 移植です。そのアイコンは MIT です。アップデータの動きは同じプロジェクトに合わせ、Rust で書き直しています。`NOTICE` を見てください。

## アイコン

`scripts/make-icons.py` がダッシュボード用アイコンを描きます。録音、停止、再生は描いた形です。ほかの字形は埋め込んだパスです。

| アイコン | セット | 作者 | ライセンス |
| --- | --- | --- | --- |
| [fontisto:mic](https://icon-sets.iconify.design/fontisto/mic/) | fontisto | Kenan Gündoğan | MIT |
| [famicons:mic-off](https://icon-sets.iconify.design/famicons/mic-off/) | famicons | Family | MIT |
| [f7:speaker-2-fill](https://icon-sets.iconify.design/f7/speaker-2-fill/) | Framework7 | Vladimir Kharlampidi | MIT |
| [f7:speaker-slash-fill](https://icon-sets.iconify.design/f7/speaker-slash-fill/) | Framework7 | Vladimir Kharlampidi | MIT |
| [fluent:add-square-28-filled](https://icon-sets.iconify.design/fluent/add-square-28-filled/) | Fluent | Microsoft | MIT |
| [heroicons:language-16-solid](https://icon-sets.iconify.design/heroicons/language-16-solid/) | Heroicons | Refactoring UI Inc | MIT |
| [at-icons:headphones](https://icon-sets.iconify.design/at-icons/headphones/) | @icons | Valentin Fossati | MIT |
| [material-symbols:settings-rounded](https://icon-sets.iconify.design/material-symbols/settings-rounded/) | Material Symbols | Google | Apache-2.0 |

## ヘッドセットでの確認（2026-10-17）

`steamvr.service` と `gamescope-session.service` が active のときに測りました。WirePlumber は再起動していません。新しいスクリプトはインストール済みで、まだ読み込まれていません。

- `~/.local/bin/frame-audio-mixer` は入りました。`frame-audio-mixer.service` は enabled で inactive です。`frame-audio-mixer-graph.service` は disabled で inactive です。`frame-mic-tuner.service` は enabled で active です。
- `frame-mic-tracker.lua` と `90-frame-mic.conf` はそのままで、`frame_amix` は出てきません。`91-frame-audio-mixer.conf` と `frame-audio-mixer.lua` はインストール済みです。`frame-mic-mix.service` のユニットはありません。
- `frame-mic.echo-cancel` は false（保存値も false）です。`frame-mic.noise-suppression` は true（保存値も true）です。`frame-mic.mix` はまだ一覧にあり、値は false（保存値も false）です。動いている WirePlumber がキーを残しています。`frame-audio-mixer.enabled` と `frame-audio-mixer.vrlink-gain` は、動いている設定にはまだありません。
- `pw-dump` のノードは 19 個で、`frame_amix_*`、`frame_mix_*`、`frame_micmix*` はありませんでした。キャプチャは `eq_capture`、`echo_cancel_capture`、`ns_capture`、ALSA のマイクループバックです。VRLink のキャプチャストリームは無く、音量は測っていません。既定のソースは `alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source`、既定のシンクは `alsa_loopback_device.stereo.alsa_output.platform-sound.HiFi__Speaker__sink` でした。
- 聞こえるかどうかの確認（マイク、エコーの二重、外部マイク）はしていません。PC の前に人が必要で、この VR セッション中はミックスのグラフを起動していません。
