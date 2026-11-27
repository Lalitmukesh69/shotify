use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use iced::keyboard::{self, key::Named, Key};
use tokio::sync::mpsc::UnboundedSender;

use crate::config::ShortcutSettings;
use crate::platform::{is_wayland_session, GlobalShortcutBackend, PlatformCommand};

/// Parse a user-facing shortcut string like `"Ctrl+Shift+8"` or `"Cmd+S"` into a `HotKey`.
pub fn parse_accelerator(raw: &str) -> Result<HotKey, String> {
  let trimmed = raw.trim();
  if trimmed.is_empty() {
    return Err("Shortcut cannot be empty".to_string());
  }

  let parts: Vec<&str> = trimmed
    .split('+')
    .map(str::trim)
    .filter(|p| !p.is_empty())
    .collect();
  if parts.is_empty() {
    return Err("Shortcut cannot be empty".to_string());
  }

  let mut mods = Modifiers::empty();
  let mut key_code: Option<Code> = None;

  for (idx, token) in parts.iter().enumerate() {
    let lower = token.to_ascii_lowercase();
    let is_last = idx + 1 == parts.len();
    if !is_last {
      match lower.as_str() {
        "ctrl" | "control" => mods |= Modifiers::CONTROL,
        "cmd" | "command" | "super" | "meta" => mods |= Modifiers::SUPER,
        "cmdorctrl" | "commandorcontrol" => {
          mods |= if cfg!(target_os = "macos") {
            Modifiers::SUPER
          } else {
            Modifiers::CONTROL
          };
        }
        "alt" | "option" => mods |= Modifiers::ALT,
        "shift" => mods |= Modifiers::SHIFT,
        other => return Err(format!("Unknown modifier '{other}' in '{raw}'")),
      }
    } else {
      key_code = Some(parse_key_token(token)?);
    }
  }

  let code = key_code.ok_or_else(|| format!("Missing main key in '{raw}'"))?;
  if mods.is_empty() {
    Ok(HotKey::new(None, code))
  } else {
    Ok(HotKey::new(Some(mods), code))
  }
}

