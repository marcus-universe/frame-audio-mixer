# Frame Audio Mixer

Frame Audio Mixer 把命名的应用输出和 Steam Frame 的麦克风混在一起，再通过无线串流送给 PC。PC 上显示为 **Steam Streaming Microphone**。

它接在 [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner) 之后。回声消除和噪声抑制仍由 Frame Mic Tuner 负责。这个应用不替换那套麦克风跟踪脚本。

## 它做什么

- 最多添加八路命名输出，例如音乐或游戏。
- 每一路都送到头显，并以各自的音量进入麦克风混音。
- 混音打开时，只把 VRLink 的采集指向这份混音。其他应用仍使用原来的默认麦克风。
- 在你打开混音之前，音频图不运行。关掉之后，PC 听到的又是原来的麦克风。

## 安装

在头显上以普通用户运行。不使用 `sudo`，只写入主目录。

git 检出和发布包的用法相同：

```bash
./install.sh
./install.sh --autostart
```

`--autostart` 还会让面板随 SteamVR 启动。混音图保持关闭，直到你在面板里打开混音。

`install.sh` 不会重启 PipeWire 或 WirePlumber。新的 WirePlumber 脚本在 WirePlumber 下次启动时加载。请只在没有 VR 会话时这样做。播放时重启 PipeWire 或 WirePlumber 会让 SteamVR 和 Steam Link 失去声音。

安装的是一个独立应用：

- 二进制文件 `~/.local/bin/frame-audio-mixer`
- 用户单元 `frame-audio-mixer.service`、`frame-audio-mixer-graph.service` 和 `frame-audio-mixer-channel@.service`
- 名为 `frame_amix_*` 的 PipeWire 节点
- WirePlumber 组件 `custom.frame-audio-mixer`
- 设置 `frame-audio-mixer.enabled` 和 `frame-audio-mixer.vrlink-gain`
- 仪表盘 overlay 键 `marcus-universe.frame-audio-mixer`

如果以前的实验留下了 `frame-mic-mix` 单元，`install.sh` 会删掉它们。它唯一改动的 `frame-mic.*` 值，是删除残留的 `frame-mic.mix`。它不会覆盖 `frame-mic-tracker.lua` 或 `90-frame-mic.conf`。

## 使用

在 SteamVR 仪表盘中打开 **Frame Audio Mixer**。程序已在运行时再启动一次，会打开同一个面板。

1. 打开混音。
2. 添加通道（最多 8 个）。名称占一行，最多 32 个字符，忽略大小写后必须唯一。
3. 在应用里选择输出 `Frame Mix: <name>`。
4. **耳机** 是这一路在头显里的音量。**到麦克风** 是 PC 能听到多少。麦克风推子和静音只影响这份混音，不改变系统默认麦克风。

**混音检查** 最多录制 10 秒 **Frame Mic + Mix**，然后在当前输出上播放。页脚用来选择语言（日本語、English、简体中文、Deutsch、Français、Español、Italiano）、随 SteamVR 启动，以及退出。**应用和更新** 检查本应用的发布，并链到 Frame Mic Tuner。

## 工作方式

麦克风就是当前的默认源。在 Frame 上，如果没有另选麦克风，这就是经过 Valve 滤波器之后的内置麦克风。

混音打开时，`frame-audio-mixer-graph.service` 建立核心：

- `frame_amix_from_mic` 采集默认源。
- `frame_amix_mic_virtual_sink` 是麦克风推子，送入 `frame_amix_bus`。
- `frame_amix_source`（**Frame Mic + Mix**）是 VRLink 录制的源。

每个通道是自己的 `frame-audio-mixer-channel@chN.service`：

- `frame_amix_chN` 是应用选择的 sink（`Frame Mix: <name>`）。
- `frame_amix_chN_virtual_sink_playback` 把这个 sink 播放到头显。这是耳机推子。
- `frame_amix_chN_virtual_sink_to_bus` 把同一个 sink 送进混音总线。这是到麦克风的推子。

