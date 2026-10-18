<p align="center">
  <img src="assets/logo/logo_banner_blue.svg" alt="Frame Audio Mixer" width="640">
</p>
<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/github/license/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="License"></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/version-1.1.0-33C4FF?style=flat-square" alt="Version 1.1.0"></a>
  <a href="https://frame-audio-mixer.marcus-universe.com/"><img src="https://img.shields.io/badge/Website-33C4FF?style=flat-square" alt="Website"></a>
  <a href="https://github.com/marcus-universe/frame-audio-mixer/stargazers"><img src="https://img.shields.io/github/stars/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="GitHub stars"></a>
</p>

[English](README.md) · [日本語](README.ja.md) · **简体中文** · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Italiano](README.it.md)

# Frame Audio Mixer

这是 Steam Frame 上的一个 SteamVR 仪表盘面板。它把应用的声音和头显麦克风混在一起，再通过无线串流把这份混音送给 PC。PC 上显示为 **Steam Streaming Microphone**。

它接在 [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner) 之后。回声消除和噪声抑制仍由 Frame Mic Tuner 负责。

## 安装

在 Frame 上打开 Konsole（底部栏上的 **+**，然后是程序列表，然后是 **Konsole**），运行：

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/frame-audio-mixer-install | sh
```

- 先打开开发者模式：Steam 设置，然后是系统，然后是 **启用开发者模式**。打开之前，列表里没有 Konsole。
- 如果还想让面板随 SteamVR 启动，把命令结尾改成 `| sh -s -- --autostart`。
- 更新时，再运行一次同一条命令。卸载时，把结尾改成 `| sh -s -- --uninstall`。
- 它不使用 `sudo`，只写入你的主目录。它从不重启 PipeWire 或 WirePlumber，所以正在进行的 VR 会话不会失去声音。
- 从 PC 安装：[通过 SSH 安装](docs/install-ssh.sc.md)。

## 功能

- **虚拟音频设备**：最多 32 个命名通道。每个通道都是一个输出（`Frame Mix: <name>`），任何应用都可以选择它。
- **应用通道**：用正在运行的应用创建通道。名称旁边会显示它的图标。
- **PC 串流麦克风混音**：PC 听到的是你的麦克风加上各个通道。
- **每个通道两个推子**：一个用于头显，一个用于 PC，各自有静音。推子会吸附到 20、50、80 和 100 %。
- **麦克风**：选择用于混音的麦克风。系统默认麦克风不会改变。
- 每个推子上都有 **电平表**。可以关闭。
- **混音检查**：录制 PC 听到的声音，最多 10 秒，然后播放。
- 七种语言，以及蓝色或粉色的配色主题。

<table>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-1.png" alt="分栏视图，蓝色主题" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-2.png" alt="列表视图，蓝色主题" height="320"></td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-3.png" alt="分栏视图，粉色主题" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-4.png" alt="列表视图，粉色主题" height="320"></td>
  </tr>
</table>

## 使用

1. 打开 SteamVR 仪表盘，选择 **Frame Audio Mixer**。
2. 打开 **混音**。
3. 按 **+** 添加通道：可以选建议、自己起的名称，或者正在运行的应用。
4. 在那个应用里选择输出 `Frame Mix: <name>`。
5. 扬声器推子决定这个通道在头显里有多响。麦克风推子决定 PC 能听到它多少。

如果使用 Frame 的扬声器，请在 Frame Mic Tuner 里打开回声消除。否则 PC 可能听到两遍应用的声音。

## 文档

- [工作方式](docs/how-it-works.sc.md)：音频图，以及 VRLink 和应用如何路由。
- [技术细节](docs/technical.sc.md)：安装了什么、麦克风和电平表的细节、应用和更新、卸载、构建。

## 致谢

Frame Audio Mixer 的灵感来自 [sasaken1102r](https://github.com/sasaken1102r) 的 [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner)。更多ささけん＠的 Steam Frame 应用见 [frame.sasaken1102s.net](https://frame.sasaken1102s.net/)。

面板、OpenVR overlay 和应用列表改编自该项目（MIT）。见 [`NOTICE`](NOTICE)。

- [图标及其许可](docs/icons.sc.md)

## 许可

[MIT](LICENSE)
