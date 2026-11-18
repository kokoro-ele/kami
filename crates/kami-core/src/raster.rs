//! 灰度图、缩放、抖动、1-bit 位图。只依赖 std。

/// 8-bit 灰度图。0 = 黑，255 = 白。
#[derive(Clone, Debug, PartialEq)]
pub struct Gray {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

impl Gray {
    pub fn new(width: usize, height: usize, fill: u8) -> Self {
        Self {
            width,
            height,
            data: vec![fill; width * height],
        }
    }

    pub fn from_raw(width: usize, height: usize, data: Vec<u8>) -> Self {
        assert_eq!(data.len(), width * height, "灰度数据长度不匹配");
        Self {
            width,
            height,
            data,
        }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> u8 {
        self.data[y * self.width + x]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, v: u8) {
        self.data[y * self.width + x] = v;
    }

    /// 高度不够时在底部补白
    pub fn ensure_height(&mut self, h: usize) {
        if h > self.height {
            self.data.resize(self.width * h, 255);
            self.height = h;
        }
    }

    /// 裁掉底部，只保留前 h 行
    pub fn truncate(&mut self, h: usize) {
        let h = h.min(self.height);
        self.data.truncate(self.width * h);
        self.height = h;
    }

    pub fn fill_rect(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, v: u8) {
        let x1 = x1.min(self.width);
        self.ensure_height(y1);
        for y in y0..y1 {
            for x in x0..x1 {
                self.set(x, y, v);
            }
        }
    }

    /// 等比缩放到指定宽度（缩小用区域平均，放大用双线性）
    pub fn resize_to_width(&self, width: usize) -> Gray {
        if width == self.width || self.width == 0 {
            return self.clone();
        }
        let scale = width as f64 / self.width as f64;
        let height = ((self.height as f64) * scale).round().max(1.0) as usize;
        if scale < 1.0 {
            self.resize_area(width, height)
        } else {
            self.resize_bilinear(width, height)
        }
    }

    fn resize_area(&self, w: usize, h: usize) -> Gray {
        let mut out = Gray::new(w, h, 255);
        let sx = self.width as f64 / w as f64;
        let sy = self.height as f64 / h as f64;
        for y in 0..h {
            let y0 = (y as f64 * sy).floor() as usize;
            let y1 = (((y + 1) as f64 * sy).ceil() as usize)
                .min(self.height)
                .max(y0 + 1);
            for x in 0..w {
                let x0 = (x as f64 * sx).floor() as usize;
                let x1 = (((x + 1) as f64 * sx).ceil() as usize)
                    .min(self.width)
                    .max(x0 + 1);
                let mut sum = 0u32;
                for yy in y0..y1 {
                    let row = yy * self.width;
                    for xx in x0..x1 {
                        sum += self.data[row + xx] as u32;
                    }
                }
                let n = ((y1 - y0) * (x1 - x0)) as u32;
                out.set(x, y, (sum / n) as u8);
            }
        }
        out
    }

    fn resize_bilinear(&self, w: usize, h: usize) -> Gray {
        let mut out = Gray::new(w, h, 255);
        let sx = self.width as f64 / w as f64;
        let sy = self.height as f64 / h as f64;
        for y in 0..h {
            let fy = ((y as f64 + 0.5) * sy - 0.5).max(0.0);
            let y0 = (fy.floor() as usize).min(self.height - 1);
            let y1 = (y0 + 1).min(self.height - 1);
            let ty = fy - y0 as f64;
            for x in 0..w {
                let fx = ((x as f64 + 0.5) * sx - 0.5).max(0.0);
                let x0 = (fx.floor() as usize).min(self.width - 1);
                let x1 = (x0 + 1).min(self.width - 1);
                let tx = fx - x0 as f64;
                let a = self.get(x0, y0) as f64 * (1.0 - tx) + self.get(x1, y0) as f64 * tx;
                let b = self.get(x0, y1) as f64 * (1.0 - tx) + self.get(x1, y1) as f64 * tx;
                out.set(
                    x,
                    y,
                    (a * (1.0 - ty) + b * ty).round().clamp(0.0, 255.0) as u8,
                );
            }
        }
        out
    }

    /// 宽度不足时左右补白，超出时居中裁剪（用于已经排好版的图）
    pub fn fit_width_exact(&self, width: usize) -> Gray {
        if self.width == width {
            return self.clone();
        }
        let mut out = Gray::new(width, self.height, 255);
        if self.width < width {
            let off = (width - self.width) / 2;
            for y in 0..self.height {
                let src = &self.data[y * self.width..(y + 1) * self.width];
                out.data[y * width + off..y * width + off + self.width].copy_from_slice(src);
            }
        } else {
            let off = (self.width - width) / 2;
            for y in 0..self.height {
                let s = y * self.width + off;
                out.data[y * width..(y + 1) * width].copy_from_slice(&self.data[s..s + width]);
            }
        }
        out
    }
}

/// 1-bit 位图：每行 `row_bytes()` 字节，MSB 在左，1 = 黑（热敏头加热）。
#[derive(Clone, Debug, PartialEq)]
pub struct Bitmap {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

impl Bitmap {
    pub fn row_bytes(&self) -> usize {
        self.width.div_ceil(8)
    }

