use crate::text_layout::text_bounds_px;
use crate::types::{
  AnnotationDocument, BoundsPx, ResizeHandle, SelectedAnnotation, StrokeAnnotation, StrokeKind,
};

/// Minimum squared distance from point `(px, py)` to line segment `(ax, ay) -> (bx, by)`.
pub fn dist_point_to_segment_sq(
  px: f64,
  py: f64,
  ax: f64,
  ay: f64,
  bx: f64,
  by: f64,
) -> f64 {
  let abx = bx - ax;
  let aby = by - ay;
  let apx = px - ax;
  let apy = py - ay;
  let ab_len_sq = abx * abx + aby * aby;
  if ab_len_sq <= 1e-6 {
    return apx * apx + apy * apy;
  }
  let t = ((apx * abx + apy * aby) / ab_len_sq).clamp(0.0, 1.0);
  let cx = ax + t * abx;
  let cy = ay + t * aby;
  let dx = px - cx;
  let dy = py - cy;
  dx * dx + dy * dy
}

/// Compute padded selection bounding box for a stroke in canvas pixel space.
pub fn stroke_bounds_px(doc_w: f64, doc_h: f64, stroke: &StrokeAnnotation) -> Option<BoundsPx> {
  if stroke.points.is_empty() {
    return None;
  }
  let mut min_x = f64::INFINITY;
  let mut min_y = f64::INFINITY;
  let mut max_x = f64::NEG_INFINITY;
  let mut max_y = f64::NEG_INFINITY;

  for p in &stroke.points {
    let x = p.x * doc_w;
    let y = p.y * doc_h;
    min_x = min_x.min(x);
    min_y = min_y.min(y);
    max_x = max_x.max(x);
    max_y = max_y.max(y);
  }

  let min_dim = doc_w.min(doc_h).max(1.0);
  let line_w = (stroke.width_rel * min_dim).round().max(1.0);
  let pad = ((line_w / 2.0).round() + 8.0).max(10.0);

  let x0 = (min_x - pad).max(0.0).round();
  let y0 = (min_y - pad).max(0.0).round();
  let x1 = (max_x + pad).min(doc_w).round();
  let y1 = (max_y + pad).min(doc_h).round();

  Some(BoundsPx {
    x: x0,
    y: y0,
    width: (x1 - x0).max(1.0),
    height: (y1 - y0).max(1.0),
  })
}

/// Hit-test a single stroke at pixel coordinate `(px, py)` matching `CanvasAnnotations.jsx`.
pub fn is_stroke_hit(
  doc_w: f64,
  doc_h: f64,
  stroke: &StrokeAnnotation,
  px: f64,
  py: f64,
  extra_radius_px: f64,
) -> bool {
  let pts = &stroke.points;
  if pts.is_empty() {
    return false;
  }

  let min_dim = doc_w.min(doc_h).max(1.0);
  let line_w = (stroke.width_rel * min_dim).round().max(1.0);
  let thresh = ((line_w / 2.0).round() + 8.0 + extra_radius_px.max(0.0)).max(8.0);
  let thresh_sq = thresh * thresh;

  if matches!(stroke.kind, StrokeKind::Rect | StrokeKind::Ellipse) && pts.len() >= 2 {
    let p0 = pts[0];
    let p1 = pts[pts.len() - 1];
    let x1 = p0.x * doc_w;
    let y1 = p0.y * doc_h;
    let x2 = p1.x * doc_w;
    let y2 = p1.y * doc_h;
    let left = x1.min(x2);
    let top = y1.min(y2);
    let width = (x2 - x1).abs().max(1.0);
    let height = (y2 - y1).abs().max(1.0);
    let right = left + width;
    let bottom = top + height;

    if stroke.kind == StrokeKind::Rect {
      if px < left - thresh || px > right + thresh || py < top - thresh || py > bottom + thresh {
        return false;
      }
      if px >= left && px <= right && py >= top && py <= bottom {
        return true;
      }
      return [
        dist_point_to_segment_sq(px, py, left, top, right, top),
        dist_point_to_segment_sq(px, py, right, top, right, bottom),
        dist_point_to_segment_sq(px, py, right, bottom, left, bottom),
        dist_point_to_segment_sq(px, py, left, bottom, left, top),
      ]
      .into_iter()
      .any(|d_sq| d_sq <= thresh_sq);
    }

    // Ellipse (interior click also selects)
    let cx = left + width / 2.0;
    let cy = top + height / 2.0;
    let rx = (width / 2.0).max(1.0);
    let ry = (height / 2.0).max(1.0);
    let dx = px - cx;
    let dy = py - cy;
    let norm = (dx * dx) / (rx * rx) + (dy * dy) / (ry * ry);
    let edge_band = thresh / rx.min(ry).max(1.0);
    return norm <= (1.0 + edge_band).powi(2);
  }

  let ax0 = pts[0].x * doc_w;
  let ay0 = pts[0].y * doc_h;
  if pts.len() == 1 {
    let dx = px - ax0;
    let dy = py - ay0;
    return dx * dx + dy * dy <= thresh_sq;
  }

  for win in pts.windows(2) {
    let ax = win[0].x * doc_w;
    let ay = win[0].y * doc_h;
    let bx = win[1].x * doc_w;
    let by = win[1].y * doc_h;
    if dist_point_to_segment_sq(px, py, ax, ay, bx, by) <= thresh_sq {
      return true;
    }
  }
  false
}

/// Top-down hit test returning the topmost text box or stroke under `(px, py)`.
pub fn hit_test_document(
  doc: &AnnotationDocument,
  doc_w: f64,
  doc_h: f64,
  px: f64,
  py: f64,
) -> Option<SelectedAnnotation> {
  for (i, t) in doc.texts.iter().enumerate().rev() {
    let box_px = text_bounds_px(doc_w, doc_h, t);
    if box_px.contains(px, py) {
      return Some(SelectedAnnotation::Text(i));
    }
  }
  for (i, s) in doc.strokes.iter().enumerate().rev() {
    if is_stroke_hit(doc_w, doc_h, s, px, py, 0.0) {
      return Some(SelectedAnnotation::Stroke(i));
    }
  }
  None
}

/// Hit-test the 8 resize handles around `bounds` within `radius_px`.
pub fn hit_test_resize_handle(
  bounds: BoundsPx,
  px: f64,
  py: f64,
  radius_px: f64,
) -> Option<ResizeHandle> {
  let r_sq = radius_px * radius_px;
  ResizeHandle::ALL.into_iter().find(|&handle| {
    let (hx, hy) = bounds.handle_center(handle);
    let dx = px - hx;
    let dy = py - hy;
    dx * dx + dy * dy <= r_sq
  })
}

/// Compute the selection bounding box for either a selected text or stroke.
pub fn selection_bounds_px(
  doc: &AnnotationDocument,
  doc_w: f64,
  doc_h: f64,
  selected: SelectedAnnotation,
) -> Option<BoundsPx> {
  match selected {
    SelectedAnnotation::Text(i) => doc.texts.get(i).map(|t| text_bounds_px(doc_w, doc_h, t)),
    SelectedAnnotation::Stroke(i) => doc
      .strokes
      .get(i)
      .and_then(|s| stroke_bounds_px(doc_w, doc_h, s)),
  }
}
