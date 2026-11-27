use glam::{Mat3, Mat4, Vec2, Vec3, Vec4};
use serde::{Deserialize, Serialize};
use tiny_skia::{Pixmap, PixmapPaint, Transform};

use crate::error::{CoreError, Result};

/// 2D translation + Z rotation + CSS 3D tilt (`perspective(1000px) rotateX() rotateY()`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardTransform3D {
  pub translate_x: f64,
  pub translate_y: f64,
  pub rotate_z_deg: f64,
  pub tilt_x_deg: f64,
  pub tilt_y_deg: f64,
  pub perspective_px: f64,
}

impl Default for CardTransform3D {
  fn default() -> Self {
    Self {
      translate_x: 0.0,
      translate_y: 0.0,
      rotate_z_deg: 0.0,
      tilt_x_deg: 0.0,
      tilt_y_deg: 0.0,
      perspective_px: 1000.0,
    }
  }
}

impl CardTransform3D {
  pub fn has_tilt(self) -> bool {
    self.tilt_x_deg.abs() > 0.0001 || self.tilt_y_deg.abs() > 0.0001
  }

  pub fn is_identity(self) -> bool {
    !self.has_tilt()
      && self.translate_x.abs() <= 0.0001
      && self.translate_y.abs() <= 0.0001
      && self.rotate_z_deg.abs() <= 0.0001
  }
}

/// Compute CSS perspective projection matrix `Rx * Ry * T * Rz` and project `(dx, dy, 0)` onto 2D.
pub fn project_card_corners_2d(
  card_w: f64,
  card_h: f64,
  canvas_center_x: f64,
  canvas_center_y: f64,
  t: CardTransform3D,
) -> [Vec2; 4] {
  let hw = (card_w / 2.0) as f32;
  let hh = (card_h / 2.0) as f32;

  let local_corners = [
    Vec4::new(-hw, -hh, 0.0, 1.0),
    Vec4::new(hw, -hh, 0.0, 1.0),
    Vec4::new(hw, hh, 0.0, 1.0),
    Vec4::new(-hw, hh, 0.0, 1.0),
  ];

  let rx = Mat4::from_rotation_x((t.tilt_x_deg as f32).to_radians());
  let ry = Mat4::from_rotation_y((t.tilt_y_deg as f32).to_radians());
  let rz = Mat4::from_rotation_z((t.rotate_z_deg as f32).to_radians());
  let trans = Mat4::from_translation(Vec3::new(t.translate_x as f32, t.translate_y as f32, 0.0));

  let m = rx * ry * trans * rz;
  let persp_d = (t.perspective_px.max(100.0)) as f32;

  let mut out = [Vec2::ZERO; 4];
  for (i, c) in local_corners.iter().enumerate() {
    let v = m * *c;
    // Standard CSS perspective division: x' = x / (1 - z/d), y' = y / (1 - z/d)
    let w = (1.0 - v.z / persp_d).max(0.05);
    out[i] = Vec2::new(
      (canvas_center_x as f32) + v.x / w,
      (canvas_center_y as f32) + v.y / w,
    );
  }
  out
}

/// Compute closed-form 3x3 homography mapping `[0,1] x [0,1]` onto quad `[q0, q1, q2, q3]`.
pub fn unit_square_to_quad_homography(q: [Vec2; 4]) -> Option<Mat3> {
  let [q0, q1, q2, q3] = q;
  let dx1 = q1.x - q2.x;
  let dx2 = q3.x - q2.x;
  let sx = q0.x - q1.x + q2.x - q3.x;

  let dy1 = q1.y - q2.y;
  let dy2 = q3.y - q2.y;
  let sy = q0.y - q1.y + q2.y - q3.y;

  let det = dx1 * dy2 - dx2 * dy1;
  if det.abs() < 1e-6 {
    return None;
  }

  let g = (sx * dy2 - dx2 * sy) / det;
  let h = (dx1 * sy - sx * dy1) / det;

  let a = q1.x - q0.x + g * q1.x;
  let b = q3.x - q0.x + h * q3.x;
  let c = q0.x;

  let d = q1.y - q0.y + g * q1.y;
  let e = q3.y - q0.y + h * q3.y;
  let f = q0.y;

  Some(Mat3::from_cols(
    Vec3::new(a, d, g),
    Vec3::new(b, e, h),
    Vec3::new(c, f, 1.0),
  ))
}

#[inline]
fn sample_pixmap_bilinear_pm(pm: &Pixmap, u_norm: f32, v_norm: f32) -> [u8; 4] {
  let w = pm.width() as f32;
  let h = pm.height() as f32;
  if u_norm < 0.0 || u_norm >= 1.0 || v_norm < 0.0 || v_norm >= 1.0 {
    return [0, 0, 0, 0];
  }

  let fx = (u_norm * w - 0.5).clamp(0.0, w - 1.0);
  let fy = (v_norm * h - 0.5).clamp(0.0, h - 1.0);

  let x0 = fx.floor() as u32;
  let y0 = fy.floor() as u32;
  let x1 = (x0 + 1).min(pm.width() - 1);
  let y1 = (y0 + 1).min(pm.height() - 1);
  let tx = fx - (x0 as f32);
  let ty = fy - (y0 as f32);

  let data = pm.data();
  let stride = (pm.width() as usize) * 4;
  let idx00 = (y0 as usize) * stride + (x0 as usize) * 4;
  let idx10 = (y0 as usize) * stride + (x1 as usize) * 4;
  let idx01 = (y1 as usize) * stride + (x0 as usize) * 4;
  let idx11 = (y1 as usize) * stride + (x1 as usize) * 4;

  let w00 = (1.0 - tx) * (1.0 - ty);
  let w10 = tx * (1.0 - ty);
  let w01 = (1.0 - tx) * ty;
  let w11 = tx * ty;

  let mut out = [0u8; 4];
  for c in 0..4 {
    let val = f32::from(data[idx00 + c]) * w00
      + f32::from(data[idx10 + c]) * w10
      + f32::from(data[idx01 + c]) * w01
      + f32::from(data[idx11 + c]) * w11;
    out[c] = val.round().clamp(0.0, 255.0) as u8;
  }
  out
}

