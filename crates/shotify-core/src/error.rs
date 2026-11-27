use thiserror::Error;

/// Errors returned by `shotify-core` operations.
#[derive(Debug, Error)]
pub enum CoreError {
  #[error("invalid dimensions: {width}x{height}")]
  InvalidDimensions { width: u32, height: u32 },

  #[error("invalid pixel buffer length: expected {expected}, got {actual}")]
  InvalidBufferLength { expected: usize, actual: usize },

  #[error("invalid color specification: {0}")]
  InvalidColor(String),

  #[error("image decode error: {0}")]
  ImageDecode(#[from] image::ImageError),

  #[error("pixmap allocation failed for {width}x{height}")]
  PixmapAllocation { width: u32, height: u32 },

  #[error("serialization error: {0}")]
  Serialization(#[from] serde_json::Error),

  #[error("I/O error: {0}")]
  Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, CoreError>;
