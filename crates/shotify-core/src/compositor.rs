use std::sync::OnceLock;

use ab_glyph::{Font, FontRef, OutlineCurve, PxScale, ScaleFont};
use image::{Rgba as ImgRgba, RgbaImage};
use serde::{Deserialize, Serialize};
use tiny_skia::{
  FillRule, LineCap, LineJoin, Mask, Paint, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke,
  Transform,
};

static UI_FONT_BYTES: &[u8] = include_bytes!("../../../assets/fonts/Cantarell-Regular.otf");
static UI_FONT_INSTANCE: OnceLock<FontRef<'static>> = OnceLock::new();

fn ui_regular_font() -> &'static FontRef<'static> {
  UI_FONT_INSTANCE.get_or_init(|| {
    FontRef::try_from_slice(UI_FONT_BYTES).expect("embedded Cantarell-Regular.otf is valid")
  })
}

use crate::background_fx::{
  pixmap_to_rgba_image, render_background_pixmap, rgba_image_to_pixmap, BackgroundSpec,
  RenderQuality,
};
use crate::color::{parse_css_color, Rgba};
use crate::defaults::{AspectMode, EditorOptions};
use crate::error::{CoreError, Result};
use crate::helpers::{aspect_class_to_dimensions, BrowserBarVariant};
use crate::image_balance::{balance_rgba_image, BalanceOptions};
use crate::inset::{get_edge_color, get_edge_palette, EdgeColorOptions};
use crate::layout::{
  compute_browser_bar_metrics, compute_card_layout, compute_inset_content_radius_px,
  compute_object_contain_rect, BrowserBarMetrics, CardLayout,
};
use crate::math::snap_to_device_pixel;
use crate::perspective::{composite_pixmap_transformed, CardTransform3D};
use crate::shadows::{
  build_rounded_rect_path, render_ambient_glow_pixmap, render_drop_shadow_pixmap, ShadowStyle,
};

/// Declarative nodes forming Shotify's platform-independent 2D scene graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CompositionNode {
  Background {
    spec: BackgroundSpec,
    blur_px: f64,
    brightness: f64,
    vignette: f64,
    noise: bool,
  },
  AmbientGlow {
    corner_radius: f64,
    edge_color: Option<String>,
  },
  Shadow {
    style: ShadowStyle,
    corner_radius: f64,
  },
  BrowserChrome {
    metrics: BrowserBarMetrics,
    width: f64,
  },
  ScreenshotCard {
    width: f64,
    height: f64,
    corner_radius: f64,
    inset_padding: f64,
    inset_color: String,
    border_width: f64,
    border_color: String,
    transform: CardTransform3D,
  },
}

/// Complete composition scene tree ready to rasterize at any scale or render inside Iced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionTree {
  pub canvas_width: u32,
  pub canvas_height: u32,
  pub scale: f64,
  pub layout: CardLayout,
  pub nodes: Vec<CompositionNode>,
}

/// Output produced by rasterizing a `CompositionTree`.
pub struct CompositionOutput {
  pub tree: CompositionTree,
  pub image: RgbaImage,
}

fn default_border_color_for_variant(variant: BrowserBarVariant) -> &'static str {
  match variant {
    BrowserBarVariant::MacLight | BrowserBarVariant::Hidden => "rgba(255,255,255,0.88)",
    BrowserBarVariant::MacDark => "rgba(22,23,26,0.88)",
    BrowserBarVariant::WindowsLight => "rgba(246,248,253,0.92)",
    BrowserBarVariant::WindowsDark => "rgba(32,36,44,0.92)",
    BrowserBarVariant::SafariDark => "rgba(38,38,38,0.92)",
  }
}

/// Compute target logical canvas dimensions matching the selected aspect preset and container width.
pub fn resolve_canvas_dimensions(
  options: &EditorOptions,
  image_w: u32,
  image_h: u32,
  target_width: u32,
) -> (u32, u32) {
  let w = target_width.max(1);
  match options.aspect_mode {
    AspectMode::Auto => {
      let pad = options.padding_size.max(0.0).round();
      let browser_bar = compute_browser_bar_metrics(
        BrowserBarVariant::parse(&options.browser_bar),
        options.browser_frame.scale,
      );
      let bar_h = if browser_bar.enabled { browser_bar.height } else { 0.0 };
      let content_w = (f64::from(w) - pad * 2.0).max(1.0);
      let aspect = if image_w > 0 && image_h > 0 {
        f64::from(image_h) / f64::from(image_w)
      } else {
        9.0 / 16.0
      };
      let h = (content_w * aspect + bar_h + pad * 2.0).round().max(1.0) as u32;
      (w, h)
    }
    AspectMode::Custom => {
      let cw = options.aspect_custom.w.max(1.0);
      let ch = options.aspect_custom.h.max(1.0);
      let h = (f64::from(w) * (ch / cw)).round().max(1.0) as u32;
      (w, h)
    }
    AspectMode::Preset => {
      if let Some((aw, ah)) = aspect_class_to_dimensions(&options.aspect_ratio) {
        let h = (f64::from(w) * (f64::from(ah) / f64::from(aw))).round().max(1.0) as u32;
        (w, h)
      } else {
        (w, w)
      }
    }
  }
}

