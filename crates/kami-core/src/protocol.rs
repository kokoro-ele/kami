//! 打印机协议驱动。只依赖 std，方便单元测试和移植。
//!
//! 新增一个品牌 = 实现 [`Driver`] 并加进 [`drivers()`]。

use crate::raster::Bitmap;

/// 一次打印任务的参数
#[derive(Clone, Copy, Debug)]
pub struct JobOptions {
    /// 浓度 0 淡 / 1 中 / 2 深
    pub density: u8,
    /// 打完后走纸点数（203dpi 下 8 点 ≈ 1mm）
    pub feed: u8,
    /// 纸张类型（LuckJingle：0 标签/间隙纸，1 黑标纸，2 连续卷纸）。None = 不设置，沿用打印机当前设置
    pub paper: Option<u8>,
}

impl Default for JobOptions {
    fn default() -> Self {
        Self {
            density: 1,
            feed: 40,
            paper: None,
        }
    }
}

pub trait Driver: Send + Sync {
    /// 人类可读的名称
    fn name(&self) -> &'static str;
    /// 根据蓝牙广播名判断是否是本驱动支持的机型
    fn matches(&self, device_name: &str) -> bool;
    /// 打印头宽度（点）
    fn width(&self) -> usize;
    /// BLE 服务 / 写入特征值 / 回执特征值（128-bit UUID）
    fn service_uuid(&self) -> u128;
    fn write_uuid(&self) -> u128;
    fn notify_uuids(&self) -> Vec<u128>;
    /// 单次写入的最大字节数，以及两次写入之间的间隔（毫秒）
    fn chunk_size(&self) -> usize {
        128
    }
    fn pace_ms(&self) -> u64 {
        35
    }
    /// 开始一次打印任务（唤醒、启用、浓度）
    fn encode_begin(&self, opts: &JobOptions) -> Vec<Vec<u8>>;
    /// 一段图像数据。连续打印时可以在一次任务里发很多段，中间不会走纸
    fn encode_raster(&self, bitmap: &Bitmap) -> Vec<Vec<u8>>;
    /// 结束任务（走纸、停止）
    fn encode_end(&self, opts: &JobOptions) -> Vec<Vec<u8>>;
    /// 完整的一次打印 = 开始 + 图像 + 结束（每条指令会再按 chunk_size 切片发送）
    fn encode(&self, bitmap: &Bitmap, opts: &JobOptions) -> Vec<Vec<u8>> {
        let mut c = self.encode_begin(opts);
        c.extend(self.encode_raster(bitmap));
        c.extend(self.encode_end(opts));
        c
    }
    /// 收到的通知是否代表"打印完成 / 已接受"
    fn is_done_ack(&self, data: &[u8]) -> bool;
}

/// 16-bit 短 UUID → 蓝牙基础 UUID
pub const fn uuid16(short: u16) -> u128 {
    ((short as u128) << 96) | 0x0000_0000_0000_1000_8000_0080_5f9b_34fb
}

// ---------------------------------------------------------------- LuckJingle / Lujiang
/// 厦门 Lujiang / LuckPrinter SDK 系列（Luck Jingle App）。
///
/// 服务 FF00：FF02 写入、FF01 回执（"OK"）、FF03 状态帧。
/// 流程：00×12 唤醒 → 10 FF F1 03 启用 → 10 FF 10 00 nn 浓度
///       → 1D 76 30 00 wL wH hL hH + 数据（GS v 0）→ 1B 4A nn 走纸 → 10 FF F1 45 结束
pub struct LuckJingle {
    pub prefixes: Vec<String>,
}

impl Default for LuckJingle {
    fn default() -> Self {
        Self {
            prefixes: vec!["PPS".into()],
        }
    }
}

/// 一个 GS v 0 光栅块最多 65535 行。LuckJingle 机型要求整张图放在同一个块里：
/// 分段发送会让机器在段与段之间走纸，打出来中间出现空白。
pub const MAX_BLOCK_ROWS: usize = u16::MAX as usize;