/// Draw a source `Pixmap` onto a destination `Pixmap` centered at `(center_x, center_y)`
/// applying translation, Z rotation, and optional 3D perspective tilt.
pub fn composite_pixmap_transformed(
  dst: &mut Pixmap,
  src: &Pixmap,
  center_x: f64,
  center_y: f64,
  transform: CardTransform3D,
) -> Result<()> {
  if src.width() == 0 || src.height() == 0 {
    return Ok(());
  }

  // Fast 2D path when there is no 3D tilt (matches ImageDisplay.jsx export reliability guard)
  if !transform.has_tilt() {
    let mut paint = PixmapPaint::default();
    paint.quality = tiny_skia::FilterQuality::Bilinear;
    let hw = (src.width() as f32) / 2.0;
    let hh = (src.height() as f32) / 2.0;

    let xform = Transform::from_translate(-hw, -hh)
      .post_rotate(transform.rotate_z_deg as f32)
      .post_translate(
        (center_x + transform.translate_x) as f32,
        (center_y + transform.translate_y) as f32,
      );
    dst.draw_pixmap(0, 0, src.as_ref(), &paint, xform, None);
    return Ok(());
  }

  let quad = project_card_corners_2d(
    f64::from(src.width()),
    f64::from(src.height()),
    center_x,
    center_y,
    transform,
  );

  let Some(h_mat) = unit_square_to_quad_homography(quad) else {
    return Err(CoreError::InvalidColor("singular perspective transform".to_string()));
  };
  let inv_h = h_mat.inverse();

  let mut min_x = f32::INFINITY;
  let mut min_y = f32::INFINITY;
  let mut max_x = f32::NEG_INFINITY;
  let mut max_y = f32::NEG_INFINITY;
  for p in &quad {
    min_x = min_x.min(p.x);
    min_y = min_y.min(p.y);
    max_x = max_x.max(p.x);
    max_y = max_y.max(p.y);
  }

  let dw = dst.width();
  let dh = dst.height();
  let x0 = (min_x.floor() as i32).max(0) as u32;
  let y0 = (min_y.floor() as i32).max(0) as u32;
  let x1 = ((max_x.ceil() as i32).max(0) as u32).min(dw);
  let y1 = ((max_y.ceil() as i32).max(0) as u32).min(dh);

  let dst_data = dst.data_mut();
  let dst_stride = (dw as usize) * 4;

  for y in y0..y1 {
    for x in x0..x1 {
      let uvw = inv_h * Vec3::new((x as f32) + 0.5, (y as f32) + 0.5, 1.0);
      if uvw.z.abs() < 1e-6 {
        continue;
      }
      let u = uvw.x / uvw.z;
      let v = uvw.y / uvw.z;
      if u < 0.0 || u >= 1.0 || v < 0.0 || v >= 1.0 {
        continue;
      }

      let src_pm = sample_pixmap_bilinear_pm(src, u, v);
      let sa = f32::from(src_pm[3]) / 255.0;
      if sa <= 0.0 {
        continue;
      }

      // Premultiplied SourceOver blend into dst
      let idx = (y as usize) * dst_stride + (x as usize) * 4;
      let inv_sa = 1.0 - sa;
      for c in 0..4 {
        let blended = f32::from(src_pm[c]) + f32::from(dst_data[idx + c]) * inv_sa;
        dst_data[idx + c] = blended.round().clamp(0.0, 255.0) as u8;
      }
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn homography_inverts_quad_corners_precisely() {
    let quad = project_card_corners_2d(
      200.0,
      100.0,
      250.0,
      250.0,
      CardTransform3D {
        tilt_x_deg: 12.0,
        tilt_y_deg: -18.0,
        ..Default::default()
      },
    );
    let h = unit_square_to_quad_homography(quad).expect("valid homography");
    let inv = h.inverse();
    let p0 = inv * Vec3::new(quad[0].x, quad[0].y, 1.0);
    assert!((p0.x / p0.z).abs() < 1e-4);
    assert!((p0.y / p0.z).abs() < 1e-4);

    let p2 = inv * Vec3::new(quad[2].x, quad[2].y, 1.0);
    assert!(((p2.x / p2.z) - 1.0).abs() < 1e-4);
    assert!(((p2.y / p2.z) - 1.0).abs() < 1e-4);
  }

  #[test]
  fn tilted_card_displays_non_empty_pixels() {
    let mut card = Pixmap::new(80, 60).unwrap();
    card.fill(tiny_skia::Color::from_rgba8(255, 100, 50, 255));
    let mut canvas = Pixmap::new(200, 200).unwrap();
    composite_pixmap_transformed(
      &mut canvas,
      &card,
      100.0,
      100.0,
      CardTransform3D {
        tilt_x_deg: 20.0,
        tilt_y_deg: 15.0,
        ..Default::default()
      },
    )
    .unwrap();
    let center = canvas.pixel(100, 100).unwrap();
    assert_eq!(center.red(), 255);
  }
}
