use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use iced::widget::image::Handle as ImageHandle;
use iced::{window, Point};
use image::RgbaImage;
use shotify_annotate::AnnotationEngine;
use shotify_core::{
  default_os_wallpaper_roots, extract_palette_from_rgba_image, generate_algorithm_gradients,
  generate_auto_gradients, list_system_wallpapers_from_paths, CompositionTree, EditorOptions,
  GradientPreset, SystemWallpaperItem,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::config::{
  load_or_initialize_config, persist_config, BackgroundSelection, Config, Preset, PresetSnapshot,
  WallpaperTab,
};
use crate::message::{ColorPickerTarget, ShortcutFieldKey};
use crate::platform::shortcuts::GlobalShortcutController;
use crate::platform::single_instance::LaunchCommand;
use crate::platform::updater::UpdateCheckStatus;
use crate::platform::{detect_platform_capabilities, PlatformCapabilities, PlatformCommand};
use crate::theme::ThemeMode;

/// Top-level active view mode in `ShotifyApp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
  Editor,
  CaptureOverlay,
}

/// Inline text editor state when clicking with the Text annotation tool or double-clicking a text box.
#[derive(Debug, Clone)]
pub struct InlineTextPrompt {
  pub x_norm: f64,
  pub y_norm: f64,
  pub draft: String,
  pub edit_index: Option<usize>,
}

/// Toast notification shown at bottom-center of the workspace.
#[derive(Debug, Clone)]
pub struct ToastNotification {
  pub id: u64,
  pub message: String,
  pub is_error: bool,
  pub is_success: bool,
}

/// Look up a `RATIO_OPTIONS` entry by its Tailwind-style aspect value (`"auto"`, `"aspect-video"`, etc.).
pub fn aspect_preset_for_value(value: &str) -> shotify_core::AspectRatioPreset {
  shotify_core::RATIO_OPTIONS
    .iter()
    .copied()
    .find(|opt| opt.value == value)
    .unwrap_or(shotify_core::RATIO_OPTIONS[0])
}

/// Render a 48×48 straight-alpha RGBA thumbnail preview for any linear or blob gradient preset.
pub fn rasterize_gradient_preset_rgba(preset: &GradientPreset) -> Option<RgbaImage> {
  match preset {
    GradientPreset::Gradient { config, .. } => shotify_core::render_linear_gradient_pixmap(
      48,
      48,
      config.angle_deg,
      &config.color_start,
      &config.color_end,
    )
    .ok()
    .map(|pm| shotify_core::pixmap_to_rgba_image(&pm)),
    GradientPreset::BlobGradient { config, .. } => {
      shotify_core::render_blob_gradient_pixmap(48, 48, 1.0, config)
        .ok()
        .map(|pm| shotify_core::pixmap_to_rgba_image(&pm))
    }
  }
}

/// Main application state for the native Iced desktop app (`shotify-rust`).
pub struct ShotifyApp {
  pub view_mode: ViewMode,
  pub config: Config,
  pub user_presets: Vec<Preset>,
  pub options: EditorOptions,
  pub wallpaper_tab: WallpaperTab,
  pub background_selection: BackgroundSelection,
  pub ratio_choice: String,

  // Loaded media & wallpapers
  pub raw_screenshot: Option<RgbaImage>,
  pub loaded_wallpapers: HashMap<PathBuf, RgbaImage>,
  pub system_wallpapers: Vec<SystemWallpaperItem>,
  pub system_wallpaper_thumbs: HashMap<PathBuf, ImageHandle>,
  pub algorithm_presets: Vec<GradientPreset>,
  pub gradient_thumbs: HashMap<String, ImageHandle>,
  pub auto_presets: Vec<GradientPreset>,
  pub auto_thumbs: HashMap<u32, ImageHandle>,
  pub custom_solid_color_hex: String,
  pub custom_ratio_w_input: String,
  pub custom_ratio_h_input: String,
  pub border_color_input: String,
  pub angle_input: String,
  pub watermark_offset_x_input: String,
  pub watermark_offset_y_input: String,
  pub card_pos_x_input: String,
  pub card_pos_y_input: String,

  // Popover & color picker state
  pub export_drawer_open: bool,
  pub ratio_menu_open: bool,
  pub active_color_picker: Option<ColorPickerTarget>,
  pub color_picker_hsv: (f32, f32, f32),
  pub color_picker_hex_draft: String,

  // Rendered scene graph + texture handle
  pub render_generation: u64,
  pub is_rendering: bool,
  pub is_dragging_slider: bool,
  pub composed_base: Option<RgbaImage>,
  pub composed_image: Option<RgbaImage>,
  pub composed_tree: Option<CompositionTree>,
  pub preview_handle: Option<ImageHandle>,

