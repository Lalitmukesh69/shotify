use std::fs;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use crate::error::{ExportError, Result};

/// Sanitize a user-supplied filename stem cleanly across Windows/macOS/Linux.
pub fn sanitize_file_stem(name: &str) -> String {
  let trimmed = name.trim();
  if trimmed.is_empty() {
    return "shotify".to_string();
  }
  let cleaned = trimmed
    .replace(['/', '\\'], "-")
    .replace([':', '*', '?', '"', '<', '>', '|'], "");
  if cleaned.trim().is_empty() {
    "shotify".to_string()
  } else {
    cleaned
  }
}

pub fn ext_from_mime(mime: &str) -> &'static str {
  let m = mime.to_ascii_lowercase();
  if m.contains("jpeg") || m.contains("jpg") {
    "jpg"
  } else if m.contains("webp") {
    "webp"
  } else {
    "png"
  }
}

/// Ensure a non-colliding file path by appending a millisecond timestamp suffix if needed.
pub fn ensure_unique_path(path: PathBuf) -> PathBuf {
  if !path.exists() {
    return path;
  }
  let parent = path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
  let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("export").to_string();
  let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_string();
  let ts = chrono::Utc::now().timestamp_millis();
  let file_name = if ext.is_empty() {
    format!("{stem}-{ts}")
  } else {
    format!("{stem}-{ts}.{ext}")
  };
  parent.join(file_name)
}

/// Decode a `data:<mime>;base64,<payload>` string into `(mime, bytes)`.
pub fn decode_data_url(data_url: &str) -> Result<(String, Vec<u8>)> {
  let s = data_url.trim();
  let comma = s
    .find(',')
    .ok_or_else(|| ExportError::DataUrl("Invalid data URL".to_string()))?;
  let (meta, payload) = s.split_at(comma);
  let payload = &payload[1..];

  let mime = meta
    .strip_prefix("data:")
    .and_then(|m| m.split(';').next())
    .unwrap_or("application/octet-stream")
    .to_string();

  let bytes = if meta.contains(";base64") {
    STANDARD.decode(payload)?
  } else {
    payload.as_bytes().to_vec()
  };

  Ok((mime, bytes))
}

/// Encode `(mime, bytes)` into a `data:<mime>;base64,...` string.
pub fn bytes_to_data_url(mime: &str, bytes: &[u8]) -> String {
  format!("data:{mime};base64,{}", STANDARD.encode(bytes))
}

/// Resolve the default Downloads export folder (`~/Downloads/shotify` with Desktop/LocalData fallbacks).
pub fn resolve_downloads_export_dir() -> Result<PathBuf> {
  let base_dir = dirs::download_dir()
    .or_else(dirs::desktop_dir)
    .or_else(dirs::data_local_dir)
    .ok_or_else(|| ExportError::OutputDirectory("No Downloads, Desktop, or LocalData dir available".to_string()))?;
  Ok(base_dir.join("shotify"))
}

fn build_output_file_name(raw_name: &str, mime: &str) -> String {
  let sanitized = sanitize_file_stem(raw_name);
  let requested = Path::new(&sanitized);
  let stem = requested
    .file_stem()
    .and_then(|s| s.to_str())
    .unwrap_or("shotify")
    .to_string();

  let ext = requested
    .extension()
    .and_then(|s| s.to_str())
    .map(|s| s.to_ascii_lowercase())
    .filter(|s| matches!(s.as_str(), "png" | "jpg" | "jpeg" | "webp"))
    .unwrap_or_else(|| ext_from_mime(mime).to_string());

  if ext == "jpeg" {
    format!("{stem}.jpg")
  } else {
    format!("{stem}.{ext}")
  }
}

/// Save encoded image bytes into `<Downloads>/shotify/<file_name>`.
pub fn save_bytes_to_downloads(bytes: &[u8], mime: &str, file_name: &str) -> Result<PathBuf> {
  let out_dir = resolve_downloads_export_dir()?;
  save_bytes_to_folder(bytes, mime, &out_dir, file_name)
}

/// Save encoded image bytes into an explicit user-chosen file path (inferring extension if omitted).
pub fn save_bytes_to_path(bytes: &[u8], mime: &str, file_path: &Path) -> Result<PathBuf> {
  if file_path.as_os_str().is_empty() {
    return Err(ExportError::InvalidPath("Empty file path".to_string()));
  }
  let mut out_path = file_path.to_path_buf();

  let missing_ext = out_path
    .extension()
    .and_then(|s| s.to_str())
    .map(|s| s.trim().is_empty())
    .unwrap_or(true);

  if missing_ext {
    let ext = ext_from_mime(mime);
    out_path.set_extension(if ext == "jpeg" { "jpg" } else { ext });
  }

  if let Some(parent) = out_path.parent() {
    if !parent.as_os_str().is_empty() {
      fs::create_dir_all(parent)?;
    }
  }

  fs::write(&out_path, bytes)?;
  Ok(out_path)
}

/// Save encoded image bytes into an explicit folder with unique filename resolution.
pub fn save_bytes_to_folder(bytes: &[u8], mime: &str, folder: &Path, file_name: &str) -> Result<PathBuf> {
  if folder.as_os_str().is_empty() {
    return Err(ExportError::InvalidPath("Missing folder path".to_string()));
  }
  fs::create_dir_all(folder)?;
  let final_name = build_output_file_name(file_name, mime);
  let out_path = ensure_unique_path(folder.join(final_name));
  fs::write(&out_path, bytes)?;
  Ok(out_path)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn data_url_roundtrip_works() {
    let payload = b"png-bytes-sample";
    let url = bytes_to_data_url("image/png", payload);
    let (mime, decoded) = decode_data_url(&url).unwrap();
    assert_eq!(mime, "image/png");
    assert_eq!(decoded, payload);
  }

  #[test]
  fn saves_to_folder_with_unique_names() {
    let tmp = std::env::temp_dir().join(format!("shotify-export-out-{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(1)));
    let p1 = save_bytes_to_folder(b"abc", "image/png", &tmp, "shot.png").unwrap();
    let p2 = save_bytes_to_folder(b"def", "image/png", &tmp, "shot.png").unwrap();
    assert!(p1.exists() && p2.exists());
    assert_ne!(p1, p2);
    let _ = fs::remove_dir_all(&tmp);
  }
}
