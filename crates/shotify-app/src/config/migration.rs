use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use shotify_core::{BlobGradientConfig, CustomTheme, ShadowParams};
use shotify_export::ExportFormat;

use super::presets::{save_user_preset_in_dir, Preset, PresetSnapshot};
use super::settings::{
  legacy_state_json_path, BackgroundSelection, Config, GeneralSettings, ShortcutSettings,
  WallpaperTab,
};
use crate::theme::ThemeMode;

/// Summary returned by legacy Tauri (`com.shotify.app`) import operations.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LegacyImportReport {
  pub imported_presets: usize,
  pub imported_settings: bool,
  pub imported_shortcuts: bool,
  pub source_description: Option<String>,
}

/// Candidate locations for Tauri WebView localStorage SQLite DBs (`com.shotify.app`).
pub fn discover_legacy_localstorage_files() -> Vec<PathBuf> {
  let mut candidates = Vec::new();

  if let Some(data_dir) = dirs::data_local_dir().or_else(dirs::data_dir) {
    let linux_dir = data_dir.join("com.shotify.app").join("localstorage");
    if let Ok(entries) = fs::read_dir(&linux_dir) {
      for e in entries.flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) == Some("localstorage") {
          candidates.push(p);
        }
      }
    }
  }

  #[cfg(target_os = "macos")]
  if let Some(home) = dirs::home_dir() {
    let mac_dir = home
      .join("Library")
      .join("WebKit")
      .join("com.shotify.app")
      .join("WebsiteData")
      .join("LocalStorage");
    if let Ok(entries) = fs::read_dir(&mac_dir) {
      for e in entries.flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) == Some("localstorage") {
          candidates.push(p);
        }
      }
    }
  }

  candidates
}

// ============================================================================
// Pure-Rust minimal SQLite 3 B-tree reader for WebKit `.localstorage` ItemTable
// ============================================================================

fn read_u16_be(buf: &[u8], off: usize) -> Option<u16> {
  buf.get(off..off + 2).map(|b| u16::from_be_bytes([b[0], b[1]]))
}

fn read_u32_be(buf: &[u8], off: usize) -> Option<u32> {
  buf
    .get(off..off + 4)
    .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// Decode SQLite variable-length integer (up to 9 bytes, returning `(value, bytes_consumed)`).
fn read_varint(buf: &[u8], start: usize) -> Option<(u64, usize)> {
  let mut val = 0_u64;
  for i in 0..9 {
    let b = *buf.get(start + i)?;
    if i == 8 {
      val = (val << 8) | u64::from(b);
      return Some((val, 9));
    }
    val = (val << 7) | u64::from(b & 0x7f);
    if (b & 0x80) == 0 {
      return Some((val, i + 1));
    }
  }
  None
}

#[derive(Debug)]
enum SqliteValue {
  Null,
  Int(i64),
  Blob(Vec<u8>),
  Text(String),
}

struct SqliteFile<'a> {
  data: &'a [u8],
  page_size: usize,
  usable_size: usize,
}

impl<'a> SqliteFile<'a> {
  fn parse(data: &'a [u8]) -> Option<Self> {
    if data.len() < 100 || &data[0..16] != b"SQLite format 3\0" {
      return None;
    }
    let raw_ps = read_u16_be(data, 16)?;
    let page_size = if raw_ps == 1 {
      65_536
    } else {
      usize::from(raw_ps)
    };
    if page_size < 512 || data.len() < page_size {
      return None;
    }
    let reserved = usize::from(*data.get(20)?);
    if reserved >= page_size {
      return None;
    }
    Some(Self {
      data,
      page_size,
      usable_size: page_size - reserved,
    })
  }

  fn page_slice(&self, page_num: u32) -> Option<(&'a [u8], usize)> {
    if page_num == 0 {
      return None;
    }
    let idx = (page_num as usize).checked_sub(1)?;
    let start = idx.checked_mul(self.page_size)?;
    let end = start.checked_add(self.page_size)?;
    let slice = self.data.get(start..end)?;
    let header_off = if page_num == 1 { 100 } else { 0 };
    Some((slice, header_off))
  }