  // Annotation interactive layer
  pub annotations: AnnotationEngine,
  pub inline_text_prompt: Option<InlineTextPrompt>,
  pub annotation_canvas_nonce: u64,
  pub annotation_color_input: String,

  // Drag alignment overlay
  pub card_dragging: bool,

  // Screen region capture overlay state
  pub capture_raw_screen: Option<RgbaImage>,
  pub capture_handle: Option<ImageHandle>,
  pub capture_selection: Option<(Point, Point)>,

  // UI dialogs & sidebar state
  pub active_preset_id: String,
  pub confirm_delete_preset_id: Option<String>,
  pub loading_wallpaper: bool,
  pub more_position_open: bool,
  pub preset_dialog_open: bool,
  pub preset_dialog_renaming_id: Option<String>,
  pub preset_name_draft: String,
  pub settings_open: bool,
  pub is_exporting: bool,

  // Phase 5 system integration & updater state
  pub main_window_id: Option<window::Id>,
  pub has_been_focused: bool,
  pub window_hidden: bool,
  pub window_maximized: bool,
  pub last_titlebar_press: Option<Instant>,
  pub suppress_auto_hide_dialog: bool,
  pub capture_copy_after_once: bool,
  pub recording_shortcut: Option<ShortcutFieldKey>,
  pub shortcut_error: Option<String>,
  pub platform_caps: PlatformCapabilities,
  pub platform_tx: Option<UnboundedSender<PlatformCommand>>,
  pub shortcut_controller: Option<GlobalShortcutController>,
  pub pending_launch_cmd: Option<LaunchCommand>,
  pub update_status: UpdateCheckStatus,

  // Toasts
  pub toasts: Vec<ToastNotification>,
  next_toast_id: u64,
}

impl Default for ShotifyApp {
  fn default() -> Self {
    Self::new()
  }
}

impl ShotifyApp {
  pub fn new() -> Self {
    Self::build_with_platform(false, None, None, false)
  }

  pub fn boot(
    tray_available: bool,
    platform_tx: UnboundedSender<PlatformCommand>,
    launch_cmd: LaunchCommand,
    start_hidden: bool,
  ) -> Self {
    Self::build_with_platform(
      tray_available,
      Some(platform_tx),
      Some(launch_cmd),
      start_hidden,
    )
  }

  fn build_with_platform(
    tray_available: bool,
    platform_tx: Option<UnboundedSender<PlatformCommand>>,
    pending_launch_cmd: Option<LaunchCommand>,
    window_hidden: bool,
  ) -> Self {
    let (config, user_presets, migration_report) = load_or_initialize_config();
    let algorithm_presets = generate_algorithm_gradients();
    let auto_presets = generate_auto_gradients(&[], 0);
    let system_wallpapers = list_system_wallpapers_from_paths(&default_os_wallpaper_roots(), &[], 56);

    let mut options = EditorOptions {
      padding_size: 32.0,
      corner_radius: 30,
      shadow: "Hug".to_string(),
      ..EditorOptions::default()
    };
    options.balance.enabled = true;
    let ratio_choice = config.last_state.ratio_choice.clone();
    match ratio_choice.as_str() {
      "auto" => options.aspect_mode = shotify_core::AspectMode::Auto,
      "custom" => options.aspect_mode = shotify_core::AspectMode::Custom,
      cls if !cls.trim().is_empty() => {
        options.aspect_mode = shotify_core::AspectMode::Preset;
        options.aspect_ratio = cls.to_string();
      }
      _ => {}
    }
    options.aspect_custom.w = f64::from(config.last_state.custom_ratio_w.clamp(1, 4000));
    options.aspect_custom.h = f64::from(config.last_state.custom_ratio_h.clamp(1, 4000));
    if !config.last_state.safari_url.trim().is_empty() {
      options.browser_url = Some(config.last_state.safari_url.clone());
    }

    let bg_sel = config.last_state.background_selection.clone();
    let mut app = Self {
      view_mode: ViewMode::Editor,
      wallpaper_tab: config.last_state.wallpaper_tab,
      background_selection: bg_sel.clone(),
      ratio_choice,
      options: options.clone(),
      custom_solid_color_hex: "#7700FF".to_string(),
      custom_ratio_w_input: config.last_state.custom_ratio_w.to_string(),
      custom_ratio_h_input: config.last_state.custom_ratio_h.to_string(),
      border_color_input: options.border_color.clone().unwrap_or_else(|| "#ffffff".to_string()),
      angle_input: (options.custom_theme.angle_deg.round() as i32).to_string(),
      watermark_offset_x_input: (options.watermark.offset_x.round() as i32).to_string(),
      watermark_offset_y_input: (options.watermark.offset_y.round() as i32).to_string(),
      card_pos_x_input: (options.position.x.round() as i32).to_string(),
      card_pos_y_input: (options.position.y.round() as i32).to_string(),
      export_drawer_open: false,
      ratio_menu_open: false,
      active_color_picker: None,
      color_picker_hsv: (268.0, 1.0, 1.0),
      color_picker_hex_draft: "#7700FF".to_string(),
      raw_screenshot: None,
      loaded_wallpapers: HashMap::new(),
      system_wallpapers,
      system_wallpaper_thumbs: HashMap::new(),
      algorithm_presets,
      gradient_thumbs: HashMap::new(),
      auto_presets,
      auto_thumbs: HashMap::new(),
      render_generation: 0,
      is_rendering: false,
      is_dragging_slider: false,
      composed_base: None,
      composed_image: None,
      composed_tree: None,
      preview_handle: None,
      annotations: {
        let mut eng = AnnotationEngine::new();
        eng.active_color = "#7700FF".to_string();
        eng
      },
      inline_text_prompt: None,
      annotation_canvas_nonce: 0,
      annotation_color_input: "#7700FF".to_string(),
      card_dragging: false,
      capture_raw_screen: None,
      capture_handle: None,
      capture_selection: None,
      active_preset_id: config.last_state.active_preset_id.clone(),
      confirm_delete_preset_id: None,
      loading_wallpaper: false,
      more_position_open: false,
      preset_dialog_open: false,
      preset_dialog_renaming_id: None,
      preset_name_draft: String::new(),
      settings_open: false,
      is_exporting: false,
      main_window_id: None,
      has_been_focused: false,
      window_hidden,
      window_maximized: false,
      last_titlebar_press: None,
      suppress_auto_hide_dialog: false,
      capture_copy_after_once: false,
      recording_shortcut: None,
      shortcut_error: None,
      platform_caps: detect_platform_capabilities(tray_available),
      platform_tx,
      shortcut_controller: None,
      pending_launch_cmd,
      update_status: UpdateCheckStatus::Idle,
      toasts: Vec::new(),
      next_toast_id: 1,
      config,
      user_presets,
    };

    app.sync_background_to_options();
    options = app.options.clone();
    options.normalize(None);
    app.angle_input = (options.custom_theme.angle_deg.round() as i32).to_string();
    app.options = options;

    if let Some(rep) = migration_report {
      app.push_toast(
        format!(
          "Migrated Tauri settings & {} preset(s) into shotify-rust",
          rep.imported_presets
        ),
        true,
        false,
      );
    }

    app
  }

