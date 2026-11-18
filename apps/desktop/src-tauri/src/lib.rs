//! Kami 桌面版后端：把 kami-core 暴露成 Tauri 命令。
//!
//! 所有命令出错时返回 JSON 字符串 `{"code": "...", "detail": "..."}`，
//! 前端按 code 显示本地化的原因和修复办法，detail 是原始信息（给"详细信息"用）。

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use kami_core::ble::{self, ConnectOptions, Printer};
use kami_core::image_io;
use kami_core::layout::{self, TapeEntry};
use kami_core::protocol::JobOptions;
use kami_core::raster::{self, Bitmap, Dither, Gray};
use kami_core::text::TextRenderer;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};
use tokio::sync::Mutex;

const WIDTH: usize = 384;
const TEXT_DITHER: Dither = Dither::Threshold(150);

#[derive(Default)]
struct AppState {
    printer: Mutex<Option<Printer>>,
    fonts: StdMutex<Option<Vec<FontInfo>>>,
    /// 连续打印（字幕逐行）的任务是否已经开始
    stream_open: std::sync::atomic::AtomicBool,
    /// 每次打印开始时告诉打印机的纸张类型；255 = 不设置
    paper_mode: std::sync::atomic::AtomicU8,
    captions: StdMutex<Option<Child>>,
    /// 每次开始收听 +1；事件都带上它，前端据此忽略旧进程发来的事件
    cap_session: std::sync::atomic::AtomicU64,
    font: StdMutex<Option<Arc<TextRenderer>>>,
}

type CmdResult<T> = Result<T, String>;

/// 结构化错误
fn fail(code: &str, detail: impl std::fmt::Display) -> String {
    serde_json::json!({ "code": code, "detail": detail.to_string() }).to_string()
}

/// 把蓝牙/打印相关的底层错误归类
fn printer_err(e: impl std::fmt::Display) -> String {
    let s = e.to_string();
    let code = if s.contains("没找到打印机") {
        "printer_not_found"
    } else if s.contains("蓝牙适配器") || s.to_lowercase().contains("bluetooth") {
        "bluetooth_unavailable"
    } else if s.contains("断开") || s.to_lowercase().contains("not connected") {
        "printer_disconnected"
    } else {
        "print_failed"
    };
    fail(code, s)
}

#[derive(Serialize)]
struct Status {
    connected: bool,
    name: Option<String>,
}

#[derive(Serialize)]
struct Device {
    name: String,
    id: String,
    rssi: Option<i16>,
    supported: bool,
}

#[derive(Serialize, Deserialize, Clone)]
struct Entry {
    time: String,
    text: String,
}

// ------------------------------------------------------------------ 连接
/// 没连上就连；返回 true 表示这次是新连接（之前的连续打印任务已经失效）
async fn ensure(slot: &mut Option<Printer>, prefix: Option<String>) -> CmdResult<bool> {
    let alive = match slot.as_ref() {
        Some(p) => p.is_connected().await,
        None => false,
    };
    if !alive {
        let opts = ConnectOptions {
            name_prefix: prefix,
            timeout: Duration::from_secs(10),
        };
        *slot = Some(Printer::connect(&opts).await.map_err(printer_err)?);
    }
    Ok(!alive)
}

#[tauri::command]
async fn scan() -> CmdResult<Vec<Device>> {
    let list = ble::scan(Duration::from_secs(5), &[])
        .await
        .map_err(printer_err)?;
    Ok(list
        .into_iter()
        .map(|d| Device {
            name: d.name,
            id: d.id,
            rssi: d.rssi,
            supported: d.supported,
        })
        .collect())
}

#[tauri::command]
async fn connect(state: State<'_, AppState>, prefix: Option<String>) -> CmdResult<Status> {
    let mut slot = state.printer.lock().await;
    ensure(&mut slot, prefix).await?;
    Ok(Status {
        connected: true,
        name: slot.as_ref().map(|p| p.name().to_string()),
    })
}

