# 紙 kami

**不用手机 App，直接从 Mac 打印到几十块钱的蓝牙热敏小打印机。**
给那些原本只能配合「Luck Jingle」之类手机 App 使用的口袋打印机，做了一个命令行工具和一个桌面 App。

[English](README.md) · [日本語](README.ja.md)

- **字幕上纸**：Mac 正在播放的声音（播客、视频、会议）在本机实时转成文字，一行一行打印出来
- **许愿机**：写下一个愿望，打出一张签（日文版是おみくじ）
- **任意本机字体**：包括 [Furifont](https://furifont.antfu.me/) 这种自带注音的字体
- **命令行可接管道**：`echo hello | kami text -`、`livecap | kami tape`，或者 `kami daemon` 常驻后用 HTTP 打印
- **体积小**：命令行是一个约 5 MB 的单文件，桌面版用 Tauri 构建

## 支持的打印机

| 驱动 | 蓝牙名 | 说明 |
|---|---|---|
| LuckJingle（Lujiang / LuckPrinter SDK） | `PPS*` | 384 点（58mm 纸），BLE 服务 `FF00`，在 ORICO PPS1 上测试 |

其他用「Luck Jingle」App 的机型可以用 `--prefix <名字>` 试试。想支持新品牌，在 `crates/kami-core/src/protocol.rs` 里实现 `Driver` 接口即可，欢迎 PR，请附上 `kami scan` 的输出。

## 安装

```bash
cargo install --git https://github.com/kokoro-ele/kami kami-cli
```

**桌面版（macOS，Apple 芯片）**：从 [Releases](https://github.com/kokoro-ele/kami/releases) 下载 `.dmg`，把 Kami 拖进「应用程序」。
Kami 还没有用 Apple 开发者证书签名，所以第一次打开时 macOS 会提示 **「Kami 已损坏，无法打开」**。文件并没有坏，是 macOS 拦截了从浏览器下载的未签名 App。在终端执行一次下面的命令，再打开就行：

```bash
xattr -cr /Applications/Kami.app
```

### 系统要求

| 功能 | 要求 |
|---|---|
| 打印、许愿机、文字、图片 | macOS 11 及以上（Windows / Linux 版尚未测试） |
| 实时字幕 | **macOS 26（Tahoe）及以上，Apple 芯片**。用的是苹果的端侧语音识别，并非所有 Mac 都支持，Kami 启动时会检测，不能用时会直接说明原因。第一次使用某种语言时，系统会下载对应的识别模型，需要联网。收听系统声音需要「屏幕与系统录音」权限，用麦克风需要「麦克风」权限。 |

## 已知问题

- 部分打印机每打完一段会自动走一小段纸，所以「逐行」打印字幕时行与行之间会有空隙。可以改用「攒行」，或者试试实验性的「连续纸模式」。
- Apple 端侧识别速度快，但中文准确率不如 Whisper 一类的模型。
- 目前只支持一个系列的打印机。

## 致谢

感谢逆向这些打印机并公开分享成果的人：[minixprint](https://github.com/SwapnanilDhol/minixprint)、[ljprint](https://github.com/MrSlopster/ljprint)、[fichero-printer](https://github.com/0xMH/fichero-printer)、[atctwo 的 L13 逆向记录](https://atctwo.net/posts/2024/07/16/thermal-printer.html)、[TiMini-Print](https://github.com/Dejniel/TiMini-Print)。

感谢 [Anthony Fu](https://github.com/antfu) 的 [Furifont](https://furifont.antfu.me/)：靠合字给汉字自动加注音，让日文字幕在纸上也能读懂。也感谢 Furifont 所基于的 Adobe 思源黑体 / 思源宋体（SIL Open Font License）。

使用了 Tauri、btleplug、fontdb、ttf-parser、fontdue、image-rs。

kami 与任何打印机厂商都没有关联，只连接你自己的设备。MIT © Zetian Yu
