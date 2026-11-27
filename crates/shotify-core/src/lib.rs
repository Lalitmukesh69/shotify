//! Pure Rust core image processing, composition scene graph, color science,
//! background FX, shadows, and layout engine for **shotify**.
//!
//! This crate intentionally has zero UI/Tauri dependencies so it can drive both
//! the hybrid Tauri bridge (via IPC validation) and the future native Iced GUI.

pub mod background;
pub mod background_fx;
pub mod color;
pub mod compositor;
pub mod defaults;
pub mod error;
pub mod gradients;
pub mod helpers;
pub mod image_balance;
pub mod inset;
pub mod layout;
pub mod math;
pub mod palette_extract;
pub mod perspective;
pub mod shadows;
pub mod wallpapers;

pub use background::{
  is_same_blob_gradient_selection, is_same_gradient_selection, is_same_wallpaper_selection,
  toggle_blob_gradient_off, toggle_gradient_off, toggle_wallpaper_off, BackgroundKind,
};
pub use background_fx::{
  apply_brightness_to_pixmap, apply_gaussian_blur_image, apply_noise_to_pixmap,
  apply_vignette_to_pixmap, boxes_for_gauss, gaussian_approx_blur_in_place, pixmap_to_rgba_image,
  render_background_pixmap, render_blob_gradient_pixmap, render_linear_gradient_pixmap,
  rgba_image_to_pixmap, BackgroundSpec, RenderQuality,
};
pub use color::{
  boost_rgba_alpha_in_css_value, hex_to_rgb, hsl_to_rgb, is_probably_hex_color,
  normalize_hex_color_input, parse_css_color, rgb_to_hex, rgba_string, to_canvas_rgba, Mulberry32,
  Rgba,
};
pub use compositor::{
  compose_screenshot_scene, resolve_canvas_dimensions, CompositionNode, CompositionOutput,
  CompositionTree,
};
pub use defaults::{
  get_rounded_px, AspectAutoPair, AspectMode, AspectRatioPair, BalanceConfig, BlobGradientConfig,
  BrowserFrameConfig, CustomTheme, EditorOptions, FxConfig, InsetConfig, PenConfig, PositionConfig,
  ShadowParams, TiltConfig, WatermarkConfig,
};
pub use error::{CoreError, Result};
pub use gradients::{
  blob_gradient_preview_css, generate_algorithm_gradients, generate_auto_gradients, GradientPreset,
};
pub use helpers::{
  aspect_class_to_dimensions, canvas_angle_deg_to_css_angle_deg, AspectRatioPreset,
  BrowserBarVariant, CanvasBlendMode, RATIO_OPTIONS,
};
pub use image_balance::{
  balance_rgba_image, detect_balanced_crop_rect, estimate_background_from_edges,
  BalanceDetectionResult, BalanceDimensions, BalanceOptions, CropRect, EdgeBounds, EdgeMargins,
  EstimatedEdgeBackground, RgbColor,
};
pub use inset::{
  create_inset_image, edge_palette_to_gradients, get_edge_color, get_edge_palette,
  EdgeColorOptions, EdgePalette, EdgePaletteGradients,
};
pub use layout::{
  compute_browser_bar_metrics, compute_card_layout, compute_effective_padding_px,
  compute_inset_content_radius_px, compute_object_contain_rect, compute_object_cover_rect,
  BrowserBarMetrics, CardLayout, RectF,
};
pub use math::{
  clamp_f32, clamp_f64, clamp_transform_to_container, lerp_f32, lerp_f64, normalize_zero,
  parse_angle_to_unit_vector, snap_to_device_pixel, snap_to_half_pixel, DragContainerBounds,
  Point2D,
};
pub use palette_extract::{
  extract_palette_from_bytes, extract_palette_from_rgba_image, ExtractedPalette,
};
pub use perspective::{
  composite_pixmap_transformed, project_card_corners_2d, unit_square_to_quad_homography,
  CardTransform3D,
};
pub use shadows::{
  build_rounded_rect_path, render_ambient_glow_pixmap, render_drop_shadow_pixmap, RenderedShadow,
  ShadowStyle,
};
pub use wallpapers::{
  collect_images_recursive, default_os_wallpaper_roots, is_wallpaper_image,
  list_system_wallpapers_from_paths, SystemWallpaperItem,
};
