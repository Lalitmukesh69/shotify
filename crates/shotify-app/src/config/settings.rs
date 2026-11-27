use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use shotify_core::{BlobGradientConfig, CustomTheme};
use shotify_export::ExportFormat;

use crate::theme::ThemeMode;

/// Wallpaper category tabs in the left sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WallpaperTab {
  System,
  #[default]
  Gradients,
  Colors,
  Auto,
}

/// Supported UI locales mirroring Shotify's `i18nStrings.js`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Language {
  #[default]
  En,
  Ko,
  Fr,
  Hi,
  Es,
  ZhHans,
  ZhHant,
}

impl Language {
  pub const ALL: &'static [Language] = &[
    Self::En,
    Self::Ko,
    Self::Fr,
    Self::Hi,
    Self::Es,
    Self::ZhHans,
    Self::ZhHant,
  ];

  pub fn display_name(self) -> &'static str {
    match self {
      Self::En => "English",
      Self::Ko => "한국어 (Korean)",
      Self::Fr => "Français (French)",
      Self::Hi => "हिन्दी (Hindi)",
      Self::Es => "Español (Spanish)",
      Self::ZhHans => "简体中文",
      Self::ZhHant => "繁體中文",
    }
  }

  pub fn parse(raw: &str) -> Self {
    match raw.trim().to_ascii_lowercase().as_str() {
      "ko" | "ko-kr" => Self::Ko,
      "fr" | "fr-fr" => Self::Fr,
      "hi" | "hi-in" => Self::Hi,
      "es" | "es-es" => Self::Es,
      "zh-hans" | "zh-cn" | "zh_cn" => Self::ZhHans,
      "zh-hant" | "zh-tw" | "zh_tw" => Self::ZhHant,
      _ => Self::En,
    }
  }
}

impl std::fmt::Display for Language {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.display_name())
  }
}

/// Active background mode chosen in the UI (mirrors React `backgroundSelection`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BackgroundSelection {
  Gradient {
    preset_id: Option<String>,
    theme: CustomTheme,
  },
  BlobGradient {
    config: BlobGradientConfig,
  },
  Wallpaper {
    file_path: PathBuf,
    name: String,
  },
  SolidColor {
    hex: String,
  },
}

impl Default for BackgroundSelection {
  fn default() -> Self {
    Self::Gradient {
      preset_id: None,
      theme: CustomTheme::default(),
    }
  }
}

/// General behavior, window policy, locale, and appearance preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GeneralSettings {
  pub theme_mode: ThemeMode,
  pub language: Language,
  pub auto_copy_after_capture: bool,
  pub hide_app_when_unfocused: bool,
  pub keep_app_in_tray: bool,
  pub hide_window_on_launch: bool,
  pub open_at_login: bool,
  pub hide_dock_icon_when_hidden: bool,
  pub close_after_copy: bool,
  pub close_after_save: bool,
  pub close_after_save_to_folder: bool,
  pub close_after_drag: bool,
  pub save_folder: Option<PathBuf>,
}

impl Default for GeneralSettings {
  fn default() -> Self {
    Self {
      theme_mode: ThemeMode::Dark,
      language: Language::En,
      auto_copy_after_capture: false,
      hide_app_when_unfocused: !cfg!(target_os = "linux"),
      keep_app_in_tray: true,
      hide_window_on_launch: false,
      open_at_login: false,
      hide_dock_icon_when_hidden: false,
      close_after_copy: false,
      close_after_save: false,
      close_after_save_to_folder: false,
      close_after_drag: false,
      save_folder: None,
    }
  }
}

/// Keyboard shortcut configuration strings matching `shortcuts.js`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ShortcutSettings {
  pub copy: String,
  pub save: String,
  pub save_to_folder: String,
  pub screenshot_region: String,
  pub screenshot_region_copy: String,
}

impl Default for ShortcutSettings {
  fn default() -> Self {
    let mod_key = if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" };
    Self {
      copy: format!("{mod_key}+C"),
      save: format!("{mod_key}+S"),
      save_to_folder: format!("{mod_key}+Shift+S"),
      screenshot_region: format!("{mod_key}+Shift+8"),
      screenshot_region_copy: format!("{mod_key}+Shift+9"),
    }
  }
}

