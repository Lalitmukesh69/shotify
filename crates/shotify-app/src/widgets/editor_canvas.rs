use iced::mouse;
use iced::widget::canvas;
use iced::widget::canvas::{path::Builder, Event, Frame, Geometry, Path, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme};
use shotify_annotate::{
  selection_bounds_px, AnnotationDocument, AnnotationTool, ResizeHandle, StrokeKind,
};

use crate::message::Message;

/// Interactive vector overlay Canvas program sitting directly atop the rendered composition image.
pub struct AnnotationCanvasProgram<'a> {
  pub document: &'a AnnotationDocument,
  pub active_tool: Option<AnnotationTool>,
  pub selection: Option<shotify_annotate::SelectedAnnotation>,
  pub card_dragging: bool,
  pub doc_w: f64,
  pub doc_h: f64,
}

#[derive(Debug, Default)]
pub struct CanvasInteractionState {
  pointer_pressed: bool,
  last_click_time: Option<std::time::Instant>,
  last_click_pos: Option<Point>,
  last_drag_pos: Option<Point>,
  dragging_card: bool,
}

impl<'a> canvas::Program<Message, Theme, Renderer> for AnnotationCanvasProgram<'a> {
  type State = CanvasInteractionState;

  fn update(
    &self,
    state: &mut Self::State,
    event: Event,
    bounds: Rectangle,
    cursor: mouse::Cursor,
  ) -> (canvas::event::Status, Option<Message>) {
    let Event::Mouse(mouse_ev) = event else {
      return (canvas::event::Status::Ignored, None);
    };

    let scale_x = if bounds.width > 0.0 {
      self.doc_w / f64::from(bounds.width)
    } else {
      1.0
    };
    let scale_y = if bounds.height > 0.0 {
      self.doc_h / f64::from(bounds.height)
    } else {
      1.0
    };

    match mouse_ev {
      mouse::Event::ButtonPressed(mouse::Button::Left) => {
        let Some(local) = cursor.position_in(bounds) else {
          return (canvas::event::Status::Ignored, None);
        };
        let px = f64::from(local.x) * scale_x;
        let py = f64::from(local.y) * scale_y;

        // Detect double-click for editing existing text annotation
        let now = std::time::Instant::now();
        let is_double = match (state.last_click_time, state.last_click_pos) {
          (Some(prev_t), Some(prev_p)) => {
            now.duration_since(prev_t).as_millis() <= 350
              && ((local.x - prev_p.x).hypot(local.y - prev_p.y) < 8.0)
          }
          _ => false,
        };
        state.last_click_time = Some(now);
        state.last_click_pos = Some(local);

        if is_double && self.active_tool.is_none() {
          return (
            canvas::event::Status::Captured,
            Some(Message::AnnotationDoubleClick {
              px,
              py,
              doc_w: self.doc_w,
              doc_h: self.doc_h,
            }),
          );
        }

        state.pointer_pressed = true;
        state.last_drag_pos = Some(local);

        // If no annotation tool active AND no annotation/handle hits this point, drag the screenshot card
        if self.active_tool.is_none() {
          let hit_handle = self
            .selection
            .and_then(|sel| selection_bounds_px(self.document, self.doc_w, self.doc_h, sel))
            .and_then(|b| shotify_annotate::hit_test_resize_handle(b, px, py, 9.0))
            .is_some();
          let hit_item =
            shotify_annotate::hit_test_document(self.document, self.doc_w, self.doc_h, px, py)
              .is_some();
          if !hit_handle && !hit_item {
            state.dragging_card = true;
            return (
              canvas::event::Status::Captured,
              Some(Message::AnnotationPointerDown {
                px,
                py,
                doc_w: self.doc_w,
                doc_h: self.doc_h,
              }),
            );
          }
        }

        state.dragging_card = false;
        (
          canvas::event::Status::Captured,
          Some(Message::AnnotationPointerDown {
            px,
            py,
            doc_w: self.doc_w,
            doc_h: self.doc_h,
          }),
        )
      }
      mouse::Event::CursorMoved { .. } => {
        if !state.pointer_pressed {
          return (canvas::event::Status::Ignored, None);
        }
        let local = cursor.position_from(bounds.position()).unwrap_or_else(|| {
          Point::new(0.0, 0.0)
        });

        if state.dragging_card {
          let prev = state.last_drag_pos.unwrap_or(local);
          state.last_drag_pos = Some(local);
          let dx = f64::from(local.x - prev.x) * scale_x;
          let dy = f64::from(local.y - prev.y) * scale_y;
          return (
            canvas::event::Status::Captured,
            Some(Message::CardDraggedDelta(dx, dy)),
          );
        }

        let px = f64::from(local.x.clamp(0.0, bounds.width)) * scale_x;
        let py = f64::from(local.y.clamp(0.0, bounds.height)) * scale_y;
        (
          canvas::event::Status::Captured,
          Some(Message::AnnotationPointerMove {
            px,
            py,
            doc_w: self.doc_w,
            doc_h: self.doc_h,
          }),
        )
      }
      mouse::Event::ButtonReleased(mouse::Button::Left) => {
        if !state.pointer_pressed {
          return (canvas::event::Status::Ignored, None);
        }
        let was_card = state.dragging_card;
        state.pointer_pressed = false;
        state.dragging_card = false;
        state.last_drag_pos = None;
        if was_card {
          (canvas::event::Status::Captured, Some(Message::SliderReleased))
        } else {
          (
            canvas::event::Status::Captured,
            Some(Message::AnnotationPointerUp),
          )
        }
      }
      _ => (canvas::event::Status::Ignored, None),
    }
  }

