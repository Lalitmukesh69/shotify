use std::collections::HashMap;
use std::path::PathBuf;

use iced::keyboard::{self, key::Named, Key, Modifiers};
use iced::widget::image::Handle as ImageHandle;
use iced::{window, Task};
use image::RgbaImage;
use shotify_annotate::{
  render_annotations_onto_rgba, selection_bounds_px, AnnotationTool, PointerDownOutcome,
  SelectedAnnotation,
};
use shotify_core::{
  clamp_transform_to_container, compose_screenshot_scene, DragContainerBounds, GradientPreset,
  Point2D, RectF, RenderQuality, SystemWallpaperItem,
};
use shotify_export::{
  apply_scene_watermark, build_default_export_file_name, execute_export, EncodeOptions,
  ExportDestination, ExportFormat, ExportRequest, RasterizeOptions,
};

use crate::capture::capture_primary_display;
use crate::config::{
  builtin_presets, delete_persisted_user_preset, discover_legacy_localstorage_files,
  import_legacy_sources, list_user_presets_in_dir, persist_user_preset, presets_dir_path,
  rename_persisted_user_preset, BackgroundSelection, Preset, ShortcutSettings,
};
use crate::message::{
  ColorPickerTarget, ExportActionKind, ExportOutcomeSummary, GeneralToggleKey, Message,
  RenderedScenePayload, ShadowFieldUpdate, ShortcutFieldKey,
};
use crate::platform::shortcuts::{
  iced_key_to_combo, matches_iced_shortcut, run_wayland_portal_shortcuts, validate_shortcut_set,
  GlobalShortcutController,
};
use crate::platform::single_instance::LaunchCommand;
use crate::platform::updater::{check_for_updates_async, UpdateCheckStatus};
use crate::platform::{
  autostart, is_wayland_session, open_url, vibrancy, GlobalShortcutBackend, PlatformCommand,
};
use crate::state::{rasterize_gradient_preset_rgba, InlineTextPrompt, ShotifyApp, ViewMode};
use crate::theme::ThemeMode;
use crate::widgets::color_picker::rgb_to_hsv;
use crate::widgets::icons::hsv_to_rgb;

fn rgba_to_handle(img: &RgbaImage) -> ImageHandle {
  ImageHandle::from_rgba(img.width(), img.height(), img.as_raw().clone())
}

fn rasterize_algorithm_thumbs_task(presets: Vec<GradientPreset>) -> Task<Message> {
  Task::perform(
    async move {
      tokio::task::spawn_blocking(move || {
        let mut out = HashMap::with_capacity(presets.len());
        for p in &presets {
          let id = match p {
            GradientPreset::Gradient { id, .. } | GradientPreset::BlobGradient { id, .. } => {
              id.clone()
            }
          };
          if let Some(rgba) = rasterize_gradient_preset_rgba(p) {
            out.insert(id, rgba_to_handle(&rgba));
          }
        }
        out
      })
      .await
      .unwrap_or_default()
    },
    Message::GradientThumbsGenerated,
  )
}

fn rasterize_auto_thumbs_task(presets: Vec<GradientPreset>) -> Task<Message> {
  Task::perform(
    async move {
      tokio::task::spawn_blocking(move || {
        let mut out = HashMap::with_capacity(presets.len());
        for p in &presets {
          if let GradientPreset::BlobGradient { config, .. } = p {
            if let Some(rgba) = rasterize_gradient_preset_rgba(p) {
              out.insert(config.seed, rgba_to_handle(&rgba));
            }
          }
        }
        out
      })
      .await
      .unwrap_or_default()
    },
    Message::AutoThumbsGenerated,
  )
}

fn load_system_wallpaper_thumbs_task(items: Vec<SystemWallpaperItem>) -> Task<Message> {
  Task::perform(
    async move {
      tokio::task::spawn_blocking(move || {
        let mut out = HashMap::new();
        for item in items.into_iter().take(56) {
          let p = PathBuf::from(&item.file_path);
          if let Ok(dyn_img) = image::open(&p) {
            let thumb = image::imageops::thumbnail(&dyn_img.to_rgba8(), 64, 64);
            out.insert(p, rgba_to_handle(&thumb));
          }
        }
        out
      })
      .await
      .unwrap_or_default()
    },
    Message::SystemWallpaperThumbsLoaded,
  )
}

fn on_new_screenshot_loaded(app: &mut ShotifyApp, toast_msg: impl Into<String>) -> Task<Message> {
  let tid = app.push_toast(toast_msg, true, false);
  Task::batch([
    trigger_scene_render(app),
    schedule_toast_dismiss(tid),
    rasterize_auto_thumbs_task(app.auto_presets.clone()),
  ])
}

fn active_color_for_target(app: &ShotifyApp, target: ColorPickerTarget) -> String {
  match target {
    ColorPickerTarget::GradientStart => app.options.custom_theme.color_start.clone(),
    ColorPickerTarget::GradientEnd => app.options.custom_theme.color_end.clone(),
    ColorPickerTarget::SolidColor => match &app.background_selection {
      BackgroundSelection::SolidColor { hex } => hex.clone(),
      _ => app.custom_solid_color_hex.clone(),
    },
    ColorPickerTarget::BorderColor => app
      .options
      .border_color
      .clone()
      .unwrap_or_else(|| "#ffffff".to_string()),
    ColorPickerTarget::CustomShadowColor => app.options.advanced_shadow.color.clone(),
    ColorPickerTarget::AnnotationColor => app.annotations.active_color.clone(),
  }
}

fn apply_picked_color(
  app: &mut ShotifyApp,
  target: ColorPickerTarget,
  hex: String,
) -> Task<Message> {
  match target {
    ColorPickerTarget::GradientStart => {
      app.options.custom_theme.color_start = hex;
      app.background_selection = BackgroundSelection::Gradient {
        preset_id: None,
        theme: app.options.custom_theme.clone(),
      };
      app.sync_background_to_options();
      trigger_scene_render(app)
    }
    ColorPickerTarget::GradientEnd => {
      app.options.custom_theme.color_end = hex;
      app.background_selection = BackgroundSelection::Gradient {
        preset_id: None,
        theme: app.options.custom_theme.clone(),
      };
      app.sync_background_to_options();
      trigger_scene_render(app)
    }
    ColorPickerTarget::SolidColor => {
      app.custom_solid_color_hex = hex.clone();
      app.background_selection = BackgroundSelection::SolidColor { hex };
      app.sync_background_to_options();
      trigger_scene_render(app)
    }
    ColorPickerTarget::BorderColor => {
      app.border_color_input = hex.clone();
      app.options.border_color = Some(hex);
      trigger_scene_render(app)
    }
    ColorPickerTarget::CustomShadowColor => {
      app.options.advanced_shadow.color = hex;
      trigger_scene_render(app)
    }
    ColorPickerTarget::AnnotationColor => {
      app.annotation_color_input = hex.clone();
      app.annotations.active_color = hex;
      Task::none()
    }
  }
}

fn schedule_toast_dismiss(toast_id: u64) -> Task<Message> {
  Task::perform(
    async move {
      tokio::time::sleep(std::time::Duration::from_millis(2800)).await;
      toast_id
    },
    Message::TickClearToast,
  )
}

/// Apply current vector annotations directly over `composed_base` without re-running background blur/shadows.
fn refresh_annotations_overlay(app: &mut ShotifyApp) {
  let Some(base) = app.composed_base.as_ref() else {
    return;
  };
  let mut out = base.clone();
  if !app.annotations.document.is_empty() {
    let _ = render_annotations_onto_rgba(&mut out, &app.annotations.document);
  }
  app.preview_handle = Some(rgba_to_handle(&out));
  app.composed_image = Some(out);
  app.annotation_canvas_nonce += 1;
}

