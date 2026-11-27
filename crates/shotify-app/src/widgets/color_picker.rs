use iced::mouse::Cursor;
use iced::widget::canvas::{self, Canvas, Event, Frame, Geometry, Path};
use iced::widget::{button, column, container, mouse_area, row, text, text_input, tooltip};
use iced::{Alignment, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme};

use crate::message::Message;
use crate::state::ShotifyApp;
use crate::theme::{
  modal_panel_style, parse_hex_to_iced_color, purple_primary_button_style, swatch_button_style,
  tokens, validated_input_style,
};
use crate::widgets::common::styled_tooltip;
use crate::widgets::icons::hsv_to_rgb;

const QUICK_SWATCHES: &[&str] = &[
  "#7700FF", "#EC4899", "#0EA5E9", "#22C55E", "#F59E0B", "#EF4444", "#FFFFFF", "#111827",
];

struct SvSquareProgram {
  hue: f32,
  sat: f32,
  val: f32,
}

#[derive(Default)]
struct DragState {
  dragging: bool,
}

impl canvas::Program<Message> for SvSquareProgram {
  type State = DragState;

  fn update(
    &self,
    state: &mut Self::State,
    event: Event,
    bounds: Rectangle,
    cursor: Cursor,
  ) -> (canvas::event::Status, Option<Message>) {
    match event {
      Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
        if let Some(pos) = cursor.position_in(bounds) {
          state.dragging = true;
          let s = (pos.x / bounds.width.max(1.0)).clamp(0.0, 1.0);
          let v = 1.0 - (pos.y / bounds.height.max(1.0)).clamp(0.0, 1.0);
          return (
            canvas::event::Status::Captured,
            Some(Message::ColorPickerHsvChanged {
              h: self.hue,
              s,
              v,
            }),
          );
        }
      }
      Event::Mouse(iced::mouse::Event::CursorMoved { .. }) if state.dragging => {
        if let Some(pos) = cursor.position() {
          let s = ((pos.x - bounds.x) / bounds.width.max(1.0)).clamp(0.0, 1.0);
          let v = 1.0 - ((pos.y - bounds.y) / bounds.height.max(1.0)).clamp(0.0, 1.0);
          return (
            canvas::event::Status::Captured,
            Some(Message::ColorPickerHsvChanged {
              h: self.hue,
              s,
              v,
            }),
          );
        }
      }
      Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left))
        if state.dragging =>
      {
        state.dragging = false;
        return (canvas::event::Status::Captured, None);
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
    _cursor: Cursor,
  ) -> Vec<Geometry> {
    let mut frame = Frame::new(renderer, bounds.size());
    let cols = 52usize;
    let rows = 32usize;
    let cw = bounds.width / cols as f32;
    let ch = bounds.height / rows as f32;

    for row in 0..rows {
      let v = 1.0 - (row as f32 + 0.5) / rows as f32;
      for col in 0..cols {
        let s = (col as f32 + 0.5) / cols as f32;
        let (r, g, b) = hsv_to_rgb(self.hue, s, v);
        frame.fill_rectangle(
          Point::new(col as f32 * cw, row as f32 * ch),
          Size::new(cw + 0.8, ch + 0.8),
          Color::from_rgb8(r, g, b),
        );
      }
    }

    let kx = self.sat.clamp(0.0, 1.0) * bounds.width;
    let ky = (1.0 - self.val.clamp(0.0, 1.0)) * bounds.height;
    let outer_ring = Path::circle(Point::new(kx, ky), 6.0);
    let inner_ring = Path::circle(Point::new(kx, ky), 4.5);
    frame.stroke(
      &outer_ring,
      canvas::Stroke::default()
        .with_width(1.5)
        .with_color(Color::from_rgba(0.0, 0.0, 0.0, 0.70)),
    );
    frame.stroke(
      &inner_ring,
      canvas::Stroke::default()
        .with_width(1.8)
        .with_color(Color::WHITE),
    );
    vec![frame.into_geometry()]
  }

  fn mouse_interaction(
    &self,
    state: &Self::State,
    bounds: Rectangle,
    cursor: Cursor,
  ) -> iced::mouse::Interaction {
    if state.dragging || cursor.is_over(bounds) {
      iced::mouse::Interaction::Crosshair
    } else {
      iced::mouse::Interaction::default()
    }
  }
}

struct HueBarProgram {
  hue: f32,
  sat: f32,
  val: f32,
}

impl canvas::Program<Message> for HueBarProgram {
  type State = DragState;

