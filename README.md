# 紙 kami

**Print from your Mac to a cheap Bluetooth thermal printer — no phone app needed.**
A tiny CLI and a desktop app for the ~$20 pocket printers that normally only work with apps like *Luck Jingle*.

[中文](README.zh-CN.md) · [日本語](README.ja.md)

- **Live captions on paper** — whatever your Mac is playing (a podcast, a video, a meeting) is transcribed on-device and printed line by line.
- **Wish machine** — write a wish, get a fortune slip (omikuji in Japanese), printed.
- **Any installed font**, including fonts with built-in furigana like [Furifont](https://furifont.antfu.me/).
- **Pipeable CLI** — `echo hello | kami text -`, `livecap | kami tape`, or run `kami daemon` and print over HTTP.
- Small: the CLI is a single ~5 MB binary; the app is built with Tauri.

## Supported printers

| Driver | Bluetooth name | Notes |
|---|---|---|
| LuckJingle (Lujiang / LuckPrinter SDK) | `PPS*` | 384 dots (58 mm paper), BLE service `FF00`. Tested on ORICO PPS1. |

Other printers that use the *Luck Jingle* app may work with `--prefix <name>`.
Adding a brand means implementing one `Driver` trait in [`crates/kami-core/src/protocol.rs`](crates/kami-core/src/protocol.rs) — PRs welcome. Please include the output of `kami scan`.

## Install

```bash
# CLI
cargo install --git https://github.com/kokoro-ele/kami kami-cli

```

**Desktop app (macOS, Apple silicon):** download the `.dmg` from [Releases](https://github.com/kokoro-ele/kami/releases) and drag Kami into Applications.
Kami isn't signed with an Apple Developer ID yet, so macOS says **"Kami is damaged and can't be opened"**. The file isn't damaged — macOS blocks unsigned apps downloaded from a browser. Run this once, then open Kami again:

```bash
xattr -cr /Applications/Kami.app
```

### Requirements

| Feature | Needs |
|---|---|
| Printing, wish machine, text, images | macOS 11+ (Windows / Linux builds are untested) |
| Live captions | **macOS 26 (Tahoe) or later on Apple silicon**. Uses Apple's on-device speech recognition, which isn't available on every Mac — Kami checks at launch and tells you if it can't be used. The first time you use a language, macOS downloads its speech model (needs internet). Grant *Screen & System Audio Recording* (system audio) or *Microphone* when asked. |

## CLI

```bash
kami scan                                  # find printers nearby
kami test                                  # print a test page
kami print photo.jpg --dither atkinson     # print an image
kami text "hello from the terminal" --center
echo "hello" | kami text -
kami text "preview only" --preview out.png # save a PNG instead of printing
kami daemon                                # auto-reconnect + queue folder + local HTTP
curl --data "hi" http://127.0.0.1:7878/text
```

Options: `--density 0|1|2` · `--feed <dots>` · `--dither fs|atkinson|threshold` · `--font <file>` · `--prefix <name>`

## Live captions (macOS 26+)

`apps/livecap` uses Apple's on-device **SpeechAnalyzer** to transcribe system audio or the microphone. It ships inside the desktop app; build it yourself only for CLI use.

```bash
cd apps/livecap && swift build -c release
.build/release/livecap --locale ja-JP | kami tape
```

Grant *Screen & System Audio Recording* to your terminal (or Kami) the first time.

## Desktop app

```bash
cd apps/desktop && npm install && npm run tauri dev
```

## Known issues

- Some printers feed a short blank after every print job, so **line-by-line captions leave gaps**. Use *Every N lines* mode, or try the experimental *Continuous paper* switch.
- Apple's on-device recognizer is fast but less accurate for Chinese than Whisper-class models.
- Only one printer family is supported so far.

## Thanks

kami stands on the work of people who reverse-engineered these printers and shared it openly:

- [minixprint](https://github.com/SwapnanilDhol/minixprint) by Swapnanil Dhol — the LuckJingle / Lujiang print sequence and BLE characteristics
- [ljprint](https://github.com/MrSlopster/ljprint) — LuckJingle BLE CLI and protocol notes
- [fichero-printer](https://github.com/0xMH/fichero-printer) — LuckPrinter SDK protocol reference
- [atctwo's L13 write-up](https://atctwo.net/posts/2024/07/16/thermal-printer.html) — the original `10 FF` command notes
- [TiMini-Print](https://github.com/Dejniel/TiMini-Print) — showing these printers deserve good desktop software

Fonts:

- [Furifont](https://furifont.antfu.me/) by [Anthony Fu](https://github.com/antfu) — furigana through ligatures; it's what made Japanese captions readable on paper
- Source Han Sans / Source Han Serif by Adobe (SIL Open Font License), which Furifont is built on

Built with [Tauri](https://tauri.app), [btleplug](https://github.com/deviceplug/btleplug), [fontdb](https://github.com/RazrFalcon/fontdb), [ttf-parser](https://github.com/RazrFalcon/ttf-parser), [fontdue](https://github.com/mooman219/fontdue) and [image-rs](https://github.com/image-rs/image).

kami is not affiliated with any printer maker. It only talks to devices you own.

## License

MIT © Zetian Yu
