#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
pub mod linux;

pub fn setup_platform_app(app: &tauri::App) {
  #[cfg(target_os = "macos")]
  {
    macos::setup_platform_app(app);
  }
  #[cfg(target_os = "linux")]
  {
    linux::setup_platform_app(app);
  }
  #[cfg(not(any(target_os = "macos", target_os = "linux")))]
  {
    let _ = app;
  }
}

pub fn prepare_main_window_show(app: &tauri::AppHandle) {
  #[cfg(target_os = "macos")]
  {
    macos::prepare_main_window_show(app);
  }
  #[cfg(target_os = "linux")]
  {
    linux::prepare_main_window_show(app);
  }
  #[cfg(not(any(target_os = "macos", target_os = "linux")))]
  {
    let _ = app;
  }
}

pub fn post_main_window_focus(win: &tauri::WebviewWindow) {
  #[cfg(target_os = "macos")]
  {
    macos::post_main_window_focus(win);
  }
  #[cfg(target_os = "linux")]
  {
    linux::post_main_window_focus(win);
  }
  #[cfg(not(any(target_os = "macos", target_os = "linux")))]
  {
    let _ = win;
  }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub fn configure_capture_overlay_window(win: &tauri::WebviewWindow) {
  #[cfg(target_os = "windows")]
  windows::configure_capture_overlay_window(win);
  #[cfg(target_os = "linux")]
  linux::configure_capture_overlay_window(win);
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
#[allow(dead_code)]
pub fn configure_capture_overlay_window(_win: &tauri::WebviewWindow) {}

#[cfg(target_os = "windows")]
pub fn capture_overlay_screenshot_to_file(app: &tauri::AppHandle) -> Result<String, String> {
  windows::capture_overlay_screenshot_to_file(app)
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub fn capture_overlay_screenshot_to_file(_app: &tauri::AppHandle) -> Result<String, String> {
  Err("Overlay screenshot capture is only available on Windows".to_string())
}

#[cfg(target_os = "windows")]
pub fn capture_interactive_region_to_file() -> Result<Option<String>, String> {
  windows::capture_interactive_region_to_file()
}

#[cfg(target_os = "windows")]
pub fn prewarm_capture_interactive_runtime() {
  windows::prewarm_capture_interactive_runtime();
}

#[cfg(not(target_os = "windows"))]
pub fn prewarm_capture_interactive_runtime() {}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub fn capture_interactive_region_to_file() -> Result<Option<String>, String> {
  Err("Interactive region capture is only available on Windows".to_string())
}

pub fn open_url(url: &str) -> Result<(), String> {
  #[cfg(target_os = "macos")]
  {
    return macos::open_url(url);
  }
  #[cfg(target_os = "windows")]
  {
    return windows::open_url(url);
  }
  #[cfg(target_os = "linux")]
  {
    return linux::open_url(url);
  }

  #[allow(unreachable_code)]
  Err("Unsupported platform".to_string())
}

pub fn set_open_at_login_enabled(enabled: bool) -> Result<(), String> {
  #[cfg(target_os = "macos")]
  {
    return macos::set_open_at_login_enabled(enabled);
  }
  #[cfg(target_os = "linux")]
  {
    return linux::set_open_at_login_enabled(enabled);
  }
  #[cfg(not(any(target_os = "macos", target_os = "linux")))]
  {
    let _ = enabled;
    Err("Open at login is currently supported on macOS and Linux".to_string())
  }
}

#[cfg(target_os = "macos")]
pub fn set_dock_icon_hidden(enabled: bool, app: &tauri::AppHandle) -> Result<(), String> {
  macos::set_dock_icon_hidden(enabled, app)
}

#[cfg(not(target_os = "macos"))]
pub fn set_dock_icon_hidden(_enabled: bool, _app: &tauri::AppHandle) -> Result<(), String> {
  Err("Dock icon visibility is only supported on macOS".to_string())
}

#[cfg(target_os = "macos")]
pub async fn open_system_settings(pane: &str) -> Result<bool, String> {
  macos::open_system_settings(pane).await
}

#[cfg(not(target_os = "macos"))]
pub async fn open_system_settings(_pane: &str) -> Result<bool, String> {
  Err("System Settings opening is only supported on macOS".to_string())
}

#[cfg(target_os = "macos")]
pub fn clipboard_png_data_url_after_screencapture(mode: Option<&str>) -> Result<String, String> {
  macos::clipboard_png_data_url_after_screencapture(mode)
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
pub fn clipboard_png_data_url_after_screencapture(_mode: Option<&str>) -> Result<String, String> {
  Err("Screenshot capture is only available on macOS in the native shell".to_string())
}

pub fn supports_native_global_shortcuts() -> bool {
  #[cfg(target_os = "linux")]
  {
    return linux::is_x11_session();
  }
  #[cfg(not(target_os = "linux"))]
  {
    true
  }
}
