use serde::{Deserialize, Serialize};
use tiny_skia::BlendMode;

/// Normalized browser frame variants supported by the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowserBarVariant {
  #[serde(rename = "hidden")]
  Hidden,
  #[serde(rename = "mac-light")]
  MacLight,
  #[serde(rename = "mac-dark")]
  MacDark,
  #[serde(rename = "windows-light")]
  WindowsLight,
  #[serde(rename = "windows-dark")]
  WindowsDark,
  #[serde(rename = "safari-dark")]
  SafariDark,
}

impl Default for BrowserBarVariant {
  fn default() -> Self {
    Self::Hidden
  }
}

impl BrowserBarVariant {
  pub fn parse(value: &str) -> Self {
    match value.trim() {
      "light" | "mac-light" => Self::MacLight,
      "dark" | "mac-dark" => Self::MacDark,
      "windows" | "windows-dark" => Self::WindowsDark,
      "windows-light" => Self::WindowsLight,
      "safari" | "safari-dark" => Self::SafariDark,
      _ => Self::Hidden,
    }
  }

  pub fn as_str(self) -> &'static str {
    match self {
      Self::Hidden => "hidden",
      Self::MacLight => "mac-light",
      Self::MacDark => "mac-dark",
      Self::WindowsLight => "windows-light",
      Self::WindowsDark => "windows-dark",
      Self::SafariDark => "safari-dark",
    }
  }

  pub fn is_enabled(self) -> bool {
    self != Self::Hidden
  }

  pub fn is_dark(self) -> bool {
    matches!(self, Self::MacDark | Self::WindowsDark | Self::SafariDark)
  }
}

/// Convert canvas gradient angle convention (0 deg = +X, 90 deg = +Y)
/// to CSS `linear-gradient()` angle convention (0 deg = up, 90 deg = right).
/// Port of `ui/src/AppHelpers.jsx::canvasAngleDegToCssAngleDeg`.
#[inline]
pub fn canvas_angle_deg_to_css_angle_deg(angle_deg: f64) -> f64 {
  let a = if angle_deg.is_finite() { angle_deg } else { 0.0 };
  let css = a + 90.0;
  ((css % 360.0) + 360.0) % 360.0
}

/// Supported canvas blend modes for procedural blob gradients and overlays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CanvasBlendMode {
  Normal,
  Multiply,
  Screen,
  Overlay,
  Darken,
  Lighten,
  SoftLight,
  HardLight,
  Difference,
  Exclusion,
  Lighter,
}

impl CanvasBlendMode {
  pub fn from_blob_config(blend: Option<&str>, base_mode: Option<&str>) -> Self {
    let raw = blend.unwrap_or("").trim().to_ascii_lowercase();
    match raw.as_str() {
      "source-over" | "normal" => Self::Normal,
      "lighter" => Self::Lighter,
      "lighten" => Self::Lighten,
      "darken" => Self::Darken,
      "multiply" => Self::Multiply,
      "screen" => Self::Screen,
      "overlay" => Self::Overlay,
      "soft-light" => Self::SoftLight,
      "hard-light" => Self::HardLight,
      "difference" => Self::Difference,
      "exclusion" => Self::Exclusion,
      _ => {
        if base_mode.unwrap_or("dark").eq_ignore_ascii_case("light") {
          Self::Multiply
        } else {
          Self::Screen
        }
      }
    }
  }

  pub fn to_skia_blend_mode(self) -> BlendMode {
    match self {
      Self::Normal => BlendMode::SourceOver,
      Self::Multiply => BlendMode::Multiply,
      Self::Screen => BlendMode::Screen,
      Self::Overlay => BlendMode::Overlay,
      Self::Darken => BlendMode::Darken,
      Self::Lighten => BlendMode::Lighten,
      Self::SoftLight => BlendMode::SoftLight,
      Self::HardLight => BlendMode::HardLight,
      Self::Difference => BlendMode::Difference,
      Self::Exclusion => BlendMode::Exclusion,
      Self::Lighter => BlendMode::Plus,
    }
  }
}

