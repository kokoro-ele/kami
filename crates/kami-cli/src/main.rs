//! kami 紙 — 蓝牙热敏小打印机命令行工具
//!
//!   kami scan                       列出附近的打印机
//!   kami test                       打印测试页
//!   kami print a.png b.jpg          打印图片
//!   kami text "今天也要加油"          打印文字（"-" 从标准输入读）
//!   livecap | kami tape             把逐行输入打成纸带（字幕、日志…）
//!   kami daemon                     常驻：自动连接 + 队列文件夹 + 本地 HTTP

mod daemon;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::{Args, Parser, Subcommand};
use kami_core::ble::{self, ConnectOptions, Printer};
use kami_core::layout::{self, Align, TapeEntry};
use kami_core::protocol::JobOptions;
use kami_core::raster::{self, Dither, Gray};
use kami_core::text::TextRenderer;
use kami_core::{image_io, Result};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

pub const DEFAULT_WIDTH: usize = 384;

#[derive(Parser)]
#[command(name = "kami", version, about = "紙 kami — 蓝牙热敏小打印机命令行工具")]
struct Cli {
    #[command(flatten)]
    g: Global,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Args, Clone, Debug)]
pub struct Global {
    /// 只连接名字以此开头的打印机（未知机型会按 LuckJingle 协议尝试）
    #[arg(long, global = true, env = "KAMI_PREFIX")]
    pub prefix: Option<String>,
    /// 浓度 0 淡 / 1 中 / 2 深
    #[arg(long, global = true, default_value_t = 1, env = "KAMI_DENSITY")]
    pub density: u8,
    /// 打完后走纸点数（8 点 ≈ 1mm）
    #[arg(long, global = true, default_value_t = 80)]
    pub feed: u8,
    /// 图片抖动：fs / atkinson / threshold
    #[arg(long, global = true, default_value = "fs")]
    pub dither: Dither,
    /// 字体文件（默认自动找系统中日文字体）
    #[arg(long, global = true, env = "KAMI_FONT")]
    pub font: Option<PathBuf>,
    /// 不打印，只保存 PNG 预览到这个路径
    #[arg(long, global = true)]
    pub preview: Option<PathBuf>,
    /// 搜索打印机的超时秒数
    #[arg(long, global = true, default_value_t = 12)]
    pub timeout: u64,
}

#[derive(Subcommand)]
enum Cmd {
    /// 扫描附近的蓝牙设备
    Scan {
        #[arg(long, default_value_t = 6)]
        seconds: u64,
    },
    /// 打印测试页
    Test,
    /// 打印图片
    Print {
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// 打印文字（传 "-" 从标准输入读取）
    Text {
        text: String,
        #[arg(long, default_value_t = 28.0)]
        size: f32,
        #[arg(long)]
        center: bool,
    },
    /// 逐行读取标准输入，攒批打成纸带（格式 "HH:MM:SS<TAB>文本" 或纯文本）
    Tape {
        #[arg(long)]
        title: Option<String>,
        /// 攒几行打一次
        #[arg(long, default_value_t = 4)]
        lines: usize,
        /// 安静几秒后把剩余内容打出来
        #[arg(long, default_value_t = 6.0)]
        idle: f64,
    },
    /// 常驻：自动连接/重连，监听队列文件夹和本地 HTTP
    Daemon {
        /// 队列文件夹（丢进图片就打印）
        #[arg(long)]
        queue: Option<PathBuf>,
        #[arg(long, default_value_t = 7878)]
        port: u16,
        #[arg(long)]
        no_http: bool,
    },
}

/// 一次命令执行期间的连接、字体、预览计数
pub struct Session {
    pub g: Global,
    printer: Option<Printer>,
    font: Option<TextRenderer>,
    previews: usize,
}

impl Session {
    pub fn new(g: Global) -> Self {
        Self {
            g,
            printer: None,
            font: None,
            previews: 0,
        }
    }

    pub fn connect_opts(&self) -> ConnectOptions {
        ConnectOptions {
            name_prefix: self.g.prefix.clone(),
            timeout: Duration::from_secs(self.g.timeout),
        }
    }

    pub async fn printer(&mut self) -> Result<&mut Printer> {
        let alive = match &self.printer {
            Some(p) => p.is_connected().await,
            None => false,
        };
        if !alive {
            eprintln!("🔍 搜索打印机…");
            let p = Printer::connect(&self.connect_opts()).await?;
            eprintln!("✅ 已连接 {}（{}）", p.name(), p.driver_name());
            self.printer = Some(p);
        }
        Ok(self.printer.as_mut().unwrap())
    }

    pub fn font(&mut self) -> Result<&TextRenderer> {
        if self.font.is_none() {
            self.font = Some(TextRenderer::load(self.g.font.as_deref())?);
        }
        Ok(self.font.as_ref().unwrap())
    }

