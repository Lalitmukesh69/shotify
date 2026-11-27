use serde::{Deserialize, Serialize};

/// Clamp `value` between `min` and `max` (inclusive).
#[inline]
pub fn clamp_f64(value: f64, min: f64, max: f64) -> f64 {
  if !value.is_finite() {
    return min;
  }
  value.clamp(min, max)
}

/// Clamp `value` between `min` and `max` (inclusive).
#[inline]
pub fn clamp_f32(value: f32, min: f32, max: f32) -> f32 {
  if !value.is_finite() {
    return min;
  }
  value.clamp(min, max)
}

/// Linearly interpolate between `a` and `b` using factor `t`.
#[inline]
pub fn lerp_f64(a: f64, b: f64, t: f64) -> f64 {
  a + (b - a) * t
}

/// Linearly interpolate between `a` and `b` using factor `t`.
#[inline]
pub fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
  a + (b - a) * t
}

/// Normalize `-0.0` to `0.0` to avoid UI/equality surprises.
#[inline]
pub fn normalize_zero(v: f64) -> f64 {
  if v == 0.0 {
    0.0
  } else {
    v
  }
}

/// Snap a scalar coordinate to half-pixel increments.
#[inline]
pub fn snap_to_half_pixel(value: f64) -> f64 {
  if !value.is_finite() {
    return 0.0;
  }
  normalize_zero((value * 2.0).round() / 2.0)
}

/// Snap a scalar coordinate to the active device pixel grid.
#[inline]
pub fn snap_to_device_pixel(value: f64, device_pixel_ratio: f64) -> f64 {
  if !value.is_finite() {
    return 0.0;
  }
  let s = if device_pixel_ratio.is_finite() {
    device_pixel_ratio.max(1.0)
  } else {
    1.0
  };
  normalize_zero((value * s).round() / s)
}

/// Convert degrees to normalized unit direction vector `(cos, sin)`.
/// Canvas angle convention: 0 deg = +X (right), 90 deg = +Y (down).
#[inline]
pub fn parse_angle_to_unit_vector(angle_deg: f64) -> (f64, f64) {
  let a = if angle_deg.is_finite() { angle_deg } else { 0.0 };
  let rad = a.to_radians();
  let x = rad.cos();
  let y = rad.sin();
  let len = x.hypot(y);
  if len > 0.0 {
    (x / len, y / len)
  } else {
    (1.0, 0.0)
  }
}

/// 2D translation offset returned by drag clamping.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2D {
  pub x: f64,
  pub y: f64,
}

/// Container and card geometry bounds used to constrain screenshot dragging.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DragContainerBounds {
  pub container_width: f64,
  pub container_height: f64,
  pub card_width: f64,
  pub card_height: f64,
  #[serde(default)]
  pub overflow_x: f64,
  #[serde(default)]
  pub overflow_y: f64,
}

/// Port of `utils/core/dragBounds.js::clampTransformToContainer`.
pub fn clamp_transform_to_container(transform: Point2D, bounds: DragContainerBounds) -> Point2D {
  let x = if transform.x.is_finite() { transform.x } else { 0.0 };
  let y = if transform.y.is_finite() { transform.y } else { 0.0 };

  let cw = if bounds.container_width.is_finite() { bounds.container_width } else { 0.0 };
  let ch = if bounds.container_height.is_finite() { bounds.container_height } else { 0.0 };
  let iw = if bounds.card_width.is_finite() { bounds.card_width } else { 0.0 };
  let ih = if bounds.card_height.is_finite() { bounds.card_height } else { 0.0 };

  if cw <= 0.0 || ch <= 0.0 || iw <= 0.0 || ih <= 0.0 {
    return Point2D { x, y };
  }

  let extra_x = if bounds.overflow_x.is_finite() { bounds.overflow_x.max(0.0) } else { 0.0 };
  let extra_y = if bounds.overflow_y.is_finite() { bounds.overflow_y.max(0.0) } else { 0.0 };

  let max_x = ((cw - iw) / 2.0 + extra_x).max(0.0);
  let max_y = ((ch - ih) / 2.0 + extra_y).max(0.0);

  let clamped_x = x.clamp(-max_x, max_x);
  let clamped_y = y.clamp(-max_y, max_y);

  Point2D {
    x: normalize_zero(clamped_x),
    y: normalize_zero(clamped_y),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn clamp_transform_centers_when_card_larger_than_container() {
    let out = clamp_transform_to_container(
      Point2D { x: 42.0, y: -18.0 },
      DragContainerBounds {
        container_width: 400.0,
        container_height: 300.0,
        card_width: 450.0,
        card_height: 350.0,
        overflow_x: 0.0,
        overflow_y: 0.0,
      },
    );
    assert_eq!(out, Point2D { x: 0.0, y: 0.0 });
    assert!(!out.x.is_sign_negative());
  }

  #[test]
  fn clamp_transform_respects_overflow() {
    let out = clamp_transform_to_container(
      Point2D { x: 200.0, y: -200.0 },
      DragContainerBounds {
        container_width: 500.0,
        container_height: 400.0,
        card_width: 300.0,
        card_height: 200.0,
        overflow_x: 20.0,
        overflow_y: 10.0,
      },
    );
    assert_eq!(out, Point2D { x: 120.0, y: -110.0 });
  }

  #[test]
  fn snap_half_and_device_pixel() {
    assert_eq!(snap_to_half_pixel(10.26), 10.5);
    assert_eq!(snap_to_half_pixel(-0.01), 0.0);
    assert_eq!(snap_to_device_pixel(10.33, 2.0), 10.5);
  }
}