#[tauri::command]
async fn status(state: State<'_, AppState>) -> CmdResult<Status> {
    let slot = state.printer.lock().await;
    let (connected, name) = match slot.as_ref() {
        Some(p) => (p.is_connected().await, Some(p.name().to_string())),
        None => (false, None),
    };
    Ok(Status {
        connected,
        name: if connected { name } else { None },
    })
}

#[tauri::command]
async fn disconnect(state: State<'_, AppState>) -> CmdResult<()> {
    let p = state.printer.lock().await.take();
    if let Some(p) = p {
        p.disconnect().await.map_err(printer_err)?;
    }
    Ok(())
}

fn paper_mode(state: &AppState) -> Option<u8> {
    match state.paper_mode.load(Ordering::SeqCst) {
        255 => None,
        v => Some(v),
    }
}

/// 设置纸张类型（null = 不设置）。连续卷纸 = 2
#[tauri::command]
fn set_paper_mode(state: State<'_, AppState>, mode: Option<u8>) {
    state
        .paper_mode
        .store(mode.unwrap_or(255), Ordering::SeqCst);
}

/// 开发版：把每次真正发给打印机的点阵图存下来，方便和屏幕预览对比
#[cfg(debug_assertions)]
fn debug_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../.debug");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[cfg(debug_assertions)]
fn dump_print(bmp: &Bitmap) {
    let dir = debug_dir();
    let _ = std::fs::create_dir_all(&dir);
    let stamp = chrono::Local::now().format("%H%M%S%3f");
    let _ = image_io::save_bitmap_png(bmp, dir.join(format!("print-{stamp}.png")));
    let _ = image_io::save_bitmap_png(bmp, dir.join("last-print.png"));
}

async fn send(state: &AppState, bmp: &Bitmap, density: u8, feed: u8) -> CmdResult<()> {
    #[cfg(debug_assertions)]
    dump_print(bmp);
    let mut slot = state.printer.lock().await;
    if ensure(&mut slot, None).await? {
        state.stream_open.store(false, Ordering::SeqCst);
    }
    let p = slot
        .as_mut()
        .ok_or_else(|| fail("printer_disconnected", "no printer"))?;
    // 正在连续打印的话，先把那次任务收尾
    if state.stream_open.swap(false, Ordering::SeqCst) {
        p.stream_end(&JobOptions {
            density,
            feed: 0,
            paper: paper_mode(state),
        })
        .await
        .map_err(printer_err)?;
    }
    p.print(
        bmp,
        &JobOptions {
            density,
            feed,
            paper: paper_mode(state),
        },
    )
    .await
    .map_err(printer_err)
}

/// 连续打印一段（字幕逐行用）：第一次调用时开始任务，之后只发图像数据，行与行之间不走纸
#[tauri::command]
async fn print_line(
    state: State<'_, AppState>,
    png: String,
    density: u8,
    dither: String,
) -> CmdResult<()> {
    let bmp = render(&png, &dither)?;
    #[cfg(debug_assertions)]
    dump_print(&bmp);
    let mut slot = state.printer.lock().await;
    if ensure(&mut slot, None).await? {
        state.stream_open.store(false, Ordering::SeqCst);
    }
    let p = slot
        .as_mut()
        .ok_or_else(|| fail("printer_disconnected", "no printer"))?;
    let opts = JobOptions {
        density,
        feed: 0,
        paper: paper_mode(&state),
    };
    if !state.stream_open.load(Ordering::SeqCst) {
        p.stream_begin(&opts).await.map_err(printer_err)?;
        state.stream_open.store(true, Ordering::SeqCst);
    }
    if let Err(e) = p.stream_raster(&bmp).await {
        state.stream_open.store(false, Ordering::SeqCst);
        return Err(printer_err(e));
    }
    Ok(())
}

/// 结束连续打印：走纸并停止
#[tauri::command]
async fn end_stream(state: State<'_, AppState>, density: u8, feed: u8) -> CmdResult<()> {
    if !state.stream_open.swap(false, Ordering::SeqCst) {
        return Ok(());
    }
    let mut slot = state.printer.lock().await;
    if let Some(p) = slot.as_mut() {
        p.stream_end(&JobOptions {
            density,
            feed,
            paper: paper_mode(&state),
        })
        .await
        .map_err(printer_err)?;
    }
    Ok(())
}

