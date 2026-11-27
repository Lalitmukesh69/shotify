//! Pure Rust export pipeline for **shotify**: scene-graph rasterization,
//! transparent edge seam trimming, pill & overlay watermark compositing,
//! PNG/JPEG/WebP encoding, file I/O, and system clipboard delivery.

pub mod clipboard;
pub mod encoder;
pub mod error;
pub mod metrics;
pub mod output;
pub mod pipeline;
pub mod rasterizer;
pub mod trim;
pub mod watermark;

pub use clipboard::{ClipboardContent, ClipboardController};
pub use encoder::{
  encode_rgba_image, resize_for_export, rgba_to_rgb_white_matte, EncodeOptions, ExportFormat,
};
pub use error::{ExportError, Result};
pub use metrics::{
  build_default_export_file_name, compute_export_dimensions,
  get_card_shadow_presentation_kind, get_export_background_frame_radius_px,
  is_export_presentation_ready, ShadowPresentationKind,
};
pub use output::{
  bytes_to_data_url, decode_data_url, ensure_unique_path, ext_from_mime,
  resolve_downloads_export_dir, sanitize_file_stem, save_bytes_to_downloads,
  save_bytes_to_folder, save_bytes_to_path,
};
pub use pipeline::{execute_export, ExportDestination, ExportOutcome, ExportRequest};
pub use rasterizer::{rasterize_export_scene, RasterizeOptions, RasterizedScene};
pub use trim::{
  trim_macos_window_screenshot, trim_transparent_edge_seams, MacOsWindowTrimOptions,
  TrimSeamsOptions,
};
pub use watermark::{
  apply_scene_watermark, composite_watermark_rgba, render_watermark_pill, WatermarkPosition,
};
