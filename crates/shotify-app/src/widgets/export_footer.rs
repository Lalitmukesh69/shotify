use iced::border::Radius;
use iced::widget::{button, column, container, mouse_area, row, slider, svg, text, tooltip};
use iced::{Alignment, Color, Element, Length};
use shotify_export::ExportFormat;

use crate::message::{ExportActionKind, Message};
use crate::state::ShotifyApp;
use crate::theme::{
  dark_slider_style, footer_action_button_style, modal_panel_style, pill_button_style,
  svg_tint_style, tokens,
};
use crate::widgets::common::styled_tooltip;
use crate::widgets::icons::{chevron_down_svg, folder_svg, trash_svg};

const SCALES: &[(f64, &str)] = &[
  (0.5, "0.5x"),
  (1.0, "1x"),
  (1.5, "1.5x"),
  (2.0, "2x"),
  (3.0, "3x"),
];

fn format_scale_badge(scale: f64) -> String {
  if (scale - 0.5).abs() < 0.01 {
    "0.5x".to_string()
  } else if (scale - 1.5).abs() < 0.01 {
    "1.5x".to_string()
  } else {
    format!("{}x", scale.round() as i32)
  }
}

/// Compact bottom-center export bar matching `ui/src/ExportFooter.jsx`.
pub fn export_footer<'a>(app: &'a ShotifyApp) -> Element<'a, Message> {
  let mode = app.theme_mode();
  let t = tokens(mode);
  let has_image = app.raw_screenshot.is_some();
  let can_export = has_image && !app.is_exporting;

  let scale_label = format_scale_badge(app.config.export.scale);
  let fmt_label = match app.config.export.format {
    ExportFormat::Png => "PNG",
    ExportFormat::Jpeg => "JPEG",
    ExportFormat::Webp => "WebP",
  };

  let drawer_active = app.export_drawer_open;
  let drawer_btn = button(
    row![
      text(scale_label).size(12).color(if drawer_active {
        Color::WHITE
      } else {
        t.text_primary
      }),
      text("•").size(12).color(if drawer_active {
        Color::from_rgba(1.0, 1.0, 1.0, 0.70)
      } else {
        t.text_muted
      }),
      text(fmt_label).size(12).color(if drawer_active {
        Color::WHITE
      } else {
        t.text_primary
      }),
      svg(chevron_down_svg())
        .width(13)
        .height(13)
        .style(svg_tint_style(if drawer_active {
          Color::WHITE
        } else {
          t.text_secondary
        })),
    ]
    .spacing(5)
    .align_y(Alignment::Center),
  )
  .on_press(Message::ToggleExportDrawer)
  .padding([5, 9])
  .width(102)
  .height(30)
  .style(pill_button_style(mode, drawer_active, 6.0));

  let copy_btn = button(
    container(text("Copy").size(12))
      .width(Length::Fill)
      .center_x(Length::Fill),
  )
  .on_press_maybe(can_export.then_some(Message::RunExport(ExportActionKind::Clipboard)))
  .padding([5, 10])
  .width(82)
  .height(30)
  .style(footer_action_button_style(mode, false));

  let save_btn = button(
    container(text(if app.is_exporting { "Saving…" } else { "Save" }).size(12))
      .width(Length::Fill)
      .center_x(Length::Fill),
  )
  .on_press_maybe(
    can_export.then_some(Message::RunExport(ExportActionKind::SaveDownloadsOrDialog)),
  )
  .padding([5, 10])
  .width(84)
  .height(30)
  .style(footer_action_button_style(mode, true));

  let folder_tip = app
    .config
    .general
    .save_folder
    .as_ref()
    .map(|p| format!("Change folder (currently: {})", p.display()))
    .unwrap_or_else(|| "Choose folder".to_string());

  let divider_color = if can_export {
    Color::from_rgba(1.0, 1.0, 1.0, 0.35)
  } else {
    t.border
  };

  let split_folder_group = row![
    button(text("Save to folder").size(12))
      .on_press_maybe(can_export.then_some(Message::RunExport(ExportActionKind::SaveToFolder)))
      .padding([5, 11])
      .height(30)
      .style(move |th, st| {
        let mut style = footer_action_button_style(mode, true)(th, st);
        style.border.radius = Radius {
          top_left: 6.0,
          bottom_left: 6.0,
          top_right: 0.0,
          bottom_right: 0.0,
        };
        style
      }),
    container(text(""))
      .width(1)
      .height(22)
      .style(move |_| container::Style {
        background: Some(divider_color.into()),
        ..Default::default()
      }),
    tooltip(
      button(
        container(
          svg(folder_svg())
            .width(14)
            .height(14)
            .style(svg_tint_style(Color::WHITE)),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill),
      )
      .on_press(Message::PickSaveFolderRequested)
      .padding([0, 9])
      .width(32)
      .height(30)
      .style(move |th, st| {
        let mut style = footer_action_button_style(mode, true)(th, st);
        style.border.radius = Radius {
          top_left: 0.0,
          bottom_left: 0.0,
          top_right: 6.0,
          bottom_right: 6.0,
        };
        style
      }),
      styled_tooltip(mode, folder_tip),
      tooltip::Position::Top,
    ),
  ]
  .align_y(Alignment::Center);

  let clear_btn = tooltip(
    button(
      container(
        svg(trash_svg())
          .width(13)
          .height(13)
          .style(svg_tint_style(if has_image {
            t.text_secondary
          } else {
            t.text_disabled
          })),
      )
      .width(Length::Fill)
      .height(Length::Fill)
      .center_x(Length::Fill)
      .center_y(Length::Fill),
    )
    .on_press_maybe(has_image.then_some(Message::ClearCanvas))
    .padding(0)
    .width(30)
    .height(30)
    .style(footer_action_button_style(mode, false)),
    styled_tooltip(mode, "Clear — discard screenshot and return to upload zone"),
    tooltip::Position::Top,
  );

  row![drawer_btn, copy_btn, save_btn, split_folder_group, clear_btn]
    .spacing(6)
    .align_y(Alignment::Center)
    .into()
}