  /// Assemble complete cell payload following overflow page links if payload spills past `max_local`.
  fn read_payload(&self, page: &'a [u8], cell_data_off: usize, payload_len: usize) -> Option<Vec<u8>> {
    let u = self.usable_size;
    let max_local = u.saturating_sub(35);
    let min_local = ((u.saturating_sub(12)) * 32 / 255).saturating_sub(23);

    if payload_len <= max_local {
      return page
        .get(cell_data_off..cell_data_off + payload_len)
        .map(|s| s.to_vec());
    }

    let k = min_local + ((payload_len - min_local) % (u.saturating_sub(4)));
    let inline_len = if k <= max_local { k } else { min_local };
    let mut out = Vec::with_capacity(payload_len);
    out.extend_from_slice(page.get(cell_data_off..cell_data_off + inline_len)?);

    let mut next_overflow = read_u32_be(page, cell_data_off + inline_len)?;
    let mut hops = 0_usize;
    while out.len() < payload_len && next_overflow != 0 && hops < 10_000 {
      hops += 1;
      let (ov_page, _) = self.page_slice(next_overflow)?;
      next_overflow = read_u32_be(ov_page, 0)?;
      let chunk_avail = u.saturating_sub(4);
      let remaining = payload_len - out.len();
      let take = remaining.min(chunk_avail);
      out.extend_from_slice(ov_page.get(4..4 + take)?);
    }

    if out.len() == payload_len {
      Some(out)
    } else {
      None
    }
  }

  fn decode_record(payload: &[u8]) -> Option<Vec<SqliteValue>> {
    let (hdr_len_u64, mut pos) = read_varint(payload, 0)?;
    let hdr_len = usize::try_from(hdr_len_u64).ok()?;
    if hdr_len > payload.len() {
      return None;
    }

    let mut serial_types = Vec::new();
    while pos < hdr_len {
      let (st, used) = read_varint(payload, pos)?;
      pos += used;
      serial_types.push(st);
    }

    let mut data_off = hdr_len;
    let mut values = Vec::with_capacity(serial_types.len());
    for st in serial_types {
      match st {
        0 => values.push(SqliteValue::Null),
        1..=6 => {
          let byte_len = match st {
            1 => 1,
            2 => 2,
            3 => 3,
            4 => 4,
            5 => 6,
            _ => 8,
          };
          let raw = payload.get(data_off..data_off + byte_len)?;
          data_off += byte_len;
          let sign_ext = if (raw[0] & 0x80) != 0 { -1_i64 } else { 0_i64 };
          let mut v = sign_ext;
          for &b in raw {
            v = (v << 8) | i64::from(b);
          }
          values.push(SqliteValue::Int(v));
        }
        7 => {
          data_off += 8;
          values.push(SqliteValue::Null);
        }
        8 => values.push(SqliteValue::Int(0)),
        9 => values.push(SqliteValue::Int(1)),
        n if n >= 12 && (n % 2) == 0 => {
          let len = usize::try_from((n - 12) / 2).ok()?;
          let raw = payload.get(data_off..data_off + len)?;
          data_off += len;
          values.push(SqliteValue::Blob(raw.to_vec()));
        }
        n if n >= 13 && (n % 2) == 1 => {
          let len = usize::try_from((n - 13) / 2).ok()?;
          let raw = payload.get(data_off..data_off + len)?;
          data_off += len;
          values.push(SqliteValue::Text(String::from_utf8_lossy(raw).into_owned()));
        }
        _ => values.push(SqliteValue::Null),
      }
    }
    Some(values)
  }

  fn collect_table_records(&self, root_page: u32, out: &mut Vec<Vec<SqliteValue>>) {
    let mut stack = vec![root_page];
    let mut visited = 0_usize;
    while let Some(pg_num) = stack.pop() {
      visited += 1;
      if visited > 100_000 {
        break;
      }
      let Some((page, hdr_off)) = self.page_slice(pg_num) else {
        continue;
      };
      let Some(&page_type) = page.get(hdr_off) else {
        continue;
      };
      let cell_count = read_u16_be(page, hdr_off + 3).unwrap_or(0) as usize;
      match page_type {
        // Interior table B-tree page
        0x05 => {
          if let Some(right_child) = read_u32_be(page, hdr_off + 8) {
            stack.push(right_child);
          }
          let ptr_start = hdr_off + 12;
          for i in (0..cell_count).rev() {
            if let Some(cell_off) = read_u16_be(page, ptr_start + i * 2).map(usize::from) {
              if let Some(left_child) = read_u32_be(page, cell_off) {
                stack.push(left_child);
              }
            }
          }
        }
        // Leaf table B-tree page
        0x0d => {
          let ptr_start = hdr_off + 8;
          for i in 0..cell_count {
            let Some(cell_off) = read_u16_be(page, ptr_start + i * 2).map(usize::from) else {
              continue;
            };
            let Some((payload_len_u64, u1)) = read_varint(page, cell_off) else {
              continue;
            };
            let Some((_rowid, u2)) = read_varint(page, cell_off + u1) else {
              continue;
            };
            let Ok(payload_len) = usize::try_from(payload_len_u64) else {
              continue;
            };
            if let Some(payload) = self.read_payload(page, cell_off + u1 + u2, payload_len) {
              if let Some(cols) = Self::decode_record(&payload) {
                out.push(cols);
              }
            }
          }
        }
        _ => {}
      }
    }
  }
}

