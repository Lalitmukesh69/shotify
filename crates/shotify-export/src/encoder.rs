use std::io::Cursor;

use image::{
  imageops::FilterType,
  codecs::{
    jpeg::JpegEncoder,
    png::{CompressionType, FilterType as PngFilterType, PngEncoder},
    webp::WebPEncoder,
  },
  DynamicImage, ImageEncoder, RgbImage, RgbaImage,
};
use serde::{Deserialize, Serialize};

use crate::error::{ExportError, Result};

/// Supported output formats matching Shotify's export drawer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
  Png,
  Jpeg,
  Webp,
}

impl Default for ExportFormat {
  fn default() -> Self {
    Self::Png
  }
}

impl ExportFormat {
  pub fn parse(raw: &str) -> Result<Self> {
    match raw.trim().to_ascii_lowercase().as_str() {
      "png" | "image/png" => Ok(Self::Png),
      "jpg" | "jpeg" | "image/jpg" | "image/jpeg" => Ok(Self::Jpeg),
      "webp" | "image/webp" => Ok(Self::Webp),
      other => Err(ExportError::InvalidFormat(other.to_string())),
    }
  }

  pub fn from_lossy_or_default(raw: &str) -> Self {
    Self::parse(raw).unwrap_or(Self::Png)
  }

  pub fn extension(self) -> &'static str {
    match self {
      Self::Png => "png",
      Self::Jpeg => "jpg",
      Self::Webp => "webp",
    }
  }

  pub fn mime_type(self) -> &'static str {
    match self {
      Self::Png => "image/png",
      Self::Jpeg => "image/jpeg",
      Self::Webp => "image/webp",
    }
  }
}

/// Encoding and scaling parameters (`imageProcessor.js::scaleAndEncodeBlob`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncodeOptions {
  #[serde(default)]
  pub format: ExportFormat,
  #[serde(default = "default_quality")]
  pub quality: f32,
  #[serde(default = "default_scale")]
  pub scale: f64,
  #[serde(default)]
  pub target_width: Option<u32>,
  #[serde(default)]
  pub target_height: Option<u32>,
}

fn default_quality() -> f32 {
  0.95
}
fn default_scale() -> f64 {
  1.0
}

impl Default for EncodeOptions {
  fn default() -> Self {
    Self {
      format: ExportFormat::Png,
      quality: 0.95,
      scale: 1.0,
      target_width: None,
      target_height: None,
    }
  }
}

/// Resize an `RgbaImage` cleanly using high-quality Lanczos3 resampling when dimensions differ.
pub fn resize_for_export(img: &RgbaImage, out_w: u32, out_h: u32) -> RgbaImage {
  let target_w = out_w.max(1);
  let target_h = out_h.max(1);
  if img.width() == target_w && img.height() == target_h {
    return img.clone();
  }
  DynamicImage::ImageRgba8(img.clone())
    .resize_exact(target_w, target_h, FilterType::Lanczos3)
    .to_rgba8()
}

/// Flatten straight-alpha RGBA over a white matte to produce opaque RGB for JPEG encoding.
pub fn rgba_to_rgb_white_matte(img: &RgbaImage) -> RgbImage {
  let (w, h) = img.dimensions();
  let mut out = RgbImage::new(w, h);
  for (src, dst) in img.as_raw().chunks_exact(4).zip(out.as_mut().chunks_exact_mut(3)) {
    let a = f32::from(src[3]) / 255.0;
    let inv = 1.0 - a;
    dst[0] = (f32::from(src[0]) * a + 255.0 * inv).round().clamp(0.0, 255.0) as u8;
    dst[1] = (f32::from(src[1]) * a + 255.0 * inv).round().clamp(0.0, 255.0) as u8;
    dst[2] = (f32::from(src[2]) * a + 255.0 * inv).round().clamp(0.0, 255.0) as u8;
  }
  out
}

/// Encode an `RgbaImage` into PNG, JPEG (quality `0.1..=1.0`), or WebP bytes.
pub fn encode_rgba_image(img: &RgbaImage, format: ExportFormat, quality: f32) -> Result<Vec<u8>> {
  let q = if quality.is_finite() {
    quality.clamp(0.1, 1.0)
  } else {
    0.95
  };
  let (w, h) = img.dimensions();
  let mut out = Vec::new();

  match format {
    ExportFormat::Png => {
      let encoder = PngEncoder::new_with_quality(
        &mut out,
        CompressionType::Default,
        PngFilterType::Adaptive,
      );
      encoder
        .write_image(img.as_raw(), w, h, image::ExtendedColorType::Rgba8)
        .map_err(|e| ExportError::Encoding(format!("PNG encode failed: {e}")))?;
    }
    ExportFormat::Jpeg => {
      let rgb = rgba_to_rgb_white_matte(img);
      let q_u8 = (q * 100.0).round().clamp(1.0, 100.0) as u8;
      let encoder = JpegEncoder::new_with_quality(&mut out, q_u8);
      encoder
        .write_image(rgb.as_raw(), w, h, image::ExtendedColorType::Rgb8)
        .map_err(|e| ExportError::Encoding(format!("JPEG encode failed: {e}")))?;
    }
    ExportFormat::Webp => {
      #[cfg(feature = "lossy-webp")]
      if q < 0.999 {
        let enc = webp::Encoder::from_rgba(img.as_raw(), w, h);
        let mem = enc.encode(q * 100.0);
        out.extend_from_slice(&mem);
        return Ok(out);
      }

      let encoder = WebPEncoder::new_lossless(Cursor::new(&mut out));
      encoder
        .write_image(img.as_raw(), w, h, image::ExtendedColorType::Rgba8)
        .map_err(|e| ExportError::Encoding(format!("WebP encode failed: {e}")))?;
    }
  }

  Ok(out)
}

#[cfg(test)]
mod tests {
  use super::*;
  use image::Rgba;

  #[test]
  fn encodes_and_decodes_png_jpeg_and_webp() {
    let mut img = RgbaImage::new(64, 48);
    for y in 0..48 {
      for x in 0..64 {
        img.put_pixel(x, y, Rgba([(x * 3) as u8, (y * 5) as u8, 200, 255]));
      }
    }

    for fmt in [ExportFormat::Png, ExportFormat::Jpeg, ExportFormat::Webp] {
      let bytes = encode_rgba_image(&img, fmt, 0.85).expect("encode succeeds");
      assert!(!bytes.is_empty());
      let decoded = image::load_from_memory(&bytes).expect("roundtrip decode succeeds");
      assert_eq!(decoded.width(), 64);
      assert_eq!(decoded.height(), 48);
    }
  }

  #[test]
  fn lossy_webp_smaller_at_low_quality() {
    let mut img = RgbaImage::new(128, 128);
    for y in 0..128 {
      for x in 0..128 {
        let r = ((x as f32 * 1.7).sin() * 127.0 + 128.0) as u8;
        let g = ((y as f32 * 2.1).cos() * 127.0 + 128.0) as u8;
        let b = (((x ^ y) as f32 * 0.9).sin() * 127.0 + 128.0) as u8;
        img.put_pixel(x, y, Rgba([r, g, b, 255]));
      }
    }
    let high_q = encode_rgba_image(&img, ExportFormat::Webp, 0.92).unwrap();
    let low_q = encode_rgba_image(&img, ExportFormat::Webp, 0.2).unwrap();
    assert!(low_q.len() < high_q.len());
  }
}