fn parse_key_token(token: &str) -> Result<Code, String> {
  let upper = token.trim().to_ascii_uppercase();
  if upper.len() == 1 {
    let ch = upper.chars().next().unwrap_or(' ');
    return match ch {
      'A' => Ok(Code::KeyA),
      'B' => Ok(Code::KeyB),
      'C' => Ok(Code::KeyC),
      'D' => Ok(Code::KeyD),
      'E' => Ok(Code::KeyE),
      'F' => Ok(Code::KeyF),
      'G' => Ok(Code::KeyG),
      'H' => Ok(Code::KeyH),
      'I' => Ok(Code::KeyI),
      'J' => Ok(Code::KeyJ),
      'K' => Ok(Code::KeyK),
      'L' => Ok(Code::KeyL),
      'M' => Ok(Code::KeyM),
      'N' => Ok(Code::KeyN),
      'O' => Ok(Code::KeyO),
      'P' => Ok(Code::KeyP),
      'Q' => Ok(Code::KeyQ),
      'R' => Ok(Code::KeyR),
      'S' => Ok(Code::KeyS),
      'T' => Ok(Code::KeyT),
      'U' => Ok(Code::KeyU),
      'V' => Ok(Code::KeyV),
      'W' => Ok(Code::KeyW),
      'X' => Ok(Code::KeyX),
      'Y' => Ok(Code::KeyY),
      'Z' => Ok(Code::KeyZ),
      '0' => Ok(Code::Digit0),
      '1' => Ok(Code::Digit1),
      '2' => Ok(Code::Digit2),
      '3' => Ok(Code::Digit3),
      '4' => Ok(Code::Digit4),
      '5' => Ok(Code::Digit5),
      '6' => Ok(Code::Digit6),
      '7' => Ok(Code::Digit7),
      '8' => Ok(Code::Digit8),
      '9' => Ok(Code::Digit9),
      '-' => Ok(Code::Minus),
      '=' => Ok(Code::Equal),
      ',' => Ok(Code::Comma),
      '.' => Ok(Code::Period),
      '/' => Ok(Code::Slash),
      '\\' => Ok(Code::Backslash),
      '[' => Ok(Code::BracketLeft),
      ']' => Ok(Code::BracketRight),
      ';' => Ok(Code::Semicolon),
      '\'' => Ok(Code::Quote),
      '`' => Ok(Code::Backquote),
      _ => Err(format!("Unsupported shortcut key '{token}'")),
    };
  }

  match upper.as_str() {
    "SPACE" => Ok(Code::Space),
    "ENTER" | "RETURN" => Ok(Code::Enter),
    "ESC" | "ESCAPE" => Ok(Code::Escape),
    "TAB" => Ok(Code::Tab),
    "BACKSPACE" => Ok(Code::Backspace),
    "DEL" | "DELETE" => Ok(Code::Delete),
    "UP" | "ARROWUP" => Ok(Code::ArrowUp),
    "DOWN" | "ARROWDOWN" => Ok(Code::ArrowDown),
    "LEFT" | "ARROWLEFT" => Ok(Code::ArrowLeft),
    "RIGHT" | "ARROWRIGHT" => Ok(Code::ArrowRight),
    "PRINTSCREEN" | "PRINT" => Ok(Code::PrintScreen),
    "MINUS" => Ok(Code::Minus),
    "EQUAL" => Ok(Code::Equal),
    "COMMA" => Ok(Code::Comma),
    "PERIOD" => Ok(Code::Period),
    "SLASH" => Ok(Code::Slash),
    "BACKSLASH" => Ok(Code::Backslash),
    "F1" => Ok(Code::F1),
    "F2" => Ok(Code::F2),
    "F3" => Ok(Code::F3),
    "F4" => Ok(Code::F4),
    "F5" => Ok(Code::F5),
    "F6" => Ok(Code::F6),
    "F7" => Ok(Code::F7),
    "F8" => Ok(Code::F8),
    "F9" => Ok(Code::F9),
    "F10" => Ok(Code::F10),
    "F11" => Ok(Code::F11),
    "F12" => Ok(Code::F12),
    _ => Err(format!("Unsupported shortcut key '{token}'")),
  }
}

/// Validate that all 5 shortcuts parse cleanly and don't collide.
pub fn validate_shortcut_set(sc: &ShortcutSettings) -> Result<(), String> {
  let items = [
    ("Copy", &sc.copy),
    ("Save", &sc.save),
    ("Save to folder", &sc.save_to_folder),
    ("Capture region", &sc.screenshot_region),
    ("Capture region + copy", &sc.screenshot_region_copy),
  ];

  let mut seen = HashSet::new();
  for (label, raw) in items {
    let hk = parse_accelerator(raw).map_err(|e| format!("{label}: {e}"))?;
    if !seen.insert((hk.mods, hk.key)) {
      return Err(format!("Duplicate shortcut '{raw}' on {label}"));
    }
  }
  Ok(())
}

