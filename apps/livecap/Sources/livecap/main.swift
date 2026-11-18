// livecap — 实时语音转写（macOS 26+ 原生 SpeechAnalyzer）
//
// 音源：系统声音（默认，ScreenCaptureKit）或麦克风（--mic）
//
// 输出：
//   默认      stdout 每句一行 "HH:mm:ss<TAB>文本"；stderr 滚动显示临时结果
//   --json    stdout 每行一个 JSON：
//               {"type":"final","time":"..","text":".."}
//               {"type":"volatile","text":".."}
//               {"type":"level","rms":0.031}        每 0.5 秒一次，用来确认真的收到声音
//               {"type":"error","message":".."}
//   --debug   stderr 额外打印音量和缓冲区信息
//
// 用法:
//   swift build -c release
//   .build/release/livecap                    # 系统声音，中文
//   .build/release/livecap --mic --locale ja-JP
//   .build/release/livecap | kami tape
//
// 权限：系统声音需要「屏幕与系统录音」，麦克风需要「麦克风」，都授权给启动它的 App（终端或 Kami）。

import AVFoundation
import CoreMedia
import Foundation
import ScreenCaptureKit
import Speech

setvbuf(stdout, nil, _IOLBF, 0)

// ------------------------------------------------------------ 参数
var localeID = "zh-CN"
var useMic = false
var json = false
var debug = false
var quiet = false
var argIter = CommandLine.arguments.dropFirst().makeIterator()
while let a = argIter.next() {
    switch a {
    case "--locale", "-l": if let v = argIter.next() { localeID = v }
    case "--mic", "-m": useMic = true
    case "--json": json = true
    case "--debug": debug = true
    case "--quiet", "-q": quiet = true
    case "--list-locales":
        for l in await SpeechTranscriber.supportedLocales.map(\.identifier).sorted() { print(l) }
        exit(0)
    case "--check":
        // Kami 启动时调用：这台 Mac 能不能用系统语音识别
        let locales = await SpeechTranscriber.supportedLocales.map(\.identifier).sorted()
        let result: [String: Any] = ["type": "check", "ok": !locales.isEmpty, "locales": locales]
        let d = try! JSONSerialization.data(withJSONObject: result)
        print(String(data: d, encoding: .utf8)!)
        exit(0)
    case "--help", "-h":
        FileHandle.standardError.write(
            "用法: livecap [--mic] [--locale zh-CN|ja-JP|en-US] [--json] [--debug] [--quiet] [--list-locales] [--check]\n"
                .data(using: .utf8)!)
        exit(0)
    default:
        FileHandle.standardError.write("未知参数: \(a)\n".data(using: .utf8)!)
        exit(2)
    }
}

// ------------------------------------------------------------ 输出
let outLock = NSLock()

func stderrLine(_ s: String) {
    FileHandle.standardError.write((s + "\n").data(using: .utf8)!)
}

func emitJSON(_ obj: [String: Any]) {
    guard let d = try? JSONSerialization.data(withJSONObject: obj),
          let s = String(data: d, encoding: .utf8) else { return }
    outLock.lock(); print(s); outLock.unlock()
}

/// code 给 Kami 用来显示本地化的原因和修复办法
func fail(_ code: String, _ msg: String) -> Never {
    if json { emitJSON(["type": "error", "code": code, "message": msg]) } else { stderrLine("❌ " + msg) }
    exit(1)
}

func info(_ msg: String) {
    if !quiet { stderrLine(msg) }
}

func volatileStatus(_ s: String) {
    guard !quiet, !json else { return }
    FileHandle.standardError.write(("\r\u{1B}[2K" + s).data(using: .utf8)!)
}

// ------------------------------------------------------------ 音频 → AnalyzerInput
final class AudioFeeder: NSObject, SCStreamOutput, SCStreamDelegate, @unchecked Sendable {
    let builder: AsyncStream<AnalyzerInput>.Continuation
    let targetFormat: AVAudioFormat
    private var converter: AVAudioConverter?
    private var sourceFormat: AVAudioFormat?
    private var sumSquares: Double = 0
    private var samples: Int = 0
    private var lastReport = Date()
    private(set) var buffersSeen = 0

    init(builder: AsyncStream<AnalyzerInput>.Continuation, targetFormat: AVAudioFormat) {
        self.builder = builder
        self.targetFormat = targetFormat
    }

    // ScreenCaptureKit 回调：画面帧直接丢掉，只要声音
    func stream(_ stream: SCStream, didOutputSampleBuffer sb: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .audio, sb.isValid, let pcm = Self.copyPCM(sb) else { return }
        feed(pcm)
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        fail("capture_stopped", "系统声音捕获停止: \(error.localizedDescription)")
    }