impl Driver for LuckJingle {
    fn name(&self) -> &'static str {
        "LuckJingle"
    }
    fn matches(&self, device_name: &str) -> bool {
        self.prefixes
            .iter()
            .any(|p| device_name.starts_with(p.as_str()))
    }
    fn width(&self) -> usize {
        384
    }
    fn service_uuid(&self) -> u128 {
        uuid16(0xFF00)
    }
    fn write_uuid(&self) -> u128 {
        uuid16(0xFF02)
    }
    fn notify_uuids(&self) -> Vec<u128> {
        vec![uuid16(0xFF01), uuid16(0xFF03)]
    }

    fn encode_begin(&self, opts: &JobOptions) -> Vec<Vec<u8>> {
        let mut c = vec![
            vec![0u8; 12],
            vec![0x10, 0xFF, 0xF1, 0x03],
            vec![0x10, 0xFF, 0x10, 0x00, opts.density.min(2)],
        ];
        // 告诉打印机纸张类型。设成连续卷纸后，部分机型不再每打一段就找标签缝隙、自动走纸
        if let Some(kind) = opts.paper {
            c.push(vec![0x10, 0xFF, 0x84, kind.min(2)]);
        }
        c
    }

    fn encode_raster(&self, bmp: &Bitmap) -> Vec<Vec<u8>> {
        let rb = bmp.row_bytes();
        let mut cmds = Vec::new();
        let mut top = 0;
        while top < bmp.height {
            let rows = MAX_BLOCK_ROWS.min(bmp.height - top);
            let mut c = Vec::with_capacity(8 + rows * rb);
            c.extend_from_slice(&[
                0x1D,
                0x76,
                0x30,
                0x00,
                (rb & 0xFF) as u8,
                (rb >> 8) as u8,
                (rows & 0xFF) as u8,
                (rows >> 8) as u8,
            ]);
            c.extend_from_slice(&bmp.data[top * rb..(top + rows) * rb]);
            cmds.push(c);
            top += rows;
        }
        cmds
    }

    fn encode_end(&self, opts: &JobOptions) -> Vec<Vec<u8>> {
        vec![vec![0x1B, 0x4A, opts.feed], vec![0x10, 0xFF, 0xF1, 0x45]]
    }

    fn is_done_ack(&self, data: &[u8]) -> bool {
        data.windows(2).any(|w| w == b"OK") || data.contains(&0xAA)
    }
}

// ---------------------------------------------------------------- 注册表
/// 所有内置驱动。`extra_prefixes` 允许用户临时把别的名字也交给 LuckJingle 驱动试试。
pub fn drivers(extra_prefixes: &[String]) -> Vec<Box<dyn Driver>> {
    let mut lj = LuckJingle::default();
    lj.prefixes.extend(extra_prefixes.iter().cloned());
    vec![Box::new(lj)]
}

pub fn driver_for(device_name: &str, extra_prefixes: &[String]) -> Option<Box<dyn Driver>> {
    drivers(extra_prefixes)
        .into_iter()
        .find(|d| d.matches(device_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raster::{dither, Dither, Gray};

    #[test]
    fn uuid_expansion() {
        assert_eq!(
            format!("{:032x}", uuid16(0xFF02)),
            "0000ff0200001000800000805f9b34fb"
        );
    }

    #[test]
    fn matches_pps1() {
        assert!(driver_for("PPS1_998D", &[]).is_some());
        assert!(driver_for("Mr.in_M02", &[]).is_none());
        assert!(driver_for("MX06", &["MX".into()]).is_some());
    }

    #[test]
    fn luckjingle_sequence() {
        let bmp = dither(&Gray::new(384, 500, 255), Dither::Threshold(128));
        let cmds = LuckJingle::default().encode(
            &bmp,
            &JobOptions {
                density: 2,
                feed: 100,
                paper: None,
            },
        );
        assert_eq!(cmds[0], vec![0; 12]);
        assert_eq!(cmds[1], vec![0x10, 0xFF, 0xF1, 0x03]);
        assert_eq!(cmds[2], vec![0x10, 0xFF, 0x10, 0x00, 2]);
        // 500 行整张放进一个光栅块（0x01F4），中间不能断
        assert_eq!(&cmds[3][..8], &[0x1D, 0x76, 0x30, 0x00, 0x30, 0x00, 0xF4, 0x01]);
        assert_eq!(cmds[3].len(), 8 + 500 * 48);
        assert_eq!(cmds[4], vec![0x1B, 0x4A, 100]);
        assert_eq!(cmds[5], vec![0x10, 0xFF, 0xF1, 0x45]);
        assert_eq!(cmds.len(), 6);
    }

    #[test]
    fn paper_type_command() {
        let opts = JobOptions {
            density: 1,
            feed: 0,
            paper: Some(2),
        };
        let begin = LuckJingle::default().encode_begin(&opts);
        assert_eq!(begin.last().unwrap(), &vec![0x10, 0xFF, 0x84, 0x02]);
    }

    #[test]
    fn ack_detection() {
        let d = LuckJingle::default();
        assert!(d.is_done_ack(b"OK"));
        assert!(d.is_done_ack(&[0xAA]));
        assert!(!d.is_done_ack(&[0x01, 0x00]));
    }
}
