use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
use std::process::{Command, Stdio};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use shotify_core::{
  collect_images_recursive, default_os_wallpaper_roots, detect_balanced_crop_rect,
  extract_palette_from_rgba_image, generate_algorithm_gradients, is_wallpaper_image,
  list_system_wallpapers_from_paths, compose_screenshot_scene, BalanceDetectionResult,
  BalanceOptions, EditorOptions, ExtractedPalette, GradientPreset, RenderQuality,
  SystemWallpaperItem,
};
use shotify_export::{
  bytes_to_data_url, composite_watermark_rgba, decode_data_url as decode_raw_data_url,
  encode_rgba_image, execute_export, save_bytes_to_folder, save_bytes_to_path, EncodeOptions,
  ExportDestination, ExportFormat, ExportRequest, RasterizeOptions,
};
use tauri::Manager;
use uuid::Uuid;

#[cfg(target_os = "macos")]
fn macos_default_tmp_dir() -> Option<PathBuf> {
  // Prefer a stable, user-writable tmp dir on the home volume. This avoids
  // intermittent failures writing to system temp locations and aligns with our
  // dev env defaults (see platform/macos/node/tauriEnv.js).
  let home = std::env::var_os("HOME")?;
  Some(
    PathBuf::from(home)
      .join("Library")
      .join("Caches")
      .join("shotstyle")
      .join("tmp"),
  )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadFileAsDataUrlArgs {
  pub file_path: String,
  pub max_dim: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadFileAsAssetArgs {
  pub file_path: String,
  pub max_dim: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadFileAsAssetResult {
  pub file_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageApplyFxBytesArgs {
  pub bytes: Vec<u8>,
  pub blur_px: Option<f32>,
  pub brightness: Option<f32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageApplyFxResult {
  pub file_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCustomBackgroundResult {
  pub ok: bool,
  pub name: Option<String>,
  pub file_path: Option<String>,
  pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCustomBackgroundArgs {
  pub data_url: String,
  pub file_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomBackgroundItem {
  pub name: String,
  pub file_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResult {
  pub ok: bool,
  pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteCustomBackgroundArgs {
  pub file_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveExportArgs {
  pub data_url: String,
  pub file_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveExportResult {
  pub ok: bool,
  pub file_path: Option<String>,
  pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveExportToPathArgs {
  pub data_url: String,
  pub file_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveExportToFolderArgs {
  pub data_url: String,
  pub folder_path: String,
  pub file_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyWatermarkArgs {
  pub base_data_url: String,
  pub watermark_data_url: String,
  pub x: f32,
  pub y: f32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardWriteImageArgs {
  pub data_url: String,
}

pub struct ClipboardState(pub std::sync::Mutex<Option<arboard::Clipboard>>);

impl Default for ClipboardState {
  fn default() -> Self {
    Self(std::sync::Mutex::new(None))
  }
}

fn app_custom_backgrounds_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
  let base = app
    .path()
    .app_data_dir()
    .map_err(|e| format!("Failed to resolve app_data_dir: {e}"))?;
  Ok(base.join("shotify").join("backgrounds"))
}

fn ensure_within_base(base: &Path, target: &Path) -> Result<(), String> {
  let base_abs = base.canonicalize().map_err(|e| format!("Failed to resolve base: {e}"))?;
  let target_abs = target.canonicalize().map_err(|e| format!("Failed to resolve target: {e}"))?;
  if !target_abs.starts_with(&base_abs) {
    return Err("Refusing to delete outside of managed directory".to_string());
  }
  Ok(())
}

pub(crate) fn write_temp_png(bytes: &[u8]) -> Result<PathBuf, String> {
  let mut out_path = std::env::temp_dir();
  #[cfg(target_os = "macos")]
  {
    if let Some(dir) = macos_default_tmp_dir() {
      out_path = dir;
    }
  }
  fs::create_dir_all(&out_path).map_err(|e| format!("Failed to create temp dir: {e}"))?;
  let id = Uuid::new_v4().to_string();
  out_path.push(format!("shotstyle-{id}.png"));
  fs::write(&out_path, bytes).map_err(|e| format!("Failed to write temp image: {e}"))?;
  Ok(out_path)
}

fn write_temp_png_from_image(img: &image::DynamicImage) -> Result<PathBuf, String> {
  let mut out: Vec<u8> = Vec::new();
  img
    .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
    .map_err(|e| format!("Failed to encode temp image: {e}"))?;
  write_temp_png(&out)
}

fn write_png_to_path(img: &image::DynamicImage, out_path: &Path) -> Result<(), String> {
  if let Some(parent) = out_path.parent() {
    fs::create_dir_all(parent).map_err(|e| format!("Failed to create output dir: {e}"))?;
  }
  let mut out: Vec<u8> = Vec::new();
  img
    .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
    .map_err(|e| format!("Failed to encode image: {e}"))?;
  fs::write(out_path, out).map_err(|e| format!("Failed to write output image: {e}"))?;
  Ok(())
}

fn apply_brightness_in_place(img: &mut image::RgbaImage, brightness: f32) {
  let b = brightness.clamp(0.25, 3.0);
  if (b - 1.0).abs() < 0.0001 {
    return;
  }
  for px in img.pixels_mut() {
    let r = (px[0] as f32 * b).round().clamp(0.0, 255.0) as u8;
    let g = (px[1] as f32 * b).round().clamp(0.0, 255.0) as u8;
    let bch = (px[2] as f32 * b).round().clamp(0.0, 255.0) as u8;
    px[0] = r;
    px[1] = g;
    px[2] = bch;
  }
}

fn decode_data_url(data_url: &str) -> Result<(String, Vec<u8>), String> {
  decode_raw_data_url(data_url).map_err(|e| e.to_string())
}

fn decode_data_url_str(data_url: &str) -> Result<(String, Vec<u8>), String> {
  decode_data_url(data_url)
}

fn read_image_bytes_with_retry(path: &Path, max_attempts: u32, delay_ms: u64) -> Result<Vec<u8>, String> {
  let attempts = max_attempts.max(1);
  let mut last_err: Option<String> = None;

  for attempt in 0..attempts {
    match fs::read(path) {
      Ok(bytes) => {
        if bytes.len() < 16 {
          last_err = Some(format!("file too small ({} bytes)", bytes.len()));
        } else if image::load_from_memory(&bytes).is_ok() {
          return Ok(bytes);
        } else {
          last_err = Some("failed to decode image".to_string());
        }
      }
      Err(e) => {
        last_err = Some(format!("Failed to read file: {e}"));
      }
    }

    if attempt + 1 < attempts {
      std::thread::sleep(std::time::Duration::from_millis(delay_ms));
    }
  }

  Err(format!(
    "Failed to read image after {attempts} attempts: {}",
    last_err.unwrap_or_else(|| "unknown error".to_string())
  ))
}

#[tauri::command]
pub async fn read_file_as_asset(app: tauri::AppHandle, args: ReadFileAsAssetArgs) -> Result<ReadFileAsAssetResult, String> {
  let path = PathBuf::from(&args.file_path);
  if !path.exists() {
    return Err("File does not exist".to_string());
  }

  // If no resize is requested, return the original file path.
  let max_dim = args.max_dim.unwrap_or(0).min(8192);
  if max_dim == 0 {
    return Ok(ReadFileAsAssetResult { file_path: path.to_string_lossy().to_string() });
  }

  // Cache resized outputs by (path, mtime, size, max_dim) so UI thumbnail requests don't
  // repeatedly re-decode/re-transcode the same wallpaper when switching tabs.
  fn cache_key_for(path: &Path, max_dim: u32) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.to_string_lossy().to_string().hash(&mut hasher);
    max_dim.hash(&mut hasher);
    if let Ok(meta) = std::fs::metadata(path) {
      meta.len().hash(&mut hasher);
      if let Ok(mtime) = meta.modified() {
        if let Ok(dur) = mtime.duration_since(std::time::UNIX_EPOCH) {
          dur.as_secs().hash(&mut hasher);
          dur.subsec_nanos().hash(&mut hasher);
        }
      }
    }
    format!("{:016x}", hasher.finish())
  }

  fn shotstyle_cache_dir(app: &tauri::AppHandle) -> Option<PathBuf> {
    // Dev/pro override: allow redirecting cache to a different volume.
    // This path is only used for app-generated thumbnails (not user files).
    if let Ok(raw) = std::env::var("SHOTSTYLE_CACHE_DIR") {
      let s = raw.trim();
      if !s.is_empty() {
        let p = PathBuf::from(s);
        if p.is_absolute() {
          if std::fs::create_dir_all(&p).is_ok() {
            return Some(p);
          }
        }
      }
    }
    app.path().app_cache_dir().ok()
  }

  fn cached_png_path(app: &tauri::AppHandle, key: &str) -> Option<PathBuf> {
    let base = shotstyle_cache_dir(app)?;
    Some(base.join("thumb-cache").join(format!("{key}.png")))
  }

  let key = cache_key_for(&path, max_dim);
  if let Some(p) = cached_png_path(&app, &key) {
    if let Ok(meta) = std::fs::metadata(&p) {
      if meta.is_file() && meta.len() > 0 {
        return Ok(ReadFileAsAssetResult { file_path: p.to_string_lossy().to_string() });
      }
    }
  }

  #[cfg(target_os = "macos")]
  {
    // macOS system wallpapers are often HEIC/HEIF. Avoid reading the entire file into memory
    // only to fail decode; use `sips` to rasterize directly.
    let ext = path
      .extension()
      .and_then(|s| s.to_str())
      .unwrap_or("")
      .to_lowercase();
    let is_heic_like = matches!(ext.as_str(), "heic" | "heif" | "avif");

    // macOS dynamic wallpapers can be videos (e.g. .mov). Use QuickLook to extract a still.
    let is_video_like = matches!(ext.as_str(), "mov" | "mp4" | "m4v");
    if is_video_like {
      fn qlmanage_thumbnail_to_png(
        app: &tauri::AppHandle,
        input: &Path,
        max_dim: u32,
        cache_key: &str,
      ) -> Result<PathBuf, String> {
        let cache_path = cached_png_path(app, cache_key)
          .ok_or_else(|| "Failed to resolve cache directory".to_string())?;
        if let Some(parent) = cache_path.parent() {
          fs::create_dir_all(parent).map_err(|e| format!("Failed to create cache dir: {e}"))?;
        }
        if let Ok(meta) = std::fs::metadata(&cache_path) {
          if meta.is_file() && meta.len() > 0 {
            return Ok(cache_path);
          }
        }

        // Write qlmanage outputs next to our cache so rename is cheap and reliable.
        let parent = cache_path
          .parent()
          .ok_or_else(|| "Invalid cache path".to_string())?;
        let out_dir = parent.join("ql-tmp").join(cache_key);
        fs::create_dir_all(&out_dir).map_err(|e| format!("Failed to create qlmanage out dir: {e}"))?;

        let mut tmp_dir = std::env::temp_dir();
        if let Some(dir) = macos_default_tmp_dir() {
          tmp_dir = dir;
        } else if let Some(dir) = shotstyle_cache_dir(app) {
          tmp_dir = dir;
        }
        fs::create_dir_all(&tmp_dir).map_err(|e| format!("Failed to create qlmanage tmp dir: {e}"))?;

        let output = Command::new("qlmanage")
          .env("TMPDIR", &tmp_dir)
          .arg("-t")
          .arg("-f")
          .arg("-s")
          .arg(max_dim.to_string())
          .arg("-o")
          .arg(&out_dir)
          .arg(input)
          .stdout(Stdio::null())
          .output()
          .map_err(|e| format!("Failed to run qlmanage: {e}"))?;

        if !output.status.success() {
          let stderr = String::from_utf8_lossy(&output.stderr);
          let msg = stderr.trim();
          let _ = fs::remove_dir_all(&out_dir);
          if msg.is_empty() {
            return Err("qlmanage thumbnail failed".to_string());
          }
          return Err(format!("qlmanage thumbnail failed: {msg}"));
        }

        // qlmanage writes one or more pngs into out_dir. Pick the first non-empty one.
        let mut picked: Option<PathBuf> = None;
        if let Ok(entries) = std::fs::read_dir(&out_dir) {
          for ent in entries.flatten() {
            let p = ent.path();
            if !p.is_file() {
              continue;
            }
            let ok_ext = p
              .extension()
              .and_then(|s| s.to_str())
              .map(|s| s.eq_ignore_ascii_case("png"))
              .unwrap_or(false);
            if !ok_ext {
              continue;
            }
            if let Ok(meta) = std::fs::metadata(&p) {
              if meta.len() > 0 {
                picked = Some(p);
                break;
              }
            }
          }
        }

        let picked = picked.ok_or_else(|| {
          let _ = fs::remove_dir_all(&out_dir);
          "qlmanage did not produce output".to_string()
        })?;

        fs::rename(&picked, &cache_path)
          .map_err(|e| format!("Failed to move qlmanage output: {e}"))?;

        let _ = fs::remove_dir_all(&out_dir);
        Ok(cache_path)
      }

      let out_path = qlmanage_thumbnail_to_png(&app, &path, max_dim, &key)?;
      return Ok(ReadFileAsAssetResult { file_path: out_path.to_string_lossy().to_string() });
    }
    if is_heic_like {
      fn sips_thumbnail_to_png(app: &tauri::AppHandle, input: &Path, max_dim: u32, cache_key: &str) -> Result<PathBuf, String> {
        let cache_path = cached_png_path(app, cache_key)
          .ok_or_else(|| "Failed to resolve cache directory".to_string())?;
        if let Some(parent) = cache_path.parent() {
          fs::create_dir_all(parent).map_err(|e| format!("Failed to create cache dir: {e}"))?;
        }
        if let Ok(meta) = std::fs::metadata(&cache_path) {
          if meta.is_file() && meta.len() > 0 {
            return Ok(cache_path);
          }
        }

        // `sips` may attempt to write intermediate files into TMPDIR; force it to a writable
        // directory to avoid permission failures.
        let mut tmp_dir = std::env::temp_dir();
        if let Some(dir) = macos_default_tmp_dir() {
          tmp_dir = dir;
        } else if let Some(dir) = shotstyle_cache_dir(app) {
          tmp_dir = dir;
        }
        fs::create_dir_all(&tmp_dir).map_err(|e| format!("Failed to create sips tmp dir: {e}"))?;

        let output = Command::new("sips")
          .env("TMPDIR", &tmp_dir)
          .arg("-Z")
          .arg(max_dim.to_string())
          .arg("-s")
          .arg("format")
          .arg("png")
          .arg(input)
          .arg("--out")
          .arg(&cache_path)
          .stdout(Stdio::null())
          .output()
          .map_err(|e| format!("Failed to run sips: {e}"))?;

        if !output.status.success() {
          let stderr = String::from_utf8_lossy(&output.stderr);
          let msg = stderr.trim();
          if msg.is_empty() {
            return Err("sips conversion failed".to_string());
          }
          return Err(format!("sips conversion failed: {msg}"));
        }
        if !cache_path.exists() {
          return Err("sips did not produce output".to_string());
        }
        Ok(cache_path)
      }

      let out_path = sips_thumbnail_to_png(&app, &path, max_dim, &key)?;
      return Ok(ReadFileAsAssetResult { file_path: out_path.to_string_lossy().to_string() });
    }
  }

  let bytes = fs::read(&path).map_err(|e| format!("Failed to read file: {e}"))?;

  match image::load_from_memory(&bytes) {
    Ok(img) => {
      let resized = img.thumbnail(max_dim, max_dim);
      let out_path = if let Some(cache_path) = cached_png_path(&app, &key) {
        if let Some(parent) = cache_path.parent() {
          fs::create_dir_all(parent).map_err(|e| format!("Failed to create cache dir: {e}"))?;
        }
        write_png_to_path(&resized, &cache_path)?;
        cache_path
      } else {
        write_temp_png_from_image(&resized)?
      };
      Ok(ReadFileAsAssetResult { file_path: out_path.to_string_lossy().to_string() })
    }
    Err(e) => {
      let _ = &e;
      #[cfg(target_os = "macos")]
      {
        // For other macOS decode failures, retry via `sips` into the same stable cache path.
        fn sips_thumbnail_to_png(app: &tauri::AppHandle, input: &Path, max_dim: u32, cache_key: &str) -> Result<PathBuf, String> {
          let cache_path = cached_png_path(app, cache_key)
            .ok_or_else(|| "Failed to resolve cache directory".to_string())?;
          if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create cache dir: {e}"))?;
          }
          if let Ok(meta) = std::fs::metadata(&cache_path) {
            if meta.is_file() && meta.len() > 0 {
              return Ok(cache_path);
            }
          }

          let mut tmp_dir = std::env::temp_dir();
          if let Some(dir) = macos_default_tmp_dir() {
            tmp_dir = dir;
          } else if let Some(dir) = shotstyle_cache_dir(app) {
            tmp_dir = dir;
          }
          fs::create_dir_all(&tmp_dir).map_err(|e| format!("Failed to create sips tmp dir: {e}"))?;

          let output = Command::new("sips")
            .env("TMPDIR", &tmp_dir)
            .arg("-Z")
            .arg(max_dim.to_string())
            .arg("-s")
            .arg("format")
            .arg("png")
            .arg(input)
            .arg("--out")
            .arg(&cache_path)
            .stdout(Stdio::null())
            .output()
            .map_err(|e| format!("Failed to run sips: {e}"))?;

          if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let msg = stderr.trim();
            if msg.is_empty() {
              return Err("sips conversion failed".to_string());
            }
            return Err(format!("sips conversion failed: {msg}"));
          }
          if !cache_path.exists() {
            return Err("sips did not produce output".to_string());
          }
          Ok(cache_path)
        }

        let out_path = sips_thumbnail_to_png(&app, &path, max_dim, &key)?;
        return Ok(ReadFileAsAssetResult { file_path: out_path.to_string_lossy().to_string() });
      }

      #[cfg(target_os = "linux")]
      {
        if let Ok(png_bytes) = crate::platform::linux::rasterize_image_to_png_with_gdk(&path, max_dim) {
          let out_path = if let Some(cache_path) = cached_png_path(&app, &key) {
            if let Some(parent) = cache_path.parent() {
              fs::create_dir_all(parent).map_err(|err| format!("Failed to create cache dir: {err}"))?;
            }
            fs::write(&cache_path, &png_bytes)
              .map_err(|err| format!("Failed to write cached PNG: {err}"))?;
            cache_path
          } else {
            write_temp_png(&png_bytes)?
          };
          return Ok(ReadFileAsAssetResult {
            file_path: out_path.to_string_lossy().to_string(),
          });
        }
      }

      #[allow(unreachable_code)]
      Err(format!("Failed to decode image: {e}"))
    }
  }
}

#[tauri::command]
pub async fn image_apply_fx_bytes(_app: tauri::AppHandle, args: ImageApplyFxBytesArgs) -> Result<ImageApplyFxResult, String> {
  if args.bytes.is_empty() {
    return Err("Missing image bytes".to_string());
  }

  let blur = args.blur_px.unwrap_or(0.0).max(0.0);
  let brightness = args.brightness.unwrap_or(1.0);

  let mut img = image::load_from_memory(&args.bytes)
    .map_err(|e| format!("Failed to decode image: {e}"))?;

  if blur > 0.0001 {
    img = img.blur(blur);
  }

  let mut rgba = img.to_rgba8();
  apply_brightness_in_place(&mut rgba, brightness);
  let dyn_img = image::DynamicImage::ImageRgba8(rgba);
  let out_path = write_temp_png_from_image(&dyn_img)?;

  Ok(ImageApplyFxResult { file_path: out_path.to_string_lossy().to_string() })
}

#[tauri::command]
pub async fn clipboard_write_image_data_url(
  app: tauri::AppHandle,
  clipboard_state: tauri::State<'_, ClipboardState>,
  args: ClipboardWriteImageArgs,
) -> Result<(), String> {
  let (mime, bytes) = decode_data_url(&args.data_url)?;

  let img = image::load_from_memory(&bytes)
    .map_err(|e| format!("Failed to decode image: {e}"))?
    .to_rgba8();

  let (w, h) = img.dimensions();
  let rgba = img.into_raw();

  let arboard_result = (|| -> Result<(), String> {
    let mut guard = clipboard_state
      .0
      .lock()
      .map_err(|_| "Clipboard lock poisoned".to_string())?;
    if guard.is_none() {
      *guard = Some(arboard::Clipboard::new().map_err(|e| format!("Clipboard init failed: {e}"))?);
    }
    let clipboard = guard
      .as_mut()
      .ok_or_else(|| "Clipboard unavailable".to_string())?;
    if let Err(err) = clipboard.set_image(arboard::ImageData {
      width: w as usize,
      height: h as usize,
      bytes: std::borrow::Cow::Owned(rgba.clone()),
    }) {
      *guard = None;
      return Err(format!("Clipboard write failed: {err}"));
    }
    Ok(())
  })();

  if let Err(err) = arboard_result {
    #[cfg(target_os = "linux")]
    {
      let png_bytes = if mime == "image/png" {
        bytes
      } else {
        use image::ImageEncoder;
        let mut out = Vec::new();
        image::codecs::png::PngEncoder::new(&mut out)
          .write_image(&rgba, w, h, image::ExtendedColorType::Rgba8)
          .map_err(|e| format!("Failed to encode clipboard PNG: {e}"))?;
        out
      };
      return crate::platform::linux::clipboard_write_png_fallback(&app, &png_bytes)
        .map_err(|fallback_err| format!("{err}; fallback failed: {fallback_err}"));
    }
    #[cfg(not(target_os = "linux"))]
    {
      let _ = &app;
      let _ = &mime;
      return Err(err);
    }
  }

  Ok(())
}

fn encode_rgba_slice_to_png(rgba: &[u8], w: u32, h: u32) -> Result<Vec<u8>, String> {
  use image::ImageEncoder;
  let mut out = Vec::new();
  image::codecs::png::PngEncoder::new(&mut out)
    .write_image(rgba, w, h, image::ExtendedColorType::Rgba8)
    .map_err(|e| format!("Failed to encode PNG: {e}"))?;
  Ok(out)
}

#[tauri::command]
pub async fn clipboard_read_image_data_url(
  app: tauri::AppHandle,
  clipboard_state: tauri::State<'_, ClipboardState>,
) -> Result<Option<String>, String> {
  enum ArboardOutcome {
    Image { rgba: Vec<u8>, width: u32, height: u32 },
    Text(String),
    None,
  }

  let arboard_res = (|| -> ArboardOutcome {
    let Ok(mut guard) = clipboard_state.0.lock() else {
      return ArboardOutcome::None;
    };
    if guard.is_none() {
      let Ok(cb) = arboard::Clipboard::new() else {
        return ArboardOutcome::None;
      };
      *guard = Some(cb);
    }
    let Some(clipboard) = guard.as_mut() else {
      return ArboardOutcome::None;
    };
    if let Ok(img) = clipboard.get_image() {
      return ArboardOutcome::Image {
        rgba: img.bytes.into_owned(),
        width: img.width as u32,
        height: img.height as u32,
      };
    }
    if let Ok(text) = clipboard.get_text() {
      return ArboardOutcome::Text(text);
    }
    ArboardOutcome::None
  })();

  match arboard_res {
    ArboardOutcome::Image { rgba, width, height } if width > 0 && height > 0 => {
      let png_bytes = encode_rgba_slice_to_png(&rgba, width, height)?;
      return Ok(Some(format!("data:image/png;base64,{}", STANDARD.encode(png_bytes))));
    }
    ArboardOutcome::Text(text) => {
      #[cfg(target_os = "linux")]
      {
        if let Some(path) = crate::platform::linux::decode_clipboard_file_uri_or_path(&text) {
          if let Ok(bytes) = fs::read(&path) {
            if let Ok(img) = image::load_from_memory(&bytes) {
              let rgba = img.to_rgba8();
              let (w, h) = rgba.dimensions();
              let png_bytes = encode_rgba_slice_to_png(rgba.as_raw(), w, h)?;
              return Ok(Some(format!("data:image/png;base64,{}", STANDARD.encode(png_bytes))));
            }
            if let Ok(png_bytes) = crate::platform::linux::rasterize_image_to_png_with_gdk(&path, 0) {
              return Ok(Some(format!("data:image/png;base64,{}", STANDARD.encode(png_bytes))));
            }
          }
        }
      }
      #[cfg(not(target_os = "linux"))]
      {
        let _ = text;
      }
    }
    _ => {}
  }

  #[cfg(target_os = "linux")]
  {
    let app_clone = app.clone();
    let fallback_bytes = tauri::async_runtime::spawn_blocking(move || {
      crate::platform::linux::clipboard_read_image_png_fallback(&app_clone)
    })
    .await
    .map_err(|e| format!("Clipboard read task failed: {e}"))??;

    if let Some(png_bytes) = fallback_bytes {
      return Ok(Some(format!("data:image/png;base64,{}", STANDARD.encode(png_bytes))));
    }
  }

  #[cfg(not(target_os = "linux"))]
  {
    let _ = app;
  }

  Ok(None)
}

#[tauri::command]
pub async fn read_file_as_data_url(_app: tauri::AppHandle, args: ReadFileAsDataUrlArgs) -> Result<String, String> {
  let path = PathBuf::from(&args.file_path);

  let mime = mime_guess::from_path(&path)
    .first_or_octet_stream()
    .to_string();

  // Allow reading arbitrary paths (desktop app behavior). If you want to lock this down later,
  // we can restrict to app_data_dir only.
  let bytes = if mime.starts_with("image/") {
    read_image_bytes_with_retry(&path, 25, 40)?
  } else {
    fs::read(&path).map_err(|e| format!("Failed to read file: {e}"))?
  };

  // Reliability: for images (notably freshly-written screenshots), ensure the file is actually
  // decodable before returning a data URL. Some platforms can observe the path before the writer
  // has flushed all bytes; returning a corrupt data URL would get cached on the JS side.
  if mime.starts_with("image/") {
    image::load_from_memory(&bytes)
      .map_err(|e| format!("Failed to decode image: {e}"))?;
  }

  // Optional thumbnailing for UI (e.g. maxDim: 256)
  if let Some(max_dim) = args.max_dim {
    if max_dim > 0 {
      if let Ok(img) = image::load_from_memory(&bytes) {
        let resized = img.thumbnail(max_dim, max_dim);
        let mut out: Vec<u8> = Vec::new();

        let format = if mime.contains("jpeg") || mime.contains("jpg") {
          image::ImageFormat::Jpeg
        } else if mime.contains("webp") {
          image::ImageFormat::WebP
        } else {
          image::ImageFormat::Png
        };

        if resized
          .write_to(&mut std::io::Cursor::new(&mut out), format)
          .is_ok()
        {
          let out_mime = match format {
            image::ImageFormat::Jpeg => "image/jpeg",
            image::ImageFormat::WebP => "image/webp",
            _ => "image/png",
          };
          return Ok(bytes_to_data_url(out_mime, &out));
        }
      }
    }
  }

  Ok(bytes_to_data_url(&mime, &bytes))
}

#[tauri::command]
pub async fn backgrounds_save_custom(app: tauri::AppHandle, args: SaveCustomBackgroundArgs) -> Result<SaveCustomBackgroundResult, String> {
  let (mime, bytes) = match decode_data_url(&args.data_url) {
    Ok(v) => v,
    Err(e) => {
      return Ok(SaveCustomBackgroundResult {
        ok: false,
        name: None,
        file_path: None,
        error: Some(e),
      })
    }
  };

  let dir = app_custom_backgrounds_dir(&app)?;
  fs::create_dir_all(&dir).map_err(|e| format!("Failed to create dir: {e}"))?;

  let safe_name = args.file_name
    .trim()
    .replace(['/', '\\'], "-")
    .replace(':', "")
    .replace('*', "")
    .replace('?', "")
    .replace('"', "")
    .replace('<', "")
    .replace('>', "")
    .replace('|', "");

  let ext = if safe_name.to_lowercase().ends_with(".png")
    || safe_name.to_lowercase().ends_with(".jpg")
    || safe_name.to_lowercase().ends_with(".jpeg")
    || safe_name.to_lowercase().ends_with(".webp")
  {
    Path::new(&safe_name)
      .extension()
      .and_then(|s| s.to_str())
      .unwrap_or("png")
      .to_string()
  } else if mime.contains("jpeg") {
    "jpg".to_string()
  } else if mime.contains("webp") {
    "webp".to_string()
  } else {
    "png".to_string()
  };

  let stem = Path::new(&safe_name)
    .file_stem()
    .and_then(|s| s.to_str())
    .unwrap_or("background")
    .to_string();

  let mut candidate = dir.join(format!("{stem}.{ext}"));
  if candidate.exists() {
    let ts = chrono::Utc::now().timestamp_millis();
    candidate = dir.join(format!("{stem}-{ts}.{ext}"));
  }

  fs::write(&candidate, bytes).map_err(|e| format!("Failed to write file: {e}"))?;

  Ok(SaveCustomBackgroundResult {
    ok: true,
    name: candidate
      .file_name()
      .and_then(|s| s.to_str())
      .map(|s| s.to_string()),
    file_path: Some(candidate.to_string_lossy().to_string()),
    error: None,
  })
}

#[tauri::command]
pub async fn backgrounds_list_custom(app: tauri::AppHandle) -> Result<Vec<CustomBackgroundItem>, String> {
  let dir = app_custom_backgrounds_dir(&app)?;
  if !dir.exists() {
    return Ok(vec![]);
  }

  let mut out: Vec<CustomBackgroundItem> = vec![];
  for entry in fs::read_dir(&dir).map_err(|e| format!("Failed to read dir: {e}"))? {
    let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
    let path = entry.path();
    if !path.is_file() {
      continue;
    }

    let name = path
      .file_name()
      .and_then(|s| s.to_str())
      .unwrap_or("")
      .to_string();

    if name.is_empty() {
      continue;
    }

    out.push(CustomBackgroundItem {
      name,
      file_path: path.to_string_lossy().to_string(),
    });
  }

  // Sort stable-ish
  out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
  Ok(out)
}

#[tauri::command]
pub async fn backgrounds_delete_custom(app: tauri::AppHandle, args: DeleteCustomBackgroundArgs) -> Result<DeleteResult, String> {
  let dir = app_custom_backgrounds_dir(&app)?;
  let target = PathBuf::from(args.file_path);

  // Safety: only allow deleting inside our managed folder.
  if target.exists() {
    ensure_within_base(&dir, &target)?;
  }

  match fs::remove_file(&target) {
    Ok(_) => Ok(DeleteResult { ok: true, error: None }),
    Err(e) => Ok(DeleteResult { ok: false, error: Some(format!("Failed to delete: {e}")) }),
  }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSystemWallpapersArgs {
  pub max_items: Option<u32>,
}

#[tauri::command]
pub async fn wallpapers_list_system(args: ListSystemWallpapersArgs) -> Result<Vec<SystemWallpaperItem>, String> {
  let max = args.max_items.unwrap_or(300).clamp(1, 500);
  let mut roots = default_os_wallpaper_roots();

  #[cfg(target_os = "windows")]
  {
    if let Ok(output) = std::process::Command::new("reg")
      .args([
        "query",
        r"HKCU\Control Panel\Desktop",
        "/v",
        "WallPaper",
      ])
      .output()
    {
      if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
          if !line.to_lowercase().contains("wallpaper") {
            continue;
          }
          let value = line
            .split("REG_SZ")
            .nth(1)
            .map(|s| s.trim())
            .unwrap_or("");
          if !value.is_empty() {
            let p = PathBuf::from(value);
            if p.is_file() {
              roots.push(p);
            } else if let Some(parent) = p.parent() {
              roots.push(parent.to_path_buf());
            }
          }
        }
      }
    }
  }

  let mut direct_paths: Vec<PathBuf> = vec![];
  #[cfg(target_os = "linux")]
  {
    for extra in crate::platform::linux::detect_extra_wallpaper_paths() {
      if extra.is_file() {
        if is_wallpaper_image(&extra) {
          direct_paths.push(extra);
        }
      } else if extra.is_dir() {
        roots.push(extra);
      }
    }
  }

  Ok(list_system_wallpapers_from_paths(&roots, &direct_paths, max))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreExtractPaletteArgs {
  pub data_url: String,
  pub color_count: Option<usize>,
}

#[tauri::command]
pub async fn core_extract_palette(args: CoreExtractPaletteArgs) -> Result<Option<ExtractedPalette>, String> {
  let (_mime, bytes) = decode_data_url(&args.data_url)?;
  let img = image::load_from_memory(&bytes)
    .map_err(|e| format!("Failed to decode image: {e}"))?
    .to_rgba8();
  Ok(extract_palette_from_rgba_image(&img, args.color_count.unwrap_or(5)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreDetectBalanceArgs {
  pub data_url: String,
}

#[tauri::command]
pub async fn core_detect_balance_crop(args: CoreDetectBalanceArgs) -> Result<BalanceDetectionResult, String> {
  let (_mime, bytes) = decode_data_url(&args.data_url)?;
  let img = image::load_from_memory(&bytes)
    .map_err(|e| format!("Failed to decode image: {e}"))?
    .to_rgba8();
  Ok(detect_balanced_crop_rect(&img, &BalanceOptions::default()))
}

#[tauri::command]
pub async fn core_generate_algorithm_gradients() -> Result<Vec<GradientPreset>, String> {
  Ok(generate_algorithm_gradients())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreComposePreviewArgs {
  pub data_url: String,
  pub wallpaper_data_url: Option<String>,
  pub options: Option<EditorOptions>,
  pub target_width: Option<u32>,
  pub scale: Option<f64>,
}

#[tauri::command]
pub async fn core_compose_preview(args: CoreComposePreviewArgs) -> Result<String, String> {
  let (_mime, bytes) = decode_data_url(&args.data_url)?;
  let screenshot = image::load_from_memory(&bytes)
    .map_err(|e| format!("Failed to decode screenshot image: {e}"))?
    .to_rgba8();

  let wallpaper_img = match args.wallpaper_data_url.as_deref() {
    Some(url) if !url.trim().is_empty() => {
      let (_wm, wbytes) = decode_data_url(url)?;
      Some(
        image::load_from_memory(&wbytes)
          .map_err(|e| format!("Failed to decode wallpaper image: {e}"))?
          .to_rgba8(),
      )
    }
    _ => None,
  };

  let opts = args.options.unwrap_or_default();
  let width = args.target_width.unwrap_or(1000).clamp(64, 4096);
  let scale = args.scale.unwrap_or(1.0).clamp(0.25, 4.0);

  let out = compose_screenshot_scene(
    &screenshot,
    wallpaper_img.as_ref(),
    &opts,
    width,
    scale,
    RenderQuality::Final,
  )
  .map_err(|e| format!("Composition failed: {e}"))?;

  let mut encoded = Vec::new();
  image::DynamicImage::ImageRgba8(out.image)
    .write_to(&mut Cursor::new(&mut encoded), image::ImageFormat::Png)
    .map_err(|e| format!("Failed to encode composed PNG: {e}"))?;

  Ok(bytes_to_data_url("image/png", &encoded))
}

#[cfg(test)]
mod system_wallpapers_tests {
  use super::*;

  fn make_temp_dir(prefix: &str) -> PathBuf {
    let base = std::env::temp_dir();
    let dir = base.join(format!("{prefix}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
  }

  fn touch_file(path: &Path) {
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent).expect("create parent dir");
    }
    std::fs::write(path, b"x").expect("write file");
  }

  #[test]
  fn is_wallpaper_image_accepts_common_formats_and_excludes_thumbs() {
    assert!(is_wallpaper_image(Path::new("/a/b/c.jpg")));
    assert!(is_wallpaper_image(Path::new("/a/b/c.jpeg")));
    assert!(is_wallpaper_image(Path::new("/a/b/c.png")));
    assert!(is_wallpaper_image(Path::new("/a/b/c.webp")));
    assert!(is_wallpaper_image(Path::new("/a/b/c.bmp")));
    assert!(is_wallpaper_image(Path::new("/a/b/c.heic")));
    assert!(is_wallpaper_image(Path::new("/a/b/c.avif")));

    assert!(!is_wallpaper_image(Path::new("/a/b/thumbs/c.jpg")));
    assert!(!is_wallpaper_image(Path::new("/a/b/c_thumb.png")));
    assert!(!is_wallpaper_image(Path::new("/a/b/c.thumb.png")));
    assert!(!is_wallpaper_image(Path::new("/a/b/c.txt")));
  }

  #[test]
  fn collect_images_recursive_respects_max_and_filters() {
    let dir = make_temp_dir("shotstyle-wallpaper-test");

    // Valid
    touch_file(&dir.join("one.jpg"));
    touch_file(&dir.join("sub/two.png"));

    // Invalid / excluded
    touch_file(&dir.join("thumbs/three.jpg"));
    touch_file(&dir.join("four.txt"));

    let mut out: Vec<PathBuf> = vec![];
    collect_images_recursive(&dir, &mut out, 1);
    assert_eq!(out.len(), 1);
    assert!(out[0].to_string_lossy().to_lowercase().ends_with("one.jpg") || out[0].to_string_lossy().to_lowercase().ends_with("two.png"));

    // Now collect with a higher max.
    let mut out2: Vec<PathBuf> = vec![];
    collect_images_recursive(&dir, &mut out2, 10);
    let paths = out2.iter().map(|p| p.to_string_lossy().to_lowercase()).collect::<Vec<_>>();
    assert!(paths.iter().any(|p| p.ends_with("one.jpg")));
    assert!(paths.iter().any(|p| p.ends_with("two.png")));
    assert!(!paths.iter().any(|p| p.contains("thumbs")));
    assert!(!paths.iter().any(|p| p.ends_with("four.txt")));

    let _ = std::fs::remove_dir_all(&dir);
  }
}

#[tauri::command]
pub async fn exports_save_to_downloads(app: tauri::AppHandle, args: SaveExportArgs) -> Result<SaveExportResult, String> {
  let (mime, bytes) = match decode_data_url_str(&args.data_url) {
    Ok(v) => v,
    Err(e) => {
      return Ok(SaveExportResult { ok: false, file_path: None, error: Some(e) })
    }
  };

  let base_dir = app
    .path()
    .download_dir()
    .or_else(|e| {
      #[cfg(target_os = "linux")]
      {
        if let Some(home) = std::env::var_os("HOME") {
          let downloads = PathBuf::from(home).join("Downloads");
          if downloads.exists() {
            return Ok(downloads);
          }
        }
      }
      Err(e)
    })
    .or_else(|_| app.path().desktop_dir())
    .or_else(|_| app.path().app_data_dir())
    .map_err(|e| format!("Failed to resolve output directory: {e}"))?;

  let out_dir = base_dir.join("shotify");
  match save_bytes_to_folder(&bytes, &mime, &out_dir, &args.file_name) {
    Ok(out_path) => Ok(SaveExportResult {
      ok: true,
      file_path: Some(out_path.to_string_lossy().to_string()),
      error: None,
    }),
    Err(e) => Ok(SaveExportResult {
      ok: false,
      file_path: None,
      error: Some(e.to_string()),
    }),
  }
}

#[tauri::command]
pub async fn exports_save_to_path(_app: tauri::AppHandle, args: SaveExportToPathArgs) -> Result<SaveExportResult, String> {
  let (mime, bytes) = match decode_data_url_str(&args.data_url) {
    Ok(v) => v,
    Err(e) => {
      return Ok(SaveExportResult { ok: false, file_path: None, error: Some(e) })
    }
  };

  match save_bytes_to_path(&bytes, &mime, Path::new(&args.file_path)) {
    Ok(out_path) => Ok(SaveExportResult {
      ok: true,
      file_path: Some(out_path.to_string_lossy().to_string()),
      error: None,
    }),
    Err(e) => Ok(SaveExportResult {
      ok: false,
      file_path: None,
      error: Some(e.to_string()),
    }),
  }
}

#[tauri::command]
pub async fn exports_save_to_folder(_app: tauri::AppHandle, args: SaveExportToFolderArgs) -> Result<SaveExportResult, String> {
  let (mime, bytes) = match decode_data_url_str(&args.data_url) {
    Ok(v) => v,
    Err(e) => {
      return Ok(SaveExportResult { ok: false, file_path: None, error: Some(e) })
    }
  };

  match save_bytes_to_folder(&bytes, &mime, Path::new(&args.folder_path), &args.file_name) {
    Ok(out_path) => Ok(SaveExportResult {
      ok: true,
      file_path: Some(out_path.to_string_lossy().to_string()),
      error: None,
    }),
    Err(e) => Ok(SaveExportResult {
      ok: false,
      file_path: None,
      error: Some(e.to_string()),
    }),
  }
}

#[tauri::command]
pub async fn exports_apply_watermark(_app: tauri::AppHandle, args: ApplyWatermarkArgs) -> Result<String, String> {
  let (base_mime, base_bytes) = decode_data_url_str(&args.base_data_url)?;
  let (_wm_mime, wm_bytes) = decode_data_url_str(&args.watermark_data_url)?;

  let mut base = image::load_from_memory(&base_bytes)
    .map_err(|e| format!("Failed to decode base image: {e}"))?
    .to_rgba8();

  let watermark = image::load_from_memory(&wm_bytes)
    .map_err(|e| format!("Failed to decode watermark image: {e}"))?
    .to_rgba8();

  composite_watermark_rgba(&mut base, &watermark, args.x, args.y);

  let format = ExportFormat::from_lossy_or_default(&base_mime);
  let out = encode_rgba_image(&base, format, 0.95).map_err(|e| e.to_string())?;
  Ok(bytes_to_data_url(format.mime_type(), &out))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderAndExportArgs {
  pub data_url: String,
  pub wallpaper_data_url: Option<String>,
  pub options: Option<EditorOptions>,
  pub target_width: Option<u32>,
  pub scale: Option<f64>,
  pub format: Option<String>,
  pub quality: Option<f32>,
  pub file_name: Option<String>,
  pub folder_path: Option<String>,
  pub file_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderAndExportResult {
  pub ok: bool,
  pub data_url: Option<String>,
  pub file_path: Option<String>,
  pub width: u32,
  pub height: u32,
  pub error: Option<String>,
}

#[tauri::command]
pub async fn exports_render_and_save(args: RenderAndExportArgs) -> Result<RenderAndExportResult, String> {
  let (_mime, bytes) = decode_data_url_str(&args.data_url)?;
  let screenshot = image::load_from_memory(&bytes)
    .map_err(|e| format!("Failed to decode screenshot image: {e}"))?
    .to_rgba8();

  let wallpaper = match args.wallpaper_data_url.as_deref() {
    Some(url) if !url.trim().is_empty() => {
      let (_wm, wb) = decode_data_url_str(url)?;
      Some(
        image::load_from_memory(&wb)
          .map_err(|e| format!("Failed to decode wallpaper: {e}"))?
          .to_rgba8(),
      )
    }
    _ => None,
  };

  let opts = args.options.unwrap_or_default();
  let format = ExportFormat::from_lossy_or_default(args.format.as_deref().unwrap_or("png"));
  let quality = args.quality.unwrap_or(0.95);
  let scale = args.scale.unwrap_or(1.0);
  let target_width = args.target_width.unwrap_or(1200).clamp(64, 4096);
  let file_name = args
    .file_name
    .unwrap_or_else(|| shotify_export::build_default_export_file_name(format));

  let destination = if let Some(fp) = args.file_path.filter(|s| !s.trim().is_empty()) {
    ExportDestination::Path { file_path: PathBuf::from(fp) }
  } else if let Some(dir) = args.folder_path.filter(|s| !s.trim().is_empty()) {
    ExportDestination::Folder { folder_path: PathBuf::from(dir), file_name }
  } else {
    ExportDestination::Downloads { file_name }
  };

  match execute_export(
    ExportRequest {
      screenshot: &screenshot,
      wallpaper: wallpaper.as_ref(),
      options: &opts,
      raster: RasterizeOptions {
        target_width,
        scale,
        ..Default::default()
      },
      encode: EncodeOptions {
        format,
        quality,
        ..Default::default()
      },
      destination,
    },
    None,
  ) {
    Ok(outcome) => Ok(RenderAndExportResult {
      ok: true,
      data_url: Some(outcome.to_data_url()),
      file_path: outcome.file_path.map(|p| p.to_string_lossy().to_string()),
      width: outcome.width,
      height: outcome.height,
      error: None,
    }),
    Err(e) => Ok(RenderAndExportResult {
      ok: false,
      data_url: None,
      file_path: None,
      width: 0,
      height: 0,
      error: Some(e.to_string()),
    }),
  }
}

#[tauri::command]
pub async fn exports_render_to_clipboard(
  app: tauri::AppHandle,
  clipboard_state: tauri::State<'_, ClipboardState>,
  args: RenderAndExportArgs,
) -> Result<(), String> {
  let (_mime, bytes) = decode_data_url_str(&args.data_url)?;
  let screenshot = image::load_from_memory(&bytes)
    .map_err(|e| format!("Failed to decode screenshot image: {e}"))?
    .to_rgba8();

  let wallpaper = match args.wallpaper_data_url.as_deref() {
    Some(url) if !url.trim().is_empty() => {
      let (_wm, wb) = decode_data_url_str(url)?;
      Some(
        image::load_from_memory(&wb)
          .map_err(|e| format!("Failed to decode wallpaper: {e}"))?
          .to_rgba8(),
      )
    }
    _ => None,
  };

  let opts = args.options.unwrap_or_default();
  let scale = args.scale.unwrap_or(1.0);
  let target_width = args.target_width.unwrap_or(1200).clamp(64, 4096);

  let outcome = execute_export(
    ExportRequest {
      screenshot: &screenshot,
      wallpaper: wallpaper.as_ref(),
      options: &opts,
      raster: RasterizeOptions {
        target_width,
        scale,
        ..Default::default()
      },
      encode: EncodeOptions {
        format: ExportFormat::Png,
        quality: 1.0,
        ..Default::default()
      },
      destination: ExportDestination::Memory,
    },
    None,
  )
  .map_err(|e| e.to_string())?;

  let data_url = outcome.to_data_url();
  clipboard_write_image_data_url(app, clipboard_state, ClipboardWriteImageArgs { data_url }).await
}