fn draw_ui_text_single_line(
  pixmap: &mut Pixmap,
  text: &str,
  start_x: f32,
  center_y: f32,
  font_size: f32,
  color: tiny_skia::Color,
  clip_mask: Option<&Mask>,
) {
  if text.is_empty() {
    return;
  }
  let font = ui_regular_font();
  let px_scale = PxScale::from(font_size.max(6.0));
  let scaled = font.as_scaled(px_scale);
  let unscaled_h = font.height_unscaled().max(1.0);
  let h_factor = px_scale.x / unscaled_h;
  let v_factor = -px_scale.y / unscaled_h;
  let baseline_y = center_y + (scaled.ascent() + scaled.descent()) * 0.5;

  let mut pb = PathBuilder::new();
  let mut cursor_x = start_x;
  let mut prev = None;
  let mut has_segments = false;
  let same_pt = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() <= 0.02 && (a.1 - b.1).abs() <= 0.02;

  for ch in text.chars() {
    let glyph_id = font.glyph_id(ch);
    if let Some(p) = prev {
      cursor_x += scaled.kern(p, glyph_id);
    }
    if let Some(outline) = font.outline(glyph_id) {
      let mut last_end: Option<(f32, f32)> = None;
      let gx = cursor_x;
      let gy = baseline_y;
      for curve in outline.curves {
        has_segments = true;
        match curve {
          OutlineCurve::Line(p0, p1) => {
            let s = (gx + p0.x * h_factor, gy + p0.y * v_factor);
            let e = (gx + p1.x * h_factor, gy + p1.y * v_factor);
            if !last_end.is_some_and(|pr| same_pt(pr, s)) {
              if last_end.is_some() {
                pb.close();
              }
              pb.move_to(s.0, s.1);
            }
            pb.line_to(e.0, e.1);
            last_end = Some(e);
          }
          OutlineCurve::Quad(p0, p1, p2) => {
            let s = (gx + p0.x * h_factor, gy + p0.y * v_factor);
            let c = (gx + p1.x * h_factor, gy + p1.y * v_factor);
            let e = (gx + p2.x * h_factor, gy + p2.y * v_factor);
            if !last_end.is_some_and(|pr| same_pt(pr, s)) {
              if last_end.is_some() {
                pb.close();
              }
              pb.move_to(s.0, s.1);
            }
            pb.quad_to(c.0, c.1, e.0, e.1);
            last_end = Some(e);
          }
          OutlineCurve::Cubic(p0, p1, p2, p3) => {
            let s = (gx + p0.x * h_factor, gy + p0.y * v_factor);
            let c1 = (gx + p1.x * h_factor, gy + p1.y * v_factor);
            let c2 = (gx + p2.x * h_factor, gy + p2.y * v_factor);
            let e = (gx + p3.x * h_factor, gy + p3.y * v_factor);
            if !last_end.is_some_and(|pr| same_pt(pr, s)) {
              if last_end.is_some() {
                pb.close();
              }
              pb.move_to(s.0, s.1);
            }
            pb.cubic_to(c1.0, c1.1, c2.0, c2.1, e.0, e.1);
            last_end = Some(e);
          }
        }
      }
      if last_end.is_some() {
        pb.close();
      }
    }
    cursor_x += scaled.h_advance(glyph_id);
    prev = Some(glyph_id);
  }

  if !has_segments {
    return;
  }
  if let Some(path) = pb.finish() {
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    pixmap.fill_path(&path, &paint, FillRule::EvenOdd, Transform::identity(), clip_mask);
  }
}

fn measure_ui_text_width(text: &str, font_size: f32) -> f32 {
  let font = ui_regular_font();
  let scaled = font.as_scaled(PxScale::from(font_size.max(6.0)));
  let mut w = 0.0_f32;
  let mut prev = None;
  for ch in text.chars() {
    let id = font.glyph_id(ch);
    if let Some(p) = prev {
      w += scaled.kern(p, id);
    }
    w += scaled.h_advance(id);
    prev = Some(id);
  }
  w
}

