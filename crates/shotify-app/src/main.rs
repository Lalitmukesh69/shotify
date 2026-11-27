mod capture;
mod config;
mod message;
mod platform;
mod state;
mod theme;
mod update;
mod views;
mod widgets;

use std::sync::{Mutex, OnceLock};

use iced::futures::{SinkExt, Stream};
use iced::stream::channel;
use iced::{window, Element, Size, Subscription, Task, Theme};
use tokio::sync::mpsc;

use crate::config::load_or_initialize_config;
use crate::message::Message;
use crate::platform::single_instance::{
  acquire_or_forward_at, default_ipc_endpoint_path, parse_cli_launch_command,
  should_allow_multiple, start_hidden_flag, LaunchCommand, SingleInstanceOutcome,
};
use crate::platform::tray::initialize_tray;
use crate::platform::PlatformCommand;
use crate::state::{ShotifyApp, ViewMode};
use crate::views::capture_overlay::capture_overlay_view;
use crate::views::editor::editor_view;

static RUST_APP_ICON_PNG: &[u8] = include_bytes!("../../../assets/icons/128x128@2x.png");

static PLATFORM_CMD_RX: OnceLock<Mutex<Option<mpsc::UnboundedReceiver<PlatformCommand>>>> =
  OnceLock::new();

fn load_rust_window_icon() -> Option<window::Icon> {
  let dyn_img = image::load_from_memory(RUST_APP_ICON_PNG).ok()?;
  let rgba = dyn_img.to_rgba8();
  let (w, h) = rgba.dimensions();
  window::icon::from_rgba(rgba.into_raw(), w, h).ok()
}

fn view_root(app: &ShotifyApp) -> Element<'_, Message> {
  match app.view_mode {
    ViewMode::Editor => editor_view(app),
    ViewMode::CaptureOverlay => capture_overlay_view(app),
  }
}

fn theme_root(app: &ShotifyApp) -> Theme {
  app.theme_mode().to_iced_theme()
}

fn platform_events_stream() -> impl Stream<Item = Message> {
  channel(64, |mut sender| async move {
    let rx_opt = PLATFORM_CMD_RX
      .get()
      .and_then(|cell| cell.lock().ok())
      .and_then(|mut guard| guard.take());

    let Some(mut rx) = rx_opt else {
      std::future::pending::<()>().await;
      return;
    };

    while let Some(cmd) = rx.recv().await {
      if sender
        .send(Message::PlatformCommandReceived(cmd))
        .await
        .is_err()
      {
        break;
      }
    }
  })
}

fn map_window_event(
  event: iced::Event,
  window_id: window::Id,
) -> Option<Message> {
  match event {
    iced::Event::Window(window::Event::Opened { .. }) => Some(Message::WindowOpened(window_id)),
    iced::Event::Window(window::Event::Resized(size)) => {
      Some(Message::WindowResized(window_id, size))
    }
    iced::Event::Window(window::Event::CloseRequested) => {
      Some(Message::WindowCloseRequested(window_id))
    }
    iced::Event::Window(window::Event::Focused) => Some(Message::WindowFocused(window_id)),
    iced::Event::Window(window::Event::Unfocused) => Some(Message::WindowUnfocused(window_id)),
    iced::Event::Window(window::Event::FileDropped(path)) => {
      Some(Message::WindowFileDropped(path))
    }
    _ => None,
  }
}

fn listen_normal_events(
  event: iced::Event,
  status: iced::event::Status,
  window_id: window::Id,
) -> Option<Message> {
  if let Some(msg) = map_window_event(event.clone(), window_id) {
    return Some(msg);
  }
  match event {
    iced::Event::Keyboard(kb_ev) if status == iced::event::Status::Ignored => {
      Some(Message::KeyboardEvent(kb_ev))
    }
    _ => None,
  }
}

fn listen_all_events(
  event: iced::Event,
  _status: iced::event::Status,
  window_id: window::Id,
) -> Option<Message> {
  if let Some(msg) = map_window_event(event.clone(), window_id) {
    return Some(msg);
  }
  match event {
    iced::Event::Keyboard(kb_ev) => Some(Message::KeyboardEvent(kb_ev)),
    _ => None,
  }
}

fn subscription_root(app: &ShotifyApp) -> Subscription<Message> {
  let listener: fn(iced::Event, iced::event::Status, window::Id) -> Option<Message> =
    if app.recording_shortcut.is_some() {
      listen_all_events
    } else {
      listen_normal_events
    };
  let window_and_keys = iced::event::listen_with(listener);

  Subscription::batch([window_and_keys, Subscription::run(platform_events_stream)])
}

pub fn main() -> iced::Result {
  let args: Vec<String> = std::env::args().skip(1).collect();
  let launch_cmd = parse_cli_launch_command(&args);
  let allow_multiple = should_allow_multiple(&args);
  let cli_start_hidden = start_hidden_flag(&args);

  let (platform_tx, platform_rx) = mpsc::unbounded_channel::<PlatformCommand>();

  let _instance_guard = if allow_multiple {
    None
  } else {
    match acquire_or_forward_at(
      &default_ipc_endpoint_path(),
      launch_cmd,
      platform_tx.clone(),
    ) {
      Ok(SingleInstanceOutcome::Primary(guard)) => Some(guard),
      Ok(SingleInstanceOutcome::ForwardedToPrimary) => {
        return Ok(());
      }
      Err(err) => {
        eprintln!("[shotify-rust] single-instance check bypassed: {err}");
        None
      }
    }
  };

  let _ = PLATFORM_CMD_RX.set(Mutex::new(Some(platform_rx)));

  let tray_holder = initialize_tray(platform_tx.clone());
  let tray_available = tray_holder.available;

  let (pre_cfg, _, _) = load_or_initialize_config();
  let start_hidden = launch_cmd == LaunchCommand::Show
    && tray_available
    && (cli_start_hidden || pre_cfg.general.hide_window_on_launch);

  let mut win = window::Settings {
    size: Size::new(1280.0, 800.0),
    min_size: Some(Size::new(900.0, 600.0)),
    decorations: cfg!(target_os = "macos"),
    visible: !start_hidden,
    icon: load_rust_window_icon(),
    ..Default::default()
  };
  #[cfg(target_os = "linux")]
  {
    win.platform_specific.application_id = "shotify-rust".to_string();
  }

  iced::application("shotify (Rust Edition)", ShotifyApp::update, view_root)
    .theme(theme_root)
    .subscription(subscription_root)
    .window(win)
    .exit_on_close_request(false)
    .run_with(move || {
      let app = ShotifyApp::boot(tray_available, platform_tx, launch_cmd, start_hidden);
      (app, Task::done(Message::InitializePlatformServices))
    })
}