这些内部播放节点的名字同时包含 `virtual` 和 `sink`，空间化不会把它们包起来、再把混音送一次到扬声器。通道 sink 不是 `alsa_output` 或 `bluez_output`，所以不会出现在 Frame Mic Tuner 的输出列表里。

WirePlumber 组件 `custom.frame-audio-mixer` 在 `frame-audio-mixer.enabled` 为 true 时，用 `object.serial` 把 VRLink 的采集固定到 `frame_amix_source`。元数据里的节点名不会移动这条流。如果源不存在，VRLink 继续使用原来的默认源。这个脚本不修改 `frame-mic.echo-cancel` 或 `frame-mic.noise-suppression`。

面板用同样的方式，以 sink 的 `object.serial` 把应用固定到通道。通道重启后 serial 会变，固定会再写一次。这个应用不会调用 `wpctl set-default`。

`frame-audio-mixer.vrlink-gain` 的默认值是 `1.0`。这个值不会改变 VRLink 的音量。其他增益会被保存，并在关闭混音时恢复。关闭混音会停止音频图单元，`frame_amix_*` 节点随之消失。

## 回声消除

使用 Frame 扬声器时，请在 Frame Mic Tuner 里打开回声消除。否则麦克风会听到扬声器，而扬声器已经在播放混音，PC 就可能听到两遍应用的声音。耳机，或者听不到扬声器的麦克风，没有这条声学路径。如果选择了外部麦克风，默认源就是它。本应用跟随那个源，不替换 Frame Mic Tuner 的开关。

## 卸载

```bash
./install.sh --uninstall
./install.sh --uninstall --purge
```

不加 `--purge` 时，设置留在 `~/.config/frame-audio-mixer`。`--purge` 还会删除已保存的 `frame-audio-mixer.*` 值。WirePlumber 脚本会从磁盘删除，但要到 WirePlumber 再次启动后才卸载出内存。请只在没有 VR 会话时这样做。

## 构建

在头显上，`rustc` 和 `cargo` 位于 `~/.cargo/bin`。这里用到的系统包是 cairo、freetype、Vulkan、PipeWire 和 libclang。

```bash
export PATH="$HOME/.cargo/bin:$PATH"
python3 scripts/make-icons.py
cargo build --release
./target/release/frame-audio-mixer --self-test
./target/release/frame-audio-mixer --language sc --dump-png /tmp/frame-audio-mixer.png
./scripts/package.sh
```

`scripts/package.sh` 会写出 `dist/frame-audio-mixer-<version>.tar.gz` 和 `dist/SHA256SUMS`。

2026-10-17 在这台头显上，`cargo build --release` 已完成，`--self-test` 打印了 `self-test ok`。`ja`、`en`、`sc`、`de`、`fr`、`es`、`it` 的 `--dump-png` 都是 1200×788 的图像。每张图页脚最右侧像素是 x=1152，没有一种语言的页脚超出 1200px 的面板。

## 麦克风、电平、应用

麦克风选择只作用于混音。应用不会调用 `wpctl set-default`。「系统默认」跟随系统音源。「Frame 内置麦克风」是 `alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source`，即 Frame Mic Tuner 之后 Valve 链路的末端。`frame-audio-mixer.mic-source` 要等头显空闲、WirePlumber 下次启动时才生效。

电平表只在面板可见、混音打开、并且语言菜单旁齿轮里的电平表为开时运行。每个采样是带 `node.virtual=true` 的 `pw-record`。声道是 passive，麦克风不是。可见时以 15 fps 重绘。

应用与更新显示本应用、作为相关应用的 Frame Mic Tuner，以及 sasaken 的 Frame 应用。安装会打开 Konsole。已经在运行的 Frame Mic Tuner 没有安装按钮。

