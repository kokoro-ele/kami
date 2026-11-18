//! 用系统 CJK 字体把文字渲染进灰度图（fontdue）。

use std::path::{Path, PathBuf};

use fontdue::{Font, FontSettings};

use crate::raster::Gray;
use crate::Result;

/// 按平台排好优先级的候选字体（都覆盖中日文）
pub fn font_candidates() -> Vec<PathBuf> {
    let list: &[&str] = if cfg!(target_os = "macos") {
        &[
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/System/Library/Fonts/STHeiti Medium.ttc",
            "/System/Library/Fonts/ヒラギノ角ゴシック W6.ttc",
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        ]
    } else if cfg!(target_os = "windows") {
        &[
            "C:\\Windows\\Fonts\\msyhbd.ttc",
            "C:\\Windows\\Fonts\\msyh.ttc",
            "C:\\Windows\\Fonts\\YuGothB.ttc",
            "C:\\Windows\\Fonts\\meiryo.ttc",
            "C:\\Windows\\Fonts\\simhei.ttf",
        ]
    } else {
        &[
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        ]
    };
    list.iter().map(PathBuf::from).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ink {
    /// 黑字（白底）
    Black,
    /// 白字（黑底反白）
    White,
}

pub struct TextRenderer {
    font: Font,
    pub source: PathBuf,
}

impl TextRenderer {
    /// 加载字体：优先用传入路径，否则按平台候选列表找第一个能用的
    pub fn load(custom: Option<&Path>) -> Result<Self> {
        let mut tried = Vec::new();
        let candidates: Vec<PathBuf> = match custom {
            Some(p) => vec![p.to_path_buf()],
            None => font_candidates(),
        };
        for path in candidates {
            if !path.exists() {
                continue;
            }
            match std::fs::read(&path) {
                Ok(bytes) => match Self::from_bytes(bytes, 0) {
                    Ok(mut r) => {
                        r.source = path;
                        return Ok(r);
                    }
                    Err(e) => tried.push(format!("{}: {e}", path.display())),
                },
                Err(e) => tried.push(format!("{}: {e}", path.display())),
            }
        }
        Err(format!(
            "找不到可用的中文字体，请用 --font 指定一个 .ttf/.ttc。尝试过: {}",
            if tried.is_empty() {
                "（候选字体都不存在）".into()
            } else {
                tried.join("; ")
            }
        )
        .into())
    }

    pub fn from_bytes(bytes: Vec<u8>, collection_index: u32) -> Result<Self> {
        let settings = FontSettings {
            collection_index,
            ..FontSettings::default()
        };
        let font = Font::from_bytes(bytes, settings).map_err(|e| e.to_string())?;
        Ok(Self {
            font,
            source: PathBuf::new(),
        })
    }

    /// (ascent, 行高)
    pub fn line_metrics(&self, px: f32) -> (f32, f32) {
        match self.font.horizontal_line_metrics(px) {
            Some(m) => (m.ascent, (m.ascent - m.descent + m.line_gap).max(px)),
            None => (px * 0.88, px * 1.25),
        }
    }

    pub fn advance(&self, ch: char, px: f32) -> f32 {
        self.font.metrics(ch, px).advance_width
    }

    pub fn measure(&self, s: &str, px: f32) -> f32 {
        s.chars().map(|c| self.advance(c, px)).sum()
    }

    /// 按像素宽度折行。英文按单词断，中日文按字断；保留原有换行。
    pub fn wrap(&self, text: &str, px: f32, max_w: f32) -> Vec<String> {
        let mut out = Vec::new();
        for para in text.split('\n') {
            let mut line = String::new();
            let mut w = 0.0f32;
            for token in tokenize(para) {
                let tw = self.measure(&token, px);
                if w + tw <= max_w || line.is_empty() && tw <= max_w {
                    line.push_str(&token);
                    w += tw;
                    continue;
                }
                if tw <= max_w {
                    out.push(std::mem::take(&mut line).trim_end().to_string());
                    let t = token.trim_start();
                    w = self.measure(t, px);
                    line.push_str(t);
                    continue;
                }
                // 超长 token（很长的英文单词/网址）逐字硬断
                for ch in token.chars() {
                    let cw = self.advance(ch, px);
                    if w + cw > max_w && !line.is_empty() {
                        out.push(std::mem::take(&mut line));
                        w = 0.0;
                    }
                    line.push(ch);
                    w += cw;
                }
            }
            out.push(line.trim_end().to_string());
        }
        out
    }

    /// 在 (x, top) 处画一行字，top 是这一行的顶部
    pub fn draw_line(&self, img: &mut Gray, s: &str, x: f32, top: f32, px: f32, ink: Ink) {
        let (ascent, line_h) = self.line_metrics(px);
        let baseline = top + ascent;
        img.ensure_height((top + line_h).ceil() as usize + 1);
        let mut pen = x;
        for ch in s.chars() {
            let (m, cov) = self.font.rasterize(ch, px);
            let gx = (pen + m.xmin as f32).round() as i64;
            let gy = (baseline - (m.height as f32 + m.ymin as f32)).round() as i64;
            for row in 0..m.height {
                let py = gy + row as i64;
                if py < 0 {
                    continue;
                }
                let py = py as usize;
                img.ensure_height(py + 1);
                for col in 0..m.width {
                    let px_ = gx + col as i64;
                    if px_ < 0 || px_ as usize >= img.width {
                        continue;
                    }
                    let c = cov[row * m.width + col] as u32;
                    if c == 0 {
                        continue;
                    }
                    let old = img.get(px_ as usize, py) as u32;
                    let new = match ink {
                        Ink::Black => old * (255 - c) / 255,
                        Ink::White => old + (255 - old) * c / 255,
                    };
                    img.set(px_ as usize, py, new as u8);
                }
            }
            pen += m.advance_width;
        }
    }
}

/// 切分成可断行的单元：连续的拉丁字母/数字算一个单词，其余每个字符单独一个
fn tokenize(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric()
            || (ch.is_ascii_punctuation() && !word.is_empty() && ch != '(')
        {
            word.push(ch);
        } else {
            if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
            out.push(ch.to_string());
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}
