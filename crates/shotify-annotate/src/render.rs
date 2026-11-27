use std::f64::consts::PI;

use ab_glyph::{Font, OutlineCurve, PxScale, ScaleFont};
use image::RgbaImage;
use shotify_core::{
  build_rounded_rect_path, parse_css_color, pixmap_to_rgba_image, rgba_image_to_pixmap, Rgba,
};
use tiny_skia::{
  Color, FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, Stroke, Transform,
};

use crate::error::{AnnotateError, Result};
use crate::freehand::{
  freehand_stroke_outline_points, stroke_outline_to_skia_path, FreehandOptions,
};
use crate::text_layout::{annotation_font, compute_text_layout};
use crate::types::{AnnotationDocument, StrokeAnnotation, StrokeKind, TextAnnotation};

fn parse_color_with_alpha(css: &str, alpha_mul: f64, fallback: Rgba) -> Color {
  let base = parse_css_color(css).unwrap_or(fallback);
  Rgba::new(
    base.r,
    base.g,
    base.b,
    (base.a * (alpha_mul as f32)).clamp(0.0, 1.0),
  )
  .to_skia_color()
}

fn build_ellipse_path(cx: f32, cy: f32, rx: f32, ry: f32) -> Option<Path> {
  if rx <= 0.1 || ry <= 0.1 {
    return None;
  }
  const KAPPA: f32 = 0.552_284_8;
  let ox = rx * KAPPA;
  let oy = ry * KAPPA;
  let mut pb = PathBuilder::new();
  pb.move_to(cx - rx, cy);
  pb.cubic_to(cx - rx, cy - oy, cx - ox, cy - ry, cx, cy - ry);
  pb.cubic_to(cx + ox, cy - ry, cx + rx, cy - oy, cx + rx, cy);
  pb.cubic_to(cx + rx, cy + oy, cx + ox, cy + ry, cx, cy + ry);
  pb.cubic_to(cx - ox, cy + ry, cx - rx, cy + oy, cx - rx, cy);
  pb.close();
  pb.finish()
}

fn build_concave_arrowhead(
  x1: f64,
  y1: f64,
  x2: f64,
  y2: f64,
  head_length: f64,
) -> Option<Path> {
  let angle = (y2 - y1).atan2(x2 - x1);
  let head_angle = PI / 5.4;
  let nock_depth = head_length * 0.25;

  let left_x = x2 - head_length * (angle - head_angle).cos();
  let left_y = y2 - head_length * (angle - head_angle).sin();
  let right_x = x2 - head_length * (angle + head_angle).cos();
  let right_y = y2 - head_length * (angle + head_angle).sin();
  let nock_x = x2 - (head_length - nock_depth) * angle.cos();
  let nock_y = y2 - (head_length - nock_depth) * angle.sin();

  let mut pb = PathBuilder::new();
  pb.move_to(x2 as f32, y2 as f32);
  pb.line_to(left_x as f32, left_y as f32);
  pb.line_to(nock_x as f32, nock_y as f32);
  pb.line_to(right_x as f32, right_y as f32);
  pb.close();
  pb.finish()
}

