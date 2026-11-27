use std::fs;
use std::path::{Path, PathBuf};

const RUST_AUTOSTART_DESKTOP_NAME: &str = "shotify-rust.desktop";
#[cfg(any(target_os = "macos", test))]
const MACOS_LAUNCH_AGENT_LABEL: &str = "com.shotify.rust";
#[cfg(any(target_os = "macos", test))]
const MACOS_LAUNCH_AGENT_PLIST: &str = "com.shotify.rust.plist";
#[cfg(target_os = "windows")]
const WINDOWS_RUN_VALUE_NAME: &str = "ShotifyRust";

/// Quote a binary path cleanly for XDG `.desktop` `Exec=` lines.
pub fn quote_desktop_exec(path: &Path) -> String {
  let raw = path.display().to_string();
  let mut out = String::from("\"");
  for ch in raw.chars() {
    match ch {
      '\\' | '"' | '$' | '`' => {
        out.push('\\');
        out.push(ch);
      }
      _ => out.push(ch),
    }
  }
  out.push('"');
  out
}

/// Escape special XML characters for macOS LaunchAgent property lists.
#[cfg(any(target_os = "macos", test))]
pub fn escape_xml(input: &str) -> String {
  input
    .replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
    .replace('\'', "&apos;")
}

/// Render an XDG autostart `.desktop` entry pointing at `exe_path`.
pub fn render_linux_autostart_desktop(exe_path: &Path) -> String {
  let quoted = quote_desktop_exec(exe_path);
  format!(
    "[Desktop Entry]\n\
Type=Application\n\
Version=1.0\n\
Name=shotify (Rust Edition)\n\
Comment=Capture, annotate, and style screenshots\n\
Exec={quoted}\n\
Icon=shotify-rust\n\
Terminal=false\n\
Categories=Utility;Graphics;\n\
X-GNOME-Autostart-enabled=true\n"
  )
}

/// Render a macOS `LaunchAgents` plist pointing at `exe_path`.
#[cfg(any(target_os = "macos", test))]
pub fn render_macos_launch_agent_plist(exe_path: &Path) -> String {
  let escaped_exe = escape_xml(&exe_path.display().to_string());
  format!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n\
<dict>\n\
  <key>Label</key>\n\
  <string>{MACOS_LAUNCH_AGENT_LABEL}</string>\n\
  <key>ProgramArguments</key>\n\
  <array>\n\
    <string>{escaped_exe}</string>\n\
  </array>\n\
  <key>RunAtLoad</key>\n\
  <true/>\n\
</dict>\n\
</plist>\n"
  )
}

/// Write or remove a Linux autostart `.desktop` entry inside `autostart_dir`.
pub fn sync_linux_autostart_in_dir(
  enabled: bool,
  autostart_dir: &Path,
  exe_path: &Path,
) -> Result<(), String> {
  let target = autostart_dir.join(RUST_AUTOSTART_DESKTOP_NAME);
  if enabled {
    fs::create_dir_all(autostart_dir)
      .map_err(|e| format!("Failed to create autostart directory: {e}"))?;
    fs::write(&target, render_linux_autostart_desktop(exe_path))
      .map_err(|e| format!("Failed to write {}: {e}", target.display()))
  } else if target.exists() {
    fs::remove_file(&target).map_err(|e| format!("Failed to remove {}: {e}", target.display()))
  } else {
    Ok(())
  }
}

/// Write or remove a macOS LaunchAgent `.plist` inside `launch_agents_dir`.
#[cfg(any(target_os = "macos", test))]
pub fn sync_macos_autostart_in_dir(
  enabled: bool,
  launch_agents_dir: &Path,
  exe_path: &Path,
) -> Result<(), String> {
  let target = launch_agents_dir.join(MACOS_LAUNCH_AGENT_PLIST);
  if enabled {
    fs::create_dir_all(launch_agents_dir)
      .map_err(|e| format!("Failed to create LaunchAgents directory: {e}"))?;
    fs::write(&target, render_macos_launch_agent_plist(exe_path))
      .map_err(|e| format!("Failed to write {}: {e}", target.display()))
  } else if target.exists() {
    fs::remove_file(&target).map_err(|e| format!("Failed to remove {}: {e}", target.display()))
  } else {
    Ok(())
  }
}

