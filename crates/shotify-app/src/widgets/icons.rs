use std::sync::OnceLock;

use iced::widget::image::Handle as ImageHandle;
use iced::widget::svg::Handle as SvgHandle;

// Lucide-style crisp vector icons matching `ui/src/AnnotationBar.jsx`, `ExportFooter.jsx`, and `Sidebar.jsx`.

const PEN_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9"/><path d="M16.376 3.622a1 1 0 0 1 3.002 3.002L7.368 18.635a2 2 0 0 1-.855.506l-2.872.838a.5.5 0 0 1-.62-.62l.838-2.872a2 2 0 0 1 .506-.854z"/></svg>"#;
const TYPE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 7 4 4 20 4 20 7"/><line x1="9" x2="15" y1="20" y2="20"/><line x1="12" x2="12" y1="4" y2="20"/></svg>"#;
const ARROW_UP_RIGHT_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M7 7h10v10"/><path d="M7 17 17 7"/></svg>"#;
const SQUARE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="18" x="3" y="3" rx="2"/></svg>"#;
const CIRCLE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/></svg>"#;
const ROTATE_CCW_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/></svg>"#;
const ROTATE_CCW_MUTED_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#8E8E93" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/></svg>"##;
const CHEVRON_DOWN_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#BDBDBD" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6"/></svg>"##;
const CHEVRON_RIGHT_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#BDBDBD" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m9 6 6 6-6 6"/></svg>"##;
const FOLDER_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/></svg>"#;
const TRASH_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#B4B4B8" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/><line x1="10" x2="10" y1="11" y2="17"/><line x1="14" x2="14" y1="11" y2="17"/></svg>"##;
const STEP_ONE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M10.5 9.5 12.5 8v8"/><path d="M10.5 16h4"/></svg>"#;
const SETTINGS_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>"#;
const UPLOAD_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" x2="12" y1="3" y2="15"/></svg>"#;
const UNDO_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 14 4 9l5-5"/><path d="M4 9h10.5a5.5 5.5 0 0 1 5.5 5.5a5.5 5.5 0 0 1-5.5 5.5H11"/></svg>"#;
const REDO_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m15 14 5-5-5-5"/><path d="M20 9H9.5A5.5 5.5 0 0 0 4 14.5A5.5 5.5 0 0 0 9.5 20H13"/></svg>"#;
const WINDOW_MINIMIZE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2.2" stroke-linecap="round"><line x1="5" y1="12" x2="19" y2="12"/></svg>"#;
const WINDOW_MAXIMIZE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="5" y="5" rx="1.5"/></svg>"#;
const WINDOW_RESTORE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="11" height="11" x="4" y="9" rx="1.5"/><path d="M8 9V6.5A1.5 1.5 0 0 1 9.5 5H18a2 2 0 0 1 2 2v8.5a1.5 1.5 0 0 1-1.5 1.5H15"/></svg>"#;
const WINDOW_CLOSE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><line x1="6" y1="6" x2="18" y2="18"/><line x1="18" y1="6" x2="6" y2="18"/></svg>"#;

// Official shadow & OS mockup thumbnails matching `ui/src/Sidebar.jsx` and `ui/src/App.jsx`.
const SHADOW_HUG_BYTES: &[u8] = include_bytes!("../../../../ui/Hugshadowthumb.png");
const SHADOW_HEAVY_BYTES: &[u8] = include_bytes!("../../../../ui/Heavyshadowthumb.png");
const SHADOW_AMBIENT_BYTES: &[u8] = include_bytes!("../../../../ui/Ambientshadowthumb.png");
const SHADOW_CUSTOM_BYTES: &[u8] = include_bytes!("../../../../ui/customshadowthumb.png");