/// Convert an Iced key press event into a normalized accelerator string like `"Ctrl+Shift+8"`.
/// Returns `None` if no non-modifier key was pressed or if a bare key has no modifier (except F-keys).
pub fn iced_key_to_combo(key: &Key, modifiers: keyboard::Modifiers) -> Option<String> {
  let key_name = match key {
    Key::Character(ch) => {
      let clean = ch.as_str().trim();
      if clean.is_empty() {
        return None;
      }
      clean.to_ascii_uppercase()
    }
    Key::Named(named) => match named {
      Named::Space => "Space".to_string(),
      Named::Enter => "Enter".to_string(),
      Named::Tab => "Tab".to_string(),
      Named::Backspace => "Backspace".to_string(),
      Named::Delete => "Delete".to_string(),
      Named::ArrowUp => "ArrowUp".to_string(),
      Named::ArrowDown => "ArrowDown".to_string(),
      Named::ArrowLeft => "ArrowLeft".to_string(),
      Named::ArrowRight => "ArrowRight".to_string(),
      Named::PrintScreen => "PrintScreen".to_string(),
      Named::F1 => "F1".to_string(),
      Named::F2 => "F2".to_string(),
      Named::F3 => "F3".to_string(),
      Named::F4 => "F4".to_string(),
      Named::F5 => "F5".to_string(),
      Named::F6 => "F6".to_string(),
      Named::F7 => "F7".to_string(),
      Named::F8 => "F8".to_string(),
      Named::F9 => "F9".to_string(),
      Named::F10 => "F10".to_string(),
      Named::F11 => "F11".to_string(),
      Named::F12 => "F12".to_string(),
      _ => return None,
    },
    Key::Unidentified => return None,
  };

  let mut tokens: Vec<&'static str> = Vec::new();
  if modifiers.contains(keyboard::Modifiers::CTRL) {
    tokens.push("Ctrl");
  }
  if modifiers.contains(keyboard::Modifiers::LOGO) {
    tokens.push("Cmd");
  }
  if modifiers.contains(keyboard::Modifiers::ALT) {
    tokens.push("Alt");
  }
  if modifiers.contains(keyboard::Modifiers::SHIFT) {
    tokens.push("Shift");
  }

  let is_function_key = key_name.starts_with('F') && key_name.len() >= 2;
  if tokens.is_empty() && !is_function_key {
    return None;
  }

  let combo = if tokens.is_empty() {
    key_name
  } else {
    format!("{}+{key_name}", tokens.join("+"))
  };
  parse_accelerator(&combo).ok()?;
  Some(combo)
}

/// Check whether an Iced key event matches a configured accelerator string (like `"Ctrl+Shift+S"`).
pub fn matches_iced_shortcut(
  combo: &str,
  key: &Key,
  modifiers: keyboard::Modifiers,
) -> bool {
  let Ok(target) = parse_accelerator(combo) else {
    return false;
  };
  let mut pressed_mods = Modifiers::empty();
  if modifiers.contains(keyboard::Modifiers::CTRL) {
    pressed_mods |= Modifiers::CONTROL;
  }
  if modifiers.contains(keyboard::Modifiers::LOGO) {
    pressed_mods |= Modifiers::SUPER;
  }
  if modifiers.contains(keyboard::Modifiers::ALT) {
    pressed_mods |= Modifiers::ALT;
  }
  if modifiers.contains(keyboard::Modifiers::SHIFT) {
    pressed_mods |= Modifiers::SHIFT;
  }

  if pressed_mods != target.mods {
    return false;
  }

  let Some(pressed_code) = iced_key_to_code(key) else {
    return false;
  };
  pressed_code == target.key
}

fn iced_key_to_code(key: &Key) -> Option<Code> {
  match key {
    Key::Character(ch) => parse_key_token(ch.as_str()).ok(),
    Key::Named(n) => {
      let s = match n {
        Named::Space => "Space",
        Named::Enter => "Enter",
        Named::Escape => "Escape",
        Named::Tab => "Tab",
        Named::Backspace => "Backspace",
        Named::Delete => "Delete",
        Named::ArrowUp => "ArrowUp",
        Named::ArrowDown => "ArrowDown",
        Named::ArrowLeft => "ArrowLeft",
        Named::ArrowRight => "ArrowRight",
        Named::PrintScreen => "PrintScreen",
        Named::F1 => "F1",
        Named::F2 => "F2",
        Named::F3 => "F3",
        Named::F4 => "F4",
        Named::F5 => "F5",
        Named::F6 => "F6",
        Named::F7 => "F7",
        Named::F8 => "F8",
        Named::F9 => "F9",
        Named::F10 => "F10",
        Named::F11 => "F11",
        Named::F12 => "F12",
        _ => return None,
      };
      parse_key_token(s).ok()
    }
    Key::Unidentified => None,
  }
}