fn decode_webkit_value(val: &SqliteValue) -> Option<String> {
  match val {
    SqliteValue::Text(s) => Some(s.clone()),
    SqliteValue::Blob(bytes) => {
      if bytes.len() >= 2 && (bytes.len() % 2) == 0 {
        let units: Vec<u16> = bytes
          .chunks_exact(2)
          .map(|c| u16::from_le_bytes([c[0], c[1]]))
          .collect();
        Some(String::from_utf16_lossy(&units))
      } else {
        Some(String::from_utf8_lossy(bytes).into_owned())
      }
    }
    _ => None,
  }
}

/// Read `(key -> decoded_string)` entries from WebKit's `ItemTable` inside a `.localstorage` SQLite DB.
pub fn read_sqlite_localstorage_bytes(bytes: &[u8]) -> HashMap<String, String> {
  let mut map = HashMap::new();
  let Some(db) = SqliteFile::parse(bytes) else {
    return map;
  };

  let mut master_rows = Vec::new();
  db.collect_table_records(1, &mut master_rows);

  let mut item_table_root: Option<u32> = None;
  for cols in &master_rows {
    let typ = match cols.first() {
      Some(SqliteValue::Text(s)) => s.as_str(),
      _ => "",
    };
    let name = match cols.get(1) {
      Some(SqliteValue::Text(s)) => s.as_str(),
      _ => "",
    };
    if typ == "table" && name == "ItemTable" {
      if let Some(SqliteValue::Int(root)) = cols.get(3) {
        if *root > 0 {
          item_table_root = Some(*root as u32);
          break;
        }
      }
    }
  }

  let Some(root_page) = item_table_root else {
    return map;
  };

  let mut rows = Vec::new();
  db.collect_table_records(root_page, &mut rows);
  for cols in rows {
    let Some(SqliteValue::Text(key)) = cols.first() else {
      continue;
    };
    if let Some(decoded) = cols.get(1).and_then(decode_webkit_value) {
      map.insert(key.clone(), decoded);
    }
  }
  map
}

