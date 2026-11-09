[English](technical.md) · [日本語](technical.ja.md) · **简体中文** · [Deutsch](technical.de.md) · [Français](technical.fr.md) · [Español](technical.es.md) · [Italiano](technical.it.md)

[← README](../README.sc.md)

# 技术细节

## 安装了什么

一行安装命令会运行 [`scripts/get.sh`](../scripts/get.sh)。它下载最新的发布包（`frame-audio-mixer-aarch64.tar.gz`），用 `SHA256SUMS` 校验，然后运行其中的 `install.sh`。还没有发布 release 时，它会改为下载 `main` 分支并构建，这需要头显上有 Rust（`cargo`）。它需要 `curl`、`sha256sum` 和 `tar`。`sh -s --` 之后的选项都会传给 `install.sh`。

`install.sh` 只写入主目录，不使用 `sudo`：

- 二进制文件 `~/.local/bin/frame-audio-mixer`
- 用户单元 `frame-audio-mixer.service` 和 `frame-audio-mixer-engine.service`
- WirePlumber 脚本 `~/.local/share/wireplumber/scripts/frame-audio-mixer.lua` 和 `~/.config/wireplumber/wireplumber.conf.d/91-frame-audio-mixer.conf`（组件 `custom.frame-audio-mixer`）
- `~/.local/share` 下的桌面项和图标
- `~/.config/frame-audio-mixer/` 中的设置，以及 `~/.config/pipewire/frame-audio-mixer/<id>.name` 中的通道名称

运行时，应用会添加名为 `frame_amix_*` 的 PipeWire 节点，WirePlumber 设置 `frame-audio-mixer.enabled`、`frame-audio-mixer.vrlink-gain` 和 `frame-audio-mixer.mic-source`，以及仪表盘 overlay `marcus-universe.frame-audio-mixer`。

`install.sh` 从不重启 PipeWire 或 WirePlumber。新的或改过的 WirePlumber 脚本会在 WirePlumber 下次启动时加载。请只在没有 VR 会话时重启头显：游玩时重启 PipeWire 或 WirePlumber 会让 SteamVR 和 Steam Link 失去声音。在那之前通道也能用，因为面板运行时也会固定它们的流。

如果以前的实验留下了 `frame-mic-mix` 单元，`install.sh` 会删掉它们。它唯一改动的 `frame-mic.*` 值是残留的 `frame-mic.mix`。它不会覆盖 Frame Mic Tuner 的 `frame-mic-tracker.lua` 或 `90-frame-mic.conf`。

在 git 检出中，`./install.sh` 会用 `cargo` 构建并安装。`./install.sh --autostart` 还会让面板随 SteamVR 启动。你用过的选项会保存在 `~/.config/frame-audio-mixer/install-args`，所以更新时会用同样的方式安装。

## 通道

- 最多 32 个通道，`ch1` 到 `ch32`。添加或删除只在 `frame-audio-mixer-engine.service` 里创建或销毁该通道的节点。
- 名称占一行，最多 32 个字符，忽略大小写后必须唯一。
- 用正在运行的应用创建的通道以该应用命名，会把该应用固定到这个通道，并显示应用 `.desktop` 项里的图标。没有图标时显示首字母。
- 这一行比面板宽时，用手柄摇杆滚动，或者拖动通道下方的滚动条。
- 每个通道条有一个推子，从无声到 +12 dB，在 0 dB 处会吸附。静音和独奏在它下面。路由按钮竖排在推子旁边：A1 到 A4（你听到的）、PC（PC 串流麦克风）、B1 到 B4（虚拟麦克风），然后是 FX1 和 FX2。有 6 或 8 个输出时，A 和 B 按钮分成两列。在列表视图里，它们排成一行，位于静音和独奏旁边。

落到未固定通道上的流会被移回：移到自己的通道，或移出混音器，在正常输出上播放。固定规则在已知时会比对应用的二进制文件——同名应用（Electron 应用都自称 “Chromium”）无法互相夺走通道。

## 项目

页眉的文件夹按钮保存路由设置。项目把通道、固定、路由、效果链和电平存为一个 JSON 文件，位于 `~/.config/frame-audio-mixer/projects/`。载入会替换当前设置；语言、主题、布局等显示偏好保持不变。

## 输出

PC 串流麦克风始终存在。应用设置可以再添加 2、4、6 或 8 个输出：2 是 A1 和 B1，8 是 A1 到 A4 和 B1 到 B4。数量调小时，已保存的路由会保留，但不会连接。

- PC 是 `frame_amix_source`，显示为 **Frame Mix**。VRLink 会录制它，所以 PC 看到的是 Steam Streaming Microphone。
- A1 是头显扬声器。A2 到 A4 可以是任何其他输出，例如蓝牙。
- B1 到 B4 是给头显上的应用用的虚拟麦克风（`frame_amix_b1` 到 `frame_amix_b4`）。每个都有名称。

输出抽屉（右侧栏上的扬声器图标）顶部是混音检查，下面每个输出一个容器：名称、名称下方的主推子，以及右侧的静音。PC 排在最前。

版本 2 的设置会在第一次加载时迁移：旧的 B1 变成 PC（连同它的推子和静音），旧的 B2 到 B4 变成 B1 到 B3。

## 信号流向

