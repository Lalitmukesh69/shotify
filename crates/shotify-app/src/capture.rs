use std::process::Command;

use image::RgbaImage;

/// Capture the primary desktop display into an `RgbaImage`.
/// Tries `xcap` first, then Wayland CLI tools (`grim`) or macOS `screencapture` as needed.
pub fn capture_primary_display() -> Result<RgbaImage, String> {
  if let Ok(monitors) = xcap::Monitor::all() {
    let primary = monitors
      .iter()
      .find(|m| m.is_primary())
      .or_else(|| monitors.first());
    if let Some(mon) = primary {
      if let Ok(img) = mon.capture_image() {
        return Ok(img);
      }
    }
  }

  #[cfg(target_os = "linux")]
  {
    let tmp = std::env::temp_dir().join(format!(
      "shotify-rust-cap-{}.png",
      chrono::Utc::now().timestamp_millis()
    ));
    if let Ok(status) = Command::new("grim").arg(&tmp).status() {
      if status.success() {
        if let Ok(dyn_img) = image::open(&tmp) {
          let _ = std::fs::remove_file(&tmp);
          return Ok(dyn_img.to_rgba8());
        }
      }
    }
  }

  #[cfg(target_os = "macos")]
  {
    let tmp = std::env::temp_dir().join(format!(
      "shotify-rust-cap-{}.png",
      chrono::Utc::now().timestamp_millis()
    ));
    if let Ok(status) = Command::new("/usr/sbin/screencapture")
      .args(["-x", "-t", "png"])
      .arg(&tmp)
      .status()
    {
      if status.success() {
        if let Ok(dyn_img) = image::open(&tmp) {
          let _ = std::fs::remove_file(&tmp);
          return Ok(dyn_img.to_rgba8());
        }
      }
    }
  }

  Err("No supported screen capture backend available on this display session".to_string())
}
