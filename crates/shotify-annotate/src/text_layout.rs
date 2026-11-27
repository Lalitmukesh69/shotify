use std::sync::OnceLock;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};

use crate::types::{BoundsPx, TextAnnotation};

static FONT_BYTES: &[u8] = include_bytes!("../../../assets/fonts/Cantarell-ExtraBold.otf");
static FONT_INSTANCE: OnceLock<FontRef<'static>> = OnceLock::new();

/// Shared embedded ExtraBold OpenType font used for annotation text boxes and step badges.
pub fn annotation_font() -> &'static FontRef<'static> {
  FONT_INSTANCE.get_or_init(|| {
    FontRef::try_from_slice(FONT_BYTES).expect("embedded Cantarell-ExtraBold.otf is valid")
  })
}

/// Measured pixel layout for a `TextAnnotation` or circular step badge.
#[derive(Debug, Clone, PartialEq)]
pub struct ComputedTextLayout {
  pub lines: Vec<String>,
  pub box_w: f64,
  pub box_h: f64,
  pub font_size: f64,
  pub line_h: f64,
  pub pad_x: f64,
  pub pad_y: f64,
  pub min_dim_px: f64,
}

/// Measure single-line text width using `ab_glyph` scaled metrics (including kerning).
pub fn measure_text_width(text: &str, font_size: f64) -> f64 {
  let font = annotation_font();
  let scaled = font.as_scaled(PxScale::from(font_size.max(1.0) as f32));
  let mut width = 0.0_f32;
  let mut prev = None;
  for ch in text.chars() {
    let glyph_id = font.glyph_id(ch);
    if let Some(p) = prev {
      width += scaled.kern(p, glyph_id);
    }
    width += scaled.h_advance(glyph_id);
    prev = Some(glyph_id);
  }
  f64::from(width.max(0.0))
}

fn wrap_single_line(text: &str, font_size: f64, max_w: Option<f64>) -> Vec<String> {
  let Some(limit) = max_w else {
    return vec![text.to_string()];
  };
  if limit <= 1.0 || text.is_empty() {
    return vec![text.to_string()];
  }

  let mut lines = Vec::new();
  let mut current = String::new();
  for word in text.split_inclusive(char::is_whitespace) {
    let candidate = if current.is_empty() {
      word.to_string()
    } else {
      format!("{current}{word}")
    };
    if !current.is_empty() && measure_text_width(candidate.trim_end(), font_size) > limit {
      lines.push(current.trim_end().to_string());
      current = word.trim_start().to_string();
    } else {
      current = candidate;
    }
  }
  if !current.is_empty() || lines.is_empty() {
    lines.push(current);
  }
  lines
}

/// Compute layout geometry for a `TextAnnotation` matching `CanvasAnnotations.jsx::computeTextLayout`.
pub fn compute_text_layout(doc_w: f64, doc_h: f64, t: &TextAnnotation) -> ComputedTextLayout {
  let canvas_w = doc_w.max(1.0);
  let min_dim_px = doc_w.min(doc_h).max(1.0);

  let font_size = (t.font_size_rel * min_dim_px).round().max(10.0);
  let line_h = (font_size * 1.12).round().max(11.0);
  let pad_x = (t.pad_x_rel * min_dim_px).round();
  let pad_y = (t.pad_y_rel * min_dim_px).round();

  let box_w_from_user = t
    .box_w_norm
    .filter(|w| w.is_finite() && *w > 0.0)
    .map(|w| (w * canvas_w).round().max(60.0));

  let content_max_w = box_w_from_user.map(|w| (w - pad_x * 2.0).max(20.0));

  let mut lines = Vec::new();
  for hl in t.value.split('\n') {
    let clean = hl.trim_end_matches('\r');
    lines.extend(wrap_single_line(clean, font_size, content_max_w));
  }
  if lines.is_empty() {
    lines.push(String::new());
  }

  let mut max_line_w = 0.0_f64;
  for line in &lines {
    max_line_w = max_line_w.max(measure_text_width(line, font_size));
  }

  let computed_box_w = (max_line_w + pad_x * 2.0).round();
  let mut box_w = box_w_from_user
    .map(|w| w.min(canvas_w))
    .unwrap_or(computed_box_w);
  let mut box_h = (lines.len() as f64 * line_h + pad_y * 2.0).round();

  let mut adj_pad_x = pad_x;
  let mut adj_pad_y = pad_y;
  if t.force_circle {
    let dim = box_w.max(box_h);
    adj_pad_x = ((dim - max_line_w) / 2.0).round();
    adj_pad_y = ((dim - lines.len() as f64 * line_h) / 2.0).round();
    box_w = dim;
    box_h = dim;
  }

  ComputedTextLayout {
    lines,
    box_w: box_w.max(1.0),
    box_h: box_h.max(1.0),
    font_size,
    line_h,
    pad_x: adj_pad_x,
    pad_y: adj_pad_y,
    min_dim_px,
  }
}

/// Compute pixel bounding box for a `TextAnnotation`.
pub fn text_bounds_px(doc_w: f64, doc_h: f64, t: &TextAnnotation) -> BoundsPx {
  let layout = compute_text_layout(doc_w, doc_h, t);
  BoundsPx {
    x: (t.x_norm * doc_w).round(),
    y: (t.y_norm * doc_h).round(),
    width: layout.box_w,
    height: layout.box_h,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn step_badge_forces_square_box() {
    let badge = TextAnnotation {
      id: 1,
      x_norm: 0.2,
      y_norm: 0.3,
      value: "12".to_string(),
      box_w_norm: Some(58.0 / 800.0),
      font_size_rel: 22.0 / 600.0,
      pad_x_rel: 14.0 / 600.0,
      pad_y_rel: 14.0 / 600.0,
      radius_rel: 999.0 / 600.0,
      stroke_width_rel: 0.0,
      bg_color: "#7c3aed".to_string(),
      text_color: "#ffffff".to_string(),
      text_stroke_color: "rgba(0,0,0,0)".to_string(),
      force_circle: true,
    };

    let layout = compute_text_layout(800.0, 600.0, &badge);
    assert!((layout.box_w - layout.box_h).abs() < 1e-6);
    assert!(layout.box_w >= 58.0);
  }
}