/// Convert `"Ctrl+Shift+8"` to an XDG GlobalShortcuts portal hint like `"<Control><Shift>8"`.
#[allow(dead_code)]
pub fn combo_to_xdg_trigger(raw: &str) -> String {
  let Ok(hk) = parse_accelerator(raw) else {
    return "<Control><Shift>8".to_string();
  };
  let mut out = String::new();
  if hk.mods.contains(Modifiers::CONTROL) {
    out.push_str("<Control>");
  }
  if hk.mods.contains(Modifiers::SUPER) {
    out.push_str("<Super>");
  }
  if hk.mods.contains(Modifiers::ALT) {
    out.push_str("<Alt>");
  }
  if hk.mods.contains(Modifiers::SHIFT) {
    out.push_str("<Shift>");
  }
  let main_key = raw
    .split('+')
    .last()
    .map(|s| s.trim().to_ascii_lowercase())
    .unwrap_or_else(|| "8".to_string());
  out.push_str(&main_key);
  out
}

/// Manages `global-hotkey` registrations on X11, macOS, and Windows.
pub struct GlobalShortcutController {
  manager: Option<GlobalHotKeyManager>,
  registered: Vec<HotKey>,
  id_map: Arc<Mutex<HashMap<u32, PlatformCommand>>>,
}

impl GlobalShortcutController {
  /// Initialize global hotkey event dispatch and OS manager (skipped on Wayland where XDG portal is used).
  pub fn new(tx: UnboundedSender<PlatformCommand>) -> Self {
    let id_map: Arc<Mutex<HashMap<u32, PlatformCommand>>> = Arc::new(Mutex::new(HashMap::new()));
    let handler_map = Arc::clone(&id_map);

    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
      if event.state != HotKeyState::Pressed {
        return;
      }
      if let Ok(guard) = handler_map.lock() {
        if let Some(cmd) = guard.get(&event.id) {
          let _ = tx.send(cmd.clone());
        }
      }
    }));

    let manager = if is_wayland_session() {
      None
    } else {
      match GlobalHotKeyManager::new() {
        Ok(m) => Some(m),
        Err(err) => {
          tracing::warn!("[shortcuts] GlobalHotKeyManager unavailable: {err}");
          None
        }
      }
    };

    Self {
      manager,
      registered: Vec::new(),
      id_map,
    }
  }

  /// Sync active OS-level global shortcuts (`screenshot_region`, `screenshot_region_copy`).
  /// Returns `Ok(true)` if native shortcuts were registered, `Ok(false)` on Wayland/headless.
  pub fn sync(&mut self, settings: &ShortcutSettings) -> Result<bool, String> {
    let Some(ref manager) = self.manager else {
      return Ok(false);
    };

    if !self.registered.is_empty() {
      let _ = manager.unregister_all(&self.registered);
      self.registered.clear();
    }
    if let Ok(mut guard) = self.id_map.lock() {
      guard.clear();
    }

    let pairs = [
      (&settings.screenshot_region, PlatformCommand::CaptureRegion),
      (
        &settings.screenshot_region_copy,
        PlatformCommand::CaptureRegionCopy,
      ),
    ];

    for (combo, cmd) in pairs {
      let hk = parse_accelerator(combo)?;
      manager
        .register(hk)
        .map_err(|e| format!("Failed registering global shortcut '{combo}': {e}"))?;
      if let Ok(mut guard) = self.id_map.lock() {
        guard.insert(hk.id, cmd);
      }
      self.registered.push(hk);
    }

    Ok(true)
  }
}