fn render_stroke(pixmap: &mut Pixmap, stroke: &StrokeAnnotation, doc_w: f64, doc_h: f64) {
  if stroke.points.is_empty() {
    return;
  }
  let min_dim = doc_w.min(doc_h).max(1.0);
  let line_width = (stroke.width_rel * min_dim).round().max(1.0);
  let color = parse_color_with_alpha(&stroke.color, stroke.alpha, Rgba::WHITE);

  match stroke.kind {
    StrokeKind::Arrow => {
      if stroke.points.len() < 2 {
        return;
      }
      let p0 = stroke.points[0];
      let p1 = stroke.points[stroke.points.len() - 1];
      let x1 = p0.x * doc_w;
      let y1 = p0.y * doc_h;
      let x2 = p1.x * doc_w;
      let y2 = p1.y * doc_h;
      if (x2 - x1).hypot(y2 - y1) < 1.0 {
        return;
      }

      let head_length = (line_width * 10.0).max(40.0 * (min_dim / 600.0).min(1.0));
      let angle = (y2 - y1).atan2(x2 - x1);
      let shaft_end_x = x2 - (head_length * 0.62) * angle.cos();
      let shaft_end_y = y2 - (head_length * 0.62) * angle.sin();

      let outline_width = (line_width * 0.55).max(3.0);
      let halo_color =
        parse_color_with_alpha("rgba(255,255,255,0.92)", stroke.alpha, Rgba::WHITE);

      let mut shaft_pb = PathBuilder::new();
      shaft_pb.move_to(x1 as f32, y1 as f32);
      shaft_pb.line_to(shaft_end_x as f32, shaft_end_y as f32);
      let Some(shaft_path) = shaft_pb.finish() else {
        return;
      };
      let arrowhead_path = build_concave_arrowhead(x1, y1, x2, y2, head_length);

      // White halo pass behind colored shaft & arrowhead
      let mut halo_paint = Paint::default();
      halo_paint.set_color(halo_color);
      halo_paint.anti_alias = true;

      let halo_shaft_stroke = Stroke {
        width: (line_width + outline_width * 2.0) as f32,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
      };
      pixmap.stroke_path(
        &shaft_path,
        &halo_paint,
        &halo_shaft_stroke,
        Transform::identity(),
        None,
      );

      if let Some(ref head_path) = arrowhead_path {
        let halo_head_stroke = Stroke {
          width: (outline_width * 2.0) as f32,
          line_join: LineJoin::Round,
          ..Default::default()
        };
        pixmap.stroke_path(
          head_path,
          &halo_paint,
          &halo_head_stroke,
          Transform::identity(),
          None,
        );
      }

      // Colored fill pass on top
      let mut fill_paint = Paint::default();
      fill_paint.set_color(color);
      fill_paint.anti_alias = true;

      let shaft_stroke = Stroke {
        width: line_width as f32,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
      };
      pixmap.stroke_path(
        &shaft_path,
        &fill_paint,
        &shaft_stroke,
        Transform::identity(),
        None,
      );

      if let Some(ref head_path) = arrowhead_path {
        pixmap.fill_path(
          head_path,
          &fill_paint,
          FillRule::Winding,
          Transform::identity(),
          None,
        );
      }
    }
    StrokeKind::Rect => {
      if stroke.points.len() < 2 {
        return;
      }
      let p0 = stroke.points[0];
      let p1 = stroke.points[stroke.points.len() - 1];
      let x1 = p0.x * doc_w;
      let y1 = p0.y * doc_h;
      let x2 = p1.x * doc_w;
      let y2 = p1.y * doc_h;
      let rx = x1.min(x2) as f32;
      let ry = y1.min(y2) as f32;
      let rw = (x2 - x1).abs() as f32;
      let rh = (y2 - y1).abs() as f32;
      if rw < 0.5 || rh < 0.5 {
        return;
      }
      let corner_r = ((line_width as f32) * 1.5).min(rw / 3.0).min(rh / 3.0).max(0.0);
      if let Some(path) = build_rounded_rect_path(rx, ry, rw, rh, corner_r, corner_r, corner_r, corner_r) {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        let sk_stroke = Stroke {
          width: line_width as f32,
          line_join: LineJoin::Round,
          ..Default::default()
        };
        pixmap.stroke_path(&path, &paint, &sk_stroke, Transform::identity(), None);
      }
    }
    StrokeKind::Ellipse => {
      if stroke.points.len() < 2 {
        return;
      }
      let p0 = stroke.points[0];
      let p1 = stroke.points[stroke.points.len() - 1];
      let x1 = p0.x * doc_w;
      let y1 = p0.y * doc_h;
      let x2 = p1.x * doc_w;
      let y2 = p1.y * doc_h;
      let cx = ((x1 + x2) / 2.0) as f32;
      let cy = ((y1 + y2) / 2.0) as f32;
      let rx = ((x2 - x1).abs() / 2.0).max(1.0) as f32;
      let ry = ((y2 - y1).abs() / 2.0).max(1.0) as f32;
      if let Some(path) = build_ellipse_path(cx, cy, rx, ry) {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        let sk_stroke = Stroke {
          width: line_width as f32,
          ..Default::default()
        };
        pixmap.stroke_path(&path, &paint, &sk_stroke, Transform::identity(), None);
      }
    }
    StrokeKind::Pen => {
      let mut paint = Paint::default();
      paint.set_color(color);
      paint.anti_alias = true;

      if stroke.points.len() == 1 {
        let cx = (stroke.points[0].x * doc_w) as f32;
        let cy = (stroke.points[0].y * doc_h) as f32;
        let r = ((line_width / 2.0) as f32).max(0.5);
        if let Some(circle) = PathBuilder::from_circle(cx, cy, r) {
          pixmap.fill_path(&circle, &paint, FillRule::Winding, Transform::identity(), None);
        }
        return;
      }

      let raw: Vec<(f64, f64, f64)> = stroke
        .points
        .iter()
        .map(|p| (p.x * doc_w, p.y * doc_h, p.pressure))
        .collect();
      let opts = FreehandOptions {
        size: line_width * 2.0,
        thinning: 0.5,
        smoothing: 0.5,
        streamline: 0.5,
        simulate_pressure: true,
        start_cap: true,
        end_cap: true,
      };
      let outline = freehand_stroke_outline_points(&raw, opts);
      if let Some(path) = stroke_outline_to_skia_path(&outline) {
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
      }
    }
  }
}

