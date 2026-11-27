use iced::mouse;
use iced::widget::canvas::{Event, Frame, Geometry, Path, Stroke};
use iced::widget::{button, canvas, column, container, image as iced_image, row, stack, text};
use iced::{Alignment, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme};

use crate::message::Message;
use crate::state::ShotifyApp;
use crate::theme::{floating_panel_style, pill_button_style, tokens};

struct CaptureSelectionProgram {
  selection: Option<(Point, Point)>,
}

#[derive(Default)]
struct CaptureDragState {
  dragging: bool,
}

impl canvas::Program<Message, Theme, Renderer> for CaptureSelectionProgram {
  type State = CaptureDragState;

  fn update(
    &self,
    state: &mut Self::State,
    event: Event,
    bounds: Rectangle,
    cursor: mouse::Cursor,
  ) -> (canvas::event::Status, Option<Message>) {
    let Event::Mouse(ev) = event else {
      return (canvas::event::Status::Ignored, None);
    };
    match ev {
      mouse::Event::ButtonPressed(mouse::Button::Left) => {
        if let Some(pos) = cursor.position_in(bounds) {
          let norm = Point::new(
            (pos.x / bounds.width.max(1.0)).clamp(0.0, 1.0),
            (pos.y / bounds.height.max(1.0)).clamp(0.0, 1.0),
          );
          state.dragging = true;
          return (
            canvas::event::Status::Captured,
            Some(Message::CaptureDragStart(norm)),
          );
        }
      }
      mouse::Event::CursorMoved { .. } => {
        if state.dragging {
          let pos = cursor.position_from(bounds.position()).unwrap_or_default();
          let norm = Point::new(
            (pos.x / bounds.width.max(1.0)).clamp(0.0, 1.0),
            (pos.y / bounds.height.max(1.0)).clamp(0.0, 1.0),
          );
          return (
            canvas::event::Status::Captured,
            Some(Message::CaptureDragMove(norm)),
          );
        }
      }
      mouse::Event::ButtonReleased(mouse::Button::Left) => {
        if state.dragging {
          state.dragging = false;
          let pos = cursor.position_from(bounds.position()).unwrap_or_default();
          let norm = Point::new(
            (pos.x / bounds.width.max(1.0)).clamp(0.0, 1.0),
            (pos.y / bounds.height.max(1.0)).clamp(0.0, 1.0),
          );
          return (
            canvas::event::Status::Captured,
            Some(Message::CaptureDragEnd(norm)),
          );
        }
      }
      _ => {}
    }
    (canvas::event::Status::Ignored, None)
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
    let dim = Color::from_rgba(0.0, 0.0, 0.0, 0.48);

    if let Some((a, b)) = self.selection {
      let x0 = a.x.min(b.x) * bounds.width;
      let y0 = a.y.min(b.y) * bounds.height;
      let x1 = a.x.max(b.x) * bounds.width;
      let y1 = a.y.max(b.y) * bounds.height;

      // Dim outside selection rectangle using 4 surrounding strips
      frame.fill_rectangle(Point::new(0.0, 0.0), Size::new(bounds.width, y0), dim);
      frame.fill_rectangle(
        Point::new(0.0, y1),
        Size::new(bounds.width, (bounds.height - y1).max(0.0)),
        dim,
      );
      frame.fill_rectangle(
        Point::new(0.0, y0),
        Size::new(x0, (y1 - y0).max(0.0)),
        dim,
      );
      frame.fill_rectangle(
        Point::new(x1, y0),
        Size::new((bounds.width - x1).max(0.0), (y1 - y0).max(0.0)),
        dim,
      );

      let sel_rect = Path::rectangle(
        Point::new(x0, y0),
        Size::new((x1 - x0).max(1.0), (y1 - y0).max(1.0)),
      );
      // Outer high-contrast white hairline + inner brand purple #8A1AFF stroke
      frame.stroke(
        &sel_rect,
        Stroke::default()
          .with_color(Color::from_rgba(1.0, 1.0, 1.0, 0.75))
          .with_width(3.0),
      );
      frame.stroke(
        &sel_rect,
        Stroke::default()
          .with_color(Color::from_rgb8(0x8A, 0x1A, 0xFF))
          .with_width(1.75),
      );
    } else {
      frame.fill_rectangle(Point::new(0.0, 0.0), bounds.size(), dim);
    }

    vec![frame.into_geometry()]
  }

  fn mouse_interaction(
    &self,
    _state: &Self::State,
    _bounds: Rectangle,
    _cursor: mouse::Cursor,
  ) -> mouse::Interaction {
    mouse::Interaction::Crosshair
  }
}

/// Interactive desktop capture region selection overlay.
pub fn capture_overlay_view<'a>(app: &'a ShotifyApp) -> Element<'a, Message> {
  let mode = app.theme_mode();
  let t = tokens(mode);

  let Some(handle) = app.capture_handle.clone() else {
    return container(
      column![
        text("Preparing desktop capture…").size(15).color(t.text_primary),
        button(text("Cancel").size(12))
          .on_press(Message::CancelCaptureOverlay)
          .padding([6, 14])
          .style(pill_button_style(mode, false, 8.0)),
      ]
      .spacing(12)
      .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into();
  };

  let bg = iced_image(handle)
    .width(Length::Fill)
    .height(Length::Fill);

  let selector = canvas(CaptureSelectionProgram {
    selection: app.capture_selection,
  })
  .width(Length::Fill)
  .height(Length::Fill);

  let prompt_text = match (app.capture_selection, app.capture_raw_screen.as_ref()) {
    (Some((a, b)), Some(screen)) => {
      let (sw, sh) = screen.dimensions();
      let cw = ((a.x.max(b.x) - a.x.min(b.x)) * (sw as f32)).round() as u32;
      let ch = ((a.y.max(b.y) - a.y.min(b.y)) * (sh as f32)).round() as u32;
      format!("Selection: {cw} × {ch} px — release to capture")
    }
    _ => "Drag across a screen region to capture (release to open in Shotify)".to_string(),
  };

  let banner = container(
    container(
      row![
        text(prompt_text).size(12).color(t.text_primary),
        button(text("Cancel (Esc)").size(11))
          .on_press(Message::CancelCaptureOverlay)
          .padding([4, 10])
          .style(pill_button_style(mode, false, 6.0)),
      ]
      .spacing(10)
      .align_y(Alignment::Center),
    )
    .padding([8, 14])
    .style(floating_panel_style(mode, 999.0)),
  )
  .width(Length::Fill)
  .padding(16)
  .center_x(Length::Fill);

  stack![bg, selector, banner].into()
}
