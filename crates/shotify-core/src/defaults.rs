use serde::{Deserialize, Serialize};

use crate::color::{is_probably_hex_color, normalize_hex_color_input};
use crate::helpers::BrowserBarVariant;
use crate::math::clamp_f64;

/// Map legacy Tailwind rounded class name to pixel corner radius.
/// Port of `utils/core/editorOptions.js::getRoundedPx`.
pub fn get_rounded_px(rounded_class: &str) -> u32 {
  match rounded_class.trim() {
    "rounded-none" => 0,
    "rounded-sm" => 2,
    "rounded" => 4,
    "rounded-md" => 6,
    "rounded-lg" => 8,
    "rounded-xl" => 12,
    "rounded-2xl" => 16,
    "rounded-3xl" => 24,
    _ => 12,
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AspectMode {
  Preset,
  Auto,
  Custom,
}

impl Default for AspectMode {
  fn default() -> Self {
    Self::Preset
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AspectRatioPair {
  pub w: f64,
  pub h: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AspectAutoPair {
  pub w: Option<u32>,
  pub h: Option<u32>,
}

impl Default for AspectAutoPair {
  fn default() -> Self {
    Self { w: None, h: None }
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomTheme {
  pub color_start: String,
  pub color_end: String,
  #[serde(default = "default_angle_135")]
  pub angle_deg: f64,
}

fn default_angle_135() -> f64 {
  135.0
}

impl Default for CustomTheme {
  fn default() -> Self {
    Self {
      color_start: "#6D28D9".to_string(),
      color_end: "#A78BFA".to_string(),
      angle_deg: 135.0,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobGradientConfig {
  #[serde(default = "default_blob_colors")]
  pub colors: Vec<String>,
  #[serde(default = "default_seed")]
  pub seed: u32,
  #[serde(default = "default_blob_count")]
  pub blob_count: u32,
  #[serde(default = "default_blob_intensity")]
  pub intensity: f64,
  #[serde(default = "default_angle_135")]
  pub angle_deg: f64,
  #[serde(default = "default_base_mode")]
  pub base_mode: String,
  #[serde(default)]
  pub base_a: Option<String>,
  #[serde(default)]
  pub base_b: Option<String>,
  #[serde(default)]
  pub blend: Option<String>,
  #[serde(default)]
  pub corner_bias: Option<f64>,
  #[serde(default)]
  pub blob_blur_px: Option<f64>,
}

fn default_blob_colors() -> Vec<String> {
  vec![
    "#6D28D9".to_string(),
    "#A78BFA".to_string(),
    "#EC4899".to_string(),
    "#22C55E".to_string(),
  ]
}
fn default_seed() -> u32 { 1 }
fn default_blob_count() -> u32 { 6 }
fn default_blob_intensity() -> f64 { 0.85 }
fn default_base_mode() -> String { "dark".to_string() }

impl Default for BlobGradientConfig {
  fn default() -> Self {
    Self {
      colors: default_blob_colors(),
      seed: 1,
      blob_count: 6,
      intensity: 0.85,
      angle_deg: 135.0,
      base_mode: "dark".to_string(),
      base_a: None,
      base_b: None,
      blend: None,
      corner_bias: None,
      blob_blur_px: None,
    }
  }
}

/// Shadow parameters mirroring `advancedShadow` and Tauri's `ShadowPreset`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShadowParams {
  #[serde(default)]
  pub enabled: bool,
  #[serde(default)]
  pub offset_x: f64,
  #[serde(default = "default_shadow_offset_y")]
  pub offset_y: f64,
  #[serde(default = "default_shadow_blur")]
  pub blur: f64,
  #[serde(default = "default_shadow_spread")]
  pub spread: f64,
  #[serde(default = "default_shadow_color")]
  pub color: String,
  #[serde(default = "default_shadow_opacity")]
  pub opacity: f64,
}

fn default_shadow_offset_y() -> f64 { 10.0 }
fn default_shadow_blur() -> f64 { 15.0 }
fn default_shadow_spread() -> f64 { -3.0 }
fn default_shadow_color() -> String { "#000000".to_string() }
fn default_shadow_opacity() -> f64 { 0.1 }

impl Default for ShadowParams {
  fn default() -> Self {
    Self {
      enabled: false,
      offset_x: 0.0,
      offset_y: 10.0,
      blur: 15.0,
      spread: -3.0,
      color: "#000000".to_string(),
      opacity: 0.1,
    }
  }
}

impl ShadowParams {
  /// Default Hug shadow preset (`ui_shadow_hug_preset` / `DEFAULT_HUG_SHADOW_PRESET`).
  pub fn hug_preset() -> Self {
    Self {
      enabled: true,
      offset_x: 0.0,
      offset_y: 10.0,
      blur: 30.0,
      spread: 0.0,
      color: "#000000".to_string(),
      opacity: 0.5,
    }
  }

  /// Heavy preset shadow (`ImageDisplay.jsx`).
  pub fn heavy_preset() -> Self {
    Self {
      enabled: true,
      offset_x: 0.0,
      offset_y: 50.0,
      blur: 90.0,
      spread: 20.0,
      color: "#000000".to_string(),
      opacity: 0.7,
    }
  }

  /// Realistic preset shadow (`ImageDisplay.jsx`).
  pub fn realistic_preset() -> Self {
    Self {
      enabled: true,
      offset_x: 25.0,
      offset_y: 40.0,
      blur: 60.0,
      spread: 8.0,
      color: "#000000".to_string(),
      opacity: 0.7,
    }
  }

  /// Sanitize custom shadow bounds (port of `AppHelpers.jsx::sanitizeCustomShadowState`).
  pub fn sanitize(&self) -> Self {
    Self {
      enabled: self.enabled,
      offset_x: clamp_f64(self.offset_x, -200.0, 200.0),
      offset_y: clamp_f64(self.offset_y, -200.0, 300.0),
      blur: clamp_f64(self.blur, 0.0, 400.0),
      spread: clamp_f64(self.spread, -200.0, 200.0),
      color: if is_probably_hex_color(&self.color) {
        normalize_hex_color_input(&self.color)
      } else {
        "#000000".to_string()
      },
      opacity: clamp_f64(self.opacity, 0.0, 1.0),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct InsetConfig {
  pub padding: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BrowserFrameConfig {
  #[serde(default = "default_one")]
  pub scale: f64,
}

fn default_one() -> f64 { 1.0 }

impl Default for BrowserFrameConfig {
  fn default() -> Self {
    Self { scale: 1.0 }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct TiltConfig {
  pub x: f64,
  pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct PositionConfig {
  pub x: f64,
  pub y: f64,
  pub rotate: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FxConfig {
  #[serde(default)]
  pub noise: bool,
  #[serde(default)]
  pub blur: f64,
  #[serde(default = "default_one")]
  pub brightness: f64,
  #[serde(default)]
  pub vignette: f64,
}

impl Default for FxConfig {
  fn default() -> Self {
    Self {
      noise: false,
      blur: 0.0,
      brightness: 1.0,
      vignette: 0.0,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BalanceConfig {
  pub enabled: bool,
}

impl Default for BalanceConfig {
  fn default() -> Self {
    Self { enabled: true }
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatermarkConfig {
  #[serde(default = "default_true")]
  pub enabled: bool,
  #[serde(default = "default_watermark_text")]
  pub text: String,
  #[serde(default)]
  pub show_twitter: bool,
  #[serde(default)]
  pub show_verified: bool,
  #[serde(default = "default_watermark_position")]
  pub position: String,
  #[serde(default)]
  pub prefix: Option<String>,
  #[serde(default)]
  pub offset_x: f64,
  #[serde(default)]
  pub offset_y: f64,
}

fn default_true() -> bool { true }
fn default_watermark_text() -> String { "Made with shotify".to_string() }
fn default_watermark_position() -> String { "inside".to_string() }

impl Default for WatermarkConfig {
  fn default() -> Self {
    Self {
      enabled: true,
      text: default_watermark_text(),
      show_twitter: false,
      show_verified: false,
      position: "inside".to_string(),
      prefix: None,
      offset_x: 0.0,
      offset_y: 0.0,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PenConfig {
  pub color: String,
}

impl Default for PenConfig {
  fn default() -> Self {
    Self {
      color: "#FFFFFF".to_string(),
    }
  }
}

/// Complete editor options state mirroring `utils/core/editorOptions.js::baseOptions`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorOptions {
  pub aspect_ratio: String,
  pub aspect_mode: AspectMode,
  pub aspect_custom: AspectRatioPair,
  pub aspect_auto: AspectAutoPair,
  pub theme: String,
  pub background_type: String,
  pub wallpaper: Option<String>,
  #[serde(default)]
  pub wallpaper_path: Option<String>,
  #[serde(default)]
  pub wallpaper_source: Option<String>,
  pub custom_theme: CustomTheme,
  #[serde(default)]
  pub blob_gradient: BlobGradientConfig,
  pub padding_size: f64,
  pub rounded: String,
  pub corner_radius: u32,
  pub shadow: String,
  pub border_width: f64,
  #[serde(default)]
  pub border_color: Option<String>,
  pub advanced_shadow: ShadowParams,
  pub inset: InsetConfig,
  pub browser_frame: BrowserFrameConfig,
  #[serde(default)]
  pub browser_url: Option<String>,
  pub tilt: TiltConfig,
  pub position: PositionConfig,
  pub noise: bool,
  pub fx: FxConfig,
  pub browser_bar: String,
  pub balance: BalanceConfig,
  pub watermark: WatermarkConfig,
  pub pen: PenConfig,
}

impl Default for EditorOptions {
  fn default() -> Self {
    Self {
      aspect_ratio: "aspect-square".to_string(),
      aspect_mode: AspectMode::Preset,
      aspect_custom: AspectRatioPair { w: 1.0, h: 1.0 },
      aspect_auto: AspectAutoPair::default(),
      theme: "bg-gradient-to-br from-based-purple via-based-purple-dark to-based-purple-light".to_string(),
      background_type: "gradient".to_string(),
      wallpaper: None,
      wallpaper_path: None,
      wallpaper_source: None,
      custom_theme: CustomTheme::default(),
      blob_gradient: BlobGradientConfig::default(),
      padding_size: 20.0,
      rounded: "rounded-xl".to_string(),
      corner_radius: 12,
      shadow: "Hug (Default)".to_string(),
      border_width: 0.0,
      border_color: None,
      advanced_shadow: ShadowParams::default(),
      inset: InsetConfig::default(),
      browser_frame: BrowserFrameConfig::default(),
      browser_url: None,
      tilt: TiltConfig::default(),
      position: PositionConfig::default(),
      noise: false,
      fx: FxConfig::default(),
      browser_bar: "hidden".to_string(),
      balance: BalanceConfig::default(),
      watermark: WatermarkConfig::default(),
      pen: PenConfig::default(),
    }
  }
}

impl EditorOptions {
  /// Normalize ranges, legacy shadow/browserBar variants, and watermark fields.
  /// Port of `utils/core/editorOptions.js::createSafeOptions`.
  pub fn normalize(&mut self, balance_default_enabled: Option<bool>) {
    self.inset.padding = clamp_f64(self.inset.padding, 0.0, 100.0);
    if let Some(default_bal) = balance_default_enabled {
      // Preserve explicit user setting if already configured; only helper for missing values.
      let _ = default_bal;
    }
    self.browser_bar = BrowserBarVariant::parse(&self.browser_bar).as_str().to_string();

    if self.watermark.text.trim().is_empty() {
      self.watermark.text = default_watermark_text();
    }

    self.fx.noise = self.fx.noise || self.noise;
    self.fx.blur = if self.fx.blur.is_finite() { clamp_f64(self.fx.blur, 0.0, 200.0) } else { 0.0 };
    self.fx.brightness = if self.fx.brightness.is_finite() { self.fx.brightness } else { 1.0 };
    self.fx.vignette = if self.fx.vignette.is_finite() { self.fx.vignette } else { 0.0 };

    if !self.padding_size.is_finite() {
      self.padding_size = 20.0;
    }

    self.corner_radius = self.corner_radius.min(64);

    if self.shadow == "Custom" {
      self.advanced_shadow.enabled = true;
    } else if self.advanced_shadow.enabled {
      self.shadow = "Custom".to_string();
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn defaults_and_normalization_match_editor_options() {
    let mut opts = EditorOptions::default();
    assert_eq!(opts.watermark.text, "Made with shotify");
    assert_eq!(opts.corner_radius, 12);

    opts.inset.padding = 250.0;
    opts.corner_radius = 999;
    opts.browser_bar = "safari".to_string();
    opts.advanced_shadow.enabled = true;
    opts.normalize(None);

    assert_eq!(opts.inset.padding, 100.0);
    assert_eq!(opts.corner_radius, 64);
    assert_eq!(opts.browser_bar, "safari-dark");
    assert_eq!(opts.shadow, "Custom");
  }
}
