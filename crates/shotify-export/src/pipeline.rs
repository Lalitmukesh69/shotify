use std::path::PathBuf;

use image::RgbaImage;
use serde::{Deserialize, Serialize};
use shotify_core::{CompositionTree, EditorOptions};

use crate::clipboard::ClipboardController;
use crate::encoder::{encode_rgba_image, resize_for_export, EncodeOptions, ExportFormat};
use crate::error::Result;
use crate::metrics::compute_export_dimensions;
use crate::output::{
  bytes_to_data_url, save_bytes_to_downloads, save_bytes_to_folder, save_bytes_to_path,
};
use crate::rasterizer::{rasterize_export_scene, RasterizeOptions};

/// Where the encoded export artifact should be delivered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ExportDestination {
  /// Return encoded bytes + optional data URL without writing to disk/clipboard.
  Memory,
  /// Save under `<Downloads>/shotify/<file_name>`.
  Downloads { file_name: String },
  /// Save to an explicit path.
  Path { file_path: PathBuf },
  /// Save into a specific user folder under `<file_name>`.
  Folder { folder_path: PathBuf, file_name: String },
  /// Write decoded pixels to the system clipboard.
  Clipboard,
}

/// Full specification for executing an end-to-end Rust export pipeline pass.
pub struct ExportRequest<'a> {
  pub screenshot: &'a RgbaImage,
  pub wallpaper: Option<&'a RgbaImage>,
  pub options: &'a EditorOptions,
  pub raster: RasterizeOptions,
  pub encode: EncodeOptions,
  pub destination: ExportDestination,
}

/// Complete output summary from `execute_export`.
pub struct ExportOutcome {
  pub bytes: Vec<u8>,
  pub format: ExportFormat,
  pub mime_type: &'static str,
  pub width: u32,
  pub height: u32,
  pub file_path: Option<PathBuf>,
  pub clipboard_written: bool,
  pub tree: CompositionTree,
}

impl ExportOutcome {
  pub fn to_data_url(&self) -> String {
    bytes_to_data_url(self.mime_type, &self.bytes)
  }
}

/// Execute the full pure-Rust export pipeline:
/// Scene Graph → Rasterize (1x–4x) → Seam Trim → Watermark → Optional Resize → Encode → Output.
pub fn execute_export(
  req: ExportRequest<'_>,
  clipboard: Option<&ClipboardController>,
) -> Result<ExportOutcome> {
  let scene = rasterize_export_scene(req.screenshot, req.wallpaper, req.options, &req.raster)?;
  let mut final_img = scene.image;

  // Apply explicit resize if `targetWidth`/`targetHeight` overrides or encode scale != 1.0 are given
  let (cur_w, cur_h) = final_img.dimensions();
  let (out_w, out_h) = compute_export_dimensions(
    cur_w,
    cur_h,
    req.encode.scale,
    req.encode.target_width,
    req.encode.target_height,
  );
  if out_w != cur_w || out_h != cur_h {
    final_img = resize_for_export(&final_img, out_w, out_h);
  }

  let (final_w, final_h) = final_img.dimensions();
  let bytes = encode_rgba_image(&final_img, req.encode.format, req.encode.quality)?;
  let mime = req.encode.format.mime_type();

  let mut file_path: Option<PathBuf> = None;
  let mut clipboard_written = false;

  match &req.destination {
    ExportDestination::Memory => {}
    ExportDestination::Downloads { file_name } => {
      file_path = Some(save_bytes_to_downloads(&bytes, mime, file_name)?);
    }
    ExportDestination::Path { file_path: target } => {
      file_path = Some(save_bytes_to_path(&bytes, mime, target)?);
    }
    ExportDestination::Folder { folder_path, file_name } => {
      file_path = Some(save_bytes_to_folder(&bytes, mime, folder_path, file_name)?);
    }
    ExportDestination::Clipboard => {
      if let Some(ctrl) = clipboard {
        ctrl.write_rgba_image(&final_img)?;
        clipboard_written = true;
      } else {
        let local = ClipboardController::default();
        local.write_rgba_image(&final_img)?;
        clipboard_written = true;
      }
    }
  }

  Ok(ExportOutcome {
    bytes,
    format: req.encode.format,
    mime_type: mime,
    width: final_w,
    height: final_h,
    file_path,
    clipboard_written,
    tree: scene.tree,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use image::Rgba;

  #[test]
  fn executes_end_to_end_pipeline_to_folder() {
    let shot = RgbaImage::from_pixel(300, 200, Rgba([60, 120, 240, 255]));
    let mut opts = EditorOptions::default();
    opts.balance.enabled = false;
    opts.shadow = "Hug".into();
    opts.browser_bar = "mac-dark".into();
    opts.watermark.enabled = true;

    let tmp = std::env::temp_dir().join(format!(
      "shotify-pipeline-test-{}",
      chrono::Utc::now().timestamp_nanos_opt().unwrap_or(1)
    ));

    let out = execute_export(
      ExportRequest {
        screenshot: &shot,
        wallpaper: None,
        options: &opts,
        raster: RasterizeOptions {
          target_width: 600,
          scale: 1.5,
          ..Default::default()
        },
        encode: EncodeOptions {
          format: ExportFormat::Jpeg,
          quality: 0.9,
          ..Default::default()
        },
        destination: ExportDestination::Folder {
          folder_path: tmp.clone(),
          file_name: "result".into(),
        },
      },
      None,
    )
    .expect("pipeline succeeds");

    let saved = out.file_path.as_ref().expect("saved file path");
    assert!(saved.exists());
    assert!(saved.extension().and_then(|s| s.to_str()) == Some("jpg"));
    assert_eq!(out.width, 900);
    assert_eq!(out.height, 900);

    let decoded = image::load_from_memory(&out.bytes).expect("valid JPEG bytes");
    assert_eq!((decoded.width(), decoded.height()), (900, 900));

    let _ = std::fs::remove_dir_all(&tmp);
  }
}
