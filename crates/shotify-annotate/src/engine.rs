use crate::hit_test::{hit_test_document, hit_test_resize_handle, selection_bounds_px};
use crate::text_layout::text_bounds_px;
use crate::types::{
  AnnotationDocument, AnnotationTool, BoundsPx, PointNorm, ResizeHandle, SelectedAnnotation,
  ShapeMode, StrokeAnnotation, StrokeKind, TextAnnotation,
};

/// Active pointer gesture inside the interactive annotation engine.
#[derive(Debug, Clone)]
enum GestureState {
  Idle,
  DrawingStroke {
    stroke_index: usize,
  },
  DraggingSelected {
    target: SelectedAnnotation,
    last_x: f64,
    last_y: f64,
  },
  ResizingSelected {
    target: SelectedAnnotation,
    handle: ResizeHandle,
    start_x: f64,
    start_y: f64,
    start_bounds: BoundsPx,
    start_text: Option<TextAnnotation>,
    start_stroke: Option<StrokeAnnotation>,
  },
}

/// Result returned from `AnnotationEngine::pointer_down`.
#[derive(Debug, Clone, PartialEq)]
pub enum PointerDownOutcome {
  None,
  StartedDrawing,
  PlacedStepBadge(usize),
  OpenTextPrompt { x_norm: f64, y_norm: f64 },
  Selected(SelectedAnnotation),
  ClearedSelection,
}

/// Stateful annotation workbench managing tools, step numbering, undo/redo, selection,
/// drag translation, and 8-handle resizing.
#[derive(Debug, Clone)]
pub struct AnnotationEngine {
  pub document: AnnotationDocument,
  pub active_tool: Option<AnnotationTool>,
  pub shape_mode: ShapeMode,
  pub active_color: String,
  pub selection: Option<SelectedAnnotation>,
  undo_stack: Vec<AnnotationDocument>,
  redo_stack: Vec<AnnotationDocument>,
  next_id: u64,
  gesture: GestureState,
  mutation_snapshot: Option<AnnotationDocument>,
}

impl Default for AnnotationEngine {
  fn default() -> Self {
    Self {
      document: AnnotationDocument::default(),
      active_tool: None,
      shape_mode: ShapeMode::Rect,
      active_color: "#7c3aed".to_string(),
      selection: None,
      undo_stack: Vec::new(),
      redo_stack: Vec::new(),
      next_id: 1,
      gesture: GestureState::Idle,
      mutation_snapshot: None,
    }
  }
}

impl AnnotationEngine {
  pub fn new() -> Self {
    Self::default()
  }

  fn alloc_id(&mut self) -> u64 {
    let id = self.next_id;
    self.next_id += 1;
    id
  }

  fn push_undo(&mut self, prev: AnnotationDocument) {
    if self.undo_stack.len() >= 100 {
      self.undo_stack.remove(0);
    }
    self.undo_stack.push(prev);
    self.redo_stack.clear();
  }

  pub fn can_undo(&self) -> bool {
    !self.undo_stack.is_empty()
  }

  pub fn can_redo(&self) -> bool {
    !self.redo_stack.is_empty()
  }

  /// Undo the last committed annotation mutation.
  pub fn undo(&mut self) -> bool {
    if let Some(prev) = self.undo_stack.pop() {
      let current = std::mem::replace(&mut self.document, prev);
      self.redo_stack.push(current);
      self.selection = None;
      self.gesture = GestureState::Idle;
      true
    } else {
      false
    }
  }

  /// Redo the last undone annotation mutation.
  pub fn redo(&mut self) -> bool {
    if let Some(next) = self.redo_stack.pop() {
      let current = std::mem::replace(&mut self.document, next);
      self.undo_stack.push(current);
      self.selection = None;
      self.gesture = GestureState::Idle;
      true
    } else {
      false
    }
  }

  /// Compute the next visible circular counter step number (`max(seen) + 1`).
  pub fn next_step_number(&self) -> u32 {
    let mut max_seen = 0_u32;
    for t in &self.document.texts {
      if !t.force_circle {
        continue;
      }
      if let Ok(n) = t.value.trim().parse::<u32>() {
        if n > max_seen {
          max_seen = n;
        }
      }
    }
    (max_seen + 1).clamp(1, 9999)
  }