/// Persisted export preferences for footer scale, format, and quality.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ExportPreferences {
  pub scale: f64,
  pub format: ExportFormat,
  pub quality: f64,
}

impl Default for ExportPreferences {
  fn default() -> Self {
    Self {
      scale: 2.0,
      format: ExportFormat::Png,
      quality: 0.92,
    }
  }
}

/// Restorable workspace UI selections saved in `config.toml`.
/// Scalar fields precede tables so `toml` serialization remains cleanly ordered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LastEditorState {
  pub active_preset_id: String,
  pub wallpaper_tab: WallpaperTab,
  pub ratio_choice: String,
  pub custom_ratio_w: u32,
  pub custom_ratio_h: u32,
  pub safari_url: String,
  pub background_selection: BackgroundSelection,
}

impl Default for LastEditorState {
  fn default() -> Self {
    Self {
      active_preset_id: "current".to_string(),
      wallpaper_tab: WallpaperTab::Gradients,
      ratio_choice: "auto".to_string(),
      custom_ratio_w: 16,
      custom_ratio_h: 9,
      safari_url: String::new(),
      background_selection: BackgroundSelection::default(),
    }
  }
}

/// Root TOML application configuration stored at `<config_dir>/shotify-rust/config.toml`.
/// Scalar fields precede nested tables to satisfy strict TOML ordering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
  pub version: u32,
  pub migrated_from_tauri: bool,
  pub general: GeneralSettings,
  pub shortcuts: ShortcutSettings,
  pub export: ExportPreferences,
  pub last_state: LastEditorState,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      version: 1,
      migrated_from_tauri: false,
      general: GeneralSettings::default(),
      shortcuts: ShortcutSettings::default(),
      export: ExportPreferences::default(),
      last_state: LastEditorState::default(),
    }
  }
}

/// Base directory holding `shotify-rust` configuration and user preset TOML files.
pub fn app_config_dir() -> Option<PathBuf> {
  dirs::config_dir().map(|d| d.join("shotify-rust"))
}

pub fn config_toml_path() -> Option<PathBuf> {
  app_config_dir().map(|d| d.join("config.toml"))
}

pub fn presets_dir_path() -> Option<PathBuf> {
  app_config_dir().map(|d| d.join("presets"))
}

pub fn legacy_state_json_path() -> Option<PathBuf> {
  app_config_dir().map(|d| d.join("state.json"))
}

/// Read and deserialize a `Config` TOML file, returning `None` if absent or malformed.
pub fn load_config_from_path(path: &Path) -> Option<Config> {
  let raw = fs::read_to_string(path).ok()?;
  toml::from_str::<Config>(&raw).ok()
}

/// Atomically serialize `Config` as pretty TOML and write it to `path`.
pub fn save_config_to_path(path: &Path, cfg: &Config) -> io::Result<()> {
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)?;
  }
  let encoded = toml::to_string_pretty(cfg)
    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
  let tmp = path.with_extension("toml.tmp");
  fs::write(&tmp, encoded)?;
  fs::rename(&tmp, path)?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn config_roundtrips_cleanly_through_toml() {
    let mut cfg = Config::default();
    cfg.migrated_from_tauri = true;
    cfg.general.theme_mode = ThemeMode::Light;
    cfg.general.language = Language::ZhHans;
    cfg.general.close_after_drag = true;
    cfg.export.scale = 3.0;
    cfg.export.format = ExportFormat::Webp;

    let tmp = std::env::temp_dir().join(format!(
      "shotify-cfg-test-{}.toml",
      chrono::Utc::now().timestamp_nanos_opt().unwrap_or(1)
    ));
    save_config_to_path(&tmp, &cfg).expect("save config TOML");
    let loaded = load_config_from_path(&tmp).expect("reload config TOML");
    assert_eq!(cfg, loaded);
    let _ = fs::remove_file(&tmp);
  }
}