  fn draw(
    &self,
    _state: &Self::State,
    renderer: &Renderer,
    _theme: &Theme,
    bounds: Rectangle,
    _cursor: mouse::Cursor,
  ) -> Vec<Geometry<Renderer>> {
    let mut frame = Frame::new(renderer, bounds.size());
    let sx = if self.doc_w > 0.0 {
      bounds.width / (self.doc_w as f32)
    } else {
      1.0
    };
    let sy = if self.doc_h > 0.0 {
      bounds.height / (self.doc_h as f32)
    } else {
      1.0
    };

    // Draw live freehand preview stroke smoothly on GPU/canvas in case of active drag
    if self.active_tool == Some(AnnotationTool::Pen) {
      if let Some(last_pen) = self.document.strokes.last().filter(|s| s.kind == StrokeKind::Pen) {
        if last_pen.points.len() >= 2 {
          let mut builder = Builder::new();
          let p0 = last_pen.points[0];
          builder.move_to(Point::new(
            (p0.x as f32) * bounds.width,
            (p0.y as f32) * bounds.height,
          ));
          for p in &last_pen.points[1..] {
            builder.line_to(Point::new(
              (p.x as f32) * bounds.width,
              (p.y as f32) * bounds.height,
            ));
          }
          let col = crate::theme::parse_hex_to_iced_color(&last_pen.color, Color::WHITE);
          let w = (last_pen.width_rel * self.doc_w.min(self.doc_h)).max(1.0) as f32 * sx;
          frame.stroke(
            &builder.build(),
            Stroke::default().with_color(col).with_width(w),
          );
        }
      }
    }

    // Draw selection box + 8 white/purple handles matching CanvasAnnotations.jsx
    if self.active_tool.is_none() {
      if let Some(sel) = self.selection {
        if let Some(bpx) = selection_bounds_px(self.document, self.doc_w, self.doc_h, sel) {
          let rect = Path::rectangle(
            Point::new((bpx.x as f32) * sx, (bpx.y as f32) * sy),
            Size::new((bpx.width as f32) * sx, (bpx.height as f32) * sy),
          );
          frame.stroke(
            &rect,
            Stroke::default()
              .with_color(Color::from_rgba8(0x77, 0x00, 0xFF, 0.85))
              .with_width(1.6),
          );

          for handle in ResizeHandle::ALL {
            let (hx, hy) = bpx.handle_center(handle);
            let center = Point::new((hx as f32) * sx, (hy as f32) * sy);
            let circle = Path::circle(center, 5.0);
            frame.fill(&circle, Color::WHITE);
            frame.stroke(
              &circle,
              Stroke::default()
                .with_color(Color::from_rgba8(0x77, 0x00, 0xFF, 0.92))
                .with_width(1.6),
            );
          }
        }
      }
    }

    // Center alignment guides while dragging card
    if self.card_dragging {
      let mid_x = bounds.width / 2.0;
      let mid_y = bounds.height / 2.0;
      let vline = Path::line(Point::new(mid_x, 0.0), Point::new(mid_x, bounds.height));
      let hline = Path::line(Point::new(0.0, mid_y), Point::new(bounds.width, mid_y));
      let guide = Stroke::default()
        .with_color(Color::from_rgba8(0x77, 0x00, 0xFF, 0.35))
        .with_width(1.0);
      frame.stroke(&vline, guide);
      frame.stroke(&hline, guide);
    }

    vec![frame.into_geometry()]
  }

  fn mouse_interaction(
    &self,
    _state: &Self::State,
    bounds: Rectangle,
    cursor: mouse::Cursor,
  ) -> mouse::Interaction {
    if !cursor.is_over(bounds) {
      return mouse::Interaction::default();
    }
    if self.active_tool.is_some() {
      return mouse::Interaction::Crosshair;
    }
    mouse::Interaction::Grab
  }
}
