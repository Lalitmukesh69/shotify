use std::collections::HashMap;

use image::{imageops::FilterType, DynamicImage, GenericImageView, RgbaImage};
use serde::{Deserialize, Serialize};

use crate::color::{hex_to_rgb, rgb_to_hex};
use crate::math::clamp_f64;

/// Result of extracting dominant and accent colors from a screenshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedPalette {
  pub colors: Vec<String>,
  pub weights: Vec<f32>,
  pub is_monochrome: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct Bucket {
  n: f64,
  r: f64,
  g: f64,
  b: f64,
  sat: f64,
}

#[derive(Debug, Clone, Copy)]
struct ScoredBucket {
  score: f64,
  sat_score: f64,
  rgb: (f64, f64, f64),
}

#[inline]
fn dist2(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
  let dr = a.0 - b.0;
  let dg = a.1 - b.1;
  let db = a.2 - b.2;
  dr * dr + dg * dg + db * db
}

fn pick_distinct(rgbs: &[(f64, f64, f64)], count: usize, min_dist_sq: f64) -> Vec<(f64, f64, f64)> {
  let mut out: Vec<(f64, f64, f64)> = Vec::new();
  for &c in rgbs {
    if out.len() >= count {
      break;
    }
    if out.is_empty() || out.iter().all(|&o| dist2(c, o) >= min_dist_sq) {
      out.push(c);
    }
  }
  out
}

/// Extract a representative color palette from raw RGBA image pixels.
/// Port of `utils/color/colorPalette.js::extractPaletteFromDataUrl`.
pub fn extract_palette_from_rgba_image(img: &RgbaImage, color_count: usize) -> Option<ExtractedPalette> {
  let (iw, ih) = img.dimensions();
  if iw == 0 || ih == 0 {
    return None;
  }

  let target_count = color_count.max(1);
  let max_dim = 220.0_f64;
  let scale = (max_dim / f64::from(iw.max(ih))).min(1.0);
  let cw = ((f64::from(iw) * scale).round() as u32).max(1);
  let ch = ((f64::from(ih) * scale).round() as u32).max(1);

  let downscaled = if cw == iw && ch == ih {
    img.clone()
  } else {
    DynamicImage::ImageRgba8(img.clone())
      .resize_exact(cw, ch, FilterType::Triangle)
      .to_rgba8()
  };

  let (w, h) = downscaled.dimensions();
  let raw = downscaled.as_raw();

  let regions = [
    // Center-heavy region (reduces window-chrome border bias)
    (
      (f64::from(w) * 0.10).floor() as u32,
      (f64::from(h) * 0.10).floor() as u32,
      ((f64::from(w) * 0.80).floor() as u32).max(1),
      ((f64::from(h) * 0.80).floor() as u32).max(1),
      1.0_f64,
    ),
    // Full image (lower weight so border accents still count)
    (0, 0, w, h, 0.45_f64),
  ];

  let mut buckets: HashMap<u16, Bucket> = HashMap::new();

  for &(sx, sy, sw, sh, weight) in &regions {
    let pixel_count = (sw as usize) * (sh as usize);
    let step = (pixel_count / 12_000).max(1);
    let mut visited = 0usize;

    let end_y = (sy + sh).min(h);
    let end_x = (sx + sw).min(w);

    for y in sy..end_y {
      for x in sx..end_x {
        if visited % step != 0 {
          visited += 1;
          continue;
        }
        visited += 1;

        let idx = ((y * w + x) as usize) * 4;
        let a = raw[idx + 3];
        if a < 128 {
          continue;
        }
        let r = f64::from(raw[idx]);
        let g = f64::from(raw[idx + 1]);
        let b = f64::from(raw[idx + 2]);

        let key = (((raw[idx] >> 4) as u16) << 8)
          | (((raw[idx + 1] >> 4) as u16) << 4)
          | ((raw[idx + 2] >> 4) as u16);

        let maxc = r.max(g).max(b);
        let minc = r.min(g).min(b);
        let sat = maxc - minc;

        let entry = buckets.entry(key).or_default();
        entry.n += weight;
        entry.r += r * weight;
        entry.g += g * weight;
        entry.b += b * weight;
        entry.sat += sat * weight;
      }
    }
  }

  let mut scored: Vec<ScoredBucket> = Vec::new();
  let mut total_n = 0.0_f64;
  let mut total_r = 0.0_f64;
  let mut total_g = 0.0_f64;
  let mut total_b = 0.0_f64;
  let mut total_sat = 0.0_f64;
  let mut luma_min = 255.0_f64;
  let mut luma_max = 0.0_f64;

  for v in buckets.values() {
    if v.n <= 0.0 {
      continue;
    }
    let avg = (v.r / v.n, v.g / v.n, v.b / v.n);
    let sat = v.sat / v.n;
    let luma = 0.2126 * avg.0 + 0.7152 * avg.1 + 0.0722 * avg.2;

    total_n += v.n;
    total_r += avg.0 * v.n;
    total_g += avg.1 * v.n;
    total_b += avg.2 * v.n;
    total_sat += sat * v.n;
    luma_min = luma_min.min(luma);
    luma_max = luma_max.max(luma);

    if sat < 10.0 && !(10.0..=245.0).contains(&luma) {
      continue;
    }

    let score = v.n.powf(0.72) * (1.0 + (sat / 36.0).min(2.6));
    let sat_score = sat * v.n.sqrt();
    scored.push(ScoredBucket { score, sat_score, rgb: avg });
  }

  let avg_rgb = if total_n > 0.0 {
    (total_r / total_n, total_g / total_n, total_b / total_n)
  } else {
    (0.0, 0.0, 0.0)
  };
  let avg_sat = if total_n > 0.0 { total_sat / total_n } else { 0.0 };
  let luma_range = if total_n > 0.0 { luma_max - luma_min } else { 0.0 };
  let is_monochrome = total_n > 0.0 && (avg_sat < 14.0 || luma_range < 18.0);

  let mut by_freq = scored.clone();
  by_freq.sort_by(|a, b| b.score.total_cmp(&a.score));
  by_freq.truncate(70);

  let mut by_sat = scored;
  by_sat.sort_by(|a, b| b.sat_score.total_cmp(&a.sat_score));
  by_sat.truncate(70);

  let mut merged = Vec::new();
  let max_len = by_freq.len().max(by_sat.len());
  for i in 0..max_len {
    if let Some(f) = by_freq.get(i) {
      merged.push(f.rgb);
    }
    if merged.len() >= 120 {
      break;
    }
    if let Some(s) = by_sat.get(i) {
      merged.push(s.rgb);
    }
    if merged.len() >= 120 {
      break;
    }
  }

  let distinct = pick_distinct(&merged, target_count.max(3), 24.0 * 24.0);
  let fallback = ["#8B5CF6", "#EC4899", "#22C55E", "#06B6D4", "#F59E0B"];

  let build_monochrome_palette = || -> Vec<String> {
    let luma = 0.2126 * avg_rgb.0 + 0.7152 * avg_rgb.1 + 0.0722 * avg_rgb.2;
    let factors = if luma > 200.0 {
      [0.85, 0.7, 0.55, 0.4, 0.25]
    } else {
      [0.6, 0.8, 1.0, 1.2, 1.4]
    };
    factors
      .iter()
      .map(|&f| {
        rgb_to_hex(
          clamp_f64(avg_rgb.0 * f, 0.0, 255.0),
          clamp_f64(avg_rgb.1 * f, 0.0, 255.0),
          clamp_f64(avg_rgb.2 * f, 0.0, 255.0),
        )
      })
      .collect()
  };

  let mut out: Vec<String> = distinct
    .into_iter()
    .map(|c| rgb_to_hex(c.0, c.1, c.2))
    .collect();

  if out.is_empty() && total_n > 0.0 {
    out = build_monochrome_palette();
  }

  while out.len() < target_count {
    if is_monochrome && total_n > 0.0 {
      let mono = build_monochrome_palette();
      let pick = mono[out.len() % mono.len()].clone();
      out.push(pick);
    } else {
      out.push(fallback[out.len() % fallback.len()].to_string());
    }
  }

  let colors: Vec<String> = out
    .into_iter()
    .take(target_count)
    .map(|h| {
      if let Some((r, g, b)) = hex_to_rgb(&h) {
        rgb_to_hex(f64::from(r), f64::from(g), f64::from(b))
      } else {
        "#000000".to_string()
      }
    })
    .collect();

  let selected_rgbs: Vec<(f64, f64, f64)> = colors
    .iter()
    .filter_map(|h| hex_to_rgb(h).map(|(r, g, b)| (f64::from(r), f64::from(g), f64::from(b))))
    .collect();

  let mut weights = vec![0.0_f64; colors.len()];
  for v in buckets.values() {
    if v.n <= 0.0 || selected_rgbs.is_empty() {
      continue;
    }
    let avg = (v.r / v.n, v.g / v.n, v.b / v.n);
    let mut best_idx = 0usize;
    let mut best_dist = f64::INFINITY;
    for (i, &c) in selected_rgbs.iter().enumerate() {
      let d = dist2(avg, c);
      if d < best_dist {
        best_dist = d;
        best_idx = i;
      }
    }
    weights[best_idx] += v.n;
  }

  let total_w: f64 = weights.iter().sum::<f64>();
  let denom = if total_w > 0.0 { total_w } else { 1.0 };
  let norm_weights: Vec<f32> = weights.into_iter().map(|w| (w / denom) as f32).collect();

  Some(ExtractedPalette {
    colors,
    weights: norm_weights,
    is_monochrome,
  })
}