  /// Place a circular numbered step badge centered on pixel coordinate `(px, py)`.
  pub fn place_step_badge(&mut self, px: f64, py: f64, doc_w: f64, doc_h: f64) -> usize {
    let prev = self.document.clone();
    let n = self.next_step_number();
    let value = n.to_string();
    let digits = value.len();

    let min_dim = doc_w.min(doc_h).max(1.0);
    let badge_diameter_px = match digits {
      1 => 52.0,
      2 => 58.0,
      _ => 64.0,
    };
    let font_size_px = match digits {
      1 => 28.0,
      2 => 22.0,
      _ => 17.0,
    };

    let x_norm = ((px - badge_diameter_px / 2.0) / doc_w.max(1.0)).clamp(0.0, 1.0);
    let y_norm = ((py - badge_diameter_px / 2.0) / doc_h.max(1.0)).clamp(0.0, 1.0);

    let bg = if self.active_color.trim().is_empty() {
      "#7c3aed".to_string()
    } else {
      self.active_color.clone()
    };

    let id = self.alloc_id();
    self.document.texts.push(TextAnnotation {
      id,
      x_norm,
      y_norm,
      value,
      box_w_norm: Some(badge_diameter_px / doc_w.max(1.0)),
      font_size_rel: font_size_px / min_dim,
      pad_x_rel: 14.0 / min_dim,
      pad_y_rel: 14.0 / min_dim,
      radius_rel: 999.0 / min_dim,
      stroke_width_rel: 0.0,
      bg_color: bg,
      text_color: "#ffffff".to_string(),
      text_stroke_color: "rgba(0,0,0,0)".to_string(),
      force_circle: true,
    });

    self.push_undo(prev);
    let idx = self.document.texts.len() - 1;
    self.selection = Some(SelectedAnnotation::Text(idx));
    idx
  }

  /// Commit a new or edited text box at normalized coordinates `(x_norm, y_norm)`.
  pub fn commit_text(
    &mut self,
    value: &str,
    x_norm: f64,
    y_norm: f64,
    doc_w: f64,
    doc_h: f64,
    edit_index: Option<usize>,
  ) -> Option<usize> {
    let trimmed = value.trim_end();
    if trimmed.trim().is_empty() {
      return None;
    }

    let prev = self.document.clone();
    let min_dim = doc_w.min(doc_h).max(1.0);
    let bg = if self.active_color.trim().is_empty() {
      "#000000".to_string()
    } else {
      self.active_color.clone()
    };

    let idx = if let Some(existing_idx) = edit_index.filter(|&i| i < self.document.texts.len()) {
      let existing = &mut self.document.texts[existing_idx];
      existing.value = trimmed.to_string();
      existing.x_norm = x_norm.clamp(0.0, 1.0);
      existing.y_norm = y_norm.clamp(0.0, 1.0);
      existing_idx
    } else {
      let id = self.alloc_id();
      self.document.texts.push(TextAnnotation {
        id,
        x_norm: x_norm.clamp(0.0, 1.0),
        y_norm: y_norm.clamp(0.0, 1.0),
        value: trimmed.to_string(),
        box_w_norm: None,
        font_size_rel: 36.0 / min_dim,
        pad_x_rel: 14.0 / min_dim,
        pad_y_rel: 8.0 / min_dim,
        radius_rel: 10.0 / min_dim,
        stroke_width_rel: 5.0 / min_dim,
        bg_color: bg,
        text_color: "#ffffff".to_string(),
        text_stroke_color: "#000000".to_string(),
        force_circle: false,
      });
      self.document.texts.len() - 1
    };

    self.push_undo(prev);
    self.selection = Some(SelectedAnnotation::Text(idx));
    Some(idx)
  }

