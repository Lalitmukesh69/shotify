use iced::widget::{button, column, container, horizontal_space, mouse_area, row, svg, text, tooltip};
use iced::{Alignment, Element, Length};

use crate::message::Message;
use crate::state::ShotifyApp;
use crate::theme::{
  svg_tint_style, tokens, window_control_button_style, window_titlebar_style,
};
use crate::widgets::common::{divider_h, styled_tooltip};
use crate::widgets::icons::{
  window_close_svg, window_maximize_svg, window_minimize_svg, window_restore_svg,
};

fn caption_button<'a>(
  app: &'a ShotifyApp,
  icon: svg::Handle,
  tip: &'a str,
  is_close: bool,
  msg: Message,
) -> Element<'a, Message> {
  let mode = app.theme_mode();
  let t = tokens(mode);

  let btn = button(
    container(
      svg(icon)
        .width(10)
        .height(10)
        .style(svg_tint_style(t.text_secondary)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill),
  )
  .on_press(msg)
  .padding(0)
  .width(26)
  .height(18)
  .style(window_control_button_style(mode, is_close));

  tooltip(btn, styled_tooltip(mode, tip), tooltip::Position::Bottom).into()
}

/// Slim 25px borderless window title bar matching `LinuxTitleBar` in `ui/src/WindowControls.jsx`.
/// Provides drag-to-move, double-click-to-maximize, and Minimize / Maximize / Close controls.
pub fn window_title_bar<'a>(app: &'a ShotifyApp) -> Element<'a, Message> {
  let mode = app.theme_mode();
  let t = tokens(mode);

  let drag_strip = mouse_area(
    row![
      text("shotify").size(11).color(t.text_muted),
      horizontal_space(),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .align_y(Alignment::Center),
  )
  .on_press(Message::WindowTitleBarPressed);

  let max_icon = if app.window_maximized {
    window_restore_svg()
  } else {
    window_maximize_svg()
  };
  let max_tip = if app.window_maximized {
    "Restore"
  } else {
    "Maximize"
  };

  let controls = row![
    caption_button(app, window_minimize_svg(), "Minimize", false, Message::WindowMinimize),
    caption_button(app, max_icon, max_tip, false, Message::WindowToggleMaximize),
    caption_button(app, window_close_svg(), "Close", true, Message::WindowCloseButton),
  ]
  .spacing(3)
  .align_y(Alignment::Center);

  let bar = container(
    row![drag_strip, controls]
      .spacing(8)
      .align_y(Alignment::Center)
      .width(Length::Fill)
      .height(Length::Fill),
  )
  .padding([0, 10])
  .height(25)
  .width(Length::Fill)
  .style(window_titlebar_style(mode));

  column![bar, divider_h(mode)]
    .width(Length::Fill)
    .into()
}