/// Aspect ratio presets available in the Shotify UI (`AppHelpers.jsx::RATIO_OPTIONS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AspectRatioPreset {
  pub value: &'static str,
  pub label: &'static str,
  pub w: u32,
  pub h: u32,
}

pub const RATIO_OPTIONS: &[AspectRatioPreset] = &[
  AspectRatioPreset { value: "auto", label: "Auto (match image)", w: 0, h: 0 },
  AspectRatioPreset { value: "aspect-square", label: "1 : 1", w: 1, h: 1 },
  AspectRatioPreset { value: "aspect-video", label: "16 : 9", w: 16, h: 9 },
  AspectRatioPreset { value: "aspect-[4/3]", label: "4 : 3", w: 4, h: 3 },
  AspectRatioPreset { value: "aspect-[3/2]", label: "3 : 2", w: 3, h: 2 },
  AspectRatioPreset { value: "aspect-[9/16]", label: "9 : 16", w: 9, h: 16 },
  AspectRatioPreset { value: "aspect-[4/5]", label: "4 : 5", w: 4, h: 5 },
  AspectRatioPreset { value: "aspect-[2/3]", label: "2 : 3", w: 2, h: 3 },
  AspectRatioPreset { value: "custom", label: "Custom…", w: 0, h: 0 },
  AspectRatioPreset { value: "aspect-[1200/627]", label: "LinkedIn Post (1200×627)", w: 1200, h: 627 },
  AspectRatioPreset { value: "aspect-[1200/630]", label: "Facebook Link (1200×630)", w: 1200, h: 630 },
  AspectRatioPreset { value: "aspect-[3/1]", label: "X Header (1500×500)", w: 3, h: 1 },
  AspectRatioPreset { value: "aspect-[1600/900]", label: "Twitter Post Landscape (1600×900)", w: 1600, h: 900 },
  AspectRatioPreset { value: "aspect-[1080/1350-twitter]", label: "Twitter Post Portrait (1080×1350)", w: 1080, h: 1350 },
  AspectRatioPreset { value: "aspect-[1080/1920]", label: "Instagram Story (1080×1920)", w: 1080, h: 1920 },
  AspectRatioPreset { value: "aspect-[1080/1350-instagram]", label: "Instagram Portrait (1080×1350)", w: 1080, h: 1350 },
];

/// Parse Tailwind-style aspect class (e.g. `aspect-square`, `aspect-[4/3]`, `aspect-[1080/1350-twitter]`) into `(w, h)`.
pub fn aspect_class_to_dimensions(aspect_class: &str) -> Option<(u32, u32)> {
  let s = aspect_class.trim();
  if s == "aspect-square" {
    return Some((1, 1));
  }
  if s == "aspect-video" {
    return Some((16, 9));
  }
  if let Some(rest) = s.strip_prefix("aspect-[") {
    if let Some(inner) = rest.strip_suffix(']') {
      let cleaned = inner.split('-').next().unwrap_or(inner);
      let mut parts = cleaned.split('/');
      if let (Some(w_str), Some(h_str)) = (parts.next(), parts.next()) {
        if let (Ok(w), Ok(h)) = (w_str.parse::<u32>(), h_str.parse::<u32>()) {
          if w > 0 && h > 0 {
            return Some((w, h));
          }
        }
      }
    }
  }
  None
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn angle_conversion_matches_css() {
    assert_eq!(canvas_angle_deg_to_css_angle_deg(0.0), 90.0);
    assert_eq!(canvas_angle_deg_to_css_angle_deg(135.0), 225.0);
    assert_eq!(canvas_angle_deg_to_css_angle_deg(315.0), 45.0);
  }

  #[test]
  fn aspect_class_parses_all_presets() {
    assert_eq!(aspect_class_to_dimensions("aspect-square"), Some((1, 1)));
    assert_eq!(aspect_class_to_dimensions("aspect-video"), Some((16, 9)));
    assert_eq!(aspect_class_to_dimensions("aspect-[1080/1350-twitter]"), Some((1080, 1350)));
    assert_eq!(aspect_class_to_dimensions("auto"), None);
  }
}
