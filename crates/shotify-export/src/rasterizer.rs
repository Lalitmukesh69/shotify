use image::RgbaImage;
use serde::{Deserialize, Serialize};
use shotify_annotate::{render_annotations_onto_rgba, AnnotationDocument};
use shotify_core::{
  compose_screenshot_scene, CompositionTree, EditorOptions, RectF, RenderQuality,
};

use crate::error::Result;
use crate::trim::{trim_transparent_edge_seams, TrimSeamsOptions};
use crate::watermark::apply_scene_watermark;

/// Options controlling headless scene-graph rasterization for export.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RasterizeOptions {
  #[serde(default = "default_target_width")]
  pub target_width: u32,
  #[serde(default = "default_scale")]
  pub scale: f64,
  #[serde(default = "default_true")]
  pub trim_transparent_seams: bool,
  #[serde(default)]
  pub trim_opts: TrimSeamsOptions,
  #[serde(default)]
  pub omit_watermark: bool,
  #[serde(default)]
  pub quality: RenderQuality,
  #[serde(default)]
  pub annotations: Option<AnnotationDocument>,
}

fn default_target_width() -> u32 { 1200 }
fn default_scale() -> f64 { 1.0 }
fn default_true() -> bool { true }

impl Default for RasterizeOptions {
  fn default() -> Self {
    Self {
      target_width: 1200,
      scale: 1.0,
      trim_transparent_seams: true,
      trim_opts: TrimSeamsOptions::default(),
      omit_watermark: false,
      quality: RenderQuality::Final,
      annotations: None,
    }
  }
}

/// Output of rasterizing a screenshot scene graph before encoding.
pub struct RasterizedScene {
  pub image: RgbaImage,
  pub tree: CompositionTree,
}

/// Rasterize `screenshot` + optional `wallpaper` + `EditorOptions` into an export-ready `RgbaImage`.
/// Handles scale multipliers directly cleanly in scene space (1x..4x), optional transparent seam trim,
/// and scene-relative pill watermark overlay.
pub fn rasterize_export_scene(
  screenshot: &RgbaImage,
  wallpaper: Option<&RgbaImage>,
  options: &EditorOptions,
  raster_opts: &RasterizeOptions,
) -> Result<RasterizedScene> {
  let scale = if raster_opts.scale.is_finite() {
    raster_opts.scale.clamp(0.25, 4.0)
  } else {
    1.0
  };
  let target_w = raster_opts.target_width.max(1);

  let mut composed = compose_screenshot_scene(
    screenshot,
    wallpaper,
    options,
    target_w,
    scale,
    raster_opts.quality,
  )?;

  if let Some(ref docs) = raster_opts.annotations {
    if !docs.is_empty() {
      render_annotations_onto_rgba(&mut composed.image, docs)?;
    }
  }

  if raster_opts.trim_transparent_seams {
    composed.image = trim_transparent_edge_seams(&composed.image, raster_opts.trim_opts);
  }

  if !raster_opts.omit_watermark && options.watermark.enabled {
    let cw = f64::from(composed.image.width());
    let ch = f64::from(composed.image.height());
    let card_w = (composed.tree.layout.display_width * scale).max(1.0);
    let card_h = (composed.tree.layout.card_total_height * scale).max(1.0);
    let card_bounds = RectF {
      left: (cw - card_w) / 2.0 + options.position.x * scale,
      top: (ch - card_h) / 2.0 + options.position.y * scale,
      width: card_w,
      height: card_h,
    };
    apply_scene_watermark(&mut composed.image, card_bounds, &options.watermark, scale)?;
  }

  Ok(RasterizedScene {
    image: composed.image,
    tree: composed.tree,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use image::Rgba;

  #[test]
  fn rasterizer_scales_cleanly_to_2x() {
    let shot = RgbaImage::from_pixel(200, 100, Rgba([100, 160, 240, 255]));
    let mut opts = EditorOptions::default();
    opts.balance.enabled = false;
    opts.aspect_ratio = "aspect-square".to_string();

    let r1 = rasterize_export_scene(
      &shot,
      None,
      &opts,
      &RasterizeOptions { target_width: 400, scale: 1.0, ..Default::default() },
    )
    .unwrap();
    let r2 = rasterize_export_scene(
      &shot,
      None,
      &opts,
      &RasterizeOptions { target_width: 400, scale: 2.0, ..Default::default() },
    )
    .unwrap();

    assert_eq!(r1.image.dimensions(), (400, 400));
    assert_eq!(r2.image.dimensions(), (800, 800));
  }

  #[test]
  fn rasterizer_renders_annotations_onto_exported_scene() {
    let shot = RgbaImage::from_pixel(200, 100, Rgba([40, 80, 160, 255]));
    let mut opts = EditorOptions::default();
    opts.balance.enabled = false;
    opts.aspect_ratio = "aspect-square".to_string();

    let base = rasterize_export_scene(
      &shot,
      None,
      &opts,
      &RasterizeOptions { target_width: 400, scale: 1.0, ..Default::default() },
    )
    .unwrap();

    let mut engine = shotify_annotate::AnnotationEngine::new();
    engine.active_tool = Some(shotify_annotate::AnnotationTool::StepBadge);
    engine.place_step_badge(200.0, 200.0, 400.0, 400.0);

    let with_ann = rasterize_export_scene(
      &shot,
      None,
      &opts,
      &RasterizeOptions {
        target_width: 400,
        scale: 1.0,
        annotations: Some(engine.document),
        ..Default::default()
      },
    )
    .unwrap();

    assert_ne!(base.image.as_raw(), with_ann.image.as_raw());
  }
}