  pub fn theme_mode(&self) -> ThemeMode {
    self.config.general.theme_mode
  }

  pub fn close_popovers(&mut self) {
    self.export_drawer_open = false;
    self.ratio_menu_open = false;
    self.active_color_picker = None;
    self.confirm_delete_preset_id = None;
  }

  pub fn save_config(&mut self) {
    self.config.last_state.background_selection = self.background_selection.clone();
    self.config.last_state.wallpaper_tab = self.wallpaper_tab;
    self.config.last_state.active_preset_id = self.active_preset_id.clone();
    self.config.last_state.ratio_choice = self.ratio_choice.clone();
    self.config.last_state.custom_ratio_w =
      self.options.aspect_custom.w.round().clamp(1.0, 4000.0) as u32;
    self.config.last_state.custom_ratio_h =
      self.options.aspect_custom.h.round().clamp(1.0, 4000.0) as u32;
    self.config.last_state.safari_url = self.options.browser_url.clone().unwrap_or_default();
    persist_config(&self.config);
  }

  pub fn push_toast(&mut self, msg: impl Into<String>, is_success: bool, is_error: bool) -> u64 {
    let id = self.next_toast_id;
    self.next_toast_id += 1;
    self.toasts.push(ToastNotification {
      id,
      message: msg.into(),
      is_error,
      is_success,
    });
    if self.toasts.len() > 4 {
      self.toasts.remove(0);
    }
    id
  }

  pub fn set_screenshot(&mut self, img: RgbaImage) {
    if let Some(pal) = extract_palette_from_rgba_image(&img, 7) {
      let seed = (img.width().wrapping_mul(31)) ^ (img.height().wrapping_mul(17));
      self.auto_presets = generate_auto_gradients(&pal.colors, seed);
      self.auto_thumbs.clear();
    }
    self.raw_screenshot = Some(img);
    self.options.position.x = 0.0;
    self.options.position.y = 0.0;
    self.card_pos_x_input = "0".to_string();
    self.card_pos_y_input = "0".to_string();
    self.annotations.clear();
    self.annotation_canvas_nonce += 1;
  }

  pub fn active_wallpaper_image(&self) -> Option<&RgbaImage> {
    match &self.background_selection {
      BackgroundSelection::Wallpaper { file_path, .. } => self.loaded_wallpapers.get(file_path),
      _ => None,
    }
  }

