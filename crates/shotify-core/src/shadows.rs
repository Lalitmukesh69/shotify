use serde::{Deserialize, Serialize};
use tiny_skia::{
  GradientStop, LinearGradient, Paint, Path, PathBuilder, Pixmap, Point, Rect, SpreadMode,
  Transform,
};

use crate::background_fx::{
  gaussian_approx_blur_in_place, pixmap_to_rgba_image, rgba_image_to_pixmap,
};
use crate::color::{parse_css_color, Rgba};
use crate::defaults::ShadowParams;
use crate::error::{CoreError, Result};
use crate::inset::EdgePalette;
use crate::math::clamp_f64;

/// Shadow styles supported on screenshot cards (`ImageDisplay.jsx`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ShadowStyle {
  None,
  Hug,
  Heavy,
  Realistic,
  Ambient,
  Custom(ShadowParams),
}

impl ShadowStyle {
  pub fn from_options(shadow_name: &str, advanced: &ShadowParams) -> Self {
    if advanced.enabled || shadow_name.eq_ignore_ascii_case("Custom") {
      return Self::Custom(advanced.sanitize());
    }
    match shadow_name.trim() {
      "Heavy" => Self::Heavy,
      "Hug" | "Hug (Default)" => Self::Hug,
      "Realistic" => Self::Realistic,
      "Ambient" => Self::Ambient,
      _ => Self::None,
    }
  }

  pub fn to_params(&self) -> Option<ShadowParams> {
    match self {
      Self::None | Self::Ambient => None,
      Self::Hug => Some(ShadowParams::hug_preset()),
      Self::Heavy => Some(ShadowParams::heavy_preset()),
      Self::Realistic => Some(ShadowParams::realistic_preset()),
      Self::Custom(p) => Some(p.sanitize()),
    }
  }
}

/// Build a rounded rectangle `tiny_skia::Path` with independent corner radii `(tl, tr, br, bl)`.
pub fn build_rounded_rect_path(
  x: f32,
  y: f32,
  w: f32,
  h: f32,
  tl_in: f32,
  tr_in: f32,
  br_in: f32,
  bl_in: f32,
) -> Option<Path> {
  if w <= 0.0 || h <= 0.0 {
    return None;
  }
  let max_r = (w.min(h)) / 2.0;
  let tl = tl_in.clamp(0.0, max_r);
  let tr = tr_in.clamp(0.0, max_r);
  let br = br_in.clamp(0.0, max_r);
  let bl = bl_in.clamp(0.0, max_r);

  // Kappa for cubic bezier circular arc approximation: 4*(sqrt(2)-1)/3
  const K: f32 = 0.552_284_8;
  let mut pb = PathBuilder::new();

  pb.move_to(x + tl, y);
  pb.line_to(x + w - tr, y);
  if tr > 0.0 {
    pb.cubic_to(
      x + w - tr * (1.0 - K),
      y,
      x + w,
      y + tr * (1.0 - K),
      x + w,
      y + tr,
    );
  }
  pb.line_to(x + w, y + h - br);
  if br > 0.0 {
    pb.cubic_to(
      x + w,
      y + h - br * (1.0 - K),
      x + w - br * (1.0 - K),
      y + h,
      x + w - br,
      y + h,
    );
  }
  pb.line_to(x + bl, y + h);
  if bl > 0.0 {
    pb.cubic_to(
      x + bl * (1.0 - K),
      y + h,
      x,
      y + h - bl * (1.0 - K),
      x,
      y + h - bl,
    );
  }
  pb.line_to(x, y + tl);
  if tl > 0.0 {
    pb.cubic_to(x, y + tl * (1.0 - K), x + tl * (1.0 - K), y, x + tl, y);
  }
  pb.close();
  pb.finish()
}

/// Rendered shadow layer with canvas draw coordinates relative to the card's top-left corner.
pub struct RenderedShadow {
  pub pixmap: Pixmap,
  pub draw_x: i32,
  pub draw_y: i32,
}

