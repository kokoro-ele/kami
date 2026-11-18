//! 图片读写（image crate）。透明部分按白底合成。

use std::path::Path;

use image::codecs::png::PngEncoder;
use image::{DynamicImage, ExtendedColorType, ImageEncoder};

use crate::raster::{Bitmap, Gray};
use crate::Result;

fn to_gray(img: DynamicImage) -> Gray {
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width() as usize, rgba.height() as usize);
    let data = rgba
        .pixels()
        .map(|p| {
            let [r, g, b, a] = p.0;
            let l = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) / 255.0;
            let a = a as f32 / 255.0;
            ((l * a + (1.0 - a)) * 255.0).round() as u8
        })
        .collect();
    Gray::from_raw(w, h, data)
}

pub fn load_gray(path: impl AsRef<Path>) -> Result<Gray> {
    Ok(to_gray(image::open(path)?))
}

pub fn decode_gray(bytes: &[u8]) -> Result<Gray> {
    Ok(to_gray(image::load_from_memory(bytes)?))
}

pub fn encode_png(g: &Gray) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    PngEncoder::new(&mut out).write_image(
        &g.data,
        g.width as u32,
        g.height as u32,
        ExtendedColorType::L8,
    )?;
    Ok(out)
}

pub fn save_png(g: &Gray, path: impl AsRef<Path>) -> Result<()> {
    std::fs::write(path, encode_png(g)?)?;
    Ok(())
}

/// 位图预览（白底黑点）
pub fn save_bitmap_png(b: &Bitmap, path: impl AsRef<Path>) -> Result<()> {
    save_png(&b.to_gray(), path)
}