/// Extract palette directly from encoded PNG/JPEG/WebP bytes.
pub fn extract_palette_from_bytes(bytes: &[u8], color_count: usize) -> crate::Result<Option<ExtractedPalette>> {
  let img = image::load_from_memory(bytes)?.to_rgba8();
  let _ = img.dimensions();
  let _ = DynamicImage::ImageRgba8(img.clone()).dimensions();
  Ok(extract_palette_from_rgba_image(&img, color_count))
}

#[cfg(test)]
mod tests {
  use super::*;
  use image::Rgba;

  #[test]
  fn extracts_vivid_colors_from_split_image() {
    let mut img = RgbaImage::new(100, 100);
    for y in 0..100 {
      for x in 0..100 {
        let px = if x < 50 {
          Rgba([230, 50, 90, 255])
        } else {
          Rgba([30, 140, 240, 255])
        };
        img.put_pixel(x, y, px);
      }
    }

    let pal = extract_palette_from_rgba_image(&img, 5).expect("palette should extract");
    assert_eq!(pal.colors.len(), 5);
    assert!(!pal.is_monochrome);
    let weight_sum: f32 = pal.weights.iter().sum();
    assert!((weight_sum - 1.0).abs() < 1e-3);
  }

  #[test]
  fn detects_monochrome_image() {
    let img = RgbaImage::from_pixel(64, 64, Rgba([120, 120, 120, 255]));
    let pal = extract_palette_from_rgba_image(&img, 5).expect("mono palette");
    assert!(pal.is_monochrome);
    assert_eq!(pal.colors.len(), 5);
  }
}