fn draw_browser_chrome(
  pixmap: &mut Pixmap,
  card_w: f32,
  metrics: &BrowserBarMetrics,
  scale: f64,
  browser_url: Option<&str>,
) {
  if !metrics.enabled || metrics.height <= 0.0 {
    return;
  }
  let s = scale as f32;
  let bar_h = (metrics.height as f32) * s;
  let bar_scale = (metrics.scale as f32) * s;

  let bg_str = match metrics.variant {
    BrowserBarVariant::WindowsLight => "rgba(246,248,253,0.92)",
    BrowserBarVariant::WindowsDark => "rgba(32,36,44,0.92)",
    BrowserBarVariant::MacLight => "rgba(255,255,255,0.85)",
    BrowserBarVariant::MacDark => "rgba(40,40,48,0.9)",
    BrowserBarVariant::SafariDark => "rgba(38,38,38,0.94)",
    BrowserBarVariant::Hidden => return,
  };
  let bg = parse_css_color(bg_str).unwrap_or(Rgba::WHITE).to_skia_color();

  if let Some(rect) = Rect::from_xywh(0.0, 0.0, card_w, bar_h) {
    let mut paint = Paint::default();
    paint.set_color(bg);
    pixmap.fill_rect(rect, &paint, Transform::identity(), None);
  }

  match metrics.variant {
    BrowserBarVariant::MacLight | BrowserBarVariant::MacDark | BrowserBarVariant::SafariDark => {
      let dots = if metrics.variant.is_dark() {
        ["#f87171", "#facc15", "#34d399"]
      } else {
        ["#fb7185", "#fde047", "#4ade80"]
      };
      let pad_x = (20.0 * bar_scale).max(10.0 * s);
      let cy = bar_h / 2.0;
      let r = 6.0 * bar_scale;
      let step = 20.0 * bar_scale;

      for (i, hex) in dots.iter().enumerate() {
        let cx = pad_x + r + (i as f32) * step;
        if let Some(circle) = PathBuilder::from_circle(cx, cy, r) {
          let mut paint = Paint::default();
          paint.set_color(parse_css_color(hex).unwrap_or(Rgba::WHITE).to_skia_color());
          paint.anti_alias = true;
          pixmap.fill_path(&circle, &paint, FillRule::Winding, Transform::identity(), None);
        }
      }

      if metrics.variant == BrowserBarVariant::SafariDark {
        let icon_col = parse_css_color("#A3A3A3").unwrap_or(Rgba::WHITE).to_skia_color();
        let mut icon_paint = Paint::default();
        icon_paint.set_color(icon_col);
        icon_paint.anti_alias = true;
        let icon_stroke = Stroke {
          width: (1.5 * bar_scale).max(1.0),
          line_cap: LineCap::Round,
          line_join: LineJoin::Round,
          ..Default::default()
        };

        // Left chevrons (< and >) after traffic lights
        let nav_x0 = pad_x + 3.0 * step + 10.0 * bar_scale;
        let nav_sz = (18.0 * bar_scale).max(10.0);
        let chev_r = nav_sz * 0.24;
        let nav_gap = 14.0 * bar_scale;
        let left_cx = nav_x0 + nav_sz * 0.5;
        let right_cx = nav_x0 + nav_sz + nav_gap + nav_sz * 0.5;
        for (cx, dir) in [(left_cx, -1.0_f32), (right_cx, 1.0_f32)] {
          let mut pb = PathBuilder::new();
          pb.move_to(cx - dir * chev_r, cy - chev_r * 1.4);
          pb.line_to(cx + dir * chev_r, cy);
          pb.line_to(cx - dir * chev_r, cy + chev_r * 1.4);
          if let Some(p) = pb.finish() {
            pixmap.stroke_path(&p, &icon_paint, &icon_stroke, Transform::identity(), None);
          }
        }

        // Right toolbar icons: Share, Copy, Plus
        let right_icon_sz = (16.0 * bar_scale).max(10.0);
        let right_gap = 12.0 * bar_scale;
        let plus_cx = card_w - pad_x - right_icon_sz * 0.5;
        let copy_cx = plus_cx - right_icon_sz - right_gap;
        let share_cx = copy_cx - right_icon_sz - right_gap;

        let hs = right_icon_sz * 0.28;
        // Plus icon (+)
        let mut plus_pb = PathBuilder::new();
        plus_pb.move_to(plus_cx, cy - hs);
        plus_pb.line_to(plus_cx, cy + hs);
        plus_pb.move_to(plus_cx - hs, cy);
        plus_pb.line_to(plus_cx + hs, cy);
        if let Some(p) = plus_pb.finish() {
          pixmap.stroke_path(&p, &icon_paint, &icon_stroke, Transform::identity(), None);
        }

        // Copy icon (two staggered boxes)
        if let Some(r_front) = build_rounded_rect_path(
          copy_cx - hs * 0.3,
          cy - hs * 0.3,
          hs * 1.35,
          hs * 1.35,
          1.5 * bar_scale,
          1.5 * bar_scale,
          1.5 * bar_scale,
          1.5 * bar_scale,
        ) {
          pixmap.stroke_path(&r_front, &icon_paint, &icon_stroke, Transform::identity(), None);
        }
        let mut back_pb = PathBuilder::new();
        back_pb.move_to(copy_cx + hs * 0.55, cy - hs * 0.3);
        back_pb.line_to(copy_cx + hs * 0.55, cy - hs * 0.95);
        back_pb.line_to(copy_cx - hs * 0.95, cy - hs * 0.95);
        back_pb.line_to(copy_cx - hs * 0.95, cy + hs * 0.55);
        back_pb.line_to(copy_cx - hs * 0.3, cy + hs * 0.55);
        if let Some(p) = back_pb.finish() {
          pixmap.stroke_path(&p, &icon_paint, &icon_stroke, Transform::identity(), None);
        }

        // Share icon (arrow up out of tray)
        let mut share_pb = PathBuilder::new();
        share_pb.move_to(share_cx, cy - hs * 1.15);
        share_pb.line_to(share_cx, cy + hs * 0.4);
        share_pb.move_to(share_cx - hs * 0.55, cy - hs * 0.55);
        share_pb.line_to(share_cx, cy - hs * 1.15);
        share_pb.line_to(share_cx + hs * 0.55, cy - hs * 0.55);
        if let Some(p) = share_pb.finish() {
          pixmap.stroke_path(&p, &icon_paint, &icon_stroke, Transform::identity(), None);
        }
        if let Some(tray) = build_rounded_rect_path(
          share_cx - hs * 1.05,
          cy + hs * 0.15,
          hs * 2.1,
          hs * 0.9,
          1.5 * bar_scale,
          1.5 * bar_scale,
          1.5 * bar_scale,
          1.5 * bar_scale,
        ) {
          pixmap.stroke_path(&tray, &icon_paint, &icon_stroke, Transform::identity(), None);
        }

        // Centered Safari address bar pill
        let left_bound = right_cx + nav_sz * 0.5 + 16.0 * bar_scale;
        let right_bound = share_cx - right_icon_sz * 0.5 - 16.0 * bar_scale;
        let avail_w = (right_bound - left_bound).max(40.0);
        let pill_w = avail_w.min(660.0 * bar_scale);
        let pill_h = (34.0 * bar_scale).max(20.0 * s).min((bar_h - 8.0 * s).max(16.0));
        let pill_x = left_bound + (avail_w - pill_w) / 2.0;
        let pill_y = cy - pill_h / 2.0;
        let pill_r = (9.0 * bar_scale).min(pill_h / 2.0);

        if let Some(pill_path) = build_rounded_rect_path(
          pill_x, pill_y, pill_w, pill_h, pill_r, pill_r, pill_r, pill_r,
        ) {
          let mut pill_paint = Paint::default();
          pill_paint.set_color(parse_css_color("#404040").unwrap_or(Rgba::BLACK).to_skia_color());
          pill_paint.anti_alias = true;
          pixmap.fill_path(&pill_path, &pill_paint, FillRule::Winding, Transform::identity(), None);

          let mut pill_mask = Mask::new(pixmap.width(), pixmap.height());
          if let Some(ref mut mask) = pill_mask {
            mask.fill_path(&pill_path, FillRule::Winding, true, Transform::identity());
          }

          let url_text = browser_url.unwrap_or("").trim();
          let font_sz = (14.0 * bar_scale).max(10.0 * s);
          let lock_sz = (12.0 * bar_scale).max(8.0 * s);
          let gap = (7.0 * bar_scale).max(4.0 * s);
          let text_w = if url_text.is_empty() {
            0.0
          } else {
            measure_ui_text_width(url_text, font_sz)
          };
          let total_w = if text_w > 0.0 {
            lock_sz + gap + text_w
          } else {
            lock_sz
          };
          let max_inner_w = (pill_w - 20.0 * bar_scale).max(lock_sz);
          let content_start_x = if total_w <= max_inner_w {
            pill_x + (pill_w - total_w) / 2.0
          } else {
            pill_x + 10.0 * bar_scale
          };

          // Padlock glyph inside address pill
          let lock_x = content_start_x;
          let lw = lock_sz * 0.72;
          let lh = lock_sz * 0.48;
          let lock_cx = lock_x + lock_sz * 0.5;
          if let Some(lock_body) = build_rounded_rect_path(
            lock_cx - lw / 2.0,
            cy - lh * 0.1,
            lw,
            lh,
            1.5 * bar_scale,
            1.5 * bar_scale,
            1.5 * bar_scale,
            1.5 * bar_scale,
          ) {
            pixmap.stroke_path(
              &lock_body,
              &icon_paint,
              &icon_stroke,
              Transform::identity(),
              pill_mask.as_ref(),
            );
          }
          let shackle_r = lw * 0.28;
          let mut shackle = PathBuilder::new();
          shackle.move_to(lock_cx - shackle_r, cy - lh * 0.1);
          shackle.line_to(lock_cx - shackle_r, cy - lh * 0.55);
          shackle.quad_to(lock_cx, cy - lh * 1.05, lock_cx + shackle_r, cy - lh * 0.55);
          shackle.line_to(lock_cx + shackle_r, cy - lh * 0.1);
          if let Some(sp) = shackle.finish() {
            pixmap.stroke_path(
              &sp,
              &icon_paint,
              &icon_stroke,
              Transform::identity(),
              pill_mask.as_ref(),
            );
          }

          if !url_text.is_empty() {
            draw_ui_text_single_line(
              pixmap,
              url_text,
              lock_x + lock_sz + gap,
              cy,
              font_sz,
              icon_col,
              pill_mask.as_ref(),
            );
          }
        }
      }
    }
    BrowserBarVariant::WindowsLight | BrowserBarVariant::WindowsDark => {
      let sep_col = if metrics.variant.is_dark() {
        parse_css_color("rgba(255,255,255,0.08)")
      } else {
        parse_css_color("rgba(15,18,27,0.08)")
      }
      .unwrap_or(Rgba::BLACK)
      .to_skia_color();

      if let Some(line) = Rect::from_xywh(0.0, (bar_h - s).max(0.0), card_w, s) {
        let mut p = Paint::default();
        p.set_color(sep_col);
        pixmap.fill_rect(line, &p, Transform::identity(), None);
      }

      let fg = if metrics.variant.is_dark() {
        Rgba::new(255, 255, 255, 0.85)
      } else {
        Rgba::new(15, 18, 27, 0.80)
      }
      .to_skia_color();

      let mut stroke = Stroke::default();
      stroke.width = (1.5 * bar_scale).max(1.0);
      let mut paint = Paint::default();
      paint.set_color(fg);
      paint.anti_alias = true;

      let cy = bar_h / 2.0;
      let box_sz = 10.0 * bar_scale;
      let half = box_sz / 2.0;
      let right_pad = 14.0 * bar_scale;
      let gap = 36.0 * bar_scale;

      // Close icon (X)
      let close_cx = card_w - right_pad - half;
      let mut pb = PathBuilder::new();
      pb.move_to(close_cx - half, cy - half);
      pb.line_to(close_cx + half, cy + half);
      pb.move_to(close_cx + half, cy - half);
      pb.line_to(close_cx - half, cy + half);
      if let Some(p) = pb.finish() {
        pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
      }

      // Maximize square
      let max_cx = close_cx - gap;
      if let Some(r) = Rect::from_xywh(max_cx - half, cy - half, box_sz, box_sz) {
        let path = PathBuilder::from_rect(r);
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
      }

      // Minimize line
      let min_cx = max_cx - gap;
      let mut pb2 = PathBuilder::new();
      pb2.move_to(min_cx - half, cy);
      pb2.line_to(min_cx + half, cy);
      if let Some(p) = pb2.finish() {
        pixmap.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
      }
    }
    BrowserBarVariant::Hidden => {}
  }
}