// ------------------------------------------------------------------ 打印
fn decode_data_url(s: &str) -> CmdResult<Gray> {
    let b64 = s.split_once(',').map(|(_, d)| d).unwrap_or(s);
    let bytes = B64.decode(b64.trim()).map_err(|e| fail("bad_image", e))?;
    image_io::decode_gray(&bytes).map_err(|e| fail("bad_image", e))
}

fn to_data_url(g: &Gray) -> CmdResult<String> {
    let png = image_io::encode_png(g).map_err(|e| fail("bad_image", e))?;
    Ok(format!("data:image/png;base64,{}", B64.encode(png)))
}

/// "threshold:190"、"fs"…，结尾加 "+bold" 表示抖动后再把细线加粗
fn parse_dither(s: &str) -> CmdResult<(Dither, bool)> {
    let (mode, bold) = match s.strip_suffix("+bold") {
        Some(m) => (m, true),
        None => (s, false),
    };
    let d = mode.parse().map_err(|e: String| fail("bad_option", e))?;
    Ok((d, bold))
}

fn render(png: &str, dither: &str) -> CmdResult<Bitmap> {
    let (mode, bold) = parse_dither(dither)?;
    let bmp = raster::prepare(&decode_data_url(png)?, WIDTH, mode);
    Ok(if bold { raster::thicken(&bmp) } else { bmp })
}

/// 前端传来的图（data URL）→ 抖动后的预览（data URL）
#[tauri::command]
fn preview_png(png: String, dither: String) -> CmdResult<String> {
    let bmp = render(&png, &dither)?;
    #[cfg(debug_assertions)]
    let _ = image_io::save_bitmap_png(&bmp, debug_dir().join("last-preview.png"));
    to_data_url(&bmp.to_gray())
}

/// feed：打完走纸多少点。字幕逐行连续打印时传 0，行与行之间不留空白
#[tauri::command]
async fn print_png(
    state: State<'_, AppState>,
    png: String,
    density: u8,
    dither: String,
    feed: Option<u8>,
) -> CmdResult<()> {
    #[cfg(debug_assertions)]
    if let Ok(src) = decode_data_url(&png) {
        // 前端画出来、还没黑白化的原图
        let _ = image_io::save_png(&src, debug_dir().join("last-print-source.png"));
    }
    let bmp = render(&png, &dither)?;
    send(&state, &bmp, density, feed.unwrap_or(80)).await
}

fn font(state: &AppState) -> CmdResult<Arc<TextRenderer>> {
    let mut f = state.font.lock().unwrap();
    if f.is_none() {
        *f = Some(Arc::new(
            TextRenderer::load(None).map_err(|e| fail("font_missing", e))?,
        ));
    }
    Ok(f.as_ref().unwrap().clone())
}

/// 用系统字体排版的纸带（命令行同款）。桌面版默认在前端 canvas 排版，这个留作备用。
#[tauri::command]
async fn print_tape(
    state: State<'_, AppState>,
    title: Option<String>,
    entries: Vec<Entry>,
    density: u8,
) -> CmdResult<()> {
    let r = font(&state)?;
    let es: Vec<TapeEntry> = entries
        .into_iter()
        .map(|x| TapeEntry {
            time: x.time,
            text: x.text,
        })
        .collect();
    let img = layout::tape(&r, title.as_deref(), &es, WIDTH);
    let bmp = raster::dither(&img, TEXT_DITHER);
    send(&state, &bmp, density, 80).await
}

// ------------------------------------------------------------------ 本机字体
#[derive(Serialize, Clone)]
struct FontInfo {
    family: String,
    /// 另一种语言下的名字（比如「冬青黑体简体中文」之于 Hiragino Sans GB）
    alias: Option<String>,
    /// 是否包含中日文字形
    cjk: bool,
}

