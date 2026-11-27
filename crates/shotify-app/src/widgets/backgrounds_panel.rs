use iced::widget::{button, column, container, row, slider, svg, text, text_input, tooltip};
use iced::{Alignment, Color, Element, Length};

use crate::config::WallpaperTab;
use crate::message::{ColorPickerTarget, Message};
use crate::state::ShotifyApp;
use crate::theme::{
  dark_slider_style, parse_hex_to_iced_color, pill_button_style, sidebar_section_card_style,
  svg_tint_style, swatch_button_style, tab_pill_button_style, tab_rail_style, tokens,
  validated_input_style, ThemeMode,
};
use crate::widgets::common::{card_section_header, divider_h, styled_tooltip};
use crate::widgets::icons::upload_svg;
use crate::widgets::wallpaper_grid::wallpaper_grid;

fn tab_button<'a>(
  app: &'a ShotifyApp,
  tab: WallpaperTab,
  label: &'a str,
) -> Element<'a, Message> {
  let mode = app.theme_mode();
  button(
    container(text(label).size(11))
      .width(Length::Fill)
      .center_x(Length::Fill),
  )
  .on_press(Message::SelectWallpaperTab(tab))
  .padding([5, 4])
  .width(Length::FillPortion(1))
  .style(tab_pill_button_style(mode, app.wallpaper_tab == tab))
  .into()
}

fn gradient_stop_row<'a>(
  mode: ThemeMode,
  label: &'a str,
  placeholder: &'a str,
  value: &'a str,
  fallback_col: Color,
  target: ColorPickerTarget,
  tip: &'a str,
  on_input: fn(String) -> Message,
) -> Element<'a, Message> {
  let t = tokens(mode);
  let col = parse_hex_to_iced_color(value, fallback_col);
  let is_valid = shotify_core::parse_css_color(value).is_some();

  row![
    text(label).size(11).color(t.text_secondary).width(36),
    tooltip(
      button(container(text("")).width(24).height(24))
        .on_press(Message::OpenColorPicker(target))
        .padding(0)
        .width(24)
        .height(24)
        .style(swatch_button_style(mode, col, false, 7.0)),
      styled_tooltip(mode, tip),
      tooltip::Position::Top,
    ),
    text_input(placeholder, value)
      .on_input(on_input)
      .padding([4, 8])
      .size(11)
      .style(validated_input_style(mode, is_valid))
      .width(Length::Fill),
  ]
  .spacing(7)
  .align_y(Alignment::Center)
  .into()
}

/// Top Backdrop card in Claude modular design language:
/// bordered surface card (`subcard_bg`), header badge, upload action, concentric segmented tabs,
/// centered 7-col wallpaper grid, and gradient stop controls.
pub fn backgrounds_panel<'a>(app: &'a ShotifyApp) -> Element<'a, Message> {
  let mode = app.theme_mode();
  let t = tokens(mode);

  let (tab_badge, group_title) = match app.wallpaper_tab {
    WallpaperTab::System => ("System", "System wallpapers"),
    WallpaperTab::Gradients => ("Gradient", "Gradient presets"),
    WallpaperTab::Colors => ("Color", "Solid palette"),
    WallpaperTab::Auto => ("Auto", "Adaptive palette"),
  };

  let upload_btn = button(
    container(
      row![
        svg(upload_svg())
          .width(13)
          .height(13)
          .style(svg_tint_style(t.text_secondary)),
        text("Upload custom wallpaper").size(12).color(t.text_primary),
      ]
      .spacing(6)
      .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .center_x(Length::Fill),
  )
  .on_press(Message::PickCustomWallpaperRequested)
  .padding([7, 10])
  .width(Length::Fill)
  .style(pill_button_style(mode, false, 8.0));

  let tabs = container(
    row![
      tab_button(app, WallpaperTab::System, "System"),
      tab_button(app, WallpaperTab::Gradients, "Gradient"),
      tab_button(app, WallpaperTab::Colors, "Color"),
      tab_button(app, WallpaperTab::Auto, "Auto"),
    ]
    .spacing(2)
    .width(Length::Fill),
  )
  .padding(3)
  .width(Length::Fill)
  .style(tab_rail_style(mode));

  let mut col = column![
    card_section_header(mode, "Backdrop", Some((tab_badge, true))),
    upload_btn,
    tabs,
    text(group_title).size(11).color(t.text_muted),
    container(wallpaper_grid(app))
      .width(Length::Fill)
      .center_x(Length::Fill),
  ]
  .spacing(9);

  if app.wallpaper_tab == WallpaperTab::Gradients {
    let start_row = gradient_stop_row(
      mode,
      "Start",
      "#7700FF",
      &app.options.custom_theme.color_start,
      t.accent_purple,
      ColorPickerTarget::GradientStart,
      "Pick start color",
      Message::GradientStartChanged,
    );

    let end_row = gradient_stop_row(
      mode,
      "End",
      "#EC4899",
      &app.options.custom_theme.color_end,
      Color::BLACK,
      ColorPickerTarget::GradientEnd,
      "Pick end color",
      Message::GradientEndChanged,
    );

    let angle_valid = app
      .angle_input
      .trim()
      .parse::<f64>()
      .ok()
      .filter(|v| (0.0..=360.0).contains(v))
      .is_some();

    let angle_row = row![
      text("Angle").size(11).color(t.text_secondary).width(36),
      slider(
        0.0..=360.0,
        app.options.custom_theme.angle_deg,
        Message::GradientAngleChanged
      )
      .step(1.0)
      .on_release(Message::SliderReleased)
      .style(dark_slider_style(mode))
      .width(Length::Fill),
      container(
        text_input("135", &app.angle_input)
          .on_input(Message::GradientAngleInputChanged)
          .padding([4, 6])
          .size(11)
          .style(validated_input_style(mode, angle_valid)),
      )
      .width(44),
      text("°").size(11).color(t.text_muted),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    col = col
      .push(divider_h(mode))
      .push(
        column![
          text("Background picker").size(11).color(t.text_secondary),
          start_row,
          end_row,
          angle_row,
        ]
        .spacing(8),
      );
  }

  container(col)
    .padding([12, 11])
    .width(Length::Fill)
    .style(sidebar_section_card_style(mode))
    .into()
}