/// Build the `CompositionTree` and rasterize the scene graph into an RGBA image.
pub fn compose_screenshot_scene(
  raw_screenshot: &RgbaImage,
  wallpaper: Option<&RgbaImage>,
  options: &EditorOptions,
  target_width: u32,
  scale: f64,
  quality: RenderQuality,
) -> Result<CompositionOutput> {
  let mut opts = options.clone();
  opts.normalize(None);

  let (source_img, _) = if opts.balance.enabled {
    balance_rgba_image(raw_screenshot, &BalanceOptions::default())
  } else {
    (raw_screenshot.clone(), crate::image_balance::BalanceDimensions {
      width: raw_screenshot.width(),
      height: raw_screenshot.height(),
      x: 0,
      y: 0,
      original_width: raw_screenshot.width(),
      original_height: raw_screenshot.height(),
    })
  };

  let (iw, ih) = source_img.dimensions();
  let (canvas_w, canvas_h) = resolve_canvas_dimensions(&opts, iw, ih, target_width);
  let browser_bar_variant = BrowserBarVariant::parse(&opts.browser_bar);
  let browser_bar = compute_browser_bar_metrics(browser_bar_variant, opts.browser_frame.scale);

  let layout = compute_card_layout(
    f64::from(canvas_w),
    f64::from(canvas_h),
    f64::from(iw),
    f64::from(ih),
    opts.padding_size,
    opts.inset.padding,
    opts.aspect_mode,
    browser_bar,
    scale,
  );

  let bg_spec = if opts.background_type.eq_ignore_ascii_case("transparent") {
    BackgroundSpec::Transparent
  } else if opts.background_type.eq_ignore_ascii_case("wallpaper") && wallpaper.is_some() {
    BackgroundSpec::Wallpaper
  } else if opts.background_type.eq_ignore_ascii_case("blobGradient") {
    BackgroundSpec::BlobGradient {
      config: opts.blob_gradient.clone(),
    }
  } else {
    BackgroundSpec::Gradient {
      angle_deg: opts.custom_theme.angle_deg,
      color_start: opts.custom_theme.color_start.clone(),
      color_end: opts.custom_theme.color_end.clone(),
    }
  };

  let shadow_style = ShadowStyle::from_options(&opts.shadow, &opts.advanced_shadow);
  let should_tint_edges = opts.inset.padding > 0.0 || opts.border_width > 0.0;
  let detected_edge_col = if should_tint_edges || matches!(shadow_style, ShadowStyle::Ambient) {
    Some(get_edge_color(
      &source_img,
      EdgeColorOptions {
        edge_size: 20,
        include_alpha: false,
        min_alpha: 16,
      },
    ))
  } else {
    None
  };
  let edge_hex = detected_edge_col.map(|c| c.to_hex());

  let inset_color = if should_tint_edges {
    edge_hex.clone().unwrap_or_else(|| "transparent".to_string())
  } else {
    "transparent".to_string()
  };

  let border_width = opts.border_width.max(0.0).round();
  let border_color = opts
    .border_color
    .as_deref()
    .map(str::trim)
    .filter(|s| !s.is_empty())
    .unwrap_or_else(|| default_border_color_for_variant(browser_bar_variant))
    .to_string();

  let transform_3d = CardTransform3D {
    translate_x: opts.position.x * scale,
    translate_y: opts.position.y * scale,
    rotate_z_deg: opts.position.rotate,
    tilt_x_deg: opts.tilt.x,
    tilt_y_deg: opts.tilt.y,
    perspective_px: 1000.0 * scale,
  };

  let nodes = vec![
    CompositionNode::Background {
      spec: bg_spec.clone(),
      blur_px: opts.fx.blur,
      brightness: opts.fx.brightness,
      vignette: opts.fx.vignette,
      noise: opts.fx.noise,
    },
    CompositionNode::Shadow {
      style: shadow_style.clone(),
      corner_radius: f64::from(opts.corner_radius),
    },
    CompositionNode::BrowserChrome {
      metrics: browser_bar,
      width: layout.display_width,
    },
    CompositionNode::ScreenshotCard {
      width: layout.display_width,
      height: layout.card_total_height,
      corner_radius: f64::from(opts.corner_radius),
      inset_padding: opts.inset.padding,
      inset_color: inset_color.clone(),
      border_width,
      border_color: border_color.clone(),
      transform: transform_3d,
    },
  ];

  let tree = CompositionTree {
    canvas_width: canvas_w,
    canvas_height: canvas_h,
    scale,
    layout,
    nodes,
  };

  // Rasterize composition
  let mut canvas = render_background_pixmap(
    canvas_w,
    canvas_h,
    scale,
    &bg_spec,
    wallpaper,
    opts.fx.blur,
    opts.fx.brightness,
    opts.fx.vignette,
    opts.fx.noise,
    quality,
  )?;

  let eff_card_w = (layout.display_width * scale).round().max(1.0);
  let eff_card_h = (layout.card_total_height * scale).round().max(1.0);
  let card_w_u32 = eff_card_w as u32;
  let card_h_u32 = eff_card_h as u32;
  let outer_radius = f64::from(opts.corner_radius) * scale;

  // Build isolated card pixmap (card + shadow if no 3D tilt, or compose shadow + card)
  let card_center_x = f64::from(canvas.width()) / 2.0;
  let card_center_y = f64::from(canvas.height()) / 2.0;

  // Draw Ambient glow or Drop shadow behind the card
  if matches!(shadow_style, ShadowStyle::Ambient) {
    let edge_pal = get_edge_palette(&source_img, 28, 20, 16);
    if let Some(glow) = render_ambient_glow_pixmap(
      layout.display_width,
      layout.card_total_height,
      f64::from(opts.corner_radius),
      scale,
      Some(&edge_pal),
      edge_hex.as_deref(),
    )? {
      let mut glow_canvas = Pixmap::new(canvas.width(), canvas.height())
        .ok_or(CoreError::PixmapAllocation { width: canvas.width(), height: canvas.height() })?;
      let gx = (card_center_x - eff_card_w / 2.0 + opts.position.x * scale + f64::from(glow.draw_x)).round() as i32;
      let gy = (card_center_y - eff_card_h / 2.0 + opts.position.y * scale + f64::from(glow.draw_y)).round() as i32;
      glow_canvas.draw_pixmap(gx, gy, glow.pixmap.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
      canvas.draw_pixmap(0, 0, glow_canvas.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
    }
  } else if let Some(shadow_params) = shadow_style.to_params() {
    if let Some(shadow) = render_drop_shadow_pixmap(
      layout.display_width,
      layout.card_total_height,
      f64::from(opts.corner_radius),
      scale,
      &shadow_params,
    )? {
      let shadow_center_offset_x = f64::from(shadow.draw_x) + f64::from(shadow.pixmap.width()) / 2.0 - eff_card_w / 2.0;
      let shadow_center_offset_y = f64::from(shadow.draw_y) + f64::from(shadow.pixmap.height()) / 2.0 - eff_card_h / 2.0;
      let mut shadow_xform = transform_3d;
      shadow_xform.translate_x += shadow_center_offset_x;
      shadow_xform.translate_y += shadow_center_offset_y;
      composite_pixmap_transformed(&mut canvas, &shadow.pixmap, card_center_x, card_center_y, shadow_xform)?;
    }
  }

  // Render card contents into its own pixmap clipped to `outer_radius`
  let pad_border = if border_width > 0.0 {
    (border_width * scale).ceil() as u32
  } else {
    0
  };
  let full_card_w = card_w_u32 + pad_border * 2;
  let full_card_h = card_h_u32 + pad_border * 2;
  let mut full_card = Pixmap::new(full_card_w.max(1), full_card_h.max(1))
    .ok_or(CoreError::PixmapAllocation { width: full_card_w, height: full_card_h })?;

  let mut inner_card = Pixmap::new(card_w_u32, card_h_u32)
    .ok_or(CoreError::PixmapAllocation { width: card_w_u32, height: card_h_u32 })?;

  // Fill inset background if tinted
  let inset_rgba = parse_css_color(&inset_color).unwrap_or(Rgba::TRANSPARENT);
  if inset_rgba.a > 0.0 {
    inner_card.fill(inset_rgba.to_skia_color());
  }

  // Draw browser chrome at top of inner card
  let bar_px = if browser_bar.enabled {
    (browser_bar.height * scale).round() as f32
  } else {
    0.0
  };
  if browser_bar.enabled {
    draw_browser_chrome(
      &mut inner_card,
      eff_card_w as f32,
      &browser_bar,
      scale,
      opts.browser_url.as_deref(),
    );
  }

  // Draw screenshot image scaled inside inset inner container
  let img_area_y = f64::from(bar_px);
  let img_area_h = (eff_card_h - img_area_y).max(1.0);
  let inner_w = (layout.inset_inner_width * scale).max(1.0);
  let inner_h = (layout.inset_inner_height * scale).max(1.0);
  let inner_left = (eff_card_w - inner_w) / 2.0;
  let inner_top = img_area_y + (img_area_h - inner_h) / 2.0;

  let inner_radius = snap_to_device_pixel(
    compute_inset_content_radius_px(f64::from(opts.corner_radius), layout.inset_scale),
    scale,
  ) * scale;
  let top_inner_r = if browser_bar.enabled { 0.0 } else { inner_radius as f32 };
  let bot_inner_r = inner_radius as f32;

  if let Some(contain) = compute_object_contain_rect(
    inner_left,
    inner_top,
    inner_w,
    inner_h,
    f64::from(iw),
    f64::from(ih),
  ) {
    let src_pm = rgba_image_to_pixmap(&source_img)?;
    let mut img_paint = PixmapPaint::default();
    img_paint.quality = tiny_skia::FilterQuality::Bilinear;
    let sx = (contain.width / f64::from(iw)) as f32;
    let sy = (contain.height / f64::from(ih)) as f32;
    let img_xform = Transform::from_scale(sx, sy).post_translate(contain.left as f32, contain.top as f32);

    let mut inner_mask = None;
    if top_inner_r > 0.0 || bot_inner_r > 0.0 {
      if let Some(path) = build_rounded_rect_path(
        inner_left as f32,
        inner_top as f32,
        inner_w as f32,
        inner_h as f32,
        top_inner_r,
        top_inner_r,
        bot_inner_r,
        bot_inner_r,
      ) {
        let mut mask = Mask::new(card_w_u32, card_h_u32)
          .ok_or(CoreError::PixmapAllocation { width: card_w_u32, height: card_h_u32 })?;
        mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
        inner_mask = Some(mask);
      }
    }

    inner_card.draw_pixmap(0, 0, src_pm.as_ref(), &img_paint, img_xform, inner_mask.as_ref());
  }

  // Mask outer card with outer rounded radius and composite onto full_card
  let card_mask = if outer_radius > 0.0 {
    if let Some(path) = build_rounded_rect_path(
      0.0,
      0.0,
      eff_card_w as f32,
      eff_card_h as f32,
      outer_radius as f32,
      outer_radius as f32,
      outer_radius as f32,
      outer_radius as f32,
    ) {
      let mut mask = Mask::new(card_w_u32, card_h_u32)
        .ok_or(CoreError::PixmapAllocation { width: card_w_u32, height: card_h_u32 })?;
      mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
      Some(mask)
    } else {
      None
    }
  } else {
    None
  };

  full_card.draw_pixmap(
    pad_border as i32,
    pad_border as i32,
    inner_card.as_ref(),
    &PixmapPaint::default(),
    Transform::identity(),
    card_mask.as_ref(),
  );

  // Draw optional crisp border overlay outside card bounds
  if border_width > 0.0 {
    let bw = (border_width * scale) as f32;
    let b_col = parse_css_color(&border_color).unwrap_or(Rgba::WHITE).to_skia_color();
    let ring_r = (outer_radius as f32) + bw * 0.5;
    let inset_half = bw * 0.5;
    if let Some(path) = build_rounded_rect_path(
      inset_half,
      inset_half,
      (full_card_w as f32) - bw,
      (full_card_h as f32) - bw,
      ring_r,
      ring_r,
      ring_r,
      ring_r,
    ) {
      let mut stroke = Stroke::default();
      stroke.width = bw;
      let mut paint = Paint::default();
      paint.set_color(b_col);
      paint.anti_alias = true;
      full_card.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
  }

  // Composite transformed card onto final canvas
  composite_pixmap_transformed(&mut canvas, &full_card, card_center_x, card_center_y, transform_3d)?;

  let _ = ImgRgba([0u8; 4]);
  Ok(CompositionOutput {
    tree,
    image: pixmap_to_rgba_image(&canvas),
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn scene_composition_produces_expected_dimensions_and_card_pixels() {
    let screenshot = RgbaImage::from_pixel(320, 180, ImgRgba([45, 130, 240, 255]));
    let mut opts = EditorOptions::default();
    opts.aspect_mode = AspectMode::Auto;
    opts.padding_size = 24.0;
    opts.shadow = "Hug".to_string();
    opts.browser_bar = "mac-dark".to_string();
    opts.balance.enabled = false;

    let out = compose_screenshot_scene(
      &screenshot,
      None,
      &opts,
      400,
      1.0,
      RenderQuality::Final,
    )
    .expect("composition succeeds");

    assert_eq!(out.tree.canvas_width, 400);
    assert_eq!(out.image.width(), 400);
    // Center pixel should show the blue screenshot card
    let center = out.image.get_pixel(200, out.image.height() / 2);
    assert!(center[2] > 200);
  }

  #[test]
  fn safari_chrome_renders_address_bar_and_url_text() {
    let screenshot = RgbaImage::from_pixel(640, 360, ImgRgba([45, 130, 240, 255]));
    let mut base_opts = EditorOptions::default();
    base_opts.aspect_mode = AspectMode::Auto;
    base_opts.padding_size = 20.0;
    base_opts.balance.enabled = false;
    base_opts.browser_bar = "safari-dark".to_string();
    base_opts.browser_url = None;

    let mut url_opts = base_opts.clone();
    url_opts.browser_url = Some("https://shotify.dev/rust-edition".to_string());

    let no_url = compose_screenshot_scene(
      &screenshot,
      None,
      &base_opts,
      800,
      1.0,
      RenderQuality::Final,
    )
    .expect("safari chrome without url succeeds");

    let with_url = compose_screenshot_scene(
      &screenshot,
      None,
      &url_opts,
      800,
      1.0,
      RenderQuality::Final,
    )
    .expect("safari chrome with url succeeds");

    assert_eq!(no_url.image.dimensions(), with_url.image.dimensions());
    assert_ne!(no_url.image.as_raw(), with_url.image.as_raw());
  }
}