fn default_linux_autostart_dir() -> Option<PathBuf> {
  if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
    let trimmed = xdg.trim();
    if !trimmed.is_empty() {
      return Some(PathBuf::from(trimmed).join("autostart"));
    }
  }
  dirs::home_dir().map(|h| h.join(".config").join("autostart"))
}

#[cfg(target_os = "macos")]
fn default_macos_launch_agents_dir() -> Option<PathBuf> {
  dirs::home_dir().map(|h| h.join("Library").join("LaunchAgents"))
}

/// Apply or remove the OS open-at-login registration for `shotify-rust`.
pub fn set_open_at_login_enabled(enabled: bool) -> Result<(), String> {
  let exe = std::env::current_exe().map_err(|e| format!("Cannot resolve current executable: {e}"))?;

  #[cfg(target_os = "linux")]
  {
    let dir = default_linux_autostart_dir()
      .ok_or_else(|| "Cannot resolve XDG autostart directory".to_string())?;
    sync_linux_autostart_in_dir(enabled, &dir, &exe)
  }

  #[cfg(target_os = "macos")]
  {
    let _ = &default_linux_autostart_dir;
    let dir = default_macos_launch_agents_dir()
      .ok_or_else(|| "Cannot resolve ~/Library/LaunchAgents".to_string())?;
    sync_macos_autostart_in_dir(enabled, &dir, &exe)
  }

  #[cfg(target_os = "windows")]
  {
    let _ = &default_linux_autostart_dir;
    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    let status = if enabled {
      let val = format!("\"{}\"", exe.display());
      std::process::Command::new("reg")
        .args(["add", key, "/v", WINDOWS_RUN_VALUE_NAME, "/t", "REG_SZ", "/d", &val, "/f"])
        .status()
        .map_err(|e| format!("Failed to invoke reg.exe: {e}"))?
    } else {
      std::process::Command::new("reg")
        .args(["delete", key, "/v", WINDOWS_RUN_VALUE_NAME, "/f"])
        .status()
        .map_err(|e| format!("Failed to invoke reg.exe: {e}"))?
    };
    if status.success() || !enabled {
      Ok(())
    } else {
      Err(format!("Windows autostart reg exited with {status}"))
    }
  }

  #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
  {
    let _ = (enabled, exe, default_linux_autostart_dir());
    Err("Autostart is not supported on this OS".to_string())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::time::{SystemTime, UNIX_EPOCH};

  fn temp_test_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap_or_default()
      .as_nanos();
    let dir = std::env::temp_dir().join(format!("shotify_rust_{prefix}_{nanos}"));
    let _ = fs::create_dir_all(&dir);
    dir
  }

  #[test]
  fn linux_autostart_creates_and_removes_distinct_desktop_entry() {
    let dir = temp_test_dir("autostart_linux");
    let exe = PathBuf::from("/home/user/My Apps/shotify-rust");

    sync_linux_autostart_in_dir(true, &dir, &exe).unwrap();
    let entry_path = dir.join(RUST_AUTOSTART_DESKTOP_NAME);
    assert!(entry_path.exists());
    let contents = fs::read_to_string(&entry_path).unwrap();
    assert!(contents.contains("Exec=\"/home/user/My Apps/shotify-rust\""));
    assert!(contents.contains("Icon=shotify-rust"));

    sync_linux_autostart_in_dir(false, &dir, &exe).unwrap();
    assert!(!entry_path.exists());
    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn macos_plist_escapes_xml_and_cleans_up() {
    let dir = temp_test_dir("autostart_macos");
    let exe = PathBuf::from("/Applications/shotify & Rust.app/Contents/MacOS/shotify-rust");

    sync_macos_autostart_in_dir(true, &dir, &exe).unwrap();
    let plist_path = dir.join(MACOS_LAUNCH_AGENT_PLIST);
    let content = fs::read_to_string(&plist_path).unwrap();
    assert!(content.contains("<string>com.shotify.rust</string>"));
    assert!(content.contains("shotify &amp; Rust.app"));

    sync_macos_autostart_in_dir(false, &dir, &exe).unwrap();
    assert!(!plist_path.exists());
    let _ = fs::remove_dir_all(&dir);
  }
}
