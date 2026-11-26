use std::env;
use std::fs;
use std::io::{Cursor, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;

use ashpd::desktop::screenshot::Screenshot;
use ashpd::desktop::ResponseError;
use image::ImageEncoder;
use serde::Serialize;
use tauri::Manager;

use crate::images::write_temp_png;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinuxCapabilities {
  pub display_server: String,
  pub compositor: String,
  pub is_wlroots_like: bool,
  pub supports_x11_global_shortcuts: bool,
  pub has_grim: bool,
  pub has_wl_copy: bool,
}

#[derive(Debug, Clone)]
pub enum LinuxCaptureOutcome {
  OverlayFile(String),
  DirectImage(String),
}

fn find_in_path(bin: &str) -> Option<PathBuf> {
  let path_var = env::var_os("PATH")?;
  for dir in env::split_paths(&path_var) {
    let candidate = dir.join(bin);
    if candidate.is_file() {
      return Some(candidate);
    }
  }
  None
}

pub fn detect_capabilities_from_env(
  wayland_display: Option<&str>,
  xdg_session_type: Option<&str>,
  display: Option<&str>,
  xdg_current_desktop: Option<&str>,
  xdg_session_desktop: Option<&str>,
  desktop_session: Option<&str>,
  has_niri_socket: bool,
  has_sway_sock: bool,
  has_hypr_sig: bool,
) -> LinuxCapabilities {
  let session_type = xdg_session_type
    .unwrap_or("")
    .trim()
    .to_ascii_lowercase();
  let wayland = !wayland_display.unwrap_or("").trim().is_empty();
  let x11 = !display.unwrap_or("").trim().is_empty();

  let display_server = if wayland || session_type == "wayland" {
    "wayland".to_string()
  } else if x11 || session_type == "x11" {
    "x11".to_string()
  } else {
    "unknown".to_string()
  };

  let desktop_joined = [
    xdg_current_desktop.unwrap_or(""),
    xdg_session_desktop.unwrap_or(""),
    desktop_session.unwrap_or(""),
  ]
  .join(" ")
  .to_ascii_lowercase();

  let compositor = if has_niri_socket || desktop_joined.contains("niri") {
    "niri".to_string()
  } else if has_sway_sock || desktop_joined.contains("sway") {
    "sway".to_string()
  } else if has_hypr_sig || desktop_joined.contains("hyprland") {
    "hyprland".to_string()
  } else if desktop_joined.contains("gnome") || desktop_joined.contains("ubuntu") {
    "gnome".to_string()
  } else if desktop_joined.contains("kde") || desktop_joined.contains("plasma") {
    "kde".to_string()
  } else if desktop_joined.contains("xfce") {
    "xfce".to_string()
  } else if desktop_joined.contains("river")
    || desktop_joined.contains("labwc")
    || desktop_joined.contains("wayfire")
  {
    "wlroots".to_string()
  } else if display_server == "x11" {
    "x11".to_string()
  } else {
    "unknown".to_string()
  };

  let is_wlroots_like = display_server == "wayland"
    && (has_niri_socket
      || has_sway_sock
      || has_hypr_sig
      || matches!(compositor.as_str(), "niri" | "sway" | "hyprland" | "wlroots"));

  LinuxCapabilities {
    supports_x11_global_shortcuts: display_server == "x11",
    display_server,
    compositor,
    is_wlroots_like,
    has_grim: find_in_path("grim").is_some(),
    has_wl_copy: find_in_path("wl-copy").is_some(),
  }
}

pub fn detect_capabilities() -> LinuxCapabilities {
  let wayland = env::var("WAYLAND_DISPLAY").ok();
  let session_type = env::var("XDG_SESSION_TYPE").ok();
  let display = env::var("DISPLAY").ok();
  let current_desktop = env::var("XDG_CURRENT_DESKTOP").ok();
  let session_desktop = env::var("XDG_SESSION_DESKTOP").ok();
  let desktop_session = env::var("DESKTOP_SESSION").ok();
  let has_niri = env::var_os("NIRI_SOCKET").is_some_and(|v| !v.is_empty());
  let has_sway = env::var_os("SWAYSOCK").is_some_and(|v| !v.is_empty());
  let has_hypr = env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some_and(|v| !v.is_empty());

  detect_capabilities_from_env(
    wayland.as_deref(),
    session_type.as_deref(),
    display.as_deref(),
    current_desktop.as_deref(),
    session_desktop.as_deref(),
    desktop_session.as_deref(),
    has_niri,
    has_sway,
    has_hypr,
  )
}

pub fn is_x11_session() -> bool {
  detect_capabilities().display_server == "x11"
}

pub fn is_wayland_session() -> bool {
  detect_capabilities().display_server == "wayland"
}

pub fn setup_platform_app(app: &tauri::App) {
  use gtk::prelude::*;

  gtk::glib::set_prgname(Some("shotify"));
  gtk::glib::set_application_name("shotify");

  if let Some(settings) = gtk::Settings::default() {
    settings.set_gtk_application_prefer_dark_theme(true);
  }

  if let Some(screen) = gdk::Screen::default() {
    let css = gtk::CssProvider::new();
    let _ = css.load_from_data(
      b"headerbar, .titlebar { background-image: none; background-color: #050507; color: #d4d4d8; border: none; border-bottom: 1px solid rgba(255,255,255,0.06); box-shadow: none; padding: 0 6px; }
        headerbar .title { font-size: 11px; color: rgba(255,255,255,0.5); font-weight: 500; }
        headerbar button.titlebutton { background-image: none; background-color: transparent; border: none; box-shadow: none; color: rgba(255,255,255,0.6); min-height: 18px; min-width: 22px; padding: 0 4px; border-radius: 5px; }
        headerbar button.titlebutton:hover { background-color: rgba(255,255,255,0.1); color: #ffffff; }",
    );
    gtk::StyleContext::add_provider_for_screen(&screen, &css, gtk::STYLE_PROVIDER_PRIORITY_USER);
  }

  if let Some(win) = app.get_webview_window("main") {
    let _ = win.set_title("shotify");
  }
}

pub fn prepare_main_window_show(_app: &tauri::AppHandle) {}

pub fn post_main_window_focus(_win: &tauri::WebviewWindow) {}

pub fn configure_capture_overlay_window(win: &tauri::WebviewWindow) {
  let _ = win.set_fullscreen(true);
}

pub fn open_url(url: &str) -> Result<(), String> {
  let launchers: [(&str, &[&str]); 3] = [
    ("xdg-open", &[url]),
    ("gio", &["open", url]),
    ("sensible-browser", &[url]),
  ];

  let mut last_error: Option<String> = None;
  for (bin, args) in launchers {
    match Command::new(bin).args(args).spawn() {
      Ok(_) => return Ok(()),
      Err(err) => last_error = Some(format!("{bin}: {err}")),
    }
  }

  Err(format!(
    "Failed to open browser with known launchers ({})",
    last_error.unwrap_or_else(|| "unknown launcher error".to_string())
  ))
}

fn autostart_dir_from_env(xdg_config_home: Option<&str>, home: Option<&str>) -> Result<PathBuf, String> {
  if let Some(cfg) = xdg_config_home.map(str::trim).filter(|s| !s.is_empty()) {
    return Ok(PathBuf::from(cfg).join("autostart"));
  }
  let home = home
    .map(str::trim)
    .filter(|s| !s.is_empty())
    .ok_or_else(|| "Neither XDG_CONFIG_HOME nor HOME is set".to_string())?;
  Ok(PathBuf::from(home).join(".config").join("autostart"))
}

fn quote_desktop_exec(path: &Path) -> String {
  let s = path.to_string_lossy();
  let escaped = s
    .replace('\\', "\\\\")
    .replace('"', "\\\"")
    .replace('$', "\\$")
    .replace('`', "\\`");
  format!("\"{escaped}\"")
}

pub fn render_autostart_desktop_entry(exec_path: &Path) -> String {
  let exec = quote_desktop_exec(exec_path);
  format!(
    "[Desktop Entry]\n\
     Type=Application\n\
     Version=1.0\n\
     Name=shotify\n\
     Comment=Screenshot capture, styling, and annotation tool\n\
     Exec={exec}\n\
     Icon=shotify\n\
     Terminal=false\n\
     Categories=Graphics;Utility;\n\
     X-GNOME-Autostart-enabled=true\n"
  )
}

fn resolve_autostart_executable() -> Result<PathBuf, String> {
  if let Ok(appimage) = env::var("APPIMAGE") {
    let p = PathBuf::from(appimage.trim());
    if p.is_file() {
      return Ok(p);
    }
  }
  env::current_exe().map_err(|e| format!("Failed to resolve app executable path: {e}"))
}

pub fn set_open_at_login_enabled(enabled: bool) -> Result<(), String> {
  let xdg_cfg = env::var("XDG_CONFIG_HOME").ok();
  let home = env::var("HOME").ok();
  let autostart_dir = autostart_dir_from_env(xdg_cfg.as_deref(), home.as_deref())?;
  let desktop_path = autostart_dir.join("shotify.desktop");

  if !enabled {
    if desktop_path.exists() {
      fs::remove_file(&desktop_path)
        .map_err(|e| format!("Failed to remove autostart desktop entry: {e}"))?;
    }
    return Ok(());
  }

  fs::create_dir_all(&autostart_dir)
    .map_err(|e| format!("Failed to create autostart directory: {e}"))?;
  let exe = resolve_autostart_executable()?;
  let content = render_autostart_desktop_entry(&exe);
  fs::write(&desktop_path, content.as_bytes())
    .map_err(|e| format!("Failed to write autostart desktop entry: {e}"))?;
  let mut perms = fs::metadata(&desktop_path)
    .map_err(|e| format!("Failed to inspect autostart file: {e}"))?
    .permissions();
  perms.set_mode(0o644);
  fs::set_permissions(&desktop_path, perms)
    .map_err(|e| format!("Failed to set autostart file permissions: {e}"))?;
  Ok(())
}

fn make_temp_capture_path(prefix: &str) -> PathBuf {
  let mut p = env::temp_dir();
  let ts = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_millis();
  p.push(format!("{prefix}-{}-{ts}.png", std::process::id()));
  p
}

fn focused_wlroots_output_name(compositor: &str) -> Option<String> {
  match compositor {
    "niri" => {
      let out = Command::new("niri").args(["msg", "-j", "focused-output"]).output().ok()?;
      if !out.status.success() {
        return None;
      }
      let json: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
      json.get("name").and_then(|v| v.as_str()).map(str::to_string)
    }
    "sway" => {
      let out = Command::new("swaymsg").args(["-t", "get_outputs"]).output().ok()?;
      if !out.status.success() {
        return None;
      }
      let arr: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).ok()?;
      arr
        .into_iter()
        .find(|o| o.get("focused").and_then(|v| v.as_bool()) == Some(true))
        .and_then(|o| o.get("name").and_then(|v| v.as_str()).map(str::to_string))
    }
    "hyprland" => {
      let out = Command::new("hyprctl").args(["activeworkspace", "-j"]).output().ok()?;
      if !out.status.success() {
        return None;
      }
      let json: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
      json.get("monitor").and_then(|v| v.as_str()).map(str::to_string)
    }
    _ => None,
  }
}

fn capture_via_grim(compositor: &str) -> Result<String, String> {
  let grim = find_in_path("grim").ok_or_else(|| "grim is not installed".to_string())?;
  let out_path = make_temp_capture_path("shotify-grim");
  let out_str = out_path.to_string_lossy().to_string();

  let focused_output = focused_wlroots_output_name(compositor);
  let output = if let Some(ref output_name) = focused_output {
    let first = Command::new(&grim)
      .args(["-o", output_name, &out_str])
      .output()
      .map_err(|e| format!("Failed to run grim: {e}"))?;
    if first.status.success() && out_path.exists() {
      first
    } else {
      Command::new(&grim)
        .arg(&out_str)
        .output()
        .map_err(|e| format!("Failed to run grim: {e}"))?
    }
  } else {
    Command::new(&grim)
      .arg(&out_str)
      .output()
      .map_err(|e| format!("Failed to run grim: {e}"))?
  };

  if !output.status.success() {
    let _ = fs::remove_file(&out_path);
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    return Err(if stderr.is_empty() {
      "grim screenshot capture failed".to_string()
    } else {
      format!("grim failed: {stderr}")
    });
  }

  let meta = fs::metadata(&out_path).map_err(|e| format!("Failed to read grim capture: {e}"))?;
  if meta.len() == 0 {
    let _ = fs::remove_file(&out_path);
    return Err("grim produced an empty screenshot file".to_string());
  }

  Ok(out_str)
}

fn decode_file_uri(uri: &str) -> Option<PathBuf> {
  let stripped = uri.strip_prefix("file://")?;
  let mut out = Vec::with_capacity(stripped.len());
  let bytes = stripped.as_bytes();
  let mut i = 0;
  while i < bytes.len() {
    if bytes[i] == b'%' && i + 2 < bytes.len() {
      if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
        if let Ok(val) = u8::from_str_radix(hex, 16) {
          out.push(val);
          i += 3;
          continue;
        }
      }
    }
    out.push(bytes[i]);
    i += 1;
  }
  Some(PathBuf::from(String::from_utf8_lossy(&out).to_string()))
}