  fn update(
    &self,
    state: &mut Self::State,
    event: Event,
    bounds: Rectangle,
    cursor: Cursor,
  ) -> (canvas::event::Status, Option<Message>) {
    match event {
      Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
        if let Some(pos) = cursor.position_in(bounds) {
          state.dragging = true;
          let h = (pos.x / bounds.width.max(1.0)).clamp(0.0, 1.0) * 360.0;
          return (
            canvas::event::Status::Captured,
            Some(Message::ColorPickerHsvChanged {
              h,
              s: self.sat,
              v: self.val,
            }),
          );
        }
      }
      Event::Mouse(iced::mouse::Event::CursorMoved { .. }) if state.dragging => {
        if let Some(pos) = cursor.position() {
          let h = ((pos.x - bounds.x) / bounds.width.max(1.0)).clamp(0.0, 1.0) * 360.0;
          return (
            canvas::event::Status::Captured,
            Some(Message::ColorPickerHsvChanged {
              h,
              s: self.sat,
              v: self.val,
            }),
          );
        }
      }
      Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left))
        if state.dragging =>
      {
        state.dragging = false;
        return (canvas::event::Status::Captured, None);
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
    _cursor: Cursor,
  ) -> Vec<Geometry> {
    let mut frame = Frame::new(renderer, bounds.size());
    let steps = 48usize;
    let sw = bounds.width / steps as f32;
    for i in 0..steps {
      let h = (i as f32 / steps as f32) * 360.0;
      let (r, g, b) = hsv_to_rgb(h, 1.0, 1.0);
      frame.fill_rectangle(
        Point::new(i as f32 * sw, 0.0),
        Size::new(sw + 0.8, bounds.height),
        Color::from_rgb8(r, g, b),
      );
    }
    let kx = (self.hue / 360.0).clamp(0.0, 1.0) * bounds.width;
    let thumb_x = (kx - 2.5).clamp(0.0, (bounds.width - 5.0).max(0.0));
    frame.fill_rectangle(
      Point::new(thumb_x, 0.0),
      Size::new(5.0, bounds.height),
      Color::from_rgba(0.0, 0.0, 0.0, 0.70),
    );
    frame.fill_rectangle(
      Point::new(thumb_x + 1.0, 1.0),
      Size::new(3.0, (bounds.height - 2.0).max(1.0)),
      Color::WHITE,
    );
    vec![frame.into_geometry()]
  }

  fn mouse_interaction(
    &self,
    state: &Self::State,
    bounds: Rectangle,
    cursor: Cursor,
  ) -> iced::mouse::Interaction {
    if state.dragging || cursor.is_over(bounds) {
      iced::mouse::Interaction::Crosshair
    } else {
      iced::mouse::Interaction::default()
    }
  }
}

/// Convert RGB 0..=255 into `(h_deg, sat, val)`.
pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
  let rf = r as f32 / 255.0;
  let gf = g as f32 / 255.0;
  let bf = b as f32 / 255.0;
  let max = rf.max(gf).max(bf);
  let min = rf.min(gf).min(bf);
  let d = max - min;
  let h = if d < 1e-6 {
    0.0
  } else if (max - rf).abs() < 1e-6 {
    60.0 * (((gf - bf) / d).rem_euclid(6.0))
  } else if (max - gf).abs() < 1e-6 {
    60.0 * (((bf - rf) / d) + 2.0)
  } else {
    60.0 * (((rf - gf) / d) + 4.0)
  };
  let s = if max < 1e-6 { 0.0 } else { d / max };
  (h, s, max)
}

/// Floating HSV + Hex color picker overlay triggered when clicking any color swatch.
pub fn color_picker_overlay<'a>(app: &'a ShotifyApp) -> Element<'a, Message> {
  let mode = app.theme_mode();
  let t = tokens(mode);
  let (h, s, v) = app.color_picker_hsv;
  let preview_col = parse_hex_to_iced_color(&app.color_picker_hex_draft, t.accent_purple);
  let hex_valid = shotify_core::parse_css_color(&app.color_picker_hex_draft).is_some();

  let mut quick_row = row![].spacing(5).align_y(Alignment::Center);
  for &hex in QUICK_SWATCHES {
    let selected = app.color_picker_hex_draft.eq_ignore_ascii_case(hex);
    quick_row = quick_row.push(tooltip(
      button(container(text("")).width(21).height(21))
        .on_press(Message::ColorPickerHexChanged(hex.to_string()))
        .padding(0)
        .width(21)
        .height(21)
        .style(swatch_button_style(
          mode,
          parse_hex_to_iced_color(hex, Color::WHITE),
          selected,
          999.0,
        )),
      styled_tooltip(mode, hex),
      tooltip::Position::Top,
    ));
  }

  let card = container(
    column![
      row![
        text("Pick Color").size(12).color(t.text_primary).width(Length::Fill),
        button(text("Done").size(11))
          .on_press(Message::CloseColorPicker)
          .padding([3, 10])
          .style(purple_primary_button_style(mode, 6.0)),
      ]
      .align_y(Alignment::Center),
      Canvas::new(SvSquareProgram { hue: h, sat: s, val: v })
        .width(204)
        .height(126),
      Canvas::new(HueBarProgram { hue: h, sat: s, val: v })
        .width(204)
        .height(14),
      quick_row,
      row![
        container(text(""))
          .width(24)
          .height(24)
          .style(move |_| container::Style {
            background: Some(preview_col.into()),
            border: iced::Border {
              color: t.border_strong,
              width: 1.0,
              radius: 6.0.into(),
            },
            ..Default::default()
          }),
        text_input("#7700FF", &app.color_picker_hex_draft)
          .on_input(Message::ColorPickerHexChanged)
          .padding([5, 8])
          .size(12)
          .style(validated_input_style(mode, hex_valid)),
      ]
      .spacing(8)
      .align_y(Alignment::Center),
    ]
    .spacing(10)
    .width(204),
  )
  .padding(14)
  .style(modal_panel_style(mode, 14.0));

  mouse_area(
    container(mouse_area(card).on_press(Message::NoOp))
      .width(Length::Fill)
      .height(Length::Fill)
      .center_x(Length::Fill)
      .center_y(Length::Fill)
      .style(|_| container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.25).into()),
        ..Default::default()
      }),
  )
  .on_press(Message::CloseColorPicker)
  .into()
}
