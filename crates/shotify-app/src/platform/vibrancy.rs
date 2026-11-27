use raw_window_handle::WindowHandle;

/// Apply native window vibrancy / translucency where supported (macOS / Windows).
/// Linux compositors manage blur independently, so Linux is a graceful no-op.
pub fn apply_vibrancy_to_handle(handle: WindowHandle<'_>, dark_mode: bool) {
  #[cfg(target_os = "macos")]
  {
    let _ = dark_mode;
    let _ = window_vibrancy::apply_vibrancy(
      &handle,
      window_vibrancy::NSVisualEffectMaterial::UnderWindowBackground,
      None,
      Some(12.0),
    );
  }

  #[cfg(target_os = "windows")]
  {
    let tint = if dark_mode {
      Some((18, 19, 24, 185))
    } else {
      Some((245, 246, 250, 190))
    };
    if window_vibrancy::apply_acrylic(&handle, tint).is_err() {
      let _ = window_vibrancy::apply_blur(&handle, tint);
    }
  }

  #[cfg(not(any(target_os = "macos", target_os = "windows")))]
  {
    let _ = (handle, dark_mode);
  }
}

/// Toggle macOS Dock icon visibility (`NSApplicationActivationPolicy::Accessory` vs `Regular`).
/// No-op on Linux and Windows.
pub fn set_dock_icon_hidden(hidden: bool) {
  #[cfg(target_os = "macos")]
  {
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    use objc2_foundation::MainThreadMarker;

    if let Some(mtm) = MainThreadMarker::new() {
      let app = NSApplication::sharedApplication(mtm);
      let policy = if hidden {
        NSApplicationActivationPolicy::Accessory
      } else {
        NSApplicationActivationPolicy::Regular
      };
      let _ = app.setActivationPolicy(policy);
    }
  }

  #[cfg(not(target_os = "macos"))]
  {
    let _ = hidden;
  }
}