fn normalize_portal_image_to_temp(src_path: &Path) -> Result<String, String> {
  let bytes = fs::read(src_path).map_err(|e| format!("Failed to read portal screenshot: {e}"))?;
  let tmp = write_temp_png(&bytes)?;
  let _ = fs::remove_file(src_path);
  Ok(tmp.to_string_lossy().to_string())
}

async fn capture_via_portal(interactive: bool) -> Result<String, String> {
  let req = Screenshot::request()
    .interactive(interactive)
    .modal(interactive)
    .send()
    .await
    .map_err(|e| match e {
      ashpd::Error::Response(ResponseError::Cancelled) => "Screenshot capture canceled".to_string(),
      other => format!("Desktop portal screenshot request failed: {other}"),
    })?;

  let resp = req.response().map_err(|e| match e {
    ashpd::Error::Response(ResponseError::Cancelled) => "Screenshot capture canceled".to_string(),
    other => format!("Desktop portal screenshot failed: {other}"),
  })?;

  let uri_str = resp.uri().as_str();
  let src_path = decode_file_uri(uri_str)
    .ok_or_else(|| format!("Unsupported screenshot URI returned by portal: {uri_str}"))?;
  normalize_portal_image_to_temp(&src_path)
}

fn capture_via_xcap_x11(app: &tauri::AppHandle) -> Result<String, String> {
  let target_bounds = app
    .get_webview_window("capture")
    .or_else(|| app.get_webview_window("main"))
    .and_then(|w| w.current_monitor().ok().flatten())
    .map(|m| {
      let p = m.position();
      let s = m.size();
      (p.x, p.y, s.width, s.height)
    });

  let monitors = xcap::Monitor::all().map_err(|e| format!("Failed to enumerate X11 monitors: {e}"))?;
  if monitors.is_empty() {
    return Err("No monitors available for X11 capture".to_string());
  }

  let selected = if let Some((tx, ty, tw, th)) = target_bounds {
    monitors
      .iter()
      .min_by_key(|m| {
        let mx = m.x();
        let my = m.y();
        let mw = m.width() as i32;
        let mh = m.height() as i32;
        (mx - tx).abs() + (my - ty).abs() + (mw - tw as i32).abs() + (mh - th as i32).abs()
      })
      .unwrap_or(&monitors[0])
  } else {
    monitors
      .iter()
      .find(|m| m.is_primary())
      .unwrap_or(&monitors[0])
  };

  let rgba = selected
    .capture_image()
    .map_err(|e| format!("Failed to capture X11 monitor: {e}"))?;
  let (w, h) = rgba.dimensions();
  let mut png_bytes = Vec::new();
  image::codecs::png::PngEncoder::new(&mut png_bytes)
    .write_image(rgba.as_raw(), w, h, image::ExtendedColorType::Rgba8)
    .map_err(|e| format!("Failed to encode X11 capture PNG: {e}"))?;
  let out = write_temp_png(&png_bytes)?;
  Ok(out.to_string_lossy().to_string())
}

