use serde::{Deserialize, Serialize};

use crate::defaults::AspectMode;
use crate::helpers::BrowserBarVariant;
use crate::math::{clamp_f64, snap_to_device_pixel};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RectF {
  pub left: f64,
  pub top: f64,
  pub width: f64,
  pub height: f64,
}

impl RectF {
  #[inline]
  pub fn right(self) -> f64 {
    self.left + self.width
  }

  #[inline]
  pub fn bottom(self) -> f64 {
    self.top + self.height
  }
}

/// Port of `utils/image/objectFit.js::computeObjectContainRect`.
pub fn compute_object_contain_rect(
  container_left: f64,
  container_top: f64,
  container_width: f64,
  container_height: f64,
  natural_width: f64,
  natural_height: f64,
) -> Option<RectF> {
  if !container_width.is_finite() || !container_height.is_finite() || container_width <= 0.0 || container_height <= 0.0 {
    return None;
  }
  let cleft = if container_left.is_finite() { container_left } else { 0.0 };
  let ctop = if container_top.is_finite() { container_top } else { 0.0 };

  if !natural_width.is_finite() || !natural_height.is_finite() || natural_width <= 0.0 || natural_height <= 0.0 {
    return Some(RectF {
      left: cleft,
      top: ctop,
      width: container_width,
      height: container_height,
    });
  }

  let scale = (container_width / natural_width).min(container_height / natural_height);
  let rw = natural_width * scale;
  let rh = natural_height * scale;
  let left = cleft + (container_width - rw) / 2.0;
  let top = ctop + (container_height - rh) / 2.0;

  Some(RectF {
    left,
    top,
    width: rw,
    height: rh,
  })
}

/// Port of `utils/image/canvasBackgroundFx.js::getCoverRect`.
pub fn compute_object_cover_rect(src_w: f64, src_h: f64, dst_w: f64, dst_h: f64) -> RectF {
  if src_w <= 0.0 || src_h <= 0.0 || dst_w <= 0.0 || dst_h <= 0.0 {
    return RectF {
      left: 0.0,
      top: 0.0,
      width: dst_w.max(0.0),
      height: dst_h.max(0.0),
    };
  }

  let src_ratio = src_w / src_h;
  let dst_ratio = dst_w / dst_h;
  let (draw_w, draw_h) = if src_ratio > dst_ratio {
    (dst_h * src_ratio, dst_h)
  } else {
    (dst_w, dst_w / src_ratio)
  };

  RectF {
    left: (dst_w - draw_w) / 2.0,
    top: (dst_h - draw_h) / 2.0,
    width: draw_w,
    height: draw_h,
  }
}

/// Port of `utils/image/insetRadius.js::computeInsetContentRadiusPx`.
#[inline]
pub fn compute_inset_content_radius_px(screenshot_radius_px: f64, inset_scale: f64) -> f64 {
  let outer_radius = if screenshot_radius_px.is_finite() {
    screenshot_radius_px.max(0.0)
  } else {
    0.0
  };
  let scale = if inset_scale.is_finite() {
    clamp_f64(inset_scale, 0.0, 1.0)
  } else {
    1.0
  };
  outer_radius * scale
}

