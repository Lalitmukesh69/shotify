use iced::widget::{button, container, row, svg, tooltip};
use iced::{Alignment, Color, Element, Length};
use shotify_annotate::{AnnotationTool, ShapeMode};

use crate::message::{ColorPickerTarget, Message};
use crate::state::ShotifyApp;
use crate::theme::{
  annotation_icon_button_style, floating_panel_style, parse_hex_to_iced_color, svg_tint_style,
  swatch_button_style, tokens,
};
use crate::widgets::common::{divider_v, styled_tooltip};
use crate::widgets::icons::{
  arrow_up_right_svg, circle_svg, pen_svg, redo_svg, rotate_ccw_svg, square_svg, step_one_svg,
  type_svg, undo_svg,
};

fn icon_tool_btn<'a>(
  app: &'a ShotifyApp,
  tool: AnnotationTool,
  icon_handle: svg::Handle,
  tip: &'a str,
) -> Element<'a, Message> {
  let enabled = app.raw_screenshot.is_some();
  let active = enabled && app.annotations.active_tool == Some(tool);
  let next = if active { None } else { Some(tool) };
  let mode = app.theme_mode();
  let t = tokens(mode);

  let icon_color = if active {
    Color::WHITE
  } else if enabled {
    t.text_primary
  } else {
    t.text_disabled
  };

  let btn = button(
    container(
      svg(icon_handle)
        .width(16)
        .height(16)
        .style(svg_tint_style(icon_color)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill),
  )
  .on_press_maybe(enabled.then_some(Message::SelectAnnotationTool(next)))
  .padding(0)
  .width(34)
  .height(34)
  .style(annotation_icon_button_style(mode, active, 999.0));

  tooltip(btn, styled_tooltip(mode, tip), tooltip::Position::Bottom).into()
}

