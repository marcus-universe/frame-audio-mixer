[English](install-pc.md) · [日本語](install-pc.ja.md) · **简体中文** · [Deutsch](install-pc.de.md) · [Français](install-pc.fr.md) · [Español](install-pc.es.md) · [Italiano](install-pc.it.md)

[← README](../README.sc.md)

# 安装 PC 音频设备

**Frame Audio Link** 是装在串流用 PC 上的 SteamVR 插件。它为 PC 添加八个音频设备：

| PC 上的名称 | 用途 |
|---|---|
| **Steam Streaming Mic B1** … **B4** | 麦克风，传来头显上混音器的 B1–B4。 |
| **Steam Streaming Out PC1** … **PC4** | 输出。在这里播放的声音可以在混音器中作为通道 PC1–PC4 添加。 |

Steam 自带的 **Steam Streaming Microphone** 和 **Steam Streaming Speakers** 保持不变。

## 开始之前

- 头显上装有 Frame Audio Mixer 2.2 或更新版本（见[安装](../README.sc.md#-安装)）。
- PC 上装有 SteamVR，并且到 Steam Frame 的无线串流可以正常使用。

## Windows

1. 从[最新版本](https://github.com/marcus-universe/frame-audio-mixer/releases/latest)下载 `frame-audio-link-windows-x86_64.zip` 并解压。
2. 双击 **install.cmd**。
3. Windows 会请求一次管理员权限。这一步会创建八个设备。

或者在 PowerShell 中：

```powershell
irm https://frame-audio-mixer.marcus-universe.com/get-windows | iex
```

程序安装在 `%LOCALAPPDATA%\Programs\FrameAudioLink`。

- 这些设备是 Steam 已安装的两个音频驱动的额外实例，不会安装新的驱动。
- Steam 会在第一次使用 Remote Play 或无线 VR 串流时安装它的驱动。如果安装程序提示缺少驱动，请先串流一次，再重新运行。
- 如果 Windows 把某个新设备设为默认，安装程序会恢复你原来的默认设备。

即使关闭 SteamVR，这些设备也会保留。

## Linux

支持 Ubuntu 22.10 或更新版本、Fedora 和 Arch。声音服务器必须是 PipeWire。用下面的命令检查：

```sh
pactl info | grep "Server Name"
```

应显示 `PulseAudio (on PipeWire …)`。Ubuntu 22.04 仍使用 PulseAudio，不受支持。在 Arch 上，如果缺少 `pipewire-pulse` 和 `wireplumber`，请先安装。

以普通用户安装，不需要 `sudo`：

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/get-linux | sh
```

程序安装在 `~/.local/bin/frame-audio-link`。在 Linux 上，这八个设备是 PipeWire 节点：只在插件运行时存在，也就是 SteamVR 运行期间。不会在声音系统中安装任何东西。

## 在 SteamVR 中启用

安装程序会把插件登记为启动应用。请确认它已启用：**SteamVR → 设置 → 启动/关闭 → 选择启动时的叠加应用 → Frame Audio Link**。

之后仪表板上会有 **Audio Link** 标签页。它显示是否找到了头显，并为八路串流各提供一个开关。关闭的串流不占用带宽。

## 使用

- **从头显到 PC**：在混音器中把麦克风或某个通道路由到 B1–B4。在 PC 上，任何应用（Discord、OBS、游戏）都可以选择 **Steam Streaming Mic B1** … **B4** 作为麦克风。
- **从 PC 到头显**：在 PC 上让应用输出到 **Steam Streaming Out PC1** … **PC4**。在混音器中按 **+**，在“正在运行”下选择 **PC1** … **PC4**。

## 检查

- **Windows**：八个设备会出现在声音控制面板（`mmsys.cpl`）中。在终端运行 `frame-audio-link devices status` 也会列出它们。
- **Linux**：插件运行期间，`pactl list short sources` 和 `pactl list short sinks` 会显示 `frame_link_b1` … 和 `frame_link_pc1` …。
- **叠加层一直显示“正在搜索”**：在 Steam Frame 上开始无线串流，并在那里打开 Frame Audio Mixer。头显只响应它正在串流的那台 PC。
- **叠加层提示缺少设备**：在 Windows 上重新运行 `install.cmd`。

日志在 Windows 上位于 `%APPDATA%\frame-audio-link\frame-audio-link.log`，在 Linux 上位于 `~/.config/frame-audio-link/frame-audio-link.log`。

## 卸载

- **Windows**：双击压缩包中的 **uninstall.cmd**。它会删除八个设备、SteamVR 登记和程序本身。
- **Linux**：运行压缩包中的 `./install-linux.sh --uninstall`。

链接的工作原理见 [PC audio link](pc-link.md)（英文）。