/// Floating export settings popover (`Scale`, `Format`, `Quality`) opened from `[2x • PNG ▾]`.
pub fn export_drawer_popover<'a>(app: &'a ShotifyApp) -> Element<'a, Message> {
  let mode = app.theme_mode();
  let t = tokens(mode);

  let mut scale_row = row![].spacing(4);
  for &(scale, label) in SCALES {
    let active = (app.config.export.scale - scale).abs() < 0.01;
    scale_row = scale_row.push(
      button(container(text(label).size(11)).center_x(Length::Fill))
        .on_press(Message::SelectExportScale(scale))
        .padding([4, 4])
        .width(Length::FillPortion(1))
        .height(28)
        .style(pill_button_style(mode, active, 5.0)),
    );
  }

  let mut fmt_row = row![].spacing(4);
  for fmt in [ExportFormat::Png, ExportFormat::Jpeg, ExportFormat::Webp] {
    let active = app.config.export.format == fmt;
    let label = match fmt {
      ExportFormat::Png => "PNG",
      ExportFormat::Jpeg => "JPEG",
      ExportFormat::Webp => "WebP",
    };
    fmt_row = fmt_row.push(
      button(container(text(label).size(11)).center_x(Length::Fill))
        .on_press(Message::SelectExportFormat(fmt))
        .padding([4, 6])
        .width(Length::FillPortion(1))
        .height(28)
        .style(pill_button_style(mode, active, 5.0)),
    );
  }

  let mut content = column![
    column![text("Scale").size(11).color(t.text_primary), scale_row].spacing(6),
    column![text("Format").size(11).color(t.text_primary), fmt_row].spacing(6),
  ]
  .spacing(12)
  .width(220);

  if app.config.export.format == ExportFormat::Png {
    content = content.push(
      text("PNG is lossless — no compression quality needed")
        .size(10)
        .color(t.text_muted),
    );
  } else {
    content = content.push(
      column![
        row![
          text("Quality").size(11).color(t.text_primary).width(Length::Fill),
          text(format!("{}%", (app.config.export.quality * 100.0).round() as i32))
            .size(11)
            .color(t.text_secondary),
        ],
        slider(0.1..=1.0, app.config.export.quality, Message::ExportQualityChanged)
          .step(0.05)
          .on_release(Message::SliderReleased)
          .width(Length::Fill)
          .style(dark_slider_style(mode)),
      ]
      .spacing(6),
    );
  }

  let drawer = mouse_area(
    container(content)
      .padding(12)
      .style(modal_panel_style(mode, 10.0)),
  )
  .on_press(Message::NoOp);

  // Offset left past 270px sidebar so popover anchors above the workspace footer's drawer button
  mouse_area(
    container(
      column![
        iced::widget::vertical_space(),
        container(drawer).padding(iced::Padding {
          top: 0.0,
          right: 16.0,
          bottom: 54.0,
          left: 310.0,
        }),
      ]
      .width(Length::Fill)
      .height(Length::Fill)
      .align_x(Alignment::Start),
    )
    .width(Length::Fill)
    .height(Length::Fill),
  )
  .on_press(Message::CloseExportDrawer)
  .into()
}
