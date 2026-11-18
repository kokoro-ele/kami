//! 小票排版：文字块、字幕纸带、测试页。

use crate::raster::Gray;
use crate::text::{Ink, TextRenderer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// 从上往下排版的画布，高度自动增长
pub struct Canvas<'a> {
    pub img: Gray,
    pub y: f32,
    pub margin: f32,
    r: &'a TextRenderer,
}

impl<'a> Canvas<'a> {
    pub fn new(r: &'a TextRenderer, width: usize) -> Self {
        Self {
            img: Gray::new(width, 64, 255),
            y: 12.0,
            margin: 12.0,
            r,
        }
    }

    pub fn inner_width(&self) -> f32 {
        self.img.width as f32 - 2.0 * self.margin
    }

    pub fn text(&mut self, s: &str, px: f32, align: Align) -> &mut Self {
        let (_, line_h) = self.r.line_metrics(px);
        for line in self.r.wrap(s, px, self.inner_width()) {
            let w = self.r.measure(&line, px);
            let x = match align {
                Align::Left => self.margin,
                Align::Center => (self.img.width as f32 - w) / 2.0,
                Align::Right => self.img.width as f32 - self.margin - w,
            };
            self.r
                .draw_line(&mut self.img, &line, x.max(0.0), self.y, px, Ink::Black);
            self.y += line_h;
        }
        self
    }

    /// 黑底白字的整行色块
    pub fn banner(&mut self, s: &str, px: f32) -> &mut Self {
        let (_, line_h) = self.r.line_metrics(px);
        let pad = px * 0.25;
        let h = line_h + pad * 2.0;
        let (x0, x1) = (self.margin as usize, self.img.width - self.margin as usize);
        self.img
            .fill_rect(x0, self.y as usize, x1, (self.y + h) as usize, 0);
        let w = self.r.measure(s, px);
        let x = (self.img.width as f32 - w) / 2.0;
        self.r
            .draw_line(&mut self.img, s, x.max(0.0), self.y + pad, px, Ink::White);
        self.y += h;
        self
    }

    pub fn rule(&mut self, thickness: usize) -> &mut Self {
        let (x0, x1) = (self.margin as usize, self.img.width - self.margin as usize);
        let y = self.y as usize;
        self.img.fill_rect(x0, y, x1, y + thickness, 0);
        self.y += thickness as f32;
        self
    }

    pub fn dashed(&mut self) -> &mut Self {
        let y = self.y as usize;
        self.img.ensure_height(y + 2);
        let mut x = self.margin as usize;
        while x + 4 < self.img.width - self.margin as usize {
            self.img.fill_rect(x, y, x + 4, y + 2, 0);
            x += 8;
        }
        self.y += 2.0;
        self
    }

    pub fn space(&mut self, dy: f32) -> &mut Self {
        self.y += dy;
        self
    }

    pub fn finish(mut self) -> Gray {
        let h = (self.y + self.margin).ceil() as usize;
        self.img.ensure_height(h);
        self.img.truncate(h);
        self.img
    }
}

/// 一段文字
pub fn text_block(r: &TextRenderer, text: &str, px: f32, align: Align, width: usize) -> Gray {
    let mut c = Canvas::new(r, width);
    c.text(text, px, align);
    c.finish()
}

#[derive(Clone, Debug)]
pub struct TapeEntry {
    pub time: String,
    pub text: String,
}

impl TapeEntry {
    /// 解析 "HH:MM:SS<TAB>文本"；没有时间戳时用当前时间
    pub fn parse(line: &str, now: impl FnOnce() -> String) -> Option<Self> {
        let line = line.trim_end();
        if line.trim().is_empty() {
            return None;
        }
        Some(match line.split_once('\t') {
            Some((t, s)) if !s.trim().is_empty() => Self {
                time: t.into(),
                text: s.trim().into(),
            },
            _ => Self {
                time: now(),
                text: line.trim().into(),
            },
        })
    }
}

/// 字幕纸带：可选抬头 + 若干条「时间 / 正文」
pub fn tape(r: &TextRenderer, title: Option<&str>, entries: &[TapeEntry], width: usize) -> Gray {
    let mut c = Canvas::new(r, width);
    if let Some(t) = title {
        c.text(t, 24.0, Align::Left).space(2.0).rule(3).space(8.0);
    }
    for e in entries {
        c.text(&e.time, 16.0, Align::Left);
        c.text(&e.text, 24.0, Align::Left).space(6.0);
    }
    c.finish()
}

/// 测试页：文字、反白、灰阶条、边框
pub fn test_page(r: &TextRenderer, width: usize, printer: &str) -> Gray {
    let mut c = Canvas::new(r, width);
    c.banner("kami 紙", 40.0).space(8.0);
    c.text(&format!("连接成功：{printer}"), 22.0, Align::Center);
    c.text("いろはにほへと ABC 123", 22.0, Align::Center)
        .space(8.0)
        .dashed()
        .space(10.0);
    let y = c.y as usize;
    let steps = 8;
    let bw = (width - 24) / steps;
    for i in 0..steps {
        let v = (i * 255 / (steps - 1)) as u8;
        c.img
            .fill_rect(12 + i * bw, y, 12 + (i + 1) * bw, y + 48, v);
    }
    c.space(60.0).rule(3);
    c.finish()
}
