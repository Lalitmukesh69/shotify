use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use shotify_core::{AspectMode, EditorOptions, FxConfig, ShadowParams, WatermarkConfig};
use shotify_export::ExportFormat;

use super::settings::{BackgroundSelection, WallpaperTab};

/// Full snapshot capturing all 32+ editor styling, layout, annotation, and export fields.
/// Scalar fields strictly precede sub-table fields for TOML compatibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PresetSnapshot {
  // Ratio & layout scalars
  pub ratio_choice: String,
  pub aspect_mode: AspectMode,
  pub custom_w: u32,
  pub custom_h: u32,
  pub padding_size: f64,
  pub inset_padding: f64,
  pub corner_radius: u32,
  pub border_width: f64,
  pub border_color: String,

  // 3D tilt, rotation & position scalars
  pub tilt_x: f64,
  pub tilt_y: f64,
  pub rotation: f64,
  pub offset_x: f64,
  pub offset_y: f64,

  // Browser mockup & balance scalars
  pub browser_bar: String,
  pub browser_url: String,
  pub ui_scale: f64,
  pub balance_enabled: bool,

  // Wallpaper tab & annotation scalars
  pub wallpaper_tab: WallpaperTab,
  pub annotation_color: String,

  // Shadow style scalar
  pub shadow: String,

  // Export settings captured by presets
  pub export_scale: f64,
  pub export_format: ExportFormat,
  pub export_quality: f64,

  // Sub-tables (must come after scalars for TOML serialization)
  pub advanced_shadow: ShadowParams,
  pub watermark: WatermarkConfig,
  pub fx: FxConfig,
  pub background_selection: BackgroundSelection,
}

impl Default for PresetSnapshot {
  fn default() -> Self {
    let opts = EditorOptions::default();
    Self::from_editor_state(
      &opts,
      "auto",
      WallpaperTab::Gradients,
      &BackgroundSelection::default(),
      "#7700FF",
      2.0,
      ExportFormat::Png,
      0.92,
    )
  }
}

impl PresetSnapshot {
  #[allow(clippy::too_many_arguments)]
  pub fn from_editor_state(
    opts: &EditorOptions,
    ratio_choice: &str,
    wallpaper_tab: WallpaperTab,
    bg: &BackgroundSelection,
    annotation_color: &str,
    export_scale: f64,
    export_format: ExportFormat,
    export_quality: f64,
  ) -> Self {
    Self {
      ratio_choice: ratio_choice.to_string(),
      aspect_mode: opts.aspect_mode,
      custom_w: opts.aspect_custom.w.round().clamp(1.0, 4000.0) as u32,
      custom_h: opts.aspect_custom.h.round().clamp(1.0, 4000.0) as u32,
      padding_size: opts.padding_size,
      inset_padding: opts.inset.padding,
      corner_radius: opts.corner_radius,
      border_width: opts.border_width,
      border_color: opts
        .border_color
        .clone()
        .unwrap_or_else(|| "#ffffff".to_string()),
      tilt_x: opts.tilt.x,
      tilt_y: opts.tilt.y,
      rotation: opts.position.rotate,
      offset_x: opts.position.x,
      offset_y: opts.position.y,
      browser_bar: opts.browser_bar.clone(),
      browser_url: opts.browser_url.clone().unwrap_or_default(),
      ui_scale: opts.browser_frame.scale,
      balance_enabled: opts.balance.enabled,
      wallpaper_tab,
      annotation_color: annotation_color.to_string(),
      shadow: opts.shadow.clone(),
      export_scale,
      export_format,
      export_quality,
      advanced_shadow: opts.advanced_shadow.clone(),
      watermark: opts.watermark.clone(),
      fx: opts.fx,
      background_selection: bg.clone(),
    }
  }
}

/// Serializable built-in or user style preset persisted as TOML under `presets/`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preset {
  pub id: String,
  pub name: String,
  pub built_in: bool,
  pub created_at: String,
  pub updated_at: String,
  pub snapshot: PresetSnapshot,
}

impl Default for Preset {
  fn default() -> Self {
    let now = chrono::Utc::now().to_rfc3339();
    Self {
      id: "builtin:default".to_string(),
      name: "Default".to_string(),
      built_in: false,
      created_at: now.clone(),
      updated_at: now,
      snapshot: PresetSnapshot::default(),
    }
  }
}

/// Default preset matching `ui/src/hooks/usePresets.js::applyDefaultPreset`.
pub fn default_preset_definition() -> Preset {
  let ts = "2026-01-01T00:00:00Z".to_string();
  Preset {
    id: "builtin:default".to_string(),
    name: "Default".to_string(),
    built_in: true,
    created_at: ts.clone(),
    updated_at: ts,
    snapshot: PresetSnapshot {
      ratio_choice: "auto".to_string(),
      aspect_mode: AspectMode::Auto,
      padding_size: 32.0,
      inset_padding: 0.0,
      corner_radius: 30,
      border_width: 0.0,
      border_color: "#ffffff".to_string(),
      shadow: "Hug".to_string(),
      browser_bar: "hidden".to_string(),
      wallpaper_tab: WallpaperTab::Gradients,
      export_scale: 2.0,
      export_format: ExportFormat::Png,
      export_quality: 0.92,
      watermark: WatermarkConfig {
        enabled: false,
        ..Default::default()
      },
      ..Default::default()
    },
  }
}