信号流向页面（页眉中「应用和更新」旁边）把整个混音画成分列的图：音源、混音器通道、输出，以及最终到达的设备。连线来自保存的路由，每条都会对照引擎的真实 PipeWire 链接检查 —— 链接丢失的路由显示为红色虚线。有声音通过的链接会带上电平颜色的光晕。拖动平移，摇杆滚动，选中节点可以看到背后的 PipeWire 节点。打开「其他应用」开关，还会显示所有不在任何通道上的应用，以及它们实际播放的设备。

## 麦克风

麦克风选择只作用于混音。本应用不会调用 `wpctl set-default`，所以 Frame Mic Tuner 使用的系统默认保持不变。

- **系统默认** 跟随 `default.audio.source`。
- **Frame 内置麦克风** 是 `alsa_loopback_device.alsa_input.platform-sound.HiFi__Mic__source`，即 Valve 链路在噪声抑制、回声消除和麦克风 EQ 之后的末端。
- 列表中的其他源按 `node.name` 保存。

选择保存为 `frame-audio-mixer.mic-source`（`""`、`builtin` 或节点名）。脚本会把 `builtin` 解析为上面的 loopback；如果那个节点不存在，就回退到系统默认；它从不使用 `frame_amix_source`。改过的值在 WirePlumber 下次启动后生效。

引擎用一个非虚拟的 loopback 采集这个源，所以 Mix 打开时 Valve 的噪声抑制保持开启。推子在 0 dB 时，麦克风条会加 12 dB 的增益修正。它的默认路由只有 PC。

## 电平表

电平表只在仪表盘可见、混音打开，并且应用设置（语言菜单旁边的齿轮）里打开了电平表时运行。只有屏幕上能看到的通道才会测量。每个电平表都是带 `node.virtual=true` 的 `pw-record` 采样，所以不会打开 Valve 的麦克风滤波器。通道的采样是被动的。麦克风的采样不是。电平表运行时，面板以大约 30 fps 重绘；仪表盘隐藏时不绘制。

## 效果

FX1 和 FX2 是共享发送，每个都带一条最多三个效果的链。均衡是其中之一：3、6、8 或 12 个点的参量均衡器，用它所在槽位上的编辑打开。外侧的点可以是钟形、搁架，或者高通（低端的点）或低通（高端的点）。发送不改变干声路径。每个返回都有自己的推子，以及 A、PC 和 B 路由。处理在引擎内部以 float32、48 kHz、480 帧进行。切换发送不会重启 PipeWire 或 WirePlumber。

## 应用和更新

**应用和更新** 显示本应用、作为相关应用的 Frame Mic Tuner，以及 sasaken 目录中的 Frame 应用。**安装** 会打开 Konsole 并运行 `curl -fsSL https://frame.sasaken1102s.net | sh`。已经在运行的 Frame Mic Tuner 没有安装按钮，所以这个页面不会替换它的分支版本。

更新器用 Rust 编写（`ureq` 配 rustls、`sha2`、`flate2` 和 `tar`）：

- 检查时读取 GitHub 上最新的 release，比较版本，并查找 `frame-audio-mixer-<version>.tar.gz` 和 `SHA256SUMS`。
- 安装时把正在运行的二进制文件复制到 `~/.cache/frame-audio-mixer/update/`，并用 `systemd-run --user --unit=frame-audio-mixer-update --collect` 启动它。
- 下载只用 HTTPS，只来自 `github.com`、`api.github.com`、`codeload.github.com` 和 `*.githubusercontent.com`，除非设置了 `FRAME_UPDATE_ALLOW_INSECURE=1`。重定向会被检查，下载到 256 MB 就会停止，SHA-256 必须一致。
- 如果压缩包里有绝对路径、`..`、链接或设备节点，就会被拒绝。
- 缓存文件在 `~/.cache/frame-audio-mixer/`（`update-check.json`、`update-state.json`、`update.log`、`update.lock/`、`update/`）。

## 卸载

使用一行命令：

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/i | sh -s -- --uninstall
```

在检出目录或发布包文件夹中：

```bash
./install.sh --uninstall
./install.sh --uninstall --purge
```

不加 `--purge` 时，设置留在 `~/.config/frame-audio-mixer`。`--purge` 还会删除这些设置和已保存的 `frame-audio-mixer.*` 值。WirePlumber 脚本会从磁盘删除，但在 WirePlumber 再次启动前仍保持加载。

## 构建

在头显上，`rustc` 和 `cargo` 位于 `~/.cargo/bin`。用到的系统库是 cairo、freetype、Vulkan、PipeWire 和 libclang。

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release
./target/release/frame-audio-mixer --self-test
./target/release/frame-audio-mixer --language en --dump-png /tmp/frame-audio-mixer.png
./scripts/package.sh
```

`--dump-png` 会在给定路径旁边写出主视图和其他视图（应用、添加、通道选择器、设置、语言菜单、录音时滚动过的一行）。

`scripts/package.sh` 会写出 `dist/frame-audio-mixer-<version>.tar.gz`（与 `dist/frame-audio-mixer-aarch64.tar.gz` 字节完全相同），以及列出这两个文件的 `dist/SHA256SUMS`。把这三个文件都上传到一个 GitHub release，一行安装和应用内更新器就会使用它。