/// Dispatch background `shotify-core` scene composition for the current editor options.
fn trigger_scene_render(app: &mut ShotifyApp) -> Task<Message> {
  let Some(screenshot) = app.raw_screenshot.clone() else {
    app.composed_base = None;
    app.composed_image = None;
    app.composed_tree = None;
    app.preview_handle = None;
    return Task::none();
  };

  app.render_generation += 1;
  let generation = app.render_generation;
  app.is_rendering = true;

  let mut opts = app.options.clone();
  opts.normalize(None);
  let wallpaper = app.active_wallpaper_image().cloned();
  let quality = if app.is_dragging_slider {
    RenderQuality::Interactive
  } else {
    RenderQuality::Final
  };
  let target_w = if app.is_dragging_slider { 900 } else { 1200 };

  Task::perform(
    async move {
      tokio::task::spawn_blocking(move || {
        let mut composed = compose_screenshot_scene(
          &screenshot,
          wallpaper.as_ref(),
          &opts,
          target_w,
          1.0,
          quality,
        )
        .map_err(|e| e.to_string())?;

        if opts.watermark.enabled {
          let cw = f64::from(composed.image.width());
          let ch = f64::from(composed.image.height());
          let card_w = composed.tree.layout.display_width.max(1.0);
          let card_h = composed.tree.layout.card_total_height.max(1.0);
          let card_bounds = RectF {
            left: (cw - card_w) / 2.0 + opts.position.x,
            top: (ch - card_h) / 2.0 + opts.position.y,
            width: card_w,
            height: card_h,
          };
          let _ = apply_scene_watermark(&mut composed.image, card_bounds, &opts.watermark, 1.0);
        }

        Ok(RenderedScenePayload {
          generation,
          image: composed.image,
          tree: composed.tree,
        })
      })
      .await
      .map_err(|e| format!("Render worker error: {e}"))?
    },
    Message::CompositionFinished,
  )
}

fn load_image_from_path(path: &PathBuf) -> Result<RgbaImage, String> {
  image::open(path)
    .map(|img| img.to_rgba8())
    .map_err(|e| format!("Failed to open image: {e}"))
}

async fn pick_folder_dialog_async() -> Option<PathBuf> {
  rfd::AsyncFileDialog::new()
    .set_title("Choose save folder")
    .pick_folder()
    .await
    .map(|h| h.path().to_path_buf())
}

fn execute_export_to_destination(
  app: &mut ShotifyApp,
  action: ExportActionKind,
  destination: ExportDestination,
) -> Task<Message> {
  let Some(screenshot) = app.raw_screenshot.clone() else {
    let tid = app.push_toast("Load or capture a screenshot first", false, true);
    return schedule_toast_dismiss(tid);
  };

  app.is_exporting = true;
  let wallpaper = app.active_wallpaper_image().cloned();
  let mut options = app.options.clone();
  options.normalize(None);
  let raster = RasterizeOptions {
    target_width: 1200,
    scale: app.config.export.scale,
    annotations: if app.annotations.document.is_empty() {
      None
    } else {
      Some(app.annotations.document.clone())
    },
    ..Default::default()
  };
  let encode = EncodeOptions {
    format: app.config.export.format,
    quality: app.config.export.quality as f32,
    ..Default::default()
  };

  Task::perform(
    async move {
      let action_copy = action;
      tokio::task::spawn_blocking(move || {
        execute_export(
          ExportRequest {
            screenshot: &screenshot,
            wallpaper: wallpaper.as_ref(),
            options: &options,
            raster,
            encode,
            destination,
          },
          None,
        )
        .map(ExportOutcomeSummary::from)
        .map_err(|e| e.to_string())
      })
      .await
      .map_err(|e| format!("Export task panicked: {e}"))
      .and_then(|res| res)
      .map(|summary| (action_copy, Ok(summary)))
      .unwrap_or_else(|err| (action_copy, Err(err)))
    },
    |(act, res)| Message::ExportFinished(act, res),
  )
}

fn run_export_task(app: &mut ShotifyApp, action: ExportActionKind) -> Task<Message> {
  if app.raw_screenshot.is_none() {
    let tid = app.push_toast("Load or capture a screenshot first", false, true);
    return schedule_toast_dismiss(tid);
  }

  match action {
    ExportActionKind::Clipboard => {
      execute_export_to_destination(app, action, ExportDestination::Clipboard)
    }
    ExportActionKind::SaveDownloadsOrDialog => {
      app.suppress_auto_hide_dialog = true;
      let default_name = build_default_export_file_name(app.config.export.format);
      let start_dir = dirs::download_dir()
        .filter(|p| p.exists())
        .or_else(|| dirs::home_dir().filter(|p| p.exists()));
      Task::perform(
        async move {
          let mut dialog = rfd::AsyncFileDialog::new()
            .set_title("Save screenshot")
            .set_file_name(&default_name)
            .add_filter("Image", &["png", "jpg", "jpeg", "webp"]);
          if let Some(dir) = start_dir {
            dialog = dialog.set_directory(dir);
          }
          dialog.save_file().await.map(|h| h.path().to_path_buf())
        },
        Message::SaveFilePicked,
      )
    }
    ExportActionKind::SaveToFolder => {
      if let Some(folder) = app.config.general.save_folder.clone() {
        let file_name = build_default_export_file_name(app.config.export.format);
        execute_export_to_destination(
          app,
          action,
          ExportDestination::Folder {
            folder_path: folder,
            file_name,
          },
        )
      } else {
        app.suppress_auto_hide_dialog = true;
        Task::perform(pick_folder_dialog_async(), Message::SaveToFolderPicked)
      }
    }
  }
}

fn show_and_focus_window_task(app: &mut ShotifyApp) -> Task<Message> {
  app.window_hidden = false;
  vibrancy::set_dock_icon_hidden(false);
  if let Some(id) = app.main_window_id {
    Task::batch([
      window::change_mode(id, window::Mode::Windowed),
      window::gain_focus(id),
    ])
  } else {
    Task::none()
  }
}

fn hide_window_task(app: &mut ShotifyApp) -> Task<Message> {
  app.window_hidden = true;
  if app.config.general.hide_dock_icon_when_hidden {
    vibrancy::set_dock_icon_hidden(true);
  }
  if let Some(id) = app.main_window_id {
    window::change_mode(id, window::Mode::Hidden)
  } else {
    Task::none()
  }
}

fn hide_or_exit_task(app: &mut ShotifyApp) -> Task<Message> {
  if app.config.general.keep_app_in_tray && app.platform_caps.tray_available {
    hide_window_task(app)
  } else {
    iced::exit()
  }
}

fn sync_shortcut_controller(app: &mut ShotifyApp) {
  let Some(ref mut ctrl) = app.shortcut_controller else {
    return;
  };
  match ctrl.sync(&app.config.shortcuts) {
    Ok(true) => {
      app.platform_caps.global_shortcuts_mode = GlobalShortcutBackend::NativeGlobalHotkey;
    }
    Ok(false) => {}
    Err(err) => {
      tracing::warn!("[shortcuts] {err}");
    }
  }
}

