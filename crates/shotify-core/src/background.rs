use serde::{Deserialize, Serialize};

use crate::defaults::{BlobGradientConfig, CustomTheme, EditorOptions};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BackgroundKind {
  Gradient,
  BlobGradient,
  Wallpaper,
  Transparent,
}

impl Default for BackgroundKind {
  fn default() -> Self {
    Self::Gradient
  }
}

/// Port of `utils/image/backgroundToggle.js` selection checks & toggle-off helpers.
pub fn is_same_wallpaper_selection(options: &EditorOptions, candidate_path: &str) -> bool {
  if !options.background_type.eq_ignore_ascii_case("wallpaper") || candidate_path.is_empty() {
    return false;
  }
  let active = options
    .wallpaper_path
    .as_deref()
    .or(options.wallpaper.as_deref());
  matches!(active, Some(a) if a == candidate_path)
}

pub fn toggle_wallpaper_off(options: &mut EditorOptions) {
  options.background_type = "gradient".to_string();
  options.wallpaper = None;
  options.wallpaper_source = None;
  options.wallpaper_path = None;
}

pub fn is_same_gradient_selection(options: &EditorOptions, candidate: &CustomTheme) -> bool {
  if !options.background_type.eq_ignore_ascii_case("gradient") {
    return false;
  }
  let a0 = options.custom_theme.color_start.trim();
  let a1 = options.custom_theme.color_end.trim();
  let b0 = candidate.color_start.trim();
  let b1 = candidate.color_end.trim();
  !a0.is_empty() && !a1.is_empty() && a0.eq_ignore_ascii_case(b0) && a1.eq_ignore_ascii_case(b1)
}

pub fn toggle_gradient_off(options: &mut EditorOptions, defaults: Option<&CustomTheme>) {
  let def = defaults.cloned().unwrap_or_default();
  options.background_type = "gradient".to_string();
  options.custom_theme.color_start = def.color_start;
  options.custom_theme.color_end = def.color_end;
  options.wallpaper = None;
  options.wallpaper_source = None;
  options.wallpaper_path = None;
}

pub fn is_same_blob_gradient_selection(options: &EditorOptions, candidate: &BlobGradientConfig) -> bool {
  if !options.background_type.eq_ignore_ascii_case("blobGradient") {
    return false;
  }
  options.blob_gradient.seed == candidate.seed
}

pub fn toggle_blob_gradient_off(options: &mut EditorOptions, defaults: Option<&CustomTheme>) {
  toggle_gradient_off(options, defaults);
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn toggle_helpers_reset_background_cleanly() {
    let mut opts = EditorOptions::default();
    opts.background_type = "wallpaper".to_string();
    opts.wallpaper = Some("/usr/share/backgrounds/test.png".to_string());
    assert!(is_same_wallpaper_selection(&opts, "/usr/share/backgrounds/test.png"));

    toggle_wallpaper_off(&mut opts);
    assert_eq!(opts.background_type, "gradient");
    assert_eq!(opts.wallpaper, None);
  }
}