/// Render a CSS-like box shadow (`offsetX`, `offsetY`, `blur`, `spread`, `color`, `opacity`) into a `Pixmap`.
pub fn render_drop_shadow_pixmap(
  card_w: f64,
  card_h: f64,
  corner_radius: f64,
  scale: f64,
  params: &ShadowParams,
) -> Result<Option<RenderedShadow>> {
  let clean = params.sanitize();
  if clean.opacity <= 0.001 || card_w <= 0.0 || card_h <= 0.0 {
    return Ok(None);
  }

  let s = clamp_f64(scale, 0.25, 8.0);
  let spread = clean.spread * s;
  let rect_w = (card_w + spread * 2.0).max(1.0);
  let rect_h = (card_h + spread * 2.0).max(1.0);
  let blur_px = clean.blur.max(0.0) * s;
  let sigma = blur_px * 0.5;
  let pad = (blur_px * 1.5).ceil().max(4.0);

  let pm_w = (rect_w + pad * 2.0).ceil() as u32;
  let pm_h = (rect_h + pad * 2.0).ceil() as u32;
  let mut pm = Pixmap::new(pm_w.max(1), pm_h.max(1))
    .ok_or(CoreError::PixmapAllocation { width: pm_w, height: pm_h })?;

  let radius = (corner_radius + spread).max(0.0) as f32;
  let base_col = parse_css_color(&clean.color).unwrap_or(Rgba::BLACK);
  let shadow_col = base_col.with_alpha_mul(clean.opacity as f32).to_skia_color();

  if let Some(path) = build_rounded_rect_path(
    pad as f32,
    pad as f32,
    rect_w as f32,
    rect_h as f32,
    radius,
    radius,
    radius,
    radius,
  ) {
    let mut paint = Paint::default();
    paint.set_color(shadow_col);
    paint.anti_alias = true;
    pm.fill_path(&path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
  }

  if sigma > 0.25 {
    let mut straight = pixmap_to_rgba_image(&pm);
    gaussian_approx_blur_in_place(straight.as_mut(), pm_w.max(1), pm_h.max(1), sigma);
    pm = rgba_image_to_pixmap(&straight)?;
  }

  let draw_x = ((clean.offset_x * s) - spread - pad).round() as i32;
  let draw_y = ((clean.offset_y * s) - spread - pad).round() as i32;

  Ok(Some(RenderedShadow {
    pixmap: pm,
    draw_x,
    draw_y,
  }))
}

/// Render an Ambient glow layer derived from edge palette or edge fallback color.
/// Port of `components/editor/ImageDisplay.jsx::screenshotAmbientGlowStyle`.
pub fn render_ambient_glow_pixmap(
  card_w: f64,
  card_h: f64,
  corner_radius: f64,
  scale: f64,
  edge_palette: Option<&EdgePalette>,
  fallback_edge_color: Option<&str>,
) -> Result<Option<RenderedShadow>> {
  let s = clamp_f64(scale, 0.25, 8.0);
  let thickness = 56.0 * s;
  let blur_px = 24.0 * s;
  let pad = (blur_px * 2.0 + thickness / 2.0).ceil();

  let glow_w = card_w + thickness;
  let glow_h = card_h + thickness;
  let pm_w = (card_w + pad * 2.0).ceil().max(1.0) as u32;
  let pm_h = (card_h + pad * 2.0).ceil().max(1.0) as u32;

  let mut pm = Pixmap::new(pm_w, pm_h).ok_or(CoreError::PixmapAllocation { width: pm_w, height: pm_h })?;

  let origin_x = (pad - thickness / 2.0) as f32;
  let origin_y = (pad - thickness / 2.0) as f32;
  let r_outer = (corner_radius.max(0.0) + thickness / 2.0) as f32;

  let make_stops = |cols: &[String]| -> Vec<GradientStop> {
    if cols.is_empty() {
      return vec![GradientStop::new(0.0, tiny_skia::Color::from_rgba8(224, 224, 224, 255))];
    }
    let denom = (cols.len().saturating_sub(1)).max(1) as f32;
    cols
      .iter()
      .enumerate()
      .map(|(i, c)| {
        let skia_c = parse_css_color(c).unwrap_or(Rgba::new(224, 224, 224, 1.0)).to_skia_color();
        GradientStop::new((i as f32) / denom, skia_c)
      })
      .collect()
  };

  if let Some(pal) = edge_palette {
    let mut layer = Pixmap::new(pm_w, pm_h).ok_or(CoreError::PixmapAllocation { width: pm_w, height: pm_h })?;
    let th = thickness as f32;
    let gw = glow_w as f32;
    let gh = glow_h as f32;

    let strips = [
      (&pal.top, Point::from_xy(origin_x, origin_y), Point::from_xy(origin_x + gw, origin_y), Rect::from_xywh(origin_x, origin_y, gw, th)),
      (&pal.right, Point::from_xy(origin_x + gw - th, origin_y), Point::from_xy(origin_x + gw - th, origin_y + gh), Rect::from_xywh(origin_x + gw - th, origin_y, th, gh)),
      (&pal.bottom, Point::from_xy(origin_x, origin_y + gh - th), Point::from_xy(origin_x + gw, origin_y + gh - th), Rect::from_xywh(origin_x, origin_y + gh - th, gw, th)),
      (&pal.left, Point::from_xy(origin_x, origin_y), Point::from_xy(origin_x, origin_y + gh), Rect::from_xywh(origin_x, origin_y, th, gh)),
    ];

    for (cols, p0, p1, rect_opt) in strips {
      if let (Some(shader), Some(rect)) = (
        LinearGradient::new(p0, p1, make_stops(cols), SpreadMode::Pad, Transform::identity()),
        rect_opt,
      ) {
        let mut paint = Paint::default();
        paint.shader = shader;
        layer.fill_rect(rect, &paint, Transform::identity(), None);
      }
    }

    if let Some(mask_path) = build_rounded_rect_path(origin_x, origin_y, gw, gh, r_outer, r_outer, r_outer, r_outer) {
      let mut mask = tiny_skia::Mask::new(pm_w, pm_h).ok_or(CoreError::PixmapAllocation { width: pm_w, height: pm_h })?;
      mask.fill_path(&mask_path, tiny_skia::FillRule::Winding, true, Transform::identity());
      pm.draw_pixmap(0, 0, layer.as_ref(), &tiny_skia::PixmapPaint::default(), Transform::identity(), Some(&mask));
    }
  } else if let Some(col_str) = fallback_edge_color {
    if col_str.eq_ignore_ascii_case("transparent") {
      return Ok(None);
    }
    let col = parse_css_color(col_str).unwrap_or(Rgba::new(224, 224, 224, 1.0));
    if let Some(path) = build_rounded_rect_path(origin_x, origin_y, glow_w as f32, glow_h as f32, r_outer, r_outer, r_outer, r_outer) {
      let mut paint = Paint::default();
      paint.set_color(col.with_alpha_mul(0.34).to_skia_color());
      pm.fill_path(&path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
    }
  } else {
    return Ok(None);
  }

  let mut straight = pixmap_to_rgba_image(&pm);
  gaussian_approx_blur_in_place(straight.as_mut(), pm_w, pm_h, blur_px * 0.75);

  // Subtle saturation boost + 0.45 overall glow opacity matching CSS filter: blur(24px) saturate(1.28)
  for px in straight.chunks_exact_mut(4) {
    let r = f32::from(px[0]);
    let g = f32::from(px[1]);
    let b = f32::from(px[2]);
    let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let sat = 1.28_f32;
    px[0] = (luma + (r - luma) * sat).round().clamp(0.0, 255.0) as u8;
    px[1] = (luma + (g - luma) * sat).round().clamp(0.0, 255.0) as u8;
    px[2] = (luma + (b - luma) * sat).round().clamp(0.0, 255.0) as u8;
    px[3] = (f32::from(px[3]) * 0.45).round().clamp(0.0, 255.0) as u8;
  }

  let out_pm = rgba_image_to_pixmap(&straight)?;
  Ok(Some(RenderedShadow {
    pixmap: out_pm,
    draw_x: -(pad.round() as i32),
    draw_y: -(pad.round() as i32),
  }))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn drop_shadow_renders_soft_alpha() {
    let res = render_drop_shadow_pixmap(100.0, 60.0, 12.0, 1.0, &ShadowParams::hug_preset())
      .unwrap()
      .expect("shadow present");
    assert!(res.pixmap.width() > 100);
    assert!(res.pixmap.height() > 60);
    // Center of shadow must have positive alpha
    let center = res.pixmap.pixel(res.pixmap.width() / 2, res.pixmap.height() / 2).unwrap();
    assert!(center.alpha() > 0);
  }
}
