pub mod migration;
pub mod presets;
pub mod settings;

use std::io;
use std::path::PathBuf;

pub use migration::{
  discover_legacy_localstorage_files, import_legacy_sources, LegacyImportReport,
};
#[cfg(test)]
pub use presets::default_preset_definition;
pub use presets::{
  builtin_presets, delete_user_preset_in_dir, list_user_presets_in_dir, rename_user_preset_in_dir,
  save_user_preset_in_dir, Preset, PresetSnapshot,
};
pub use settings::{
  app_config_dir, config_toml_path, presets_dir_path, BackgroundSelection, Config, Language,
  ShortcutSettings, WallpaperTab,
};
use settings::{legacy_state_json_path, load_config_from_path, save_config_to_path};

/// Initialize `Config` from `<config_dir>/shotify-rust/config.toml` and load user presets
/// from `<config_dir>/shotify-rust/presets/`. On first run, automatically imports any
/// legacy Tauri (`com.shotify.app`) localStorage settings & presets (or Phase 3 `state.json`).
pub fn load_or_initialize_config() -> (Config, Vec<Preset>, Option<LegacyImportReport>) {
  if cfg!(test) {
    return (Config::default(), Vec::new(), None);
  }

  let cfg_path = config_toml_path();
  let p_dir = presets_dir_path();

  let mut cfg = cfg_path
    .as_deref()
    .and_then(load_config_from_path)
    .unwrap_or_default();

  let mut import_report = None;
  if !cfg.migrated_from_tauri {
    let legacy_dbs = discover_legacy_localstorage_files();
    let state_json = legacy_state_json_path();
    let report = import_legacy_sources(
      &mut cfg,
      p_dir.as_deref(),
      state_json.as_deref(),
      &legacy_dbs,
    );
    cfg.migrated_from_tauri = true;
    if let Some(ref p) = cfg_path {
      let _ = save_config_to_path(p, &cfg);
    }
    if report.imported_presets > 0 || report.imported_settings || report.imported_shortcuts {
      import_report = Some(report);
    }
  } else if cfg_path.as_ref().is_some_and(|p| !p.exists()) {
    if let Some(ref p) = cfg_path {
      let _ = save_config_to_path(p, &cfg);
    }
  }

  let user_presets = p_dir
    .as_deref()
    .map(list_user_presets_in_dir)
    .unwrap_or_default();

  (cfg, user_presets, import_report)
}

/// Persist root `Config` as TOML (no-op inside unit tests).
pub fn persist_config(cfg: &Config) {
  if cfg!(test) {
    return;
  }
  if let Some(path) = config_toml_path() {
    let _ = save_config_to_path(&path, cfg);
  }
}

/// Persist a user preset TOML file under `presets/`.
pub fn persist_user_preset(preset: &Preset) -> io::Result<Option<PathBuf>> {
  if cfg!(test) {
    return Ok(None);
  }
  let Some(dir) = presets_dir_path() else {
    return Ok(None);
  };
  save_user_preset_in_dir(&dir, preset).map(Some)
}

/// Rename a user preset TOML file on disk.
pub fn rename_persisted_user_preset(id: &str, new_name: &str) -> io::Result<Option<Preset>> {
  if cfg!(test) {
    return Ok(None);
  }
  let Some(dir) = presets_dir_path() else {
    return Ok(None);
  };
  rename_user_preset_in_dir(&dir, id, new_name)
}

/// Delete a user preset TOML file on disk.
pub fn delete_persisted_user_preset(id: &str) -> io::Result<bool> {
  if cfg!(test) {
    return Ok(true);
  }
  let Some(dir) = presets_dir_path() else {
    return Ok(false);
  };
  delete_user_preset_in_dir(&dir, id)
}
