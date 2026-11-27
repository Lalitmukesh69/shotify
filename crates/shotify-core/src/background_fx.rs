use image::{imageops::FilterType, DynamicImage, Rgba as ImgRgba, RgbaImage};
use serde::{Deserialize, Serialize};
use tiny_skia::{
  BlendMode, GradientStop, LinearGradient, Paint, PathBuilder, Pixmap, Point, RadialGradient, Rect,
  SpreadMode, Transform,
};

use crate::color::{parse_css_color, Mulberry32, Rgba};
use crate::defaults::BlobGradientConfig;
use crate::error::{CoreError, Result};
use crate::helpers::CanvasBlendMode;
use crate::layout::compute_object_cover_rect;
use crate::math::{clamp_f32, clamp_f64, parse_angle_to_unit_vector};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RenderQuality {
  Interactive,
  Final,
}

impl Default for RenderQuality {
  fn default() -> Self {
    Self::Final
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BackgroundSpec {
  Transparent,
  Solid {
    color: String,
  },
  Gradient {
    #[serde(rename = "angleDeg", default = "default_angle_135")]
    angle_deg: f64,
    #[serde(rename = "colorStart")]
    color_start: String,
    #[serde(rename = "colorEnd")]
    color_end: String,
  },
  BlobGradient {
    config: BlobGradientConfig,
  },
  Wallpaper,
}

fn default_angle_135() -> f64 {
  135.0
}

/// Convert a straight-alpha `image::RgbaImage` into a premultiplied `tiny_skia::Pixmap`.
pub fn rgba_image_to_pixmap(img: &RgbaImage) -> Result<Pixmap> {
  let (w, h) = img.dimensions();
  if w == 0 || h == 0 {
    return Err(CoreError::InvalidDimensions { width: w, height: h });
  }
  let mut pixmap = Pixmap::new(w, h).ok_or(CoreError::PixmapAllocation { width: w, height: h })?;
  let dst = pixmap.data_mut();
  let src = img.as_raw();
  for (s, d) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
    let pm = tiny_skia::ColorU8::from_rgba(s[0], s[1], s[2], s[3]).premultiply();
    d[0] = pm.red();
    d[1] = pm.green();
    d[2] = pm.blue();
    d[3] = pm.alpha();
  }
  Ok(pixmap)
}

/// Convert a premultiplied `tiny_skia::Pixmap` back into straight-alpha `image::RgbaImage`.
pub fn pixmap_to_rgba_image(pixmap: &Pixmap) -> RgbaImage {
  let w = pixmap.width();
  let h = pixmap.height();
  let mut out = RgbaImage::new(w, h);
  for (p, dst) in pixmap.pixels().iter().zip(out.as_mut().chunks_exact_mut(4)) {
    let demul = p.demultiply();
    dst[0] = demul.red();
    dst[1] = demul.green();
    dst[2] = demul.blue();
    dst[3] = demul.alpha();
  }
  out
}

/// Peter Kovesi box sizes for n-pass Gaussian blur approximation.
/// Port of `utils/image/canvasBackgroundFx.js::boxesForGauss`.
pub fn boxes_for_gauss(sigma: f64, n: usize) -> Vec<u32> {
  let s = if sigma.is_finite() { sigma.max(0.0) } else { 0.0 };
  if s <= 0.0 || n == 0 {
    return vec![1; n];
  }
  let n_f = n as f64;
  let w_ideal = ((12.0 * s * s / n_f) + 1.0).sqrt();
  let mut wl = w_ideal.floor() as i64;
  if wl % 2 == 0 {
    wl -= 1;
  }
  let wl = wl.max(1) as f64;
  let wu = wl + 2.0;

  let m_ideal = (12.0 * s * s - n_f * wl * wl - 4.0 * n_f * wl - 3.0 * n_f) / (-4.0 * wl - 4.0);
  let m = m_ideal.round() as i64;

  (0..n)
    .map(|i| if (i as i64) < m { wl as u32 } else { wu as u32 })
    .collect()
}

fn box_blur_h(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
  if w == 0 || h == 0 || r == 0 {
    dst.copy_from_slice(src);
    return;
  }
  let iarr = 1.0_f64 / ((r + r + 1) as f64);

  for y in 0..h {
    let row_start = y * w * 4;
    let fv_r = f64::from(src[row_start]);
    let fv_g = f64::from(src[row_start + 1]);
    let fv_b = f64::from(src[row_start + 2]);
    let fv_a = f64::from(src[row_start + 3]);

    let lv_idx = row_start + (w - 1) * 4;
    let lv_r = f64::from(src[lv_idx]);
    let lv_g = f64::from(src[lv_idx + 1]);
    let lv_b = f64::from(src[lv_idx + 2]);
    let lv_a = f64::from(src[lv_idx + 3]);

    let mut val_r = ((r + 1) as f64) * fv_r;
    let mut val_g = ((r + 1) as f64) * fv_g;
    let mut val_b = ((r + 1) as f64) * fv_b;
    let mut val_a = ((r + 1) as f64) * fv_a;

    for j in 0..r {
      let idx = row_start + j.min(w - 1) * 4;
      val_r += f64::from(src[idx]);
      val_g += f64::from(src[idx + 1]);
      val_b += f64::from(src[idx + 2]);
      val_a += f64::from(src[idx + 3]);
    }

    for x in 0..w {
      let (add_r, add_g, add_b, add_a) = if x + r < w {
        let idx = row_start + (x + r) * 4;
        (f64::from(src[idx]), f64::from(src[idx + 1]), f64::from(src[idx + 2]), f64::from(src[idx + 3]))
      } else {
        (lv_r, lv_g, lv_b, lv_a)
      };

      let (sub_r, sub_g, sub_b, sub_a) = if x >= r {
        let idx = row_start + (x - r) * 4;
        (f64::from(src[idx]), f64::from(src[idx + 1]), f64::from(src[idx + 2]), f64::from(src[idx + 3]))
      } else {
        (fv_r, fv_g, fv_b, fv_a)
      };

      val_r += add_r - sub_r;
      val_g += add_g - sub_g;
      val_b += add_b - sub_b;
      val_a += add_a - sub_a;

      let out_idx = row_start + x * 4;
      dst[out_idx] = (val_r * iarr).round().clamp(0.0, 255.0) as u8;
      dst[out_idx + 1] = (val_g * iarr).round().clamp(0.0, 255.0) as u8;
      dst[out_idx + 2] = (val_b * iarr).round().clamp(0.0, 255.0) as u8;
      dst[out_idx + 3] = (val_a * iarr).round().clamp(0.0, 255.0) as u8;
    }
  }
}

fn box_blur_v(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
  if w == 0 || h == 0 || r == 0 {
    dst.copy_from_slice(src);
    return;
  }
  let iarr = 1.0_f64 / ((r + r + 1) as f64);

  for x in 0..w {
    let col_start = x * 4;
    let fv_r = f64::from(src[col_start]);
    let fv_g = f64::from(src[col_start + 1]);
    let fv_b = f64::from(src[col_start + 2]);
    let fv_a = f64::from(src[col_start + 3]);

    let lv_idx = ((h - 1) * w + x) * 4;
    let lv_r = f64::from(src[lv_idx]);
    let lv_g = f64::from(src[lv_idx + 1]);
    let lv_b = f64::from(src[lv_idx + 2]);
    let lv_a = f64::from(src[lv_idx + 3]);

    let mut val_r = ((r + 1) as f64) * fv_r;
    let mut val_g = ((r + 1) as f64) * fv_g;
    let mut val_b = ((r + 1) as f64) * fv_b;
    let mut val_a = ((r + 1) as f64) * fv_a;

    for j in 0..r {
      let idx = (j.min(h - 1) * w + x) * 4;
      val_r += f64::from(src[idx]);
      val_g += f64::from(src[idx + 1]);
      val_b += f64::from(src[idx + 2]);
      val_a += f64::from(src[idx + 3]);
    }

    for y in 0..h {
      let (add_r, add_g, add_b, add_a) = if y + r < h {
        let idx = ((y + r) * w + x) * 4;
        (f64::from(src[idx]), f64::from(src[idx + 1]), f64::from(src[idx + 2]), f64::from(src[idx + 3]))
      } else {
        (lv_r, lv_g, lv_b, lv_a)
      };

      let (sub_r, sub_g, sub_b, sub_a) = if y >= r {
        let idx = ((y - r) * w + x) * 4;
        (f64::from(src[idx]), f64::from(src[idx + 1]), f64::from(src[idx + 2]), f64::from(src[idx + 3]))
      } else {
        (fv_r, fv_g, fv_b, fv_a)
      };

      val_r += add_r - sub_r;
      val_g += add_g - sub_g;
      val_b += add_b - sub_b;
      val_a += add_a - sub_a;

      let out_idx = (y * w + x) * 4;
      dst[out_idx] = (val_r * iarr).round().clamp(0.0, 255.0) as u8;
      dst[out_idx + 1] = (val_g * iarr).round().clamp(0.0, 255.0) as u8;
      dst[out_idx + 2] = (val_b * iarr).round().clamp(0.0, 255.0) as u8;
      dst[out_idx + 3] = (val_a * iarr).round().clamp(0.0, 255.0) as u8;
    }
  }
}

/// Fast 3-pass sliding box Gaussian blur approximation in place over an RGBA buffer (w * h * 4 bytes).
pub fn gaussian_approx_blur_in_place(data: &mut [u8], w: u32, h: u32, sigma: f64) {
  let s = if sigma.is_finite() { sigma.max(0.0) } else { 0.0 };
  if s <= 0.0001 || w == 0 || h == 0 {
    return;
  }
  let expected = (w as usize) * (h as usize) * 4;
  if data.len() != expected {
    return;
  }

  let sizes = boxes_for_gauss(s, 3);
  let mut tmp = vec![0u8; expected];
  for size in sizes {
    let r = (size.saturating_sub(1) / 2) as usize;
    if r == 0 {
      continue;
    }
    box_blur_h(data, &mut tmp, w as usize, h as usize, r);
    box_blur_v(&tmp, data, w as usize, h as usize, r);
  }
}

/// Apply downscale -> Peter Kovesi Gaussian blur -> upscale onto an `RgbaImage`.
/// Port of `canvasBackgroundFx.js::drawWithGaussianBlur`.
pub fn apply_gaussian_blur_image(src: &RgbaImage, sigma_px: f64, quality: RenderQuality) -> RgbaImage {
  let sigma = if sigma_px.is_finite() { sigma_px.max(0.0) } else { 0.0 };
  let (dst_w, dst_h) = src.dimensions();
  if sigma <= 0.0001 || dst_w == 0 || dst_h == 0 {
    return src.clone();
  }

  let mut factor = clamp_f64(1.0 / (1.0 + sigma / 10.0), 0.12, 1.0);
  let mut bw = ((f64::from(dst_w) * factor).round() as u32).max(1);
  let mut bh = ((f64::from(dst_h) * factor).round() as u32).max(1);
  let max_dim = match quality {
    RenderQuality::Interactive => 650u32,
    RenderQuality::Final => 900u32,
  };
  if bw > max_dim || bh > max_dim {
    let s = (f64::from(max_dim) / f64::from(bw)).min(f64::from(max_dim) / f64::from(bh));
    bw = ((f64::from(bw) * s).round() as u32).max(1);
    bh = ((f64::from(bh) * s).round() as u32).max(1);
    factor = (factor * s).max(0.05);
  }
  let blur_sigma = sigma * factor;

  let mut work = if bw == dst_w && bh == dst_h {
    src.clone()
  } else {
    DynamicImage::ImageRgba8(src.clone())
      .resize_exact(bw, bh, FilterType::Triangle)
      .to_rgba8()
  };

  gaussian_approx_blur_in_place(work.as_mut(), bw, bh, blur_sigma);

  if bw == dst_w && bh == dst_h {
    work
  } else {
    DynamicImage::ImageRgba8(work)
      .resize_exact(dst_w, dst_h, FilterType::Triangle)
      .to_rgba8()
  }
}

/// Apply CSS-matched brightness pass on premultiplied `Pixmap` (`<1` multiply black, `>1` screen white).
/// Port of `canvasBackgroundFx.js` brightness pass.
pub fn apply_brightness_to_pixmap(pixmap: &mut Pixmap, brightness: f64) {
  if !brightness.is_finite() || (brightness - 1.0).abs() < 0.0001 {
    return;
  }
  let b = clamp_f64(brightness, 0.25, 3.0);
  let mut paint = Paint::default();
  if b < 1.0 {
    let a = clamp_f64(1.0 - b, 0.0, 1.0) as f32;
    paint.blend_mode = BlendMode::Multiply;
    paint.set_color(tiny_skia::Color::from_rgba(0.0, 0.0, 0.0, a).unwrap_or(tiny_skia::Color::BLACK));
  } else {
    let t = clamp_f64((b - 1.0) / 0.5, 0.0, 1.0);
    let a = clamp_f64(t * 0.4, 0.0, 0.4) as f32;
    paint.blend_mode = BlendMode::Screen;
    paint.set_color(tiny_skia::Color::from_rgba(1.0, 1.0, 1.0, a).unwrap_or(tiny_skia::Color::WHITE));
  }

  let rect = match Rect::from_xywh(0.0, 0.0, pixmap.width() as f32, pixmap.height() as f32) {
    Some(r) => r,
    None => return,
  };
  pixmap.fill_rect(rect, &paint, Transform::identity(), None);
}

/// Render a linear gradient onto a new `tiny_skia::Pixmap`.
pub fn render_linear_gradient_pixmap(
  w: u32,
  h: u32,
  angle_deg: f64,
  color_start: &str,
  color_end: &str,
) -> Result<Pixmap> {
  let mut pixmap = Pixmap::new(w.max(1), h.max(1))
    .ok_or(CoreError::PixmapAllocation { width: w, height: h })?;
  draw_linear_gradient_into(&mut pixmap, angle_deg, color_start, color_end);
  Ok(pixmap)
}

pub fn draw_linear_gradient_into(pixmap: &mut Pixmap, angle_deg: f64, color_start: &str, color_end: &str) {
  let w = pixmap.width() as f64;
  let h = pixmap.height() as f64;
  let (ux, uy) = parse_angle_to_unit_vector(angle_deg);
  let cx = w / 2.0;
  let cy = h / 2.0;
  let half = w.hypot(h) / 2.0;

  let start = Point::from_xy((cx - ux * half) as f32, (cy - uy * half) as f32);
  let end = Point::from_xy((cx + ux * half) as f32, (cy + uy * half) as f32);

  let c0 = parse_css_color(color_start).unwrap_or(Rgba::TRANSPARENT).to_skia_color();
  let c1 = parse_css_color(color_end).unwrap_or(Rgba::TRANSPARENT).to_skia_color();

  if let Some(shader) = LinearGradient::new(
    start,
    end,
    vec![GradientStop::new(0.0, c0), GradientStop::new(1.0, c1)],
    SpreadMode::Pad,
    Transform::identity(),
  ) {
    let mut paint = Paint::default();
    paint.shader = shader;
    if let Some(rect) = Rect::from_xywh(0.0, 0.0, w as f32, h as f32) {
      pixmap.fill_rect(rect, &paint, Transform::identity(), None);
    }
  }
}

/// Render a procedural multi-lobe blob gradient onto a `Pixmap`.
/// Port of `utils/image/canvasBackgroundFx.js` blobGradient renderer.
pub fn render_blob_gradient_pixmap(
  width_px: u32,
  height_px: u32,
  render_scale: f64,
  cfg: &BlobGradientConfig,
) -> Result<Pixmap> {
  let width = width_px.max(1);
  let height = height_px.max(1);
  let scale = clamp_f64(render_scale, 0.25, 4.0);
  let internal_scale = scale.min(1.25);

  let iw = ((f64::from(width) / scale * internal_scale).round() as u32).max(1);
  let ih = ((f64::from(height) / scale * internal_scale).round() as u32).max(1);

  let mut work = Pixmap::new(iw, ih).ok_or(CoreError::PixmapAllocation { width: iw, height: ih })?;

  let base_mode = if cfg.base_mode.is_empty() { "dark" } else { &cfg.base_mode };
  let is_light = base_mode.eq_ignore_ascii_case("light");
  let base_a = cfg.base_a.as_deref().unwrap_or(if is_light { "#ffffff" } else { "#05060A" });
  let base_b = cfg.base_b.as_deref().unwrap_or(if is_light { "#f3f4f6" } else { "#111827" });
  draw_linear_gradient_into(&mut work, cfg.angle_deg, base_a, base_b);

  let colors: Vec<&str> = cfg.colors.iter().map(String::as_str).filter(|s| !s.is_empty()).take(8).collect();
  let c2 = colors.first().copied().unwrap_or("#8b5cf6");
  let c3 = colors.get(1).copied().or_else(|| colors.first().copied()).unwrap_or("#ec4899");

  let mut rand = Mulberry32::from_nonzero_seed(cfg.seed);
  let blob_count = cfg.blob_count.clamp(2, 12);
  let intensity = clamp_f64(cfg.intensity, 0.0, 1.0);
  let corner_bias = clamp_f64(cfg.corner_bias.unwrap_or(0.42), 0.0, 1.0);
  let min_dim = f64::from(iw.min(ih));
  let base_r = min_dim * (0.44 + rand.next_f64() * 0.18);
  let blob_blur_px = clamp_f64(cfg.blob_blur_px.unwrap_or(min_dim * 0.055), 0.0, 140.0);

  let blend_mode = CanvasBlendMode::from_blob_config(cfg.blend.as_deref(), Some(base_mode));
  let mut blob_layer = Pixmap::new(iw, ih).ok_or(CoreError::PixmapAllocation { width: iw, height: ih })?;

  for i in 0..blob_count as usize {
    let use_corner = rand.next_f64() < corner_bias;
    let mut px = f64::from(iw) * (0.08 + rand.next_f64() * 0.84);
    let mut py = f64::from(ih) * (0.08 + rand.next_f64() * 0.84);
    if use_corner {
      let corner = (rand.next_f64() * 4.0).floor() as u8;
      let ox = if corner == 0 || corner == 2 { -0.10 } else { 1.10 };
      let oy = if corner == 0 || corner == 1 { -0.10 } else { 1.10 };
      px = f64::from(iw) * (ox + rand.next_f64() * 0.18);
      py = f64::from(ih) * (oy + rand.next_f64() * 0.18);
    }

    let r = base_r * (0.9 + rand.next_f64() * 1.15);
    let col = if !colors.is_empty() {
      colors[i % colors.len()]
    } else if i % 2 == 0 {
      c2
    } else {
      c3
    };
    let parsed_col = parse_css_color(col).unwrap_or(Rgba::WHITE);
    let alpha_base = if is_light {
      intensity * (0.42 + rand.next_f64() * 0.22)
    } else {
      intensity * (0.78 + rand.next_f64() * 0.22)
    };

    let lobes = 3 + (rand.next_f64() * 4.0).floor() as usize;
    for _ in 0..lobes {
      let ang = rand.next_f64() * std::f64::consts::TAU;
      let off = (0.10 + rand.next_f64() * 0.22) * r;
      let lx = px + ang.cos() * off;
      let ly = py + ang.sin() * off;
      let rot = rand.next_f64() * 360.0;
      let sx = 0.65 + rand.next_f64() * 0.85;
      let sy = 0.65 + rand.next_f64() * 0.85;
      let rr = (r * (0.55 + rand.next_f64() * 0.6)) as f32;

      let center_col = parsed_col.with_alpha_mul(alpha_base as f32).to_skia_color();
      let edge_col = parsed_col.with_alpha_mul(0.0).to_skia_color();

      let xform = Transform::from_scale(sx as f32, sy as f32)
        .post_rotate(rot as f32)
        .post_translate(lx as f32, ly as f32);

      if let Some(shader) = RadialGradient::new(
        Point::zero(),
        0.0,
        Point::zero(),
        rr.max(1.0),
        vec![GradientStop::new(0.0, center_col), GradientStop::new(1.0, edge_col)],
        SpreadMode::Pad,
        Transform::identity(),
      ) {
        let mut paint = Paint::default();
        paint.shader = shader;
        paint.blend_mode = BlendMode::SourceOver;
        if let Some(rect) = Rect::from_xywh(-rr, -rr, rr * 2.0, rr * 2.0) {
          blob_layer.fill_rect(rect, &paint, xform, None);
        }
      }
    }
  }

  // Blur the blob layer separately if blob_blur_px > 0, then composite over base wash
  if blob_blur_px > 0.5 {
    let mut straight = pixmap_to_rgba_image(&blob_layer);
    gaussian_approx_blur_in_place(straight.as_mut(), iw, ih, blob_blur_px * 0.6);
    blob_layer = rgba_image_to_pixmap(&straight)?;
  }

  let mut composite_paint = tiny_skia::PixmapPaint::default();
  composite_paint.blend_mode = blend_mode.to_skia_blend_mode();
  work.draw_pixmap(0, 0, blob_layer.as_ref(), &composite_paint, Transform::identity(), None);

  if iw == width && ih == height {
    return Ok(work);
  }

  let straight = pixmap_to_rgba_image(&work);
  let upscaled = DynamicImage::ImageRgba8(straight)
    .resize_exact(width, height, FilterType::Triangle)
    .to_rgba8();
  rgba_image_to_pixmap(&upscaled)
}

/// Apply rectangular vignette dark-edge overlay matching `ImageDisplay.jsx`.
pub fn apply_vignette_to_pixmap(pixmap: &mut Pixmap, vignette: f64) {
  let v = clamp_f64(vignette, 0.0, 1.0);
  if v <= 0.0001 {
    return;
  }
  let w = pixmap.width() as f32;
  let h = pixmap.height() as f32;
  let edge_alpha = (0.85 * v) as f32;
  let dark = tiny_skia::Color::from_rgba(0.0, 0.0, 0.0, edge_alpha).unwrap_or(tiny_skia::Color::BLACK);
  let clear = tiny_skia::Color::from_rgba(0.0, 0.0, 0.0, 0.0).unwrap_or(tiny_skia::Color::TRANSPARENT);
  let fade = 0.42_f32;

  let gradients = [
    (Point::from_xy(0.0, 0.0), Point::from_xy(w * fade, 0.0)),
    (Point::from_xy(w, 0.0), Point::from_xy(w * (1.0 - fade), 0.0)),
    (Point::from_xy(0.0, 0.0), Point::from_xy(0.0, h * fade)),
    (Point::from_xy(0.0, h), Point::from_xy(0.0, h * (1.0 - fade))),
  ];

  let rect = match Rect::from_xywh(0.0, 0.0, w, h) {
    Some(r) => r,
    None => return,
  };

  for (p0, p1) in gradients {
    if let Some(shader) = LinearGradient::new(
      p0,
      p1,
      vec![GradientStop::new(0.0, dark), GradientStop::new(1.0, clear)],
      SpreadMode::Pad,
      Transform::identity(),
    ) {
      let mut paint = Paint::default();
      paint.shader = shader;
      paint.blend_mode = BlendMode::SourceOver;
      pixmap.fill_rect(rect, &paint, Transform::identity(), None);
    }
  }
}

/// Apply deterministic film grain noise matching `opacity-[0.15]` noise overlay.
pub fn apply_noise_to_pixmap(pixmap: &mut Pixmap, seed: u32, opacity: f32) {
  let op = clamp_f32(opacity, 0.0, 1.0);
  if op <= 0.0001 {
    return;
  }
  let w = pixmap.width() as usize;
  let h = pixmap.height() as usize;
  let mut rand = Mulberry32::from_nonzero_seed(seed);
  let data = pixmap.data_mut();
  for idx in (0..(w * h * 4)).step_by(4) {
    let n = (rand.next_f64() - 0.5) * 2.0; // -1..1
    let delta = (n * 255.0 * f64::from(op)).round() as i16;
    if data[idx + 3] == 0 {
      continue;
    }
    data[idx] = (i16::from(data[idx]) + delta).clamp(0, i16::from(data[idx + 3])) as u8;
    data[idx + 1] = (i16::from(data[idx + 1]) + delta).clamp(0, i16::from(data[idx + 3])) as u8;
    data[idx + 2] = (i16::from(data[idx + 2]) + delta).clamp(0, i16::from(data[idx + 3])) as u8;
  }
}

/// Full background rasterizer combining base layer + Gaussian blur + brightness + vignette + noise.
/// Port of `utils/image/canvasBackgroundFx.js::renderBackgroundToCanvas`.
pub fn render_background_pixmap(
  width_px: u32,
  height_px: u32,
  render_scale: f64,
  spec: &BackgroundSpec,
  wallpaper: Option<&RgbaImage>,
  blur_px: f64,
  brightness: f64,
  vignette: f64,
  noise: bool,
  quality: RenderQuality,
) -> Result<Pixmap> {
  let w = width_px.max(1);
  let h = height_px.max(1);
  let scale = clamp_f64(render_scale, 0.25, 8.0);
  let canvas_w = ((f64::from(w) * scale).round() as u32).max(1);
  let canvas_h = ((f64::from(h) * scale).round() as u32).max(1);

  if matches!(spec, BackgroundSpec::Transparent) {
    return Pixmap::new(canvas_w, canvas_h)
      .ok_or(CoreError::PixmapAllocation { width: canvas_w, height: canvas_h });
  }

  let mut base = match spec {
    BackgroundSpec::Transparent => unreachable!(),
    BackgroundSpec::Solid { color } => {
      let mut pm = Pixmap::new(canvas_w, canvas_h)
        .ok_or(CoreError::PixmapAllocation { width: canvas_w, height: canvas_h })?;
      let col = parse_css_color(color).unwrap_or(Rgba::TRANSPARENT).to_skia_color();
      pm.fill(col);
      pm
    }
    BackgroundSpec::Gradient { angle_deg, color_start, color_end } => {
      render_linear_gradient_pixmap(canvas_w, canvas_h, *angle_deg, color_start, color_end)?
    }
    BackgroundSpec::BlobGradient { config } => {
      render_blob_gradient_pixmap(canvas_w, canvas_h, scale, config)?
    }
    BackgroundSpec::Wallpaper => {
      let mut pm = Pixmap::new(canvas_w, canvas_h)
        .ok_or(CoreError::PixmapAllocation { width: canvas_w, height: canvas_h })?;
      if let Some(wp) = wallpaper {
        let (sw, sh) = wp.dimensions();
        if sw > 0 && sh > 0 {
          let cover = compute_object_cover_rect(f64::from(sw), f64::from(sh), f64::from(canvas_w), f64::from(canvas_h));
          let wp_pm = rgba_image_to_pixmap(wp)?;
          let mut paint = tiny_skia::PixmapPaint::default();
          paint.quality = tiny_skia::FilterQuality::Bilinear;
          let sx = (cover.width / f64::from(sw)) as f32;
          let sy = (cover.height / f64::from(sh)) as f32;
          let xform = Transform::from_scale(sx, sy).post_translate(cover.left as f32, cover.top as f32);
          pm.draw_pixmap(0, 0, wp_pm.as_ref(), &paint, xform, None);
        }
      }
      pm
    }
  };

  let eff_blur = clamp_f64(blur_px, 0.0, 200.0) * scale;
  if eff_blur > 0.0001 {
    let straight = pixmap_to_rgba_image(&base);
    let blurred = apply_gaussian_blur_image(&straight, eff_blur, quality);
    base = rgba_image_to_pixmap(&blurred)?;
  }

  apply_brightness_to_pixmap(&mut base, brightness);
  apply_vignette_to_pixmap(&mut base, vignette);
  if noise {
    apply_noise_to_pixmap(&mut base, 1337, 0.15);
  }

  // Silence unused warning for PathBuilder if not yet needed in this file
  let _ = PathBuilder::new;
  let _ = ImgRgba([0u8; 4]);

  Ok(base)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn linear_gradient_pixmap_fills_canvas() {
    let pm = render_linear_gradient_pixmap(64, 64, 135.0, "#FF0000", "#0000FF").unwrap();
    // At 135 deg in canvas space (0=+X, 90=+Y), vector points (-cos45, +sin45), i.e., top-right -> bottom-left.
    let top_right = pm.pixel(63, 0).unwrap();
    let bot_left = pm.pixel(0, 63).unwrap();
    assert!(top_right.red() > top_right.blue());
    assert!(bot_left.blue() > bot_left.red());
  }

  #[test]
  fn blob_gradient_is_deterministic_across_runs() {
    let cfg = BlobGradientConfig::default();
    let a = render_blob_gradient_pixmap(64, 64, 1.0, &cfg).unwrap();
    let b = render_blob_gradient_pixmap(64, 64, 1.0, &cfg).unwrap();
    assert_eq!(a.data(), b.data());
  }

  #[test]
  fn gaussian_blur_handles_narrow_images_without_panic() {
    let img = RgbaImage::from_pixel(3, 5, ImgRgba([100, 150, 200, 255]));
    let blurred = apply_gaussian_blur_image(&img, 18.0, RenderQuality::Final);
    assert_eq!(blurred.dimensions(), (3, 5));
  }
}