pub async fn capture_screen_for_linux(
  app: &tauri::AppHandle,
  prefer_overlay: bool,
) -> Result<LinuxCaptureOutcome, String> {
  let caps = detect_capabilities();

  if caps.display_server == "x11" {
    let app_clone = app.clone();
    let path = tauri::async_runtime::spawn_blocking(move || capture_via_xcap_x11(&app_clone))
      .await
      .map_err(|e| format!("Capture task failed: {e}"))??;
    return Ok(LinuxCaptureOutcome::OverlayFile(path));
  }

  if caps.display_server == "wayland" {
    if caps.is_wlroots_like && caps.has_grim {
      let comp = caps.compositor.clone();
      match tauri::async_runtime::spawn_blocking(move || capture_via_grim(&comp)).await {
        Ok(Ok(path)) => return Ok(LinuxCaptureOutcome::OverlayFile(path)),
        Ok(Err(err)) => log::warn!("[linux-capture] grim failed, falling back to portal: {err}"),
        Err(err) => log::warn!("[linux-capture] grim task join error: {err}"),
      }
    }

    let monitor_count = app
      .get_webview_window("main")
      .and_then(|w| w.available_monitors().ok())
      .map(|m| m.len())
      .unwrap_or(1);

    if prefer_overlay && monitor_count <= 1 {
      match capture_via_portal(false).await {
        Ok(path) => return Ok(LinuxCaptureOutcome::OverlayFile(path)),
        Err(err) if err.contains("canceled") => return Err(err),
        Err(err) => {
          log::warn!("[linux-capture] non-interactive portal capture failed, retrying interactive: {err}");
        }
      }
    }

    let interactive = prefer_overlay;
    let path = capture_via_portal(interactive).await?;
    return Ok(if interactive {
      LinuxCaptureOutcome::DirectImage(path)
    } else {
      LinuxCaptureOutcome::OverlayFile(path)
    });
  }

  Err("Unsupported Linux display server (neither Wayland nor X11 detected)".to_string())
}