  pub fn load_wallpaper_if_needed(&mut self, path: &PathBuf) {
    if self.loaded_wallpapers.contains_key(path) {
      return;
    }
    if let Ok(dyn_img) = image::open(path) {
      self.loaded_wallpapers.insert(path.clone(), dyn_img.to_rgba8());
    }
  }

  pub fn sync_background_to_options(&mut self) {
    match &self.background_selection {
      BackgroundSelection::Gradient { theme, .. } => {
        self.options.background_type = "gradient".to_string();
        self.options.custom_theme = theme.clone();
      }
      BackgroundSelection::BlobGradient { config } => {
        self.options.background_type = "blobGradient".to_string();
        self.options.blob_gradient = config.clone();
      }
      BackgroundSelection::Wallpaper { file_path, .. } => {
        let p = file_path.clone();
        self.load_wallpaper_if_needed(&p);
        self.options.background_type = "wallpaper".to_string();
      }
      BackgroundSelection::SolidColor { hex } => {
        self.options.background_type = "gradient".to_string();
        self.options.custom_theme.color_start = hex.clone();
        self.options.custom_theme.color_end = hex.clone();
        self.options.custom_theme.angle_deg = 0.0;
      }
    }
  }

  pub fn capture_preset_snapshot(&self) -> PresetSnapshot {
    PresetSnapshot::from_editor_state(
      &self.options,
      &self.ratio_choice,
      self.wallpaper_tab,
      &self.background_selection,
      &self.annotations.active_color,
      self.config.export.scale,
      self.config.export.format,
      self.config.export.quality,
    )
  }

  pub fn apply_snapshot(&mut self, snap: &PresetSnapshot) {
    self.wallpaper_tab = snap.wallpaper_tab;
    self.background_selection = snap.background_selection.clone();
    self.ratio_choice = snap.ratio_choice.clone();
    self.options.aspect_mode = snap.aspect_mode;
    if snap.aspect_mode == shotify_core::AspectMode::Preset && !snap.ratio_choice.trim().is_empty()
    {
      self.options.aspect_ratio = snap.ratio_choice.clone();
    }
    self.options.aspect_custom.w = f64::from(snap.custom_w.clamp(1, 4000));
    self.options.aspect_custom.h = f64::from(snap.custom_h.clamp(1, 4000));
    self.custom_ratio_w_input = snap.custom_w.to_string();
    self.custom_ratio_h_input = snap.custom_h.to_string();
    self.options.padding_size = snap.padding_size;
    self.options.inset.padding = snap.inset_padding;
    self.options.corner_radius = snap.corner_radius;
    self.options.border_width = snap.border_width;
    self.options.border_color = Some(snap.border_color.clone());
    self.border_color_input = snap.border_color.clone();
    self.options.shadow = snap.shadow.clone();
    self.options.advanced_shadow = snap.advanced_shadow.clone();
    self.options.browser_bar = snap.browser_bar.clone();
    self.options.browser_url = if snap.browser_url.trim().is_empty() {
      None
    } else {
      Some(snap.browser_url.clone())
    };
    self.options.browser_frame.scale = snap.ui_scale;
    self.options.balance.enabled = snap.balance_enabled;
    self.options.watermark = snap.watermark.clone();
    self.options.tilt = shotify_core::TiltConfig {
      x: snap.tilt_x,
      y: snap.tilt_y,
    };
    self.options.position.x = snap.offset_x;
    self.options.position.y = snap.offset_y;
    self.options.position.rotate = snap.rotation;
    self.watermark_offset_x_input = (snap.watermark.offset_x.round() as i32).to_string();
    self.watermark_offset_y_input = (snap.watermark.offset_y.round() as i32).to_string();
    self.card_pos_x_input = (snap.offset_x.round() as i32).to_string();
    self.card_pos_y_input = (snap.offset_y.round() as i32).to_string();
    self.options.fx = snap.fx;
    self.annotations.active_color = snap.annotation_color.clone();
    self.annotation_color_input = snap.annotation_color.clone();
    self.config.export.scale = snap.export_scale;
    self.config.export.format = snap.export_format;
    self.config.export.quality = snap.export_quality;
    self.sync_background_to_options();
    self.options.normalize(None);
    self.angle_input = (self.options.custom_theme.angle_deg.round() as i32).to_string();
  }

  pub fn current_aspect_option(&self) -> shotify_core::AspectRatioPreset {
    match self.options.aspect_mode {
      shotify_core::AspectMode::Auto => aspect_preset_for_value("auto"),
      shotify_core::AspectMode::Custom => aspect_preset_for_value("custom"),
      shotify_core::AspectMode::Preset => aspect_preset_for_value(&self.options.aspect_ratio),
    }
  }
}
