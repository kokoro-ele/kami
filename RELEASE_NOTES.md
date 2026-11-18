First release 🧾

Print from your Mac to cheap Bluetooth thermal printers — no phone app needed.

- Live captions on paper (on-device speech recognition)
- Wish machine / omikuji
- Any installed font, including Furifont (furigana via ligatures)
- Pipeable CLI: `kami print`, `kami text`, `kami tape`, `kami daemon`

### Install (macOS, Apple silicon)

```bash
curl -fsSL https://raw.githubusercontent.com/kokoro-ele/kami/main/install.sh | sh
```

Installs Kami into Applications and opens it — no security prompt. Prefix `sh` with `KAMI_CLI=1` to also get the CLI.

### Downloads
- `Kami_0.1.0_aarch64.dmg` — desktop app for Apple silicon Macs
- `kami-cli-aarch64-apple-darwin.tar.gz` — CLI

### "Apple could not verify Kami is free of malware"
Kami isn't signed with an Apple Developer ID yet. Drag Kami into Applications and open it once, click *Done*, then go to **System Settings → Privacy & Security**, scroll down and click **Open Anyway** next to Kami. Or run this once in Terminal:

```bash
xattr -cr /Applications/Kami.app
```

### Requirements
- Printing, wish machine, text, images: macOS 11+
- **Live captions: macOS 26 (Tahoe) or later on Apple silicon.** Not every Mac supports Apple's on-device speech recognition; Kami checks at launch and explains if captions can't be used. The first time you use a language, macOS downloads its speech model (needs internet). Grant *Screen & System Audio Recording* or *Microphone* when asked.

### Known issues
- Some printers feed a blank after every job, so line-by-line captions have gaps. Use "Every N lines", or try the experimental "Continuous paper" switch.
- Apple's recognizer is fast but less accurate for Chinese than Whisper-class models.
- Tested only on ORICO PPS1 (LuckJingle).

Thanks to minixprint, ljprint, fichero-printer, atctwo, TiMini-Print, Anthony Fu's Furifont, and Adobe Source Han.