fn scan_fonts() -> Vec<FontInfo> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let mut map: std::collections::BTreeMap<String, FontInfo> = Default::default();
    for face in db.faces() {
        let Some((family, _)) = face.families.first() else {
            continue;
        };
        if family.starts_with('.') || family.is_empty() {
            continue; // 系统隐藏字体
        }
        let entry = map.entry(family.clone()).or_insert_with(|| FontInfo {
            family: family.clone(),
            alias: face
                .families
                .iter()
                .map(|(n, _)| n)
                .find(|n| *n != family)
                .cloned(),
            cjk: false,
        });
        if !entry.cjk {
            entry.cjk = db
                .with_face_data(face.id, |data, index| {
                    ttf_parser::Face::parse(data, index)
                        .map(|f| f.glyph_index('紙').is_some() || f.glyph_index('纸').is_some())
                        .unwrap_or(false)
                })
                .unwrap_or(false);
        }
    }
    map.into_values().collect()
}

/// 读出某个字体的文件内容，交给前端用 FontFace 加载。
/// WebKit 的 canvas 有时用不了用户自己装的字体（会悄悄退回系统字体），自己加载就一定能用。
#[tauri::command]
async fn font_file(family: String) -> CmdResult<tauri::ipc::Response> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        // 同一家族里优先挑常规体
        let face = db
            .faces()
            .filter(|f| f.families.iter().any(|(n, _)| *n == family))
            .min_by_key(|f| {
                (
                    (f.weight.0 as i32 - 400).abs(),
                    f.style != fontdb::Style::Normal,
                )
            })
            .ok_or_else(|| fail("font_missing", &family))?;
        let path = match &face.source {
            fontdb::Source::File(p) => p.clone(),
            fontdb::Source::SharedFile(p, _) => p.clone(),
            _ => return Err(fail("font_collection", &family)),
        };
        let is_collection = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("ttc") || e.eq_ignore_ascii_case("otc"))
            .unwrap_or(false);
        if is_collection || face.index != 0 {
            // 字体集合没法直接交给 FontFace；这类多是系统字体，canvas 本来就能用
            return Err(fail("font_collection", path.display()));
        }
        let bytes = std::fs::read(&path).map_err(|e| fail("font_missing", e))?;
        Ok(tauri::ipc::Response::new(bytes))
    })
    .await
    .map_err(|e| fail("unknown", e))?
}

/// 列出本机已安装的字体（第一次调用会扫描一遍，之后用缓存）
#[tauri::command]
async fn list_fonts(state: State<'_, AppState>) -> CmdResult<Vec<FontInfo>> {
    if let Some(cached) = state.fonts.lock().unwrap().clone() {
        return Ok(cached);
    }
    let list = tauri::async_runtime::spawn_blocking(scan_fonts)
        .await
        .map_err(|e| fail("unknown", e))?;
    *state.fonts.lock().unwrap() = Some(list.clone());
    Ok(list)
}

// ------------------------------------------------------------------ AI（Claude / OpenAI 兼容）
#[derive(Deserialize)]
struct AiRequest {
    /// "anthropic" | "openai"
    provider: String,
    base_url: String,
    model: String,
    api_key: String,
    prompt: String,
}

/// 在后端发请求：不受 WebView 跨域限制，Ollama / DeepSeek / 自建网关都能用
#[tauri::command]
async fn ai_complete(req: AiRequest) -> CmdResult<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(45))
        .build()
        .map_err(|e| fail("ai_failed", e))?;
    let base = req.base_url.trim_end_matches('/');

    let resp = if req.provider == "anthropic" {
        let base = if base.is_empty() {
            "https://api.anthropic.com"
        } else {
            base
        };
        client
            .post(format!("{base}/v1/messages"))
            .header("x-api-key", &req.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&serde_json::json!({
                "model": req.model,
                "max_tokens": 800,
                "messages": [{ "role": "user", "content": req.prompt }],
            }))
            .send()
            .await
    } else {
        let base = if base.is_empty() {
            "https://api.openai.com/v1"
        } else {
            base
        };
        let mut r = client
            .post(format!("{base}/chat/completions"))
            .json(&serde_json::json!({
                "model": req.model,
                "messages": [{ "role": "user", "content": req.prompt }],
            }));
        if !req.api_key.is_empty() {
            r = r.bearer_auth(&req.api_key);
        }
        r.send().await
    }
    .map_err(|e| fail("ai_unreachable", e))?;

    let status = resp.status();
    let body: serde_json::Value = resp.json().await.map_err(|e| fail("ai_failed", e))?;
    if !status.is_success() {
        let msg = body["error"]["message"].as_str().unwrap_or("").to_string();
        let code = if status.as_u16() == 401 || status.as_u16() == 403 {
            "ai_auth"
        } else {
            "ai_failed"
        };
        return Err(fail(code, format!("HTTP {status} {msg}")));
    }
    let text = if req.provider == "anthropic" {
        body["content"][0]["text"].as_str()
    } else {
        body["choices"][0]["message"]["content"].as_str()
    };
    text.map(str::to_string)
        .ok_or_else(|| fail("ai_failed", "empty response"))
}