fn push_curve_contour(
  pb: &mut PathBuilder,
  last_end: &mut Option<(f32, f32)>,
  xform_pt: impl Fn(ab_glyph::Point) -> (f32, f32),
  curve: OutlineCurve,
) {
  let eps = 0.02_f32;
  let same_pt = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() <= eps && (a.1 - b.1).abs() <= eps;

  match curve {
    OutlineCurve::Line(p0, p1) => {
      let s = xform_pt(p0);
      let e = xform_pt(p1);
      if !last_end.is_some_and(|prev| same_pt(prev, s)) {
        if last_end.is_some() {
          pb.close();
        }
        pb.move_to(s.0, s.1);
      }
      pb.line_to(e.0, e.1);
      *last_end = Some(e);
    }
    OutlineCurve::Quad(p0, p1, p2) => {
      let s = xform_pt(p0);
      let c = xform_pt(p1);
      let e = xform_pt(p2);
      if !last_end.is_some_and(|prev| same_pt(prev, s)) {
        if last_end.is_some() {
          pb.close();
        }
        pb.move_to(s.0, s.1);
      }
      pb.quad_to(c.0, c.1, e.0, e.1);
      *last_end = Some(e);
    }
    OutlineCurve::Cubic(p0, p1, p2, p3) => {
      let s = xform_pt(p0);
      let c1 = xform_pt(p1);
      let c2 = xform_pt(p2);
      let e = xform_pt(p3);
      if !last_end.is_some_and(|prev| same_pt(prev, s)) {
        if last_end.is_some() {
          pb.close();
        }
        pb.move_to(s.0, s.1);
      }
      pb.cubic_to(c1.0, c1.1, c2.0, c2.1, e.0, e.1);
      *last_end = Some(e);
    }
  }
}

fn build_text_line_vector_path(line: &str, origin_x: f32, top_y: f32, font_size: f32) -> Option<Path> {
  let font = annotation_font();
  let px_scale = PxScale::from(font_size.max(1.0));
  let scaled = font.as_scaled(px_scale);
  let ascent = scaled.ascent();
  let baseline_y = top_y + ascent;
  let unscaled_h = font.height_unscaled().max(1.0);
  let h_factor = px_scale.x / unscaled_h;
  let v_factor = -px_scale.y / unscaled_h;

  let mut pb = PathBuilder::new();
  let mut cursor_x = origin_x;
  let mut prev = None;
  let mut has_segments = false;

  for ch in line.chars() {
    let glyph_id = font.glyph_id(ch);
    if let Some(p) = prev {
      cursor_x += scaled.kern(p, glyph_id);
    }

    if let Some(outline) = font.outline(glyph_id) {
      let mut last_end = None;
      let gx = cursor_x;
      let gy = baseline_y;
      for curve in outline.curves {
        has_segments = true;
        push_curve_contour(
          &mut pb,
          &mut last_end,
          |pt| (gx + pt.x * h_factor, gy + pt.y * v_factor),
          curve,
        );
      }
      if last_end.is_some() {
        pb.close();
      }
    }

    cursor_x += scaled.h_advance(glyph_id);
    prev = Some(glyph_id);
  }

  if !has_segments {
    return None;
  }
  pb.finish()
}