  /// Handle pointer down inside canvas pixel coordinates `(px, py)` over surface `doc_w x doc_h`.
  pub fn pointer_down(
    &mut self,
    px: f64,
    py: f64,
    doc_w: f64,
    doc_h: f64,
  ) -> PointerDownOutcome {
    let w = doc_w.max(1.0);
    let h = doc_h.max(1.0);
    let x_norm = (px / w).clamp(0.0, 1.0);
    let y_norm = (py / h).clamp(0.0, 1.0);
    let min_dim = w.min(h).max(1.0);

    match self.active_tool {
      Some(AnnotationTool::Text) => PointerDownOutcome::OpenTextPrompt { x_norm, y_norm },
      Some(AnnotationTool::StepBadge) => {
        let idx = self.place_step_badge(px, py, w, h);
        PointerDownOutcome::PlacedStepBadge(idx)
      }
      Some(tool) => {
        let prev = self.document.clone();
        let (kind, width_rel, default_color, initial_pts) = match tool {
          AnnotationTool::Pen => (
            StrokeKind::Pen,
            4.0 / min_dim,
            "#ffffff",
            vec![PointNorm::new(x_norm, y_norm)],
          ),
          AnnotationTool::Arrow => (
            StrokeKind::Arrow,
            6.0 / min_dim,
            "#FF3B30",
            vec![PointNorm::new(x_norm, y_norm), PointNorm::new(x_norm, y_norm)],
          ),
          AnnotationTool::Shape => {
            let kind = match self.shape_mode {
              ShapeMode::Rect => StrokeKind::Rect,
              ShapeMode::Ellipse => StrokeKind::Ellipse,
            };
            (
              kind,
              3.0 / min_dim,
              "#FF3B30",
              vec![PointNorm::new(x_norm, y_norm), PointNorm::new(x_norm, y_norm)],
            )
          }
          AnnotationTool::Text | AnnotationTool::StepBadge => unreachable!(),
        };

        let color = if self.active_color.trim().is_empty() {
          default_color.to_string()
        } else {
          self.active_color.clone()
        };

        let id = self.alloc_id();
        self.document.strokes.push(StrokeAnnotation {
          id,
          kind,
          points: initial_pts,
          width_rel,
          color,
          alpha: 1.0,
        });
        let stroke_index = self.document.strokes.len() - 1;
        self.mutation_snapshot = Some(prev);
        self.gesture = GestureState::DrawingStroke { stroke_index };
        self.selection = None;
        PointerDownOutcome::StartedDrawing
      }
      None => {
        // Check active selection resize handles first
        if let Some(selected) = self.selection {
          if let Some(bounds) = selection_bounds_px(&self.document, w, h, selected) {
            if let Some(handle) = hit_test_resize_handle(bounds, px, py, 9.0) {
              self.mutation_snapshot = Some(self.document.clone());
              let start_text = match selected {
                SelectedAnnotation::Text(i) => self.document.texts.get(i).cloned(),
                SelectedAnnotation::Stroke(_) => None,
              };
              let start_stroke = match selected {
                SelectedAnnotation::Stroke(i) => self.document.strokes.get(i).cloned(),
                SelectedAnnotation::Text(_) => None,
              };
              self.gesture = GestureState::ResizingSelected {
                target: selected,
                handle,
                start_x: px,
                start_y: py,
                start_bounds: bounds,
                start_text,
                start_stroke,
              };
              return PointerDownOutcome::Selected(selected);
            }
          }
        }

        if let Some(hit) = hit_test_document(&self.document, w, h, px, py) {
          self.selection = Some(hit);
          self.mutation_snapshot = Some(self.document.clone());
          self.gesture = GestureState::DraggingSelected {
            target: hit,
            last_x: px,
            last_y: py,
          };
          PointerDownOutcome::Selected(hit)
        } else {
          let had = self.selection.take().is_some();
          self.gesture = GestureState::Idle;
          if had {
            PointerDownOutcome::ClearedSelection
          } else {
            PointerDownOutcome::None
          }
        }
      }
    }
  }

