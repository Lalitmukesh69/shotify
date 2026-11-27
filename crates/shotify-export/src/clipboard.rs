use std::borrow::Cow;
use std::sync::Mutex;

use image::RgbaImage;

use crate::encoder::{encode_rgba_image, ExportFormat};
use crate::error::{ExportError, Result};

/// Decoded item returned from clipboard inspection.
pub enum ClipboardContent {
  Image(RgbaImage),
  Text(String),
}

/// Thread-safe clipboard controller using `arboard`.
pub struct ClipboardController(pub Mutex<Option<arboard::Clipboard>>);

impl Default for ClipboardController {
  fn default() -> Self {
    Self(Mutex::new(None))
  }
}

impl ClipboardController {
  pub fn new() -> Self {
    Self::default()
  }

  /// Write raw straight-alpha `RgbaImage` directly to the OS clipboard.
  pub fn write_rgba_image(&self, img: &RgbaImage) -> Result<()> {
    let (w, h) = img.dimensions();
    let raw = img.as_raw().clone();

    let mut guard = self
      .0
      .lock()
      .map_err(|_| ExportError::Clipboard("Clipboard lock poisoned".into()))?;
    if guard.is_none() {
      *guard = Some(
        arboard::Clipboard::new()
          .map_err(|e| ExportError::Clipboard(format!("Clipboard init failed: {e}")))?,
      );
    }
    let clipboard = guard
      .as_mut()
      .ok_or_else(|| ExportError::Clipboard("Clipboard unavailable".into()))?;

    if let Err(err) = clipboard.set_image(arboard::ImageData {
      width: w as usize,
      height: h as usize,
      bytes: Cow::Owned(raw),
    }) {
      *guard = None;
      return Err(ExportError::Clipboard(format!("Clipboard write failed: {err}")));
    }

    Ok(())
  }

  /// Decode encoded image bytes (PNG/JPEG/WebP) and write the pixels to the OS clipboard.
  pub fn write_encoded_bytes(&self, bytes: &[u8]) -> Result<()> {
    let rgba = image::load_from_memory(bytes)?.to_rgba8();
    self.write_rgba_image(&rgba)
  }

  /// Read image pixels or text from the OS clipboard, if present.
  pub fn read_content(&self) -> Result<Option<ClipboardContent>> {
    let mut guard = self
      .0
      .lock()
      .map_err(|_| ExportError::Clipboard("Clipboard lock poisoned".into()))?;
    if guard.is_none() {
      let Ok(cb) = arboard::Clipboard::new() else {
        return Ok(None);
      };
      *guard = Some(cb);
    }
    let Some(clipboard) = guard.as_mut() else {
      return Ok(None);
    };

    if let Ok(img) = clipboard.get_image() {
      let w = img.width as u32;
      let h = img.height as u32;
      if w > 0 && h > 0 {
        if let Some(rgba) = RgbaImage::from_raw(w, h, img.bytes.into_owned()) {
          return Ok(Some(ClipboardContent::Image(rgba)));
        }
      }
    }

    if let Ok(text) = clipboard.get_text() {
      return Ok(Some(ClipboardContent::Text(text)));
    }

    Ok(None)
  }

  /// Convenience helper to read clipboard image encoded as PNG bytes.
  pub fn read_image_png_bytes(&self) -> Result<Option<Vec<u8>>> {
    match self.read_content()? {
      Some(ClipboardContent::Image(rgba)) => {
        let bytes = encode_rgba_image(&rgba, ExportFormat::Png, 1.0)?;
        Ok(Some(bytes))
      }
      _ => Ok(None),
    }
  }
}