pub fn rasterize_image_to_png_with_gdk(path: &Path, max_dim: u32) -> Result<Vec<u8>, String> {
  let pixbuf = if max_dim > 0 {
    gdk_pixbuf::Pixbuf::from_file_at_scale(path, max_dim as i32, max_dim as i32, true)
  } else {
    gdk_pixbuf::Pixbuf::from_file(path)
  }
  .map_err(|e| format!("gdk-pixbuf failed to load image: {e}"))?;

  pixbuf
    .save_to_bufferv("png", &[])
    .map_err(|e| format!("gdk-pixbuf failed to encode PNG: {e}"))
}

pub fn clipboard_write_png_fallback(app: &tauri::AppHandle, png_bytes: &[u8]) -> Result<(), String> {
  if is_wayland_session() && find_in_path("wl-copy").is_some() {
    if let Ok(mut child) = Command::new("wl-copy")
      .args(["--type", "image/png"])
      .stdin(Stdio::piped())
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .spawn()
    {
      if let Some(mut stdin) = child.stdin.take() {
        let write_ok = stdin.write_all(png_bytes).is_ok();
        drop(stdin);
        if write_ok {
          if let Ok(status) = child.wait() {
            if status.success() {
              return Ok(());
            }
          }
        }
      }
    }
  }

  let bytes = png_bytes.to_vec();
  let (tx, rx) = mpsc::channel::<Result<(), String>>();
  app
    .run_on_main_thread(move || {
      let res = (|| -> Result<(), String> {
        let pixbuf = gdk_pixbuf::Pixbuf::from_read(Cursor::new(bytes))
          .map_err(|e| format!("Failed to decode clipboard PNG in gdk-pixbuf: {e}"))?;
        let clipboard = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD);
        clipboard.set_image(&pixbuf);
        clipboard.store();
        Ok(())
      })();
      let _ = tx.send(res);
    })
    .map_err(|e| format!("Failed to dispatch GTK clipboard write: {e}"))?;

  rx.recv()
    .map_err(|e| format!("GTK clipboard response channel failed: {e}"))?
}