    pub fn is_black(&self, x: usize, y: usize) -> bool {
        self.data[y * self.row_bytes() + x / 8] & (0x80 >> (x % 8)) != 0
    }

    /// 展开回灰度图，用于预览
    pub fn to_gray(&self) -> Gray {
        let mut g = Gray::new(self.width, self.height, 255);
        for y in 0..self.height {
            for x in 0..self.width {
                if self.is_black(x, y) {
                    g.set(x, y, 0);
                }
            }
        }
        g
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Dither {
    /// 固定阈值，适合纯文字
    Threshold(u8),
    /// Floyd–Steinberg 误差扩散，照片通用
    #[default]
    FloydSteinberg,
    /// Atkinson，对比更强、更"复古 Mac"，热敏纸上很好看
    Atkinson,
}

impl std::str::FromStr for Dither {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "fs" | "floyd" | "floyd-steinberg" => Ok(Dither::FloydSteinberg),
            "atkinson" | "at" => Ok(Dither::Atkinson),
            "threshold" | "none" | "text" => Ok(Dither::Threshold(128)),
            other => match other.strip_prefix("threshold:") {
                Some(n) => n
                    .parse::<u8>()
                    .map(Dither::Threshold)
                    .map_err(|e| e.to_string()),
                None => Err(format!(
                    "未知抖动方式: {s}（可选 fs / atkinson / threshold）"
                )),
            },
        }
    }
}

/// 灰度 → 1-bit
pub fn dither(img: &Gray, mode: Dither) -> Bitmap {
    let (w, h) = (img.width, img.height);
    let rb = w.div_ceil(8);
    let mut out = vec![0u8; rb * h];
    let mut put = |x: usize, y: usize| out[y * rb + x / 8] |= 0x80 >> (x % 8);

    match mode {
        Dither::Threshold(t) => {
            for y in 0..h {
                for x in 0..w {
                    if img.get(x, y) < t {
                        put(x, y);
                    }
                }
            }
        }
        Dither::FloydSteinberg | Dither::Atkinson => {
            let mut buf: Vec<f32> = img.data.iter().map(|&v| v as f32).collect();
            let idx = |x: usize, y: usize| y * w + x;
            for y in 0..h {
                for x in 0..w {
                    let old = buf[idx(x, y)];
                    let new = if old < 128.0 { 0.0 } else { 255.0 };
                    if new == 0.0 {
                        put(x, y);
                    }
                    let e = old - new;
                    let mut spread = |dx: isize, dy: usize, f: f32| {
                        let nx = x as isize + dx;
                        let ny = y + dy;
                        if nx >= 0 && (nx as usize) < w && ny < h {
                            buf[idx(nx as usize, ny)] += e * f;
                        }
                    };
                    if mode == Dither::FloydSteinberg {
                        spread(1, 0, 7.0 / 16.0);
                        spread(-1, 1, 3.0 / 16.0);
                        spread(0, 1, 5.0 / 16.0);
                        spread(1, 1, 1.0 / 16.0);
                    } else {
                        let f = 1.0 / 8.0;
                        spread(1, 0, f);
                        spread(2, 0, f);
                        spread(-1, 1, f);
                        spread(0, 1, f);
                        spread(1, 1, f);
                        spread(0, 2, f);
                    }
                }
            }
        }
    }
    Bitmap {
        width: w,
        height: h,
        data: out,
    }
}

/// 细线加粗：每个黑点向右、向下各扩一个点（2×2 膨胀）。
/// 热敏头加热孤立的单点线条常常印不出来（比如注音、细字体），加粗后才能在纸上看见。
pub fn thicken(b: &Bitmap) -> Bitmap {
    let rb = b.row_bytes();
    // 先横向：每行右移一位再 OR
    let mut h = b.data.clone();
    for y in 0..b.height {
        let mut carry = 0u8;
        for i in 0..rb {
            let v = b.data[y * rb + i];
            h[y * rb + i] |= (v >> 1) | carry;
            carry = (v & 1) << 7;
        }
    }
    // 再纵向：把横向结果往下一行 OR
    let mut out = h.clone();
    for y in 0..b.height.saturating_sub(1) {
        for i in 0..rb {
            out[(y + 1) * rb + i] |= h[y * rb + i];
        }
    }
    // 宽度不是 8 的倍数时，最后一个字节多出来的位清零
    let extra = rb * 8 - b.width;
    if extra > 0 {
        let mask = 0xFFu8 << extra;
        for y in 0..b.height {
            out[y * rb + rb - 1] &= mask;
        }
    }
    Bitmap { width: b.width, height: b.height, data: out }
}

/// 一步到位：缩放到打印宽度并抖动
pub fn prepare(img: &Gray, width: usize, mode: Dither) -> Bitmap {
    dither(&img.resize_to_width(width), mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_pixel_is_msb() {
        let mut g = Gray::new(16, 1, 255);
        g.set(0, 0, 0);
        g.set(9, 0, 0);
        let b = dither(&g, Dither::Threshold(128));
        assert_eq!(b.data, vec![0x80, 0x40]);
        assert!(b.is_black(0, 0) && b.is_black(9, 0) && !b.is_black(1, 0));
    }

    #[test]
    fn white_stays_white_black_stays_black() {
        for mode in [
            Dither::FloydSteinberg,
            Dither::Atkinson,
            Dither::Threshold(128),
        ] {
            assert!(dither(&Gray::new(384, 4, 255), mode)
                .data
                .iter()
                .all(|&b| b == 0));
            assert!(dither(&Gray::new(384, 4, 0), mode)
                .data
                .iter()
                .all(|&b| b == 0xFF));
        }
    }

    #[test]
    fn mid_gray_is_about_half() {
        let b = dither(&Gray::new(384, 64, 128), Dither::FloydSteinberg);
        let ones: u32 = b.data.iter().map(|v| v.count_ones()).sum();
        let ratio = ones as f64 / (384.0 * 64.0);
        assert!((0.4..0.6).contains(&ratio), "ratio = {ratio}");
    }

    #[test]
    fn resize_keeps_aspect() {
        let g = Gray::new(1000, 500, 200);
        let r = g.resize_to_width(384);
        assert_eq!((r.width, r.height), (384, 192));
        assert!(r.data.iter().all(|&v| v == 200));
        let up = Gray::new(100, 50, 30).resize_to_width(384);
        assert_eq!((up.width, up.height), (384, 192));
        assert!(up.data.iter().all(|&v| v == 30));
    }

    #[test]
    fn fit_width_pads_and_crops() {
        let g = Gray::new(100, 2, 0);
        let p = g.fit_width_exact(104);
        assert_eq!(p.get(0, 0), 255);
        assert_eq!(p.get(2, 0), 0);
        let c = Gray::new(400, 1, 7).fit_width_exact(384);
        assert_eq!(c.data.len(), 384);
    }

    #[test]
    fn thicken_grows_single_dot() {
        let mut g = Gray::new(16, 3, 255);
        g.set(7, 0, 0);
        let b = thicken(&dither(&g, Dither::Threshold(128)));
        assert!(b.is_black(7, 0) && b.is_black(8, 0) && b.is_black(7, 1) && b.is_black(8, 1));
        assert!(!b.is_black(6, 0) && !b.is_black(9, 0) && !b.is_black(7, 2));
    }

    #[test]
    fn parse_dither() {
        assert_eq!("fs".parse::<Dither>().unwrap(), Dither::FloydSteinberg);
        assert_eq!(
            "threshold:90".parse::<Dither>().unwrap(),
            Dither::Threshold(90)
        );
        assert!("nope".parse::<Dither>().is_err());
    }
}
