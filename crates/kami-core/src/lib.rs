//! kami-core — 便宜蓝牙热敏打印机的驱动、光栅化和传输层。
//!
//! - [`raster`]   灰度图 → 缩放 → 抖动 → 1-bit 位图（纯 std）
//! - [`protocol`] 各品牌协议驱动（纯 std）
//! - [`text`]     系统字体渲染文字（feature `text`）
//! - [`layout`]   小票排版模板：文字块、字幕纸带、测试页（feature `text`）
//! - [`image_io`] 读写 PNG/JPEG（feature `image`）
//! - [`ble`]      扫描、连接、发送打印任务（feature `ble`）

pub mod protocol;
pub mod raster;

#[cfg(feature = "text")]
pub mod layout;
#[cfg(feature = "text")]
pub mod text;

#[cfg(feature = "image")]
pub mod image_io;

#[cfg(feature = "ble")]
pub mod ble;

pub type Error = Box<dyn std::error::Error + Send + Sync + 'static>;
pub type Result<T> = std::result::Result<T, Error>;

/// 便捷构造错误
pub fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(msg.into().into())
}