fn is_png_signature(bytes: &[u8]) -> bool {
  bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
}

pub fn decode_clipboard_file_uri_or_path(raw: &str) -> Option<PathBuf> {
  for line in raw.lines() {
    let trimmed = line.trim().trim_matches('\r');
    if trimmed.is_empty() || trimmed.starts_with('#') {
      continue;
    }
    let candidate = if trimmed.starts_with("file://") {
      decode_file_uri(trimmed)?
    } else {
      let p = PathBuf::from(trimmed);
      if !p.is_absolute() {
        return None;
      }
      p
    };
    let ext = candidate
      .extension()
      .and_then(|s| s.to_str())
      .unwrap_or("")
      .to_ascii_lowercase();
    let is_image = matches!(
      ext.as_str(),
      "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" | "tiff" | "tif" | "avif" | "heic" | "heif" | "svg"
    );
    if is_image && candidate.is_file() {
      return Some(candidate);
    }
    return None;
  }
  None
}

fn normalize_clipboard_image_bytes(bytes: &[u8], hint_path: Option<&Path>) -> Option<Vec<u8>> {
  if bytes.is_empty() {
    return None;
  }
  if is_png_signature(bytes) {
    return Some(bytes.to_vec());
  }
  if let Ok(img) = image::load_from_memory(bytes) {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut out = Vec::new();
    if image::codecs::png::PngEncoder::new(&mut out)
      .write_image(rgba.as_raw(), w, h, image::ExtendedColorType::Rgba8)
      .is_ok()
    {
      return Some(out);
    }
  }
  if let Some(path) = hint_path {
    if let Ok(png) = rasterize_image_to_png_with_gdk(path, 0) {
      return Some(png);
    }
  }
  None
}