    func feed(_ pcm: AVAudioPCMBuffer) {
        buffersSeen += 1
        meter(pcm)

        if converter == nil || sourceFormat != pcm.format {
            converter = AVAudioConverter(from: pcm.format, to: targetFormat)
            sourceFormat = pcm.format
            if debug { stderrLine("[debug] 输入格式 \(pcm.format) → \(targetFormat)") }
        }
        guard let converter else { return }

        let ratio = targetFormat.sampleRate / pcm.format.sampleRate
        let capacity = AVAudioFrameCount(Double(pcm.frameLength) * ratio) + 1024
        guard let out = AVAudioPCMBuffer(pcmFormat: targetFormat, frameCapacity: capacity) else { return }

        var fed = false
        var err: NSError?
        let status = converter.convert(to: out, error: &err) { _, outStatus in
            if fed {
                outStatus.pointee = .noDataNow
                return nil
            }
            fed = true
            outStatus.pointee = .haveData
            return pcm
        }
        if status == .error, debug { stderrLine("[debug] 转换失败: \(err?.localizedDescription ?? "?")") }
        if status != .error, out.frameLength > 0 {
            builder.yield(AnalyzerInput(buffer: out))
        }
    }

    /// 计算音量，每 0.5 秒报告一次（确认确实收到了声音）
    private func meter(_ pcm: AVAudioPCMBuffer) {
        guard let ch = pcm.floatChannelData else { return }
        let n = Int(pcm.frameLength)
        for i in 0..<n {
            let v = Double(ch[0][i])
            sumSquares += v * v
        }
        samples += n
        if Date().timeIntervalSince(lastReport) >= 0.5, samples > 0 {
            let rms = (sumSquares / Double(samples)).squareRoot()
            if json { emitJSON(["type": "level", "rms": (rms * 10_000).rounded() / 10_000]) }
            if debug { stderrLine(String(format: "[debug] 音量 rms=%.4f  已收到 %d 个缓冲", rms, buffersSeen)) }
            sumSquares = 0
            samples = 0
            lastReport = Date()
        }
    }

    static func copyPCM(_ sb: CMSampleBuffer) -> AVAudioPCMBuffer? {
        guard let fd = CMSampleBufferGetFormatDescription(sb),
              let asbdPtr = CMAudioFormatDescriptionGetStreamBasicDescription(fd)
        else { return nil }
        var asbd = asbdPtr.pointee
        guard let fmt = AVAudioFormat(streamDescription: &asbd) else { return nil }
        let frames = AVAudioFrameCount(CMSampleBufferGetNumSamples(sb))
        guard frames > 0, let pcm = AVAudioPCMBuffer(pcmFormat: fmt, frameCapacity: frames) else { return nil }
        pcm.frameLength = frames
        let st = CMSampleBufferCopyPCMDataIntoAudioBufferList(
            sb, at: 0, frameCount: Int32(frames), into: pcm.mutableAudioBufferList)
        return st == noErr ? pcm : nil
    }
}

// ------------------------------------------------------------ 识别器
guard let locale = await SpeechTranscriber.supportedLocale(equivalentTo: Locale(identifier: localeID)) else {
    fail("unsupported_locale", "不支持的语言 \(localeID)，用 --list-locales 查看可用语言")
}

let transcriber = SpeechTranscriber(
    locale: locale,
    transcriptionOptions: [],
    reportingOptions: [.volatileResults, .fastResults],
    attributeOptions: []
)

do {
    if let req = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
        info("⬇️  正在下载 \(locale.identifier) 语音模型（只需一次）…")
        try await req.downloadAndInstall()
    }
} catch {
    fail("model_download", "语音模型下载失败: \(error.localizedDescription)")
}

let analyzer = SpeechAnalyzer(modules: [transcriber])
guard let analyzerFormat = await SpeechAnalyzer.bestAvailableAudioFormat(compatibleWith: [transcriber]) else {
    fail("recognizer", "拿不到识别器需要的音频格式")
}
if debug { stderrLine("[debug] 识别器格式 \(analyzerFormat)") }

let (inputSequence, inputBuilder) = AsyncStream<AnalyzerInput>.makeStream()
let feeder = AudioFeeder(builder: inputBuilder, targetFormat: analyzerFormat)

do {
    try await analyzer.start(inputSequence: inputSequence)
} catch {
    fail("recognizer", "识别器启动失败: \(error.localizedDescription)")
}

// ------------------------------------------------------------ 音源
var scStream: SCStream?
let audioEngine = AVAudioEngine()

