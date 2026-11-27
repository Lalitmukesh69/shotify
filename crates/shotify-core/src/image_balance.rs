use image::{imageops::FilterType, DynamicImage, RgbaImage};
use serde::{Deserialize, Serialize};

use crate::math::clamp_f64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RgbColor {
  pub r: u8,
  pub g: u8,
  pub b: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EstimatedEdgeBackground {
  pub r: u8,
  pub g: u8,
  pub b: u8,
  pub sampled: usize,
  pub edge_size: u32,
  pub alpha_min_bg: u8,
  pub mad: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceOptions {
  pub analysis_max_dim: u32,
  pub threshold: f64,
  pub alpha_min: u8,
  pub edge_size: u32,
  pub alpha_min_bg: u8,
  pub content_density: f64,
  pub min_content_pixels: usize,
  pub min_bg_samples: usize,
  pub max_edge_noise: u32,
  pub min_margin_fraction: f64,
  pub bg: Option<RgbColor>,
}

impl Default for BalanceOptions {
  fn default() -> Self {
    Self {
      analysis_max_dim: 1024,
      threshold: 12.0,
      alpha_min: 8,
      edge_size: 20,
      alpha_min_bg: 180,
      content_density: 0.01,
      min_content_pixels: 6,
      min_bg_samples: 200,
      max_edge_noise: 22,
      min_margin_fraction: 0.01,
      bg: None,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CropRect {
  pub x: u32,
  pub y: u32,
  pub w: u32,
  pub h: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeBounds {
  pub left: u32,
  pub top: u32,
  pub right: u32,
  pub bottom: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeMargins {
  pub top: u32,
  pub left: u32,
  pub right: u32,
  pub bottom: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceDetectionResult {
  pub has_content: bool,
  pub bg: EstimatedEdgeBackground,
  pub threshold: f64,
  pub alpha_min: u8,
  pub ratio: f64,
  pub sw: u32,
  pub sh: u32,
  pub content: EdgeBounds,
  pub margins: EdgeMargins,
  pub uniform_margin: u32,
  pub crop: CropRect,
  pub skipped_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceDimensions {
  pub width: u32,
  pub height: u32,
  pub x: u32,
  pub y: u32,
  pub original_width: u32,
  pub original_height: u32,
}

#[inline]
fn color_dist(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8) -> f64 {
  let dr = f64::from(r1) - f64::from(r2);
  let dg = f64::from(g1) - f64::from(g2);
  let db = f64::from(b1) - f64::from(b2);
  (dr * dr + dg * dg + db * db).sqrt()
}

fn median_sorted(mut values: Vec<u8>) -> (u8, f64) {
  if values.is_empty() {
    return (224, 0.0);
  }
  values.sort_unstable();
  let med = values[values.len() / 2];
  let sum_dev: usize = values.iter().map(|&v| v.abs_diff(med) as usize).sum();
  let mad = (sum_dev as f64) / (values.len() as f64);
  (med, mad)
}

/// Robust background estimation: sample edge strips and compute per-channel median + MAD.
/// Port of `utils/image/imageBalance.js::estimateBackgroundFromEdges`.
pub fn estimate_background_from_edges(
  img: &RgbaImage,
  edge_size_opt: u32,
  alpha_min_bg: u8,
) -> EstimatedEdgeBackground {
  let (w, h) = img.dimensions();
  if w == 0 || h == 0 {
    return EstimatedEdgeBackground {
      r: 224,
      g: 224,
      b: 224,
      sampled: 0,
      edge_size: 1,
      alpha_min_bg,
      mad: 0,
    };
  }

  let edge_size = edge_size_opt.clamp(1, w.min(h));
  let raw = img.as_raw();
  let mut rs = Vec::new();
  let mut gs = Vec::new();
  let mut bs = Vec::new();

  let mut push_px = |idx: usize| {
    if raw[idx + 3] < alpha_min_bg {
      return;
    }
    rs.push(raw[idx]);
    gs.push(raw[idx + 1]);
    bs.push(raw[idx + 2]);
  };

  for y in 0..edge_size {
    let row = (y as usize) * (w as usize) * 4;
    for x in 0..w {
      push_px(row + (x as usize) * 4);
    }
  }
  for y in (h - edge_size)..h {
    let row = (y as usize) * (w as usize) * 4;
    for x in 0..w {
      push_px(row + (x as usize) * 4);
    }
  }
  for y in 0..h {
    let row = (y as usize) * (w as usize) * 4;
    for x in 0..edge_size {
      push_px(row + (x as usize) * 4);
    }
  }
  for y in 0..h {
    let row = (y as usize) * (w as usize) * 4;
    for x in (w - edge_size)..w {
      push_px(row + (x as usize) * 4);
    }
  }

  if rs.is_empty() {
    return EstimatedEdgeBackground {
      r: 224,
      g: 224,
      b: 224,
      sampled: 0,
      edge_size,
      alpha_min_bg,
      mad: 0,
    };
  }

  let sampled = rs.len();
  let (r, mad_r) = median_sorted(rs);
  let (g, mad_g) = median_sorted(gs);
  let (b, mad_b) = median_sorted(bs);
  let mad = ((mad_r + mad_g + mad_b) / 3.0).round() as u32;

  EstimatedEdgeBackground {
    r,
    g,
    b,
    sampled,
    edge_size,
    alpha_min_bg,
    mad,
  }
}

/// Detect balanced crop rectangle matching Xnapper-style density scanning algorithm.
/// Port of `utils/image/imageBalance.js::detectBalancedCropRect`.
pub fn detect_balanced_crop_rect(img: &RgbaImage, opts: &BalanceOptions) -> BalanceDetectionResult {
  let (iw, ih) = img.dimensions();
  if iw == 0 || ih == 0 {
    return BalanceDetectionResult {
      has_content: false,
      bg: EstimatedEdgeBackground {
        r: 224,
        g: 224,
        b: 224,
        sampled: 0,
        edge_size: 1,
        alpha_min_bg: opts.alpha_min_bg,
        mad: 0,
      },
      threshold: opts.threshold,
      alpha_min: opts.alpha_min,
      ratio: 1.0,
      sw: 0,
      sh: 0,
      content: EdgeBounds { left: 0, top: 0, right: 0, bottom: 0 },
      margins: EdgeMargins { top: 0, left: 0, right: 0, bottom: 0 },
      uniform_margin: 0,
      crop: CropRect { x: 0, y: 0, w: iw, h: ih },
      skipped_reason: Some("empty-image".to_string()),
    };
  }

  let analysis_max_dim = opts.analysis_max_dim.clamp(256, 3000);
  let ratio = (f64::from(analysis_max_dim) / f64::from(iw.max(ih))).min(1.0);
  let sw = ((f64::from(iw) * ratio).round() as u32).max(1);
  let sh = ((f64::from(ih) * ratio).round() as u32).max(1);

  let analysis_img = if sw == iw && sh == ih {
    img.clone()
  } else {
    DynamicImage::ImageRgba8(img.clone())
      .resize_exact(sw, sh, FilterType::Triangle)
      .to_rgba8()
  };

  let threshold = opts.threshold.max(1.0);
  let alpha_min = opts.alpha_min;
  let content_density = clamp_f64(opts.content_density, 0.003, 0.5);
  let min_content_pixels = opts.min_content_pixels.max(1);

  let bg = match opts.bg {
    Some(c) => EstimatedEdgeBackground {
      r: c.r,
      g: c.g,
      b: c.b,
      sampled: opts.min_bg_samples,
      edge_size: opts.edge_size,
      alpha_min_bg: opts.alpha_min_bg,
      mad: 0,
    },
    None => estimate_background_from_edges(&analysis_img, opts.edge_size, opts.alpha_min_bg),
  };

  if bg.sampled < opts.min_bg_samples || bg.mad > opts.max_edge_noise {
    let reason = if bg.sampled < opts.min_bg_samples {
      "edge-samples-low"
    } else {
      "edge-noise-high"
    };
    return BalanceDetectionResult {
      has_content: true,
      bg,
      threshold,
      alpha_min,
      ratio,
      sw,
      sh,
      content: EdgeBounds { left: 0, top: 0, right: sw - 1, bottom: sh - 1 },
      margins: EdgeMargins { top: 0, left: 0, right: 0, bottom: 0 },
      uniform_margin: 0,
      crop: CropRect { x: 0, y: 0, w: iw, h: ih },
      skipped_reason: Some(reason.to_string()),
    };
  }

  let data = analysis_img.as_raw();
  let is_content_at = |x: u32, y: u32| -> bool {
    let idx = ((y * sw + x) as usize) * 4;
    if data[idx + 3] <= alpha_min {
      return false;
    }
    color_dist(data[idx], data[idx + 1], data[idx + 2], bg.r, bg.g, bg.b) > threshold
  };

  let min_row_content = min_content_pixels.max((f64::from(sw) * content_density).round() as usize);
  let min_col_content = min_content_pixels.max((f64::from(sh) * content_density).round() as usize);

  let row_has_content = |y: u32| -> bool {
    let mut count = 0usize;
    for x in 0..sw {
      if is_content_at(x, y) {
        count += 1;
        if count >= min_row_content {
          return true;
        }
      }
    }
    false
  };

  let col_has_content = |x: u32| -> bool {
    let mut count = 0usize;
    for y in 0..sh {
      if is_content_at(x, y) {
        count += 1;
        if count >= min_col_content {
          return true;
        }
      }
    }
    false
  };

  let mut top = 0u32;
  while top < sh && !row_has_content(top) {
    top += 1;
  }
  let mut bottom: i64 = (sh as i64) - 1;
  while bottom >= 0 && !row_has_content(bottom as u32) {
    bottom -= 1;
  }
  let mut left = 0u32;
  while left < sw && !col_has_content(left) {
    left += 1;
  }
  let mut right: i64 = (sw as i64) - 1;
  while right >= 0 && !col_has_content(right as u32) {
    right -= 1;
  }

  let has_content = (left as i64) <= right && (top as i64) <= bottom;
  if !has_content {
    return BalanceDetectionResult {
      has_content: false,
      bg,
      threshold,
      alpha_min,
      ratio,
      sw,
      sh,
      content: EdgeBounds { left: 0, top: 0, right: sw - 1, bottom: sh - 1 },
      margins: EdgeMargins { top: 0, left: 0, right: 0, bottom: 0 },
      uniform_margin: 0,
      crop: CropRect { x: 0, y: 0, w: iw, h: ih },
      skipped_reason: Some("no-content".to_string()),
    };
  }

  let right_u = right as u32;
  let bottom_u = bottom as u32;
  let margins = EdgeMargins {
    top,
    left,
    right: (sw - 1) - right_u,
    bottom: (sh - 1) - bottom_u,
  };

  let min_dim = sw.min(sh);
  let min_margin_threshold = ((f64::from(min_dim) * opts.min_margin_fraction).round() as u32).max(3);
  let min_margin = margins.top.min(margins.left).min(margins.right).min(margins.bottom);

  if min_margin < min_margin_threshold {
    return BalanceDetectionResult {
      has_content: true,
      bg,
      threshold,
      alpha_min,
      ratio,
      sw,
      sh,
      content: EdgeBounds { left, top, right: right_u, bottom: bottom_u },
      margins,
      uniform_margin: 0,
      crop: CropRect { x: 0, y: 0, w: iw, h: ih },
      skipped_reason: Some("edge-content".to_string()),
    };
  }

  let crop_left = left.saturating_sub(min_margin);
  let crop_top = top.saturating_sub(min_margin);
  let crop_right = (right_u + min_margin).min(sw - 1);
  let crop_bottom = (bottom_u + min_margin).min(sh - 1);

  let x = (f64::from(crop_left) / ratio).floor().max(0.0) as u32;
  let y = (f64::from(crop_top) / ratio).floor().max(0.0) as u32;
  let x2 = ((f64::from(crop_right + 1) / ratio).ceil() as u32).min(iw);
  let y2 = ((f64::from(crop_bottom + 1) / ratio).ceil() as u32).min(ih);
  let w = x2.saturating_sub(x).max(1);
  let h = y2.saturating_sub(y).max(1);

  BalanceDetectionResult {
    has_content: true,
    bg,
    threshold,
    alpha_min,
    ratio,
    sw,
    sh,
    content: EdgeBounds { left, top, right: right_u, bottom: bottom_u },
    margins,
    uniform_margin: min_margin,
    crop: CropRect { x, y, w, h },
    skipped_reason: None,
  }
}

/// Crop an `RgbaImage` to balanced uniform margins around its detected content.
/// Port of `utils/image/imageBalance.js::computeBalancedCanvas`.
pub fn balance_rgba_image(img: &RgbaImage, opts: &BalanceOptions) -> (RgbaImage, BalanceDimensions) {
  let (iw, ih) = img.dimensions();
  if iw == 0 || ih == 0 {
    return (
      img.clone(),
      BalanceDimensions {
        width: iw,
        height: ih,
        x: 0,
        y: 0,
        original_width: iw,
        original_height: ih,
      },
    );
  }

  let detection = detect_balanced_crop_rect(img, opts);
  let c = detection.crop;
  let cropped = image::imageops::crop_imm(img, c.x, c.y, c.w, c.h).to_image();

  (
    cropped,
    BalanceDimensions {
      width: c.w,
      height: c.h,
      x: c.x,
      y: c.y,
      original_width: iw,
      original_height: ih,
    },
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use image::Rgba;

  #[test]
  fn balances_asymmetric_whitespace_to_uniform_margin() {
    let mut img = RgbaImage::from_pixel(400, 300, Rgba([250, 250, 250, 255]));
    // Content block positioned closer to left/top (margin 30px) than right/bottom (170px, 150px)
    for y in 30..150 {
      for x in 30..230 {
        img.put_pixel(x, y, Rgba([20, 40, 200, 255]));
      }
    }

    let (balanced, dims) = balance_rgba_image(&img, &BalanceOptions::default());
    assert_eq!(dims.x, 0);
    assert_eq!(dims.y, 0);
    // Right and bottom margins should be trimmed to match the 30px top/left margin -> 260x180
    assert_eq!(balanced.dimensions(), (260, 180));
  }
}
