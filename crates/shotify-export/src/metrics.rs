use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::encoder::ExportFormat;

/// Presentation state of the card shadow during preview vs export.
/// Port of `utils/export/exportPresentation.js`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShadowPresentationKind {
  None,
  BoxShadow,
  DropShadow,
}

#[inline]
pub fn get_export_background_frame_radius_px(is_exporting: bool, preview_radius_px: f64) -> f64 {
  let radius = if preview_radius_px.is_finite() {
    preview_radius_px.max(0.0)
  } else {
    0.0
  };
  if is_exporting {
    0.0
  } else {
    radius
  }
}

#[inline]
pub fn get_card_shadow_presentation_kind(
  is_exporting: bool,
  box_shadow: Option<&str>,
  filter: Option<&str>,
) -> ShadowPresentationKind {
  let box_s = box_shadow.unwrap_or("none").trim().to_ascii_lowercase();
  let filt = filter.unwrap_or("none").trim().to_ascii_lowercase();

  if is_exporting {
    if filt != "none" && filt.contains("drop-shadow(") {
      ShadowPresentationKind::DropShadow
    } else {
      ShadowPresentationKind::None
    }
  } else if box_s != "none" {
    ShadowPresentationKind::BoxShadow
  } else {
    ShadowPresentationKind::None
  }
}

#[inline]
pub fn is_export_presentation_ready(
  is_exporting: bool,
  shadow_kind: ShadowPresentationKind,
  background_frame_radius_px: f64,
) -> bool {
  if !is_exporting {
    return true;
  }
  let radius = if background_frame_radius_px.is_finite() {
    background_frame_radius_px.max(0.0)
  } else {
    0.0
  };
  matches!(
    shadow_kind,
    ShadowPresentationKind::DropShadow | ShadowPresentationKind::None
  ) && radius == 0.0
}

/// Compute target export pixel dimensions respecting explicit `target_width`/`target_height`
/// overrides or uniform scale multiplier.
/// Port of `utils/export/imageProcessor.js::scaleAndEncodeBlob` sizing logic.
pub fn compute_export_dimensions(
  src_w: u32,
  src_h: u32,
  scale: f64,
  target_width: Option<u32>,
  target_height: Option<u32>,
) -> (u32, u32) {
  if let (Some(tw), Some(th)) = (target_width, target_height) {
    if tw > 0 && th > 0 {
      return (tw, th);
    }
  }
  let eff_scale = if scale.is_finite() && scale > 0.0 {
    scale
  } else {
    1.0
  };
  let out_w = (f64::from(src_w.max(1)) * eff_scale).round().max(1.0) as u32;
  let out_h = (f64::from(src_h.max(1)) * eff_scale).round().max(1.0) as u32;
  (out_w, out_h)
}

/// Generate default timestamped export filename (`shotify-YYYY-MM-DD_HH-MM-SS.<ext>`).
/// Port of `ui/src/hooks/useExportFunctions.js::buildDefaultExportFileName`.
pub fn build_default_export_file_name(format: ExportFormat) -> String {
  let stamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
  format!("shotify-{stamp}.{}", format.extension())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn export_presentation_checks_match_js() {
    assert_eq!(get_export_background_frame_radius_px(true, 26.0), 0.0);
    assert_eq!(get_export_background_frame_radius_px(false, 26.0), 26.0);
    assert_eq!(
      get_card_shadow_presentation_kind(true, Some("0 10px 30px #000"), Some("drop-shadow(0 10px 30px #000)")),
      ShadowPresentationKind::DropShadow
    );
    assert!(is_export_presentation_ready(true, ShadowPresentationKind::DropShadow, 0.0));
    assert!(!is_export_presentation_ready(true, ShadowPresentationKind::BoxShadow, 0.0));
  }

  #[test]
  fn computes_export_dimensions_at_scale_and_target_override() {
    assert_eq!(compute_export_dimensions(800, 600, 2.0, None, None), (1600, 1200));
    assert_eq!(compute_export_dimensions(800, 600, 2.0, Some(1280), Some(720)), (1280, 720));
  }
}