  /// Update the in-progress draw, move, or resize gesture at canvas pixel coordinate `(px, py)`.
  pub fn pointer_move(&mut self, px: f64, py: f64, doc_w: f64, doc_h: f64) -> bool {
    let w = doc_w.max(1.0);
    let h = doc_h.max(1.0);
    let min_dim = w.min(h).max(1.0);

    match &mut self.gesture {
      GestureState::Idle => false,
      GestureState::DrawingStroke { stroke_index } => {
        let Some(stroke) = self.document.strokes.get_mut(*stroke_index) else {
          return false;
        };
        let x_norm = (px / w).clamp(0.0, 1.0);
        let y_norm = (py / h).clamp(0.0, 1.0);
        if stroke.kind == StrokeKind::Pen {
          stroke.points.push(PointNorm::new(x_norm, y_norm));
        } else if stroke.points.len() >= 2 {
          let last = stroke.points.len() - 1;
          stroke.points[last] = PointNorm::new(x_norm, y_norm);
        } else {
          stroke.points.push(PointNorm::new(x_norm, y_norm));
        }
        true
      }
      GestureState::DraggingSelected {
        target,
        last_x,
        last_y,
      } => {
        let dx = px - *last_x;
        let dy = py - *last_y;
        *last_x = px;
        *last_y = py;

        match *target {
          SelectedAnnotation::Text(idx) => {
            let Some(t) = self.document.texts.get_mut(idx) else {
              return false;
            };
            let box_px = text_bounds_px(w, h, t);
            let cur_x = t.x_norm * w;
            let cur_y = t.y_norm * h;
            let next_x = (cur_x + dx).clamp(0.0, (w - box_px.width).max(0.0));
            let next_y = (cur_y + dy).clamp(0.0, (h - box_px.height).max(0.0));
            t.x_norm = (next_x / w).clamp(0.0, 1.0);
            t.y_norm = (next_y / h).clamp(0.0, 1.0);
            true
          }
          SelectedAnnotation::Stroke(idx) => {
            let Some(s) = self.document.strokes.get_mut(idx) else {
              return false;
            };
            let dx_norm = dx / w;
            let dy_norm = dy / h;
            for pt in &mut s.points {
              pt.x = (pt.x + dx_norm).clamp(0.0, 1.0);
              pt.y = (pt.y + dy_norm).clamp(0.0, 1.0);
            }
            true
          }
        }
      }
      GestureState::ResizingSelected {
        target,
        handle,
        start_x,
        start_y,
        start_bounds,
        start_text,
        start_stroke,
      } => {
        let dx = px - *start_x;
        let dy = py - *start_y;
        let min_w = 40.0_f64;
        let min_h = 24.0_f64;

        let start_l = start_bounds.x;
        let start_t = start_bounds.y;
        let start_r = start_bounds.right();
        let start_b = start_bounds.bottom();

        let next_l = if handle.has_left() {
          (start_l + dx).clamp(0.0, (start_r - min_w).max(0.0))
        } else {
          start_l
        };
        let next_r = if handle.has_right() {
          (start_r + dx).clamp((start_l + min_w).min(w), w)
        } else {
          start_r
        };
        let next_t = if handle.has_top() {
          (start_t + dy).clamp(0.0, (start_b - min_h).max(0.0))
        } else {
          start_t
        };
        let next_b = if handle.has_bottom() {
          (start_b + dy).clamp((start_t + min_h).min(h), h)
        } else {
          start_b
        };

        let next_w = (next_r - next_l).max(min_w);
        let next_h = (next_b - next_t).max(min_h);

        match *target {
          SelectedAnnotation::Text(idx) => {
            let (Some(t), Some(base)) = (self.document.texts.get_mut(idx), start_text.as_ref())
            else {
              return false;
            };
            t.x_norm = (next_l / w).clamp(0.0, 1.0);
            t.y_norm = (next_t / h).clamp(0.0, 1.0);
            t.box_w_norm = Some((next_w / w).clamp(0.01, 1.0));

            let is_width_only = (handle.has_left() || handle.has_right())
              && !(handle.has_top() || handle.has_bottom());
            let is_height_only = (handle.has_top() || handle.has_bottom())
              && !(handle.has_left() || handle.has_right());

            if !is_width_only {
              let scale_w = (next_w / start_bounds.width.max(1.0)).clamp(0.2, 6.0);
              let scale_h = (next_h / start_bounds.height.max(1.0)).clamp(0.2, 6.0);
              let raw_scale = if is_height_only {
                scale_h
              } else {
                scale_w.min(scale_h)
              };
              let base_font = base.font_size_rel.max(1e-6);
              let min_scale = (10.0 / min_dim) / base_font;
              let scale = raw_scale.clamp(min_scale.min(6.0), 6.0);
              t.font_size_rel = base.font_size_rel * scale;
              t.pad_x_rel = base.pad_x_rel * scale;
              t.pad_y_rel = base.pad_y_rel * scale;
              t.radius_rel = base.radius_rel * scale;
              t.stroke_width_rel = base.stroke_width_rel * scale;
            }
            true
          }
          SelectedAnnotation::Stroke(idx) => {
            let (Some(s), Some(base)) =
              (self.document.strokes.get_mut(idx), start_stroke.as_ref())
            else {
              return false;
            };
            let pad_px = 10.0;
            let orig_l = (start_bounds.x + pad_px).min(start_bounds.right() - 1.0);
            let orig_t = (start_bounds.y + pad_px).min(start_bounds.bottom() - 1.0);
            let orig_w = (start_bounds.width - 2.0 * pad_px).max(1.0);
            let orig_h = (start_bounds.height - 2.0 * pad_px).max(1.0);

            let new_l = next_l + pad_px;
            let new_t = next_t + pad_px;
            let new_w = (next_w - 2.0 * pad_px).max(1.0);
            let new_h = (next_h - 2.0 * pad_px).max(1.0);

            for (pt, bpt) in s.points.iter_mut().zip(base.points.iter()) {
              let bx = bpt.x * w;
              let by = bpt.y * h;
              let rel_x = (bx - orig_l) / orig_w;
              let rel_y = (by - orig_t) / orig_h;
              pt.x = ((new_l + rel_x * new_w) / w).clamp(0.0, 1.0);
              pt.y = ((new_t + rel_y * new_h) / h).clamp(0.0, 1.0);
            }
            true
          }
        }
      }
    }
  }