if useMic {
    switch AVCaptureDevice.authorizationStatus(for: .audio) {
    case .notDetermined:
        if !(await AVCaptureDevice.requestAccess(for: .audio)) {
            fail("mic_permission", "没有麦克风权限：系统设置 → 隐私与安全性 → 麦克风")
        }
    case .denied, .restricted:
        fail("mic_permission", "没有麦克风权限：系统设置 → 隐私与安全性 → 麦克风，打开启动 livecap 的 App")
    default: break
    }
    let input = audioEngine.inputNode
    let fmt = input.outputFormat(forBus: 0)
    if fmt.sampleRate == 0 { fail("no_microphone", "找不到可用的麦克风") }
    input.installTap(onBus: 0, bufferSize: 4096, format: fmt) { buf, _ in feeder.feed(buf) }
    do {
        try audioEngine.start()
    } catch {
        fail("mic_failed", "麦克风启动失败: \(error.localizedDescription)")
    }
    info("🎙  正在收听麦克风（\(locale.identifier)），Ctrl+C 结束")
} else {
    do {
        let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
        guard let display = content.displays.first else { fail("capture_failed", "找不到显示器") }
        let filter = SCContentFilter(display: display, excludingWindows: [])
        let cfg = SCStreamConfiguration()
        cfg.capturesAudio = true
        cfg.excludesCurrentProcessAudio = true
        cfg.sampleRate = 48_000
        cfg.channelCount = 1
        cfg.width = 2
        cfg.height = 2
        cfg.minimumFrameInterval = CMTime(value: 1, timescale: 2)
        cfg.queueDepth = 3
        let s = SCStream(filter: filter, configuration: cfg, delegate: feeder)
        let q = DispatchQueue(label: "livecap.capture")
        // 同时挂上画面输出：只挂音频时，部分系统版本不会开始投递声音
        try s.addStreamOutput(feeder, type: .screen, sampleHandlerQueue: q)
        try s.addStreamOutput(feeder, type: .audio, sampleHandlerQueue: q)
        try await s.startCapture()
        scStream = s
    } catch {
        fail("screen_permission", "系统声音捕获启动失败: \(error.localizedDescription)。"
            + "请到 系统设置 → 隐私与安全性 → 屏幕与系统录音 打开启动 livecap 的 App（终端或 Kami），然后完全退出该 App 再重开")
    }
    info("🎧 正在收听系统声音（\(locale.identifier)），Ctrl+C 结束")

    // 5 秒内一个缓冲都没收到：多半是权限没生效
    Task {
        try? await Task.sleep(for: .seconds(5))
        if feeder.buffersSeen == 0 {
            let msg = "5 秒内没有收到任何系统声音：确认正在播放声音，并检查「屏幕与系统录音」权限；也可以先用 --mic 试试麦克风"
            if json { emitJSON(["type": "error", "code": "no_audio", "message": msg]) } else { stderrLine("⚠️  " + msg) }
        }
    }
}

// ------------------------------------------------------------ Ctrl+C / SIGTERM：把剩余结果定稿再退出
func shutdown() {
    Task {
        if let s = scStream { try? await s.stopCapture() }
        audioEngine.stop()
        inputBuilder.finish()
        try? await analyzer.finalizeAndFinishThroughEndOfInput()
        try? await Task.sleep(for: .milliseconds(300))
        info("\n👋 已停止")
        exit(0)
    }
}

var signalSources: [DispatchSourceSignal] = []
for sig in [SIGINT, SIGTERM] {
    signal(sig, SIG_IGN)
    let src = DispatchSource.makeSignalSource(signal: sig, queue: .global())
    src.setEventHandler { shutdown() }
    src.resume()
    signalSources.append(src)
}

// ------------------------------------------------------------ 结果
let timeFmt = DateFormatter()
timeFmt.dateFormat = "HH:mm:ss"

do {
    for try await result in transcriber.results {
        let text = String(result.text.characters).trimmingCharacters(in: .whitespacesAndNewlines)
        if result.isFinal {
            volatileStatus("")
            guard !text.isEmpty else { continue }
            let t = timeFmt.string(from: Date())
            if json {
                emitJSON(["type": "final", "time": t, "text": text])
            } else {
                outLock.lock(); print("\(t)\t\(text)"); outLock.unlock()
            }
        } else if !text.isEmpty {
            if json { emitJSON(["type": "volatile", "text": text]) }
            volatileStatus("… " + String(text.suffix(60)))
        }
    }
} catch {
    fail("recognizer", "识别出错: \(error.localizedDescription)")
}