fn select_preferred_image_mime(types_output: &str) -> Option<String> {
  let lines: Vec<&str> = types_output
    .lines()
    .map(str::trim)
    .filter(|l| !l.is_empty())
    .collect();
  for preferred in [
    "image/png",
    "image/jpeg",
    "image/jpg",
    "image/webp",
    "image/bmp",
    "image/tiff",
  ] {
    if lines.iter().any(|l| l.eq_ignore_ascii_case(preferred)) {
      return Some(preferred.to_string());
    }
  }
  lines
    .into_iter()
    .find(|l| l.to_ascii_lowercase().starts_with("image/"))
    .map(str::to_string)
}

pub fn clipboard_read_image_png_fallback(app: &tauri::AppHandle) -> Result<Option<Vec<u8>>, String> {
  if is_wayland_session() && find_in_path("wl-paste").is_some() {
    if let Ok(types_out) = Command::new("wl-paste").arg("--list-types").output() {
      if types_out.status.success() {
        let types_str = String::from_utf8_lossy(&types_out.stdout);
        if let Some(mime) = select_preferred_image_mime(&types_str) {
          if let Ok(img_out) = Command::new("wl-paste")
            .args(["--no-newline", "--type", &mime])
            .output()
          {
            if img_out.status.success() && !img_out.stdout.is_empty() {
              if mime.eq_ignore_ascii_case("image/svg+xml") {
                if let Ok(pixbuf) = gdk_pixbuf::Pixbuf::from_read(Cursor::new(img_out.stdout)) {
                  if let Ok(png) = pixbuf.save_to_bufferv("png", &[]) {
                    return Ok(Some(png));
                  }
                }
              } else if let Some(png) = normalize_clipboard_image_bytes(&img_out.stdout, None) {
                return Ok(Some(png));
              }
            }
          }
        }

        if types_str.lines().any(|l| l.trim().eq_ignore_ascii_case("text/uri-list")) {
          if let Ok(uri_out) = Command::new("wl-paste")
            .args(["--no-newline", "--type", "text/uri-list"])
            .output()
          {
            if uri_out.status.success() {
              let uri_text = String::from_utf8_lossy(&uri_out.stdout);
              if let Some(path) = decode_clipboard_file_uri_or_path(&uri_text) {
                if let Ok(raw) = fs::read(&path) {
                  if let Some(png) = normalize_clipboard_image_bytes(&raw, Some(&path)) {
                    return Ok(Some(png));
                  }
                }
              }
            }
          }
        }
      }
    }
  }

  let (tx, rx) = mpsc::channel::<Result<Option<Vec<u8>>, String>>();
  app
    .run_on_main_thread(move || {
      let res = (|| -> Result<Option<Vec<u8>>, String> {
        let clipboard = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD);
        if let Some(pixbuf) = clipboard.wait_for_image() {
          let bytes = pixbuf
            .save_to_bufferv("png", &[])
            .map_err(|e| format!("Failed to encode GTK clipboard image: {e}"))?;
          return Ok(Some(bytes));
        }
        if let Some(text) = clipboard.wait_for_text() {
          if let Some(path) = decode_clipboard_file_uri_or_path(text.as_str()) {
            if let Ok(raw) = fs::read(&path) {
              if let Some(png) = normalize_clipboard_image_bytes(&raw, Some(&path)) {
                return Ok(Some(png));
              }
            }
          }
        }
        Ok(None)
      })();
      let _ = tx.send(res);
    })
    .map_err(|e| format!("Failed to dispatch GTK clipboard read: {e}"))?;

  rx.recv()
    .map_err(|e| format!("GTK clipboard read response channel failed: {e}"))?
}