fn render_text_annotation(pixmap: &mut Pixmap, t: &TextAnnotation, doc_w: f64, doc_h: f64) {
  let layout = compute_text_layout(doc_w, doc_h, t);
  let x = (t.x_norm * doc_w).round() as f32;
  let y = (t.y_norm * doc_h).round() as f32;
  let box_w = layout.box_w as f32;
  let box_h = layout.box_h as f32;
  let radius = (t.radius_rel * layout.min_dim_px).round() as f32;
  let r = radius.clamp(0.0, (box_w / 2.0).min(box_h / 2.0));

  let bg_col = parse_color_with_alpha(&t.bg_color, 1.0, Rgba::BLACK);
  let fg_col = parse_color_with_alpha(&t.text_color, 1.0, Rgba::WHITE);
  let stroke_col = if t.force_circle {
    parse_color_with_alpha("rgba(0,0,0,0.75)", 1.0, Rgba::BLACK)
  } else {
    parse_color_with_alpha(&t.text_stroke_color, 1.0, Rgba::BLACK)
  };

  if let Some(bg_path) = build_rounded_rect_path(x, y, box_w, box_h, r, r, r, r) {
    let mut bg_paint = Paint::default();
    bg_paint.set_color(bg_col);
    bg_paint.anti_alias = true;
    pixmap.fill_path(
      &bg_path,
      &bg_paint,
      FillRule::Winding,
      Transform::identity(),
      None,
    );
  }

  let effective_stroke_w = if t.force_circle {
    (layout.font_size * 0.22).round().max(2.0)
  } else {
    (t.stroke_width_rel * layout.min_dim_px).round().max(2.0)
  };

  let mut stroke_paint = Paint::default();
  stroke_paint.set_color(stroke_col);
  stroke_paint.anti_alias = true;
  let glyph_stroke = Stroke {
    width: effective_stroke_w as f32,
    line_join: LineJoin::Round,
    line_cap: LineCap::Round,
    miter_limit: 2.0,
    ..Default::default()
  };

  let mut fill_paint = Paint::default();
  fill_paint.set_color(fg_col);
  fill_paint.anti_alias = true;

  for (i, line) in layout.lines.iter().enumerate() {
    if line.is_empty() {
      continue;
    }
    let line_w = crate::text_layout::measure_text_width(line, layout.font_size);
    let tx = if t.force_circle {
      x + (box_w - (line_w as f32)) / 2.0
    } else {
      x + (layout.pad_x as f32)
    };
    let ty = if t.force_circle {
      y + ((box_h - (layout.lines.len() as f32) * (layout.line_h as f32)) / 2.0)
        + (i as f32) * (layout.line_h as f32)
    } else {
      y + (layout.pad_y as f32) + (i as f32) * (layout.line_h as f32)
    };

    if let Some(glyph_path) = build_text_line_vector_path(line, tx, ty, layout.font_size as f32) {
      pixmap.stroke_path(
        &glyph_path,
        &stroke_paint,
        &glyph_stroke,
        Transform::identity(),
        None,
      );
      pixmap.fill_path(
        &glyph_path,
        &fill_paint,
        FillRule::EvenOdd,
        Transform::identity(),
        None,
      );
    }
  }
}

/// Composite all annotations in `doc` directly onto an existing premultiplied `Pixmap`.
pub fn render_annotations_onto_pixmap(pixmap: &mut Pixmap, doc: &AnnotationDocument) -> Result<()> {
  let w = f64::from(pixmap.width());
  let h = f64::from(pixmap.height());
  if w < 1.0 || h < 1.0 {
    return Err(AnnotateError::InvalidDimensions {
      width: pixmap.width(),
      height: pixmap.height(),
    });
  }
  for stroke in &doc.strokes {
    render_stroke(pixmap, stroke, w, h);
  }
  for text in &doc.texts {
    render_text_annotation(pixmap, text, w, h);
  }
  Ok(())
}

/// Composite all annotations in `doc` directly onto a straight RGBA `RgbaImage`.
pub fn render_annotations_onto_rgba(image: &mut RgbaImage, doc: &AnnotationDocument) -> Result<()> {
  if doc.is_empty() {
    return Ok(());
  }
  let mut pixmap = rgba_image_to_pixmap(image)?;
  render_annotations_onto_pixmap(&mut pixmap, doc)?;
  *image = pixmap_to_rgba_image(&pixmap);
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::engine::AnnotationEngine;
  use crate::types::AnnotationTool;

  #[test]
  fn renders_pen_arrow_rect_and_step_badge_to_rgba() {
    let mut engine = AnnotationEngine::new();
    engine.active_tool = Some(AnnotationTool::Arrow);
    engine.pointer_down(40.0, 40.0, 400.0, 300.0);
    engine.pointer_move(220.0, 140.0, 400.0, 300.0);
    engine.pointer_up();

    engine.active_tool = Some(AnnotationTool::StepBadge);
    engine.place_step_badge(280.0, 200.0, 400.0, 300.0);

    engine.commit_text("Click here", 0.1, 0.7, 400.0, 300.0, None);

    let mut img = RgbaImage::from_pixel(400, 300, image::Rgba([20, 20, 30, 255]));
    render_annotations_onto_rgba(&mut img, &engine.document).expect("renders annotations cleanly");

    // Verify rendered pixels changed significantly around badge and text areas
    let bg = image::Rgba([20, 20, 30, 255]);
    let changed = img.pixels().filter(|&&p| p != bg).count();
    assert!(changed > 500, "expected rendered annotation pixels, got {changed}");
  }
}