/// Top-center 44px floating annotation bar matching `ui/src/AnnotationBar.jsx`.
pub fn annotation_toolbar<'a>(app: &'a ShotifyApp) -> Element<'a, Message> {
  let mode = app.theme_mode();
  let t = tokens(mode);
  let enabled = app.raw_screenshot.is_some();
  let has_annotations = !app.annotations.document.is_empty();
  let active_color = parse_hex_to_iced_color(&app.annotations.active_color, t.accent_purple);

  let shape_icon = match app.annotations.shape_mode {
    ShapeMode::Rect => square_svg(),
    ShapeMode::Ellipse => circle_svg(),
  };

  let mut bar = row![
    icon_tool_btn(app, AnnotationTool::Pen, pen_svg(), "Pen"),
    icon_tool_btn(app, AnnotationTool::Text, type_svg(), "Text"),
    icon_tool_btn(app, AnnotationTool::Arrow, arrow_up_right_svg(), "Arrow"),
    icon_tool_btn(app, AnnotationTool::Shape, shape_icon, "Shape"),
  ]
  .spacing(4)
  .align_y(Alignment::Center);

  if enabled && app.annotations.active_tool == Some(AnnotationTool::Shape) {
    let rect_active = app.annotations.shape_mode == ShapeMode::Rect;
    let circle_active = app.annotations.shape_mode == ShapeMode::Ellipse;

    bar = bar.push(
      row![
        tooltip(
          button(
            container(
              svg(square_svg())
                .width(14)
                .height(14)
                .style(svg_tint_style(if rect_active {
                  Color::WHITE
                } else {
                  t.text_primary
                })),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
          )
          .on_press(Message::SetShapeMode(ShapeMode::Rect))
          .padding([6, 6])
          .width(28)
          .height(28)
          .style(annotation_icon_button_style(mode, rect_active, 6.0)),
          styled_tooltip(mode, "Rectangle"),
          tooltip::Position::Bottom,
        ),
        tooltip(
          button(
            container(
              svg(circle_svg())
                .width(14)
                .height(14)
                .style(svg_tint_style(if circle_active {
                  Color::WHITE
                } else {
                  t.text_primary
                })),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
          )
          .on_press(Message::SetShapeMode(ShapeMode::Ellipse))
          .padding([6, 6])
          .width(28)
          .height(28)
          .style(annotation_icon_button_style(mode, circle_active, 6.0)),
          styled_tooltip(mode, "Ellipse"),
          tooltip::Position::Bottom,
        ),
      ]
      .spacing(2)
      .align_y(Alignment::Center),
    );
  }

  let color_btn = tooltip(
    button(iced::widget::container(iced::widget::text("")).width(20).height(20))
      .on_press_maybe(
        enabled.then_some(Message::OpenColorPicker(ColorPickerTarget::AnnotationColor)),
      )
      .padding(0)
      .width(20)
      .height(20)
      .style(swatch_button_style(mode, active_color, false, 999.0)),
    styled_tooltip(mode, "Annotation color"),
    tooltip::Position::Bottom,
  );

  let can_undo = enabled && app.annotations.can_undo();
  let can_redo = enabled && app.annotations.can_redo();
  let can_clear = enabled && has_annotations;

  let undo_tip = if cfg!(target_os = "macos") {
    "Undo (Cmd+Z)"
  } else {
    "Undo (Ctrl+Z)"
  };
  let redo_tip = if cfg!(target_os = "macos") {
    "Redo (Cmd+Shift+Z)"
  } else {
    "Redo (Ctrl+Y)"
  };

  let undo_btn = tooltip(
    button(
      container(
        svg(undo_svg())
          .width(15)
          .height(15)
          .style(svg_tint_style(if can_undo {
            t.text_primary
          } else {
            t.text_disabled
          })),
      )
      .width(Length::Fill)
      .height(Length::Fill)
      .center_x(Length::Fill)
      .center_y(Length::Fill),
    )
    .on_press_maybe(can_undo.then_some(Message::AnnotationUndo))
    .padding(0)
    .width(30)
    .height(30)
    .style(annotation_icon_button_style(mode, false, 999.0)),
    styled_tooltip(mode, undo_tip),
    tooltip::Position::Bottom,
  );

  let redo_btn = tooltip(
    button(
      container(
        svg(redo_svg())
          .width(15)
          .height(15)
          .style(svg_tint_style(if can_redo {
            t.text_primary
          } else {
            t.text_disabled
          })),
      )
      .width(Length::Fill)
      .height(Length::Fill)
      .center_x(Length::Fill)
      .center_y(Length::Fill),
    )
    .on_press_maybe(can_redo.then_some(Message::AnnotationRedo))
    .padding(0)
    .width(30)
    .height(30)
    .style(annotation_icon_button_style(mode, false, 999.0)),
    styled_tooltip(mode, redo_tip),
    tooltip::Position::Bottom,
  );

  let clear_btn = tooltip(
    button(
      container(
        svg(rotate_ccw_svg())
          .width(15)
          .height(15)
          .style(svg_tint_style(if can_clear {
            t.text_primary
          } else {
            t.text_disabled
          })),
      )
      .width(Length::Fill)
      .height(Length::Fill)
      .center_x(Length::Fill)
      .center_y(Length::Fill),
    )
    .on_press_maybe(can_clear.then_some(Message::AnnotationClearAll))
    .padding(0)
    .width(32)
    .height(32)
    .style(annotation_icon_button_style(mode, false, 999.0)),
    styled_tooltip(mode, "Clear all annotations"),
    tooltip::Position::Bottom,
  );

  bar = bar
    .push(icon_tool_btn(
      app,
      AnnotationTool::StepBadge,
      step_one_svg(),
      "Step Badge",
    ))
    .push(container(divider_v(mode, 20.0)).padding([0, 3]))
    .push(container(color_btn).padding([0, 4]).align_y(Alignment::Center))
    .push(undo_btn)
    .push(redo_btn)
    .push(clear_btn);

  container(bar)
    .height(44)
    .padding([5, 5])
    .align_y(Alignment::Center)
    .style(floating_panel_style(mode, 100.0))
    .into()
}
