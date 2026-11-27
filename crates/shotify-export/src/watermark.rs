use image::RgbaImage;
use serde::{Deserialize, Serialize};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

use shotify_core::{
  apply_gaussian_blur_image, build_rounded_rect_path, clamp_f64, pixmap_to_rgba_image,
  rgba_image_to_pixmap, RectF, RenderQuality, WatermarkConfig,
};

use crate::error::{ExportError, Result};

/// Watermark placement mode matching `ImageDisplay.jsx`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WatermarkPosition {
  Inside,
  CenterTop,
  BottomLeft,
  BottomRight,
  TopLeft,
  TopRight,
  CenterBottom,
}

impl WatermarkPosition {
  pub fn parse(raw: &str) -> Self {
    match raw.trim().to_ascii_lowercase().as_str() {
      "center-top" => Self::CenterTop,
      "bottom-left" => Self::BottomLeft,
      "bottom-right" => Self::BottomRight,
      "top-left" => Self::TopLeft,
      "top-right" => Self::TopRight,
      "center-bottom" => Self::CenterBottom,
      _ => Self::Inside,
    }
  }
}

/// Alpha-blend a pre-rendered watermark `RgbaImage` directly onto `base` at `(x, y)`.
/// Port of `src-tauri/src/images.rs::exports_apply_watermark`.
pub fn composite_watermark_rgba(base: &mut RgbaImage, watermark: &RgbaImage, x: f32, y: f32) {
  let (base_w, base_h) = base.dimensions();
  let (wm_w, wm_h) = watermark.dimensions();

  let x0 = x.round().max(0.0) as i64;
  let y0 = y.round().max(0.0) as i64;

  for wy in 0..wm_h {
    let by = y0 + i64::from(wy);
    if by < 0 || by >= i64::from(base_h) {
      continue;
    }
    for wx in 0..wm_w {
      let bx = x0 + i64::from(wx);
      if bx < 0 || bx >= i64::from(base_w) {
        continue;
      }

      let wm_px = watermark.get_pixel(wx, wy);
      let wa = f32::from(wm_px[3]) / 255.0;
      if wa <= 0.0 {
        continue;
      }

      let base_px = base.get_pixel_mut(bx as u32, by as u32);
      let ba = f32::from(base_px[3]) / 255.0;
      let out_a = wa + (ba * (1.0 - wa));
      if out_a <= 0.0 {
        base_px.0 = [0, 0, 0, 0];
        continue;
      }

      let br = f32::from(base_px[0]) / 255.0;
      let bg = f32::from(base_px[1]) / 255.0;
      let bb = f32::from(base_px[2]) / 255.0;
      let wr = f32::from(wm_px[0]) / 255.0;
      let wg = f32::from(wm_px[1]) / 255.0;
      let wb = f32::from(wm_px[2]) / 255.0;

      let out_r = (wr * wa + br * ba * (1.0 - wa)) / out_a;
      let out_g = (wg * wa + bg * ba * (1.0 - wa)) / out_a;
      let out_b = (wb * wa + bb * ba * (1.0 - wa)) / out_a;

      base_px[0] = (out_r * 255.0).round().clamp(0.0, 255.0) as u8;
      base_px[1] = (out_g * 255.0).round().clamp(0.0, 255.0) as u8;
      base_px[2] = (out_b * 255.0).round().clamp(0.0, 255.0) as u8;
      base_px[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
    }
  }
}

/// Compact built-in 5x7 ASCII bitmap glyphs so watermark pills render cleanly
/// without external font file dependencies in headless exports.
fn glyph_5x7(ch: char) -> [u8; 7] {
  match ch {
    'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
    'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
    'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
    'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
    'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
    'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
    'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110],
    'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
    'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
    'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100],
    'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
    'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
    'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
    'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
    'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
    'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
    'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
    'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
    'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
    'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
    'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
    'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
    'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
    'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
    'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
    'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
    'a' => [0b00000, 0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111],
    'b' => [0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b11110],
    'c' => [0b00000, 0b00000, 0b01110, 0b10000, 0b10000, 0b10001, 0b01110],
    'd' => [0b00001, 0b00001, 0b01111, 0b10001, 0b10001, 0b10001, 0b01111],
    'e' => [0b00000, 0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110],
    'f' => [0b00110, 0b01001, 0b01000, 0b11100, 0b01000, 0b01000, 0b01000],
    'g' => [0b00000, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
    'h' => [0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b10001],
    'i' => [0b00100, 0b00000, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110],
    'j' => [0b00010, 0b00000, 0b00110, 0b00010, 0b00010, 0b10010, 0b01100],
    'k' => [0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010],
    'l' => [0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
    'm' => [0b00000, 0b00000, 0b11010, 0b10101, 0b10101, 0b10001, 0b10001],
    'n' => [0b00000, 0b00000, 0b11110, 0b10001, 0b10001, 0b10001, 0b10001],
    'o' => [0b00000, 0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110],
    'p' => [0b00000, 0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000],
    'q' => [0b00000, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001],
    'r' => [0b00000, 0b00000, 0b10110, 0b11001, 0b10000, 0b10000, 0b10000],
    's' => [0b00000, 0b00000, 0b01111, 0b10000, 0b01110, 0b00001, 0b11110],
    't' => [0b01000, 0b01000, 0b11100, 0b01000, 0b01000, 0b01001, 0b00110],
    'u' => [0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b10011, 0b01101],
    'v' => [0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
    'w' => [0b00000, 0b00000, 0b10001, 0b10001, 0b10101, 0b11011, 0b10001],
    'x' => [0b00000, 0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001],
    'y' => [0b00000, 0b10001, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
    'z' => [0b00000, 0b00000, 0b11111, 0b00010, 0b00100, 0b01000, 0b11111],
    '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
    '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
    '2' => [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
    '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
    '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
    '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
    '6' => [0b01110, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b01110],
    '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
    '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
    '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110],
    '@' => [0b01110, 0b10001, 0b10111, 0b10101, 0b10110, 0b10000, 0b01110],
    '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100],
    '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
    '_' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111],
    '/' => [0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000],
    ':' => [0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000],
    ' ' => [0; 7],
    _ => [0b00000, 0b01110, 0b01010, 0b01010, 0b01010, 0b01110, 0b00000],
  }
}

/// Render Shotify's pill watermark badge into an `RgbaImage` (`rgba(15,17,21,0.35)` frosted badge).
pub fn render_watermark_pill(cfg: &WatermarkConfig, scale: f64) -> Result<RgbaImage> {
  let s = clamp_f64(scale, 0.5, 4.0) as f32;
  let text = if cfg.text.trim().is_empty() {
    "Made with shotify"
  } else {
    cfg.text.trim()
  };

  let char_count = text.chars().count().max(1);
  let px_scale = (1.6 * s).max(1.0);
  let glyph_w = 5.0 * px_scale;
  let glyph_h = 7.0 * px_scale;
  let glyph_step = 6.2 * px_scale;
  let text_w = (char_count as f32) * glyph_step - (1.2 * px_scale);

  let prefix_kind = cfg.prefix.as_deref().unwrap_or(if cfg.show_twitter { "twitter" } else { "none" });
  let has_prefix = prefix_kind != "none";
  let prefix_w = if has_prefix { 11.0 * s + 4.0 * s } else { 0.0 };
  let verified_w = if cfg.show_verified { 11.0 * s + 4.0 * s } else { 0.0 };

  let pad_x = 8.0 * s;
  let pad_y = 4.0 * s;
  let inner_w = (pad_x * 2.0 + prefix_w + text_w + verified_w).ceil().max(24.0);
  let inner_h = (pad_y * 2.0 + glyph_h).ceil().max(16.0);

  let halo_pad = (6.0 * s).ceil();
  let total_w = (inner_w + halo_pad * 2.0) as u32;
  let total_h = (inner_h + halo_pad * 2.0) as u32;

  // Soft outer blur backdrop
  let mut halo_pm = Pixmap::new(total_w, total_h)
    .ok_or_else(|| ExportError::Encoding("watermark pixmap alloc failed".into()))?;
  let radius = 6.0 * s;
  if let Some(path) = build_rounded_rect_path(
    halo_pad,
    halo_pad,
    inner_w,
    inner_h,
    radius,
    radius,
    radius,
    radius,
  ) {
    let mut paint = Paint::default();
    paint.set_color(tiny_skia::Color::from_rgba8(15, 17, 21, 90));
    paint.anti_alias = true;
    halo_pm.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
  }
  let halo_rgba = pixmap_to_rgba_image(&halo_pm);
  let blurred = apply_gaussian_blur_image(&halo_rgba, 5.0 * f64::from(s), RenderQuality::Final);
  let mut pm = rgba_image_to_pixmap(&blurred)?;

  // Inner frosted badge fill + subtle 1px white border
  if let Some(path) = build_rounded_rect_path(
    halo_pad,
    halo_pad,
    inner_w,
    inner_h,
    radius,
    radius,
    radius,
    radius,
  ) {
    let mut fill_paint = Paint::default();
    fill_paint.set_color(tiny_skia::Color::from_rgba8(15, 17, 21, 92));
    fill_paint.anti_alias = true;
    pm.fill_path(&path, &fill_paint, FillRule::Winding, Transform::identity(), None);

    let mut stroke_paint = Paint::default();
    stroke_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 26));
    stroke_paint.anti_alias = true;
    let mut stroke = Stroke::default();
    stroke.width = 1.0 * s;
    pm.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
  }

  let text_y = halo_pad + (inner_h - glyph_h) / 2.0;
  let mut cursor_x = halo_pad + pad_x;

  // Optional prefix icon (Twitter/Mail/YouTube geometric indicator)
  if has_prefix {
    let cy = halo_pad + inner_h / 2.0;
    let cx = cursor_x + 4.5 * s;
    if let Some(circle) = PathBuilder::from_circle(cx, cy, 4.0 * s) {
      let mut p = Paint::default();
      let col = match prefix_kind {
        "youtube" => tiny_skia::Color::from_rgba8(239, 68, 68, 235),
        "mail" => tiny_skia::Color::from_rgba8(167, 139, 250, 235),
        _ => tiny_skia::Color::from_rgba8(56, 189, 248, 235),
      };
      p.set_color(col);
      p.anti_alias = true;
      pm.fill_path(&circle, &p, FillRule::Winding, Transform::identity(), None);
    }
    cursor_x += prefix_w;
  }

  // Draw crisp glyphs
  let mut fg_paint = Paint::default();
  fg_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 235));
  fg_paint.anti_alias = false;
  for ch in text.chars() {
    let bitmap = glyph_5x7(ch);
    for (row_idx, bits) in bitmap.iter().enumerate() {
      for col_idx in 0..5u8 {
        if (bits >> (4 - col_idx)) & 1 == 1 {
          let rx = cursor_x + f32::from(col_idx) * px_scale;
          let ry = text_y + (row_idx as f32) * px_scale;
          if let Some(rect) = tiny_skia::Rect::from_xywh(rx, ry, px_scale, px_scale) {
            pm.fill_rect(rect, &fg_paint, Transform::identity(), None);
          }
        }
      }
    }
    cursor_x += glyph_step;
  }

  let _ = glyph_w;

  // Optional verified check badge
  if cfg.show_verified {
    let cy = halo_pad + inner_h / 2.0;
    let cx = cursor_x + 4.5 * s;
    if let Some(circle) = PathBuilder::from_circle(cx, cy, 4.2 * s) {
      let mut p = Paint::default();
      p.set_color(tiny_skia::Color::from_rgba8(29, 155, 240, 240));
      p.anti_alias = true;
      pm.fill_path(&circle, &p, FillRule::Winding, Transform::identity(), None);
    }
    let mut pb = PathBuilder::new();
    pb.move_to(cx - 2.0 * s, cy);
    pb.line_to(cx - 0.4 * s, cy + 1.6 * s);
    pb.line_to(cx + 2.2 * s, cy - 1.6 * s);
    if let Some(check) = pb.finish() {
      let mut p = Paint::default();
      p.set_color(tiny_skia::Color::WHITE);
      p.anti_alias = true;
      let mut stroke = Stroke::default();
      stroke.width = 1.2 * s;
      pm.stroke_path(&check, &p, &stroke, Transform::identity(), None);
    }
  }

  Ok(pixmap_to_rgba_image(&pm))
}

/// Position and composite the watermark pill onto a rasterized export canvas.
/// Port of `ImageDisplay.jsx` watermark placement (`inside` smart corner avoidance + outside anchors).
pub fn apply_scene_watermark(
  canvas: &mut RgbaImage,
  card_bounds: RectF,
  cfg: &WatermarkConfig,
  scale: f64,
) -> Result<()> {
  if !cfg.enabled {
    return Ok(());
  }

  let pill = render_watermark_pill(cfg, scale)?;
  let pw = f64::from(pill.width());
  let ph = f64::from(pill.height());
  let cw = f64::from(canvas.width());
  let ch = f64::from(canvas.height());

  let inset = 12.0 * clamp_f64(scale, 0.5, 4.0);
  let ox = cfg.offset_x * scale;
  let oy = cfg.offset_y * scale;

  let pos = WatermarkPosition::parse(&cfg.position);
  let (x, y) = match pos {
    WatermarkPosition::Inside => {
      let candidates = [
        (card_bounds.right() - inset - pw + ox, card_bounds.bottom() - inset - ph + oy),
        (card_bounds.right() - inset - pw + ox, card_bounds.top + inset + oy),
        (card_bounds.left + inset + ox, card_bounds.bottom() - inset - ph + oy),
        (card_bounds.left + inset + ox, card_bounds.top + inset + oy),
      ];
      candidates
        .into_iter()
        .find(|&(cx, cy)| cx >= 1.0 && cy >= 1.0 && (cx + pw) <= cw - 1.0 && (cy + ph) <= ch - 1.0)
        .unwrap_or(candidates[0])
    }
    WatermarkPosition::CenterTop => ((cw - pw) / 2.0 + ox, inset + oy),
    WatermarkPosition::BottomLeft => (inset + ox, ch - inset - ph + oy),
    WatermarkPosition::BottomRight => (cw - inset - pw + ox, ch - inset - ph + oy),
    WatermarkPosition::TopLeft => (inset + ox, inset + oy),
    WatermarkPosition::TopRight => (cw - inset - pw + ox, inset + oy),
    WatermarkPosition::CenterBottom => {
      let gap = 12.0 * clamp_f64(scale, 0.5, 4.0);
      ((card_bounds.left + card_bounds.width / 2.0) - pw / 2.0 + ox, card_bounds.bottom() + gap + oy)
    }
  };

  composite_watermark_rgba(canvas, &pill, x as f32, y as f32);
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use image::Rgba;

  #[test]
  fn watermark_pill_renders_and_composites() {
    let mut base = RgbaImage::from_pixel(400, 300, Rgba([30, 30, 80, 255]));
    let cfg = WatermarkConfig {
      enabled: true,
      text: "Made with shotify".into(),
      show_verified: true,
      ..WatermarkConfig::default()
    };
    let card = RectF { left: 40.0, top: 40.0, width: 320.0, height: 220.0 };
    apply_scene_watermark(&mut base, card, &cfg, 1.0).unwrap();

    // Ensure pixels near bottom-right inside card changed due to watermark
    let unchanged = Rgba([30, 30, 80, 255]);
    let mut changed = 0;
    for y in 200..260 {
      for x in 200..360 {
        if *base.get_pixel(x, y) != unchanged {
          changed += 1;
        }
      }
    }
    assert!(changed > 20);
  }
}