// ============================================================================
// Serde bridge types matching legacy Tauri localStorage JSON format
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyGeneralJson {
  hide_app_when_unfocused: Option<bool>,
  keep_app_in_tray: Option<bool>,
  hide_window_on_launch: Option<bool>,
  open_at_login: Option<bool>,
  hide_dock_icon_when_hidden: Option<bool>,
  close_after_copy: Option<bool>,
  close_after_save: Option<bool>,
  close_after_save_to_folder: Option<bool>,
  close_after_drag: Option<bool>,
  auto_copy_after_capture: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyShortcutsJson {
  copy: Option<String>,
  save: Option<String>,
  save_to_folder: Option<String>,
  screenshot_region: Option<String>,
  screenshot_region_copy: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyShadowJson {
  offset_x: Option<f64>,
  offset_y: Option<f64>,
  blur: Option<f64>,
  spread: Option<f64>,
  opacity: Option<f64>,
  color: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyPresetRecord {
  id: Option<String>,
  name: Option<String>,
  snapshot: Option<serde_json::Value>,
}

fn parse_wallpaper_tab(raw: &str) -> WallpaperTab {
  match raw.trim().to_ascii_lowercase().as_str() {
    "system" => WallpaperTab::System,
    "colors" => WallpaperTab::Colors,
    "image" | "auto" => WallpaperTab::Auto,
    _ => WallpaperTab::Gradients,
  }
}

fn parse_legacy_background(val: &serde_json::Value) -> Option<BackgroundSelection> {
  let obj = val.as_object()?;
  let typ = obj.get("type")?.as_str()?;
  match typ {
    "wallpaper" => {
      let wp = obj.get("wallpaper")?.as_object()?;
      let fp = wp.get("filePath")?.as_str()?;
      if fp.starts_with("data:") {
        return None;
      }
      let name = wp
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Wallpaper")
        .to_string();
      Some(BackgroundSelection::Wallpaper {
        file_path: PathBuf::from(fp),
        name,
      })
    }
    "color" => {
      let hex = obj.get("color")?.get("hex")?.as_str()?.to_string();
      Some(BackgroundSelection::SolidColor { hex })
    }
    "blobGradient" => {
      let cfg: BlobGradientConfig = serde_json::from_value(obj.get("blobGradient")?.clone()).ok()?;
      Some(BackgroundSelection::BlobGradient { config: cfg })
    }
    "gradient" => {
      let preset_id = obj
        .get("gradientPresetId")
        .and_then(|v| v.as_str())
        .map(String::from);
      let g = obj.get("gradient")?;
      let theme = CustomTheme {
        color_start: g
          .get("colorStart")
          .and_then(|v| v.as_str())
          .unwrap_or("#7C3AED")
          .to_string(),
        color_end: g
          .get("colorEnd")
          .and_then(|v| v.as_str())
          .unwrap_or("#EC4899")
          .to_string(),
        angle_deg: g.get("angleDeg").and_then(|v| v.as_f64()).unwrap_or(135.0),
      };
      Some(BackgroundSelection::Gradient { preset_id, theme })
    }
    _ => None,
  }
}

fn parse_legacy_preset_snapshot(val: &serde_json::Value) -> PresetSnapshot {
  let mut snap = PresetSnapshot::default();
  let Some(obj) = val.as_object() else {
    return snap;
  };
  if let Some(rc) = obj.get("ratioChoice").and_then(|v| v.as_str()) {
    snap.ratio_choice = rc.to_string();
    snap.aspect_mode = match rc {
      "auto" => shotify_core::AspectMode::Auto,
      "custom" => shotify_core::AspectMode::Custom,
      _ => shotify_core::AspectMode::Preset,
    };
  }
  if let Some(cr) = obj.get("customRatio").and_then(|v| v.as_object()) {
    snap.custom_w = cr.get("w").and_then(|v| v.as_u64()).unwrap_or(16).clamp(1, 4000) as u32;
    snap.custom_h = cr.get("h").and_then(|v| v.as_u64()).unwrap_or(9).clamp(1, 4000) as u32;
  }
  if let Some(v) = obj.get("padding").and_then(|v| v.as_f64()) {
    snap.padding_size = v;
  }
  if let Some(v) = obj.get("inset").and_then(|v| v.as_f64()) {
    snap.inset_padding = v;
  }
  if let Some(v) = obj.get("curve").and_then(|v| v.as_u64()) {
    snap.corner_radius = v.min(64) as u32;
  }
  if let Some(v) = obj.get("border").and_then(|v| v.as_f64()) {
    snap.border_width = v;
  }
  if let Some(v) = obj.get("borderColor").and_then(|v| v.as_str()) {
    snap.border_color = v.to_string();
  }
  if let Some(v) = obj.get("tiltX").and_then(|v| v.as_f64()) {
    snap.tilt_x = v;
  }
  if let Some(v) = obj.get("tiltY").and_then(|v| v.as_f64()) {
    snap.tilt_y = v;
  }
  if let Some(v) = obj.get("rotation").and_then(|v| v.as_f64()) {
    snap.rotation = v;
  }
  if let Some(tr) = obj.get("transform").and_then(|v| v.as_object()) {
    snap.offset_x = tr.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
    snap.offset_y = tr.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
  }
  if let Some(v) = obj.get("uiScale").and_then(|v| v.as_f64()) {
    snap.ui_scale = v;
  }
  if let Some(mockup) = obj.get("selectedOsMockup").and_then(|v| v.as_str()) {
    snap.browser_bar = match mockup {
      "mac-1" => "mac-light",
      "mac-2" => "mac-dark",
      "win-2" => "windows-light",
      "win-1" => "windows-dark",
      "safari-1" => "safari-dark",
      _ => "hidden",
    }
    .to_string();
  }
  if let Some(v) = obj.get("safariMockupText").and_then(|v| v.as_str()) {
    snap.browser_url = v.trim().to_string();
  }
  if let Some(v) = obj.get("wallpaperType").and_then(|v| v.as_str()) {
    snap.wallpaper_tab = parse_wallpaper_tab(v);
  }
  if let Some(bg) = obj.get("backgroundSelection").and_then(parse_legacy_background) {
    snap.background_selection = bg;
  }
  if let Some(v) = obj.get("autoCropEnabled").and_then(|v| v.as_bool()) {
    snap.balance_enabled = v;
  }
  if let Some(v) = obj.get("selectedShadowStyle").and_then(|v| v.as_str()) {
    snap.shadow = if v == "Realistic" { "Heavy" } else { v }.to_string();
  }
  if let Some(cs) = obj
    .get("customShadow")
    .and_then(|v| serde_json::from_value::<LegacyShadowJson>(v.clone()).ok())
  {
    snap.advanced_shadow = ShadowParams {
      enabled: snap.shadow == "Custom",
      offset_x: cs.offset_x.unwrap_or(0.0),
      offset_y: cs.offset_y.unwrap_or(10.0),
      blur: cs.blur.unwrap_or(30.0),
      spread: cs.spread.unwrap_or(0.0),
      opacity: cs.opacity.unwrap_or(0.35),
      color: cs.color.unwrap_or_else(|| "#000000".to_string()),
    };
  }
  if let Some(v) = obj.get("annotationColor").and_then(|v| v.as_str()) {
    snap.annotation_color = v.to_string();
  }
  if let Some(v) = obj.get("watermarkEnabled").and_then(|v| v.as_bool()) {
    snap.watermark.enabled = v;
  }
  if let Some(v) = obj.get("watermarkText").and_then(|v| v.as_str()) {
    snap.watermark.text = v.to_string();
  }
  if let Some(v) = obj.get("watermarkPosition").and_then(|v| v.as_str()) {
    snap.watermark.position = v.to_string();
  }
  if let Some(v) = obj.get("watermarkPrefix").and_then(|v| v.as_str()) {
    snap.watermark.prefix = if v == "none" { None } else { Some(v.to_string()) };
  }
  if let Some(v) = obj.get("watermarkOffsetX").and_then(|v| v.as_f64()) {
    snap.watermark.offset_x = v;
  }
  if let Some(v) = obj.get("watermarkOffsetY").and_then(|v| v.as_f64()) {
    snap.watermark.offset_y = v;
  }
  if let Some(v) = obj.get("watermarkShowTwitter").and_then(|v| v.as_bool()) {
    snap.watermark.show_twitter = v;
  }
  if let Some(v) = obj.get("watermarkShowVerified").and_then(|v| v.as_bool()) {
    snap.watermark.show_verified = v;
  }
  if let Some(v) = obj.get("exportScale").and_then(|v| v.as_f64()) {
    snap.export_scale = v.clamp(0.5, 4.0);
  }
  if let Some(v) = obj.get("exportFormat").and_then(|v| v.as_str()) {
    if let Ok(fmt) = ExportFormat::parse(v) {
      snap.export_format = fmt;
    }
  }
  if let Some(v) = obj.get("exportQuality").and_then(|v| v.as_f64()) {
    snap.export_quality = v.clamp(0.1, 1.0);
  }
  snap
}

/// Apply key-value localStorage pairs into `Config` and write user presets into `presets_dir`.
pub fn apply_localstorage_kv_to_config(
  kv: &HashMap<String, String>,
  cfg: &mut Config,
  presets_dir: Option<&Path>,
) -> LegacyImportReport {
  let mut report = LegacyImportReport::default();

  if let Some(raw) = kv.get("settings.general") {
    if let Ok(lg) = serde_json::from_str::<LegacyGeneralJson>(raw) {
      if let Some(v) = lg.hide_app_when_unfocused {
        cfg.general.hide_app_when_unfocused = v;
      }
      if let Some(v) = lg.keep_app_in_tray {
        cfg.general.keep_app_in_tray = v;
      }
      if let Some(v) = lg.hide_window_on_launch {
        cfg.general.hide_window_on_launch = v;
      }
      if let Some(v) = lg.open_at_login {
        cfg.general.open_at_login = v;
      }
      if let Some(v) = lg.hide_dock_icon_when_hidden {
        cfg.general.hide_dock_icon_when_hidden = v;
      }
      if let Some(v) = lg.close_after_copy {
        cfg.general.close_after_copy = v;
      }
      if let Some(v) = lg.close_after_save {
        cfg.general.close_after_save = v;
      }
      if let Some(v) = lg.close_after_save_to_folder {
        cfg.general.close_after_save_to_folder = v;
      }
      if let Some(v) = lg.close_after_drag {
        cfg.general.close_after_drag = v;
      }
      if let Some(v) = lg.auto_copy_after_capture {
        cfg.general.auto_copy_after_capture = v;
      }
      report.imported_settings = true;
    }
  }

  if let Some(raw) = kv.get("settings.autoHideEnabled") {
    if let Ok(v) = serde_json::from_str::<bool>(raw) {
      cfg.general.hide_app_when_unfocused = v;
      report.imported_settings = true;
    }
  }

  if let Some(raw) = kv.get("settings.language").or_else(|| kv.get("i18n.language")) {
    let clean = raw.trim().trim_matches('"');
    if !clean.is_empty() {
      cfg.general.language = super::settings::Language::parse(clean);
      report.imported_settings = true;
    }
  }

  if let Some(raw) = kv.get("settings.shortcuts") {
    if let Ok(ls) = serde_json::from_str::<LegacyShortcutsJson>(raw) {
      if let Some(v) = ls.copy {
        cfg.shortcuts.copy = v;
      }
      if let Some(v) = ls.save {
        cfg.shortcuts.save = v;
      }
      if let Some(v) = ls.save_to_folder {
        cfg.shortcuts.save_to_folder = v;
      }
      if let Some(v) = ls.screenshot_region {
        cfg.shortcuts.screenshot_region = v;
      }
      if let Some(v) = ls.screenshot_region_copy {
        cfg.shortcuts.screenshot_region_copy = v;
      }
      report.imported_shortcuts = true;
    }
  }

  if let Some(raw) = kv.get("ui-exp:export-scale") {
    if let Ok(s) = serde_json::from_str::<f64>(raw) {
      cfg.export.scale = s.clamp(0.5, 4.0);
    }
  }
  if let Some(raw) = kv.get("ui-exp:export-format") {
    let clean = raw.trim().trim_matches('"');
    if let Ok(fmt) = ExportFormat::parse(clean) {
      cfg.export.format = fmt;
    }
  }
  if let Some(raw) = kv.get("ui-exp:export-quality") {
    if let Ok(q) = serde_json::from_str::<f64>(raw) {
      cfg.export.quality = q.clamp(0.1, 1.0);
    }
  }
  if let Some(raw) = kv.get("uiExp.safariMockupText") {
    let clean = raw.trim().trim_matches('"').to_string();
    cfg.last_state.safari_url = clean;
  }
  if let Some(raw) = kv.get("ui-exp:ratio") {
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(raw) {
      if let Some(rc) = val.get("value").and_then(|v| v.as_str()) {
        cfg.last_state.ratio_choice = rc.to_string();
      }
      if let Some(c) = val.get("custom") {
        cfg.last_state.custom_ratio_w = c.get("w").and_then(|v| v.as_u64()).unwrap_or(16).clamp(1, 4000) as u32;
        cfg.last_state.custom_ratio_h = c.get("h").and_then(|v| v.as_u64()).unwrap_or(9).clamp(1, 4000) as u32;
      }
    }
  }
  if let Some(raw) = kv.get("uiExp.backgroundState") {
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(raw) {
      if let Some(wt) = val.get("wallpaperType").and_then(|v| v.as_str()) {
        cfg.last_state.wallpaper_tab = parse_wallpaper_tab(wt);
      }
      if let Some(bg) = val.get("backgroundSelection").and_then(parse_legacy_background) {
        cfg.last_state.background_selection = bg;
      }
    }
  }

  if let (Some(raw), Some(dir)) = (kv.get("uiExp.userPresets"), presets_dir) {
    if let Ok(records) = serde_json::from_str::<Vec<LegacyPresetRecord>>(raw) {
      let now = chrono::Utc::now().to_rfc3339();
      for rec in records {
        let Some(name) = rec.name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) else {
          continue;
        };
        let id = rec
          .id
          .filter(|s| !s.trim().is_empty())
          .unwrap_or_else(|| format!("imported-{}", uuid::Uuid::new_v4()));
        let snapshot = rec
          .snapshot
          .as_ref()
          .map(parse_legacy_preset_snapshot)
          .unwrap_or_default();
        let preset = Preset {
          id,
          name,
          built_in: false,
          created_at: now.clone(),
          updated_at: now.clone(),
          snapshot,
        };
        if save_user_preset_in_dir(dir, &preset).is_ok() {
          report.imported_presets += 1;
        }
      }
    }
  }

  report
}

/// Phase 3 `state.json` fallback schema so early Shotify Rust builds migrate seamlessly into TOML + `presets/`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyStateJson {
  general: Option<LegacyPhase3General>,
  shortcuts: Option<ShortcutSettings>,
  user_presets: Option<Vec<LegacyPhase3Preset>>,
  export_scale: Option<f64>,
  export_format: Option<ExportFormat>,
  export_quality: Option<f64>,
  last_background: Option<BackgroundSelection>,
  last_wallpaper_tab: Option<WallpaperTab>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyPhase3General {
  theme_mode: Option<ThemeMode>,
  auto_copy_after_capture: Option<bool>,
  hide_app_when_unfocused: Option<bool>,
  keep_app_in_tray: Option<bool>,
  open_at_login: Option<bool>,
  close_after_copy: Option<bool>,
  close_after_save: Option<bool>,
  close_after_save_to_folder: Option<bool>,
  save_folder: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyPhase3Preset {
  id: String,
  name: String,
  snapshot: PresetSnapshot,
}

/// Import any Phase 3 `state.json` AND/OR legacy Tauri `.localstorage` into `cfg` + `presets_dir`.
pub fn import_legacy_sources(
  cfg: &mut Config,
  presets_dir: Option<&Path>,
  state_json_path: Option<&Path>,
  localstorage_files: &[PathBuf],
) -> LegacyImportReport {
  let mut combined = LegacyImportReport::default();

  // First import legacy Tauri localStorage (WebKitGTK / WKWebView SQLite ItemTable)
  for db_path in localstorage_files {
    let Ok(bytes) = fs::read(db_path) else {
      continue;
    };
    let kv = read_sqlite_localstorage_bytes(&bytes);
    if kv.is_empty() {
      continue;
    }
    let rep = apply_localstorage_kv_to_config(&kv, cfg, presets_dir);
    combined.imported_presets += rep.imported_presets;
    combined.imported_settings |= rep.imported_settings;
    combined.imported_shortcuts |= rep.imported_shortcuts;
    if combined.source_description.is_none() {
      combined.source_description = Some(db_path.display().to_string());
    }
  }

  // Then layer Phase 3 state.json if present (so recent Rust edits win over older Tauri defaults)
  if let Some(sp) = state_json_path {
    if let Ok(raw) = fs::read_to_string(sp) {
      if let Ok(legacy) = serde_json::from_str::<LegacyStateJson>(&raw) {
        if let Some(g) = legacy.general {
          if let Some(v) = g.theme_mode {
            cfg.general.theme_mode = v;
          }
          if let Some(v) = g.auto_copy_after_capture {
            cfg.general.auto_copy_after_capture = v;
          }
          if let Some(v) = g.hide_app_when_unfocused {
            cfg.general.hide_app_when_unfocused = v;
          }
          if let Some(v) = g.keep_app_in_tray {
            cfg.general.keep_app_in_tray = v;
          }
          if let Some(v) = g.open_at_login {
            cfg.general.open_at_login = v;
          }
          if let Some(v) = g.close_after_copy {
            cfg.general.close_after_copy = v;
          }
          if let Some(v) = g.close_after_save {
            cfg.general.close_after_save = v;
          }
          if let Some(v) = g.close_after_save_to_folder {
            cfg.general.close_after_save_to_folder = v;
          }
          if g.save_folder.is_some() {
            cfg.general.save_folder = g.save_folder;
          }
          combined.imported_settings = true;
        }
        if let Some(sc) = legacy.shortcuts {
          cfg.shortcuts = sc;
          combined.imported_shortcuts = true;
        }
        if let Some(s) = legacy.export_scale {
          cfg.export.scale = s;
        }
        if let Some(f) = legacy.export_format {
          cfg.export.format = f;
        }
        if let Some(q) = legacy.export_quality {
          cfg.export.quality = q;
        }
        if let Some(bg) = legacy.last_background {
          cfg.last_state.background_selection = bg;
        }
        if let Some(tab) = legacy.last_wallpaper_tab {
          cfg.last_state.wallpaper_tab = tab;
        }
        if let (Some(ups), Some(dir)) = (legacy.user_presets, presets_dir) {
          let now = chrono::Utc::now().to_rfc3339();
          for p in ups {
            let preset = Preset {
              id: p.id,
              name: p.name,
              built_in: false,
              created_at: now.clone(),
              updated_at: now.clone(),
              snapshot: p.snapshot,
            };
            if save_user_preset_in_dir(dir, &preset).is_ok() {
              combined.imported_presets += 1;
            }
          }
        }
      }
    }
  }

  let _ = GeneralSettings::default();
  let _ = legacy_state_json_path();
  combined
}

#[cfg(test)]
mod tests {
  use super::*;

  fn encode_varint(v: u64) -> Vec<u8> {
    if v < 0x80 {
      return vec![v as u8];
    }
    let mut parts = Vec::new();
    let mut rem = v;
    parts.push((rem & 0x7f) as u8);
    rem >>= 7;
    while rem > 0 {
      parts.push(((rem & 0x7f) as u8) | 0x80);
      rem >>= 7;
    }
    parts.reverse();
    parts
  }

  fn build_sqlite_record(cols: &[(&[u8], bool)]) -> Vec<u8> {
    let mut header_body = Vec::new();
    let mut body = Vec::new();
    for &(bytes, is_text) in cols {
      let st = if is_text {
        (bytes.len() as u64) * 2 + 13
      } else {
        (bytes.len() as u64) * 2 + 12
      };
      header_body.extend(encode_varint(st));
      body.extend_from_slice(bytes);
    }
    let total_hdr_len = 1 + header_body.len();
    let mut rec = encode_varint(total_hdr_len as u64);
    rec.extend(header_body);
    rec.extend(body);
    rec
  }

  fn write_leaf_page(buf: &mut [u8], header_off: usize, cells: &[(u64, &[u8])]) {
    buf[header_off] = 0x0d; // leaf table page
    let count = cells.len() as u16;
    buf[header_off + 3..header_off + 5].copy_from_slice(&count.to_be_bytes());

    let mut write_end = buf.len();
    for (idx, &(rowid, payload)) in cells.iter().enumerate() {
      let mut cell = encode_varint(payload.len() as u64);
      cell.extend(encode_varint(rowid));
      cell.extend_from_slice(payload);
      write_end -= cell.len();
      buf[write_end..write_end + cell.len()].copy_from_slice(&cell);
      let ptr_pos = header_off + 8 + idx * 2;
      buf[ptr_pos..ptr_pos + 2].copy_from_slice(&(write_end as u16).to_be_bytes());
    }
  }

  #[test]
  fn parses_webkit_sqlite_localstorage_and_imports_presets() {
    let page_size = 1024_usize;
    let mut db = vec![0_u8; page_size * 2];
    db[0..16].copy_from_slice(b"SQLite format 3\0");
    db[16..18].copy_from_slice(&(page_size as u16).to_be_bytes());

    // Page 1: sqlite_master pointing ItemTable -> root page 2 (serial type 1 int = 1 byte)
    let mut master_rec = Vec::new();
    let t_text = b"table";
    let name_text = b"ItemTable";
    let mut hdr = Vec::new();
    hdr.extend(encode_varint((t_text.len() as u64) * 2 + 13));
    hdr.extend(encode_varint((name_text.len() as u64) * 2 + 13));
    hdr.extend(encode_varint((name_text.len() as u64) * 2 + 13));
    hdr.push(1); // 1-byte signed int rootpage
    hdr.push(13); // empty sql text
    master_rec.push((1 + hdr.len()) as u8);
    master_rec.extend(hdr);
    master_rec.extend_from_slice(t_text);
    master_rec.extend_from_slice(name_text);
    master_rec.extend_from_slice(name_text);
    master_rec.push(2); // root page = 2

    write_leaf_page(&mut db[0..page_size], 100, &[(1, &master_rec)]);

    // Page 2: ItemTable rows with UTF-16LE JSON blobs
    let to_u16 = |s: &str| -> Vec<u8> {
      s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
    };
    let shortcuts_val = to_u16(r#"{"copy":"Alt+C","screenshotRegion":"Ctrl+Shift+7"}"#);
    let presets_val = to_u16(
      r#"[{"id":"u1","name":"Tauri Dark Card","snapshot":{"padding":56,"curve":18,"selectedShadowStyle":"Heavy"}}]"#,
    );
    let row1 = build_sqlite_record(&[(b"settings.shortcuts", true), (&shortcuts_val, false)]);
    let row2 = build_sqlite_record(&[(b"uiExp.userPresets", true), (&presets_val, false)]);

    write_leaf_page(&mut db[page_size..page_size * 2], 0, &[(1, &row1), (2, &row2)]);

    let kv = read_sqlite_localstorage_bytes(&db);
    assert!(kv.contains_key("settings.shortcuts"));
    assert!(kv.contains_key("uiExp.userPresets"));

    let mut cfg = Config::default();
    let pdir = std::env::temp_dir().join(format!(
      "shotify-migrate-test-{}",
      chrono::Utc::now().timestamp_nanos_opt().unwrap_or(1)
    ));
    let report = apply_localstorage_kv_to_config(&kv, &mut cfg, Some(&pdir));
    assert!(report.imported_shortcuts);
    assert_eq!(report.imported_presets, 1);
    assert_eq!(cfg.shortcuts.copy, "Alt+C");
    assert_eq!(cfg.shortcuts.screenshot_region, "Ctrl+Shift+7");

    let saved = super::super::presets::list_user_presets_in_dir(&pdir);
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].name, "Tauri Dark Card");
    assert_eq!(saved[0].snapshot.padding_size, 56.0);
    assert_eq!(saved[0].snapshot.shadow, "Heavy");

    let _ = fs::remove_dir_all(&pdir);
  }
}
