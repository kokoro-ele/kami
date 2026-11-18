# 紙 kami

**スマホアプリなしで、Mac から激安 Bluetooth サーマルプリンターに印刷。**
「Luck Jingle」などのアプリ専用だったポケットプリンター向けの CLI とデスクトップアプリです。

[English](README.md) · [中文](README.zh-CN.md)

- **字幕を紙に**：Mac で流れている音声（ポッドキャスト、動画、会議）をオンデバイスで文字起こしして、1 行ずつ印刷
- **おみくじ機**：願いごとを書くと、おみくじが出てきます
- **インストール済みのフォントならなんでも**：ふりがな付きの [Furifont](https://furifont.antfu.me/) も使えます
- **パイプでつなげる CLI**：`echo hello | kami text -`、`livecap | kami tape`、`kami daemon` で HTTP 印刷も
- **軽量**：CLI は約 5 MB の単一バイナリ、アプリは Tauri 製

## 対応プリンター

| ドライバー | Bluetooth 名 | メモ |
|---|---|---|
| LuckJingle（Lujiang / LuckPrinter SDK） | `PPS*` | 384 ドット（58 mm 紙）、BLE サービス `FF00`。ORICO PPS1 で動作確認 |

「Luck Jingle」アプリで使う他の機種も `--prefix <名前>` で動くかもしれません。新しい機種は `crates/kami-core/src/protocol.rs` の `Driver` を実装するだけです。PR 歓迎、`kami scan` の出力を添えてください。

## インストール

```bash
cargo install --git https://github.com/kokoro-ele/kami kami-cli
```

**デスクトップ版（macOS・Apple シリコン）**：[Releases](https://github.com/kokoro-ele/kami/releases) から `.dmg` をダウンロードし、Kami を「アプリケーション」にドラッグします。
まだ Apple の Developer ID で署名していないため、初回は **「"Kami" は壊れているため開けません」** と表示されます。ファイルは壊れていません。ブラウザでダウンロードした未署名アプリを macOS がブロックしているだけです。ターミナルで一度だけ次を実行してから開き直してください：

```bash
xattr -cr /Applications/Kami.app
```

### 動作環境

| 機能 | 必要なもの |
|---|---|
| 印刷・おみくじ・テキスト・画像 | macOS 11 以降（Windows / Linux 版は未検証） |
| リアルタイム字幕 | **macOS 26（Tahoe）以降、Apple シリコン**。Apple のオンデバイス音声認識を使うため、すべての Mac で使えるわけではありません。Kami は起動時に確認し、使えない場合は理由を表示します。言語を初めて使うときは音声モデルのダウンロードが必要です（ネット接続が必要）。システム音声には「画面とシステムオーディオ録音」、マイクには「マイク」の許可が必要です。 |

## 既知の問題

- 印刷のたびに少し紙送りする機種では、「1 行ずつ」モードで行間にすき間ができます。「行をまとめて」モードか、試験的な「連続紙モード」をお試しください。
- Apple のオンデバイス認識は速いですが、中国語の精度は Whisper 系に及びません。
- 対応機種はまだ 1 系統のみです。

## 謝辞

これらのプリンターを解析して公開してくださった方々に感謝します：[minixprint](https://github.com/SwapnanilDhol/minixprint)、[ljprint](https://github.com/MrSlopster/ljprint)、[fichero-printer](https://github.com/0xMH/fichero-printer)、[atctwo さんの L13 解析記事](https://atctwo.net/posts/2024/07/16/thermal-printer.html)、[TiMini-Print](https://github.com/Dejniel/TiMini-Print)。

[Anthony Fu](https://github.com/antfu) さんの [Furifont](https://furifont.antfu.me/) に感謝します。合字でふりがなを付けるこのフォントのおかげで、日本語の字幕が紙の上でも読めるようになりました。ベースとなった Adobe の源ノ角ゴシック / 源ノ明朝（SIL Open Font License）にも感謝します。

Tauri、btleplug、fontdb、ttf-parser、fontdue、image-rs を使っています。

kami はプリンターメーカーとは無関係で、ご自身のデバイスにのみ接続します。MIT © Zetian Yu