/// Built-in presets exposed in `PresetSelectorTopRight` (`Default` matches React `usePresets.js`).
pub fn builtin_presets() -> Vec<Preset> {
  vec![default_preset_definition()]
}

fn sanitize_preset_filename(id: &str) -> String {
  let clean: String = id
    .chars()
    .map(|c| {
      if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
        c
      } else {
        '_'
      }
    })
    .collect();
  if clean.is_empty() {
    "preset".to_string()
  } else {
    clean
  }
}

pub fn preset_file_path(dir: &Path, id: &str) -> PathBuf {
  dir.join(format!("{}.toml", sanitize_preset_filename(id)))
}

/// Load all user presets stored as `.toml` (or `.json`) files inside `dir`.
pub fn list_user_presets_in_dir(dir: &Path) -> Vec<Preset> {
  let Ok(entries) = fs::read_dir(dir) else {
    return Vec::new();
  };

  let mut out = Vec::new();
  for entry in entries.flatten() {
    let path = entry.path();
    if !path.is_file() {
      continue;
    }
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let parsed = if ext.eq_ignore_ascii_case("toml") {
      fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str::<Preset>(&s).ok())
    } else if ext.eq_ignore_ascii_case("json") {
      fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<Preset>(&s).ok())
    } else {
      None
    };

    if let Some(mut preset) = parsed {
      preset.built_in = false;
      out.push(preset);
    }
  }

  out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
  out
}

/// Save a user preset cleanly as a TOML file inside `dir`.
pub fn save_user_preset_in_dir(dir: &Path, preset: &Preset) -> io::Result<PathBuf> {
  if preset.built_in {
    return Err(io::Error::new(
      io::ErrorKind::InvalidInput,
      "Cannot overwrite built-in preset",
    ));
  }
  fs::create_dir_all(dir)?;
  let target = preset_file_path(dir, &preset.id);
  let encoded = toml::to_string_pretty(preset)
    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
  let tmp = target.with_extension("toml.tmp");
  fs::write(&tmp, encoded)?;
  fs::rename(&tmp, &target)?;
  Ok(target)
}

/// Rename a user preset on disk and return the updated `Preset`.
pub fn rename_user_preset_in_dir(
  dir: &Path,
  id: &str,
  new_name: &str,
) -> io::Result<Option<Preset>> {
  let trimmed = new_name.trim();
  if trimmed.is_empty() {
    return Err(io::Error::new(
      io::ErrorKind::InvalidInput,
      "Preset name cannot be empty",
    ));
  }
  let mut presets = list_user_presets_in_dir(dir);
  let Some(mut found) = presets.drain(..).find(|p| p.id == id) else {
    return Ok(None);
  };
  found.name = trimmed.to_string();
  found.updated_at = chrono::Utc::now().to_rfc3339();
  save_user_preset_in_dir(dir, &found)?;
  Ok(Some(found))
}

/// Delete a user preset `.toml` (or legacy `.json`) file from `dir`.
pub fn delete_user_preset_in_dir(dir: &Path, id: &str) -> io::Result<bool> {
  let toml_path = preset_file_path(dir, id);
  if toml_path.exists() {
    fs::remove_file(toml_path)?;
    return Ok(true);
  }
  let json_path = dir.join(format!("{}.json", sanitize_preset_filename(id)));
  if json_path.exists() {
    fs::remove_file(json_path)?;
    return Ok(true);
  }
  Ok(false)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn all_builtin_presets_serialize_cleanly_to_toml() {
    let presets = builtin_presets();
    assert_eq!(presets.len(), 1);
    assert_eq!(presets[0].name, "Default");
    for p in presets {
      let s = toml::to_string_pretty(&p).expect("builtin preset serializes to TOML");
      let round: Preset = toml::from_str(&s).expect("roundtrips from TOML");
      assert_eq!(p.id, round.id);
      assert_eq!(p.name, round.name);
      assert_eq!(p.snapshot, round.snapshot);
    }
  }

  #[test]
  fn user_preset_crud_lifecycle_works_on_disk() {
    let dir = std::env::temp_dir().join(format!(
      "shotify-presets-crud-{}",
      chrono::Utc::now().timestamp_nanos_opt().unwrap_or(1)
    ));
    let mut p = Preset::default();
    p.id = "user-test-1".to_string();
    p.name = "My Video Theme".to_string();
    p.snapshot.padding_size = 44.0;

    save_user_preset_in_dir(&dir, &p).expect("save user preset");
    let listed = list_user_presets_in_dir(&dir);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "My Video Theme");

    let renamed = rename_user_preset_in_dir(&dir, "user-test-1", "Renamed Theme")
      .expect("rename ok")
      .expect("preset found");
    assert_eq!(renamed.name, "Renamed Theme");
    assert_eq!(list_user_presets_in_dir(&dir)[0].name, "Renamed Theme");

    assert!(delete_user_preset_in_dir(&dir, "user-test-1").expect("delete ok"));
    assert!(list_user_presets_in_dir(&dir).is_empty());
    let _ = fs::remove_dir_all(&dir);
  }
}
