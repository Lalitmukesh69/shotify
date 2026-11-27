use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc::UnboundedSender;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

use crate::platform::PlatformCommand;

static RUST_TRAY_ICON_PNG: &[u8] = include_bytes!("../../../../assets/icons/64x64.png");

const MENU_OPEN: &str = "tray-open";
const MENU_CAPTURE: &str = "tray-capture";
const MENU_SETTINGS: &str = "tray-settings";
const MENU_QUIT: &str = "tray-quit";

/// Owns the live OS tray icon / GTK loop so closing the main window can stay resident in tray.
pub struct TrayHolder {
  pub available: bool,
  #[cfg(target_os = "linux")]
  stop_flag: Option<Arc<AtomicBool>>,
  #[cfg(target_os = "linux")]
  gtk_thread: Option<std::thread::JoinHandle<()>>,
  #[cfg(not(target_os = "linux"))]
  _tray_icon: Option<tray_icon::TrayIcon>,
}

impl Drop for TrayHolder {
  fn drop(&mut self) {
    #[cfg(target_os = "linux")]
    {
      if let Some(flag) = self.stop_flag.take() {
        flag.store(true, Ordering::SeqCst);
      }
      if let Some(handle) = self.gtk_thread.take() {
        let _ = handle.join();
      }
    }
  }
}

fn load_rust_tray_icon() -> Result<Icon, String> {
  let dyn_img = image::load_from_memory(RUST_TRAY_ICON_PNG)
    .map_err(|e| format!("Failed decoding red tray icon asset: {e}"))?;
  let rgba = dyn_img.to_rgba8();
  let (w, h) = rgba.dimensions();
  Icon::from_rgba(rgba.into_raw(), w, h).map_err(|e| format!("Bad tray RGBA icon: {e}"))
}

fn build_tray_menu() -> Result<Menu, String> {
  let menu = Menu::new();
  let item_open = MenuItem::with_id(MENU_OPEN, "Open shotify (Rust)", true, None);
  let item_capture = MenuItem::with_id(MENU_CAPTURE, "Capture region", true, None);
  let item_settings = MenuItem::with_id(MENU_SETTINGS, "Settings", true, None);
  let sep = PredefinedMenuItem::separator();
  let item_quit = MenuItem::with_id(MENU_QUIT, "Quit", true, None);

  menu
    .append_items(&[&item_open, &item_capture, &item_settings, &sep, &item_quit])
    .map_err(|e| format!("Failed building tray menu: {e}"))?;
  Ok(menu)
}

fn install_tray_event_handlers(tx: UnboundedSender<PlatformCommand>) {
  let menu_tx = tx.clone();
  MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
    let cmd = match event.id().as_ref() {
      MENU_OPEN => Some(PlatformCommand::ShowWindow),
      MENU_CAPTURE => Some(PlatformCommand::CaptureRegion),
      MENU_SETTINGS => Some(PlatformCommand::OpenSettings),
      MENU_QUIT => Some(PlatformCommand::QuitApp),
      _ => None,
    };
    if let Some(cmd) = cmd {
      let _ = menu_tx.send(cmd);
    }
  }));

  TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| match event {
    TrayIconEvent::Click {
      button: MouseButton::Left,
      button_state: MouseButtonState::Up,
      ..
    }
    | TrayIconEvent::DoubleClick {
      button: MouseButton::Left,
      ..
    } => {
      let _ = tx.send(PlatformCommand::ShowWindow);
    }
    _ => {}
  }));
}

/// Initialize the native system tray icon (with red-tinted logo) and bridge menu events to `tx`.
/// Gracefully degrades on Linux sessions without a tray host so startup never aborts.
pub fn initialize_tray(tx: UnboundedSender<PlatformCommand>) -> TrayHolder {
  let _ = &Arc::new(AtomicBool::new(false));
  let _ = Ordering::SeqCst;

  install_tray_event_handlers(tx);

  #[cfg(target_os = "linux")]
  {
    let stop_flag = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop_flag);
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<bool>(1);

    let join = std::thread::Builder::new()
      .name("shotify-tray-gtk".to_string())
      .spawn(move || {
        if gtk::init().is_err() {
          let _ = ready_tx.send(false);
          return;
        }

        let icon = match load_rust_tray_icon() {
          Ok(i) => i,
          Err(_) => {
            let _ = ready_tx.send(false);
            return;
          }
        };
        let menu = match build_tray_menu() {
          Ok(m) => m,
          Err(_) => {
            let _ = ready_tx.send(false);
            return;
          }
        };

        let tray = match TrayIconBuilder::new()
          .with_menu(Box::new(menu))
          .with_icon(icon)
          .build()
        {
          Ok(t) => t,
          Err(err) => {
            tracing::warn!("[tray] system tray host unavailable: {err}");
            let _ = ready_tx.send(false);
            return;
          }
        };

        let _ = ready_tx.send(true);
        while !thread_stop.load(Ordering::SeqCst) {
          while gtk::events_pending() {
            gtk::main_iteration_do(false);
          }
          std::thread::sleep(std::time::Duration::from_millis(25));
        }
        drop(tray);
      });

    match join {
      Ok(handle) => {
        let ok = ready_rx
          .recv_timeout(std::time::Duration::from_secs(2))
          .unwrap_or(false);
        if ok {
          TrayHolder {
            available: true,
            stop_flag: Some(stop_flag),
            gtk_thread: Some(handle),
          }
        } else {
          stop_flag.store(true, Ordering::SeqCst);
          TrayHolder {
            available: false,
            stop_flag: None,
            gtk_thread: None,
          }
        }
      }
      Err(_) => TrayHolder {
        available: false,
        stop_flag: None,
        gtk_thread: None,
      },
    }
  }

  #[cfg(not(target_os = "linux"))]
  {
    let icon = match load_rust_tray_icon() {
      Ok(i) => i,
      Err(err) => {
        tracing::warn!("[tray] failed loading icon: {err}");
        return TrayHolder {
          available: false,
          _tray_icon: None,
        };
      }
    };
    let menu = match build_tray_menu() {
      Ok(m) => m,
      Err(err) => {
        tracing::warn!("[tray] failed building menu: {err}");
        return TrayHolder {
          available: false,
          _tray_icon: None,
        };
      }
    };
    match TrayIconBuilder::new()
      .with_menu(Box::new(menu))
      .with_tooltip("shotify (Rust Edition)")
      .with_icon(icon)
      .build()
    {
      Ok(tray) => TrayHolder {
        available: true,
        _tray_icon: Some(tray),
      },
      Err(err) => {
        tracing::warn!("[tray] failed building OS tray icon: {err}");
        TrayHolder {
          available: false,
          _tray_icon: None,
        }
      }
    }
  }
}