/// Start Linux Wayland `xdg-desktop-portal` GlobalShortcuts background session.
#[cfg(target_os = "linux")]
pub async fn run_wayland_portal_shortcuts(
  region_combo: String,
  region_copy_combo: String,
  tx: UnboundedSender<PlatformCommand>,
) {
  use ashpd::desktop::global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut};
  use ashpd::desktop::CreateSessionOptions;
  use futures_util::StreamExt;

  let gs = match GlobalShortcuts::new().await {
    Ok(proxy) => proxy,
    Err(err) => {
      tracing::info!("[shortcuts] Wayland GlobalShortcuts portal unavailable: {err}");
      let _ = tx.send(PlatformCommand::ShortcutBackendUpdated {
        backend: GlobalShortcutBackend::CliOnly,
        portal_active: false,
      });
      return;
    }
  };

  let session = match gs.create_session(CreateSessionOptions::default()).await {
    Ok(s) => s,
    Err(err) => {
      tracing::info!("[shortcuts] Wayland GlobalShortcuts session unavailable: {err}");
      let _ = tx.send(PlatformCommand::ShortcutBackendUpdated {
        backend: GlobalShortcutBackend::CliOnly,
        portal_active: false,
      });
      return;
    }
  };

  let region_trigger = combo_to_xdg_trigger(&region_combo);
  let copy_trigger = combo_to_xdg_trigger(&region_copy_combo);
  let shortcuts = [
    NewShortcut::new("capture_region", "Capture screen region").preferred_trigger(&*region_trigger),
    NewShortcut::new("capture_region_copy", "Capture region and copy")
      .preferred_trigger(&*copy_trigger),
  ];

  if let Err(err) = gs
    .bind_shortcuts(&session, &shortcuts, None, BindShortcutsOptions::default())
    .await
  {
    tracing::info!("[shortcuts] Wayland portal bind_shortcuts unavailable: {err}");
    let _ = tx.send(PlatformCommand::ShortcutBackendUpdated {
      backend: GlobalShortcutBackend::CliOnly,
      portal_active: false,
    });
    return;
  }

  let mut stream = match gs.receive_activated().await {
    Ok(s) => s,
    Err(err) => {
      tracing::warn!("[shortcuts] Failed subscribing to Wayland portal activations: {err}");
      let _ = tx.send(PlatformCommand::ShortcutBackendUpdated {
        backend: GlobalShortcutBackend::CliOnly,
        portal_active: false,
      });
      return;
    }
  };

  let _ = tx.send(PlatformCommand::ShortcutBackendUpdated {
    backend: GlobalShortcutBackend::WaylandPortal,
    portal_active: true,
  });

  while let Some(activated) = stream.next().await {
    match activated.shortcut_id() {
      "capture_region" => {
        let _ = tx.send(PlatformCommand::CaptureRegion);
      }
      "capture_region_copy" => {
        let _ = tx.send(PlatformCommand::CaptureRegionCopy);
      }
      _ => {}
    }
  }

  drop(session);
}

#[cfg(not(target_os = "linux"))]
pub async fn run_wayland_portal_shortcuts(
  _region_combo: String,
  _region_copy_combo: String,
  _tx: UnboundedSender<PlatformCommand>,
) {
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_cross_platform_accelerators() {
    let hk = parse_accelerator("Ctrl+Shift+8").unwrap();
    assert_eq!(hk.mods, Modifiers::CONTROL | Modifiers::SHIFT);
    assert_eq!(hk.key, Code::Digit8);

    let hk_mac = parse_accelerator("Cmd+S").unwrap();
    assert_eq!(hk_mac.mods, Modifiers::SUPER);
    assert_eq!(hk_mac.key, Code::KeyS);

    let hk_fn = parse_accelerator("Alt+F12").unwrap();
    assert_eq!(hk_fn.mods, Modifiers::ALT);
    assert_eq!(hk_fn.key, Code::F12);
  }

  #[test]
  fn validates_conflicts_and_matches_iced_keys() {
    let mut sc = ShortcutSettings::default();
    assert!(validate_shortcut_set(&sc).is_ok());

    sc.screenshot_region = sc.copy.clone();
    assert!(validate_shortcut_set(&sc).is_err());

    assert!(matches_iced_shortcut(
      "Ctrl+Shift+S",
      &Key::Character("s".into()),
      keyboard::Modifiers::CTRL | keyboard::Modifiers::SHIFT
    ));
    assert!(!matches_iced_shortcut(
      "Ctrl+Shift+S",
      &Key::Character("s".into()),
      keyboard::Modifiers::CTRL
    ));
  }

  #[test]
  fn converts_xdg_portal_trigger_syntax() {
    assert_eq!(combo_to_xdg_trigger("Ctrl+Shift+8"), "<Control><Shift>8");
    assert_eq!(combo_to_xdg_trigger("Cmd+Shift+S"), "<Super><Shift>s");
  }
}
