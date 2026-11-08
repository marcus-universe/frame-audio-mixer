<p align="center">
  <img src="assets/logo/logo_banner_blue.svg" alt="Frame Audio Mixer" width="640">
</p>
<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/github/license/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="License"></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/version-2.0.0-33C4FF?style=flat-square" alt="Version 2.0.0"></a>
  <a href="https://frame-audio-mixer.marcus-universe.com/"><img src="https://img.shields.io/badge/Website-33C4FF?style=flat-square" alt="Website"></a>
  <a href="https://github.com/marcus-universe/frame-audio-mixer/stargazers"><img src="https://img.shields.io/github/stars/marcus-universe/frame-audio-mixer?style=flat-square&color=33C4FF" alt="GitHub stars"></a>
</p>

[English](README.md) · [日本語](README.ja.md) · **简体中文** · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Italiano](README.it.md)

# 🎚️ Frame Audio Mixer

Frame Audio Mixer 把应用的声音和头显麦克风混在一起，再通过无线串流把这份混音送给 PC，送进 SteamVR 已经提供的 **Steam Streaming Microphone**。

它接在 [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner) 之后。回声消除和噪声抑制仍由 Frame Mic Tuner 负责。

## 📥 安装

在 Frame 上打开 Konsole（底部栏上的 **+**，然后是程序列表，然后是 **Konsole**），运行：

```sh
curl -fsSL https://frame-audio-mixer.marcus-universe.com/i | sh
```

- 🛠️ 先打开开发者模式：Steam 设置，然后是系统，然后是 **启用开发者模式**。打开之前，列表里没有 Konsole。
- 💻 从 PC 安装：[通过 SSH 安装](docs/install-ssh.sc.md)。

## ✨ 功能

- 🔄 **方向**：输入和输出各自在直立推子和横排之间切换。
- 🎛️ **效果总线**：FX1 和 FX2，内有混响、压缩、均衡、变调、镶边和相位。每条通道都可以发送到任意一条总线。
- 🎤 **虚拟麦克风**：B1 到 B4，可以命名，加在 PC 串流麦克风之上。
- 🎧 **扬声器、蓝牙和 USB**：连接后可以在 Frame 扬声器、蓝牙耳机或 USB 音频上听混音。
- 🧩 **软件通道**：最多 32 个命名通道。每个都是任意应用都能选择的输出。
- 🖥️ **宽屏**：21:9 布局。可以拖动两栏之间的分界。每一侧最少占窗口的五分之一。
- 📊 **电平、静音和吸附**：每个推子用同一刻度，从 +6 到 −48。每个推子下有静音。输出推子会吸到刻度上。
- ⏺️ **混音检查**：在输出区顶部。选择一个输出，录制最多 10 秒，然后播放或暂停。
- 🌐 七种语言，以及蓝色或粉色主题。

<table>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-1.jpg" alt="宽屏，蓝色主题" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-2.jpg" alt="输入横排，粉色主题" height="320"></td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-3.jpg" alt="效果总线，粉色主题" height="320"></td>
    <td width="50%"><img src="docs/assets/Preview-Frame-Audio-Mixer-4.jpg" alt="仪表板，蓝色主题" height="320"></td>
  </tr>
</table>

## ▶️ 使用

1. 打开 SteamVR 仪表盘，选择 **Frame Audio Mixer**。
2. 打开混音。PC 上会显示为 **Steam Streaming Microphone**。
3. 按 **+** 添加通道。给它起名，或选择一个正在运行的应用。
4. 在那个应用里，选择和通道同名的输出。
5. 左边的**输入**是进入混音的声音。右边的**输出**是它去的地方：PC、Frame 扬声器、蓝牙、USB，或一个虚拟麦克风。

🎧 如果用 Frame 的扬声器听，请在 Frame Mic Tuner 里打开回声消除。否则 PC 可能听到两遍。

## 📚 文档

- 📖 [工作方式](docs/how-it-works.sc.md)：音频图，以及 VRLink 和应用如何路由。
- 🔧 [技术细节](docs/technical.sc.md)：安装了什么、麦克风和电平表的细节、应用和更新、卸载、构建。

## 🙏 致谢

Frame Audio Mixer 的灵感来自 [sasaken1102r](https://github.com/sasaken1102r) 的 [Frame Mic Tuner](https://github.com/sasaken1102r/frame-mic-tuner)。更多ささけん＠的 Steam Frame 应用见 [frame.sasaken1102s.net](https://frame.sasaken1102s.net/)。

面板、OpenVR overlay 和应用列表改编自该项目（MIT）。见 [`NOTICE`](NOTICE)。

- 🎨 [图标及其许可](docs/icons.sc.md)

## 📄 许可

[MIT](LICENSE)
