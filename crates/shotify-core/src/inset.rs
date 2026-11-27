use image::{imageops::FilterType, DynamicImage, Rgba as ImgRgba, RgbaImage};
use serde::{Deserialize, Serialize};

use crate::color::{parse_css_color, rgb_to_hex, Rgba};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgePalette {
  pub top: Vec<String>,
  pub right: Vec<String>,
  pub bottom: Vec<String>,
  pub left: Vec<String>,
}

impl Default for EdgePalette {
  fn default() -> Self {
    Self {
      top: vec!["#e0e0e0".to_string()],
      right: vec!["#e0e0e0".to_string()],
      bottom: vec!["#e0e0e0".to_string()],
      left: vec!["#e0e0e0".to_string()],
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgePaletteGradients {
  pub top: String,
  pub right: String,
  pub bottom: String,
  pub left: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeColorOptions {
  pub edge_size: u32,
  pub include_alpha: bool,
  pub min_alpha: u8,
}

impl Default for EdgeColorOptions {
  fn default() -> Self {
    Self {
      edge_size: 20,
      include_alpha: true,
      min_alpha: 16,
    }
  }
}

/// Detect the alpha-weighted dominant edge color of an image.
/// Port of `utils/image/imageInset.js::getEdgeColor`.
pub fn get_edge_color(img: &RgbaImage, opts: EdgeColorOptions) -> Rgba {
  let (iw, ih) = img.dimensions();
  if iw == 0 || ih == 0 {
    return Rgba::new(0xE0, 0xE0, 0xE0, 1.0);
  }

  let scale = (1024.0_f64 / f64::from(iw.max(ih))).min(1.0);
  let w = ((f64::from(iw) * scale).round() as u32).max(1);
  let h = ((f64::from(ih) * scale).round() as u32).max(1);
  let work = if w == iw && h == ih {
    img.clone()
  } else {
    DynamicImage::ImageRgba8(img.clone())
      .resize_exact(w, h, FilterType::Triangle)
      .to_rgba8()
  };

  let e = ((f64::from(opts.edge_size.max(1)) * scale).round() as u32).clamp(1, w.min(h));
  let mut sum_r = 0.0_f64;
  let mut sum_g = 0.0_f64;
  let mut sum_b = 0.0_f64;
  let mut sum_a = 0.0_f64;
  let mut weight = 0.0_f64;

  let mut sample_rect = |x0: u32, y0: u32, rw: u32, rh: u32| {
    let x1 = (x0 + rw).min(w);
    let y1 = (y0 + rh).min(h);
    for y in y0..y1 {
      for x in x0..x1 {
        let px = work.get_pixel(x, y);
        let alpha = px[3];
        if alpha < opts.min_alpha {
          continue;
        }
        let wgt = f64::from(alpha) / 255.0;
        sum_r += f64::from(px[0]) * wgt;
        sum_g += f64::from(px[1]) * wgt;
        sum_b += f64::from(px[2]) * wgt;
        sum_a += f64::from(alpha);
        weight += wgt;
      }
    }
  };

  sample_rect(0, 0, w, e);
  sample_rect(0, h - e, w, e);
  sample_rect(0, 0, e, h);
  sample_rect(w - e, 0, e, h);

  if weight <= 0.0 {
    return Rgba::new(0xE0, 0xE0, 0xE0, 1.0);
  }

  let r = (sum_r / weight).round().clamp(0.0, 255.0) as u8;
  let g = (sum_g / weight).round().clamp(0.0, 255.0) as u8;
  let b = (sum_b / weight).round().clamp(0.0, 255.0) as u8;
  let a = if opts.include_alpha {
    (((sum_a / (weight * 255.0)) * 100.0).round() / 100.0).clamp(0.0, 1.0) as f32
  } else {
    1.0
  };
  Rgba::new(r, g, b, a)
}

/// Sample multiple hex colors along top/right/bottom/left borders for Ambient glow gradients.
/// Port of `utils/image/imageInset.js::getEdgePalette`.
pub fn get_edge_palette(
  img: &RgbaImage,
  samples: usize,
  edge_size: u32,
  min_alpha: u8,
) -> EdgePalette {
  let (iw, ih) = img.dimensions();
  if iw == 0 || ih == 0 {
    return EdgePalette::default();
  }

  let sample_count = samples.max(1);
  let scale = (1024.0_f64 / f64::from(iw.max(ih))).min(1.0);
  let w = ((f64::from(iw) * scale).round() as u32).max(1);
  let h = ((f64::from(ih) * scale).round() as u32).max(1);

  let work = if w == iw && h == ih {
    img.clone()
  } else {
    DynamicImage::ImageRgba8(img.clone())
      .resize_exact(w, h, FilterType::Triangle)
      .to_rgba8()
  };

  let e = ((f64::from(edge_size.max(1)) * scale).round() as u32).clamp(1, w.min(h));

  let sample_region_hex = |x0: u32, y0: u32, rw: u32, rh: u32| -> Option<String> {
    let start_x = x0.min(w - 1);
    let start_y = y0.min(h - 1);
    let end_x = (x0 + rw).min(w);
    let end_y = (y0 + rh).min(h);
    let mut sum_r = 0.0_f64;
    let mut sum_g = 0.0_f64;
    let mut sum_b = 0.0_f64;
    let mut weight = 0.0_f64;

    for yy in start_y..end_y {
      for xx in start_x..end_x {
        let px = work.get_pixel(xx, yy);
        let a = px[3];
        if a >= min_alpha {
          let wgt = f64::from(a) / 255.0;
          sum_r += f64::from(px[0]) * wgt;
          sum_g += f64::from(px[1]) * wgt;
          sum_b += f64::from(px[2]) * wgt;
          weight += wgt;
        }
      }
    }

    if weight <= 0.0 {
      None
    } else {
      Some(rgb_to_hex(sum_r / weight, sum_g / weight, sum_b / weight))
    }
  };

  let sample_stops = |n: usize, length: u32| -> Vec<u32> {
    if n <= 1 {
      return vec![length / 2];
    }
    (0..n)
      .map(|i| {
        (((i as f64) * f64::from(length - 1)) / ((n - 1) as f64)).round() as u32
      })
      .collect()
  };

  let top_xs = sample_stops(sample_count, w);
  let left_ys = sample_stops(sample_count, h);

  let mut top = Vec::with_capacity(sample_count);
  let mut bottom = Vec::with_capacity(sample_count);
  let mut left = Vec::with_capacity(sample_count);
  let mut right = Vec::with_capacity(sample_count);

  for i in 0..sample_count {
    let x = top_xs[i];
    let y = left_ys[i];
    let top_x0 = x.saturating_sub(e / 2).min(w.saturating_sub(e));
    let left_y0 = y.saturating_sub(e / 2).min(h.saturating_sub(e));

    let t = sample_region_hex(top_x0, 0, e, e);
    let btm = sample_region_hex(top_x0, h.saturating_sub(e), e, e);
    let l = sample_region_hex(0, left_y0, e, e);
    let rgt = sample_region_hex(w.saturating_sub(e), left_y0, e, e);

    top.push(t.unwrap_or_else(|| top.last().cloned().unwrap_or_else(|| "#e0e0e0".to_string())));
    bottom.push(btm.unwrap_or_else(|| bottom.last().cloned().unwrap_or_else(|| "#e0e0e0".to_string())));
    left.push(l.unwrap_or_else(|| left.last().cloned().unwrap_or_else(|| "#e0e0e0".to_string())));
    right.push(rgt.unwrap_or_else(|| right.last().cloned().unwrap_or_else(|| "#e0e0e0".to_string())));
  }

  EdgePalette { top, right, bottom, left }
}

/// Format an `EdgePalette` into CSS `linear-gradient(...)` strings.
/// Port of `utils/image/imageInset.js::edgePaletteToGradients`.
pub fn edge_palette_to_gradients(palette: &EdgePalette) -> EdgePaletteGradients {
  let stops = |colors: &[String]| -> String {
    let fallback = vec!["#e0e0e0".to_string()];
    let list = if colors.is_empty() { &fallback } else { colors };
    let denom = (list.len().saturating_sub(1)).max(1) as f64;
    list
      .iter()
      .enumerate()
      .map(|(i, c)| format!("{c} {}%", (i as f64 / denom) * 100.0))
      .collect::<Vec<_>>()
      .join(", ")
  };

  EdgePaletteGradients {
    top: format!("linear-gradient(90deg, {})", stops(&palette.top)),
    right: format!("linear-gradient(180deg, {})", stops(&palette.right)),
    bottom: format!("linear-gradient(90deg, {})", stops(&palette.bottom)),
    left: format!("linear-gradient(180deg, {})", stops(&palette.left)),
  }
}

/// Create an inset-padded image filled with either custom color or detected edge color.
pub fn create_inset_image(
  img: &RgbaImage,
  padding: u32,
  custom_color: Option<&str>,
) -> RgbaImage {
  let (iw, ih) = img.dimensions();
  let pad = padding;
  let out_w = iw + pad * 2;
  let out_h = ih + pad * 2;

  let fill = custom_color
    .and_then(parse_css_color)
    .unwrap_or_else(|| {
      get_edge_color(
        img,
        EdgeColorOptions {
          edge_size: 20,
          include_alpha: false,
          min_alpha: 16,
        },
      )
    });

  let fill_px = ImgRgba([
    fill.r,
    fill.g,
    fill.b,
    (fill.a.clamp(0.0, 1.0) * 255.0).round() as u8,
  ]);

  let mut out = RgbaImage::from_pixel(out_w, out_h, fill_px);
  image::imageops::overlay(&mut out, img, i64::from(pad), i64::from(pad));
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn edge_color_ignores_transparent_border() {
    let mut img = RgbaImage::from_pixel(60, 60, ImgRgba([0, 0, 0, 0]));
    for y in 10..50 {
      for x in 10..50 {
        img.put_pixel(x, y, ImgRgba([30, 120, 210, 255]));
      }
    }
    let c = get_edge_color(&img, EdgeColorOptions { edge_size: 20, include_alpha: false, min_alpha: 16 });
    assert_eq!((c.r, c.g, c.b), (30, 120, 210));
  }

  #[test]
  fn edge_palette_samples_distinct_sides() {
    let mut img = RgbaImage::from_pixel(64, 64, ImgRgba([255, 255, 255, 255]));
    for x in 0..64 {
      for y in 0..8 {
        img.put_pixel(x, y, ImgRgba([255, 0, 0, 255]));
      }
    }
    let pal = get_edge_palette(&img, 4, 4, 16);
    assert_eq!(pal.top.len(), 4);
    assert_eq!(pal.top[0], "#FF0000");
    let css = edge_palette_to_gradients(&pal);
    assert!(css.top.starts_with("linear-gradient(90deg, #FF0000"));
  }
}
