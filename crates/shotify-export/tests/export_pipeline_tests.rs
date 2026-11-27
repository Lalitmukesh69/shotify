use image::{Rgba, RgbaImage};
use shotify_core::{AspectMode, EditorOptions};
use shotify_export::{
  execute_export, EncodeOptions, ExportDestination, ExportFormat, ExportRequest, RasterizeOptions,
};

fn sample_screenshot() -> RgbaImage {
  let mut img = RgbaImage::new(240, 150);
  for y in 0..150 {
    for x in 0..240 {
      img.put_pixel(
        x,
        y,
        Rgba([((x * 255) / 240) as u8, ((y * 255) / 150) as u8, 190, 255]),
      );
    }
  }
  img
}

#[test]
fn export_pipeline_renders_all_formats_and_scales_accurately() {
  let shot = sample_screenshot();
  let mut opts = EditorOptions::default();
  opts.balance.enabled = false;
  opts.aspect_mode = AspectMode::Preset;
  opts.aspect_ratio = "aspect-video".to_string();
  opts.shadow = "Realistic".to_string();
  opts.browser_bar = "safari-dark".to_string();
  opts.watermark.enabled = true;
  opts.watermark.show_verified = true;

  for (fmt, scale, expected_w, expected_h) in [
    (ExportFormat::Png, 1.0, 500, 281),
    (ExportFormat::Png, 2.0, 1000, 562),
    (ExportFormat::Jpeg, 1.5, 750, 422),
    (ExportFormat::Webp, 1.0, 500, 281),
  ] {
    let outcome = execute_export(
      ExportRequest {
        screenshot: &shot,
        wallpaper: None,
        options: &opts,
        raster: RasterizeOptions {
          target_width: 500,
          scale,
          ..Default::default()
        },
        encode: EncodeOptions {
          format: fmt,
          quality: 0.88,
          ..Default::default()
        },
        destination: ExportDestination::Memory,
      },
      None,
    )
    .expect("export succeeds");

    assert_eq!((outcome.width, outcome.height), (expected_w, expected_h));
    let decoded = image::load_from_memory(&outcome.bytes).expect("roundtrip decode");
    assert_eq!((decoded.width(), decoded.height()), (expected_w, expected_h));
  }
}

#[test]
fn jpeg_flattens_transparent_background_over_white_matte() {
  let shot = sample_screenshot();
  let mut opts = EditorOptions::default();
  opts.balance.enabled = false;
  opts.background_type = "transparent".to_string();
  opts.shadow = "None".to_string();
  opts.watermark.enabled = false;
  opts.padding_size = 40.0;
  opts.aspect_ratio = "aspect-square".to_string();

  let out = execute_export(
    ExportRequest {
      screenshot: &shot,
      wallpaper: None,
      options: &opts,
      raster: RasterizeOptions {
        target_width: 300,
        scale: 1.0,
        trim_transparent_seams: false,
        ..Default::default()
      },
      encode: EncodeOptions {
        format: ExportFormat::Jpeg,
        quality: 0.95,
        ..Default::default()
      },
      destination: ExportDestination::Memory,
    },
    None,
  )
  .expect("jpeg transparent flatten succeeds");

  let decoded = image::load_from_memory(&out.bytes).unwrap().to_rgb8();
  let corner = decoded.get_pixel(0, 0);
  // JPEG outer transparent border should be clean white matte, not black
  assert!(corner[0] > 245 && corner[1] > 245 && corner[2] > 245);
}

#[test]
fn target_width_override_rescales_final_output() {
  let shot = sample_screenshot();
  let mut opts = EditorOptions::default();
  opts.balance.enabled = false;
  opts.aspect_ratio = "aspect-square".to_string();

  let out = execute_export(
    ExportRequest {
      screenshot: &shot,
      wallpaper: None,
      options: &opts,
      raster: RasterizeOptions {
        target_width: 400,
        scale: 1.0,
        ..Default::default()
      },
      encode: EncodeOptions {
        format: ExportFormat::Png,
        quality: 1.0,
        scale: 1.0,
        target_width: Some(640),
        target_height: Some(640),
      },
      destination: ExportDestination::Memory,
    },
    None,
  )
  .unwrap();

  assert_eq!((out.width, out.height), (640, 640));
}