// ------------------------------------------------------------------ 打开系统设置（权限问题的修复入口）
#[tauri::command]
fn open_system_settings(pane: String) -> CmdResult<()> {
    #[cfg(target_os = "macos")]
    {
        let anchor = match pane.as_str() {
            "screen" => "Privacy_ScreenCapture",
            "mic" => "Privacy_Microphone",
            "bluetooth" => "Privacy_Bluetooth",
            _ => "",
        };
        let url = format!("x-apple.systempreferences:com.apple.preference.security?{anchor}");
        Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| fail("unknown", e))?;
    }
    #[cfg(not(target_os = "macos"))]
    let _ = pane;
    Ok(())
}

// ------------------------------------------------------------------ 实时字幕（macOS 26+ livecap）
fn find_livecap() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("KAMI_LIVECAP") {
        return Some(PathBuf::from(p));
    }
    let mut c: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            c.push(dir.join("livecap")); // 打包进 App 的 sidecar
        }
    }
    // 仓库里编译出来的版本优先于 PATH，避免用到旧原型
    c.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../livecap/.build/release/livecap"));
    if let Some(home) = std::env::var_os("HOME") {
        c.push(
            PathBuf::from(home)
                .join("Projects/thermal-printer/kami/apps/livecap/.build/release/livecap"),
        );
    }
    if let Some(path) = std::env::var_os("PATH") {
        c.extend(std::env::split_paths(&path).map(|d| d.join("livecap")));
    }
    c.into_iter().find(|p| p.is_file())
}

#[tauri::command]
fn start_captions(
    app: AppHandle,
    state: State<'_, AppState>,
    locale: String,
    mic: bool,
    fast: Option<bool>,
) -> CmdResult<u64> {
    let mut slot = state.captions.lock().unwrap();
    if let Some(mut old) = slot.take() {
        let _ = old.kill();
    }
    let bin = find_livecap().ok_or_else(|| fail("livecap_missing", "livecap not found"))?;
    let session = state
        .cap_session
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        + 1;
    let mut args = vec![
        "--json".to_string(),
        "--quiet".into(),
        "--locale".into(),
        locale,
    ];
    if mic {
        args.push("--mic".into());
    }
    if fast.unwrap_or(false) {
        args.push("--fast".into()); // 更快出字，准确率会下降
    }
    let mut child = Command::new(&bin)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| fail("captions_failed", e))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| fail("captions_failed", "no stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| fail("captions_failed", "no stderr"))?;

    // 所有事件都带 session，前端只处理当前这次收听的事件
    // stderr：旧版 livecap 或崩溃时的原始输出
    let app_err = app.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }
            let code = if line.contains("--json") || line.contains("未知参数") {
                "livecap_outdated"
            } else {
                "captions_failed"
            };
            let _ = app_err.emit(
                "captions-error",
                serde_json::json!({ "session": session, "code": code, "detail": line }),
            );
        }
    });

    // stdout：JSON 行 —— final / volatile / level / error
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            let text = v["text"].as_str().unwrap_or_default().to_string();
            match v["type"].as_str() {
                Some("final") => {
                    let time = v["time"]
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| chrono::Local::now().format("%H:%M:%S").to_string());
                    let _ = app.emit(
                        "caption",
                        serde_json::json!({ "session": session, "time": time, "text": text }),
                    );
                }
                Some("volatile") => {
                    let _ = app.emit(
                        "caption-volatile",
                        serde_json::json!({ "session": session, "text": text }),
                    );
                }
                Some("level") => {
                    let _ = app.emit(
                        "caption-level",
                        serde_json::json!({ "session": session, "rms": v["rms"].as_f64().unwrap_or(0.0) }),
                    );
                }
                Some("error") => {
                    let _ = app.emit(
                        "captions-error",
                        serde_json::json!({
                            "session": session,
                            "code": v["code"].as_str().unwrap_or("captions_failed"),
                            "detail": v["message"].as_str().unwrap_or_default(),
                        }),
                    );
                }
                _ => {}
            }
        }
        let _ = app.emit("captions-ended", serde_json::json!({ "session": session }));
    });
    *slot = Some(child);
    Ok(session)
}

