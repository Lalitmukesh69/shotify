pub mod engine;
pub mod error;
pub mod freehand;
pub mod hit_test;
pub mod render;
pub mod text_layout;
pub mod types;

pub use engine::{AnnotationEngine, PointerDownOutcome};
pub use error::{AnnotateError, Result};
pub use freehand::{
  freehand_stroke_outline_points, stroke_outline_to_bez_path, stroke_outline_to_skia_path,
  stroke_outline_to_svg_d, FreehandOptions,
};
pub use hit_test::{
  dist_point_to_segment_sq, hit_test_document, hit_test_resize_handle, is_stroke_hit,
  selection_bounds_px, stroke_bounds_px,
};
pub use render::{render_annotations_onto_pixmap, render_annotations_onto_rgba};
pub use text_layout::{compute_text_layout, measure_text_width, text_bounds_px, ComputedTextLayout};
pub use types::{
  AnnotationDocument, AnnotationTool, BoundsPx, PointNorm, ResizeHandle, SelectedAnnotation,
  ShapeMode, StrokeAnnotation, StrokeKind, TextAnnotation,
};
