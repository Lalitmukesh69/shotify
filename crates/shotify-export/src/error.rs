use thiserror::Error;

/// Errors produced by `shotify-export` operations.
#[derive(Debug, Error)]
pub enum ExportError {
  #[error(transparent)]
  Core(#[from] shotify_core::CoreError),

  #[error(transparent)]
  Annotate(#[from] shotify_annotate::AnnotateError),

  #[error("image error: {0}")]
  Image(#[from] image::ImageError),

  #[error("I/O error: {0}")]
  Io(#[from] std::io::Error),

  #[error("unsupported export format: {0}")]
  InvalidFormat(String),

  #[error("encoding failed: {0}")]
  Encoding(String),

  #[error("clipboard error: {0}")]
  Clipboard(String),

  #[error("failed to resolve output directory: {0}")]
  OutputDirectory(String),

  #[error("invalid data URL: {0}")]
  DataUrl(String),

  #[error("base64 decode error: {0}")]
  Base64(#[from] base64::DecodeError),

  #[error("invalid path: {0}")]
  InvalidPath(String),
}

pub type Result<T> = std::result::Result<T, ExportError>;
