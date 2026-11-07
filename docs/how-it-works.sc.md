[English](how-it-works.md) · [日本語](how-it-works.ja.md) · **简体中文** · [Deutsch](how-it-works.de.md) · [Français](how-it-works.fr.md) · [Español](how-it-works.es.md) · [Italiano](how-it-works.it.md)

[← README](../README.sc.md)

# 工作方式

```mermaid
flowchart LR
  appA[应用] --> chSink["frame_amix_chN"]
  micSrc["Valve 滤波器之后的麦克风"] --> micStrip["麦克风条"]
  chSink --> chStrip["通道条"]
  chStrip --> busA["A1 到 A4"]
  chStrip --> busB["B1 到 B4"]
  micStrip --> busB
  busA --> speakers["扬声器或其他输出"]
  busB --> frameMix["frame_amix_source"]
  frameMix --> vrlink["VRLink"]
```

## 麦克风

混音从面板里选择的麦克风开始。**系统默认**跟随默认源。**Frame 麦克风**是经过 Valve 滤波器之后的内置麦克风。这些滤波器由 Frame Mic Tuner 控制。本应用不改动它们，也不改变系统默认。

## 引擎

`frame-audio-mixer-engine.service` 持有整张图。面板关闭后混音继续。

- 每个通道是一个名为 `frame_amix_chN` 的 sink。应用看到的是通道名（`DJ`）。
- 通道条应用插入效果、推子、静音、独奏和电平，然后送到打开的 A 和 B 总线。
- A1 是头显扬声器。A2 到 A4 是你选择的输出。
- B1 是 `frame_amix_source`（**Frame Mix**）。B2 到 B4 是更多虚拟麦克风。
- 麦克风条默认只送到 B1。采集端不是虚拟节点，所以 Mix 打开时 Valve 的噪声抑制保持开启。

添加或删除通道只创建或销毁该通道的节点。PipeWire 和 WirePlumber 不会重启。

内部节点名同时包含 `virtual` 和 `sink`，所以空间音频不会把混音再送到扬声器。

## 路由到 PC

混音打开时（`frame-audio-mixer.enabled` 为 true），WirePlumber 组件 `custom.frame-audio-mixer` 会把 VRLink 的采集流指向 `frame_amix_source`。它把这个源的 `object.serial` 写入默认元数据，因为在那里写节点名不会移动这条流。如果源不存在，VRLink 继续使用原来的默认麦克风。其他录制麦克风的应用不会被移动。

`frame-audio-mixer.vrlink-gain` 默认是 `1.0`，这样 VRLink 的音量保持不变。其他值在混音打开时生效，关闭混音后会恢复之前的音量。

## 把应用路由到通道

选择了 `Frame Mix: <name>` 的应用仍会经过空间音频，并且把节点名设为目标会被忽略。所以 Frame Audio Mixer 用输出的 `object.serial` 把流固定到通道。WirePlumber 脚本和面板都会这样做。通道重启后 serial 会变，输出重新出现时会再写一次固定。删除输出时会清除固定，应用就回到普通输出。

用正在运行的应用创建的通道还会记住那个应用。混音打开时，即使那个应用没有输出设置，它的流也会被固定到这个通道。

本应用不会调用 `wpctl set-default`。

## 回声消除

使用 Frame 的扬声器时，麦克风会听到扬声器。扬声器已经在播放各个通道，所以没有回声消除时，PC 会听到两遍应用的声音：一遍来自混音，一遍经空气传到麦克风。使用扬声器时，请在 Frame Mic Tuner 里打开回声消除。耳机没有这个问题。