fn handle_keyboard_shortcut(app: &mut ShotifyApp, ev: keyboard::Event) -> Task<Message> {
  let keyboard::Event::KeyPressed {
    key, modifiers, ..
  } = ev
  else {
    return Task::none();
  };

  if let Some(field) = app.recording_shortcut {
    if key == Key::Named(Named::Escape) {
      app.recording_shortcut = None;
      app.shortcut_error = None;
      return Task::none();
    }
    let Some(combo) = iced_key_to_combo(&key, modifiers) else {
      return Task::none();
    };
    let mut candidate: ShortcutSettings = app.config.shortcuts.clone();
    match field {
      ShortcutFieldKey::Copy => candidate.copy = combo.clone(),
      ShortcutFieldKey::Save => candidate.save = combo.clone(),
      ShortcutFieldKey::SaveToFolder => candidate.save_to_folder = combo.clone(),
      ShortcutFieldKey::ScreenshotRegion => candidate.screenshot_region = combo.clone(),
      ShortcutFieldKey::ScreenshotRegionCopy => candidate.screenshot_region_copy = combo.clone(),
    }
    if let Err(err) = validate_shortcut_set(&candidate) {
      app.shortcut_error = Some(err.clone());
      let tid = app.push_toast(err, false, true);
      return schedule_toast_dismiss(tid);
    }
    app.config.shortcuts = candidate;
    app.recording_shortcut = None;
    app.shortcut_error = None;
    app.save_config();
    sync_shortcut_controller(app);
    let tid = app.push_toast(format!("Shortcut updated -> {combo}"), true, false);
    return schedule_toast_dismiss(tid);
  }

  let primary = if cfg!(target_os = "macos") {
    modifiers.contains(Modifiers::LOGO)
  } else {
    modifiers.contains(Modifiers::CTRL)
  };
  let shift = modifiers.contains(Modifiers::SHIFT);

  if key == Key::Named(Named::Escape) {
    if app.active_color_picker.is_some() {
      app.active_color_picker = None;
      return Task::none();
    }
    if app.ratio_menu_open {
      app.ratio_menu_open = false;
      return Task::none();
    }
    if app.export_drawer_open {
      app.export_drawer_open = false;
      return Task::none();
    }
    if app.view_mode == ViewMode::CaptureOverlay {
      app.view_mode = ViewMode::Editor;
      app.capture_selection = None;
      app.capture_copy_after_once = false;
      return Task::none();
    }
    if app.inline_text_prompt.is_some() {
      app.inline_text_prompt = None;
      return Task::none();
    }
    if app.settings_open {
      app.settings_open = false;
      return Task::none();
    }
    if app.preset_dialog_open {
      app.preset_dialog_open = false;
      return Task::none();
    }
    if app.confirm_delete_preset_id.is_some() {
      app.confirm_delete_preset_id = None;
      return Task::none();
    }
    app.annotations.active_tool = None;
    app.annotations.selection = None;
    return Task::none();
  }

  if key == Key::Named(Named::Backspace) || key == Key::Named(Named::Delete) {
    if app.annotations.active_tool.is_none() && app.annotations.delete_selected() {
      refresh_annotations_overlay(app);
    }
    return Task::none();
  }

  if matches_iced_shortcut(&app.config.shortcuts.copy, &key, modifiers) {
    return run_export_task(app, ExportActionKind::Clipboard);
  }
  if matches_iced_shortcut(&app.config.shortcuts.save_to_folder, &key, modifiers) {
    return run_export_task(app, ExportActionKind::SaveToFolder);
  }
  if matches_iced_shortcut(&app.config.shortcuts.save, &key, modifiers) {
    return run_export_task(app, ExportActionKind::SaveDownloadsOrDialog);
  }
  if matches_iced_shortcut(&app.config.shortcuts.screenshot_region_copy, &key, modifiers) {
    app.capture_copy_after_once = true;
    return ShotifyApp::update_msg(app, Message::StartRegionCaptureRequested);
  }
  if matches_iced_shortcut(&app.config.shortcuts.screenshot_region, &key, modifiers) {
    return ShotifyApp::update_msg(app, Message::StartRegionCaptureRequested);
  }

  if !primary {
    return Task::none();
  }

  if let Key::Character(ref ch) = key {
    match ch.as_str() {
      "z" | "Z" => {
        if shift {
          if app.annotations.redo() {
            refresh_annotations_overlay(app);
          }
        } else if app.annotations.undo() {
          refresh_annotations_overlay(app);
        }
      }
      "y" | "Y" => {
        if app.annotations.redo() {
          refresh_annotations_overlay(app);
        }
      }
      "v" | "V" => return ShotifyApp::update_msg(app, Message::PasteFromClipboardRequested),
      _ => {}
    }
  }

  Task::none()
}

impl ShotifyApp {
  pub fn update(&mut self, message: Message) -> Task<Message> {
    Self::update_msg(self, message)
  }