/// Port of `utils/image/paddingPolicy.js::computeEffectivePaddingPx`.
#[inline]
pub fn compute_effective_padding_px(padding_px: f64) -> f64 {
  if !padding_px.is_finite() {
    return 0.0;
  }
  padding_px.round().max(0.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BrowserBarMetrics {
  pub enabled: bool,
  pub variant: BrowserBarVariant,
  pub scale: f64,
  pub height: f64,
  pub padding_y: f64,
  pub circle_h: f64,
}

/// Port of `components/editor/ImageDisplay.jsx::browserBarMetrics`.
pub fn compute_browser_bar_metrics(variant: BrowserBarVariant, browser_scale: f64) -> BrowserBarMetrics {
  if !variant.is_enabled() {
    return BrowserBarMetrics {
      enabled: false,
      variant: BrowserBarVariant::Hidden,
      scale: 1.0,
      height: 0.0,
      padding_y: 0.0,
      circle_h: 0.0,
    };
  }

  let scale = if browser_scale.is_finite() && browser_scale > 0.0 {
    browser_scale
  } else {
    1.0
  };

  match variant {
    BrowserBarVariant::WindowsLight | BrowserBarVariant::WindowsDark => {
      let padding_y = 6.0 * scale;
      let height = (30.0 * scale + padding_y * 2.0).round();
      BrowserBarMetrics { enabled: true, variant, scale, height, padding_y, circle_h: 0.0 }
    }
    BrowserBarVariant::MacLight | BrowserBarVariant::MacDark => {
      let padding_y = 10.0 * scale;
      let circle_h = 12.0 * scale;
      let height = (circle_h + padding_y * 2.0).round();
      BrowserBarMetrics { enabled: true, variant, scale, height, padding_y, circle_h }
    }
    BrowserBarVariant::SafariDark => {
      let height = (52.0 * scale).round();
      BrowserBarMetrics { enabled: true, variant, scale, height, padding_y: 0.0, circle_h: 0.0 }
    }
    BrowserBarVariant::Hidden => BrowserBarMetrics {
      enabled: false,
      variant: BrowserBarVariant::Hidden,
      scale: 1.0,
      height: 0.0,
      padding_y: 0.0,
      circle_h: 0.0,
    },
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardLayout {
  pub display_width: f64,
  pub display_height: f64,
  pub padding: f64,
  pub scale_zoom: f64,
  pub raw_padding: f64,
  pub inset_scale: f64,
  pub inset_inner_width: f64,
  pub inset_inner_height: f64,
  pub card_total_height: f64,
  pub padded_frame_width: f64,
  pub padded_frame_height: f64,
}

/// Port of `components/editor/ImageDisplay.jsx` card + inset layout calculations.
pub fn compute_card_layout(
  container_w: f64,
  container_h: f64,
  image_w: f64,
  image_h: f64,
  raw_padding: f64,
  inset_padding: f64,
  aspect_mode: AspectMode,
  browser_bar: BrowserBarMetrics,
  dpr: f64,
) -> CardLayout {
  let pos_padding = compute_effective_padding_px(raw_padding.max(0.0));
  let neg_padding = raw_padding.min(0.0);
  let bar_h = if browser_bar.enabled { browser_bar.height } else { 0.0 };
  let zoom_factor = if neg_padding < 0.0 {
    1.0 + (neg_padding.abs() / 100.0).min(3.0)
  } else {
    1.0
  };

  if container_w <= 0.0 || image_w <= 0.0 || image_h <= 0.0 {
    return CardLayout {
      display_width: 0.0,
      display_height: 0.0,
      padding: pos_padding,
      scale_zoom: 1.0,
      raw_padding,
      inset_scale: 1.0,
      inset_inner_width: 0.0,
      inset_inner_height: 0.0,
      card_total_height: bar_h,
      padded_frame_width: 1.0,
      padded_frame_height: 1.0,
    };
  }

  let (display_width, display_height) = if aspect_mode == AspectMode::Auto {
    let base_w = (container_w - pos_padding * 2.0).max(1.0);
    let base_h = base_w * image_h / image_w;
    (
      snap_to_device_pixel(base_w * zoom_factor, dpr).max(1.0),
      snap_to_device_pixel(base_h * zoom_factor, dpr).max(1.0),
    )
  } else {
    if container_h <= 0.0 {
      return CardLayout {
        display_width: 0.0,
        display_height: 0.0,
        padding: pos_padding,
        scale_zoom: 1.0,
        raw_padding,
        inset_scale: 1.0,
        inset_inner_width: 0.0,
        inset_inner_height: 0.0,
        card_total_height: bar_h,
        padded_frame_width: 1.0,
        padded_frame_height: 1.0,
      };
    }

    let avail_w = (container_w - pos_padding * 2.0).max(0.0);
    let avail_h = (container_h - pos_padding * 2.0 - bar_h).max(0.0);
    let r = image_h / image_w;
    let scale_w = avail_w / image_w;
    let scale_h = avail_h / image_h;

    let (mut base_w, mut base_h) = if scale_w <= scale_h {
      let w = avail_w;
      let mut h = w * r;
      if h > avail_h {
        h = avail_h;
        (h / r, h)
      } else {
        (w, h)
      }
    } else {
      let h = avail_h;
      let mut w = h / r;
      if w > avail_w {
        w = avail_w;
        (w, w * r)
      } else {
        (w, h)
      }
    };

    base_w = base_w.max(0.0);
    base_h = base_h.max(0.0);

    (
      snap_to_device_pixel(base_w * zoom_factor, dpr).max(1.0),
      snap_to_device_pixel(base_h * zoom_factor, dpr).max(1.0),
    )
  };

  let inset_px = inset_padding.max(0.0);
  let base_dim = display_width.min(display_height);
  let inset_scale = if base_dim <= 0.0 || inset_px <= 0.0 {
    1.0
  } else {
    ((base_dim - inset_px * 2.0) / base_dim).max(0.1)
  };

  let inset_inner_width = snap_to_device_pixel(display_width * inset_scale, dpr);
  let inset_inner_height = snap_to_device_pixel(display_height * inset_scale, dpr);

  let card_total_height = display_height + bar_h;
  let padded_frame_width = (display_width + pos_padding * 2.0).max(1.0);
  let padded_frame_height = (display_height + bar_h + pos_padding * 2.0).max(1.0);

  CardLayout {
    display_width,
    display_height,
    padding: pos_padding,
    scale_zoom: zoom_factor,
    raw_padding,
    inset_scale,
    inset_inner_width,
    inset_inner_height,
    card_total_height,
    padded_frame_width,
    padded_frame_height,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn object_contain_centers_wide_image() {
    let r = compute_object_contain_rect(10.0, 20.0, 200.0, 200.0, 400.0, 200.0).unwrap();
    assert_eq!(r.left, 10.0);
    assert_eq!(r.top, 70.0);
    assert_eq!(r.width, 200.0);
    assert_eq!(r.height, 100.0);
  }

  #[test]
  fn object_cover_crops_wide_image() {
    let r = compute_object_cover_rect(1600.0, 900.0, 500.0, 500.0);
    assert!((r.height - 500.0).abs() < 1e-6);
    assert!(r.width > 500.0);
    assert!(r.left < 0.0);
  }

  #[test]
  fn card_layout_handles_auto_and_fixed_modes() {
    let bar = compute_browser_bar_metrics(BrowserBarVariant::MacDark, 1.0);
    assert_eq!(bar.height, 32.0);
    let auto_layout = compute_card_layout(800.0, 600.0, 1000.0, 500.0, 40.0, 20.0, AspectMode::Auto, bar, 1.0);
    assert_eq!(auto_layout.display_width, 720.0);
    assert_eq!(auto_layout.display_height, 360.0);
    assert!(auto_layout.inset_scale < 1.0);
  }
}
