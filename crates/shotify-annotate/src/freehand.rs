use std::f64::consts::PI;

use kurbo::{BezPath, Point};
use tiny_skia::PathBuilder;

/// Options controlling variable-width freehand stroke polygon generation,
/// modeled directly on `perfect-freehand`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FreehandOptions {
  pub size: f64,
  pub thinning: f64,
  pub smoothing: f64,
  pub streamline: f64,
  pub simulate_pressure: bool,
  pub start_cap: bool,
  pub end_cap: bool,
}

impl Default for FreehandOptions {
  fn default() -> Self {
    Self {
      size: 8.0,
      thinning: 0.5,
      smoothing: 0.5,
      streamline: 0.5,
      simulate_pressure: true,
      start_cap: true,
      end_cap: true,
    }
  }
}

#[derive(Debug, Clone, Copy)]
struct StrokeSample {
  point: Point,
  pressure: f64,
  dir: (f64, f64),
}

fn dist(a: Point, b: Point) -> f64 {
  a.distance(b)
}

fn unit_vector(dx: f64, dy: f64) -> (f64, f64) {
  let len = (dx * dx + dy * dy).sqrt();
  if len <= 1e-9 {
    (1.0, 0.0)
  } else {
    (dx / len, dy / len)
  }
}

fn perp((ux, uy): (f64, f64)) -> (f64, f64) {
  (-uy, ux)
}

fn rotate_point_around(center: Point, pt: Point, angle_rad: f64) -> Point {
  let (s, c) = angle_rad.sin_cos();
  let dx = pt.x - center.x;
  let dy = pt.y - center.y;
  Point::new(center.x + dx * c - dy * s, center.y + dx * s + dy * c)
}

fn ease_pressure(x: f64) -> f64 {
  x.sin()
}

fn stroke_radius(size: f64, thinning: f64, pressure: f64) -> f64 {
  let base = size * (0.5 - thinning * (0.5 - pressure));
  base.max(0.4)
}

fn build_stroke_samples(raw_points: &[(f64, f64, f64)], opts: FreehandOptions) -> Vec<StrokeSample> {
  if raw_points.is_empty() {
    return Vec::new();
  }

  let t = 0.15 + (1.0 - opts.streamline.clamp(0.0, 0.99)) * 0.85;
  let mut pts: Vec<(Point, f64)> = Vec::with_capacity(raw_points.len() + 2);

  let p0 = Point::new(raw_points[0].0, raw_points[0].1);
  pts.push((p0, raw_points[0].2));

  // Add an offset tail if 2+ identical points or keep smooth streamline interpolation.
  let mut prev = p0;
  let mut total_len = 0.0_f64;
  for &(rx, ry, rp) in raw_points.iter().skip(1) {
    let target = Point::new(rx, ry);
    let lerped = prev.lerp(target, t);
    let seg_len = dist(prev, lerped);
    if seg_len < 0.25 && raw_points.len() > 2 {
      continue;
    }
    total_len += seg_len;
    pts.push((lerped, rp));
    prev = lerped;
  }

  if pts.len() == 1 {
    let p = pts[0].0;
    pts.push((Point::new(p.x + 0.01, p.y + 0.01), pts[0].1));
  } else if pts.len() >= 2 && total_len < 0.1 {
    pts[1].0.x += 0.01;
    pts[1].0.y += 0.01;
  }

  let mut samples: Vec<StrokeSample> = Vec::with_capacity(pts.len());
  let mut prev_pressure = if opts.simulate_pressure { 0.5 } else { pts[0].1 };
  let rate_of_pressure_change = 0.275;

  for i in 0..pts.len() {
    let curr = pts[i].0;
    let next = if i + 1 < pts.len() { pts[i + 1].0 } else { curr };
    let prev_pt = if i > 0 { pts[i - 1].0 } else { curr };

    let step = if i > 0 { dist(prev_pt, curr) } else { 0.0 };

    let pressure = if opts.simulate_pressure {
      let sp = (step / opts.size.max(1.0)).min(1.0);
      let rp = (1.0 - sp).min(1.0);
      let next_p = (prev_pressure + ((rp - prev_pressure) * (sp * rate_of_pressure_change).clamp(0.08, 0.5))).clamp(0.0, 1.0);
      prev_pressure = next_p;
      ease_pressure(next_p * (PI * 0.5))
    } else {
      pts[i].1.clamp(0.0, 1.0)
    };

    let (dx, dy) = if i + 1 < pts.len() {
      (next.x - curr.x, next.y - curr.y)
    } else {
      (curr.x - prev_pt.x, curr.y - prev_pt.y)
    };
    let dir = unit_vector(dx, dy);

    samples.push(StrokeSample {
      point: curr,
      pressure,
      dir,
    });
  }

  samples
}

