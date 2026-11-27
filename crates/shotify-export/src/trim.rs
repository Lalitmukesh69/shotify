use image::RgbaImage;
use serde::{Deserialize, Serialize};

/// Options for trimming transparent outer edge seams (1..2px artifacts on export).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrimSeamsOptions {
  pub max_trim_per_side: u32,
  pub alpha_max: u8,
}

impl Default for TrimSeamsOptions {
  fn default() -> Self {
    Self {
      max_trim_per_side: 2,
      alpha_max: 0,
    }
  }
}

/// Trim up to `max_trim_per_side` (clamped 1..=8) fully transparent rows/columns from all 4 edges.
/// Port of `utils/export/imageProcessor.js::trimTransparentEdgeSeamsDataUrl`.
pub fn trim_transparent_edge_seams(img: &RgbaImage, opts: TrimSeamsOptions) -> RgbaImage {
  let (w, h) = img.dimensions();
  if w <= 1 || h <= 1 {
    return img.clone();
  }

  let threshold = opts.alpha_max;
  let max_trim = opts.max_trim_per_side.clamp(1, 8);
  let raw = img.as_raw();

  let col_transparent = |x: u32| -> bool {
    for y in 0..h {
      let a = raw[((y * w + x) as usize) * 4 + 3];
      if a > threshold {
        return false;
      }
    }
    true
  };

  let row_transparent = |y: u32| -> bool {
    let row = (y as usize) * (w as usize);
    for x in 0..w {
      let a = raw[(row + (x as usize)) * 4 + 3];
      if a > threshold {
        return false;
      }
    }
    true
  };

  let mut trim_left = 0u32;
  while trim_left < max_trim && trim_left < w - 1 && col_transparent(trim_left) {
    trim_left += 1;
  }

  let mut trim_right = 0u32;
  while trim_right < max_trim
    && trim_right < w - 1 - trim_left
    && col_transparent(w - 1 - trim_right)
  {
    trim_right += 1;
  }

  let mut trim_top = 0u32;
  while trim_top < max_trim && trim_top < h - 1 && row_transparent(trim_top) {
    trim_top += 1;
  }

  let mut trim_bottom = 0u32;
  while trim_bottom < max_trim
    && trim_bottom < h - 1 - trim_top
    && row_transparent(h - 1 - trim_bottom)
  {
    trim_bottom += 1;
  }

  if trim_left == 0 && trim_right == 0 && trim_top == 0 && trim_bottom == 0 {
    return img.clone();
  }

  let out_w = (w - trim_left - trim_right).max(1);
  let out_h = (h - trim_top - trim_bottom).max(1);
  image::imageops::crop_imm(img, trim_left, trim_top, out_w, out_h).to_image()
}

/// Options for trimming transparent macOS window capture drop-shadow padding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacOsWindowTrimOptions {
  pub alpha_threshold: u8,
  pub min_inset: u32,
}

impl Default for MacOsWindowTrimOptions {
  fn default() -> Self {
    Self {
      alpha_threshold: 250,
      min_inset: 3,
    }
  }
}

/// Trim macOS window screenshot transparent outer shadow borders when inset >= `min_inset`.
/// Keeps a 1px border halo around the opaque window rectangle.
/// Port of `utils/export/imageProcessor.js::trimMacOsWindowScreenshotDataUrl`.
pub fn trim_macos_window_screenshot(img: &RgbaImage, opts: MacOsWindowTrimOptions) -> RgbaImage {
  let (w, h) = img.dimensions();
  if w == 0 || h == 0 {
    return img.clone();
  }

  let t = opts.alpha_threshold;
  let raw = img.as_raw();
  let mut min_x = w;
  let mut min_y = h;
  let mut max_x: i64 = -1;
  let mut max_y: i64 = -1;

  for y in 0..h {
    let row = (y as usize) * (w as usize) * 4;
    for x in 0..w {
      let a = raw[row + (x as usize) * 4 + 3];
      if a >= t {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(i64::from(x));
        max_y = max_y.max(i64::from(y));
      }
    }
  }

  if max_x < 0 || max_y < 0 {
    return img.clone();
  }

  let min_x = min_x.saturating_sub(1);
  let min_y = min_y.saturating_sub(1);
  let max_x = ((max_x as u32) + 1).min(w - 1);
  let max_y = ((max_y as u32) + 1).min(h - 1);

  let inset_x = min_x.min(w - 1 - max_x);
  let inset_y = min_y.min(h - 1 - max_y);
  let inset = inset_x.min(inset_y);
  if inset < opts.min_inset {
    return img.clone();
  }

  let crop_w = (max_x - min_x + 1).max(1);
  let crop_h = (max_y - min_y + 1).max(1);
  image::imageops::crop_imm(img, min_x, min_y, crop_w, crop_h).to_image()
}

#[cfg(test)]
mod tests {
  use super::*;
  use image::Rgba;

  #[test]
  fn trims_up_to_two_transparent_seam_pixels() {
    let mut img = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 0]));
    for y in 2..9 {
      for x in 1..8 {
        img.put_pixel(x, y, Rgba([20, 40, 60, 255]));
      }
    }
    let out = trim_transparent_edge_seams(&img, TrimSeamsOptions::default());
    // Trims 1 left, 2 right, 2 top, 1 bottom => 7x7
    assert_eq!(out.dimensions(), (7, 7));
  }

  #[test]
  fn macos_window_trim_removes_shadow_border() {
    let mut img = RgbaImage::from_pixel(60, 60, Rgba([0, 0, 0, 20]));
    for y in 10..50 {
      for x in 10..50 {
        img.put_pixel(x, y, Rgba([255, 255, 255, 255]));
      }
    }
    let out = trim_macos_window_screenshot(&img, MacOsWindowTrimOptions::default());
    // 10..49 with 1px halo -> 9..50 => 42x42
    assert_eq!(out.dimensions(), (42, 42));
  }
}
