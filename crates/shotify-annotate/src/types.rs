use serde::{Deserialize, Serialize};

/// Active interactive annotation tool selected by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnnotationTool {
  Pen,
  Arrow,
  Shape,
  Text,
  StepBadge,
}

/// Sub-mode when `AnnotationTool::Shape` is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShapeMode {
  #[default]
  Rect,
  Ellipse,
}

/// Kind of vector stroke annotation stored in the scene document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StrokeKind {
  Pen,
  Arrow,
  Rect,
  Ellipse,
}

/// Normalized annotation coordinate in `[0.0, 1.0]` relative to canvas dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PointNorm {
  pub x: f64,
  pub y: f64,
  #[serde(default = "default_pressure")]
  pub pressure: f64,
}

fn default_pressure() -> f64 {
  0.5
}

impl PointNorm {
  pub fn new(x: f64, y: f64) -> Self {
    Self {
      x: x.clamp(0.0, 1.0),
      y: y.clamp(0.0, 1.0),
      pressure: 0.5,
    }
  }

  pub fn with_pressure(x: f64, y: f64, pressure: f64) -> Self {
    Self {
      x: x.clamp(0.0, 1.0),
      y: y.clamp(0.0, 1.0),
      pressure: pressure.clamp(0.0, 1.0),
    }
  }
}

/// Stroke annotation (`pen`, `arrow`, `rect`, or `ellipse`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrokeAnnotation {
  pub id: u64,
  pub kind: StrokeKind,
  pub points: Vec<PointNorm>,
  /// Stroke width relative to `min(canvas_w, canvas_h)`.
  pub width_rel: f64,
  pub color: String,
  #[serde(default = "default_alpha")]
  pub alpha: f64,
}

fn default_alpha() -> f64 {
  1.0
}

/// Text box or circular numbered step badge annotation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextAnnotation {
  pub id: u64,
  pub x_norm: f64,
  pub y_norm: f64,
  pub value: String,
  pub box_w_norm: Option<f64>,
  pub font_size_rel: f64,
  pub pad_x_rel: f64,
  pub pad_y_rel: f64,
  pub radius_rel: f64,
  pub stroke_width_rel: f64,
  pub bg_color: String,
  pub text_color: String,
  pub text_stroke_color: String,
  pub force_circle: bool,
}

/// Complete serializable annotation document overlaid on a Shotify composition.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationDocument {
  #[serde(default)]
  pub strokes: Vec<StrokeAnnotation>,
  #[serde(default)]
  pub texts: Vec<TextAnnotation>,
}

impl AnnotationDocument {
  pub fn is_empty(&self) -> bool {
    self.strokes.is_empty() && self.texts.is_empty()
  }
}

/// Currently selected annotation item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedAnnotation {
  Stroke(usize),
  Text(usize),
}

/// Eight resize handle anchors matching `CanvasAnnotations.jsx`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeHandle {
  TopLeft,
  Top,
  TopRight,
  Right,
  BottomRight,
  Bottom,
  BottomLeft,
  Left,
}

impl ResizeHandle {
  pub const ALL: [ResizeHandle; 8] = [
    Self::TopLeft,
    Self::Top,
    Self::TopRight,
    Self::Right,
    Self::BottomRight,
    Self::Bottom,
    Self::BottomLeft,
    Self::Left,
  ];

  pub fn has_left(self) -> bool {
    matches!(self, Self::TopLeft | Self::Left | Self::BottomLeft)
  }

  pub fn has_right(self) -> bool {
    matches!(self, Self::TopRight | Self::Right | Self::BottomRight)
  }

  pub fn has_top(self) -> bool {
    matches!(self, Self::TopLeft | Self::Top | Self::TopRight)
  }

  pub fn has_bottom(self) -> bool {
    matches!(self, Self::BottomLeft | Self::Bottom | Self::BottomRight)
  }
}

/// Bounding box in canvas pixel coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundsPx {
  pub x: f64,
  pub y: f64,
  pub width: f64,
  pub height: f64,
}

impl BoundsPx {
  pub fn right(self) -> f64 {
    self.x + self.width
  }

  pub fn bottom(self) -> f64 {
    self.y + self.height
  }

  pub fn contains(self, px: f64, py: f64) -> bool {
    px >= self.x && px <= self.right() && py >= self.y && py <= self.bottom()
  }

  pub fn handle_center(self, handle: ResizeHandle) -> (f64, f64) {
    let mid_x = self.x + self.width / 2.0;
    let mid_y = self.y + self.height / 2.0;
    let r = self.right();
    let b = self.bottom();
    match handle {
      ResizeHandle::TopLeft => (self.x, self.y),
      ResizeHandle::Top => (mid_x, self.y),
      ResizeHandle::TopRight => (r, self.y),
      ResizeHandle::Right => (r, mid_y),
      ResizeHandle::BottomRight => (r, b),
      ResizeHandle::Bottom => (mid_x, b),
      ResizeHandle::BottomLeft => (self.x, b),
      ResizeHandle::Left => (self.x, mid_y),
    }
  }
}