更新器是 Rust（`ureq` 与 rustls、`sha2`、`flate2`、`tar`）。不再安装 `frame-update.sh`。检查会读取 GitHub 最新 release。安装会复制二进制并用 `systemd-run --user --unit=frame-audio-mixer-update` 启动。下载仅限 GitHub 的 HTTPS，最多 256 MB，SHA-256 必须一致，tar 不得包含绝对路径、`..`、链接或设备文件。`scripts/get.sh` 需要 `curl`、`sha256sum` 和 `tar`。在发布 release 之前，它会说明还没有 release。`scripts/package.sh` 还会写出内容相同的 `dist/frame-audio-mixer-aarch64.tar.gz`。

## 致谢

面板和 OpenVR overlay 移植自 sasaken1102r 的 Frame Mic Tuner（MIT）：https://github.com/sasaken1102r/frame-mic-tuner

应用列表是 frame-apps 0.3.1 的 Rust 移植，那些图标是 MIT。更新器按该项目的行为用 Rust 重写。见 `NOTICE`。

## 图标

`scripts/make-icons.py` 绘制仪表盘图标。录音、停止和播放是绘制的形状。其余字形是嵌入的路径：

| 图标 | 集合 | 作者 | 许可 |
| --- | --- | --- | --- |
| [fontisto:mic](https://icon-sets.iconify.design/fontisto/mic/) | fontisto | Kenan Gündoğan | MIT |
| [famicons:mic-off](https://icon-sets.iconify.design/famicons/mic-off/) | famicons | Family | MIT |
| [f7:speaker-2-fill](https://icon-sets.iconify.design/f7/speaker-2-fill/) | Framework7 | Vladimir Kharlampidi | MIT |
| [f7:speaker-slash-fill](https://icon-sets.iconify.design/f7/speaker-slash-fill/) | Framework7 | Vladimir Kharlampidi | MIT |
| [fluent:add-square-28-filled](https://icon-sets.iconify.design/fluent/add-square-28-filled/) | Fluent | Microsoft | MIT |
| [heroicons:language-16-solid](https://icon-sets.iconify.design/heroicons/language-16-solid/) | Heroicons | Refactoring UI Inc | MIT |
| [at-icons:headphones](https://icon-sets.iconify.design/at-icons/headphones/) | @icons | Valentin Fossati | MIT |
| [material-symbols:settings-rounded](https://icon-sets.iconify.design/material-symbols/settings-rounded/) | Material Symbols | Google | Apache-2.0 |

## 头显上的检查（2026-10-17）

测量时 `steamvr.service` 和 `gamescope-session.service` 都是 active。没有重启 WirePlumber。新脚本已经安装，尚未加载。

- `~/.local/bin/frame-audio-mixer` 已安装。`frame-audio-mixer.service` 为 enabled、inactive。`frame-audio-mixer-graph.service` 为 disabled、inactive。`frame-mic-tuner.service` 为 enabled、active。
- `frame-mic-tracker.lua` 和 `90-frame-mic.conf` 仍在，其中没有 `frame_amix`。`91-frame-audio-mixer.conf` 和 `frame-audio-mixer.lua` 已安装。没有 `frame-mic-mix.service` 单元。
- `frame-mic.echo-cancel` 为 false（保存值也是 false）。`frame-mic.noise-suppression` 为 true（保存值也是 true）。`frame-mic.mix` 仍在列表中，值为 false（保存值也是 false）：正在运行的 WirePlumber 保留了这个键。`frame-audio-mixer.enabled` 和 `frame-audio-mixer.vrlink-gain` 还不在正在运行的设置里。
- `pw-dump` 有 19 个节点，没有 `frame_amix_*`、`frame_mix_*` 或 `frame_micmix*`。采集流是 `eq_capture`、`echo_cancel_capture`、`ns_capture` 和 ALSA 麦克风 loopback。没有 VRLink 采集流，因此没有测量它的音量。默认源是 `alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source`。默认 sink 是 `alsa_loopback_device.stereo.alsa_output.platform-sound.HiFi__Speaker__sink`。
- 没有做听感检查（麦克风、回声叠成两遍、外接麦克风）。那需要有人在 PC 旁，而且这次 VR 会话中没有启动混音图。