fn parse_xdg_data_dirs_wallpapers(xdg_data_dirs: Option<&str>) -> Vec<PathBuf> {
  let mut out = Vec::new();
  let Some(raw) = xdg_data_dirs else {
    return out;
  };
  for dir in env::split_paths(raw) {
    if dir.as_os_str().is_empty() {
      continue;
    }
    out.push(dir.join("backgrounds"));
    out.push(dir.join("wallpapers"));
  }
  out
}

fn clean_gsettings_uri(raw: &str) -> Option<PathBuf> {
  let trimmed = raw.trim().trim_matches('\'').trim_matches('"');
  if trimmed.is_empty() || trimmed == "none" {
    return None;
  }
  if let Some(p) = decode_file_uri(trimmed) {
    return Some(p);
  }
  let p = PathBuf::from(trimmed);
  if p.is_absolute() {
    Some(p)
  } else {
    None
  }
}

pub fn detect_extra_wallpaper_paths() -> Vec<PathBuf> {
  let mut out = parse_xdg_data_dirs_wallpapers(env::var("XDG_DATA_DIRS").ok().as_deref());

  if find_in_path("gsettings").is_some() {
    for key in ["picture-uri", "picture-uri-dark"] {
      if let Ok(res) = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.background", key])
        .output()
      {
        if res.status.success() {
          let stdout = String::from_utf8_lossy(&res.stdout);
          if let Some(path) = clean_gsettings_uri(&stdout) {
            if path.exists() {
              out.push(path);
            }
          }
        }
      }
    }
  }

  if let Ok(home) = env::var("HOME") {
    let kde_cfg = PathBuf::from(home).join(".config").join("plasma-org.kde.plasma.desktop-appletsrc");
    if let Ok(contents) = fs::read_to_string(&kde_cfg) {
      for line in contents.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("Image=") {
          if let Some(p) = clean_gsettings_uri(val) {
            if p.exists() {
              out.push(p);
            }
          }
        }
      }
    }
  }

  out
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn detects_wayland_niri_from_env() {
    let caps = detect_capabilities_from_env(
      Some("wayland-1"),
      Some("wayland"),
      Some(":0"),
      Some("niri"),
      Some("niri"),
      None,
      true,
      false,
      false,
    );
    assert_eq!(caps.display_server, "wayland");
    assert_eq!(caps.compositor, "niri");
    assert!(caps.is_wlroots_like);
    assert!(!caps.supports_x11_global_shortcuts);
  }

  #[test]
  fn detects_x11_session_from_env() {
    let caps = detect_capabilities_from_env(
      None,
      Some("x11"),
      Some(":0"),
      Some("XFCE"),
      None,
      Some("xfce"),
      false,
      false,
      false,
    );
    assert_eq!(caps.display_server, "x11");
    assert_eq!(caps.compositor, "xfce");
    assert!(!caps.is_wlroots_like);
    assert!(caps.supports_x11_global_shortcuts);
  }

  #[test]
  fn autostart_entry_quotes_spaces_and_resolves_xdg() {
    let dir = autostart_dir_from_env(Some("/tmp/xdg-config"), Some("/home/test")).unwrap();
    assert_eq!(dir, PathBuf::from("/tmp/xdg-config/autostart"));

    let entry = render_autostart_desktop_entry(Path::new("/home/user/My Apps/shotify.AppImage"));
    assert!(entry.contains("Exec=\"/home/user/My Apps/shotify.AppImage\""));
    assert!(entry.contains("Name=shotify"));
    assert!(entry.contains("Type=Application"));
  }

  #[test]
  fn xdg_data_dirs_extracts_wallpaper_subdirs() {
    let dirs = parse_xdg_data_dirs_wallpapers(Some("/usr/local/share:/usr/share:/var/lib/flatpak/exports/share"));
    assert!(dirs.contains(&PathBuf::from("/usr/share/backgrounds")));
    assert!(dirs.contains(&PathBuf::from("/usr/share/wallpapers")));
    assert!(dirs.contains(&PathBuf::from("/var/lib/flatpak/exports/share/backgrounds")));
  }

  #[test]
  fn decode_file_uri_handles_percent_encoding() {
    let parsed = decode_file_uri("file:///home/user/My%20Wallpapers/test%2520.png");
    assert_eq!(parsed, Some(PathBuf::from("/home/user/My Wallpapers/test%20.png")));
  }

  #[test]
  fn clipboard_mime_selection_prefers_png_over_other_formats() {
    let picked = select_preferred_image_mime("text/plain\nimage/webp\nimage/png\nimage/jpeg\n");
    assert_eq!(picked.as_deref(), Some("image/png"));
    let fallback = select_preferred_image_mime("text/html\nimage/tiff\n");
    assert_eq!(fallback.as_deref(), Some("image/tiff"));
  }

  #[test]
  fn decode_clipboard_file_uri_or_path_accepts_existing_image_file() {
    let tmp = std::env::temp_dir().join(format!("shotify-clip-test-{}.png", std::process::id()));
    fs::write(&tmp, b"not-empty").unwrap();
    let uri = format!("file://{}", tmp.to_string_lossy().replace(' ', "%20"));
    assert_eq!(decode_clipboard_file_uri_or_path(&uri), Some(tmp.clone()));
    let _ = fs::remove_file(&tmp);
    assert_eq!(decode_clipboard_file_uri_or_path(&uri), None);
  }
}