    pub fn width(&self) -> usize {
        self.printer
            .as_ref()
            .map(|p| p.width())
            .unwrap_or(DEFAULT_WIDTH)
    }

    /// 输出一张灰度图：预览模式存 PNG，否则抖动后发给打印机
    pub async fn emit(&mut self, img: &Gray, mode: Dither) -> Result<()> {
        if let Some(path) = self.g.preview.clone() {
            let bmp = raster::prepare(img, self.width(), mode);
            let path = if self.previews == 0 {
                path
            } else {
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("preview");
                path.with_file_name(format!("{stem}-{}.png", self.previews + 1))
            };
            image_io::save_bitmap_png(&bmp, &path)?;
            eprintln!("🖼  预览：{}", path.display());
            self.previews += 1;
            return Ok(());
        }
        let opts = JobOptions {
            density: self.g.density,
            feed: self.g.feed,
            paper: None,
        };
        let p = self.printer().await?;
        let bmp = raster::prepare(img, p.width(), mode);
        p.print(&bmp, &opts).await
    }

    pub async fn close(self) {
        if let Some(p) = self.printer {
            p.disconnect().await.ok();
        }
    }
}

/// 排好版的文字用阈值，保持字边锐利
pub const TEXT_DITHER: Dither = Dither::Threshold(150);

pub fn now_hms() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut s = Session::new(cli.g.clone());

    let res: Result<()> = async {
        match cli.cmd {
            Cmd::Scan { seconds } => {
                eprintln!("🔍 扫描 {seconds} 秒…");
                let extra: Vec<String> = s.g.prefix.iter().cloned().collect();
                let list = ble::scan(Duration::from_secs(seconds), &extra).await?;
                if list.is_empty() {
                    println!("（没有发现带名字的设备）");
                }
                for d in list {
                    println!(
                        "{} {:<20} {:>4} dBm  {}",
                        if d.supported { "✓" } else { " " },
                        d.name,
                        d.rssi.map(|r| r.to_string()).unwrap_or_else(|| "?".into()),
                        d.id
                    );
                }
            }
            Cmd::Test => {
                let name = if s.g.preview.is_some() {
                    "预览".to_string()
                } else {
                    s.printer().await?.name().to_string()
                };
                let w = s.width();
                let img = layout::test_page(s.font()?, w, &name);
                s.emit(&img, Dither::FloydSteinberg).await?;
            }
            Cmd::Print { files } => {
                for f in files {
                    eprintln!("🖨  {}", f.display());
                    let img = image_io::load_gray(&f)?;
                    let mode = s.g.dither;
                    s.emit(&img, mode).await?;
                }
            }
            Cmd::Text { text, size, center } => {
                let text = if text == "-" {
                    let mut buf = String::new();
                    tokio::io::stdin().read_to_string(&mut buf).await?;
                    buf
                } else {
                    text
                };
                let align = if center { Align::Center } else { Align::Left };
                let w = s.width();
                let img = layout::text_block(s.font()?, text.trim_end(), size, align, w);
                s.emit(&img, TEXT_DITHER).await?;
            }
            Cmd::Tape { title, lines, idle } => tape(&mut s, title, lines, idle).await?,
            Cmd::Daemon {
                queue,
                port,
                no_http,
            } => daemon::run(s.g.clone(), queue, if no_http { None } else { Some(port) }).await?,
        }
        Ok(())
    }
    .await;

    s.close().await;
    if let Err(e) = &res {
        eprintln!("❌ {e}");
        std::process::exit(1);
    }
    Ok(())
}

async fn tape(s: &mut Session, title: Option<String>, max_lines: usize, idle: f64) -> Result<()> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut pending: Vec<TapeEntry> = Vec::new();
    let mut header = title;
    let mut last = Instant::now();
    let idle = Duration::from_secs_f64(idle);

    loop {
        let eof = match tokio::time::timeout(Duration::from_millis(500), lines.next_line()).await {
            Ok(Ok(Some(l))) => {
                if let Some(e) = TapeEntry::parse(&l, now_hms) {
                    eprintln!("[{}] {}", e.time, e.text);
                    pending.push(e);
                    last = Instant::now();
                }
                false
            }
            Ok(Ok(None)) => true,
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => false,
        };
        let due = !pending.is_empty() && (pending.len() >= max_lines || last.elapsed() > idle);
        if due || (eof && !pending.is_empty()) {
            let w = s.width();
            let img = layout::tape(s.font()?, header.take().as_deref(), &pending, w);
            s.emit(&img, TEXT_DITHER).await?;
            pending.clear();
        }
        if eof {
            return Ok(());
        }
    }
}
