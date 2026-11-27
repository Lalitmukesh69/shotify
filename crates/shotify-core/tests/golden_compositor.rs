use image::{Rgba, RgbaImage};
use shotify_core::{
  compose_screenshot_scene, extract_palette_from_rgba_image, generate_algorithm_gradients,
  render_background_pixmap, AspectMode, BackgroundSpec, EditorOptions, RenderQuality,
};

fn make_test_screenshot(w: u32, h: u32) -> RgbaImage {
  let mut img = RgbaImage::new(w, h);
  for y in 0..h {
    for x in 0..w {
      let r = ((x * 255) / w.max(1)) as u8;
      let g = ((y * 255) / h.max(1)) as u8;
      img.put_pixel(x, y, Rgba([r, g, 210, 255]));
    }
  }
  img
}

#[test]
fn golden_compositor_variants_are_deterministic_and_distinct() {
  let shot = make_test_screenshot(320, 200);

  // 1. Default Hug + MacDark Chrome + Gradient
  let mut base_opts = EditorOptions::default();
  base_opts.aspect_mode = AspectMode::Preset;
  base_opts.aspect_ratio = "aspect-video".to_string();
  base_opts.padding_size = 32.0;
  base_opts.browser_bar = "mac-dark".to_string();
  base_opts.shadow = "Hug".to_string();
  base_opts.balance.enabled = false;

  let out_a = compose_screenshot_scene(&shot, None, &base_opts, 640, 1.0, RenderQuality::Final)
    .expect("base compose");
  let out_b = compose_screenshot_scene(&shot, None, &base_opts, 640, 1.0, RenderQuality::Final)
    .expect("determinism check");

  assert_eq!(out_a.image.dimensions(), (640, 360));
  assert_eq!(out_a.image.as_raw(), out_b.image.as_raw(), "render must be bit-deterministic");

  // 2. BlobGradient preset + WindowsDark chrome + Heavy shadow + Vignette
  let mut blob_opts = base_opts.clone();
  blob_opts.background_type = "blobGradient".to_string();
  blob_opts.browser_bar = "windows-dark".to_string();
  blob_opts.shadow = "Heavy".to_string();
  blob_opts.fx.vignette = 0.6;
  blob_opts.border_width = 3.0;
  let out_blob = compose_screenshot_scene(&shot, None, &blob_opts, 640, 1.0, RenderQuality::Final)
    .expect("blob compose");
  assert_ne!(out_a.image.as_raw(), out_blob.image.as_raw());

  // 3. Ambient glow + 3D Tilt + Inset padding
  let mut tilt_opts = base_opts.clone();
  tilt_opts.shadow = "Ambient".to_string();
  tilt_opts.inset.padding = 16.0;
  tilt_opts.tilt.x = 10.0;
  tilt_opts.tilt.y = -14.0;
  let out_tilt = compose_screenshot_scene(&shot, None, &tilt_opts, 640, 1.0, RenderQuality::Final)
    .expect("tilt + ambient compose");
  assert_eq!(out_tilt.image.dimensions(), (640, 360));
  assert_ne!(out_a.image.as_raw(), out_tilt.image.as_raw());

  // 4. Transparent background preserves outer transparency
  let mut trans_opts = base_opts.clone();
  trans_opts.background_type = "transparent".to_string();
  trans_opts.shadow = "None".to_string();
  let out_trans = compose_screenshot_scene(&shot, None, &trans_opts, 640, 1.0, RenderQuality::Final)
    .expect("transparent compose");
  assert_eq!(out_trans.image.get_pixel(0, 0)[3], 0);
}

#[test]
fn golden_palette_drives_algorithmic_and_wallpaper_renders() {
  let shot = make_test_screenshot(200, 150);
  let pal = extract_palette_from_rgba_image(&shot, 5).expect("palette");
  assert_eq!(pal.colors.len(), 5);

  let presets = generate_algorithm_gradients();
  assert_eq!(presets.len(), 56);

  // Wallpaper background FX with Gaussian blur & brightness
  let wp = make_test_screenshot(400, 300);
  let bg = render_background_pixmap(
    300,
    200,
    1.0,
    &BackgroundSpec::Wallpaper,
    Some(&wp),
    12.0,
    0.85,
    0.4,
    true,
    RenderQuality::Final,
  )
  .expect("wallpaper fx render");
  assert_eq!((bg.width(), bg.height()), (300, 200));
}
