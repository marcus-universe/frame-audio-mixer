[English](install-ssh.md) · [日本語](install-ssh.ja.md) · **简体中文** · [Deutsch](install-ssh.de.md) · [Français](install-ssh.fr.md) · [Español](install-ssh.es.md) · [Italiano](install-ssh.it.md)

[← README](../README.sc.md)

# 通过 SSH 安装

从 PC 安装 Frame Audio Mixer。你登录到 Steam Frame，然后运行和 Konsole 里相同的命令。PC 和头显需要在同一个网络里。

## 打开 SSH

1. 在 Frame 上打开 Steam 设置，然后是系统，然后是 **启用开发者模式**。
2. 在开发者项里设置密码。这个密码就是 SSH 登录密码。请用不容易猜到的密码：打开 SSH 之后，同一网络里知道密码的人都能登录头显。
3. 记下头显的 IP 地址。它显示在开发者设置里，或者显示在路由器上。

用户名是 `steamos`。

## 连接

Windows 10 和 11、macOS 和 Linux 都带有 OpenSSH 客户端。在 PC 的终端里运行：

```sh
ssh steamos@<headset-ip>
```

把 `<headset-ip>` 换成上面记下的地址。第一次会要求确认主机密钥，然后输入开发者密码。

## 安装

在这个会话里运行：

```sh
curl -fsSL https://raw.githubusercontent.com/marcus-universe/frame-audio-mixer/main/scripts/get.sh | sh
```

- 如果还想让面板随 SteamVR 启动，把命令结尾改成 `| sh -s -- --autostart`。
- 更新时，再运行一次同一条命令。卸载时，把结尾改成 `| sh -s -- --uninstall`。
- 它不使用 `sudo`，只写入头显上的主目录。它从不重启 PipeWire 或 WirePlumber，所以正在进行的 VR 会话不会失去声音。

也可以不留在会话里，用一行命令发送。在 macOS、Linux 或 Windows Terminal 里：

```sh
ssh steamos@<headset-ip> "curl -fsSL https://raw.githubusercontent.com/marcus-universe/frame-audio-mixer/main/scripts/get.sh | sh"
```

安装程序只在 Steam Frame 上运行。SSH 会话已经在头显上，所以这项检查会通过。

然后打开 SteamVR 仪表盘，选择 **Frame Audio Mixer**。