/// 界面刷新后用来找回状态：如果还在收听，返回当前的 session
#[tauri::command]
fn captions_running(state: State<'_, AppState>) -> Option<u64> {
    let mut slot = state.captions.lock().unwrap();
    match slot.as_mut().map(|c| c.try_wait()) {
        Some(Ok(None)) => Some(state.cap_session.load(std::sync::atomic::Ordering::SeqCst)),
        _ => None,
    }
}

fn kill_captions(state: &AppState) {
    if let Some(mut c) = state.captions.lock().unwrap().take() {
        let _ = c.kill();
    }
}

#[tauri::command]
fn stop_captions(state: State<'_, AppState>) -> CmdResult<()> {
    kill_captions(&state);
    Ok(())
}

/// 字幕能不能用："ok" | "platform"（不是 macOS）| "os"（低于 macOS 26）
/// | "missing"（找不到 livecap）| "unsupported"（这台 Mac 没有可用的语音识别）
#[tauri::command]
async fn captions_available() -> String {
    tauri::async_runtime::spawn_blocking(check_captions)
        .await
        .unwrap_or_else(|_| "missing".into())
}

#[cfg(not(target_os = "macos"))]
fn check_captions() -> String {
    "platform".into()
}

#[cfg(target_os = "macos")]
fn check_captions() -> String {
    // livecap 按 macOS 26 编译，旧系统上连启动都会失败，所以先看系统版本
    let major = Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|v| v.trim().split('.').next()?.parse::<u32>().ok())
        .unwrap_or(0);
    if major < 26 {
        return "os".into();
    }
    let Some(bin) = find_livecap() else {
        return "missing".into();
    };
    let Ok(mut child) = Command::new(&bin)
        .arg("--check")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return "missing".into();
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(50))
            }
            // 超时：不确定就当可用，真出错时 start_captions 会给出具体原因
            _ => {
                let _ = child.kill();
                return "ok".into();
            }
        }
    }
    let mut out = String::new();
    if let Some(mut so) = child.stdout.take() {
        use std::io::Read;
        let _ = so.read_to_string(&mut out);
    }
    match serde_json::from_str::<serde_json::Value>(out.trim()) {
        Ok(v) if v["ok"] == false => "unsupported".into(),
        Ok(_) => "ok".into(),
        // 旧版 livecap 不认识 --check：当作可用
        Err(_) => "ok".into(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            paper_mode: std::sync::atomic::AtomicU8::new(255),
            ..Default::default()
        })
        .invoke_handler(tauri::generate_handler![
            scan,
            connect,
            status,
            disconnect,
            preview_png,
            print_png,
            print_tape,
            print_line,
            end_stream,
            set_paper_mode,
            font_file,
            ai_complete,
            list_fonts,
            open_system_settings,
            start_captions,
            stop_captions,
            captions_running,
            captions_available
        ])
        .build(tauri::generate_context!())
        .expect("Kami failed to start")
        .run(|app, event| {
            // 退出时结束字幕进程，免得它在后台继续占用麦克风/录屏
            if let RunEvent::Exit = event {
                kill_captions(&app.state::<AppState>());
            }
        });
}
