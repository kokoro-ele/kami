//! kami daemon：自动连接/断线重连 + 队列文件夹 + 本地 HTTP 接口
//!
//!   队列：把 png/jpg 丢进 queue/ → 打印 → 移到 queue/done/
//!   HTTP（只监听 127.0.0.1）：
//!     curl --data-binary @a.png http://127.0.0.1:7878/print
//!     curl --data "今天也要加油"    http://127.0.0.1:7878/text
//!     curl http://127.0.0.1:7878/status

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kami_core::layout::{self, Align};
use kami_core::raster::{Dither, Gray};
use kami_core::{image_io, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::{Global, Session, TEXT_DITHER};

enum Job {
    Image(Gray, Dither),
    Text(String),
}

type Status = Arc<Mutex<String>>;

fn set(status: &Status, s: impl Into<String>) {
    *status.lock().unwrap() = s.into();
}

pub async fn run(g: Global, queue: Option<PathBuf>, port: Option<u16>) -> Result<()> {
    let (tx, mut rx) = mpsc::channel::<Job>(64);
    let status: Status = Arc::new(Mutex::new("starting".into()));

    let queue = queue.unwrap_or_else(|| PathBuf::from("queue"));
    std::fs::create_dir_all(queue.join("done"))?;
    eprintln!("📥 队列文件夹：{}", queue.display());
    tokio::spawn(watch_queue(queue, tx.clone(), g.dither));

    if let Some(port) = port {
        let listener = TcpListener::bind(("127.0.0.1", port)).await?;
        eprintln!(
            "🌐 HTTP：http://127.0.0.1:{port}  （POST /print 图片，POST /text 文字，GET /status）"
        );
        tokio::spawn(serve(listener, tx.clone(), status.clone(), g.dither));
    }
    drop(tx);

    let mut s = Session::new(g);
    let mut tick = tokio::time::interval(Duration::from_secs(5));
    let mut was_connected = false;

    loop {
        tokio::select! {
            job = rx.recv() => {
                let Some(job) = job else { break };
                let res = match job {
                    Job::Image(img, mode) => s.emit(&img, mode).await,
                    Job::Text(t) => {
                        let w = s.width();
                        match s.font() {
                            Ok(f) => {
                                let img = layout::text_block(f, t.trim_end(), 28.0, Align::Left, w);
                                s.emit(&img, TEXT_DITHER).await
                            }
                            Err(e) => Err(e),
                        }
                    }
                };
                match res {
                    Ok(()) => eprintln!("   ✓ 已打印"),
                    Err(e) => eprintln!("   ✗ 打印失败：{e}"),
                }
            }
            _ = tick.tick() => {
                // 打印机没连上就悄悄重试 —— 开机即自动连上
                match s.printer().await {
                    Ok(p) => {
                        if !was_connected {
                            set(&status, format!("connected {}", p.name()));
                        }
                        was_connected = true;
                    }
                    Err(e) => {
                        if was_connected {
                            eprintln!("⚠️  打印机断开，等待重新连接…");
                        }
                        was_connected = false;
                        set(&status, format!("waiting: {e}"));
                    }
                }
            }
        }
    }
    s.close().await;
    Ok(())
}

fn is_image(p: &Path) -> bool {
    matches!(
        p.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "bmp" | "gif" | "webp")
    )
}

async fn watch_queue(dir: PathBuf, tx: mpsc::Sender<Job>, mode: Dither) {
    loop {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_file() && is_image(p))
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        for p in files {
            // 等文件写完：大小 300ms 内不再变化
            let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            tokio::time::sleep(Duration::from_millis(300)).await;
            if std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0) != size {
                continue;
            }
            let name = p.file_name().unwrap_or_default().to_owned();
            match image_io::load_gray(&p) {
                Ok(img) => {
                    eprintln!("🖨  队列：{}", name.to_string_lossy());
                    if tx.send(Job::Image(img, mode)).await.is_err() {
                        return;
                    }
                }
                Err(e) => eprintln!("   ✗ 读不了 {}：{e}", name.to_string_lossy()),
            }
            let _ = std::fs::rename(&p, dir.join("done").join(&name));
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn serve(listener: TcpListener, tx: mpsc::Sender<Job>, status: Status, mode: Dither) {
    loop {
        let Ok((sock, _)) = listener.accept().await else {
            continue;
        };
        let (tx, status) = (tx.clone(), status.clone());
        tokio::spawn(async move {
            if let Err(e) = handle(sock, tx, status, mode).await {
                eprintln!("   HTTP 错误：{e}");
            }
        });
    }
}

const MAX_BODY: usize = 20 * 1024 * 1024;

async fn handle(
    mut sock: TcpStream,
    tx: mpsc::Sender<Job>,
    status: Status,
    mode: Dither,
) -> Result<()> {
    // 读请求头
    let mut buf = Vec::with_capacity(4096);
    let header_end = loop {
        let mut tmp = [0u8; 4096];
        let n = sock.read(&mut tmp).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        if buf.len() > 64 * 1024 {
            return reply(&mut sock, 431, "headers too large").await;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut first = head.lines().next().unwrap_or("").split_whitespace();
    let (method, path) = (first.next().unwrap_or(""), first.next().unwrap_or("/"));
    let len: usize = head
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.eq_ignore_ascii_case("content-length")
                .then(|| v.trim().parse().ok())?
        })
        .unwrap_or(0);
    if len > MAX_BODY {
        return reply(&mut sock, 413, "body too large").await;
    }
    let mut body = buf[header_end..].to_vec();
    while body.len() < len {
        let mut tmp = vec![0u8; (len - body.len()).min(64 * 1024)];
        let n = sock.read(&mut tmp).await?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }

    match (method, path) {
        ("GET", "/" | "/status") => {
            let s = status.lock().unwrap().clone();
            reply(&mut sock, 200, &format!("kami daemon: {s}\n")).await
        }
        ("POST", "/print") => match image_io::decode_gray(&body) {
            Ok(img) => {
                tx.send(Job::Image(img, mode)).await.ok();
                reply(&mut sock, 202, "queued\n").await
            }
            Err(e) => reply(&mut sock, 400, &format!("bad image: {e}\n")).await,
        },
        ("POST", "/text") => {
            let text = String::from_utf8_lossy(&body).to_string();
            if text.trim().is_empty() {
                return reply(&mut sock, 400, "empty text\n").await;
            }
            tx.send(Job::Text(text)).await.ok();
            reply(&mut sock, 202, "queued\n").await
        }
        _ => reply(&mut sock, 404, "not found\n").await,
    }
}

async fn reply(sock: &mut TcpStream, code: u16, body: &str) -> Result<()> {
    let reason = match code {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        404 => "Not Found",
        413 => "Payload Too Large",
        _ => "Error",
    };
    let msg = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    sock.write_all(msg.as_bytes()).await?;
    Ok(())
}