const MOCKUP_MAC_LIGHT_BYTES: &[u8] = include_bytes!("../../../../ui/os-mockup-mac-1-thumb.png");
const MOCKUP_MAC_DARK_BYTES: &[u8] = include_bytes!("../../../../ui/os-mockup-mac-2-thumb.png");
const MOCKUP_WIN_LIGHT_BYTES: &[u8] = include_bytes!("../../../../ui/os-mockup-win-2-thumb.png");
const MOCKUP_WIN_DARK_BYTES: &[u8] = include_bytes!("../../../../ui/os-mockup-win-1-thumb.png");
const MOCKUP_SAFARI_DARK_BYTES: &[u8] = include_bytes!("../../../../ui/os-mockup-safari-1-thumb.png");

pub fn pen_svg() -> SvgHandle {
  SvgHandle::from_memory(PEN_SVG)
}
pub fn type_svg() -> SvgHandle {
  SvgHandle::from_memory(TYPE_SVG)
}
pub fn arrow_up_right_svg() -> SvgHandle {
  SvgHandle::from_memory(ARROW_UP_RIGHT_SVG)
}
pub fn square_svg() -> SvgHandle {
  SvgHandle::from_memory(SQUARE_SVG)
}
pub fn circle_svg() -> SvgHandle {
  SvgHandle::from_memory(CIRCLE_SVG)
}
pub fn rotate_ccw_svg() -> SvgHandle {
  SvgHandle::from_memory(ROTATE_CCW_SVG)
}
pub fn rotate_ccw_muted_svg() -> SvgHandle {
  SvgHandle::from_memory(ROTATE_CCW_MUTED_SVG)
}
pub fn chevron_down_svg() -> SvgHandle {
  SvgHandle::from_memory(CHEVRON_DOWN_SVG)
}
pub fn chevron_right_svg() -> SvgHandle {
  SvgHandle::from_memory(CHEVRON_RIGHT_SVG)
}
pub fn folder_svg() -> SvgHandle {
  SvgHandle::from_memory(FOLDER_SVG)
}
pub fn trash_svg() -> SvgHandle {
  SvgHandle::from_memory(TRASH_SVG)
}
pub fn step_one_svg() -> SvgHandle {
  SvgHandle::from_memory(STEP_ONE_SVG)
}
pub fn settings_svg() -> SvgHandle {
  SvgHandle::from_memory(SETTINGS_SVG)
}
pub fn upload_svg() -> SvgHandle {
  SvgHandle::from_memory(UPLOAD_SVG)
}
pub fn undo_svg() -> SvgHandle {
  SvgHandle::from_memory(UNDO_SVG)
}
pub fn redo_svg() -> SvgHandle {
  SvgHandle::from_memory(REDO_SVG)
}
pub fn window_minimize_svg() -> SvgHandle {
  SvgHandle::from_memory(WINDOW_MINIMIZE_SVG)
}
pub fn window_maximize_svg() -> SvgHandle {
  SvgHandle::from_memory(WINDOW_MAXIMIZE_SVG)
}
pub fn window_restore_svg() -> SvgHandle {
  SvgHandle::from_memory(WINDOW_RESTORE_SVG)
}
pub fn window_close_svg() -> SvgHandle {
  SvgHandle::from_memory(WINDOW_CLOSE_SVG)
}

fn decode_static_thumb(bytes: &'static [u8]) -> ImageHandle {
  match image::load_from_memory(bytes) {
    Ok(img) => {
      let rgba = img.to_rgba8();
      let (w, h) = rgba.dimensions();
      ImageHandle::from_rgba(w, h, rgba.into_raw())
    }
    Err(_) => ImageHandle::from_rgba(1, 1, vec![44, 44, 44, 255]),
  }
}

