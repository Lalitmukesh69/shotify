pub mod autostart;
pub mod shortcuts;
pub mod single_instance;
pub mod tray;
pub mod updater;
pub mod vibrancy;

use std::fmt;

/// Cross-platform commands pushed from tray, global hotkeys, Wayland portal, or secondary IPC instances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformCommand {
  ShowWindow,
  CaptureRegion,
  CaptureRegionCopy,
  OpenSettings,
  QuitApp,
  ShortcutBackendUpdated {
    backend: GlobalShortcutBackend,
    portal_active: bool,
  },
}

/// Active global-shortcut backend detected for this OS/session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlobalShortcutBackend {
  #[default]
  NativeGlobalHotkey,
  WaylandPortal,
  CliOnly,
}

impl fmt::Display for GlobalShortcutBackend {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::NativeGlobalHotkey => f.write_str("Native (OS / X11)"),
      Self::WaylandPortal => f.write_str("Wayland Portal"),
      Self::CliOnly => f.write_str("CLI Only (shotify-rust --capture)"),
    }
  }
}

/// Runtime platform capabilities exposed in Settings UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformCapabilities {
  pub os: &'static str,
  pub is_wayland: bool,
  pub portal_session_active: bool,
  pub global_shortcuts_mode: GlobalShortcutBackend,
  pub tray_available: bool,
  pub supports_vibrancy: bool,
  pub supports_dock_hide: bool,
}

pub fn is_wayland_session() -> bool {
  #[cfg(target_os = "linux")]
  {
    let wayland_display = std::env::var("WAYLAND_DISPLAY")
      .map(|v| !v.trim().is_empty())
      .unwrap_or(false);
    let session_type = std::env::var("XDG_SESSION_TYPE")
      .map(|v| v.trim().eq_ignore_ascii_case("wayland"))
      .unwrap_or(false);
    wayland_display || session_type
  }
  #[cfg(not(target_os = "linux"))]
  {
    false
  }
}

/// Detect baseline OS capabilities for `shotify-rust`.
pub fn detect_platform_capabilities(tray_available: bool) -> PlatformCapabilities {
  let is_wayland = is_wayland_session();
  let global_shortcuts_mode = if is_wayland {
    GlobalShortcutBackend::CliOnly
  } else {
    GlobalShortcutBackend::NativeGlobalHotkey
  };

  PlatformCapabilities {
    os: std::env::consts::OS,
    is_wayland,
    portal_session_active: false,
    global_shortcuts_mode,
    tray_available,
    supports_vibrancy: cfg!(any(target_os = "macos", target_os = "windows")),
    supports_dock_hide: cfg!(target_os = "macos"),
  }
}

/// Open an external `http://` or `https://` URL using the OS browser launcher.
pub fn open_url(url: &str) -> Result<(), String> {
  let trimmed = url.trim();
  if !(trimmed.starts_with("https://") || trimmed.starts_with("http://")) {
    return Err("Refusing to open non-HTTP URL".to_string());
  }

  #[cfg(target_os = "linux")]
  {
    std::process::Command::new("xdg-open")
      .arg(trimmed)
      .spawn()
      .map(|_| ())
      .map_err(|e| format!("Failed launching xdg-open: {e}"))
  }

  #[cfg(target_os = "macos")]
  {
    std::process::Command::new("open")
      .arg(trimmed)
      .spawn()
      .map(|_| ())
      .map_err(|e| format!("Failed launching open: {e}"))
  }

  #[cfg(target_os = "windows")]
  {
    std::process::Command::new("cmd")
      .args(["/C", "start", "", trimmed])
      .spawn()
      .map(|_| ())
      .map_err(|e| format!("Failed launching URL via cmd: {e}"))
  }

  #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
  {
    let _ = trimmed;
    Err("Opening URLs is not supported on this OS".to_string())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn open_url_rejects_unsafe_schemes() {
    assert!(open_url("file:///etc/passwd").is_err());
    assert!(open_url("javascript:alert(1)").is_err());
  }

  #[test]
  fn platform_capabilities_matches_host_os() {
    let caps = detect_platform_capabilities(true);
    assert_eq!(caps.os, std::env::consts::OS);
    assert!(caps.tray_available);
    assert_eq!(
      caps.supports_vibrancy,
      cfg!(any(target_os = "macos", target_os = "windows"))
    );
  }
}
