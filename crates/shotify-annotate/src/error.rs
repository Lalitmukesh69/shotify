use thiserror::Error;

/// Errors returned by the `shotify-annotate` vector and raster annotation engine.
#[derive(Debug, Error)]
pub enum AnnotateError {
  #[error("invalid font data: {0}")]
  FontLoad(String),
  #[error("invalid target surface dimensions {width}x{height}")]
  InvalidDimensions { width: u32, height: u32 },
  #[error("color parse error: {0}")]
  Color(String),
  #[error("core geometry error: {0}")]
  Core(#[from] shotify_core::CoreError),
}

/// Convenience result alias for `shotify-annotate`.
pub type Result<T> = std::result::Result<T, AnnotateError>;