pub fn shadow_hug_handle() -> ImageHandle {
  static CACHE: OnceLock<ImageHandle> = OnceLock::new();
  CACHE.get_or_init(|| decode_static_thumb(SHADOW_HUG_BYTES)).clone()
}
pub fn shadow_heavy_handle() -> ImageHandle {
  static CACHE: OnceLock<ImageHandle> = OnceLock::new();
  CACHE.get_or_init(|| decode_static_thumb(SHADOW_HEAVY_BYTES)).clone()
}
pub fn shadow_ambient_handle() -> ImageHandle {
  static CACHE: OnceLock<ImageHandle> = OnceLock::new();
  CACHE.get_or_init(|| decode_static_thumb(SHADOW_AMBIENT_BYTES)).clone()
}
pub fn shadow_custom_handle() -> ImageHandle {
  static CACHE: OnceLock<ImageHandle> = OnceLock::new();
  CACHE.get_or_init(|| decode_static_thumb(SHADOW_CUSTOM_BYTES)).clone()
}

pub fn mockup_mac_light_handle() -> ImageHandle {
  static CACHE: OnceLock<ImageHandle> = OnceLock::new();
  CACHE.get_or_init(|| decode_static_thumb(MOCKUP_MAC_LIGHT_BYTES)).clone()
}
pub fn mockup_mac_dark_handle() -> ImageHandle {
  static CACHE: OnceLock<ImageHandle> = OnceLock::new();
  CACHE.get_or_init(|| decode_static_thumb(MOCKUP_MAC_DARK_BYTES)).clone()
}
pub fn mockup_win_light_handle() -> ImageHandle {
  static CACHE: OnceLock<ImageHandle> = OnceLock::new();
  CACHE.get_or_init(|| decode_static_thumb(MOCKUP_WIN_LIGHT_BYTES)).clone()
}
pub fn mockup_win_dark_handle() -> ImageHandle {
  static CACHE: OnceLock<ImageHandle> = OnceLock::new();
  CACHE.get_or_init(|| decode_static_thumb(MOCKUP_WIN_DARK_BYTES)).clone()
}
pub fn mockup_safari_dark_handle() -> ImageHandle {
  static CACHE: OnceLock<ImageHandle> = OnceLock::new();
  CACHE.get_or_init(|| decode_static_thumb(MOCKUP_SAFARI_DARK_BYTES)).clone()
}

/// Rainbow conic gradient swatch icon matching the 14th `+` custom color tile in `WallpaperGrid.jsx`.
pub fn conic_custom_color_handle() -> ImageHandle {
  static CACHE: OnceLock<ImageHandle> = OnceLock::new();
  CACHE
    .get_or_init(|| {
      let size = 48u32;
      let mut rgba = vec![0u8; (size * size * 4) as usize];
      let cx = (size as f32) / 2.0;
      let cy = (size as f32) / 2.0;

      for y in 0..size {
        for x in 0..size {
          let dx = (x as f32) + 0.5 - cx;
          let dy = (y as f32) + 0.5 - cy;
          let ang = (dy.atan2(dx).to_degrees() + 210.0).rem_euclid(360.0);
          let (r, g, b) = hsv_to_rgb(ang, 0.88, 0.96);
          let idx = ((y * size + x) * 4) as usize;
          rgba[idx] = r;
          rgba[idx + 1] = g;
          rgba[idx + 2] = b;
          rgba[idx + 3] = 255;
        }
      }
      ImageHandle::from_rgba(size, size, rgba)
    })
    .clone()
}

pub fn hsv_to_rgb(h_deg: f32, s: f32, v: f32) -> (u8, u8, u8) {
  let h = h_deg.rem_euclid(360.0) / 60.0;
  let c = v * s;
  let x = c * (1.0 - (h % 2.0 - 1.0).abs());
  let m = v - c;
  let (r1, g1, b1) = match h.floor() as i32 {
    0 => (c, x, 0.0),
    1 => (x, c, 0.0),
    2 => (0.0, c, x),
    3 => (0.0, x, c),
    4 => (x, 0.0, c),
    _ => (c, 0.0, x),
  };
  (
    ((r1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    ((g1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    ((b1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
  )
}