  /// Finalize the active gesture and record an undo entry if the document changed.
  pub fn pointer_up(&mut self) -> bool {
    let gesture = std::mem::replace(&mut self.gesture, GestureState::Idle);
    let snapshot = self.mutation_snapshot.take();

    match gesture {
      GestureState::Idle => false,
      _ => {
        if let Some(prev) = snapshot {
          if prev != self.document {
            self.push_undo(prev);
            return true;
          }
        }
        false
      }
    }
  }

  /// Delete the currently selected annotation item.
  pub fn delete_selected(&mut self) -> bool {
    let Some(sel) = self.selection.take() else {
      return false;
    };
    let prev = self.document.clone();
    match sel {
      SelectedAnnotation::Stroke(i) if i < self.document.strokes.len() => {
        self.document.strokes.remove(i);
      }
      SelectedAnnotation::Text(i) if i < self.document.texts.len() => {
        self.document.texts.remove(i);
      }
      _ => return false,
    }
    self.push_undo(prev);
    true
  }

  /// Remove all strokes and text/badge annotations cleanly (undoable).
  pub fn clear(&mut self) -> bool {
    if self.document.is_empty() {
      self.selection = None;
      return false;
    }
    let prev = std::mem::replace(&mut self.document, AnnotationDocument::default());
    self.selection = None;
    self.gesture = GestureState::Idle;
    self.push_undo(prev);
    true
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn step_badge_increments_and_supports_undo_redo() {
    let mut engine = AnnotationEngine::new();
    engine.active_tool = Some(AnnotationTool::StepBadge);

    let i1 = engine.place_step_badge(100.0, 100.0, 800.0, 600.0);
    let i2 = engine.place_step_badge(200.0, 150.0, 800.0, 600.0);
    assert_eq!(engine.document.texts[i1].value, "1");
    assert_eq!(engine.document.texts[i2].value, "2");

    assert!(engine.undo());
    assert_eq!(engine.document.texts.len(), 1);
    assert_eq!(engine.next_step_number(), 2);

    assert!(engine.redo());
    assert_eq!(engine.document.texts.len(), 2);
  }

  #[test]
  fn arrow_drawing_and_selection_delete() {
    let mut engine = AnnotationEngine::new();
    engine.active_tool = Some(AnnotationTool::Arrow);
    engine.pointer_down(100.0, 100.0, 800.0, 600.0);
    engine.pointer_move(300.0, 220.0, 800.0, 600.0);
    assert!(engine.pointer_up());
    assert_eq!(engine.document.strokes.len(), 1);

    engine.active_tool = None;
    let out = engine.pointer_down(200.0, 160.0, 800.0, 600.0);
    assert_eq!(out, PointerDownOutcome::Selected(SelectedAnnotation::Stroke(0)));
    engine.pointer_up();
    assert!(engine.delete_selected());
    assert!(engine.document.strokes.is_empty());
  }
}