  pub fn update_msg(app: &mut ShotifyApp, message: Message) -> Task<Message> {
    match message {
      Message::InitializePlatformServices => {
        if let Some(tx) = app.platform_tx.clone() {
          let mut ctrl = GlobalShortcutController::new(tx.clone());
          match ctrl.sync(&app.config.shortcuts) {
            Ok(true) => {
              app.platform_caps.global_shortcuts_mode = GlobalShortcutBackend::NativeGlobalHotkey;
            }
            Ok(false) => {}
            Err(err) => {
              tracing::warn!("[shortcuts] startup sync warning: {err}");
            }
          }
          app.shortcut_controller = Some(ctrl);

          if is_wayland_session() {
            let region_combo = app.config.shortcuts.screenshot_region.clone();
            let copy_combo = app.config.shortcuts.screenshot_region_copy.clone();
            tokio::spawn(run_wayland_portal_shortcuts(region_combo, copy_combo, tx));
          }
        }

        if app.window_hidden && app.config.general.hide_dock_icon_when_hidden {
          vibrancy::set_dock_icon_hidden(true);
        }

        let mut startup_tasks = vec![
          rasterize_algorithm_thumbs_task(app.algorithm_presets.clone()),
          rasterize_auto_thumbs_task(app.auto_presets.clone()),
          load_system_wallpaper_thumbs_task(app.system_wallpapers.clone()),
        ];

        match app.pending_launch_cmd.take() {
          Some(LaunchCommand::CaptureRegion) => {
            startup_tasks.push(ShotifyApp::update_msg(app, Message::StartRegionCaptureRequested));
          }
          Some(LaunchCommand::CaptureRegionCopy) => {
            app.capture_copy_after_once = true;
            startup_tasks.push(ShotifyApp::update_msg(app, Message::StartRegionCaptureRequested));
          }
          _ => {}
        }

        Task::batch(startup_tasks)
      }
      Message::WindowOpened(id) => {
        app.main_window_id = Some(id);
        let dark_mode = app.theme_mode() == ThemeMode::Dark;
        Task::batch([
          window::run_with_handle(id, move |handle| {
            vibrancy::apply_vibrancy_to_handle(handle, dark_mode);
          })
          .discard(),
          window::get_maximized(id).map(Message::WindowMaximizedQueried),
        ])
      }
      Message::WindowResized(id, _size) => {
        app.main_window_id = Some(id);
        window::get_maximized(id).map(Message::WindowMaximizedQueried)
      }
      Message::WindowMaximizedQueried(maximized) => {
        app.window_maximized = maximized;
        Task::none()
      }
      Message::WindowTitleBarPressed => {
        let now = std::time::Instant::now();
        let is_double_click = app
          .last_titlebar_press
          .map(|prev| now.duration_since(prev).as_millis() < 350)
          .unwrap_or(false);
        if is_double_click {
          app.last_titlebar_press = None;
          ShotifyApp::update_msg(app, Message::WindowToggleMaximize)
        } else {
          app.last_titlebar_press = Some(now);
          if let Some(id) = app.main_window_id {
            window::drag(id)
          } else {
            Task::none()
          }
        }
      }
      Message::WindowMinimize => {
        if let Some(id) = app.main_window_id {
          window::minimize(id, true)
        } else {
          Task::none()
        }
      }
      Message::WindowToggleMaximize => {
        app.window_maximized = !app.window_maximized;
        if let Some(id) = app.main_window_id {
          Task::batch([
            window::toggle_maximize(id),
            window::get_maximized(id).map(Message::WindowMaximizedQueried),
          ])
        } else {
          Task::none()
        }
      }
      Message::WindowCloseButton => hide_or_exit_task(app),
      Message::WindowCloseRequested(id) => {
        app.main_window_id = Some(id);
        hide_or_exit_task(app)
      }
      Message::WindowFocused(id) => {
        app.main_window_id = Some(id);
        app.has_been_focused = true;
        app.window_hidden = false;
        vibrancy::set_dock_icon_hidden(false);
        Task::none()
      }
      Message::WindowUnfocused(_id) => {
        if app.config.general.hide_app_when_unfocused
          && app.has_been_focused
          && !app.suppress_auto_hide_dialog
          && app.view_mode != ViewMode::CaptureOverlay
        {
          hide_window_task(app)
        } else {
          Task::none()
        }
      }
      Message::PlatformCommandReceived(cmd) => match cmd {
        PlatformCommand::ShowWindow => show_and_focus_window_task(app),
        PlatformCommand::CaptureRegion => Task::batch([
          show_and_focus_window_task(app),
          ShotifyApp::update_msg(app, Message::StartRegionCaptureRequested),
        ]),
        PlatformCommand::CaptureRegionCopy => {
          app.capture_copy_after_once = true;
          Task::batch([
            show_and_focus_window_task(app),
            ShotifyApp::update_msg(app, Message::StartRegionCaptureRequested),
          ])
        }
        PlatformCommand::OpenSettings => {
          app.settings_open = true;
          show_and_focus_window_task(app)
        }
        PlatformCommand::QuitApp => iced::exit(),
        PlatformCommand::ShortcutBackendUpdated {
          backend,
          portal_active,
        } => {
          app.platform_caps.global_shortcuts_mode = backend;
          app.platform_caps.portal_session_active = portal_active;
          Task::none()
        }
      },
      Message::TickClearToast(id) | Message::DismissToast(id) => {
        app.toasts.retain(|t| t.id != id);
        Task::none()
      }
      Message::WindowFileDropped(path) => match load_image_from_path(&path) {
        Ok(img) => {
          app.set_screenshot(img);
          on_new_screenshot_loaded(app, "Loaded dropped image")
        }
        Err(err) => {
          let tid = app.push_toast(err, false, true);
          schedule_toast_dismiss(tid)
        }
      },
      Message::KeyboardEvent(ev) => handle_keyboard_shortcut(app, ev),

      // Media loading & capture
      Message::OpenImageDialogRequested => {
        app.suppress_auto_hide_dialog = true;
        Task::perform(
          async move {
            rfd::AsyncFileDialog::new()
              .add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp"])
              .pick_file()
              .await
              .map(|h| h.path().to_path_buf())
          },
          Message::ImageFilePicked,
        )
      }
      Message::ImageFilePicked(Some(path)) => {
        app.suppress_auto_hide_dialog = false;
        match load_image_from_path(&path) {
          Ok(img) => {
            app.set_screenshot(img);
            on_new_screenshot_loaded(app, "Image loaded")
          }
          Err(err) => {
            let tid = app.push_toast(err, false, true);
            schedule_toast_dismiss(tid)
          }
        }
      }
      Message::ImageFilePicked(None) => {
        app.suppress_auto_hide_dialog = false;
        Task::none()
      }

      Message::PasteFromClipboardRequested => {
        let pasted = arboard::Clipboard::new()
          .ok()
          .and_then(|mut cb| cb.get_image().ok())
          .and_then(|img_data| {
            RgbaImage::from_raw(
              img_data.width as u32,
              img_data.height as u32,
              img_data.bytes.into_owned(),
            )
          });
        if let Some(img) = pasted {
          app.set_screenshot(img);
          on_new_screenshot_loaded(app, "Pasted clipboard image")
        } else {
          let tid = app.push_toast("Clipboard does not contain an image", false, true);
          schedule_toast_dismiss(tid)
        }
      }

      Message::StartRegionCaptureRequested => {
        app.view_mode = ViewMode::CaptureOverlay;
        app.capture_handle = None;
        app.capture_raw_screen = None;
        app.capture_selection = None;
        Task::perform(
          async move {
            tokio::task::spawn_blocking(capture_primary_display)
              .await
              .map_err(|e| format!("Capture worker error: {e}"))?
          },
          Message::ScreenCapturedForOverlay,
        )
      }
      Message::ScreenCapturedForOverlay(Ok(img)) => {
        app.capture_handle = Some(rgba_to_handle(&img));
        app.capture_raw_screen = Some(img);
        Task::none()
      }
      Message::ScreenCapturedForOverlay(Err(err)) => {
        app.view_mode = ViewMode::Editor;
        app.capture_copy_after_once = false;
        let tid = app.push_toast(err, false, true);
        schedule_toast_dismiss(tid)
      }
      Message::CaptureDragStart(pt) => {
        app.capture_selection = Some((pt, pt));
        Task::none()
      }
      Message::CaptureDragMove(pt) => {
        if let Some((start, _)) = app.capture_selection {
          app.capture_selection = Some((start, pt));
        }
        Task::none()
      }
      Message::CaptureDragEnd(pt) => {
        let Some((start, _)) = app.capture_selection else {
          return Task::none();
        };
        let Some(screen) = app.capture_raw_screen.take() else {
          app.view_mode = ViewMode::Editor;
          app.capture_copy_after_once = false;
          return Task::none();
        };
        let (sw, sh) = screen.dimensions();
        let x0 = ((start.x.min(pt.x)) * (sw as f32)).round() as u32;
        let y0 = ((start.y.min(pt.y)) * (sh as f32)).round() as u32;
        let x1 = ((start.x.max(pt.x)) * (sw as f32)).round() as u32;
        let y1 = ((start.y.max(pt.y)) * (sh as f32)).round() as u32;
        let cw = x1.saturating_sub(x0);
        let ch = y1.saturating_sub(y0);

        app.view_mode = ViewMode::Editor;
        app.capture_handle = None;
        app.capture_selection = None;

        if cw >= 8 && ch >= 8 {
          let cropped = image::imageops::crop_imm(&screen, x0, y0, cw, ch).to_image();
          app.set_screenshot(cropped);
          let tid = app.push_toast(format!("Captured {cw}×{ch}"), true, false);
          let mut tasks = vec![
            trigger_scene_render(app),
            schedule_toast_dismiss(tid),
            rasterize_auto_thumbs_task(app.auto_presets.clone()),
          ];
          let should_copy =
            app.capture_copy_after_once || app.config.general.auto_copy_after_capture;
          app.capture_copy_after_once = false;
          if should_copy {
            tasks.push(run_export_task(app, ExportActionKind::Clipboard));
          }
          return Task::batch(tasks);
        }
        app.capture_copy_after_once = false;
        Task::none()
      }
      Message::CancelCaptureOverlay => {
        app.view_mode = ViewMode::Editor;
        app.capture_raw_screen = None;
        app.capture_handle = None;
        app.capture_selection = None;
        app.capture_copy_after_once = false;
        Task::none()
      }
      Message::ClearCanvas => {
        app.close_popovers();
        app.raw_screenshot = None;
        app.composed_base = None;
        app.composed_image = None;
        app.composed_tree = None;
        app.preview_handle = None;
        app.annotations.clear();
        app.annotations.active_tool = None;
        app.inline_text_prompt = None;
        app.annotation_canvas_nonce += 1;
        app.options.position.x = 0.0;
        app.options.position.y = 0.0;
        Task::none()
      }

      // Thumbnail cache & color picker popovers
      Message::GradientThumbsGenerated(thumbs) => {
        app.gradient_thumbs = thumbs;
        Task::none()
      }
      Message::AutoThumbsGenerated(thumbs) => {
        app.auto_thumbs = thumbs;
        Task::none()
      }
      Message::SystemWallpaperThumbsLoaded(thumbs) => {
        app.system_wallpaper_thumbs = thumbs;
        Task::none()
      }
      Message::OpenColorPicker(target) => {
        app.close_popovers();
        let hex = active_color_for_target(app, target);
        if let Some(rgba) = shotify_core::parse_css_color(&hex) {
          app.color_picker_hsv = rgb_to_hsv(rgba.r, rgba.g, rgba.b);
        }
        app.color_picker_hex_draft = hex;
        app.active_color_picker = Some(target);
        Task::none()
      }
      Message::CloseColorPicker => {
        app.active_color_picker = None;
        app.save_config();
        Task::none()
      }
      Message::ColorPickerHsvChanged { h, s, v } => {
        app.color_picker_hsv = (h, s, v);
        let (r, g, b) = hsv_to_rgb(h, s, v);
        let hex = format!("#{r:02X}{g:02X}{b:02X}");
        app.color_picker_hex_draft = hex.clone();
        if let Some(target) = app.active_color_picker {
          return apply_picked_color(app, target, hex);
        }
        Task::none()
      }
      Message::ColorPickerHexChanged(hex) => {
        app.color_picker_hex_draft = hex.clone();
        if let Some(rgba) = shotify_core::parse_css_color(&hex) {
          app.color_picker_hsv = rgb_to_hsv(rgba.r, rgba.g, rgba.b);
          if let Some(target) = app.active_color_picker {
            return apply_picked_color(app, target, hex);
          }
        }
        Task::none()
      }

      // Backgrounds & Wallpaper
      Message::SelectWallpaperTab(tab) => {
        app.wallpaper_tab = tab;
        app.save_config();
        Task::none()
      }
      Message::PickCustomWallpaperRequested => {
        app.suppress_auto_hide_dialog = true;
        Task::perform(
          async move {
            rfd::AsyncFileDialog::new()
              .add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp"])
              .pick_file()
              .await
              .map(|h| h.path().to_path_buf())
          },
          Message::CustomWallpaperPicked,
        )
      }
      Message::CustomWallpaperPicked(Some(path)) => {
        app.suppress_auto_hide_dialog = false;
        match load_image_from_path(&path) {
          Ok(img) => {
            let name = path
              .file_name()
              .and_then(|s| s.to_str())
              .unwrap_or("Wallpaper")
              .to_string();
            app.loaded_wallpapers.insert(path.clone(), img);
            app.background_selection = BackgroundSelection::Wallpaper {
              file_path: path,
              name,
            };
            app.sync_background_to_options();
            app.save_config();
            let tid = app.push_toast("Custom wallpaper loaded", true, false);
            Task::batch([trigger_scene_render(app), schedule_toast_dismiss(tid)])
          }
          Err(err) => {
            let tid = app.push_toast(err, false, true);
            schedule_toast_dismiss(tid)
          }
        }
      }
      Message::CustomWallpaperPicked(None) => {
        app.suppress_auto_hide_dialog = false;
        Task::none()
      }

      Message::SelectSystemWallpaper(path) => {
        let name = path
          .file_name()
          .and_then(|s| s.to_str())
          .unwrap_or("Wallpaper")
          .to_string();
        app.background_selection = BackgroundSelection::Wallpaper {
          file_path: path.clone(),
          name,
        };
        if app.loaded_wallpapers.contains_key(&path) {
          app.loading_wallpaper = false;
          app.sync_background_to_options();
          app.save_config();
          return trigger_scene_render(app);
        }
        app.loading_wallpaper = true;
        app.options.background_type = "wallpaper".to_string();
        let path_for_task = path.clone();
        Task::perform(
          async move {
            let p = path_for_task.clone();
            let res = tokio::task::spawn_blocking(move || load_image_from_path(&p))
              .await
              .map_err(|e| format!("Wallpaper worker error: {e}"))
              .and_then(|inner| inner);
            (path_for_task, res)
          },
          |(p, res)| Message::WallpaperLoaded(p, res),
        )
      }
      Message::WallpaperLoaded(path, Ok(img)) => {
        app.loading_wallpaper = false;
        app.loaded_wallpapers.insert(path.clone(), img);
        if matches!(
          &app.background_selection,
          BackgroundSelection::Wallpaper { file_path, .. } if file_path == &path
        ) {
          app.sync_background_to_options();
          app.save_config();
          return trigger_scene_render(app);
        }
        Task::none()
      }
      Message::WallpaperLoaded(_, Err(err)) => {
        app.loading_wallpaper = false;
        let tid = app.push_toast(err, false, true);
        schedule_toast_dismiss(tid)
      }
      Message::SelectGradientPreset(preset) => {
        match preset {
          GradientPreset::Gradient { id, config, .. } => {
            app.background_selection = BackgroundSelection::Gradient {
              preset_id: Some(id),
              theme: config,
            };
          }
          GradientPreset::BlobGradient { config, .. } => {
            app.background_selection = BackgroundSelection::BlobGradient { config };
          }
        }
        app.sync_background_to_options();
        app.save_config();
        trigger_scene_render(app)
      }
      Message::SelectBlobPreset(config) => {
        app.background_selection = BackgroundSelection::BlobGradient { config };
        app.sync_background_to_options();
        app.save_config();
        trigger_scene_render(app)
      }
      Message::SelectSolidColor(hex) => {
        app.custom_solid_color_hex = hex.clone();
        app.background_selection = BackgroundSelection::SolidColor { hex };
        app.sync_background_to_options();
        app.save_config();
        trigger_scene_render(app)
      }
      Message::GradientStartChanged(val) => {
        app.options.custom_theme.color_start = val;
        app.background_selection = BackgroundSelection::Gradient {
          preset_id: None,
          theme: app.options.custom_theme.clone(),
        };
        app.sync_background_to_options();
        app.save_config();
        trigger_scene_render(app)
      }
      Message::GradientEndChanged(val) => {
        app.options.custom_theme.color_end = val;
        app.background_selection = BackgroundSelection::Gradient {
          preset_id: None,
          theme: app.options.custom_theme.clone(),
        };
        app.sync_background_to_options();
        app.save_config();
        trigger_scene_render(app)
      }
      Message::GradientAngleChanged(deg) => {
        app.is_dragging_slider = true;
        app.options.custom_theme.angle_deg = deg;
        app.angle_input = (deg.round() as i32).to_string();
        app.background_selection = BackgroundSelection::Gradient {
          preset_id: None,
          theme: app.options.custom_theme.clone(),
        };
        app.sync_background_to_options();
        trigger_scene_render(app)
      }
      Message::GradientAngleInputChanged(raw) => {
        app.angle_input = raw.clone();
        if let Ok(val) = raw.trim().parse::<f64>() {
          let clamped = val.clamp(0.0, 360.0);
          app.options.custom_theme.angle_deg = clamped;
          app.background_selection = BackgroundSelection::Gradient {
            preset_id: None,
            theme: app.options.custom_theme.clone(),
          };
          app.sync_background_to_options();
          app.save_config();
          return trigger_scene_render(app);
        }
        Task::none()
      }

      // Card styling
      Message::ToggleRatioMenu => {
        let next = !app.ratio_menu_open;
        app.close_popovers();
        app.ratio_menu_open = next;
        Task::none()
      }
      Message::AspectModeSelected(mode, ratio_class) => {
        app.ratio_menu_open = false;
        app.options.aspect_mode = mode;
        app.ratio_choice = match mode {
          shotify_core::AspectMode::Auto => "auto".to_string(),
          shotify_core::AspectMode::Custom => "custom".to_string(),
          shotify_core::AspectMode::Preset => ratio_class.clone(),
        };
        if ratio_class != "auto" && ratio_class != "custom" {
          app.options.aspect_ratio = ratio_class;
        }
        app.save_config();
        trigger_scene_render(app)
      }
      Message::CustomRatioWChanged(s) => {
        app.custom_ratio_w_input = s.clone();
        if let Ok(w) = s.trim().parse::<u32>() {
          app.options.aspect_custom.w = f64::from(w.clamp(1, 4000));
          return trigger_scene_render(app);
        }
        Task::none()
      }
      Message::CustomRatioHChanged(s) => {
        app.custom_ratio_h_input = s.clone();
        if let Ok(h) = s.trim().parse::<u32>() {
          app.options.aspect_custom.h = f64::from(h.clamp(1, 4000));
          return trigger_scene_render(app);
        }
        Task::none()
      }
      Message::PaddingChanged(v) => {
        app.is_dragging_slider = true;
        app.options.padding_size = v;
        trigger_scene_render(app)
      }
      Message::InsetChanged(v) => {
        app.is_dragging_slider = true;
        app.options.inset.padding = v;
        trigger_scene_render(app)
      }
      Message::CurveChanged(v) => {
        app.is_dragging_slider = true;
        app.options.corner_radius = v;
        trigger_scene_render(app)
      }
      Message::BorderWidthChanged(v) => {
        app.is_dragging_slider = true;
        app.options.border_width = v;
        trigger_scene_render(app)
      }
      Message::AutoCropToggled(enabled) => {
        app.options.balance.enabled = enabled;
        trigger_scene_render(app)
      }
      Message::SelectShadowStyle(style_name) => {
        app.options.shadow = style_name;
        trigger_scene_render(app)
      }
      Message::UpdateCustomShadow(field) => {
        match field {
          ShadowFieldUpdate::OffsetX(v) => {
            app.is_dragging_slider = true;
            app.options.advanced_shadow.offset_x = v;
          }
          ShadowFieldUpdate::OffsetY(v) => {
            app.is_dragging_slider = true;
            app.options.advanced_shadow.offset_y = v;
          }
          ShadowFieldUpdate::Blur(v) => {
            app.is_dragging_slider = true;
            app.options.advanced_shadow.blur = v;
          }
          ShadowFieldUpdate::Spread(v) => {
            app.is_dragging_slider = true;
            app.options.advanced_shadow.spread = v;
          }
          ShadowFieldUpdate::Opacity(v) => {
            app.is_dragging_slider = true;
            app.options.advanced_shadow.opacity = v;
          }
        }
        trigger_scene_render(app)
      }
      Message::SelectMockup(name) => {
        app.options.browser_bar = name;
        trigger_scene_render(app)
      }
      Message::SafariUrlChanged(url) => {
        app.options.browser_url = if url.trim().is_empty() { None } else { Some(url) };
        trigger_scene_render(app)
      }
      Message::UiScaleChanged(v) => {
        app.is_dragging_slider = true;
        app.options.browser_frame.scale = v;
        trigger_scene_render(app)
      }

      // Watermark & Position
      Message::WatermarkEnabledToggled(enabled) => {
        app.options.watermark.enabled = enabled;
        trigger_scene_render(app)
      }
      Message::WatermarkPositionSelected(pos) => {
        app.options.watermark.position = pos;
        trigger_scene_render(app)
      }
      Message::WatermarkTextChanged(txt) => {
        app.options.watermark.text = txt;
        trigger_scene_render(app)
      }
      Message::WatermarkPrefixSelected(pfx) => {
        app.options.watermark.prefix = if pfx == "none" { None } else { Some(pfx) };
        trigger_scene_render(app)
      }
      Message::WatermarkVerifiedToggled(v) => {
        app.options.watermark.show_verified = v;
        trigger_scene_render(app)
      }
      Message::WatermarkOffsetXInputChanged(raw) => {
        app.watermark_offset_x_input = raw.clone();
        if let Ok(v) = raw.trim().parse::<f64>() {
          app.options.watermark.offset_x = v.clamp(-200.0, 200.0);
          return trigger_scene_render(app);
        }
        Task::none()
      }
      Message::WatermarkOffsetYInputChanged(raw) => {
        app.watermark_offset_y_input = raw.clone();
        if let Ok(v) = raw.trim().parse::<f64>() {
          app.options.watermark.offset_y = v.clamp(-200.0, 200.0);
          return trigger_scene_render(app);
        }
        Task::none()
      }
      Message::ToggleMorePosition => {
        app.more_position_open = !app.more_position_open;
        Task::none()
      }
      Message::TiltXChanged(v) => {
        app.is_dragging_slider = true;
        app.options.tilt.x = v;
        trigger_scene_render(app)
      }
      Message::TiltYChanged(v) => {
        app.is_dragging_slider = true;
        app.options.tilt.y = v;
        trigger_scene_render(app)
      }
      Message::RotationChanged(v) => {
        app.is_dragging_slider = true;
        app.options.position.rotate = v;
        trigger_scene_render(app)
      }
      Message::CardPositionXInputChanged(raw) => {
        app.card_pos_x_input = raw.clone();
        if let Ok(v) = raw.trim().parse::<f64>() {
          app.options.position.x = v.clamp(-600.0, 600.0);
          return trigger_scene_render(app);
        }
        Task::none()
      }
      Message::CardPositionYInputChanged(raw) => {
        app.card_pos_y_input = raw.clone();
        if let Ok(v) = raw.trim().parse::<f64>() {
          app.options.position.y = v.clamp(-600.0, 600.0);
          return trigger_scene_render(app);
        }
        Task::none()
      }
      Message::CardDraggedDelta(dx, dy) => {
        app.card_dragging = true;
        app.is_dragging_slider = true;
        let next = Point2D {
          x: app.options.position.x + dx,
          y: app.options.position.y + dy,
        };
        if let Some(ref tree) = app.composed_tree {
          let clamped = clamp_transform_to_container(
            next,
            DragContainerBounds {
              container_width: f64::from(tree.canvas_width),
              container_height: f64::from(tree.canvas_height),
              card_width: tree.layout.display_width,
              card_height: tree.layout.card_total_height,
              overflow_x: 0.0,
              overflow_y: 0.0,
            },
          );
          app.options.position.x = clamped.x;
          app.options.position.y = clamped.y;
        } else {
          app.options.position.x = next.x.clamp(-500.0, 500.0);
          app.options.position.y = next.y.clamp(-500.0, 500.0);
        }
        app.card_pos_x_input = (app.options.position.x.round() as i32).to_string();
        app.card_pos_y_input = (app.options.position.y.round() as i32).to_string();
        trigger_scene_render(app)
      }
      Message::SliderReleased => {
        app.card_dragging = false;
        if app.is_dragging_slider {
          app.is_dragging_slider = false;
          app.save_config();
          return trigger_scene_render(app);
        }
        Task::none()
      }
      Message::NoOp => Task::none(),

      Message::CompositionFinished(Ok(payload)) => {
        if payload.generation == app.render_generation {
          app.is_rendering = false;
        }
        if payload.generation + 1 < app.render_generation {
          return Task::none();
        }
        app.composed_tree = Some(payload.tree);
        app.composed_base = Some(payload.image);
        refresh_annotations_overlay(app);
        Task::none()
      }
      Message::CompositionFinished(Err(err)) => {
        app.is_rendering = false;
        let tid = app.push_toast(err, false, true);
        schedule_toast_dismiss(tid)
      }

      // Annotations
      Message::SelectAnnotationTool(tool) => {
        app.annotations.active_tool = tool;
        if tool.is_some() {
          app.annotations.selection = None;
        }
        Task::none()
      }
      Message::SetShapeMode(sm) => {
        app.annotations.shape_mode = sm;
        app.annotations.active_tool = Some(AnnotationTool::Shape);
        Task::none()
      }
      Message::AnnotationPointerDown {
        px,
        py,
        doc_w,
        doc_h,
      } => {
        match app.annotations.pointer_down(px, py, doc_w, doc_h) {
          PointerDownOutcome::OpenTextPrompt { x_norm, y_norm } => {
            app.inline_text_prompt = Some(InlineTextPrompt {
              x_norm,
              y_norm,
              draft: String::new(),
              edit_index: None,
            });
            return iced::widget::text_input::focus(
              crate::widgets::canvas_viewport::TEXT_PROMPT_INPUT_ID.clone(),
            );
          }
          PointerDownOutcome::PlacedStepBadge(_) => {
            refresh_annotations_overlay(app);
          }
          PointerDownOutcome::Selected(_) | PointerDownOutcome::ClearedSelection => {
            app.annotation_canvas_nonce += 1;
          }
          PointerDownOutcome::StartedDrawing | PointerDownOutcome::None => {}
        }
        Task::none()
      }
      Message::AnnotationPointerMove {
        px,
        py,
        doc_w,
        doc_h,
      } => {
        if app.annotations.pointer_move(px, py, doc_w, doc_h) {
          // Live-render shapes/arrows/moves smoothly without full background blur recomposition
          if !matches!(app.annotations.active_tool, Some(AnnotationTool::Pen)) {
            refresh_annotations_overlay(app);
          }
        }
        Task::none()
      }
      Message::AnnotationPointerUp => {
        if app.annotations.pointer_up() {
          refresh_annotations_overlay(app);
        }
        Task::none()
      }
      Message::AnnotationDoubleClick {
        px,
        py,
        doc_w,
        doc_h,
      } => {
        if let Some(SelectedAnnotation::Text(idx)) =
          shotify_annotate::hit_test_document(&app.annotations.document, doc_w, doc_h, px, py)
        {
          if let Some(t) = app.annotations.document.texts.get(idx) {
            if let Some(b) = selection_bounds_px(&app.annotations.document, doc_w, doc_h, SelectedAnnotation::Text(idx)) {
              let _ = b;
            }
            app.inline_text_prompt = Some(InlineTextPrompt {
              x_norm: t.x_norm,
              y_norm: t.y_norm,
              draft: t.value.clone(),
              edit_index: Some(idx),
            });
            return iced::widget::text_input::focus(
              crate::widgets::canvas_viewport::TEXT_PROMPT_INPUT_ID.clone(),
            );
          }
        }
        Task::none()
      }
      Message::TextPromptChanged(val) => {
        if let Some(ref mut prompt) = app.inline_text_prompt {
          prompt.draft = val;
        }
        Task::none()
      }
      Message::TextPromptCommit => {
        let Some(prompt) = app.inline_text_prompt.take() else {
          return Task::none();
        };
        let (doc_w, doc_h) = app
          .composed_image
          .as_ref()
          .map(|img| (f64::from(img.width()), f64::from(img.height())))
          .unwrap_or((1200.0, 675.0));
        if app
          .annotations
          .commit_text(&prompt.draft, prompt.x_norm, prompt.y_norm, doc_w, doc_h, prompt.edit_index)
          .is_some()
        {
          refresh_annotations_overlay(app);
        }
        if prompt.edit_index.is_none() {
          app.annotations.active_tool = None;
        }
        Task::none()
      }
      Message::TextPromptCancel => {
        app.inline_text_prompt = None;
        Task::none()
      }
      Message::AnnotationUndo => {
        if app.annotations.undo() {
          refresh_annotations_overlay(app);
        }
        Task::none()
      }
      Message::AnnotationRedo => {
        if app.annotations.redo() {
          refresh_annotations_overlay(app);
        }
        Task::none()
      }
      Message::AnnotationClearAll => {
        if app.annotations.clear() {
          refresh_annotations_overlay(app);
        }
        Task::none()
      }

      // Presets
      Message::PresetDropdownSelected(choice) => {
        if choice == "Save current as preset…" {
          app.close_popovers();
          app.preset_dialog_renaming_id = None;
          app.preset_name_draft.clear();
          app.preset_dialog_open = true;
          return iced::widget::text_input::focus(
            crate::widgets::preset_dialog::PRESET_NAME_INPUT_ID.clone(),
          );
        }
        if choice == "Current" {
          app.active_preset_id = "current".to_string();
          app.save_config();
          return Task::none();
        }
        if let Some(builtin) = builtin_presets().into_iter().find(|b| b.name == choice) {
          app.active_preset_id = builtin.id;
          app.apply_snapshot(&builtin.snapshot);
          app.save_config();
          return trigger_scene_render(app);
        }
        if let Some(preset) = app
          .user_presets
          .iter()
          .find(|p| p.name == choice)
          .cloned()
        {
          app.active_preset_id = preset.id.clone();
          app.apply_snapshot(&preset.snapshot);
          app.save_config();
          return trigger_scene_render(app);
        }
        Task::none()
      }
      Message::OpenRenamePresetDialog(id) => {
        app.close_popovers();
        if let Some(existing) = app.user_presets.iter().find(|p| p.id == id) {
          app.preset_name_draft = existing.name.clone();
          app.preset_dialog_renaming_id = Some(id);
          app.preset_dialog_open = true;
          return iced::widget::text_input::focus(
            crate::widgets::preset_dialog::PRESET_NAME_INPUT_ID.clone(),
          );
        }
        Task::none()
      }
      Message::CloseSavePresetDialog => {
        app.preset_dialog_open = false;
        app.preset_dialog_renaming_id = None;
        Task::none()
      }
      Message::PresetNameDraftChanged(s) => {
        app.preset_name_draft = s;
        Task::none()
      }
      Message::ConfirmSavePreset => {
        let name = app.preset_name_draft.trim().to_string();
        if name.is_empty() {
          return Task::none();
        }

        if let Some(rename_id) = app.preset_dialog_renaming_id.take() {
          let now = chrono::Utc::now().to_rfc3339();
          if let Some(p) = app.user_presets.iter_mut().find(|p| p.id == rename_id) {
            p.name = name.clone();
            p.updated_at = now;
          }
          let _ = rename_persisted_user_preset(&rename_id, &name);
          app.preset_dialog_open = false;
          app.save_config();
          let tid = app.push_toast(format!("Preset renamed: {name}"), true, false);
          return schedule_toast_dismiss(tid);
        }

        let id = format!("user-{}", uuid::Uuid::new_v4());
        let now = chrono::Utc::now().to_rfc3339();
        let snapshot = app.capture_preset_snapshot();
        let preset = Preset {
          id: id.clone(),
          name: name.clone(),
          built_in: false,
          created_at: now.clone(),
          updated_at: now,
          snapshot,
        };
        let _ = persist_user_preset(&preset);
        app.user_presets.push(preset);
        app.active_preset_id = id;
        app.preset_dialog_open = false;
        app.save_config();
        let tid = app.push_toast(format!("Preset saved: {name}"), true, false);
        schedule_toast_dismiss(tid)
      }
      Message::RequestDeletePreset(id) => {
        app.confirm_delete_preset_id = Some(id);
        Task::none()
      }
      Message::CancelDeletePreset => {
        app.confirm_delete_preset_id = None;
        Task::none()
      }
      Message::DeletePreset(id) => {
        app.confirm_delete_preset_id = None;
        app.user_presets.retain(|p| p.id != id);
        let _ = delete_persisted_user_preset(&id);
        if app.active_preset_id == id {
          app.active_preset_id = "current".to_string();
        }
        app.save_config();
        let tid = app.push_toast("Preset deleted", true, false);
        schedule_toast_dismiss(tid)
      }

      // Export
      Message::ToggleExportDrawer => {
        let next = !app.export_drawer_open;
        app.close_popovers();
        app.export_drawer_open = next;
        Task::none()
      }
      Message::CloseExportDrawer => {
        app.export_drawer_open = false;
        Task::none()
      }
      Message::SelectExportScale(s) => {
        app.config.export.scale = s;
        app.save_config();
        Task::none()
      }
      Message::SelectExportFormat(fmt) => {
        app.config.export.format = fmt;
        app.save_config();
        Task::none()
      }
      Message::ExportQualityChanged(q) => {
        app.config.export.quality = q;
        app.save_config();
        Task::none()
      }
      Message::RunExport(action) => run_export_task(app, action),
      Message::SaveFilePicked(Some(file_path)) => {
        app.suppress_auto_hide_dialog = false;
        execute_export_to_destination(
          app,
          ExportActionKind::SaveDownloadsOrDialog,
          ExportDestination::Path { file_path },
        )
      }
      Message::SaveFilePicked(None) => {
        app.suppress_auto_hide_dialog = false;
        Task::none()
      }
      Message::SaveToFolderPicked(Some(folder)) => {
        app.suppress_auto_hide_dialog = false;
        app.config.general.save_folder = Some(folder.clone());
        app.save_config();
        let file_name = build_default_export_file_name(app.config.export.format);
        execute_export_to_destination(
          app,
          ExportActionKind::SaveToFolder,
          ExportDestination::Folder {
            folder_path: folder,
            file_name,
          },
        )
      }
      Message::SaveToFolderPicked(None) => {
        app.suppress_auto_hide_dialog = false;
        Task::none()
      }
      Message::ExportFinished(action, Ok(summary)) => {
        app.is_exporting = false;
        let kb = (summary.bytes_len / 1024).max(1);
        let msg = match action {
          ExportActionKind::Clipboard => {
            format!("Copied {}×{} ({kb} KB) to clipboard", summary.width, summary.height)
          }
          ExportActionKind::SaveDownloadsOrDialog | ExportActionKind::SaveToFolder => {
            let where_str = summary
              .file_path
              .as_ref()
              .map(|p| p.display().to_string())
              .unwrap_or_else(|| "disk".to_string());
            format!(
              "Saved {}×{} ({}, {kb} KB) -> {}",
              summary.width,
              summary.height,
              ExportFormat::as_str_label(summary.format),
              where_str
            )
          }
        };
        let should_close = match action {
          ExportActionKind::Clipboard => app.config.general.close_after_copy,
          ExportActionKind::SaveDownloadsOrDialog => app.config.general.close_after_save,
          ExportActionKind::SaveToFolder => app.config.general.close_after_save_to_folder,
        };
        if should_close {
          return hide_or_exit_task(app);
        }
        let tid = app.push_toast(msg, true, false);
        schedule_toast_dismiss(tid)
      }
      Message::ExportFinished(_, Err(err)) => {
        app.is_exporting = false;
        let tid = app.push_toast(err, false, true);
        schedule_toast_dismiss(tid)
      }
      Message::PickSaveFolderRequested => {
        app.suppress_auto_hide_dialog = true;
        Task::perform(pick_folder_dialog_async(), Message::SaveFolderPicked)
      }
      Message::SaveFolderPicked(Some(folder)) => {
        app.suppress_auto_hide_dialog = false;
        app.config.general.save_folder = Some(folder.clone());
        app.save_config();
        let tid = app.push_toast(
          format!("Save folder set: {}", folder.display()),
          true,
          false,
        );
        schedule_toast_dismiss(tid)
      }
      Message::SaveFolderPicked(None) => {
        app.suppress_auto_hide_dialog = false;
        Task::none()
      }

      // Settings modal & Phase 5 integrations
      Message::OpenSettings => {
        app.close_popovers();
        app.settings_open = true;
        Task::none()
      }
      Message::CloseSettings => {
        app.settings_open = false;
        app.recording_shortcut = None;
        app.shortcut_error = None;
        Task::none()
      }
      Message::SelectThemeMode(mode) => {
        app.config.general.theme_mode = mode;
        app.save_config();
        Task::none()
      }
      Message::SelectLanguage(lang) => {
        app.config.general.language = lang;
        app.save_config();
        Task::none()
      }
      Message::ToggleGeneralSetting(key, val) => {
        let mut toast_after: Option<(String, bool)> = None;
        match key {
          GeneralToggleKey::HideAppWhenUnfocused => app.config.general.hide_app_when_unfocused = val,
          GeneralToggleKey::KeepAppInTray => app.config.general.keep_app_in_tray = val,
          GeneralToggleKey::HideWindowOnLaunch => app.config.general.hide_window_on_launch = val,
          GeneralToggleKey::OpenAtLogin => {
            if let Err(err) = autostart::set_open_at_login_enabled(val) {
              toast_after = Some((err, true));
            } else {
              app.config.general.open_at_login = val;
              toast_after = Some((
                if val {
                  "Enabled open at login".to_string()
                } else {
                  "Disabled open at login".to_string()
                },
                false,
              ));
            }
          }
          GeneralToggleKey::HideDockIconWhenHidden => {
            app.config.general.hide_dock_icon_when_hidden = val;
            if app.window_hidden {
              vibrancy::set_dock_icon_hidden(val);
            }
          }
          GeneralToggleKey::CloseAfterCopy => app.config.general.close_after_copy = val,
          GeneralToggleKey::CloseAfterSave => app.config.general.close_after_save = val,
          GeneralToggleKey::CloseAfterSaveToFolder => {
            app.config.general.close_after_save_to_folder = val;
          }
        }
        app.save_config();
        if let Some((msg, is_err)) = toast_after {
          let tid = app.push_toast(msg, !is_err, is_err);
          schedule_toast_dismiss(tid)
        } else {
          Task::none()
        }
      }
      Message::StartRecordingShortcut(field) => {
        app.recording_shortcut = Some(field);
        app.shortcut_error = None;
        Task::none()
      }
      Message::ResetShortcutField(field) => {
        let defs = ShortcutSettings::default();
        match field {
          ShortcutFieldKey::Copy => app.config.shortcuts.copy = defs.copy,
          ShortcutFieldKey::Save => app.config.shortcuts.save = defs.save,
          ShortcutFieldKey::SaveToFolder => app.config.shortcuts.save_to_folder = defs.save_to_folder,
          ShortcutFieldKey::ScreenshotRegion => {
            app.config.shortcuts.screenshot_region = defs.screenshot_region
          }
          ShortcutFieldKey::ScreenshotRegionCopy => {
            app.config.shortcuts.screenshot_region_copy = defs.screenshot_region_copy
          }
        }
        if app.recording_shortcut == Some(field) {
          app.recording_shortcut = None;
        }
        app.shortcut_error = None;
        app.save_config();
        sync_shortcut_controller(app);
        Task::none()
      }
      Message::CancelRecordingShortcut => {
        app.recording_shortcut = None;
        app.shortcut_error = None;
        Task::none()
      }
      Message::ResetAllShortcuts => {
        app.config.shortcuts = ShortcutSettings::default();
        app.recording_shortcut = None;
        app.shortcut_error = None;
        app.save_config();
        sync_shortcut_controller(app);
        let tid = app.push_toast("Shortcuts reset to defaults", true, false);
        schedule_toast_dismiss(tid)
      }
      Message::ImportLegacyTauriRequested => {
        let p_dir = presets_dir_path();
        let dbs = discover_legacy_localstorage_files();
        let rep = import_legacy_sources(&mut app.config, p_dir.as_deref(), None, &dbs);
        app.config.migrated_from_tauri = true;
        if let Some(ref dir) = p_dir {
          app.user_presets = list_user_presets_in_dir(dir);
        }
        app.save_config();
        sync_shortcut_controller(app);
        let msg = if rep.imported_presets > 0 || rep.imported_settings || rep.imported_shortcuts {
          format!(
            "Imported {} preset(s), settings={}, shortcuts={}",
            rep.imported_presets, rep.imported_settings, rep.imported_shortcuts
          )
        } else {
          "No legacy Tauri localStorage data found on this device".to_string()
        };
        let tid = app.push_toast(msg, true, false);
        schedule_toast_dismiss(tid)
      }
      Message::CheckForUpdatesRequested => {
        app.update_status = UpdateCheckStatus::Checking;
        let current = env!("CARGO_PKG_VERSION").to_string();
        Task::perform(
          check_for_updates_async(current),
          Message::UpdateCheckFinished,
        )
      }
      Message::UpdateCheckFinished(status) => {
        app.update_status = status;
        Task::none()
      }
      Message::OpenExternalUrl(url) => {
        if let Err(err) = open_url(&url) {
          let tid = app.push_toast(err, false, true);
          return schedule_toast_dismiss(tid);
        }
        Task::none()
      }
    }
  }
}