/// Generate a closed outline polygon (`[x, y]` in pixel coordinates) around a polyline,
/// matching `perfect-freehand` styling in `CanvasAnnotations.jsx`.
pub fn freehand_stroke_outline_points(
  raw_points: &[(f64, f64, f64)],
  opts: FreehandOptions,
) -> Vec<Point> {
  if raw_points.is_empty() {
    return Vec::new();
  }

  if raw_points.len() == 1 {
    let center = Point::new(raw_points[0].0, raw_points[0].1);
    let r = (opts.size * 0.5).max(0.5);
    let steps = 16;
    return (0..steps)
      .map(|i| {
        let a = (i as f64 / steps as f64) * 2.0 * PI;
        Point::new(center.x + r * a.cos(), center.y + r * a.sin())
      })
      .collect();
  }

  let samples = build_stroke_samples(raw_points, opts);
  if samples.is_empty() {
    return Vec::new();
  }

  let mut left_pts: Vec<Point> = Vec::with_capacity(samples.len());
  let mut right_pts: Vec<Point> = Vec::with_capacity(samples.len());

  for s in &samples {
    let r = stroke_radius(opts.size, opts.thinning, s.pressure);
    let (nx, ny) = perp(s.dir);
    let offset_x = nx * r;
    let offset_y = ny * r;
    left_pts.push(Point::new(s.point.x - offset_x, s.point.y - offset_y));
    right_pts.push(Point::new(s.point.x + offset_x, s.point.y + offset_y));
  }

  let first = samples[0];
  let last = samples[samples.len() - 1];

  let mut start_cap = Vec::new();
  if opts.start_cap {
    let right0 = right_pts[0];
    for step in (1..=12).map(|k| k as f64 / 13.0) {
      start_cap.push(rotate_point_around(first.point, right0, step * PI));
    }
  }

  let mut end_cap = Vec::new();
  if opts.end_cap {
    let left_last = left_pts[left_pts.len() - 1];
    for step in (1..=12).map(|k| k as f64 / 13.0) {
      end_cap.push(rotate_point_around(last.point, left_last, step * PI));
    }
  }

  let mut outline = Vec::with_capacity(left_pts.len() + right_pts.len() + start_cap.len() + end_cap.len());
  outline.extend(left_pts);
  outline.extend(end_cap);
  outline.extend(right_pts.into_iter().rev());
  outline.extend(start_cap);
  outline
}

/// Convert outline polygon points into a smooth quadratic `kurbo::BezPath`,
/// matching `getSvgPathFromStrokePoints` and `annotation_pen_outline_path`.
pub fn stroke_outline_to_bez_path(points: &[Point]) -> BezPath {
  let mut path = BezPath::new();
  if points.is_empty() {
    return path;
  }
  if points.len() == 1 {
    path.move_to(points[0]);
    path.line_to(points[0]);
    path.close_path();
    return path;
  }

  path.move_to(points[0]);
  for i in 0..points.len() {
    let a = points[i];
    let b = points[(i + 1) % points.len()];
    let mid = a.midpoint(b);
    path.quad_to(a, mid);
  }
  path.close_path();
  path
}

/// Convert a smoothed outline polygon directly into a closed `tiny_skia::Path`.
pub fn stroke_outline_to_skia_path(points: &[Point]) -> Option<tiny_skia::Path> {
  if points.len() < 2 {
    return None;
  }
  let mut pb = PathBuilder::new();
  pb.move_to(points[0].x as f32, points[0].y as f32);
  for i in 0..points.len() {
    let a = points[i];
    let b = points[(i + 1) % points.len()];
    let mid = a.midpoint(b);
    pb.quad_to(a.x as f32, a.y as f32, mid.x as f32, mid.y as f32);
  }
  pb.close();
  pb.finish()
}

/// Format an outline polygon into an SVG `d` path attribute matching `annotation_pen_outline_path`.
pub fn stroke_outline_to_svg_d(points: &[Point]) -> String {
  if points.is_empty() {
    return String::new();
  }
  if points.len() == 1 {
    let p = points[0];
    return format!("M {:.2},{:.2} L {:.2},{:.2} Z", p.x, p.y, p.x, p.y);
  }
  let mut d = format!("M {:.2},{:.2} ", points[0].x, points[0].y);
  for i in 0..points.len() {
    let a = points[i];
    let b = points[(i + 1) % points.len()];
    let mid = a.midpoint(b);
    d.push_str(&format!(
      "Q {:.2},{:.2} {:.2},{:.2} ",
      a.x, a.y, mid.x, mid.y
    ));
  }
  d.push('Z');
  d
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn freehand_outline_produces_closed_smooth_path() {
    let pts = vec![
      (10.0, 10.0, 0.5),
      (40.0, 22.0, 0.5),
      (80.0, 35.0, 0.5),
      (120.0, 30.0, 0.5),
    ];
    let outline = freehand_stroke_outline_points(&pts, FreehandOptions::default());
    assert!(outline.len() >= 20);
    assert!(stroke_outline_to_skia_path(&outline).is_some());
    let svg = stroke_outline_to_svg_d(&outline);
    assert!(svg.starts_with("M "));
    assert!(svg.ends_with('Z'));
  }

  #[test]
  fn freehand_single_point_renders_circular_dot_outline() {
    let outline = freehand_stroke_outline_points(&[(50.0, 60.0, 0.5)], FreehandOptions::default());
    assert_eq!(outline.len(), 16);
    assert!(stroke_outline_to_skia_path(&outline).is_some());
  }
}