trait ExportFormatLabel {
  fn as_str_label(fmt: Self) -> &'static str;
}

impl ExportFormatLabel for ExportFormat {
  fn as_str_label(fmt: Self) -> &'static str {
    match fmt {
      ExportFormat::Png => "PNG",
      ExportFormat::Jpeg => "JPEG",
      ExportFormat::Webp => "WebP",
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::config::default_preset_definition;
  use crate::message::{ExportActionKind, Message};
  use crate::state::{aspect_preset_for_value, rasterize_gradient_preset_rgba, ShotifyApp};
  use crate::widgets::wallpaper_grid::SOLID_COLOR_SWATCHES;
  use shotify_core::{generate_algorithm_gradients, generate_auto_gradients, RATIO_OPTIONS};

  #[test]
  fn ratio_options_include_all_16_presets() {
    assert_eq!(RATIO_OPTIONS.len(), 16);
    for opt in RATIO_OPTIONS {
      assert_eq!(aspect_preset_for_value(opt.value).value, opt.value);
    }
  }

  #[test]
  fn all_gradients_and_auto_presets_rasterize_clean_thumbnails() {
    let alg = generate_algorithm_gradients();
    assert_eq!(alg.len(), 56);
    for preset in &alg {
      let rgba = rasterize_gradient_preset_rgba(preset).expect("algorithm thumb rasterizes");
      assert_eq!(rgba.dimensions(), (48, 48));
    }

    let auto = generate_auto_gradients(&["#7700FF".into(), "#EC4899".into()], 12345);
    assert_eq!(auto.len(), 14);
    for preset in &auto {
      let rgba = rasterize_gradient_preset_rgba(preset).expect("auto thumb rasterizes");
      assert_eq!(rgba.dimensions(), (48, 48));
    }
  }

  #[test]
  fn default_preset_and_color_swatches_match_normal_ui() {
    assert_eq!(SOLID_COLOR_SWATCHES.len(), 13);
    assert_eq!(SOLID_COLOR_SWATCHES[12], ("Indigo", "#4F46E5"));

    let def = default_preset_definition();
    assert_eq!(def.name, "Default");
    assert_eq!(def.snapshot.padding_size, 32.0);
    assert_eq!(def.snapshot.corner_radius, 30);
    assert_eq!(def.snapshot.shadow, "Hug");
    assert_eq!(def.snapshot.export_scale, 2.0);
  }

  #[test]
  fn save_dialog_cancellations_and_empty_canvas_are_handled_cleanly() {
    let mut app = ShotifyApp::new();
    assert!(app.raw_screenshot.is_none());

    let _ = ShotifyApp::update_msg(&mut app, Message::RunExport(ExportActionKind::SaveDownloadsOrDialog));
    assert!(!app.is_exporting);
    assert!(!app.suppress_auto_hide_dialog);
    assert!(app
      .toasts
      .last()
      .map(|t| t.message.contains("Load or capture a screenshot first"))
      .unwrap_or(false));

    app.suppress_auto_hide_dialog = true;
    let _ = ShotifyApp::update_msg(&mut app, Message::SaveFilePicked(None));
    assert!(!app.suppress_auto_hide_dialog);
    assert!(!app.is_exporting);

    app.suppress_auto_hide_dialog = true;
    let _ = ShotifyApp::update_msg(&mut app, Message::SaveToFolderPicked(None));
    assert!(!app.suppress_auto_hide_dialog);
    assert!(!app.is_exporting);
  }

  #[test]
  fn ui_polish_state_defaults_and_delete_confirmation_flow() {
    let mut app = ShotifyApp::new();
    assert_eq!(app.annotation_color_input, "#7700FF");
    assert_eq!(app.annotations.active_color, "#7700FF");
    assert!(!app.more_position_open);

    let _ = ShotifyApp::update_msg(&mut app, Message::RequestDeletePreset("user-1".into()));
    assert_eq!(app.confirm_delete_preset_id.as_deref(), Some("user-1"));

    let _ = ShotifyApp::update_msg(&mut app, Message::CancelDeletePreset);
    assert!(app.confirm_delete_preset_id.is_none());

    let _ = ShotifyApp::update_msg(&mut app, Message::NoOp);
  }

  #[test]
  fn window_controls_update_state_safely() {
    let mut app = ShotifyApp::new();
    assert!(!app.window_maximized);

    let _ = ShotifyApp::update_msg(&mut app, Message::WindowMaximizedQueried(true));
    assert!(app.window_maximized);

    let _ = ShotifyApp::update_msg(&mut app, Message::WindowToggleMaximize);
    assert!(!app.window_maximized);

    let _ = ShotifyApp::update_msg(&mut app, Message::WindowMinimize);
    let _ = ShotifyApp::update_msg(&mut app, Message::WindowTitleBarPressed);
    assert!(app.last_titlebar_press.is_some());
    // Immediate second press acts as double-click and toggles maximize.
    let _ = ShotifyApp::update_msg(&mut app, Message::WindowTitleBarPressed);
    assert!(app.last_titlebar_press.is_none());
    assert!(app.window_maximized);
  }
}
